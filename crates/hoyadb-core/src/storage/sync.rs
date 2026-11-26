//! API-0007 / TC-0032: atomic application and command recovery capability.
use super::*;
use crate::{
    command::{
        durable::{self, StoredLedger},
        records::SyncLedger,
    },
    model::ROW_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};

pub const SYNC_TABLE: &str = "__hoya_sync";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowGuard {
    pub table: String,
    pub id: String,
    pub expected: Option<Row>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncTransition {
    Normal,
    Reset {
        discard_recovery: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlan {
    pub transition: SyncTransition,
    pub expected_model: ModelState,
    pub next_model: ModelState,
    pub expected_sync: Option<Vec<Row>>,
    pub next_sync: Vec<Row>,
    pub guards: Vec<RowGuard>,
    pub mutations: Vec<Mutation>,
}

/// Owned adapter inputs extracted from a proof. These fields do not provide a
/// way to construct a proof or assert a validated ledger transition.
pub struct SyncDeltaParts {
    pub expected_model: ModelState,
    pub next_model: ModelState,
    pub expected_packets: Vec<(String, Option<Row>)>,
    pub upserts: Vec<Row>,
    pub deletes: Vec<String>,
    pub guards: Vec<RowGuard>,
    pub mutations: Vec<Mutation>,
}

/// A bounded transition proven by a validated Exclusive command owner.
/// Adapters may consume its rows, but untrusted parts cannot create a proof.
///
/// ```compile_fail
/// use hoyadb_core::storage::{SyncDelta, SyncDeltaParts};
/// fn forge(parts: SyncDeltaParts) -> SyncDelta { SyncDelta { parts } }
/// ```
/// ```compile_fail
/// use hoyadb_core::storage::{SyncDelta, SyncDeltaParts};
/// fn forge(parts: SyncDeltaParts) -> SyncDelta { parts.into() }
/// ```
/// ```compile_fail
/// use hoyadb_core::storage::SyncDelta;
/// fn duplicate(proof: SyncDelta) -> SyncDelta { proof.clone() }
/// ```
/// ```compile_fail
/// use hoyadb_core::storage::SyncDelta;
/// let proof: SyncDelta = Default::default();
/// ```
pub struct SyncDelta {
    parts: SyncDeltaParts,
}

impl SyncDelta {
    /// The sole owner persist caller must already have checked the complete
    /// bounded record transition (including frozen base plus entity extension),
    /// touched metadata, model/entity rules, absence of live recovery before
    /// entity adoption, and capacity/history. This payload
    /// check cannot reconstruct unchanged companion packets or replace that
    /// semantic proof. Strict open and owner-only writes establish the base.
    pub(crate) fn validated_exclusive(parts: SyncDeltaParts) -> Result<Self, StorageError> {
        validate_delta_parts(&parts)?;
        Ok(Self {
            parts,
        })
    }

    pub fn expected_model(&self) -> &ModelState {
        &self.parts.expected_model
    }
    pub fn next_model(&self) -> &ModelState {
        &self.parts.next_model
    }
    pub fn expected_packets(&self) -> &[(String, Option<Row>)] {
        &self.parts.expected_packets
    }
    pub fn upserts(&self) -> &[Row] {
        &self.parts.upserts
    }
    pub fn deletes(&self) -> &[String] {
        &self.parts.deletes
    }
    pub fn guards(&self) -> &[RowGuard] {
        &self.parts.guards
    }
    pub fn mutations(&self) -> &[Mutation] {
        &self.parts.mutations
    }
    pub fn into_parts(self) -> SyncDeltaParts {
        self.parts
    }
}

/// Recheck bounded payload shape before an adapter consumes the proof. Full
/// record and touched-metadata semantics were checked by the core owner; an
/// adapter is not expected to reconstruct a ledger from a partial packet set.
#[doc(hidden)]
pub fn validate_sync_delta(delta: &SyncDelta) -> Result<Schema, StorageError> {
    validate_delta_parts(&delta.parts)
}

fn validate_delta_parts(parts: &SyncDeltaParts) -> Result<Schema, StorageError> {
    use crate::command::records::{decode_header, decode_packet, validate_packet};
    let definition = model_definition(&parts.expected_model)?;
    model_definition(&parts.next_model)?;
    if parts.next_model.version != parts.expected_model.version
        || parts.next_model.definition != parts.expected_model.definition
        || parts.next_model.next_id < parts.expected_model.next_id
    {
        return Err(schema_conflict("invalid command model transition"));
    }
    let mut expected = BTreeMap::new();
    for (id, image) in &parts.expected_packets {
        if expected.insert(id.as_str(), image.as_ref()).is_some()
            || image.as_ref().is_some_and(|row| row.id != *id)
        {
            return Err(schema_conflict("invalid delta packet expectation"));
        }
    }
    let mut upserts = BTreeMap::new();
    for row in &parts.upserts {
        if upserts.insert(row.id.as_str(), row).is_some() {
            return Err(schema_conflict("duplicate delta packet upsert"));
        }
    }
    let mut deletes = BTreeSet::new();
    for id in &parts.deletes {
        if !deletes.insert(id.as_str()) || upserts.contains_key(id.as_str()) {
            return Err(schema_conflict(
                "duplicate or overlapping delta packet delete",
            ));
        }
    }
    let required = upserts
        .keys()
        .copied()
        .chain(deletes.iter().copied())
        .chain(["state", "entity:state"])
        .collect::<BTreeSet<_>>();
    if expected.keys().copied().collect::<BTreeSet<_>>() != required {
        return Err(schema_conflict(
            "delta requires exact changed-packet and header expectations",
        ));
    }
    let next = |id: &str| {
        if deletes.contains(id) {
            None
        } else {
            upserts
                .get(id)
                .copied()
                .or_else(|| expected.get(id).copied().flatten())
        }
    };
    let previous_header = expected["state"]
        .map(|row| decode_header(&decode_packet(row)?))
        .transpose()?;
    let next_header =
        next("state").ok_or_else(|| schema_conflict("delta retains command header"))?;
    let next_header = decode_header(&decode_packet(next_header)?)?;
    // Header-only ledgers validate the intrinsic counter/option constraints
    // without walking any entity metadata or copying command records.
    for header in previous_header.iter().chain(std::iter::once(&next_header)) {
        SyncLedger {
            header: header.clone(),
            records: BTreeMap::new(),
            entity: None,
        }
        .validate()?;
    }
    if let Some(previous) = &previous_header
        && (previous.namespace != next_header.namespace
            || next_header.sequence < previous.sequence
            || next_header.last_ms < previous.last_ms)
    {
        return Err(schema_conflict("command identity counters cannot regress"));
    }
    let previous_stored_entity = expected["entity:state"]
        .map(durable::decode_entity_header)
        .transpose()?;
    let next_stored_entity = next("entity:state")
        .map(durable::decode_entity_header)
        .transpose()?;
    let previous_durable = previous_stored_entity
        .as_ref()
        .is_some_and(|(_, enabled)| *enabled);
    let next_durable = next_stored_entity
        .as_ref()
        .is_some_and(|(_, enabled)| *enabled);
    if previous_stored_entity.is_some() && previous_durable != next_durable {
        return Err(schema_conflict(
            "entity ledger format cannot change implicitly",
        ));
    }
    let previous_entity = previous_stored_entity.map(|(entity, _)| entity);
    let next_entity = next_stored_entity.map(|(entity, _)| entity);
    for entity in previous_entity.iter().chain(next_entity.iter()) {
        entity.validate(&definition)?;
    }
    match (&previous_entity, &next_entity) {
        (Some(_), None) => return Err(schema_conflict("entity comparison state cannot disappear")),
        (None, Some(entity))
            if !entity.is_pristine() || entity.generation != 1 || entity.revision != 0 =>
        {
            return Err(schema_conflict(
                "initial entity binding must be pristine generation one",
            ));
        }
        (Some(old), Some(new)) => {
            let metadata_changed = upserts
                .iter()
                .any(|(id, row)| id.starts_with("entity:metadata:") && expected[id] != Some(*row))
                || deletes.iter().any(|id| id.starts_with("entity:metadata:"));
            let aliases_changed = upserts
                .iter()
                .any(|(id, row)| id.starts_with("entity:alias:") && expected[id] != Some(*row))
                || deletes.iter().any(|id| id.starts_with("entity:alias:"));
            let changed = !parts.mutations.is_empty() || metadata_changed || aliases_changed;
            if old.ledger_format != new.ledger_format
                || old.generation != new.generation
                || old.revision.checked_add(i64::from(changed)) != Some(new.revision)
                || new.pages < old.pages
                || new.changes < old.changes
                || (old.ready && !new.ready)
                || (old.checkpoint.is_some() && new.checkpoint.is_none())
            {
                return Err(schema_conflict("invalid entity header transition"));
            }
            if old.config != new.config {
                return Err(schema_conflict(
                    "entity reconfiguration requires a full inventory plan",
                ));
            }
        }
        _ => {}
    }
    for row in expected.values().filter_map(|row| *row) {
        if row.id == "entity:state" {
            durable::decode_entity_header(row)?;
        } else if row.id.starts_with("entity:alias:") {
            if !previous_durable {
                return Err(schema_conflict("alias requires durable entity format"));
            }
            let entity = previous_entity
                .as_ref()
                .ok_or_else(|| schema_conflict("alias lacks entity header"))?;
            let header = previous_header
                .as_ref()
                .ok_or_else(|| schema_conflict("alias lacks command header"))?;
            durable::validate_alias_packet(row, &header.namespace, &entity.config.tables)?;
        } else {
            validate_packet(row, previous_entity.as_ref())?;
        }
    }
    for row in upserts.values() {
        if row.id == "entity:state" {
            durable::decode_entity_header(row)?;
        } else if row.id.starts_with("entity:alias:") {
            if !next_durable {
                return Err(schema_conflict("alias requires durable entity format"));
            }
            if previous_entity.is_none() {
                return Err(schema_conflict(
                    "initial durable binding must have no aliases",
                ));
            }
            let entity = next_entity
                .as_ref()
                .ok_or_else(|| schema_conflict("alias lacks entity header"))?;
            let ((_, local), _) =
                durable::validate_alias_packet(row, &next_header.namespace, &entity.config.tables)?;
            validate_alias_counter(&local, &parts.next_model)?;
            if expected[row.id.as_str()].is_some_and(|prior| prior != *row) {
                return Err(schema_conflict("accepted entity aliases are immutable"));
            }
        } else {
            validate_packet(row, next_entity.as_ref())?;
        }
    }
    if deletes.iter().any(|id| id.starts_with("entity:alias:")) {
        return Err(schema_conflict(
            "entity aliases require explicit reset to clear",
        ));
    }
    // A deleted packet has a real prior image. This also rejects an invented
    // packet namespace that no validated owner could have touched.
    if deletes.iter().any(|id| expected[id].is_none()) {
        return Err(schema_conflict(
            "delta deletion requires an existing packet",
        ));
    }
    let schema = definition.storage_schema()?;
    validate_sync_writes(
        &parts.expected_model,
        &parts.next_model,
        &schema,
        &parts.guards,
        &parts.mutations,
    )?;
    Ok(schema)
}

/// SPEC-0037 R3.0: whether this adapter proves that no other owner can reach its store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreOwnership {
    /// No exclusivity is claimed; owners share the process-wide coordinator and
    /// keep full per-command validation.
    Unproven,
    /// While this adapter value is owned, no writer exists through any process,
    /// connection or adapter instance. Return this only after proving the
    /// equivalent of TC-0041 SI-L01 and SI-L03 (a second same-process open and a
    /// second-process open are rejected), or when construction guarantees that no
    /// second instance can reach the same store.
    Exclusive,
}

/// Commit is synchronous: no future can be abandoned between commit and owner update.
pub trait SyncStorage: ObservationStorage {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError>;
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError>;
    /// Conservative provided fallback: retain a full prior-image CAS and the
    /// complete SyncPlan validation for custom adapters.
    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        validate_sync_delta(&delta)?;
        let parts = delta.into_parts();
        let prior = self.load_sync()?;
        let prior_by_id = prior
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|row| (row.id.as_str(), row))
            .collect::<BTreeMap<_, _>>();
        for (id, expected) in &parts.expected_packets {
            if prior_by_id.get(id.as_str()).copied() != expected.as_ref() {
                return Err(schema_conflict("command recovery changed before commit"));
            }
        }
        count_work!(rows_cloned, prior_by_id.len() as u64);
        let mut next = prior_by_id
            .into_iter()
            .map(|(id, row)| (id.to_owned(), row.clone()))
            .collect::<BTreeMap<_, _>>();
        for id in parts.deletes {
            next.remove(&id);
        }
        for row in parts.upserts {
            next.insert(row.id.clone(), row);
        }
        self.apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: parts.expected_model,
            next_model: parts.next_model,
            expected_sync: prior,
            next_sync: next.into_values().collect(),
            guards: parts.guards,
            mutations: parts.mutations,
        })
    }
    /// Unknown adapters stay on the conservative process-wide coordinator. An
    /// override returning `Exclusive` makes the promise documented on
    /// [`StoreOwnership::Exclusive`].
    fn ownership(&self) -> StoreOwnership {
        StoreOwnership::Unproven
    }
    /// SPEC-0037 R6: `true` promises that every future returned by `get`,
    /// `scan`, `query`, `query_validated`, `project` and `catalog` completes on
    /// its first poll. Independent of ownership; unknown adapters stay `false`.
    fn reads_ready(&self) -> bool {
        false
    }
}

