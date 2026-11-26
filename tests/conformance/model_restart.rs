//! TC-0026 D16: same managed writer/reader assertions in native and browser owners.
use hoyadb_core::{model::*, storage::*};
use std::collections::BTreeMap;

fn column(kind: ColumnType) -> Column {
    Column {
        kind,
        optional: false,
    }
}

pub fn schema(version: u32) -> SchemaDefinition {
    let mut orders = TableDefinition {
        columns: BTreeMap::from([("amount".into(), column(ColumnType::Integer))]),
        relations: BTreeMap::from([(
            "customer".into(),
            RelationDefinition::ToOne {
                table: "customers".into(),
                foreign_key: "customer_id".into(),
                optional: false,
            },
        )]),
    };
    if version >= 2 {
        orders
            .columns
            .insert("discount".into(), column(ColumnType::Integer));
    }
    SchemaDefinition {
        version,
        tables: BTreeMap::from([
            (
                "customers".into(),
                TableDefinition {
                    columns: BTreeMap::from([("country".into(), column(ColumnType::String))]),
                    relations: BTreeMap::from([(
                        "orders".into(),
                        RelationDefinition::HasMany {
                            table: "orders".into(),
                            foreign_key: "customer_id".into(),
                        },
                    )]),
                },
            ),
            ("orders".into(), orders),
        ]),
    }
}

pub fn migration() -> Migration {
    Migration {
        from_version: 1,
        to: schema(2),
        steps: vec![MigrationStep::AddColumn {
            table: "orders".into(),
            name: "discount".into(),
            column: column(ColumnType::Integer),
            default: Some(Value::Integer(5)),
        }],
    }
}

fn order_fields() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("amount".into(), Value::Integer(4200)),
        ("customer_id".into(), Value::Reference("local:1".into())),
        ("discount".into(), Value::Integer(5)),
    ])
}

pub async fn write<S: SchemaStorage>(storage: S) -> Result<(), StorageError> {
    let mut db = Database::open(storage, schema(1), vec![]).await?;
    assert_eq!(
        db.insert(
            "customers",
            BTreeMap::from([("country".into(), Value::String("FR\0🌿".into()))])
        )
        .await?
        .id,
        "local:1"
    );
    let mut fields = order_fields();
    fields.remove("discount");
    assert_eq!(db.insert("orders", fields).await?.id, "local:2");
    let db = Database::open(db.into_storage(), schema(2), vec![migration()]).await?;
    let invalid = Migration {
        from_version: 2,
        to: schema(3),
        steps: vec![MigrationStep::Backfill {
            table: "orders".into(),
            column: "amount".into(),
            value: Value::String("invalid".into()),
        }],
    };
    let (error, storage) =
        match Database::open_recoverable(db.into_storage(), schema(3), vec![invalid]).await {
            Err(value) => value,
            Ok(_) => panic!("invalid restart migration succeeded"),
        };
    assert!(matches!(error, StorageError::MigrationFailure { .. }));
    assert_eq!(storage.catalog().await?.state.unwrap().version, 2);
    Ok(())
}

pub async fn read<S: SchemaStorage>(
    storage: S,
    advance: bool,
) -> Result<(Vec<Row>, Option<Row>), StorageError> {
    let db = Database::open(storage, schema(2), vec![]).await?;
    let expected = Row {
        id: "local:2".into(),
        fields: order_fields(),
    };
    let rows = db.query("orders").fetch(&db).await?;
    assert_eq!(rows, vec![expected.clone()]);
    let customer = Row {
        id: "local:1".into(),
        fields: BTreeMap::from([("country".into(), Value::String("FR\0🌿".into()))]),
    };
    assert_eq!(
        db.related("orders", "local:2", "customer")
            .fetch(&db)
            .await?,
        vec![customer]
    );
    assert_eq!(
        db.related("customers", "local:1", "orders")
            .fetch(&db)
            .await?,
        vec![expected]
    );
    let storage = db.into_storage();
    let state = storage.catalog().await?.state.unwrap();
    assert_eq!(state.version, 2);
    assert_eq!(state.next_id, 3);
    assert_eq!(state.definition, schema(2).canonical()?);
    let next = if advance {
        let mut db = Database::open(storage, schema(2), vec![]).await?;
        let row = db.insert("orders", order_fields()).await?;
        assert_eq!(row.id, "local:3");
        Some(row)
    } else {
        None
    };
    Ok((rows, next))
}
