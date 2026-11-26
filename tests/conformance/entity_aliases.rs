//! SPEC-0048 / TC-0052: the same alias assertions run on memory and SQLite.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("concrete adapter unexpectedly suspended"),
    }
}
fn fields(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(value.into()))])
}
fn schema() -> SchemaDefinition {
    let table = TableDefinition {
        columns: BTreeMap::from([(
            "body".into(),
            Column {
                kind: ColumnType::String,
                optional: false,
            },
        )]),
        relations: BTreeMap::new(),
    };
    let mut nodes = table.clone();
    nodes.relations.insert(
        "parent".into(),
        RelationDefinition::ToOne {
            table: "nodes".into(),
            foreign_key: "parent_id".into(),
            optional: true,
        },
    );
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            ("nodes".into(), nodes),
            ("other".into(), table.clone()),
            ("local".into(), table),
        ]),
    }
}
fn configure<S: SyncStorage>(db: &mut CommandDatabase<S>) {
    db.configure_entities(
        EntityConfig {
            scope: "alias-fixture".into(),
            tables: BTreeSet::from(["nodes".into(), "other".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1000,
            })
        },
    )
    .unwrap();
}
fn open<S: SyncStorage>(storage: S, capacity: usize) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [0x72; 16],
        SyncOptions {
            capacity,
            ..Default::default()
        },
    ))
    .unwrap()
}
fn insert(alias: Option<&str>, parent: Option<&str>) -> AliasedWrite {
    AliasedWrite {
        write: Write::Insert {
            table: "nodes".into(),
            fields: fields("p"),
        },
        patch: false,
        proposed_id: None,
        alias: alias.map(str::to_owned),
        references: parent
            .map(|name| BTreeMap::from([("parent_id".into(), name.into())]))
            .unwrap_or_default(),
    }
}
fn plain(write: Write) -> AliasedWrite {
    AliasedWrite {
        write,
        patch: false,
        proposed_id: None,
        alias: None,
        references: BTreeMap::new(),
    }
}
fn envelope(batch: &PushBatch) -> BTreeMap<String, Json> {
    assert_eq!(batch.envelopes.len(), 1);
    let Json::Object(value) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("envelope")
    };
    value
}

