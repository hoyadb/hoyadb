//! TC-0026 D13/D14: identical atomic capability assertions for every adapter.
use super::fresh;
use hoyadb_core::{
    model::{ROW_VERSION, SchemaDefinition, TableDefinition},
    storage::*,
};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future + Send>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("synchronous adapter suspended"),
    }
}

pub fn definition(version: u32) -> SchemaDefinition {
    SchemaDefinition {
        version,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "number".into(),
                    Column {
                        kind: ColumnType::Integer,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn state(version: u32) -> ModelState {
    ModelState {
        version,
        definition: definition(version).canonical().unwrap(),
        next_id: 1,
    }
}

pub fn row(number: i64, version: u32) -> Row {
    Row {
        id: "seed".into(),
        fields: BTreeMap::from([
            ("number".into(), Value::Integer(number)),
            (ROW_VERSION.into(), Value::Integer(i64::from(version))),
        ]),
    }
}

pub fn initial() -> SchemaPlan {
    SchemaPlan {
        expected: None,
        next: state(1),
        schema: definition(1).storage_schema().unwrap(),
        operations: vec![
            SchemaMutation::CreateTable("items".into()),
            SchemaMutation::Write(Mutation::Insert {
                table: "items".into(),
                row: row(7, 1),
            }),
        ],
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn s01_initialize_catalog_and_owned_scan() {
    let mut store = fresh();
    assert_eq!(
        run(store.catalog()),
        Ok(ModelCatalog {
            state: None,
            tables: vec![]
        })
    );
    run(store.apply_schema_plan(initial())).unwrap();
    assert_eq!(
        run(store.catalog()),
        Ok(ModelCatalog {
            state: Some(state(1)),
            tables: vec!["items".into()]
        })
    );
    let mut scanned = run(store.scan("items")).unwrap();
    assert_eq!(scanned, vec![row(7, 1)]);
    scanned[0].fields.clear();
    assert_eq!(run(store.scan("items")), Ok(vec![row(7, 1)]));
    assert!(
        matches!(run(store.scan("missing")),Err(StorageError::TableNotFound{table}) if table=="missing")
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn s02_failed_prefix_preserves_all_state() {
    let mut store = fresh();
    run(store.apply_schema_plan(initial())).unwrap();
    let plan = SchemaPlan {
        expected: Some(state(1)),
        next: state(2),
        schema: definition(2).storage_schema().unwrap(),
        operations: vec![
            SchemaMutation::Write(Mutation::Replace {
                table: "items".into(),
                row: row(99, 2),
            }),
            SchemaMutation::CreateTable("temporary".into()),
            SchemaMutation::Write(Mutation::Insert {
                table: "items".into(),
                row: row(1, 2),
            }),
        ],
    };
    assert!(matches!(
        run(store.apply_schema_plan(plan)),
        Err(StorageError::RowAlreadyExists { .. })
    ));
    assert_eq!(run(store.catalog()).unwrap().state, Some(state(1)));
    assert_eq!(run(store.scan("items")), Ok(vec![row(7, 1)]));
    assert!(matches!(
        run(store.scan("temporary")),
        Err(StorageError::TableNotFound { .. })
    ));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn s03_stale_expected_state_rejects_before_operations() {
    let mut store = fresh();
    run(store.apply_schema_plan(initial())).unwrap();
    let mut plan = initial();
    plan.operations = vec![SchemaMutation::DropTable("items".into())];
    assert!(matches!(
        run(store.apply_schema_plan(plan)),
        Err(StorageError::SchemaConflict { .. })
    ));
    assert_eq!(run(store.scan("items")), Ok(vec![row(7, 1)]));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn s04_unknown_bookkeeping_and_corrupt_metadata_never_become_absent() {
    let mut store = fresh();
    run(store.apply_schema_plan(initial())).unwrap();
    let mut metadata = model_state_row(&state(1)).unwrap();
    metadata.fields.insert("format".into(), Value::Integer(2));
    run(store.replace(MODEL_TABLE, metadata.clone())).unwrap();
    assert!(matches!(
        run(store.catalog()),
        Err(StorageError::BackendFailure { .. })
    ));
    assert_eq!(run(store.get(MODEL_TABLE, "state")), Ok(Some(metadata)));
    drop(store);
    let mut store = fresh();
    run(store.apply_schema_plan(initial())).unwrap();
    run(store.create_table("__hoya_queue")).unwrap();
    assert!(matches!(
        run(store.catalog()),
        Err(StorageError::BackendFailure { .. })
    ));
    assert_eq!(run(store.scan("items")), Ok(vec![row(7, 1)]));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn s05_checkpoints_and_final_candidate_validate_before_commit() {
    let mut store = fresh();
    run(store.apply_schema_plan(initial())).unwrap();
    let plan = SchemaPlan {
        expected: Some(state(1)),
        next: state(2),
        schema: definition(2).storage_schema().unwrap(),
        operations: vec![
            SchemaMutation::Validate {
                schema: definition(2).storage_schema().unwrap(),
                version: 2,
            },
            SchemaMutation::Write(Mutation::Replace {
                table: "items".into(),
                row: row(9, 2),
            }),
        ],
    };
    assert!(run(store.apply_schema_plan(plan)).is_err());
    assert_eq!(run(store.scan("items")), Ok(vec![row(7, 1)]));
    let mut plan = initial();
    plan.expected = Some(state(1));
    plan.next = state(2);
    plan.schema = definition(2).storage_schema().unwrap();
    plan.operations = vec![SchemaMutation::DropTable("items".into())];
    assert!(run(store.apply_schema_plan(plan)).is_err());
    assert_eq!(run(store.catalog()).unwrap().state, Some(state(1)));
}
