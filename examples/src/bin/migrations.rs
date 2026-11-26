use hoyadb_doc_examples::*;

fn main() {
    let mut db = run(Database::open(MemoryStorage::default(), schema(), vec![])).unwrap();
    run(db.insert("notes", fields("Keep this row"))).unwrap();
    // docs:start target
    let mut next = schema();
    next.version = 2;
    next.tables
        .get_mut("notes")
        .unwrap()
        .columns
        .insert("archived".into(), column(ColumnType::Boolean));
    // docs:end target
    // docs:start plan
    let migration = Migration {
        from_version: 1,
        to: next.clone(),
        steps: vec![MigrationStep::AddColumn {
            table: "notes".into(),
            name: "archived".into(),
            column: column(ColumnType::Boolean),
            default: Some(Value::Boolean(false)),
        }],
    };
    // docs:end plan
    // docs:start reopen
    let db = run(Database::open(db.into_storage(), next, vec![migration])).unwrap();
    // docs:end reopen
    assert_eq!(db.schema().version, 2);
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap()[0].fields["archived"],
        Value::Boolean(false)
    );
    println!("PASS migrations: authored version change and backfill");
}
