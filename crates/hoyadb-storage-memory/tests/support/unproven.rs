//! TC-0041: a custom adapter that inherits the conservative `ownership()`.
use hoyadb_core::storage::*;
use hoyadb_storage_memory::MemoryStorage;
use std::future::Future;

/// A custom adapter that inherits the provided `ownership()`.
#[derive(Default)]
pub struct Unproven(pub MemoryStorage);

impl Storage for Unproven {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.0.get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.0.remove(table, id)
    }
}

impl TransactionalStorage for Unproven {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_batch(mutations)
    }
}

impl TypedStorage for Unproven {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.query(schema, query)
    }
}

impl TypedTransactionalStorage for Unproven {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.0.apply_typed_batch(schema, mutations)
    }
}

impl SchemaStorage for Unproven {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.0.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_schema_plan(plan)
    }
}

impl ObservationStorage for Unproven {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.0.project(schema, query, fields)
    }
}

impl SyncStorage for Unproven {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.0.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.0.apply_sync_plan(plan)
    }
}
