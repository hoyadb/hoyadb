use hoyadb_doc_examples::*;

fn main() {
    // docs:start schema
    let schema = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "todos".into(),
            TableDefinition {
                columns: BTreeMap::from([
                    ("title".into(), column(ColumnType::String)),
                    ("done".into(), column(ColumnType::Boolean)),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    };
    // docs:end schema
    // docs:start open
    let mut db = run(CommandDatabase::open(
        MemoryStorage::default(),
        schema,
        vec![],
        [9; 16], // Unique namespace for this new store; retain it when reopening.
        SyncOptions::default(),
    ))
    .unwrap();
    // docs:end open
    // docs:start observe
    let query = db.query("todos");
    let subscription = run(db.observe(&query)).unwrap();
    let initial = db.drain_notifications(0);
    // docs:end observe
    assert_eq!(initial.len(), 1);
    // docs:start definition
    let definition = CommandDefinition {
        name: "add-task-v1".into(),
        arguments: BTreeMap::from([(
            "title".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![],
    };
    // docs:end definition

    // docs:start begin
    let request = db.request().unwrap();
    let arguments = Json::Object(BTreeMap::from([(
        "title".into(),
        Json::String("Read offline".into()),
    )]));
    let admitted = run(db.begin(request, definition, arguments, Some(Json::Null), 100)).unwrap();
    // docs:end begin
    assert!(admitted);

    // docs:start handler
    // The local handler runs between begin and finish. Use scope_* operations.
    let task = run(db.scope_insert(request, "todos", fields("Read offline"))).unwrap();
    let receipt = run(db.finish(request, Json::Null)).unwrap();
    // docs:end handler
    assert!(task.id.starts_with("local:"));
    assert_eq!(db.queue_status().pending, 1);
    // docs:start query
    let rows = run(db.query("todos").fetch(&db)).unwrap();
    // docs:end query
    assert_eq!(rows, vec![task.clone()]);
    let changed = db.drain_notifications(1);
    assert!(
        changed
            .iter()
            .any(|event| event.subscription == subscription)
    );
    // docs:start prepare
    let batch = db.prepare_push(100, 0).unwrap();
    // docs:end prepare
    assert_eq!(batch.envelopes.len(), 1);
    // docs:start backend
    // Replace this stand-in with your transport and server handler. The server
    // authenticates the caller, deduplicates the command, and assigns an ID.
    let server_id = "server-task-1";
    db.receive(&receipt.key, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    // docs:end backend
    assert_eq!(db.pending_reconciliations().len(), 1);
    // docs:start sync
    run(db.reconcile(
        &receipt.key,
        ReconcilePlan {
            writes: vec![],
            mappings: BTreeMap::from([(task.id, server_id.into())]),
        },
    ))
    .unwrap();
    // docs:end sync
    let saved = run(db.query("todos").fetch(&db)).unwrap();
    assert_eq!(saved[0].id, server_id);
    assert_eq!(db.queue_status().live, 0);
    // docs:start cleanup
    let unsubscribed = db.unsubscribe(subscription);
    db.close();
    // docs:end cleanup
    assert!(unsubscribed);
    println!("rust todo tour: ok");
}