/// The conflict every adapter reports when a guarded row differs at commit time.
pub const GUARD_CONFLICT: &str = "command application row changed before commit";

/// Validate exact owned inputs before an adapter performs its guarded transaction.
#[doc(hidden)]
pub fn validate_sync_plan(plan: &SyncPlan) -> Result<Schema, StorageError> {
    if matches!(plan.transition, SyncTransition::Reset { .. }) && plan.expected_sync.is_none() {
        return Err(schema_conflict(
            "reset requires previously loaded recovery state",
        ));
    }
    let definition = model_definition(&plan.expected_model)?;
    model_definition(&plan.next_model)?;
    if plan.next_model.version != plan.expected_model.version
        || plan.next_model.definition != plan.expected_model.definition
        || plan.next_model.next_id < plan.expected_model.next_id
    {
        return Err(schema_conflict("invalid command model transition"));
    }
    let next_stored = StoredLedger::from_rows(&plan.next_sync)?;
    validate_stored_model(&next_stored, &plan.next_model)?;
    let next = &next_stored.ledger;
    if plan.expected_sync.is_none() && !next_stored.aliases.mappings.is_empty() {
        return Err(schema_conflict(
            "initial durable binding must have no aliases",
        ));
    }
    if plan.expected_sync.is_none()
        && next
            .entity
            .as_ref()
            .is_some_and(|e| !e.is_pristine() || e.generation != 1 || e.revision != 0)
    {
        return Err(schema_conflict(
            "initial entity binding must be pristine generation one",
        ));
    }
    if let Some(rows) = &plan.expected_sync {
        let previous_stored = StoredLedger::from_rows(rows)?;
        validate_stored_model(&previous_stored, &plan.expected_model)?;
        validate_alias_transition(plan.transition, &previous_stored, &next_stored)?;
        let previous = &previous_stored.ledger;
        if previous.header.namespace != next.header.namespace
            || next.header.sequence < previous.header.sequence
            || next.header.last_ms < previous.header.last_ms
        {
            return Err(schema_conflict("command identity counters cannot regress"));
        }
        validate_entity_transition(
            plan,
            previous,
            next,
            previous_stored.aliases != next_stored.aliases,
        )?;
        for (key, old) in &previous.records {
            if let Some(new) = next.records.get(key)
                && old.frozen.is_some()
                && (old.frozen != new.frozen || old.envelope() != new.envelope())
            {
                return Err(schema_conflict("frozen command envelope cannot change"));
            }
        }
    }
    let schema = definition.storage_schema()?;
    validate_sync_writes(
        &plan.expected_model,
        &plan.next_model,
        &schema,
        &plan.guards,
        &plan.mutations,
    )?;
    Ok(schema)
}