pub fn atomic_and_recovery<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = open(fresh(), 1000);
    for id in ["existing", "removed"] {
        run(db.insert_proposed("nodes", fields(id), Some(id.into()))).unwrap();
    }
    configure(&mut db);
    let query = db.query("nodes");
    run(db.observe(&query)).unwrap();
    db.drain_notifications(0);
    let result = run(db.apply_entity_batch_aliased(vec![
        insert(Some("p"), None),
        insert(Some("c"), Some("p")),
        insert(None, Some("p")),
        AliasedWrite {
            write: Write::Replace {
                table: "nodes".into(),
                id: "existing".into(),
                fields: fields("updated"),
            },
            references: BTreeMap::from([("parent_id".into(), "p".into())]),
            ..plain(Write::Remove {
                table: String::new(),
                id: String::new(),
            })
        },
        plain(Write::Remove {
            table: "nodes".into(),
            id: "removed".into(),
        }),
        AliasedWrite {
            proposed_id: Some("marker".into()),
            ..plain(Write::Insert {
                table: "local".into(),
                fields: fields("complete"),
            })
        },
    ]))
    .unwrap();
    let parent = result.aliases["p"].clone();
    let child = result.aliases["c"].clone();
    assert!(parent.starts_with("entity:72727272727272727272727272727272:"));
    assert!(parent.ends_with("0000000000000001"));
    assert_ne!(parent, child);
    assert_eq!(result.changes.len(), 6);
    let rows = run(query.fetch(&db)).unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows.iter().filter(|row| row.id != parent) {
        assert_eq!(row.fields["parent_id"], Value::Reference(parent.clone()));
    }
    assert_eq!(
        rows.iter().find(|row| row.id == child).unwrap().fields["body"],
        Value::String("p".into())
    );
    let events = db.drain_notifications(1);
    assert_eq!(events.len(), 1, "single atomic query publication");
    assert_eq!(
        db.queue_status().live,
        5,
        "local marker has no outbound record"
    );
    let status = db.queue_status();
    assert!(
        status
            .commands
            .iter()
            .filter(|row| row.record_id.as_deref() != Some(&parent)
                && row.record_id.as_deref() != Some("removed"))
            .all(|row| row.blocked_on == vec![parent.clone()])
    );

    // Deletion is independent; isolate the parent's send/confirmation by exact tuple.
    let batch = db.prepare_push(1000, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 2);
    let parent_envelope = batch
        .envelopes
        .iter()
        .map(|body| Json::parse(body).unwrap())
        .find_map(|value| {
            let Json::Object(value) = value else {
                return None;
            };
            (value["recordId"] == Json::String(parent.clone())).then_some(value)
        })
        .unwrap();
    assert_eq!(parent_envelope["op"], Json::String("create".into()));
    let Json::String(key) = &parent_envelope["writeId"] else {
        panic!("writeId")
    };
    db.receive_entity(
        "nodes",
        &parent,
        key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        key,
        ReconcilePlan {
            mappings: BTreeMap::from([(parent.clone(), "canonical-parent".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let mut db = open(db.into_storage(), 1000);
    configure(&mut db);
    let rows = run(db.query("nodes").fetch(&db)).unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().any(|row| row.id == "canonical-parent"));
    for row in rows.iter().filter(|row| row.id != "canonical-parent") {
        assert_eq!(
            row.fields["parent_id"],
            Value::Reference("canonical-parent".into())
        );
    }
    let pending = db.prepare_push(1000, 0).unwrap();
    assert_eq!(pending.envelopes.len(), 3);
    for body in pending.envelopes {
        let Json::Object(value) = Json::parse(&body).unwrap() else {
            panic!("object")
        };
        let Json::Object(changes) = &value["changes"] else {
            panic!("changes")
        };
        assert_eq!(
            changes["parent_id"],
            Json::String("canonical-parent".into())
        );
        assert!(!body.contains("\"alias\""));
    }
}

pub fn rejection_is_atomic<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = open(fresh(), 1000);
    configure(&mut db);
    let query = db.query("nodes");
    run(db.observe(&query)).unwrap();
    db.drain_notifications(0);
    let mut duplicate = insert(Some("p"), None);
    duplicate.references.insert("parent_id".into(), "p".into());
    let mut wrong_kind = insert(Some("c"), None);
    wrong_kind.references.insert("body".into(), "p".into());
    let mut overlap = insert(Some("c"), Some("p"));
    let Write::Insert {
        fields,
        ..
    } = &mut overlap.write
    else {
        unreachable!()
    };
    fields.insert("parent_id".into(), Value::Null);
    let mut wrong_table = insert(Some("p"), None);
    let Write::Insert {
        table,
        ..
    } = &mut wrong_table.write
    else {
        unreachable!()
    };
    *table = "other".into();
    let mut local_alias = insert(Some("p"), None);
    let Write::Insert {
        table,
        ..
    } = &mut local_alias.write
    else {
        unreachable!()
    };
    *table = "local".into();
    let mut proposed = insert(Some("p"), None);
    proposed.proposed_id = Some("entity:forged".into());
    let mut malformed = insert(None, Some("p"));
    let Write::Insert {
        fields,
        ..
    } = &mut malformed.write
    else {
        unreachable!()
    };
    fields.clear();
    let cases = vec![
        vec![insert(Some("p"), None), duplicate],
        vec![insert(Some(""), None)],
        vec![insert(Some("c"), Some("missing"))],
        vec![insert(Some("c"), Some("p")), insert(Some("p"), None)],
        vec![insert(Some("p"), None), wrong_kind],
        vec![insert(Some("p"), None), overlap],
        vec![wrong_table, insert(None, Some("p"))],
        vec![local_alias],
        vec![proposed],
        vec![insert(Some("p"), None), malformed],
    ];
    for writes in cases {
        assert!(run(db.apply_entity_batch_aliased(writes)).is_err());
        assert!(run(query.fetch(&db)).unwrap().is_empty());
        assert!(run(db.query("other").fetch(&db)).unwrap().is_empty());
        assert!(run(db.query("local").fetch(&db)).unwrap().is_empty());
        assert_eq!(db.queue_status().live, 0);
        assert!(db.drain_notifications(1).is_empty());
    }
    let result = run(db.apply_entity_batch_aliased(vec![insert(Some("p"), None)])).unwrap();
    assert!(result.aliases["p"].ends_with("0000000000000001"));
    assert_eq!(
        envelope(&db.prepare_push(1000, 0).unwrap())["recordId"],
        Json::String(result.aliases["p"].clone())
    );

    let mut db = open(fresh(), 1);
    configure(&mut db);
    assert!(matches!(
        run(db.apply_entity_batch_aliased(vec![insert(Some("p"), None), insert(None, Some("p"))])),
        Err(CommandError::Capacity)
    ));
    assert!(run(db.query("nodes").fetch(&db)).unwrap().is_empty());
    assert_eq!(db.queue_status().live, 0);
    let result = run(db.apply_entity_batch_aliased(vec![insert(Some("p"), None)])).unwrap();
    assert!(result.aliases["p"].ends_with("0000000000000001"));
}

pub fn partial_batches<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = open(fresh(), 1);
    run(db.insert_proposed("nodes", fields("parent"), Some("remote-parent".into()))).unwrap();
    let mut current = fields("before");
    current.insert("parent_id".into(), Value::Reference("remote-parent".into()));
    run(db.insert_proposed("nodes", current, Some("existing".into()))).unwrap();
    configure(&mut db);
    let patch = |id: &str, value: &str| AliasedWrite {
        patch: true,
        ..plain(Write::Replace {
            table: "nodes".into(),
            id: id.into(),
            fields: fields(value),
        })
    };
    let marker = || AliasedWrite {
        proposed_id: Some("marker".into()),
        ..plain(Write::Insert {
            table: "local".into(),
            fields: fields("complete"),
        })
    };
    assert!(run(db.apply_entity_batch_aliased(vec![marker(), patch("missing", "fails")])).is_err());
    assert!(run(db.query("local").fetch(&db)).unwrap().is_empty());
    let result =
        run(db.apply_entity_batch_aliased(vec![patch("existing", "client"), marker()])).unwrap();
    assert_eq!(
        result.changes[0].after.as_ref().unwrap().fields["parent_id"],
        Value::Reference("remote-parent".into())
    );
    assert_eq!(db.queue_status().live, 1);
    let key = db.queue_status().commands[0].key.clone();
    let unchanged = run(db.query("nodes").fetch(&db)).unwrap();
    assert!(matches!(
        run(db.apply_entity_batch_aliased(vec![
            plain(Write::Replace {
                table: "local".into(),
                id: "marker".into(),
                fields: fields("must-not-commit")
            }),
            patch("existing", "must-not-commit"),
        ])),
        Err(CommandError::Capacity)
    ));
    assert_eq!(run(db.query("nodes").fetch(&db)).unwrap(), unchanged);
    assert_eq!(
        run(db.query("local").fetch(&db)).unwrap()[0].fields,
        fields("complete")
    );
    let sent = envelope(&db.prepare_push(1000, 0).unwrap());
    assert_eq!(
        sent["changes"],
        Json::Object(BTreeMap::from([(
            "body".into(),
            Json::String("client".into())
        )]))
    );
    db.receive_entity(
        "nodes",
        "existing",
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    run(db.apply_entity_batch_aliased(vec![patch("existing", "client")])).unwrap();
    assert_eq!(
        db.queue_status().live,
        1,
        "same-value partial intent still queues"
    );
    let mut db = open(db.into_storage(), 1);
    configure(&mut db);
    assert_eq!(
        envelope(&db.prepare_push(1000, 0).unwrap())["changes"],
        sent["changes"]
    );
    assert_eq!(
        run(db.query("local").fetch(&db)).unwrap()[0].fields,
        fields("complete")
    );
}

pub fn staged_patch_order<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = open(fresh(), 1000);
    run(db.insert_proposed("nodes", fields("old"), Some("existing".into()))).unwrap();
    configure(&mut db);
    let result = run(db.apply_entity_batch_aliased(vec![
        insert(Some("p"), None),
        plain(Write::Replace {
            table: "nodes".into(),
            id: "existing".into(),
            fields: fields("replacement"),
        }),
        AliasedWrite {
            patch: true,
            ..plain(Write::Replace {
                table: "nodes".into(),
                id: "existing".into(),
                fields: fields("first-patch"),
            })
        },
        AliasedWrite {
            patch: true,
            references: BTreeMap::from([("parent_id".into(), "p".into())]),
            ..plain(Write::Replace {
                table: "nodes".into(),
                id: "existing".into(),
                fields: BTreeMap::new(),
            })
        },
    ]))
    .unwrap();
    let row = result.changes[3].after.as_ref().unwrap();
    assert_eq!(row.fields["body"], Value::String("first-patch".into()));
    assert_eq!(
        row.fields["parent_id"],
        Value::Reference(result.aliases["p"].clone())
    );
    let rows = run(db.query("nodes").fetch(&db)).unwrap();
    let queue = db.queue_status();
    assert!(
        run(db.apply_entity_batch_aliased(vec![
            plain(Write::Remove {
                table: "nodes".into(),
                id: "existing".into()
            }),
            AliasedWrite {
                patch: true,
                ..plain(Write::Replace {
                    table: "nodes".into(),
                    id: "existing".into(),
                    fields: fields("invalid-after-remove")
                })
            },
        ]))
        .is_err()
    );
    assert_eq!(run(db.query("nodes").fetch(&db)).unwrap(), rows);
    assert!(db.queue_status() == queue);
}
