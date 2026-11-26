//! API-0007 / TC-0032 C06/C11/C17: exact, redacted recovery bookkeeping.
use super::{Json, entity::*};
use crate::storage::{CommittedChange, Row, StorageError, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncOptions {
    pub capacity: usize,
    pub history: usize,
    pub initial_delay_ms: i64,
    pub multiplier: u32,
    pub max_delay_ms: i64,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            capacity: 1000,
            history: 100,
            initial_delay_ms: 1000,
            multiplier: 2,
            max_delay_ms: 60000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncHeader {
    pub namespace: String,
    pub sequence: i64,
    pub last_ms: i64,
    pub options: SyncOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathPart {
    Key(String),
    Index(usize),
}

#[derive(Clone, PartialEq, Eq)]
pub enum RemoteOutcome {
    Applied(Json),
    Corrected(Json),
    Rejected(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncState {
    Discarded,
    Queued,
    Sent,
    Acknowledged,
    Applied,
    Corrected,
    Rejected,
    Cancelled,
}

#[derive(PartialEq, Eq)]
pub struct SyncRecord {
    pub entity: Option<EntityDescriptor>,
    pub metadata_undo: Vec<MetadataChange>,
    pub key: String,
    pub command: String,
    pub args: Json,
    pub context: Json,
    pub schema_version: u32,
    pub order: i64,
    pub reference_paths: Vec<Vec<PathPart>>,
    pub dependencies: BTreeSet<String>,
    pub created: BTreeMap<String, String>,
    pub mappings: BTreeMap<String, String>,
    pub undo: Vec<CommittedChange>,
    pub state: SyncState,
    pub frozen: Option<String>,
    pub attempts: u32,
    pub due_ms: i64,
    pub outcome: Option<RemoteOutcome>,
    pub rolled_back: bool,
    pub reconciled: bool,
}

/// SPEC-0037 R4: every copy counts one command record; its nested metadata
/// images count through `RowMetadata`; nested undo `Row` images count here.
/// Exhaustive destructuring makes a new
/// field a compile error here.
impl Clone for SyncRecord {
    fn clone(&self) -> Self {
        let Self {
            entity,
            metadata_undo,
            key,
            command,
            args,
            context,
            schema_version,
            order,
            reference_paths,
            dependencies,
            created,
            mappings,
            undo,
            state,
            frozen,
            attempts,
            due_ms,
            outcome,
            rolled_back,
            reconciled,
        } = self;
        crate::storage::count_work!(ledger_entries_cloned, 1);
        crate::storage::count_work!(
            rows_cloned,
            undo.iter()
                .map(|change| u64::from(change.before.is_some()) + u64::from(change.after.is_some()))
                .sum::<u64>()
        );
        Self {
            entity: entity.clone(),
            metadata_undo: metadata_undo.clone(),
            key: key.clone(),
            command: command.clone(),
            args: args.clone(),
            context: context.clone(),
            schema_version: *schema_version,
            order: *order,
            reference_paths: reference_paths.clone(),
            dependencies: dependencies.clone(),
            created: created.clone(),
            mappings: mappings.clone(),
            undo: undo.clone(),
            state: state.clone(),
            frozen: frozen.clone(),
            attempts: *attempts,
            due_ms: *due_ms,
            outcome: outcome.clone(),
            rolled_back: *rolled_back,
            reconciled: *reconciled,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SyncLedger {
    pub entity: Option<EntityLedger>,
    pub header: SyncHeader,
    pub records: BTreeMap<String, SyncRecord>,
}

impl SyncRecord {
    pub fn check_send_size(&self) -> Result<(), StorageError> {
        encode_record(self).canonical().map_err(|_| invalid())?;
        encode_extension(self).canonical().map_err(|_| invalid())?;
        let mut frozen = self.clone();
        frozen.frozen = Some(self.envelope().canonical().map_err(|_| invalid())?);
        let encoded = encode_record(&frozen).canonical().map_err(|_| invalid())?;
        // Enqueue must leave room for later retry counters and receipt state;
        // those scalars grow independently of the immutable frozen envelope.
        let growth = u32::MAX
            .to_string()
            .len()
            .saturating_sub(self.attempts.to_string().len())
            + i64::MAX
                .to_string()
                .len()
                .saturating_sub(self.due_ms.to_string().len())
            + state_name(&SyncState::Acknowledged)
                .len()
                .saturating_sub(state_name(&self.state).len());
        if encoded
            .len()
            .checked_add(growth)
            .is_none_or(|length| length > super::json::MAX_BYTES)
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub fn envelope(&self) -> Json {
        if let Some(entity) = &self.entity {
            let args = match &self.args {
                Json::Object(args) => args,
                _ => return Json::Null,
            };
            return obj([
                ("table", string(&entity.table)),
                ("op", string(op_name(entity.op))),
                (
                    "recordId",
                    args.get("recordId").cloned().unwrap_or(Json::Null),
                ),
                (
                    "changes",
                    args.get("changes").cloned().unwrap_or(Json::Null),
                ),
                (
                    "removedFields",
                    Json::Array(
                        entity
                            .removed_fields
                            .iter()
                            .map(|name| string(name))
                            .collect(),
                    ),
                ),
                (
                    "clientTimestamp",
                    string(&super::owner::utc_time(entity.timestamp_ms)),
                ),
                ("schemaVersion", integer(i64::from(self.schema_version))),
                ("context", self.context.clone()),
                ("writeId", string(&self.key)),
            ]);
        }
        Json::Object(BTreeMap::from([
            ("args".into(), self.args.clone()),
            ("command".into(), Json::String(self.command.clone())),
            ("context".into(), self.context.clone()),
            ("idempotencyKey".into(), Json::String(self.key.clone())),
            (
                "schemaVersion".into(),
                Json::Integer(i64::from(self.schema_version)),
            ),
        ]))
    }

    pub fn is_live(&self) -> bool {
        match self.state {
            SyncState::Discarded => false,
            SyncState::Queued | SyncState::Sent | SyncState::Acknowledged => true,
            SyncState::Applied | SyncState::Corrected => !self.reconciled,
            SyncState::Rejected => !self.reconciled || !self.rolled_back,
            SyncState::Cancelled => !self.rolled_back || !self.reconciled || self.frozen.is_some(),
        }
    }
}

impl SyncLedger {
    pub fn from_rows(rows: &[Row]) -> Result<Self, StorageError> {
        let mut header = None;
        let mut records = BTreeMap::new();
        let mut entity = None;
        let mut extensions = BTreeMap::new();
        let mut metadata = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for row in rows {
            if !ids.insert(&row.id) {
                return Err(invalid());
            }
            let json = decode_packet(row)?;
            if row.id == "state" {
                header = Some(decode_header(&json)?);
            } else if let Some(key) = row.id.strip_prefix("command:") {
                let record = decode_record(&json)?;
                if key != record.key {
                    return Err(invalid());
                }
                records.insert(record.key.clone(), record);
            } else if row.id == "entity:state" {
                entity = Some(header_from_json(&json)?);
            } else if let Some(key) = row.id.strip_prefix("entity:record:") {
                extensions.insert(key.to_owned(), decode_extension(&json)?);
            } else if let Some(ordinal) = row.id.strip_prefix("entity:metadata:") {
                let f = fields(&json, &["table", "record_id", "metadata"])?;
                metadata.insert(
                    ordinal,
                    (
                        (
                            text(&f["table"])?.to_owned(),
                            text(&f["record_id"])?.to_owned(),
                        ),
                        metadata_from_json(&f["metadata"])?,
                    ),
                );
            } else {
                return Err(invalid());
            }
        }
        if entity.is_none() && (!extensions.is_empty() || !metadata.is_empty()) {
            return Err(invalid());
        }
        for (key, (descriptor, undo)) in extensions {
            let record = records.get_mut(&key).ok_or_else(invalid)?;
            record.entity = descriptor;
            record.metadata_undo = undo;
        }
        if let Some(entity) = &mut entity {
            let mut previous = None;
            for (expected, (ordinal, (identity, value))) in metadata.into_iter().enumerate() {
                crate::storage::count_work!(metadata_entries_visited, 1);
                let rank = validate_metadata_suffix(
                    entity.ledger_format,
                    ordinal,
                    &identity.0,
                    &identity.1,
                )?;
                if entity.ledger_format == 2
                    && (rank != Some(expected as u64)
                        || previous.as_ref().is_some_and(|p| p >= &identity))
                {
                    return Err(invalid());
                }
                previous = Some(identity.clone());
                if entity.metadata.insert(identity, value).is_some() {
                    return Err(invalid());
                }
            }
        }
        let ledger = Self {
            entity,
            header: header.ok_or_else(invalid)?,
            records,
        };
        ledger.validate()?;
        Ok(ledger)
    }

    pub fn to_rows(&self) -> Result<Vec<Row>, StorageError> {
        self.validate()?;
        let mut rows = vec![sync_header_row(&self.header)?];
        let mut extensions = Vec::new();
        for record in self.records.values() {
            let mut encoded = record_rows(record)?.into_iter();
            rows.push(
                encoded
                    .next()
                    .expect("record always has its command packet"),
            );
            extensions.extend(encoded);
        }
        if let Some(entity) = &self.entity {
            rows.push(entity_header_row(entity)?);
            rows.extend(extensions);
            for (ordinal, ((table, id), metadata)) in entity.metadata.iter().enumerate() {
                crate::storage::count_work!(metadata_entries_visited, 1);
                rows.push(if entity.ledger_format == 3 {
                    identity_metadata_row(table, id, metadata)?
                } else {
                    metadata_row(ordinal as u64, table, id, metadata)?
                });
            }
            if entity.ledger_format == 3 {
                rows.sort_by(|left, right| left.id.cmp(&right.id));
            }
        }
        Ok(rows)
    }

    /// Checks all header, command, dependency and history invariants without
    /// revisiting untouched comparison metadata of a validated owner.
    pub(crate) fn validate_bounded(&self) -> Result<(), StorageError> {
        if let Some(entity) = &self.entity {
            entity.validate_header_intrinsic()?;
        }
        self.validate_records()
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        if let Some(entity) = &self.entity {
            entity.validate_intrinsic()?;
        }
        self.validate_records()
    }

    fn validate_records(&self) -> Result<(), StorageError> {
        let h = &self.header;
        if h.namespace.len() != 32
            || !h.namespace.bytes().all(lower_hex)
            || h.sequence < 1
            || h.last_ms < 0
            || h.options.capacity == 0
            || h.options.initial_delay_ms < 1
            || h.options.multiplier < 2
            || h.options.max_delay_ms < h.options.initial_delay_ms
            || i64::try_from(h.options.capacity).is_err()
            || i64::try_from(h.options.history).is_err()
        {
            return Err(invalid());
        }
        let mut orders = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut final_ids = BTreeSet::new();
        for (key, r) in &self.records {
            if key != &r.key || !orders.insert(r.order) {
                return Err(invalid());
            }
            if (r.entity.is_some() || !r.metadata_undo.is_empty()) && self.entity.is_none() {
                return Err(invalid());
            }
            validate_record(r, h)?;
            encode_extension(r).canonical().map_err(|_| invalid())?;
            for change in &r.metadata_undo {
                validate_metadata_change(change)?;
            }
            if let Some(entity) = &r.entity
                && !self
                    .entity
                    .as_ref()
                    .is_some_and(|state| state.config.tables.contains(&entity.table))
            {
                return Err(invalid());
            }
            encode_record(r).canonical().map_err(|_| invalid())?;
            for (id, table) in &r.created {
                let entity_owned=r.entity.as_ref().is_some_and(|entity| entity.op==EntityOperation::Create && entity.table==*table && valid_entity_id(id,&h.namespace) && matches!(&r.args,Json::Object(args) if args.get("recordId")==Some(&string(id))));
                if (!(id.starts_with("local:") && id.len() > 6) && !entity_owned)
                    || (r.entity.is_some() && !entity_owned)
                    || crate::model::check_name(table).is_err()
                    || owners.insert(id.clone(), r.key.clone()).is_some()
                {
                    return Err(invalid());
                }
            }
            for (local, remote) in &r.mappings {
                let table = r.created.get(local).ok_or_else(invalid)?;
                if remote.is_empty()
                    || remote.starts_with("local:")
                    || !final_ids.insert((table.clone(), remote.clone()))
                {
                    return Err(invalid());
                }
            }
        }
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for r in self.records.values() {
            let mut expected = BTreeSet::new();
            let mut paths = BTreeSet::new();
            for path in &r.reference_paths {
                if path.is_empty() || !paths.insert(path) {
                    return Err(invalid());
                }
                let id = text(path_value(&r.args, path).ok_or_else(invalid)?)?;
                if id.is_empty() {
                    return Err(invalid());
                }
                if let Some(parent) = owners.get(id) {
                    match self.records[parent].mappings.get(id) {
                        Some(mapped) if mapped == id && id.starts_with("entity:") => (),
                        Some(_) => return Err(invalid()),
                        None => {
                            expected.insert(id.to_owned());
                            edges
                                .entry(r.key.clone())
                                .or_default()
                                .insert(parent.clone());
                        }
                    }
                } else if id.starts_with("local:") {
                    return Err(invalid());
                }
            }
            if expected != r.dependencies {
                return Err(invalid());
            }
        }
        // Iterative topological removal avoids stack growth on large configured DAGs.
        let mut remaining = self.records.keys().cloned().collect::<BTreeSet<_>>();
        while !remaining.is_empty() {
            let ready = remaining
                .iter()
                .filter(|key| {
                    edges
                        .get(*key)
                        .is_none_or(|parents| parents.is_disjoint(&remaining))
                })
                .cloned()
                .collect::<Vec<_>>();
            if ready.is_empty() {
                return Err(invalid());
            }
            for key in ready {
                remaining.remove(&key);
            }
        }
        let live = self.live_keys();
        if live.len() > h.options.capacity || self.records.len() - live.len() > h.options.history {
            return Err(invalid());
        }
        Ok(())
    }

    pub fn live_keys(&self) -> BTreeSet<String> {
        self.live_key_refs()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    pub(super) fn live_key_refs(&self) -> BTreeSet<&str> {
        let mut live = self
            .records
            .values()
            .filter(|record| record.is_live())
            .map(|record| record.key.as_str())
            .collect::<BTreeSet<_>>();
        loop {
            let mut needed = BTreeSet::new();
            for key in &live {
                needed.extend(self.retained_producers(key));
            }
            let before = live.len();
            live.extend(needed);
            if live.len() == before {
                return live;
            }
        }
    }

    /// The producers one live record keeps live: every record whose unmapped or
    /// renamed creation it still names in dependencies, reference paths or undo images.
    fn retained_producers(&self, key: &str) -> Vec<&str> {
        let Some(record) = self.records.get(key) else {
            return vec![];
        };
        let mut refs = record
            .dependencies
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for path in &record.reference_paths {
            if let Some(Json::String(id)) = path_value(&record.args, path) {
                refs.insert(id.as_str());
            }
        }
        for change in &record.undo {
            for row in change.before.iter().chain(change.after.iter()) {
                for value in row.fields.values() {
                    if let Value::Reference(id) = value {
                        refs.insert(id.as_str());
                    }
                }
            }
        }
        self.records
            .values()
            .filter(|producer| {
                producer
                    .created
                    .keys()
                    .any(|id| refs.contains(id.as_str()) && producer.mappings.get(id) != Some(id))
            })
            .map(|producer| producer.key.as_str())
            .collect()
    }

    /// Extends a complete `live_keys` result by one newly inserted live record,
    /// visiting only that record's own retention closure. Records already live
    /// keep their closure; no other record may have changed since `live` was taken.
    pub(super) fn extend_live(&self, live: &mut BTreeSet<String>, key: &str) {
        let mut pending = vec![key];
        while let Some(key) = pending.pop() {
            if live.insert(key.to_owned()) {
                pending.extend(self.retained_producers(key));
            }
        }
    }

    pub fn has_live_recovery(&self) -> bool {
        !self.live_keys().is_empty()
    }
}

fn invalid() -> StorageError {
    StorageError::BackendFailure {
        message: "invalid command recovery bookkeeping".into(),
    }
}

fn obj<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn string(value: &str) -> Json {
    Json::String(value.into())
}

fn integer(value: i64) -> Json {
    Json::Integer(value)
}

fn encode_extension(record: &SyncRecord) -> Json {
    obj([
        (
            "entity",
            record
                .entity
                .as_ref()
                .map(descriptor_json)
                .unwrap_or(Json::Null),
        ),
        (
            "metadata_undo",
            Json::Array(
                record
                    .metadata_undo
                    .iter()
                    .map(metadata_change_json)
                    .collect(),
            ),
        ),
    ])
}
fn entity_packet(id: String, data: Json) -> Result<Row, StorageError> {
    let mut row = packet(id, data)?;
    row.fields.insert("format".into(), Value::Integer(2));
    Ok(row)
}
fn valid_entity_id(id: &str, namespace: &str) -> bool {
    let Some(rest) = id.strip_prefix("entity:") else {
        return false;
    };
    let Some((store, counter)) = rest.split_once(':') else {
        return false;
    };
    store == namespace
        && counter.len() == 16
        && counter.bytes().all(lower_hex)
        && i64::from_str_radix(counter, 16).is_ok_and(|n| n > 0)
}

/// Exact envelope decoding shared by full recovery and bounded delta checks.
pub(crate) fn decode_packet(row: &Row) -> Result<Json, StorageError> {
    let format = if row.id.starts_with("entity:") {
        2
    } else {
        1
    };
    if row.fields.len() != 2 || row.fields.get("format") != Some(&Value::Integer(format)) {
        return Err(invalid());
    }
    let Some(Value::String(data)) = row.fields.get("data") else {
        return Err(invalid());
    };
    let json = Json::parse(data).map_err(|_| invalid())?;
    crate::storage::count_work!(packets_parsed, 1);
    if json.canonical().map_err(|_| invalid())? != *data {
        return Err(invalid());
    }
    Ok(json)
}

/// Validates one packet's exact payload shape and identity. The owner validates
/// record/header relationships through `validate_bounded`; this helper does not
/// establish a complete ledger or validate application-schema column names.
pub(crate) fn validate_packet(
    row: &Row,
    entity: Option<&EntityLedger>,
) -> Result<(), StorageError> {
    let json = decode_packet(row)?;
    if row.id == "state" {
        decode_header(&json)?;
    } else if let Some(key) = row.id.strip_prefix("command:") {
        if decode_record(&json)?.key != key {
            return Err(invalid());
        }
    } else if row.id == "entity:state" {
        header_from_json(&json)?;
    } else if let Some(key) = row.id.strip_prefix("entity:record:") {
        let state = entity.ok_or_else(invalid)?;
        let (descriptor, _) = decode_extension(&json)?;
        if key.is_empty()
            || descriptor
                .as_ref()
                .is_some_and(|d| !state.config.tables.contains(&d.table))
        {
            return Err(invalid());
        }
    } else if let Some(ordinal) = row.id.strip_prefix("entity:metadata:") {
        let entity = entity.ok_or_else(invalid)?;
        let f = fields(&json, &["table", "record_id", "metadata"])?;
        validate_metadata_suffix(
            entity.ledger_format,
            ordinal,
            text(&f["table"])?,
            text(&f["record_id"])?,
        )?;
        validate_metadata_entry(
            text(&f["table"])?,
            text(&f["record_id"])?,
            &metadata_from_json(&f["metadata"])?,
        )?;
    } else {
        return Err(invalid());
    }
    Ok(())
}

fn decode_extension(
    value: &Json,
) -> Result<(Option<EntityDescriptor>, Vec<MetadataChange>), StorageError> {
    let f = fields(value, &["entity", "metadata_undo"])?;
    let descriptor = if f["entity"] == Json::Null {
        None
    } else {
        Some(descriptor_from_json(&f["entity"])?)
    };
    let undo = array(&f["metadata_undo"])?
        .iter()
        .map(metadata_change_from_json)
        .collect::<Result<Vec<_>, _>>()?;
    if descriptor.is_none() && undo.is_empty() {
        return Err(invalid());
    }
    Ok((descriptor, undo))
}

pub(crate) fn sync_header_row(header: &SyncHeader) -> Result<Row, StorageError> {
    packet("state".into(), encode_header(header))
}

pub(crate) fn entity_header_row(entity: &EntityLedger) -> Result<Row, StorageError> {
    if !matches!(entity.ledger_format, 2 | 3) {
        return Err(invalid());
    }
    entity_packet("entity:state".into(), header_json(entity))
}

pub(crate) fn record_rows(record: &SyncRecord) -> Result<Vec<Row>, StorageError> {
    let mut rows = vec![packet(
        format!("command:{}", record.key),
        encode_record(record),
    )?];
    if record.entity.is_some() || !record.metadata_undo.is_empty() {
        rows.push(entity_packet(
            format!("entity:record:{}", record.key),
            encode_extension(record),
        )?);
    }
    Ok(rows)
}

/// Format3 identity uses canonical lowercase hex over a fixed u32 byte-length prefix.
pub(crate) fn identity_metadata_id(table: &str, id: &str) -> Result<String, StorageError> {
    let combined = table.len().checked_add(id.len()).ok_or_else(invalid)?;
    if combined > super::json::MAX_BYTES {
        return Err(invalid());
    }
    let length = u32::try_from(table.len()).map_err(|_| invalid())?;
    let bytes = combined.checked_add(4).ok_or_else(invalid)?;
    let capacity = bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add("entity:metadata:".len()))
        .ok_or_else(invalid)?;
    let mut encoded = String::with_capacity(capacity);
    encoded.push_str("entity:metadata:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in length
        .to_be_bytes()
        .into_iter()
        .chain(table.bytes())
        .chain(id.bytes())
    {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(encoded)
}

fn validate_metadata_suffix(
    format: u8,
    suffix: &str,
    table: &str,
    id: &str,
) -> Result<Option<u64>, StorageError> {
    if format == 2 {
        if suffix.len() != 16 || !suffix.bytes().all(lower_hex) {
            return Err(invalid());
        }
        return u64::from_str_radix(suffix, 16)
            .map(Some)
            .map_err(|_| invalid());
    }
    let bound = super::json::MAX_BYTES
        .checked_add(4)
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(invalid)?;
    // Check raw size and canonical alphabet before allocating decoded bytes.
    if format != 3
        || suffix.len() < 8
        || suffix.len() > bound
        || !suffix.len().is_multiple_of(2)
        || !suffix.bytes().all(lower_hex)
    {
        return Err(invalid());
    }
    let nibble = |value: u8| {
        if value <= b'9' {
            value - b'0'
        } else {
            value - b'a' + 10
        }
    };
    let length = u32::from_str_radix(&suffix[..8], 16).map_err(|_| invalid())?;
    let table_length = usize::try_from(length).map_err(|_| invalid())?;
    let identity_length = suffix.len() / 2 - 4;
    if table_length > identity_length
        || table_length != table.len()
        || identity_length - table_length != id.len()
        || id.is_empty()
    {
        return Err(invalid());
    }
    crate::model::check_name(table).map_err(|_| invalid())?;
    let decoded = suffix.as_bytes()[8..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]));
    // Equality to the payload's UTF-8 strings validates both decoded components
    // without allocating a buffer. Length checks preserve their exact boundary.
    if !decoded.eq(table.bytes().chain(id.bytes())) {
        return Err(invalid());
    }
    // Fixed-width length plus lowercase hex has exactly one encoding for these bytes.
    Ok(None)
}

pub(crate) fn identity_metadata_row(
    table: &str,
    id: &str,
    metadata: &RowMetadata,
) -> Result<Row, StorageError> {
    entity_packet(
        identity_metadata_id(table, id)?,
        obj([
            ("table", string(table)),
            ("record_id", string(id)),
            ("metadata", metadata_json(metadata)),
        ]),
    )
}

pub(crate) fn metadata_row(
    ordinal: u64,
    table: &str,
    id: &str,
    metadata: &RowMetadata,
) -> Result<Row, StorageError> {
    entity_packet(
        format!("entity:metadata:{ordinal:016x}"),
        obj([
            ("table", string(table)),
            ("record_id", string(id)),
            ("metadata", metadata_json(metadata)),
        ]),
    )
}

fn packet(id: String, data: Json) -> Result<Row, StorageError> {
    let data = data.canonical().map_err(|_| invalid())?;
    crate::storage::count_work!(packets_encoded, 1);
    Ok(Row {
        id,
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(1)),
            ("data".into(), Value::String(data)),
        ]),
    })
}

fn fields<'a>(value: &'a Json, names: &[&str]) -> Result<&'a BTreeMap<String, Json>, StorageError> {
    let Json::Object(fields) = value else {
        return Err(invalid());
    };
    if fields.len() != names.len() || names.iter().any(|name| !fields.contains_key(*name)) {
        return Err(invalid());
    }
    Ok(fields)
}

fn field<'a>(fields: &'a BTreeMap<String, Json>, name: &str) -> Result<&'a Json, StorageError> {
    fields.get(name).ok_or_else(invalid)
}

fn text(value: &Json) -> Result<&str, StorageError> {
    if let Json::String(value) = value {
        Ok(value)
    } else {
        Err(invalid())
    }
}

fn num(value: &Json) -> Result<i64, StorageError> {
    if let Json::Integer(value) = value {
        Ok(*value)
    } else {
        Err(invalid())
    }
}

fn boolean(value: &Json) -> Result<bool, StorageError> {
    if let Json::Bool(value) = value {
        Ok(*value)
    } else {
        Err(invalid())
    }
}

fn array(value: &Json) -> Result<&[Json], StorageError> {
    if let Json::Array(value) = value {
        Ok(value)
    } else {
        Err(invalid())
    }
}

fn uint(value: &Json) -> Result<usize, StorageError> {
    usize::try_from(num(value)?).map_err(|_| invalid())
}

fn u32num(value: &Json) -> Result<u32, StorageError> {
    u32::try_from(num(value)?).map_err(|_| invalid())
}

fn string_map(value: &Json) -> Result<BTreeMap<String, String>, StorageError> {
    let Json::Object(values) = value else {
        return Err(invalid());
    };
    values
        .iter()
        .map(|(k, v)| Ok((k.clone(), text(v)?.into())))
        .collect()
}

fn map_json(values: &BTreeMap<String, String>) -> Json {
    Json::Object(values.iter().map(|(k, v)| (k.clone(), string(v))).collect())
}

fn encode_header(h: &SyncHeader) -> Json {
    obj([
        ("namespace", string(&h.namespace)),
        ("sequence", integer(h.sequence)),
        ("last_ms", integer(h.last_ms)),
        (
            "options",
            obj([
                ("capacity", integer(h.options.capacity as i64)),
                ("history", integer(h.options.history as i64)),
                ("initial_delay_ms", integer(h.options.initial_delay_ms)),
                ("multiplier", integer(i64::from(h.options.multiplier))),
                ("max_delay_ms", integer(h.options.max_delay_ms)),
            ]),
        ),
    ])
}

pub(crate) fn decode_header(value: &Json) -> Result<SyncHeader, StorageError> {
    let f = fields(value, &["namespace", "sequence", "last_ms", "options"])?;
    let o = fields(
        field(f, "options")?,
        &[
            "capacity",
            "history",
            "initial_delay_ms",
            "multiplier",
            "max_delay_ms",
        ],
    )?;
    Ok(SyncHeader {
        namespace: text(field(f, "namespace")?)?.into(),
        sequence: num(field(f, "sequence")?)?,
        last_ms: num(field(f, "last_ms")?)?,
        options: SyncOptions {
            capacity: uint(field(o, "capacity")?)?,
            history: uint(field(o, "history")?)?,
            initial_delay_ms: num(field(o, "initial_delay_ms")?)?,
            multiplier: u32num(field(o, "multiplier")?)?,
            max_delay_ms: num(field(o, "max_delay_ms")?)?,
        },
    })
}

fn state_name(state: &SyncState) -> &'static str {
    match state {
        SyncState::Discarded => "discarded",
        SyncState::Queued => "queued",
        SyncState::Sent => "sent",
        SyncState::Acknowledged => "acknowledged",
        SyncState::Applied => "applied",
        SyncState::Corrected => "corrected",
        SyncState::Rejected => "rejected",
        SyncState::Cancelled => "cancelled",
    }
}

fn decode_state(value: &Json) -> Result<SyncState, StorageError> {
    Ok(match text(value)? {
        "discarded" => SyncState::Discarded,
        "queued" => SyncState::Queued,
        "sent" => SyncState::Sent,
        "acknowledged" => SyncState::Acknowledged,
        "applied" => SyncState::Applied,
        "corrected" => SyncState::Corrected,
        "rejected" => SyncState::Rejected,
        "cancelled" => SyncState::Cancelled,
        _ => return Err(invalid()),
    })
}

fn encode_outcome(value: &RemoteOutcome) -> Json {
    match value {
        RemoteOutcome::Applied(result) => {
            obj([("state", string("applied")), ("result", result.clone())])
        }
        RemoteOutcome::Corrected(result) => {
            obj([("state", string("corrected")), ("result", result.clone())])
        }
        RemoteOutcome::Rejected(reason) => {
            obj([("state", string("rejected")), ("reason", string(reason))])
        }
    }
}

fn decode_outcome(value: &Json) -> Result<RemoteOutcome, StorageError> {
    let Json::Object(f) = value else {
        return Err(invalid());
    };
    match text(field(f, "state")?)? {
        "applied" => {
            fields(value, &["state", "result"])?;
            Ok(RemoteOutcome::Applied(field(f, "result")?.clone()))
        }
        "corrected" => {
            fields(value, &["state", "result"])?;
            Ok(RemoteOutcome::Corrected(field(f, "result")?.clone()))
        }
        "rejected" => {
            fields(value, &["state", "reason"])?;
            Ok(RemoteOutcome::Rejected(text(field(f, "reason")?)?.into()))
        }
        _ => Err(invalid()),
    }
}

fn encode_value(value: &Value) -> Json {
    match value {
        Value::Null => obj([("kind", string("null"))]),
        Value::String(value) => obj([("kind", string("string")), ("value", string(value))]),
        Value::Integer(value) => obj([("kind", string("integer")), ("value", integer(*value))]),
        Value::Float(value) => obj([("kind", string("float")), ("value", Json::Float(*value))]),
        Value::Boolean(value) => obj([("kind", string("boolean")), ("value", Json::Bool(*value))]),
        Value::Timestamp(value) => obj([("kind", string("timestamp")), ("value", integer(*value))]),
        Value::Reference(value) => obj([("kind", string("reference")), ("value", string(value))]),
    }
}

fn decode_value(value: &Json) -> Result<Value, StorageError> {
    let Json::Object(f) = value else {
        return Err(invalid());
    };
    let tag = text(field(f, "kind")?)?;
    if tag == "null" {
        fields(value, &["kind"])?;
        return Ok(Value::Null);
    }
    fields(value, &["kind", "value"])?;
    let value = field(f, "value")?;
    Ok(match tag {
        "string" => Value::String(text(value)?.into()),
        "integer" => Value::Integer(num(value)?),
        "float" => {
            let Json::Float(value) = value else {
                return Err(invalid());
            };
            Value::Float(*value)
        }
        "boolean" => Value::Boolean(boolean(value)?),
        "timestamp" => Value::Timestamp(num(value)?),
        "reference" => Value::Reference(text(value)?.into()),
        _ => return Err(invalid()),
    })
}

fn encode_row(row: &Row) -> Json {
    obj([
        ("id", string(&row.id)),
        (
            "fields",
            Json::Object(
                row.fields
                    .iter()
                    .map(|(k, v)| (k.clone(), encode_value(v)))
                    .collect(),
            ),
        ),
    ])
}

fn decode_row(value: &Json) -> Result<Row, StorageError> {
    let f = fields(value, &["id", "fields"])?;
    let Json::Object(values) = field(f, "fields")? else {
        return Err(invalid());
    };
    Ok(Row {
        id: text(field(f, "id")?)?.into(),
        fields: values
            .iter()
            .map(|(k, v)| Ok((k.clone(), decode_value(v)?)))
            .collect::<Result<_, StorageError>>()?,
    })
}

fn encode_change(value: &CommittedChange) -> Json {
    obj([
        ("table", string(&value.table)),
        ("id", string(&value.id)),
        (
            "before",
            value.before.as_ref().map(encode_row).unwrap_or(Json::Null),
        ),
        (
            "after",
            value.after.as_ref().map(encode_row).unwrap_or(Json::Null),
        ),
    ])
}

fn decode_optional_row(value: &Json) -> Result<Option<Row>, StorageError> {
    if value == &Json::Null {
        Ok(None)
    } else {
        decode_row(value).map(Some)
    }
}

fn decode_change(value: &Json) -> Result<CommittedChange, StorageError> {
    let f = fields(value, &["table", "id", "before", "after"])?;
    Ok(CommittedChange {
        table: text(field(f, "table")?)?.into(),
        id: text(field(f, "id")?)?.into(),
        before: decode_optional_row(field(f, "before")?)?,
        after: decode_optional_row(field(f, "after")?)?,
    })
}

fn encode_record(r: &SyncRecord) -> Json {
    obj([
        ("key", string(&r.key)),
        ("command", string(&r.command)),
        ("args", r.args.clone()),
        ("context", r.context.clone()),
        ("schema_version", integer(i64::from(r.schema_version))),
        ("order", integer(r.order)),
        (
            "reference_paths",
            Json::Array(
                r.reference_paths
                    .iter()
                    .map(|path| {
                        Json::Array(
                            path.iter()
                                .map(|part| match part {
                                    PathPart::Key(key) => string(key),
                                    PathPart::Index(index) => integer(*index as i64),
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "dependencies",
            Json::Array(r.dependencies.iter().map(|v| string(v)).collect()),
        ),
        ("created", map_json(&r.created)),
        ("mappings", map_json(&r.mappings)),
        (
            "undo",
            Json::Array(r.undo.iter().map(encode_change).collect()),
        ),
        ("state", string(state_name(&r.state))),
        (
            "frozen",
            r.frozen.as_deref().map(string).unwrap_or(Json::Null),
        ),
        ("attempts", integer(i64::from(r.attempts))),
        ("due_ms", integer(r.due_ms)),
        (
            "outcome",
            r.outcome.as_ref().map(encode_outcome).unwrap_or(Json::Null),
        ),
        ("rolled_back", Json::Bool(r.rolled_back)),
        ("reconciled", Json::Bool(r.reconciled)),
    ])
}

fn decode_record(value: &Json) -> Result<SyncRecord, StorageError> {
    let f = fields(
        value,
        &[
            "key",
            "command",
            "args",
            "context",
            "schema_version",
            "order",
            "reference_paths",
            "dependencies",
            "created",
            "mappings",
            "undo",
            "state",
            "frozen",
            "attempts",
            "due_ms",
            "outcome",
            "rolled_back",
            "reconciled",
        ],
    )?;
    let reference_paths = array(field(f, "reference_paths")?)?
        .iter()
        .map(|path| {
            array(path)?
                .iter()
                .map(|part| match part {
                    Json::String(key) => Ok(PathPart::Key(key.clone())),
                    Json::Integer(_) => Ok(PathPart::Index(uint(part)?)),
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, StorageError>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dependencies = array(field(f, "dependencies")?)?
        .iter()
        .map(|v| text(v).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    if dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid());
    }
    Ok(SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: text(field(f, "key")?)?.into(),
        command: text(field(f, "command")?)?.into(),
        args: field(f, "args")?.clone(),
        context: field(f, "context")?.clone(),
        schema_version: u32num(field(f, "schema_version")?)?,
        order: num(field(f, "order")?)?,
        reference_paths,
        dependencies: dependencies.into_iter().collect(),
        created: string_map(field(f, "created")?)?,
        mappings: string_map(field(f, "mappings")?)?,
        undo: array(field(f, "undo")?)?
            .iter()
            .map(decode_change)
            .collect::<Result<_, _>>()?,
        state: decode_state(field(f, "state")?)?,
        frozen: match field(f, "frozen")? {
            Json::Null => None,
            value => Some(text(value)?.into()),
        },
        attempts: u32num(field(f, "attempts")?)?,
        due_ms: num(field(f, "due_ms")?)?,
        outcome: match field(f, "outcome")? {
            Json::Null => None,
            value => Some(decode_outcome(value)?),
        },
        rolled_back: boolean(field(f, "rolled_back")?)?,
        reconciled: boolean(field(f, "reconciled")?)?,
    })
}

pub fn path_value<'a>(mut value: &'a Json, path: &[PathPart]) -> Option<&'a Json> {
    for part in path {
        value = match (part, value) {
            (PathPart::Key(key), Json::Object(values)) => values.get(key)?,
            (PathPart::Index(index), Json::Array(values)) => values.get(*index)?,
            _ => return None,
        };
    }
    Some(value)
}

fn lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn valid_date(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 24
        || [4, 7, 10, 13, 16, 19, 23]
            .into_iter()
            .zip(*b"--T::.Z")
            .any(|(i, c)| b[i] != c)
        || b.iter()
            .enumerate()
            .any(|(i, c)| ![4, 7, 10, 13, 16, 19, 23].contains(&i) && !c.is_ascii_digit())
    {
        return false;
    }
    let number = |a, b| value[a..b].parse::<u32>().unwrap_or(u32::MAX);
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    (1970..=9999).contains(&year)
        && day >= 1
        && day <= days
        && number(11, 13) < 24
        && number(14, 16) < 60
        && number(17, 19) < 60
}

fn validate_record(r: &SyncRecord, h: &SyncHeader) -> Result<(), StorageError> {
    let segments = r.key.split('-').collect::<Vec<_>>();
    if segments.len() != 3
        || segments[0].len() != 16
        || segments[1] != h.namespace
        || segments[2].len() != 16
        || !segments[0].bytes().all(lower_hex)
        || !segments[2].bytes().all(lower_hex)
        || i64::from_str_radix(segments[0], 16).map_or(true, |time| time > h.last_ms)
        || i64::from_str_radix(segments[2], 16).ok() != Some(r.order)
        || r.order < 1
        || r.order >= h.sequence
        || r.schema_version == 0
        || r.due_ms < 0
        || !matches!(r.args, Json::Object(_))
    {
        return Err(invalid());
    }
    if let Some(entity) = &r.entity {
        validate_descriptor(entity)?;
        if !r.command.is_empty() {
            return Err(invalid());
        }
        let args = fields(&r.args, &["recordId", "changes"])?;
        if text(&args["recordId"])?.is_empty() {
            return Err(invalid());
        }
        let Json::Object(changes) = &args["changes"] else {
            return Err(invalid());
        };
        for name in changes.keys() {
            crate::model::check_name(name).map_err(|_| invalid())?;
        }
        if entity.op == EntityOperation::Delete && !changes.is_empty() {
            return Err(invalid());
        }
        for name in &entity.removed_fields {
            if changes.get(name) != Some(&Json::Null) {
                return Err(invalid());
            }
        }
        let retired_unsent = r.state == SyncState::Cancelled
            && r.frozen.is_none()
            && r.rolled_back
            && r.reconciled
            && r.created.is_empty()
            && r.undo.is_empty()
            && r.metadata_undo.is_empty();
        if entity.op == EntityOperation::Create
            && (!valid_entity_id(text(&args["recordId"])?, &h.namespace)
                || (!retired_unsent
                    && (r.created.len() != 1
                        || r.created.get(text(&args["recordId"])?) != Some(&entity.table))))
        {
            return Err(invalid());
        }
        if entity.op != EntityOperation::Create && !r.created.is_empty() {
            return Err(invalid());
        }
        for path in &r.reference_paths {
            match path.as_slice() {
                [PathPart::Key(key)]
                    if key == "recordId" && entity.op != EntityOperation::Create => {}
                [PathPart::Key(key), PathPart::Key(name)]
                    if key == "changes" && changes.contains_key(name) => {}
                _ => return Err(invalid()),
            }
        }
    } else {
        let Some((prefix, version)) = r.command.rsplit_once("-v") else {
            return Err(invalid());
        };
        if prefix.is_empty()
            || r.command.len() > 128
            || version.is_empty()
            || !version.bytes().all(|b| b.is_ascii_digit())
            || version.bytes().all(|b| b == b'0')
        {
            return Err(invalid());
        }
    }
    let context = fields(&r.context, &["identity", "capturedAt"])?;
    if r.entity.as_ref().is_some_and(|entity| {
        field(context, "capturedAt").ok()
            != Some(&string(&super::owner::utc_time(entity.timestamp_ms)))
    }) {
        return Err(invalid());
    }
    if !valid_date(text(field(context, "capturedAt")?)?) {
        return Err(invalid());
    }
    if let Some(frozen) = &r.frozen {
        let envelope = Json::parse(frozen).map_err(|_| invalid())?;
        if envelope != r.envelope()
            || envelope.canonical().map_err(|_| invalid())? != *frozen
            || !r.dependencies.is_empty()
        {
            return Err(invalid());
        }
    }
    let sent = r.frozen.is_some() && r.attempts > 0;
    if !r.mappings.is_empty()
        && (!r.reconciled || !matches!(r.state, SyncState::Applied | SyncState::Corrected))
    {
        return Err(invalid());
    }
    let legal = match r.state {
        SyncState::Discarded => {
            r.reconciled
                && !r.rolled_back
                && r.entity.is_none()
                && r.metadata_undo.is_empty()
                && r.reference_paths.is_empty()
                && r.dependencies.is_empty()
                && ((sent && matches!(r.outcome, Some(RemoteOutcome::Rejected(_))))
                    || (r.frozen.is_none() && r.attempts == 0 && r.outcome.is_none()))
        }
        SyncState::Queued => {
            r.frozen.is_none()
                && r.attempts == 0
                && r.outcome.is_none()
                && !r.rolled_back
                && !r.reconciled
        }
        SyncState::Sent | SyncState::Acknowledged => {
            sent && r.outcome.is_none() && !r.rolled_back && !r.reconciled
        }
        SyncState::Applied => {
            sent && matches!(r.outcome, Some(RemoteOutcome::Applied(_))) && !r.rolled_back
        }
        SyncState::Corrected => {
            sent && matches!(r.outcome, Some(RemoteOutcome::Corrected(_))) && !r.rolled_back
        }
        SyncState::Rejected => {
            sent && matches!(r.outcome, Some(RemoteOutcome::Rejected(_)))
                && (!r.rolled_back || r.reconciled)
        }
        SyncState::Cancelled => {
            (sent || (r.frozen.is_none() && r.attempts == 0))
                && matches!(r.outcome, None | Some(RemoteOutcome::Rejected(_)))
                && (r.outcome.is_none() || sent)
                && (!r.rolled_back || r.reconciled)
        }
    };
    if !legal {
        return Err(invalid());
    }
    for change in &r.undo {
        if change.id.is_empty()
            || crate::model::check_name(&change.table).is_err()
            || change.before.iter().chain(change.after.iter()).any(|row| {
                row.id != change.id
                    || row
                        .fields
                        .keys()
                        .any(|name| name.starts_with(crate::model::RESERVED_PREFIX))
            })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::FiniteFloat;

    fn ledger() -> SyncLedger {
        SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "0".repeat(32),
                sequence: 20,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        }
    }

    fn record(order: i64) -> SyncRecord {
        SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: format!("{:016x}-{}-{order:016x}", 100, "0".repeat(32)),
            command: "test-v1".into(),
            args: Json::Object(BTreeMap::new()),
            context: Json::Object(BTreeMap::from([
                ("identity".into(), Json::Null),
                (
                    "capturedAt".into(),
                    Json::String("2026-10-23T12:00:00.000Z".into()),
                ),
            ])),
            schema_version: 1,
            order,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: BTreeMap::new(),
            mappings: BTreeMap::new(),
            undo: vec![],
            state: SyncState::Queued,
            frozen: None,
            attempts: 0,
            due_ms: 0,
            outcome: None,
            rolled_back: false,
            reconciled: false,
        }
    }

    fn add(ledger: &mut SyncLedger, record: SyncRecord) {
        ledger.records.insert(record.key.clone(), record);
    }

    #[test]
    fn c17_incremental_live_closure_matches_full_recomputation() {
        let reference = |id: &str| CommittedChange {
            table: "items".into(),
            id: "row".into(),
            before: None,
            after: Some(Row {
                id: "row".into(),
                fields: BTreeMap::from([("parent".into(), Value::Reference(id.into()))]),
            }),
        };
        let mut ledger = ledger();
        // A non-live discarded producer whose creation is still referenced.
        let mut discarded = record(1);
        discarded.state = SyncState::Discarded;
        discarded.reconciled = true;
        discarded.created = BTreeMap::from([("local:kept".into(), "items".into())]);
        // A second non-live producer reached only through the first one's undo image.
        let mut inner = record(2);
        inner.state = SyncState::Discarded;
        inner.reconciled = true;
        inner.created = BTreeMap::from([("local:inner".into(), "items".into())]);
        discarded.undo = vec![reference("local:inner")];
        let unrelated = {
            let mut value = record(3);
            value.state = SyncState::Discarded;
            value.reconciled = true;
            value.created = BTreeMap::from([("local:unrelated".into(), "items".into())]);
            value
        };
        let (kept, nested, idle) = (
            discarded.key.clone(),
            inner.key.clone(),
            unrelated.key.clone(),
        );
        add(&mut ledger, discarded);
        add(&mut ledger, inner);
        add(&mut ledger, unrelated);
        add(&mut ledger, record(4));
        let mut live = ledger.live_keys();
        assert_eq!(live.len(), 1);
        let mut plain = record(5);
        plain.undo = vec![reference("canonical")];
        let mut consumer = record(6);
        consumer.undo = vec![reference("local:kept")];
        for next in [plain, consumer, record(7)] {
            let key = next.key.clone();
            add(&mut ledger, next);
            ledger.extend_live(&mut live, &key);
            assert_eq!(live, ledger.live_keys());
        }
        assert!(live.contains(&kept) && live.contains(&nested) && !live.contains(&idle));
        assert_eq!(live.len(), 6);
        // Re-adding a known live key is a no-op.
        ledger.extend_live(&mut live, &kept);
        assert_eq!(live, ledger.live_keys());
    }

    fn roundtrip(ledger: &SyncLedger) {
        assert_eq!(
            SyncLedger::from_rows(&ledger.to_rows().unwrap()).unwrap(),
            *ledger
        );
    }

    fn entity_ledger() -> SyncLedger {
        let mut ledger = ledger();
        ledger.entity = Some(header_from_json(&Json::parse(r#"{"config":{"scope":"tenant","tables":["items"]},"generation":1,"revision":0,"checkpoint":null,"ready":false,"pages":0,"changes":0}"#).unwrap()).unwrap());
        let mut record = record(1);
        record.command.clear();
        record.context = obj([
            ("identity", Json::Null),
            ("capturedAt", string(&super::super::owner::utc_time(100))),
        ]);
        record.entity = Some(EntityDescriptor {
            table: "items".into(),
            op: EntityOperation::Create,
            timestamp_ms: 100,
            removed_fields: BTreeSet::new(),
        });
        let id = format!("entity:{}:{:016x}", ledger.header.namespace, 1);
        record.args = obj([
            ("recordId", string(&id)),
            ("changes", obj([("name", string("value"))])),
        ]);
        record.created.insert(id.clone(), "items".into());
        let metadata = RowMetadata {
            fields: BTreeMap::from([(
                "name".into(),
                FieldStamp {
                    timestamp_ms: 100,
                    authoritative: false,
                },
            )]),
            tombstone: None,
        };
        record.metadata_undo.push(MetadataChange {
            table: "items".into(),
            record_id: id.clone(),
            before: None,
            after: Some(metadata.clone()),
        });
        ledger
            .entity
            .as_mut()
            .unwrap()
            .metadata
            .insert(("items".into(), id), metadata);
        add(&mut ledger, record);
        ledger
    }

    #[test]
    fn si_f03_header_helper_refuses_invalid_internal_format() {
        for format in [0, 1, 4, u8::MAX] {
            let mut ledger = entity_ledger();
            ledger.entity.as_mut().unwrap().ledger_format = format;
            assert!(
                entity_header_row(ledger.entity.as_ref().unwrap()).is_err(),
                "header helper encoded invalid format {format}"
            );
        }
    }

    #[test]
    fn si_f03_invalid_internal_entity_formats_cannot_serialize() {
        for format in [0, 1, 4, u8::MAX] {
            let mut ledger = entity_ledger();
            ledger.entity.as_mut().unwrap().ledger_format = format;
            assert!(
                ledger.to_rows().is_err(),
                "invalid internal format {format} serialized"
            );
        }
    }

    #[test]
    fn si_y_bounded_validation_skips_only_untouched_metadata() {
        let mut value = entity_ledger();
        value.validate().unwrap();
        value.entity.as_mut().unwrap().metadata.insert(
            ("items".into(), "bad".into()),
            RowMetadata {
                fields: BTreeMap::from([(
                    "name".into(),
                    FieldStamp {
                        timestamp_ms: -1,
                        authoritative: true,
                    },
                )]),
                tombstone: None,
            },
        );
        assert!(
            value.validate().is_err(),
            "full validation must remain strict"
        );
        value
            .validate_bounded()
            .expect("bounded validation must skip unrelated metadata");
        value.records.values_mut().next().unwrap().schema_version = 0;
        assert!(
            value.validate_bounded().is_err(),
            "record checks must remain active"
        );
    }

    #[test]
    fn si_y_bounded_validation_visits_zero_metadata_at_scale() {
        for count in [10, 1000] {
            let mut value = entity_ledger();
            value.entity.as_mut().unwrap().metadata = (0..count)
                .map(|i| (("items".into(), format!("row-{i}")), RowMetadata::default()))
                .collect();
            value.validate().unwrap();
            crate::storage::counters::reset();
            value.validate_bounded().unwrap();
            assert_eq!(
                crate::storage::counters::snapshot().metadata_entries_visited,
                0,
                "bounded validation at M={count}"
            );
        }
    }

    #[test]
    fn si_y_bounded_validation_preserves_global_checks() {
        let mut value = entity_ledger();
        value.validate_bounded().unwrap();
        let mut duplicate = record(1);
        duplicate.key.push('x');
        value.records.insert(duplicate.key.clone(), duplicate);
        assert!(
            value.validate_bounded().is_err(),
            "duplicate orders remain invalid"
        );
        let mut value = entity_ledger();
        value
            .records
            .values_mut()
            .next()
            .unwrap()
            .dependencies
            .insert("local:missing".into());
        assert!(
            value.validate_bounded().is_err(),
            "dependency mismatches remain invalid"
        );
        let mut value = ledger();
        value.header.options.capacity = 1;
        add(&mut value, record(1));
        add(&mut value, record(2));
        assert!(
            value.validate_bounded().is_err(),
            "capacity bounds remain active"
        );
    }

    #[test]
    fn si_y_packet_helpers_preserve_full_encoding_order_and_counters() {
        let mut value = entity_ledger();
        add(&mut value, record(2));
        let mut expected = vec![packet("state".into(), encode_header(&value.header)).unwrap()];
        for record in value.records.values() {
            expected
                .push(packet(format!("command:{}", record.key), encode_record(record)).unwrap());
        }
        let entity = value.entity.as_ref().unwrap();
        expected.push(entity_packet("entity:state".into(), header_json(entity)).unwrap());
        for record in value.records.values() {
            if record.entity.is_some() || !record.metadata_undo.is_empty() {
                expected.push(
                    entity_packet(
                        format!("entity:record:{}", record.key),
                        encode_extension(record),
                    )
                    .unwrap(),
                );
            }
        }
        for (ordinal, ((table, id), metadata)) in entity.metadata.iter().enumerate() {
            expected.push(
                entity_packet(
                    format!("entity:metadata:{ordinal:016x}"),
                    obj([
                        ("table", string(table)),
                        ("record_id", string(id)),
                        ("metadata", metadata_json(metadata)),
                    ]),
                )
                .unwrap(),
            );
        }
        crate::storage::counters::reset();
        let actual = value.to_rows().unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            crate::storage::counters::snapshot().packets_encoded,
            expected.len() as u64
        );
        crate::storage::counters::reset();
        for row in &actual {
            validate_packet(row, value.entity.as_ref()).unwrap();
        }
        assert_eq!(
            crate::storage::counters::snapshot().packets_parsed,
            expected.len() as u64
        );
        assert_eq!(SyncLedger::from_rows(&actual).unwrap(), value);
    }

    #[test]
    fn si_y_packet_decoder_rejects_noncanonical_and_wrong_envelopes() {
        let original = sync_header_row(&ledger().header).unwrap();
        let json = decode_packet(&original).unwrap();
        assert!(matches!(json, Json::Object(_)));
        let mut row = original.clone();
        row.fields.insert("extra".into(), Value::Null);
        assert!(decode_packet(&row).is_err());
        let mut row = original.clone();
        row.fields.insert("format".into(), Value::Integer(2));
        assert!(decode_packet(&row).is_err());
        let mut row = original.clone();
        let Value::String(data) = row.fields.get_mut("data").unwrap() else {
            unreachable!()
        };
        data.push(' ');
        assert!(decode_packet(&row).is_err());
        let mut row = original.clone();
        row.fields.insert("data".into(), Value::String("{".into()));
        assert!(decode_packet(&row).is_err());
        let mut row = original;
        row.id = "unknown".into();
        assert!(validate_packet(&row, None).is_err());
    }

    #[test]
    fn si_y_touched_packets_reject_identity_payload_and_intrinsic_faults() {
        let value = entity_ledger();
        let rows = value.to_rows().unwrap();
        for original in &rows {
            validate_packet(original, value.entity.as_ref()).unwrap();
            let mut row = original.clone();
            let mut json = decode_packet(&row).unwrap();
            let Json::Object(fields) = &mut json else {
                unreachable!()
            };
            fields.insert("unexpected".into(), Json::Null);
            row.fields
                .insert("data".into(), Value::String(json.canonical().unwrap()));
            assert!(validate_packet(&row, value.entity.as_ref()).is_err());
        }
        let mut row = rows
            .iter()
            .find(|r| r.id.starts_with("command:"))
            .unwrap()
            .clone();
        row.id.push('x');
        assert!(validate_packet(&row, value.entity.as_ref()).is_err());
        let row = rows
            .iter()
            .find(|r| r.id.starts_with("entity:metadata:"))
            .unwrap();
        assert!(validate_packet(row, None).is_err());
        for id in ["entity:metadata:0", "entity:metadata:000000000000000A"] {
            let mut invalid = row.clone();
            invalid.id = id.into();
            assert!(validate_packet(&invalid, value.entity.as_ref()).is_err());
        }
        for (table, id, metadata) in [
            ("", "r", RowMetadata::default()),
            ("items", "", RowMetadata::default()),
            (
                "items",
                "r",
                RowMetadata {
                    fields: BTreeMap::from([(
                        "name".into(),
                        FieldStamp {
                            timestamp_ms: -1,
                            authoritative: false,
                        },
                    )]),
                    tombstone: None,
                },
            ),
        ] {
            let invalid = metadata_row(0, table, id, &metadata).unwrap();
            assert!(validate_packet(&invalid, value.entity.as_ref()).is_err());
        }
        let invalid = entity_packet(
            "entity:record:key".into(),
            obj([
                ("entity", Json::Null),
                ("metadata_undo", Json::Array(vec![])),
            ]),
        )
        .unwrap();
        assert!(validate_packet(&invalid, value.entity.as_ref()).is_err());
    }

    #[test]
    fn entity_codec_adds_exact_packets_and_rejects_orphans_gaps_and_wrong_formats() {
        let ledger = entity_ledger();
        let mut rows = ledger.to_rows().unwrap();
        assert_eq!(rows.len(), 5);
        assert!(
            rows.iter()
                .any(|r| r.id == "entity:metadata:0000000000000000")
        );
        for row in &rows {
            assert_eq!(
                row.fields["format"],
                Value::Integer(if row.id.starts_with("entity:") {
                    2
                } else {
                    1
                })
            );
        }
        rows.reverse();
        assert_eq!(SyncLedger::from_rows(&rows).unwrap(), ledger);
        let without_state = rows
            .iter()
            .filter(|r| r.id != "entity:state")
            .cloned()
            .collect::<Vec<_>>();
        assert!(SyncLedger::from_rows(&without_state).is_err());
        let mut invalid = rows.clone();
        invalid
            .iter_mut()
            .find(|r| r.id.starts_with("entity:metadata:"))
            .unwrap()
            .id = "entity:metadata:0000000000000001".into();
        assert!(SyncLedger::from_rows(&invalid).is_err());
        let mut invalid = rows.clone();
        invalid
            .iter_mut()
            .find(|r| r.id == "entity:state")
            .unwrap()
            .fields
            .insert("format".into(), Value::Integer(1));
        assert!(SyncLedger::from_rows(&invalid).is_err());
        let invalid = rows
            .iter()
            .filter(|r| !r.id.starts_with("command:"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(SyncLedger::from_rows(&invalid).is_err());
    }

    #[test]
    fn entity_envelope_has_no_command_key_and_stable_identity_survives_history_eviction() {
        let mut ledger = entity_ledger();
        let key = ledger.records.keys().next().unwrap().clone();
        let entry = &ledger.records[&key];
        let envelope = entry.envelope();
        let f = fields(
            &envelope,
            &[
                "table",
                "op",
                "recordId",
                "changes",
                "removedFields",
                "clientTimestamp",
                "schemaVersion",
                "context",
                "writeId",
            ],
        )
        .unwrap();
        assert_eq!(f["writeId"], string(&key));
        assert_eq!(f["op"], string("create"));
        let id = text(&f["recordId"]).unwrap().to_owned();
        let entry = ledger.records.get_mut(&key).unwrap();
        entry.frozen = Some(entry.envelope().canonical().unwrap());
        entry.attempts = 1;
        entry.state = SyncState::Applied;
        entry.outcome = Some(RemoteOutcome::Applied(Json::Null));
        entry.reconciled = true;
        entry.mappings.insert(id.clone(), id.clone());
        entry.metadata_undo.clear();
        roundtrip(&ledger);
        let mut consumer = record(2);
        consumer.args = obj([("ref", string(&id))]);
        consumer.reference_paths = vec![vec![PathPart::Key("ref".into())]];
        add(&mut ledger, consumer);
        ledger.validate().unwrap();
        ledger.records.remove(&key);
        roundtrip(&ledger);
    }

    #[test]
    fn entity_opt_in_preserves_old_base_packets_and_frozen_command_bytes() {
        let mut old = ledger();
        let mut command = record(1);
        command.frozen = Some(command.envelope().canonical().unwrap());
        command.attempts = 1;
        command.state = SyncState::Sent;
        add(&mut old, command);
        let original = old.to_rows().unwrap();
        let frozen = old.records.values().next().unwrap().frozen.clone();
        let mut upgraded = old.clone();
        upgraded.entity = Some(EntityLedger::new(EntityConfig {
            scope: "tenant".into(),
            tables: BTreeSet::from(["items".into()]),
        }));
        let next = upgraded.to_rows().unwrap();
        for row in &original {
            assert!(next.contains(row));
        }
        assert_eq!(
            SyncLedger::from_rows(&next)
                .unwrap()
                .records
                .values()
                .next()
                .unwrap()
                .frozen,
            frozen
        );
    }

    #[test]
    fn entity_packets_are_individually_bounded_and_metadata_undo_is_preflighted() {
        let mut ledger = ledger();
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "tenant".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        let metadata = RowMetadata {
            fields: BTreeMap::from([(
                "x".repeat(600_000),
                FieldStamp {
                    timestamp_ms: 1,
                    authoritative: true,
                },
            )]),
            tombstone: None,
        };
        entity
            .metadata
            .insert(("items".into(), "a".into()), metadata.clone());
        entity
            .metadata
            .insert(("items".into(), "b".into()), metadata);
        ledger.entity = Some(entity);
        let rows = ledger.to_rows().unwrap();
        let sizes = rows
            .iter()
            .map(|row| match &row.fields["data"] {
                Value::String(data) => data.len(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert!(sizes.iter().sum::<usize>() > super::super::json::MAX_BYTES);
        assert!(
            sizes
                .iter()
                .all(|size| *size <= super::super::json::MAX_BYTES)
        );
        roundtrip(&ledger);
        let mut entry = record(1);
        entry.metadata_undo = vec![
            MetadataChange {
                table: "items".into(),
                record_id: "x".repeat(600_000),
                before: None,
                after: None
            };
            2
        ];
        assert!(entry.check_send_size().is_err());
    }

    #[test]
    fn entity_codec_rejects_duplicate_metadata_identity_bad_extensions_and_unknown_keys() {
        let ledger = entity_ledger();
        let rows = ledger.to_rows().unwrap();
        let metadata = rows
            .iter()
            .find(|row| row.id.starts_with("entity:metadata:"))
            .unwrap();
        let mut duplicate = metadata.clone();
        duplicate.id = "entity:metadata:0000000000000001".into();
        let mut corrupt = rows.clone();
        corrupt.push(duplicate);
        assert!(SyncLedger::from_rows(&corrupt).is_err());
        let mut corrupt = rows.clone();
        let extension = corrupt
            .iter_mut()
            .find(|row| row.id.starts_with("entity:record:"))
            .unwrap();
        extension.fields.insert(
            "data".into(),
            Value::String(
                obj([
                    ("entity", Json::Null),
                    ("metadata_undo", Json::Array(vec![])),
                ])
                .canonical()
                .unwrap(),
            ),
        );
        assert!(SyncLedger::from_rows(&corrupt).is_err());
        let mut corrupt = rows.clone();
        let state = corrupt
            .iter_mut()
            .find(|row| row.id == "entity:state")
            .unwrap();
        let Value::String(data) = &state.fields["data"] else {
            unreachable!()
        };
        let Json::Object(mut decoded) = Json::parse(data).unwrap() else {
            unreachable!()
        };
        decoded.insert("future".into(), Json::Null);
        state.fields.insert(
            "data".into(),
            Value::String(Json::Object(decoded).canonical().unwrap()),
        );
        assert!(SyncLedger::from_rows(&corrupt).is_err());
    }

    #[test]
    fn c06_exact_record_states_and_typed_images_roundtrip() {
        let mut ledger = ledger();
        let mut record = record(1);
        record.undo.push(CommittedChange {
            table: "items".into(),
            id: "local:1".into(),
            before: None,
            after: Some(Row {
                id: "local:1".into(),
                fields: BTreeMap::from([
                    ("null".into(), Value::Null),
                    ("string".into(), Value::String("東京".into())),
                    ("integer".into(), Value::Integer(i64::MIN)),
                    (
                        "float".into(),
                        Value::Float(FiniteFloat::new(42.0).unwrap()),
                    ),
                    ("boolean".into(), Value::Boolean(true)),
                    ("timestamp".into(), Value::Timestamp(i64::MAX)),
                    ("reference".into(), Value::Reference("local:2".into())),
                ]),
            }),
        });
        add(&mut ledger, record.clone());
        roundtrip(&ledger);
        for (state, outcome) in [
            (SyncState::Sent, None),
            (SyncState::Acknowledged, None),
            (SyncState::Applied, Some(RemoteOutcome::Applied(Json::Null))),
            (
                SyncState::Corrected,
                Some(RemoteOutcome::Corrected(Json::Integer(i64::MAX))),
            ),
            (
                SyncState::Rejected,
                Some(RemoteOutcome::Rejected("no".into())),
            ),
            (SyncState::Cancelled, None),
        ] {
            let mut value = record.clone();
            value.state = state;
            value.outcome = outcome;
            value.attempts = 1;
            value.frozen = Some(value.envelope().canonical().unwrap());
            ledger.records.clear();
            add(&mut ledger, value);
            roundtrip(&ledger);
        }
        let mut value = record;
        value.state = SyncState::Cancelled;
        value.rolled_back = true;
        value.reconciled = true;
        ledger.records.clear();
        add(&mut ledger, value);
        roundtrip(&ledger);
        assert!(!ledger.has_live_recovery());
    }

    #[test]
    fn c11_exact_shapes_versions_envelopes_and_redacted_errors() {
        let mut ledger = ledger();
        add(&mut ledger, record(1));
        let rows = ledger.to_rows().unwrap();
        for bad in [vec![], vec![rows[0].clone(), rows[0].clone()]] {
            assert!(SyncLedger::from_rows(&bad).is_err());
        }
        for field in ["format", "data"] {
            let mut bad = rows.clone();
            bad[0].fields.remove(field);
            assert!(SyncLedger::from_rows(&bad).is_err());
        }
        let mut bad = rows.clone();
        bad[0].fields.insert("format".into(), Value::Integer(2));
        assert!(SyncLedger::from_rows(&bad).is_err());
        let mut bad = rows;
        bad[0]
            .fields
            .insert("secret".into(), Value::String("private-payload".into()));
        assert_eq!(
            bad[0].fields["secret"],
            Value::String("private-payload".into())
        );
        let error = SyncLedger::from_rows(&bad).unwrap_err();
        assert!(!format!("{error:?}").contains("private-payload"));
        let StorageError::BackendFailure {
            message,
        } = error
        else {
            panic!("invalid recovery must return the static backend failure");
        };
        assert_eq!(message, "invalid command recovery bookkeeping");
        assert!(
            !message.contains("private-payload"),
            "the underlying error must be safe independently of Debug"
        );
        let mut invalid = ledger.clone();
        let value = invalid.records.values_mut().next().unwrap();
        value.state = SyncState::Sent;
        value.attempts = 1;
        value.frozen = Some("{}".into());
        assert!(invalid.to_rows().is_err());
        let mut invalid = ledger;
        invalid.records.values_mut().next().unwrap().context = Json::Object(BTreeMap::from([
            ("identity".into(), Json::Null),
            (
                "capturedAt".into(),
                Json::String("2026-02-30T12:00:00.000Z".into()),
            ),
        ]));
        assert!(invalid.to_rows().is_err());
    }

    #[test]
    fn c11_dependencies_validate_ownership_paths_mappings_and_cycles() {
        let mut ledger = ledger();
        let mut a = record(1);
        a.created.insert("local:a".into(), "items".into());
        let mut b = record(2);
        b.created.insert("local:b".into(), "items".into());
        b.args = Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String("local:a".into()),
        )]));
        b.reference_paths = vec![vec![PathPart::Key("parent".into())]];
        b.dependencies.insert("local:a".into());
        add(&mut ledger, a.clone());
        add(&mut ledger, b.clone());
        roundtrip(&ledger);
        let mut bad = ledger.clone();
        bad.records.get_mut(&b.key).unwrap().dependencies.clear();
        assert!(bad.to_rows().is_err());
        let mut bad = ledger.clone();
        bad.records
            .get_mut(&a.key)
            .unwrap()
            .mappings
            .insert("local:a".into(), "server-a".into());
        assert!(bad.to_rows().is_err());
        let mut bad = ledger.clone();
        bad.records
            .get_mut(&b.key)
            .unwrap()
            .created
            .insert("local:a".into(), "items".into());
        assert!(bad.to_rows().is_err());
        let mut bad = ledger;
        let parent = bad.records.get_mut(&a.key).unwrap();
        parent.args = Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String("local:b".into()),
        )]));
        parent.reference_paths = vec![vec![PathPart::Key("parent".into())]];
        parent.dependencies.insert("local:b".into());
        assert!(bad.to_rows().is_err());
    }

    #[test]
    fn c17_live_recovery_capacity_history_and_counter_invariants() {
        let mut ledger = ledger();
        let mut value = record(1);
        value.state = SyncState::Rejected;
        value.outcome = Some(RemoteOutcome::Rejected("no".into()));
        value.attempts = 1;
        value.frozen = Some(value.envelope().canonical().unwrap());
        value.reconciled = true;
        add(&mut ledger, value);
        roundtrip(&ledger);
        assert!(ledger.has_live_recovery());
        ledger.records.values_mut().next().unwrap().rolled_back = true;
        roundtrip(&ledger);
        assert!(!ledger.has_live_recovery());
        ledger.header.options.history = 0;
        assert!(ledger.to_rows().is_err());
        ledger.header.options.history = 100;
        ledger.header.sequence = 1;
        assert!(ledger.to_rows().is_err());
    }

    #[test]
    fn c06_records_have_individual_not_aggregate_json_limits() {
        let mut ledger = ledger();
        for order in 1..=10 {
            let mut value = record(order);
            value.args = Json::Object(BTreeMap::from([(
                "payload".into(),
                Json::String("x".repeat(150_000)),
            )]));
            add(&mut ledger, value);
        }
        let rows = ledger.to_rows().unwrap();
        assert!(
            rows.iter()
                .map(|row| match &row.fields["data"] {
                    Value::String(data) => data.len(),
                    _ => 0,
                })
                .sum::<usize>()
                > 1_048_576
        );
        assert!(SyncLedger::from_rows(&rows).unwrap() == ledger);
    }

    #[test]
    fn c11_mapping_requires_reconciled_authoritative_success() {
        for state in [
            SyncState::Queued,
            SyncState::Sent,
            SyncState::Acknowledged,
            SyncState::Rejected,
        ] {
            let mut ledger = ledger();
            let mut value = record(1);
            value.created.insert("local:a".into(), "items".into());
            if state != SyncState::Queued {
                value.attempts = 1;
                value.frozen = Some(value.envelope().canonical().unwrap());
            }
            if state == SyncState::Rejected {
                value.outcome = Some(RemoteOutcome::Rejected("no".into()));
            }
            value.state = state;
            value.mappings.insert("local:a".into(), "server-a".into());
            add(&mut ledger, value);
            assert!(ledger.to_rows().is_err());
        }
    }

    #[test]
    fn c11_unsent_cancellation_cannot_claim_remote_rejection() {
        let mut ledger = ledger();
        let mut value = record(1);
        value.state = SyncState::Cancelled;
        value.outcome = Some(RemoteOutcome::Rejected("no".into()));
        add(&mut ledger, value);
        assert!(ledger.to_rows().is_err());
    }

    #[test]
    fn c17_authoritative_ids_do_not_retain_old_schema_mapping_history() {
        let mut ledger = ledger();
        let mut producer = record(1);
        producer.state = SyncState::Applied;
        producer.outcome = Some(RemoteOutcome::Applied(Json::Null));
        producer.reconciled = true;
        producer.attempts = 1;
        producer.frozen = Some(producer.envelope().canonical().unwrap());
        producer.created.insert("local:a".into(), "items".into());
        producer
            .mappings
            .insert("local:a".into(), "server-a".into());
        let mut consumer = record(2);
        consumer.schema_version = 2;
        consumer.args = Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String("server-a".into()),
        )]));
        consumer.reference_paths = vec![vec![PathPart::Key("parent".into())]];
        add(&mut ledger, producer);
        add(&mut ledger, consumer.clone());
        assert_eq!(ledger.live_keys(), BTreeSet::from([consumer.key]));
    }

    #[test]
    fn c06_send_size_is_preflighted_without_changing_blocked_records() {
        let mut value = record(1);
        value.args = Json::Object(BTreeMap::from([(
            "payload".into(),
            Json::String("x".repeat(540_000)),
        )]));
        let mut ledger = ledger();
        add(&mut ledger, value.clone());
        assert!(ledger.to_rows().is_ok());
        assert!(value.check_send_size().is_err());
        let mut blocked = record(2);
        blocked.dependencies.insert("local:parent".into());
        let original = blocked.clone();
        blocked.check_send_size().unwrap();
        assert_eq!(blocked, original);
    }

    #[test]
    fn c11_public_undo_rejects_every_reserved_field() {
        for field in ["__hoya_future", crate::model::ROW_VERSION] {
            let mut ledger = ledger();
            let mut value = record(1);
            value.undo.push(CommittedChange {
                table: "items".into(),
                id: "local:a".into(),
                before: None,
                after: Some(Row {
                    id: "local:a".into(),
                    fields: BTreeMap::from([(field.into(), Value::Integer(1))]),
                }),
            });
            add(&mut ledger, value);
            assert!(ledger.to_rows().is_err());
        }
    }
}

/// TC-0041 SI-W03: manual counted clones copy every field and count each
/// record and metadata entry, including nested ones.
#[cfg(test)]
mod counted_clone_tests {
    use super::*;
    use crate::storage::counters;

    fn stamp(timestamp_ms: i64) -> FieldStamp {
        FieldStamp {
            timestamp_ms,
            authoritative: true,
        }
    }

    fn metadata(timestamp_ms: i64) -> RowMetadata {
        RowMetadata {
            fields: BTreeMap::from([("body".into(), stamp(timestamp_ms))]),
            tombstone: Some(stamp(timestamp_ms + 1)),
        }
    }

    fn row(id: &str) -> Row {
        Row {
            id: id.into(),
            fields: BTreeMap::from([("body".into(), Value::String(id.into()))]),
        }
    }

    /// Every field differs from its default.
    fn populated() -> SyncRecord {
        SyncRecord {
            entity: Some(EntityDescriptor {
                table: "items".into(),
                op: EntityOperation::Update,
                timestamp_ms: 7,
                removed_fields: BTreeSet::from(["gone".into()]),
            }),
            metadata_undo: vec![MetadataChange {
                table: "items".into(),
                record_id: "remote".into(),
                before: Some(metadata(3)),
                after: Some(metadata(5)),
            }],
            key: "key".into(),
            command: "edit-v1".into(),
            args: Json::Object(BTreeMap::from([("a".into(), Json::Integer(1))])),
            context: Json::String("context".into()),
            schema_version: 3,
            order: 5,
            reference_paths: vec![vec![PathPart::Key("a".into()), PathPart::Index(1)]],
            dependencies: BTreeSet::from(["parent".into()]),
            created: BTreeMap::from([("local:x".into(), "items".into())]),
            mappings: BTreeMap::from([("local:x".into(), "remote".into())]),
            undo: vec![CommittedChange {
                table: "items".into(),
                id: "local:x".into(),
                before: Some(row("before")),
                after: Some(row("after")),
            }],
            state: SyncState::Corrected,
            frozen: Some("frozen".into()),
            attempts: 2,
            due_ms: 9,
            outcome: Some(RemoteOutcome::Corrected(Json::Bool(true))),
            rolled_back: true,
            reconciled: true,
        }
    }

    #[test]
    fn si_w03_row_metadata_clone_is_exact_and_counted() {
        let original = metadata(4);
        counters::reset();
        let copy = original.clone();
        assert_eq!(counters::snapshot().ledger_entries_cloned, 1);
        assert_eq!(copy, original);
        // Equality controls: each field is compared.
        let mut fields = original.clone();
        fields.fields.insert("other".into(), stamp(1));
        assert_ne!(fields, original);
        let mut tombstone = original.clone();
        tombstone.tombstone = None;
        assert_ne!(tombstone, original);
    }

    #[test]
    fn si_w03_sync_record_clone_is_exact_and_counts_nested_metadata() {
        let original = populated();
        counters::reset();
        let copy = original.clone();
        assert_eq!(
            counters::snapshot().ledger_entries_cloned,
            3,
            "one record plus its before and after metadata"
        );
        assert_eq!(
            counters::snapshot().rows_cloned,
            2,
            "both nested undo Row images"
        );
        assert!(copy == original, "a counted clone dropped a field");
    }

    #[test]
    fn si_w03_ledger_clone_counts_every_record_and_metadata_entry() {
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "s".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        for k in 0..4 {
            entity
                .metadata
                .insert(("items".into(), format!("r{k}")), metadata(k));
        }
        let mut plain = populated();
        plain.key = "plain".into();
        plain.metadata_undo.clear();
        let ledger = SyncLedger {
            entity: Some(entity),
            header: SyncHeader {
                namespace: "0".repeat(32),
                sequence: 2,
                last_ms: 1,
                options: SyncOptions::default(),
            },
            records: BTreeMap::from([("key".into(), populated()), ("plain".into(), plain)]),
        };
        counters::reset();
        let copy = ledger.clone();
        assert_eq!(counters::snapshot().ledger_entries_cloned, 2 + 2 + 4);
        assert_eq!(
            counters::snapshot().rows_cloned,
            4,
            "two undo images per copied record"
        );
        assert!(copy == ledger);
    }

    /// The derived comparison over the same fields: the oracle for the
    /// counting manual `PartialEq`.
    #[derive(PartialEq)]
    struct Derived {
        fields: BTreeMap<String, FieldStamp>,
        tombstone: Option<FieldStamp>,
    }

    fn derived(value: &RowMetadata) -> Derived {
        let RowMetadata {
            fields,
            tombstone,
        } = value;
        Derived {
            fields: fields.clone(),
            tombstone: tombstone.clone(),
        }
    }

    #[test]
    fn si_w03_row_metadata_comparison_matches_derived_oracle() {
        let base = metadata(4);
        let mut variants = vec![base.clone(), metadata(4)];
        let mut extra = base.clone();
        extra.fields.insert("other".into(), stamp(1));
        variants.push(extra);
        let mut later = base.clone();
        later.fields.get_mut("body").unwrap().timestamp_ms += 1;
        variants.push(later);
        let mut provisional = base.clone();
        provisional.fields.get_mut("body").unwrap().authoritative = false;
        variants.push(provisional);
        let mut renamed = base.clone();
        let stamp_value = renamed.fields.remove("body").unwrap();
        renamed.fields.insert("title".into(), stamp_value);
        variants.push(renamed);
        let mut live = base.clone();
        live.tombstone = None;
        variants.push(live);
        let mut tombstone_later = base.clone();
        tombstone_later.tombstone.as_mut().unwrap().timestamp_ms += 1;
        variants.push(tombstone_later);
        let mut grave = base.clone();
        grave.tombstone.as_mut().unwrap().authoritative = false;
        variants.push(grave);
        let (mut equal, mut unequal) = (0, 0);
        for left in &variants {
            for right in &variants {
                counters::reset();
                let manual = left == right;
                assert_eq!(counters::snapshot().metadata_entries_visited, 1);
                assert_eq!(manual, derived(left) == derived(right));
                if manual {
                    equal += 1;
                } else {
                    unequal += 1;
                }
            }
        }
        // The diagonal plus the two separately built equal values.
        assert_eq!((equal, unequal), (9 + 2, 81 - 11));
    }

    #[test]
    fn si_w03_metadata_comparisons_visit_each_entry() {
        let left = BTreeMap::from([
            (("items".to_owned(), "a".to_owned()), metadata(1)),
            (("items".to_owned(), "b".to_owned()), metadata(2)),
        ]);
        let right = left.clone();
        counters::reset();
        assert_eq!(left, right);
        assert_eq!(counters::snapshot().metadata_entries_visited, 2);
    }
}