fn validate_sync_writes(
    expected_model: &ModelState,
    next_model: &ModelState,
    schema: &Schema,
    guards: &[RowGuard],
    mutations: &[Mutation],
) -> Result<(), StorageError> {
    let mut mutated = BTreeSet::new();
    for mutation in mutations {
        let (table, id, row) = match mutation {
            Mutation::Insert {
                table,
                row,
            }
            | Mutation::Replace {
                table,
                row,
            } => (table, &row.id, Some(row)),
            Mutation::Remove {
                table,
                id,
            } => (table, id, None),
        };
        if !schema.tables.contains_key(table) {
            return Err(schema_conflict(
                "command mutation addresses an unmanaged table",
            ));
        }
        if let Some(row) = row {
            validate_row(schema, table, row)?;
            if row.fields.get(ROW_VERSION) != Some(&Value::Integer(i64::from(next_model.version))) {
                return Err(schema_conflict("command row version differs from model"));
            }
        }
        mutated.insert((table.clone(), id.clone()));
    }
    let mut guarded = BTreeSet::new();
    for guard in guards {
        if !guarded.insert((guard.table.clone(), guard.id.clone())) {
            return Err(schema_conflict("duplicate command row guard"));
        }
        // SPEC-0039: also an expected-absence guard must address a declared application table.
        if !schema.tables.contains_key(&guard.table) {
            return Err(schema_conflict(
                "command guard addresses an unmanaged table",
            ));
        }
        if let Some(row) = &guard.expected {
            if row.id != guard.id {
                return Err(schema_conflict("command guard row identity differs"));
            }
            validate_row(schema, &guard.table, row)?;
            if row.fields.get(ROW_VERSION)
                != Some(&Value::Integer(i64::from(expected_model.version)))
            {
                return Err(schema_conflict("command guard version differs from model"));
            }
        }
    }
    // SPEC-0039: surplus guards are read-only expectations checked in the same commit.
    if !mutated.is_subset(&guarded) {
        return Err(schema_conflict(
            "command mutations require exact initial guards",
        ));
    }
    Ok(())
}

