use hoyadb_doc_examples::*;

fn main() {
    let mut definition = schema();

    // docs:start to-one
    let author_relation = RelationDefinition::ToOne {
        table: "authors".into(),
        foreign_key: "author_id".into(),
        optional: false,
    };

    definition
        .tables
        .get_mut("notes")
        .unwrap()
        .relations
        .insert("author".into(), author_relation);
    // docs:end to-one

    // docs:start has-many
    definition.tables.insert(
        "authors".into(),
        TableDefinition {
            columns: BTreeMap::from([("name".into(), column(ColumnType::String))]),
            relations: BTreeMap::from([(
                "notes".into(),
                RelationDefinition::HasMany {
                    table: "notes".into(),
                    foreign_key: "author_id".into(),
                },
            )]),
        },
    );
    // docs:end has-many
    let mut db = run(Database::open(MemoryStorage::default(), definition, vec![])).unwrap();

    // docs:start insert
    let author = run(db.insert(
        "authors",
        BTreeMap::from([("name".into(), Value::String("Ada".into()))]),
    ))
    .unwrap();

    let mut note_fields = fields("Relations");
    note_fields.insert("author_id".into(), Value::Reference(author.id.clone()));

    let note = run(db.insert("notes", note_fields)).unwrap();
    // docs:end insert

    // docs:start navigate
    let notes = run(db.related("authors", &author.id, "notes").fetch(&db)).unwrap();
    let authors = run(db.related("notes", &note.id, "author").fetch(&db)).unwrap();
    // docs:end navigate
    assert_eq!(notes, vec![note.clone()]);
    assert_eq!(authors, vec![author]);

    // docs:start filter
    let matching = run(db
        .query("notes")
        .where_("author.name", Operator::Eq, Value::String("Ada".into()))
        .fetch(&db))
    .unwrap();
    // docs:end filter
    assert_eq!(matching, vec![note]);
    println!("PASS relations: to-one, has-many and related-field predicate");
}
