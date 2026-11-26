use hoyadb_doc_examples::*;

fn main() {
    // docs:start seed
    let mut db = run(Database::open(MemoryStorage::default(), schema(), vec![])).unwrap();
    run(db.insert("notes", fields("Alpha"))).unwrap();
    run(db.insert("notes", fields("Beta"))).unwrap();
    // docs:end seed
    // docs:start filter
    let query = db
        .query("notes")
        .where_("done", Operator::Eq, Value::Boolean(false))
        .where_in(
            "title",
            vec![Value::String("Alpha".into()), Value::String("Beta".into())],
        )
        .order_by("title", Direction::Desc)
        .limit(1);
    // docs:end filter
    // docs:start fetch
    let rows = run(query.fetch(&db)).unwrap();
    // docs:end fetch
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].fields["title"], Value::String("Beta".into()));
    // docs:start invalid
    let bad = db
        .query("notes")
        .where_("missing", Operator::Eq, Value::Null);
    let error = run(bad.fetch(&db));
    // docs:end invalid
    assert!(matches!(error, Err(StorageError::InvalidQuery { .. })));
    println!("PASS queries: conjunction, IN, order, limit, typed rejection");
}