fn validate_entity_transition(
    plan: &SyncPlan,
    previous: &SyncLedger,
    next: &SyncLedger,
    aliases_changed: bool,
) -> Result<(), StorageError> {
    match plan.transition {
        SyncTransition::Normal => match (&previous.entity, &next.entity) {
            (None, None) => {}
            (None, Some(e)) => {
                if previous.has_live_recovery()
                    || !e.is_pristine()
                    || e.generation != 1
                    || e.revision != 0
                {
                    return Err(schema_conflict("invalid entity adoption"));
                }
            }
            (Some(_), None) => {
                return Err(schema_conflict("entity comparison state cannot disappear"));
            }
            (Some(old), Some(new)) => {
                if old.config != new.config
                    && (old.generation <= 1
                        || !previous.records.is_empty()
                        || !next.records.is_empty()
                        || plan.expected_model != plan.next_model
                        || previous.header != next.header
                        || !plan.guards.is_empty()
                        || !plan.mutations.is_empty())
                {
                    return Err(schema_conflict(
                        "entity reconfiguration requires an unchanged empty post-reset shell",
                    ));
                }
                if new.pages < old.pages
                    || new.changes < old.changes
                    || (old.ready && !new.ready)
                    || (old.checkpoint.is_some() && new.checkpoint.is_none())
                {
                    return Err(schema_conflict(
                        "bootstrap progress requires explicit reset to regress",
                    ));
                }
                let changed =
                    !plan.mutations.is_empty() || old.metadata != new.metadata || aliases_changed;
                let revision = old
                    .revision
                    .checked_add(i64::from(changed))
                    .ok_or_else(|| schema_conflict("entity revision exhausted"))?;
                if old.ledger_format != new.ledger_format
                    || old.generation != new.generation
                    || new.revision != revision
                    || (old.config != new.config && !(old.is_pristine() && new.is_pristine()))
                {
                    return Err(schema_conflict(
                        "invalid entity generation/configuration/revision transition",
                    ));
                }
            }
        },
        SyncTransition::Reset {
            discard_recovery,
        } => {
            let old = previous
                .entity
                .as_ref()
                .ok_or_else(|| schema_conflict("reset requires entity comparison state"))?;
            let new = next
                .entity
                .as_ref()
                .ok_or_else(|| schema_conflict("reset retains entity binding"))?;
            if (!discard_recovery && previous.has_live_recovery())
                || !next.records.is_empty()
                || !new.is_pristine()
                || old.config != new.config
                || old.ledger_format != new.ledger_format
                || old.generation.checked_add(1) != Some(new.generation)
                || old.revision.checked_add(1) != Some(new.revision)
                || previous.header != next.header
                || plan.next_model != plan.expected_model
                || plan
                    .mutations
                    .iter()
                    .any(|m| !matches!(m, Mutation::Remove { .. }))
                || plan.guards.iter().any(|g| g.expected.is_none())
            {
                return Err(schema_conflict("invalid explicit reset transition"));
            }
        }
    }
    Ok(())
}

/// Avoid scanning application rows for ordinary queue/pull commits.
#[doc(hidden)]
pub fn sync_inventory_required(plan: &SyncPlan) -> Result<bool, StorageError> {
    if matches!(plan.transition, SyncTransition::Reset { .. }) {
        return Ok(true);
    }
    fn config(rows: &[Row]) -> Result<Option<crate::command::entity::EntityConfig>, StorageError> {
        let Some(row) = rows.iter().find(|row| row.id == "entity:state") else {
            return Ok(None);
        };
        Ok(Some(durable::decode_entity_header(row)?.0.config))
    }
    let previous = plan
        .expected_sync
        .as_deref()
        .map(config)
        .transpose()?
        .flatten();
    let next = config(&plan.next_sync)?;
    Ok(previous
        .zip(next)
        .is_some_and(|(previous, next)| previous != next))
}

/// Reset completeness and reconfiguration emptiness are checked inside the atomic candidate.
#[doc(hidden)]
pub fn validate_reset_inventory(
    plan: &SyncPlan,
    actual: impl IntoIterator<Item = (String, String)>,
) -> Result<(), StorageError> {
    if !sync_inventory_required(plan)? {
        return Ok(());
    }
    let actual = actual.into_iter().collect::<BTreeSet<_>>();
    if matches!(plan.transition, SyncTransition::Reset { .. }) {
        let guarded = plan
            .guards
            .iter()
            .map(|g| (g.table.clone(), g.id.clone()))
            .collect::<BTreeSet<_>>();
        if actual != guarded || plan.mutations.len() != actual.len() {
            return Err(schema_conflict(
                "reset must cover the complete application inventory",
            ));
        }
    } else if !actual.is_empty() {
        return Err(schema_conflict(
            "entity reconfiguration requires empty application tables",
        ));
    }
    Ok(())
}

/// Validate the actual persisted inventory, including opt-in private format-4 aliases.
/// Legacy public `SyncLedger` decoding remains unchanged and rejects format 4.
#[doc(hidden)]
pub fn validate_sync_inventory(rows: &[Row], model: &ModelState) -> Result<(), StorageError> {
    validate_stored_model(&StoredLedger::from_rows(rows)?, model)
}

/// A schema change cannot discard retained identity authority or live recovery.
#[doc(hidden)]
pub fn validate_sync_schema_transition(
    rows: &[Row],
    model: &ModelState,
) -> Result<(), StorageError> {
    let stored = StoredLedger::from_rows(rows)?;
    if stored.ledger.has_live_recovery()
        || stored
            .ledger
            .entity
            .as_ref()
            .is_some_and(|entity| !entity.is_pristine())
        || !stored.aliases.mappings.is_empty()
    {
        return Err(schema_conflict(
            "schema transition blocked by live recovery or comparison state",
        ));
    }
    validate_stored_model(&stored, model)
}

fn validate_stored_model(stored: &StoredLedger, model: &ModelState) -> Result<(), StorageError> {
    validate_sync_model(&stored.ledger, model)?;
    for (_, local) in stored.aliases.mappings.keys() {
        validate_alias_counter(local, model)?;
    }
    Ok(())
}

fn validate_alias_counter(local: &str, model: &ModelState) -> Result<(), StorageError> {
    let counter = local
        .rsplit_once(':')
        .and_then(|(_, counter)| u64::from_str_radix(counter, 16).ok())
        .ok_or_else(|| schema_conflict("invalid durable alias identity"))?;
    if counter == 0 || counter >= model.next_id as u64 {
        return Err(schema_conflict(
            "durable alias exceeds allocated model identities",
        ));
    }
    Ok(())
}

fn validate_alias_transition(
    transition: SyncTransition,
    previous: &StoredLedger,
    next: &StoredLedger,
) -> Result<(), StorageError> {
    if previous.ledger.entity.is_some() && previous.aliases.enabled != next.aliases.enabled {
        return Err(schema_conflict(
            "entity ledger format cannot change implicitly",
        ));
    }
    if previous.ledger.entity.is_none() && !next.aliases.mappings.is_empty() {
        return Err(schema_conflict(
            "initial durable binding must have no aliases",
        ));
    }
    match transition {
        SyncTransition::Normal => {
            for (table, local) in next.aliases.mappings.keys() {
                if !previous
                    .aliases
                    .mappings
                    .contains_key(&(table.clone(), local.clone()))
                    && !previous.ledger.records.values().any(|record| {
                        !record.reconciled
                            && matches!(
                                record.state,
                                crate::command::SyncState::Applied
                                    | crate::command::SyncState::Corrected
                            )
                            && record
                                .entity
                                .as_ref()
                                .is_some_and(|entity| &entity.table == table)
                            && record.created.get(local) == Some(table)
                    })
                {
                    return Err(schema_conflict(
                        "new durable alias requires an accepted entity producer",
                    ));
                }
            }
            if previous.ledger.entity.as_ref().map(|entity| &entity.config)
                != next.ledger.entity.as_ref().map(|entity| &entity.config)
                && (!previous.aliases.mappings.is_empty() || !next.aliases.mappings.is_empty())
            {
                return Err(schema_conflict(
                    "entity reconfiguration cannot retain aliases",
                ));
            }
            if previous
                .aliases
                .mappings
                .iter()
                .any(|(id, target)| next.aliases.mappings.get(id) != Some(target))
            {
                return Err(schema_conflict("accepted entity aliases are immutable"));
            }
        }
        SyncTransition::Reset {
            ..
        } => {
            if !next.aliases.mappings.is_empty() {
                return Err(schema_conflict("reset must clear durable aliases"));
            }
        }
    }
    Ok(())
}

