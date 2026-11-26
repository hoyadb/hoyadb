use hoyadb_doc_examples::*;

fn main() {
    let mut db = owner();
    // docs:start configure
    db.configure_entities(
        EntityConfig {
            scope: "account-fixture".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    // docs:end configure
    // docs:start insert
    let note = run(db.insert("notes", fields("Entity"))).unwrap();
    // docs:end insert
    // docs:start patch
    let patched = run(db.patch(
        "notes",
        &note.id,
        BTreeMap::from([("done".into(), Value::Boolean(true))]),
    ))
    .unwrap();
    // docs:end patch
    assert_eq!(patched.fields["title"], Value::String("Entity".into()));
    assert_eq!(patched.fields["done"], Value::Boolean(true));
    assert_eq!(db.queue_status().pending, 2);
    assert!(
        db.queue_status()
            .commands
            .iter()
            .all(|entry| entry.kind == QueuedWriteKind::Entity)
    );
    println!("PASS entities: configured ordinary insert and partial patch");
}
