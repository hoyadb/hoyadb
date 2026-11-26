use hoyadb_doc_examples::*;

fn main() {
    // docs:start open
    let mut db = run(Database::open(MemoryStorage::default(), schema(), vec![])).unwrap();
    // docs:end open
    // docs:start insert
    let note = run(db.insert("notes", fields("Read offline"))).unwrap();
    // docs:end insert
    // docs:start fetch
    let rows = run(db.query("notes").fetch(&db)).unwrap();
    // docs:end fetch
    assert_eq!(rows, vec![note.clone()]);
    // docs:start update
    let mut changed = note.fields.clone();
    changed.insert("done".into(), Value::Boolean(true));
    run(db.replace("notes", &note.id, changed)).unwrap();
    // docs:end update
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap()[0].fields["done"],
        Value::Boolean(true)
    );
    // docs:start remove
    let removed = run(db.remove("notes", &note.id)).unwrap();
    // docs:end remove
    assert!(removed);
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    println!("PASS quickstart: insert, fetch, replace, remove");
}