#[doc(hidden)]
pub fn validate_sync_model(ledger: &SyncLedger, model: &ModelState) -> Result<(), StorageError> {
    validate_sync_model_inner(ledger, model, false)
}

fn validate_sync_model_inner(
    ledger: &SyncLedger,
    model: &ModelState,
    bounded: bool,
) -> Result<(), StorageError> {
    let definition = model_definition(model)?;
    let schema = definition.storage_schema()?;
    if let Some(entity) = &ledger.entity {
        if bounded {
            entity.validate_header(&definition)?;
        } else {
            entity.validate(&definition)?;
        }
    }
    for record in ledger.records.values() {
        for change in &record.metadata_undo {
            let table = definition
                .tables
                .get(&change.table)
                .ok_or_else(|| schema_conflict("metadata undo addresses unmanaged table"))?;
            for metadata in change.before.iter().chain(change.after.iter()) {
                metadata.validate()?;
                if metadata
                    .fields
                    .keys()
                    .any(|field| !table.columns.contains_key(field))
                {
                    return Err(schema_conflict("metadata undo addresses undeclared field"));
                }
            }
        }
    }
    for key in ledger.live_keys() {
        let record = &ledger.records[&key];
        if let Some(entity) = &record.entity {
            validate_entity_payload(record, entity, &definition)?;
        }
        if record.schema_version != model.version {
            return Err(schema_conflict("live recovery schema differs from model"));
        }
        if record
            .created
            .values()
            .any(|table| !schema.tables.contains_key(table))
        {
            return Err(schema_conflict("live recovery owns an unmanaged table"));
        }
        for change in &record.undo {
            if !schema.tables.contains_key(&change.table) {
                return Err(schema_conflict("live undo addresses an unmanaged table"));
            }
            for row in change.before.iter().chain(change.after.iter()) {
                let mut raw = row.clone();
                raw.fields
                    .insert(ROW_VERSION.into(), Value::Integer(i64::from(model.version)));
                validate_row(&schema, &change.table, &raw)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_sync_model_bounded(
    ledger: &SyncLedger,
    model: &ModelState,
) -> Result<(), StorageError> {
    validate_sync_model_inner(ledger, model, true)
}

fn validate_entity_payload(
    record: &crate::command::records::SyncRecord,
    descriptor: &crate::command::EntityDescriptor,
    definition: &crate::model::SchemaDefinition,
) -> Result<(), StorageError> {
    use crate::command::{EntityOperation, Json, PathPart};
    let table = definition
        .tables
        .get(&descriptor.table)
        .ok_or_else(|| schema_conflict("entity diff addresses unmanaged table"))?;
    let Json::Object(args) = &record.args else {
        return Err(schema_conflict("invalid entity diff args"));
    };
    let Some(Json::Object(changes)) = args.get("changes") else {
        return Err(schema_conflict("invalid entity diff changes"));
    };
    let mut expected = BTreeSet::new();
    if descriptor.op != EntityOperation::Create {
        expected.insert(vec![PathPart::Key("recordId".into())]);
    }
    for (name, value) in changes {
        let column = table
            .columns
            .get(name)
            .ok_or_else(|| schema_conflict("entity diff field is undeclared"))?;
        let valid = match (value, &column.kind) {
            (Json::Null, _) => column.optional,
            (Json::String(_), ColumnType::String | ColumnType::Reference) => true,
            (Json::Integer(_), ColumnType::Integer | ColumnType::Timestamp) => true,
            (Json::Float(_), ColumnType::Float) => true,
            (Json::Bool(_), ColumnType::Boolean) => true,
            _ => false,
        };
        if !valid {
            return Err(schema_conflict("entity diff field has wrong type"));
        }
        if column.kind == ColumnType::Reference && !matches!(value, Json::Null) {
            expected.insert(vec![
                PathPart::Key("changes".into()),
                PathPart::Key(name.clone()),
            ]);
        }
    }
    if descriptor.removed_fields.iter().any(|name| {
        !table
            .columns
            .get(name)
            .is_some_and(|column| column.optional)
    }) || (descriptor.op == EntityOperation::Create
        && table
            .columns
            .iter()
            .any(|(name, column)| !column.optional && !changes.contains_key(name)))
    {
        return Err(schema_conflict(
            "entity diff removal or required field is invalid",
        ));
    }
    if record
        .reference_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err(schema_conflict(
            "entity diff reference paths differ from schema",
        ));
    }
    Ok(())
}

#[doc(hidden)]
pub fn check_sync_rows(
    actual: Option<&[Row]>,
    expected: Option<&[Row]>,
) -> Result<(), StorageError> {
    fn indexed(rows: &[Row]) -> BTreeMap<&str, &Row> {
        rows.iter().map(|row| (row.id.as_str(), row)).collect()
    }
    let equal = match (actual, expected) {
        (None, None) => true,
        (Some(a), Some(b)) => a.len() == b.len() && indexed(a) == indexed(b),
        _ => false,
    };
    if equal {
        Ok(())
    } else {
        Err(schema_conflict("command recovery changed before commit"))
    }
}

#[cfg(test)]
mod delta_parts_tests {
    use super::*;
    use crate::{
        command::{
            Json,
            records::{SyncHeader, SyncOptions},
        },
        model::{SchemaDefinition, TableDefinition},
    };

    fn parts() -> SyncDeltaParts {
        let definition = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let model = ModelState {
            version: 1,
            definition: definition.canonical().unwrap(),
            next_id: 2,
        };
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "a".repeat(32),
                sequence: 2,
                last_ms: 10,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        };
        let state = ledger.to_rows().unwrap().remove(0);
        SyncDeltaParts {
            expected_model: model.clone(),
            next_model: model,
            expected_packets: vec![
                ("state".into(), Some(state.clone())),
                ("entity:state".into(), None),
            ],
            upserts: vec![state],
            deletes: vec![],
            guards: vec![],
            mutations: vec![],
        }
    }

    fn edit_header(parts: &mut SyncDeltaParts, field: &str, value: Json) {
        edit_packet(&mut parts.upserts[0], field, value);
    }

    fn edit_packet(row: &mut Row, field: &str, value: Json) {
        let Some(Value::String(data)) = row.fields.get("data") else {
            panic!("data")
        };
        let Json::Object(mut fields) = Json::parse(data).unwrap() else {
            panic!("header")
        };
        fields.insert(field.into(), value);
        row.fields.insert(
            "data".into(),
            Value::String(Json::Object(fields).canonical().unwrap()),
        );
    }

    fn entity_parts(pristine: bool) -> SyncDeltaParts {
        use crate::command::{EntityConfig, EntityLedger, records::entity_header_row};
        let mut p = parts();
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "entities".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        // This R9 revision fixture intentionally uses ordinal format-2 packets.
        entity.ledger_format = 2;
        entity.generation = 2;
        if !pristine {
            entity.revision = 3;
            entity.pages = 2;
            entity.changes = 2;
            entity.ready = true;
            entity.checkpoint = Some("checkpoint".into());
        }
        let row = entity_header_row(&entity).unwrap();
        p.expected_packets[1].1 = Some(row.clone());
        p.upserts.push(row);
        p
    }

    #[test]
    fn si_y03_delta_entity_header_refusals() {
        for fault in 0..8 {
            let mut p = entity_parts(false);
            assert!(validate_delta_parts(&p).is_ok());
            match fault {
                0 => {
                    p.upserts.pop();
                    p.deletes.push("entity:state".into());
                }
                1 => p.expected_packets[1].1 = None,
                2 => edit_packet(&mut p.upserts[1], "generation", Json::Integer(3)),
                3 => edit_packet(&mut p.upserts[1], "revision", Json::Integer(5)),
                4 => edit_packet(&mut p.upserts[1], "pages", Json::Integer(1)),
                5 => edit_packet(&mut p.upserts[1], "changes", Json::Integer(1)),
                6 => edit_packet(&mut p.upserts[1], "ready", Json::Bool(false)),
                _ => edit_packet(&mut p.upserts[1], "checkpoint", Json::Null),
            }
            assert!(validate_delta_parts(&p).is_err(), "entity fault {fault}");
        }
        let mut p = parts();
        p.expected_packets.push(("command:missing".into(), None));
        p.deletes.push("command:missing".into());
        assert!(
            validate_delta_parts(&p).is_err(),
            "single deletion needs prior image"
        );
    }

    #[test]
    fn si_y03_delta_rejects_unearned_revision_increment() {
        let mut p = entity_parts(false);
        edit_packet(&mut p.upserts[1], "revision", Json::Integer(4));
        assert!(validate_delta_parts(&p).is_err());
    }

    #[test]
    fn si_y03_delta_requires_revision_for_application_mutation() {
        let mut p = entity_parts(false);
        p.mutations.push(Mutation::Remove {
            table: "items".into(),
            id: "absent".into(),
        });
        p.guards.push(RowGuard {
            table: "items".into(),
            id: "absent".into(),
            expected: None,
        });
        assert!(validate_delta_parts(&p).is_err());
        edit_packet(&mut p.upserts[1], "revision", Json::Integer(4));
        assert!(validate_delta_parts(&p).is_ok());
    }

    #[test]
    fn si_y03_delta_requires_revision_for_metadata_insert_and_delete() {
        use crate::command::{RowMetadata, records::metadata_row};
        for deleting in [false, true] {
            let mut p = entity_parts(false);
            let row = metadata_row(0, "items", "metadata-only", &RowMetadata::default()).unwrap();
            p.expected_packets.push((
                row.id.clone(),
                if deleting {
                    Some(row.clone())
                } else {
                    None
                },
            ));
            if deleting {
                p.deletes.push(row.id);
            } else {
                p.upserts.push(row);
            }
            assert!(
                validate_delta_parts(&p).is_err(),
                "metadata delete={deleting}"
            );
            edit_packet(&mut p.upserts[1], "revision", Json::Integer(4));
            assert!(
                validate_delta_parts(&p).is_ok(),
                "metadata delete={deleting}"
            );
        }
    }

    #[test]
    fn si_y03_delta_reconfiguration_requires_full_inventory_plan() {
        let mut p = entity_parts(true);
        let config = Json::Object(BTreeMap::from([
            ("scope".into(), Json::String("new-scope".into())),
            (
                "tables".into(),
                Json::Array(vec![Json::String("items".into())]),
            ),
        ]));
        edit_packet(&mut p.upserts[1], "config", config);
        assert!(validate_delta_parts(&p).is_err());
    }

    #[test]
    fn si_y03_valid_delta_payload_and_read_only_guard() {
        let mut p = parts();
        assert!(validate_delta_parts(&p).is_ok());
        p.guards.push(RowGuard {
            table: "items".into(),
            id: "absent".into(),
            expected: None,
        });
        assert!(
            validate_delta_parts(&p).is_ok(),
            "read-only absence guards remain legal"
        );
    }

    #[test]
    fn si_y03_delta_rejects_duplicate_or_overlapping_lists() {
        for duplicate in 0..4 {
            let mut p = parts();
            match duplicate {
                0 => p.expected_packets.push(p.expected_packets[0].clone()),
                1 => p.upserts.push(p.upserts[0].clone()),
                2 => {
                    p.deletes = vec!["command:gone".into(), "command:gone".into()];
                    p.expected_packets.push(("command:gone".into(), None));
                }
                _ => p.deletes.push("state".into()),
            }
            assert!(
                validate_delta_parts(&p).is_err(),
                "invalid list case {duplicate}"
            );
        }
    }

    #[test]
    fn si_y03_delta_requires_exact_expected_id_union() {
        for fault in 0..4 {
            let mut p = parts();
            match fault {
                0 => {
                    p.expected_packets.remove(0);
                }
                1 => {
                    p.expected_packets.remove(1);
                }
                2 => p.expected_packets.push(("command:surplus".into(), None)),
                _ => p.expected_packets[0].1.as_mut().unwrap().id = "other".into(),
            }
            assert!(
                validate_delta_parts(&p).is_err(),
                "expectation case {fault}"
            );
        }
    }

    #[test]
    fn si_y03_delta_rejects_model_or_header_regression() {
        for fault in 0..6 {
            let mut p = parts();
            match fault {
                0 => p.next_model.next_id = 1,
                1 => p.next_model.version = 2,
                2 => edit_header(&mut p, "namespace", Json::String("b".repeat(32))),
                3 => edit_header(&mut p, "sequence", Json::Integer(1)),
                4 => edit_header(&mut p, "last_ms", Json::Integer(9)),
                _ => {
                    p.upserts.clear();
                    p.deletes.push("state".into());
                }
            }
            assert!(validate_delta_parts(&p).is_err(), "regression case {fault}");
        }
    }

    #[test]
    fn si_y03_delta_rejects_bad_packet_schema_or_identity() {
        for fault in 0..5 {
            let mut p = parts();
            match fault {
                0 => {
                    p.upserts[0].fields.insert("extra".into(), Value::Null);
                }
                1 => {
                    p.upserts[0]
                        .fields
                        .insert("format".into(), Value::Integer(2));
                }
                2 => {
                    p.upserts[0]
                        .fields
                        .insert("data".into(), Value::String("{} ".into()));
                }
                3 => {
                    p.upserts[0].id = "unknown".into();
                    p.expected_packets.push(("unknown".into(), None));
                }
                _ => {
                    p.expected_packets[0]
                        .1
                        .as_mut()
                        .unwrap()
                        .fields
                        .insert("format".into(), Value::Integer(99));
                }
            }
            assert!(validate_delta_parts(&p).is_err(), "packet case {fault}");
        }
    }

    #[test]
    fn si_y03_delta_requires_guards_for_mutations() {
        let mut p = parts();
        p.mutations.push(Mutation::Remove {
            table: "items".into(),
            id: "absent".into(),
        });
        assert!(validate_delta_parts(&p).is_err());
        p.guards.push(RowGuard {
            table: "items".into(),
            id: "absent".into(),
            expected: None,
        });
        assert!(validate_delta_parts(&p).is_ok());
        p.guards.push(p.guards[0].clone());
        assert!(validate_delta_parts(&p).is_err(), "duplicate guard");
    }

    #[test]
    fn si_y01_bounded_model_validation_does_not_visit_untouched_metadata() {
        use crate::command::{EntityConfig, EntityLedger, RowMetadata};
        let p = parts();
        for m in [10, 1000] {
            let mut ledger = SyncLedger::from_rows(&p.upserts).unwrap();
            let mut entity = EntityLedger::new(EntityConfig {
                scope: "bounded".into(),
                tables: BTreeSet::from(["items".into()]),
            });
            for i in 0..m {
                entity.metadata.insert(
                    ("items".into(), format!("r{i}")),
                    RowMetadata {
                        fields: BTreeMap::new(),
                        tombstone: None,
                    },
                );
            }
            ledger.entity = Some(entity);
            validate_sync_model(&ledger, &p.expected_model).unwrap();
            counters::reset();
            validate_sync_model_bounded(&ledger, &p.expected_model).unwrap();
            assert_eq!(counters::snapshot().metadata_entries_visited, 0);
            ledger.entity.as_mut().unwrap().metadata.insert(
                ("missing-table".into(), "unrelated".into()),
                RowMetadata {
                    fields: BTreeMap::new(),
                    tombstone: None,
                },
            );
            assert!(validate_sync_model(&ledger, &p.expected_model).is_err());
            assert!(validate_sync_model_bounded(&ledger, &p.expected_model).is_ok());
        }
    }
}

#[cfg(test)]
mod entity_transition_tests {
    use super::*;
    use crate::{
        command::records::{SyncHeader, SyncOptions},
        model::SchemaDefinition,
    };
    #[test]
    fn e17_reset_requires_a_loaded_entity_ledger() {
        let definition = SchemaDefinition {
            version: 1,
            tables: BTreeMap::new(),
        };
        let model = ModelState {
            version: 1,
            definition: definition.canonical().unwrap(),
            next_id: 1,
        };
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "a".repeat(32),
                sequence: 1,
                last_ms: 0,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        };
        let plan = SyncPlan {
            transition: SyncTransition::Reset {
                discard_recovery: true,
            },
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        };
        assert!(
            validate_sync_plan(&plan).is_err(),
            "reset cannot masquerade as initial recovery bookkeeping"
        );
    }
}

#[cfg(test)]
mod read_only_guard_tests {
    // SPEC-0039 / TC-0043: mutated rows must be a subset of guarded rows; extra guards are
    // read-only expectations. Reset inventory, duplicate and unknown-table checks are unchanged.
    use super::*;
    use crate::{
        command::records::{SyncHeader, SyncOptions},
        model::{SchemaDefinition, TableDefinition},
        storage::{Column, ColumnType},
    };

    fn plan(guards: Vec<RowGuard>, mutations: Vec<Mutation>) -> SyncPlan {
        let definition = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "value".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    )]),
                    relations: BTreeMap::new(),
                },
            )]),
        };
        let model = ModelState {
            version: 1,
            definition: definition.canonical().unwrap(),
            next_id: 1,
        };
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "a".repeat(32),
                sequence: 1,
                last_ms: 0,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        };
        SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards,
            mutations,
        }
    }

    fn guard(table: &str, id: &str) -> RowGuard {
        RowGuard {
            table: table.into(),
            id: id.into(),
            expected: None,
        }
    }

    #[test]
    fn rg_read_only_absence_guard_is_a_valid_plan() {
        assert!(validate_sync_plan(&plan(vec![guard("items", "absent")], vec![])).is_ok());
    }

    #[test]
    fn rg_unknown_table_and_duplicate_guards_are_refused() {
        for table in ["unmanaged", "__hoya_sync", "__hoya_model"] {
            assert!(validate_sync_plan(&plan(vec![guard(table, "x")], vec![])).is_err());
        }
        let twice = vec![guard("items", "x"), guard("items", "x")];
        assert!(validate_sync_plan(&plan(twice, vec![])).is_err());
        let unguarded = vec![Mutation::Remove {
            table: "items".into(),
            id: "x".into(),
        }];
        assert!(validate_sync_plan(&plan(vec![guard("items", "y")], unguarded)).is_err());
    }

    #[test]
    fn rg_reset_inventory_still_requires_exact_guards() {
        let mut reset = plan(
            vec![
                RowGuard {
                    expected: Some(Row {
                        id: "a".into(),
                        fields: BTreeMap::new(),
                    }),
                    ..guard("items", "a")
                },
                guard("items", "extra"),
            ],
            vec![Mutation::Remove {
                table: "items".into(),
                id: "a".into(),
            }],
        );
        reset.transition = SyncTransition::Reset {
            discard_recovery: true,
        };
        let actual = [("items".to_string(), "a".to_string())];
        assert!(validate_reset_inventory(&reset, actual.clone()).is_err());
        reset.guards.pop();
        assert!(validate_reset_inventory(&reset, actual).is_ok());
    }
}

