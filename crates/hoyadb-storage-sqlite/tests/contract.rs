//! TC-0022: exactly the shared TC-0019/TC-0020 assertions.
use hoyadb_core::storage::{
    CommittedChange, Mutation, Query, Row, Schema, Storage, StorageError, TransactionalStorage,
    TypedStorage, TypedTransactionalStorage,
};
use hoyadb_storage_sqlite::SqliteStorage;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    storage: Option<SqliteStorage>,
    directory: PathBuf,
}

fn fresh() -> Fixture {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-contract-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let storage = Some(SqliteStorage::open_unprotected(directory.join("fixture.sqlite")).unwrap());
    Fixture {
        storage,
        directory,
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.storage.take());
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

impl Storage for Fixture {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.storage.as_mut().unwrap().remove(table, id)
    }
}

impl TransactionalStorage for Fixture {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().apply_batch(mutations)
    }
}

#[path = "../../../tests/conformance/storage.rs"]
mod conformance;

impl TypedStorage for Fixture {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().query(schema, query)
    }
}

impl TypedTransactionalStorage for Fixture {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.storage
            .as_mut()
            .unwrap()
            .apply_typed_batch(schema, mutations)
    }
}

#[path = "../../../tests/conformance/typed_storage.rs"]
mod typed_conformance;
impl hoyadb_core::storage::SchemaStorage for Fixture {
    fn catalog(
        &self,
    ) -> impl Future<Output = Result<hoyadb_core::storage::ModelCatalog, StorageError>> + Send {
        self.storage.as_ref().unwrap().catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: hoyadb_core::storage::SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().apply_schema_plan(plan)
    }
}

#[path = "../../../tests/conformance/data_model.rs"]
mod model_conformance;
#[path = "../../../tests/conformance/schema_storage.rs"]
mod schema_conformance;

impl hoyadb_core::storage::ObservationStorage for Fixture {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<hoyadb_core::storage::ProjectedRow>, StorageError>> + Send
    {
        self.storage
            .as_ref()
            .unwrap()
            .project(schema, query, fields)
    }
}

#[path = "../../../tests/conformance/projection.rs"]
mod projection_conformance;

#[path = "../../../tests/conformance/reactivity.rs"]
mod reactive_conformance;
