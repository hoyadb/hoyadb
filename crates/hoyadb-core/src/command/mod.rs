//! SPEC-0029 / API-0006: host-driven command execution and owned values.
#[doc(hidden)]
pub mod convert;
pub mod json;
pub use json::{Json, JsonError};
pub mod entity;
pub use entity::{
    EntityChange, EntityConfig, EntityContext, EntityDescriptor, EntityLedger, EntityOperation,
    EntityResolution, FieldStamp, MergeHook, MergeInput, MetadataChange, RowMetadata,
    resolve_entity,
};
pub(crate) mod durable;
mod entity_batch;
mod entity_owner;
pub use entity_batch::{AliasedBatchResult, AliasedWrite};
pub(crate) mod execution;
mod ledger_txn;
mod merge;
pub(crate) mod overlay;
mod owner;
mod proposed;
pub use merge::{MergeSession, MergeStep, OwnedMergeInput};
#[doc(hidden)]
pub mod records;
pub use owner::{
    Argument, ArgumentType, CapturedContext, CommandDatabase, CommandDefinition, CommandError,
    CommandRequest, CommandResult,
};
pub use records::{PathPart, RemoteOutcome, SyncOptions, SyncState};
mod queue;
pub use queue::{
    CommandEvent, CommandOutcome, CorrectionGuard, PushBatch, QueueEntry, QueueStatus,
    QueuedWriteKind, ReconcilePlan, ReconcileWrite, SyncSubscription,
};

mod security;
mod sync;
pub use security::DiagnosticSnapshot;

mod policy;
mod pull;
mod reconcile;
pub use policy::{ConflictPolicy, ConflictReaction, Resolution};
pub use pull::{ProgressPhase, PullBatch, PullPage, PullRequest, ResetMode, SyncProgress};

#[cfg(test)]
mod phase3_cases;

// Public-owner fixtures share the process coordinator, just like applications.
#[cfg(test)]
static TEST_OWNERS: std::sync::Mutex<()> = std::sync::Mutex::new(());