#[cfg(test)]
mod format_retention_tests {
    use super::*;
    use crate::{
        command::{
            EntityConfig, EntityLedger, Json,
            records::{SyncHeader, SyncOptions},
        },
        model::{SchemaDefinition, TableDefinition},
    };

    fn model() -> ModelState {
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        ModelState {
            version: 1,
            definition: schema.canonical().unwrap(),
            next_id: 1,
        }
    }

    fn ledger(format: u8) -> SyncLedger {
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "format-retention".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        entity.ledger_format = format;
        SyncLedger {
            entity: Some(entity),
            header: SyncHeader {
                namespace: "a".repeat(32),
                sequence: 1,
                last_ms: 0,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        }
    }

    fn checked_rows(ledger: &SyncLedger, format: u8) -> Vec<Row> {
        let rows = ledger
            .to_rows()
            .expect("intrinsically valid ledger serializes");
        let decoded = SyncLedger::from_rows(&rows)
            .expect("both valid formats decode before transition assertions");
        assert_eq!(decoded.entity.as_ref().unwrap().ledger_format, format);
        assert_eq!(&decoded, ledger, "format and all ledger fields round trip");
        rows
    }

    fn plan(from: u8, to: u8, transition: SyncTransition) -> SyncPlan {
        let old = ledger(from);
        let mut new = old.clone();
        let entity = new.entity.as_mut().unwrap();
        entity.ledger_format = to;
        if matches!(transition, SyncTransition::Reset { .. }) {
            entity.generation += 1;
            entity.revision += 1;
        }
        let model = model();
        SyncPlan {
            transition,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: Some(checked_rows(&old, from)),
            next_sync: checked_rows(&new, to),
            guards: vec![],
            mutations: vec![],
        }
    }

    fn delta(from: u8, to: u8) -> SyncDeltaParts {
        let old = checked_rows(&ledger(from), from);
        let next = checked_rows(&ledger(to), to);
        let model = model();
        SyncDeltaParts {
            expected_model: model.clone(),
            next_model: model,
            expected_packets: old
                .into_iter()
                .map(|row| (row.id.clone(), Some(row)))
                .collect(),
            upserts: next
                .into_iter()
                .filter(|row| row.id == "entity:state")
                .collect(),
            deletes: vec![],
            guards: vec![],
            mutations: vec![],
        }
    }

    fn refuse_plan_flip(from: u8, to: u8, transition: SyncTransition) {
        validate_sync_plan(&plan(from, from, transition))
            .expect("valid source-format positive control");
        validate_sync_plan(&plan(to, to, transition))
            .expect("valid target-format positive control");
        assert!(
            matches!(
                validate_sync_plan(&plan(from, to, transition)),
                Err(StorageError::SchemaConflict { .. })
            ),
            "a valid ordinary full plan must not change entity ledger format {from} to {to}"
        );
    }
    fn refuse_delta_flip(from: u8, to: u8) {
        validate_delta_parts(&delta(from, from)).expect("valid source-format delta control");
        validate_delta_parts(&delta(to, to)).expect("valid target-format delta control");
        assert!(
            matches!(
                validate_delta_parts(&delta(from, to)),
                Err(StorageError::SchemaConflict { .. })
            ),
            "valid delta parts must not change entity ledger format {from} to {to}"
        );
    }

    #[test]
    fn si_f02_same_format_full_plan_controls_cover_both_resets() {
        for format in [2, 3] {
            for transition in [
                SyncTransition::Normal,
                SyncTransition::Reset {
                    discard_recovery: false,
                },
                SyncTransition::Reset {
                    discard_recovery: true,
                },
            ] {
                validate_sync_plan(&plan(format, format, transition)).unwrap();
            }
        }
    }
    #[test]
    fn si_f02_same_format_delta_controls() {
        for format in [2, 3] {
            validate_delta_parts(&delta(format, format)).unwrap();
        }
    }
    #[test]
    fn si_f02_normal_refuses_2_to_3() {
        refuse_plan_flip(2, 3, SyncTransition::Normal);
    }
    #[test]
    fn si_f02_normal_refuses_3_to_2() {
        refuse_plan_flip(3, 2, SyncTransition::Normal);
    }
    #[test]
    fn si_f02_preserve_reset_refuses_2_to_3() {
        refuse_plan_flip(
            2,
            3,
            SyncTransition::Reset {
                discard_recovery: false,
            },
        );
    }
    #[test]
    fn si_f02_preserve_reset_refuses_3_to_2() {
        refuse_plan_flip(
            3,
            2,
            SyncTransition::Reset {
                discard_recovery: false,
            },
        );
    }
    #[test]
    fn si_f02_discard_reset_refuses_2_to_3() {
        refuse_plan_flip(
            2,
            3,
            SyncTransition::Reset {
                discard_recovery: true,
            },
        );
    }
    #[test]
    fn si_f02_discard_reset_refuses_3_to_2() {
        refuse_plan_flip(
            3,
            2,
            SyncTransition::Reset {
                discard_recovery: true,
            },
        );
    }
    #[test]
    fn si_f02_delta_refuses_2_to_3() {
        refuse_delta_flip(2, 3);
    }
    #[test]
    fn si_f02_delta_refuses_3_to_2() {
        refuse_delta_flip(3, 2);
    }

    fn invalid_marker(rows: &mut [Row], value: i64) {
        let row = rows
            .iter_mut()
            .find(|row| row.id == "entity:state")
            .unwrap();
        let Value::String(encoded) = &row.fields["data"] else {
            panic!("data")
        };
        let Json::Object(mut data) = Json::parse(encoded).unwrap() else {
            panic!("object")
        };
        data.insert("ledger_format".into(), Json::Integer(value));
        row.fields.insert(
            "data".into(),
            Value::String(Json::Object(data).canonical().unwrap()),
        );
    }
    #[test]
    fn si_f03_storage_validators_retain_intrinsic_invalid_marker_refusal() {
        for marker in [0, 4] {
            for prior in [false, true] {
                let mut p = plan(3, 3, SyncTransition::Normal);
                validate_sync_plan(&p).unwrap();
                invalid_marker(
                    if prior {
                        p.expected_sync.as_mut().unwrap()
                    } else {
                        &mut p.next_sync
                    },
                    marker,
                );
                assert!(validate_sync_plan(&p).is_err());
                let mut d = delta(3, 3);
                validate_delta_parts(&d).unwrap();
                if prior {
                    let (_, image) = d
                        .expected_packets
                        .iter_mut()
                        .find(|(id, _)| id == "entity:state")
                        .unwrap();
                    invalid_marker(std::slice::from_mut(image.as_mut().unwrap()), marker);
                } else {
                    invalid_marker(&mut d.upserts, marker);
                }
                assert!(validate_delta_parts(&d).is_err());
            }
        }
    }
}
