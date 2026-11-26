//! Shared setup for the executable public documentation examples.
pub use hoyadb_core::{command::*, model::*, storage::*};
pub use hoyadb_storage_memory::MemoryStorage;
pub use std::collections::{BTreeMap, BTreeSet};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

/// These examples use the current ready-future adapters only. Applications with
/// yielding adapters must await them on their own async runtime instead.
pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("this example requires a ready-future adapter"),
    }
}

pub fn column(kind: ColumnType) -> Column {
    Column {
        kind,
        optional: false,
    }
}

// docs:start schema
pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "notes".into(),
            TableDefinition {
                columns: BTreeMap::from([
                    ("title".into(), column(ColumnType::String)),
                    ("done".into(), column(ColumnType::Boolean)),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

// docs:end schema

// docs:start fields
pub fn fields(title: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("title".into(), Value::String(title.into())),
        ("done".into(), Value::Boolean(false)),
    ])
}

// docs:end fields

pub fn owner() -> CommandDatabase<MemoryStorage> {
    run(CommandDatabase::open(
        MemoryStorage::default(),
        schema(),
        vec![],
        [7; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}
