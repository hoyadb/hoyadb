//! API-0011: forward concrete adapters; storage algorithms stay in the core.
use hoyadb_core::storage::*;
use hoyadb_storage_memory::MemoryStorage;
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    future::{Future, ready},
    task::{Context, Poll, Waker},
};

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../tests/conformance/work_counters.rs"]
mod delta_scaling;

pub(crate) enum Engine {
    Memory(MemoryStorage),
    Sqlite(SqliteStorage),
}

fn first<T>(future: impl Future<Output = Result<T, StorageError>>) -> Result<T, StorageError> {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => Err(StorageError::BackendFailure {
            message: "binding storage unexpectedly suspended".into(),
        }),
    }
}

macro_rules! forward {
    ($this:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $this {
            Engine::Memory(store) => first(store.$method($($arg),*)),
            Engine::Sqlite(store) => first(store.$method($($arg),*)),
        }
    };
}

impl Storage for Engine {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(forward!(self, create_table(name)))
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(forward!(self, insert(table, row)))
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(forward!(self, get(table, id)))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(forward!(self, replace(table, row)))
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(forward!(self, remove(table, id)))
    }
}

impl TransactionalStorage for Engine {
    fn apply_batch(
        &mut self,
        writes: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(forward!(self, apply_batch(writes)))
    }
}

impl TypedStorage for Engine {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(forward!(self, query(schema, query)))
    }

    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(forward!(self, query_validated(proof)))
    }
}

impl TypedTransactionalStorage for Engine {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        writes: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(forward!(self, apply_typed_batch(schema, writes)))
    }
}

impl SchemaStorage for Engine {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(forward!(self, catalog()))
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(forward!(self, scan(table)))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(forward!(self, apply_schema_plan(plan)))
    }
}

impl ObservationStorage for Engine {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(forward!(self, project(schema, query, fields)))
    }
}

impl SyncStorage for Engine {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        match self {
            Self::Memory(store) => store.load_sync(),
            Self::Sqlite(store) => store.load_sync(),
        }
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        match self {
            Self::Memory(store) => store.apply_sync_plan(plan),
            Self::Sqlite(store) => store.apply_sync_plan(plan),
        }
    }

    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        match self {
            Self::Memory(store) => store.apply_sync_delta(delta),
            Self::Sqlite(store) => store.apply_sync_delta(delta),
        }
    }

    fn ownership(&self) -> StoreOwnership {
        match self {
            Self::Memory(store) => store.ownership(),
            Self::Sqlite(store) => store.ownership(),
        }
    }

    fn reads_ready(&self) -> bool {
        match self {
            Self::Memory(store) => store.reads_ready(),
            Self::Sqlite(store) => store.reads_ready(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounded_delta_work(small: counters::WorkSnapshot, large: counters::WorkSnapshot) {
        // SI-Y03: the Engine must carry the owner's proof to the concrete
        // adapter. Inheriting the conservative provided method reloads and
        // rewrites M packets, even after the core planner is bounded.
        assert!(small.packets_encoded > 0, "the patch must persist packets");
        assert!(small.rows_cloned > 0, "the patch must stage a real write");
        assert_eq!(
            small.packets_parsed, large.packets_parsed,
            "{small:?} vs {large:?}"
        );
        assert_eq!(
            small.packets_encoded, large.packets_encoded,
            "{small:?} vs {large:?}"
        );
        assert_eq!(
            small.rows_cloned, large.rows_cloned,
            "{small:?} vs {large:?}"
        );
        assert_eq!(
            small.sqlite_rows_written, large.sqlite_rows_written,
            "{small:?} vs {large:?}"
        );
        assert_eq!(small.sqlite_fullscan_steps, 0, "{small:?}");
        assert_eq!(large.sqlite_fullscan_steps, 0, "{large:?}");
    }

    #[test]
    fn si_y03_engine_memory_preserves_bounded_delta_work() {
        bounded_delta_work(
            delta_scaling::patch_work(Engine::Memory(MemoryStorage::default()), 10),
            delta_scaling::patch_work(Engine::Memory(MemoryStorage::default()), 1000),
        );
    }

    #[test]
    fn si_y03_engine_sqlite_preserves_bounded_delta_work() {
        let directory =
            std::env::temp_dir().join(format!("hoya-engine-delta-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let measured = |m| {
            delta_scaling::patch_work(
                Engine::Sqlite(
                    SqliteStorage::open_unprotected(directory.join(format!("{m}.sqlite"))).unwrap(),
                ),
                m,
            )
        };
        let small = measured(10);
        let large = measured(1000);
        std::fs::remove_dir_all(directory).unwrap();
        bounded_delta_work(small, large);
    }

    /// TC-0041 SI-O08: the facade engine forwards its adapter's readiness.
    #[test]
    fn si_o08_engine_forwards_read_readiness() {
        let memory = MemoryStorage::default();
        assert_eq!(
            Engine::Memory(MemoryStorage::default()).reads_ready(),
            memory.reads_ready()
        );
        assert!(Engine::Memory(memory).reads_ready());
        let path =
            std::env::temp_dir().join(format!("hoya-engine-ready-{}.sqlite", std::process::id()));
        let store = SqliteStorage::open_unprotected(&path).unwrap();
        assert!(store.reads_ready());
        assert!(Engine::Sqlite(store).reads_ready());
        std::fs::remove_file(path).unwrap();
    }
}
