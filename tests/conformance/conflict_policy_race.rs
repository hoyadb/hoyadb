//! TC-0044 CP10: skipped outside writes retain their atomic storage expectations.
use super::cases as policy;
#[allow(dead_code)]
#[path = "guard_race.rs"]
mod guarded_adapter;
use guarded_adapter::{Race, Racing};
use hoyadb_core::{command::*, model::ROW_VERSION, storage::*};
use policy::*;
use std::collections::BTreeMap;

pub fn cp10<S: SyncStorage>(storage: S) {
    let (storage, armed, seen) = Racing::recording(storage);
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "local");
    let outside =
        run(db.insert_proposed("parents", fields("outside"), Some("outside".into()))).unwrap();
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
    let (mut db, before) = capture(db);
    discard_policy(&mut db);
    db.observe_queue_status().unwrap();
    db.observe_outcome(&key).unwrap();
    db.drain_events(100);
    let mut racing_row = Row {
        id: outside.id.clone(),
        fields: fields("concurrent"),
    };
    racing_row
        .fields
        .insert(ROW_VERSION.into(), Value::Integer(1));
    *armed.lock().unwrap() = Some(Race::Replace("parents", racing_row));
    let plan = ReconcilePlan {
        mappings: BTreeMap::from([(row.id.clone(), "remote".into())]),
        writes: vec![
            ReconcileWrite::Put {
                table: "parents".into(),
                id: "remote".into(),
                fields: fields("server"),
            },
            ReconcileWrite::Remove {
                table: "parents".into(),
                id: outside.id.clone(),
            },
        ],
    };
    assert_eq!(
        run(db.reconcile_resolving(
            &key,
            plan,
            vec![CorrectionGuard {
                table: "parents".into(),
                id: outside.id.clone(),
                expected: Some(outside.fields.clone())
            }]
        )),
        Err(CommandError::RollbackConflict)
    );
    assert!(db.drain_events(100).is_empty());
    let last = seen.lock().unwrap().last().unwrap().clone();
    assert!(
        last.guards.iter().any(|guard| guard.id == outside.id
            && guard.expected.as_ref().is_some_and(
                |row| row.fields.get("name") == Some(&Value::String("outside".into()))
            )),
        "skipped Remove retains explicit guard"
    );
    let current = rows(&db);
    assert!(
        current
            .iter()
            .any(|r| r.id == row.id && r.fields == fields("direct"))
    );
    assert!(!current.iter().any(|r| r.id == "remote"));
    let (db, after) = capture(db);
    assert_eq!(
        before, after,
        "no identity or recovery commit after external guard race"
    );
    db.close();
}
