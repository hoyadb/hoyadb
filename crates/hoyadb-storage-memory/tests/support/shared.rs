//! TC-0041: an Unproven adapter whose store another writer can still edit, as
//! a backend without proven exclusivity allows.
use hoyadb_core::storage::*;
use hoyadb_storage_memory::MemoryStorage;
use std::{
    future::{Future, ready},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

#[derive(Clone, Default)]
pub struct Shared(pub Arc<Mutex<MemoryStorage>>);

fn now<T>(future: impl Future<Output = T>) -> T {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("memory operation suspended"),
    }
}

impl Shared {
    fn with<T>(&self, operation: impl FnOnce(&mut MemoryStorage) -> T) -> T {
        operation(&mut self.0.lock().unwrap())
    }
}

impl Storage for Shared {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.with(|store| now(store.create_table(name))))
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.with(|store| now(store.insert(table, row))))
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(self.with(|store| now(store.get(table, id))))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.with(|store| now(store.replace(table, row))))
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(self.with(|store| now(store.remove(table, id))))
    }
}

impl TransactionalStorage for Shared {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.with(|store| now(store.apply_batch(mutations))))
    }
}

impl TypedStorage for Shared {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(self.with(|store| now(store.query(schema, query))))
    }
}

impl TypedTransactionalStorage for Shared {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(self.with(|store| now(store.apply_typed_batch(schema, mutations))))
    }
}

impl SchemaStorage for Shared {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(self.with(|store| now(store.catalog())))
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(self.with(|store| now(store.scan(table))))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.with(|store| now(store.apply_schema_plan(plan))))
    }
}

impl ObservationStorage for Shared {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(self.with(|store| now(store.project(schema, query, fields))))
    }
}

impl SyncStorage for Shared {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.with(|store| store.load_sync())
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.with(|store| store.apply_sync_plan(plan))
    }
}
