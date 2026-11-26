//! API-0002: the initial typed, asynchronous storage subset of SPEC-0001.
use std::collections::BTreeMap;
use std::future::Future;
mod typed;
pub use typed::*;
mod model;
pub use model::*;
mod observation;
pub use observation::*;
mod sync;
pub use sync::*;

#[doc(hidden)]
pub mod snapshot;
#[doc(hidden)]
pub use snapshot::SnapshotStorage;
#[cfg(feature = "work-counters")]
#[doc(hidden)]
pub mod counters;

/// SPEC-0037 R4: counts entries of work; expands to nothing without `work-counters`.
#[cfg(feature = "work-counters")]
macro_rules! count_work {
    ($counter:ident, $entries:expr) => {
        $crate::storage::counters::add(|counters| &counters.$counter, $entries)
    };
}
#[cfg(not(feature = "work-counters"))]
macro_rules! count_work {
    ($counter:ident, $entries:expr) => {};
}
pub(crate) use count_work;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    String(String),
    Integer(i64),
    Boolean(bool),
    Float(FiniteFloat),
    Timestamp(i64),
    Reference(String),
    Null,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum StorageError {
    MigrationFailure {
        message: String,
    },
    SchemaConflict {
        message: String,
    },
    InvalidSchema {
        message: String,
    },
    InvalidQuery {
        message: String,
    },
    InvalidRow {
        table: String,
        id: String,
        message: String,
    },
    BackendFailure {
        message: String,
    },
    TableNotFound {
        table: String,
    },
    RowAlreadyExists {
        table: String,
        id: String,
    },
    RowNotFound {
        table: String,
        id: String,
    },
    /// Another connection or process owns this store (SPEC-0037 R1).
    StoreInUse,
}

/// Owned write log for the optional atomic capability (SPEC-0020).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mutation {
    Insert {
        table: String,
        row: Row,
    },
    Replace {
        table: String,
        row: Row,
    },
    Remove {
        table: String,
        id: String,
    },
}

/// Optional ordered atomic writes; this does not promise persistence or a read
/// transaction. Implementing `Storage` alone does not imply this capability.
pub trait TransactionalStorage: Storage {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}

/// Internal CRUD foundation. This trait alone promises neither transactions
/// nor schema validation or observation; atomic writes use `TransactionalStorage`.
/// Futures may borrow their inputs.
pub trait Storage {
    fn create_table(&mut self, name: &str)
    -> impl Future<Output = Result<(), StorageError>> + Send;
    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send;
    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;
}
