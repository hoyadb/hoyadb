//! TC-0041 SI-O06-SI-O08: Exclusive custom adapters whose reads may suspend.
//! `Adapter<false>` keeps the provided `reads_ready` default and every read
//! yields Pending once (waking its task). `Adapter<true>` falsely promises ready
//! reads while reads of `pending_id` yield. Writes and SyncStorage are
//! synchronous.
use hoyadb_core::storage::*;
use hoyadb_storage_memory::MemoryStorage;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

#[derive(Clone, Default)]
pub struct Counts {
    /// Base reads started on the adapter.
    pub reads: Arc<AtomicUsize>,
    /// Polls of reads that yield.
    pub pending_polls: Arc<AtomicUsize>,
    /// Real delta commits and projection calls, for SI-Y01 cancellation cases.
    pub delta_commits: Arc<AtomicUsize>,
    pub projects: Arc<AtomicUsize>,
}

impl Counts {
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    pub fn pending_polls(&self) -> usize {
        self.pending_polls.load(Ordering::SeqCst)
    }

    pub fn delta_commits(&self) -> usize {
        self.delta_commits.load(Ordering::SeqCst)
    }

    pub fn projects(&self) -> usize {
        self.projects.load(Ordering::SeqCst)
    }
}

pub struct Adapter<const READY: bool> {
    pub inner: MemoryStorage,
    pub counts: Counts,
    /// For `Adapter<true>`: the row id whose reads yield.
    pub pending_id: &'static str,
}

impl<const READY: bool> Adapter<READY> {
    pub fn new(counts: Counts, pending_id: &'static str) -> Self {
        Self {
            inner: MemoryStorage::default(),
            counts,
            pending_id,
        }
    }

    fn yields(&self, id: Option<&str>) -> bool {
        !READY || id == Some(self.pending_id)
    }
}

/// Counts a base read and returns it behind a gate that yields once if asked.
fn gate<F: Future + Send>(
    counts: &Counts,
    pending: bool,
    read: F,
) -> impl Future<Output = F::Output> + Send + use<F> {
    counts.reads.fetch_add(1, Ordering::SeqCst);
    let gate = Yield {
        pending,
        polls: counts.pending_polls.clone(),
    };
    async move {
        gate.await;
        read.await
    }
}

struct Yield {
    pending: bool,
    polls: Arc<AtomicUsize>,
}

impl Future for Yield {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if !self.pending {
            return Poll::Ready(());
        }
        self.polls.fetch_add(1, Ordering::SeqCst);
        self.pending = false;
        context.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<const READY: bool> Storage for Adapter<READY> {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        gate(
            &self.counts,
            self.yields(Some(id)),
            self.inner.get(table, id),
        )
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.inner.remove(table, id)
    }
}

impl<const READY: bool> TransactionalStorage for Adapter<READY> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(mutations)
    }
}

impl<const READY: bool> TypedStorage for Adapter<READY> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        gate(
            &self.counts,
            self.yields(None),
            self.inner.query(schema, query),
        )
    }

    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        gate(
            &self.counts,
            self.yields(None),
            self.inner.query_validated(proof),
        )
    }
}

impl<const READY: bool> TypedTransactionalStorage for Adapter<READY> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(schema, mutations)
    }
}

impl<const READY: bool> SchemaStorage for Adapter<READY> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        gate(&self.counts, self.yields(None), self.inner.catalog())
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        gate(&self.counts, self.yields(None), self.inner.scan(table))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(plan)
    }
}

impl<const READY: bool> ObservationStorage for Adapter<READY> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.counts.projects.fetch_add(1, Ordering::SeqCst);
        gate(
            &self.counts,
            self.yields(None),
            self.inner.project(schema, query, fields),
        )
    }
}

/// Honest yielding adapter: Exclusive, `reads_ready` left at its default.
impl SyncStorage for Adapter<false> {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.inner.apply_sync_plan(plan)
    }

    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        let changes = self.inner.apply_sync_delta(delta)?;
        self.counts.delta_commits.fetch_add(1, Ordering::SeqCst);
        Ok(changes)
    }

    fn ownership(&self) -> StoreOwnership {
        StoreOwnership::Exclusive
    }
}

/// Lying adapter: Exclusive and claims ready reads, but `pending_id` yields.
impl SyncStorage for Adapter<true> {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.inner.apply_sync_plan(plan)
    }

    fn ownership(&self) -> StoreOwnership {
        StoreOwnership::Exclusive
    }

    fn reads_ready(&self) -> bool {
        true
    }
}
