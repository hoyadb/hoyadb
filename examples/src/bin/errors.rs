use hoyadb_doc_examples::*;

fn main() {
    let mut db = run(Database::open(MemoryStorage::default(), schema(), vec![])).unwrap();
    // docs:start invalid
    let invalid = BTreeMap::from([("title".into(), Value::Integer(1))]);
    let rejected = run(db.insert("notes", invalid));
    // docs:end invalid
    assert!(matches!(rejected, Err(StorageError::InvalidRow { .. })));
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    // docs:start batch
    let result = run(db.apply_batch(vec![
        Write::Insert {
            table: "notes".into(),
            fields: fields("Must roll back"),
        },
        Write::Replace {
            table: "notes".into(),
            id: "missing".into(),
            fields: fields("Invalid"),
        },
    ]));
    // docs:end batch
    assert!(result.is_err());
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    // docs:start number
    let number = Json::parse("9223372036854775807").unwrap();
    // docs:end number
    assert_eq!(number.canonical().unwrap(), "9223372036854775807");
    assert!(FiniteFloat::new(f64::NAN).is_none());
    println!("PASS errors: typed rejection, atomic failure, lossless integer and finite float");
}
