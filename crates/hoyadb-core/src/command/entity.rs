//! API-0009: owned entity comparison state and pure per-field resolution.
use super::{CommandError, Json};
use crate::{
    model::SchemaDefinition,
    storage::{Row, StorageError, Value},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityConfig {
    pub scope: String,
    pub tables: BTreeSet<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EntityContext {
    pub identity: Json,
    pub now_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityOperation {
    Create,
    Update,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityDescriptor {
    pub table: String,
    pub op: EntityOperation,
    pub timestamp_ms: i64,
    pub removed_fields: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldStamp {
    pub timestamp_ms: i64,
    pub authoritative: bool,
}

#[derive(Debug, Eq, Default)]
pub struct RowMetadata {
    pub fields: BTreeMap<String, FieldStamp>,
    pub tombstone: Option<FieldStamp>,
}

/// SPEC-0037 R4: every copy counts one metadata entry. Exhaustive
/// destructuring makes a new field a compile error here.
impl Clone for RowMetadata {
    fn clone(&self) -> Self {
        let Self {
            fields,
            tombstone,
        } = self;
        crate::storage::count_work!(ledger_entries_cloned, 1);
        Self {
            fields: fields.clone(),
            tombstone: tombstone.clone(),
        }
    }
}

/// SPEC-0037 R4: every comparison examines one metadata entry.
impl PartialEq for RowMetadata {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            fields,
            tombstone,
        } = self;
        let Self {
            fields: other_fields,
            tombstone: other_tombstone,
        } = other;
        crate::storage::count_work!(metadata_entries_visited, 1);
        fields == other_fields && tombstone == other_tombstone
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataChange {
    pub table: String,
    pub record_id: String,
    pub before: Option<RowMetadata>,
    pub after: Option<RowMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityLedger {
    pub ledger_format: u8,
    pub config: EntityConfig,
    pub generation: i64,
    pub revision: i64,
    pub checkpoint: Option<String>,
    pub ready: bool,
    pub pages: i64,
    pub changes: i64,
    pub metadata: BTreeMap<(String, String), RowMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityChange {
    pub table: String,
    pub record_id: String,
    pub op: EntityOperation,
    pub changes: BTreeMap<String, Value>,
    pub removed_fields: BTreeSet<String>,
    pub timestamp_ms: i64,
    pub field_timestamps: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityResolution {
    pub row: Option<Row>,
    pub metadata: RowMetadata,
}

pub struct MergeInput<'a> {
    pub schema: &'a SchemaDefinition,
    pub current: Option<&'a Row>,
    pub metadata: &'a RowMetadata,
    pub incoming: &'a EntityChange,
    pub default: &'a EntityResolution,
}

pub type MergeHook = fn(&MergeInput<'_>) -> Result<EntityResolution, CommandError>;

impl EntityLedger {
    pub fn new(config: EntityConfig) -> Self {
        Self {
            ledger_format: 3,
            config,
            generation: 1,
            revision: 0,
            checkpoint: None,
            ready: false,
            pages: 0,
            changes: 0,
            metadata: BTreeMap::new(),
        }
    }

    pub fn is_pristine(&self) -> bool {
        self.checkpoint.is_none()
            && !self.ready
            && self.pages == 0
            && self.changes == 0
            && self.metadata.is_empty()
    }

    pub fn validate(&self, schema: &SchemaDefinition) -> Result<(), StorageError> {
        self.validate_intrinsic()?;
        let normalized = schema.storage_schema()?;
        if self
            .config
            .tables
            .iter()
            .any(|table| !normalized.tables.contains_key(table))
        {
            return Err(invalid());
        }
        for ((table, _), metadata) in &self.metadata {
            crate::storage::count_work!(metadata_entries_visited, 1);
            let definition = normalized.tables.get(table).ok_or_else(invalid)?;
            if metadata.fields.keys().any(|key| {
                key == crate::model::ROW_VERSION || !definition.columns.contains_key(key)
            }) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// Validates header/configuration without walking comparison metadata.
    pub(crate) fn validate_header(&self, schema: &SchemaDefinition) -> Result<(), StorageError> {
        self.validate_header_intrinsic()?;
        let normalized = schema.storage_schema()?;
        if self
            .config
            .tables
            .iter()
            .any(|table| !normalized.tables.contains_key(table))
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn validate_header_intrinsic(&self) -> Result<(), StorageError> {
        if !matches!(self.ledger_format, 2 | 3)
            || self.config.scope.is_empty()
            || self.generation < 1
            || self.revision < 0
            || self.pages < 0
            || self.changes < 0
            || (self.pages == 0 && (self.checkpoint.is_some() || self.ready || self.changes != 0))
            || (self.pages > 0 && self.checkpoint.is_none())
        {
            return Err(invalid());
        }
        for table in &self.config.tables {
            crate::model::check_name(table).map_err(|_| invalid())?;
        }
        bounded(&header_json(self))
    }

    pub(crate) fn validate_intrinsic(&self) -> Result<(), StorageError> {
        self.validate_header_intrinsic()?;
        for ((table, id), metadata) in &self.metadata {
            validate_metadata_entry(table, id, metadata)?;
        }
        Ok(())
    }
}

/// The intrinsic checks for one touched comparison-state entry.
pub(crate) fn validate_metadata_entry(
    table: &str,
    id: &str,
    metadata: &RowMetadata,
) -> Result<(), StorageError> {
    crate::storage::count_work!(metadata_entries_visited, 1);
    crate::model::check_name(table).map_err(|_| invalid())?;
    if id.is_empty() {
        return Err(invalid());
    }
    metadata.validate()?;
    bounded(&object([
        ("table", text(table)),
        ("record_id", text(id)),
        ("metadata", metadata_json(metadata)),
    ]))
}

const MAX_TIME: i64 = 253_402_300_799_999;
pub(crate) fn valid_time(time: i64) -> bool {
    (0..=MAX_TIME).contains(&time)
}
impl RowMetadata {
    pub fn validate(&self) -> Result<(), StorageError> {
        for (name, stamp) in &self.fields {
            crate::model::check_name(name).map_err(|_| invalid())?;
            if !valid_time(stamp.timestamp_ms) {
                return Err(invalid());
            }
        }
        if self
            .tombstone
            .as_ref()
            .is_some_and(|stamp| !valid_time(stamp.timestamp_ms))
        {
            return Err(invalid());
        }
        bounded(&metadata_json(self))
    }
}

pub fn resolve_entity(
    schema: &SchemaDefinition,
    current: Option<&Row>,
    metadata: &RowMetadata,
    incoming: &EntityChange,
    authoritative: bool,
    hook: Option<MergeHook>,
) -> Result<EntityResolution, CommandError> {
    let mut storage = schema.storage_schema()?;
    for table in storage.tables.values_mut() {
        table.columns.remove(crate::model::ROW_VERSION);
    }
    let definition = storage
        .tables
        .get(&incoming.table)
        .ok_or(CommandError::InvalidCommand)?;
    metadata.validate()?;
    if incoming.record_id.is_empty()
        || !valid_time(incoming.timestamp_ms)
        || incoming.field_timestamps.keys().ne(incoming.changes.keys())
        || incoming
            .field_timestamps
            .values()
            .any(|time| !valid_time(*time))
        || metadata
            .fields
            .keys()
            .any(|name| !definition.columns.contains_key(name))
    {
        return Err(CommandError::InvalidCommand);
    }
    if let Some(current) = current {
        if current.id != incoming.record_id || metadata.tombstone.is_some() {
            return Err(CommandError::InvalidCommand);
        }
        crate::storage::validate_row(&storage, &incoming.table, current)?;
        bounded(&row_json(current))?;
    }
    let descriptor = EntityDescriptor {
        table: incoming.table.clone(),
        op: incoming.op,
        timestamp_ms: incoming.timestamp_ms,
        removed_fields: incoming.removed_fields.clone(),
    };
    validate_descriptor(&descriptor)?;
    if incoming.op == EntityOperation::Delete
        && (!incoming.changes.is_empty() || !incoming.field_timestamps.is_empty())
    {
        return Err(CommandError::InvalidCommand);
    }
    for (name, value) in &incoming.changes {
        let column = definition
            .columns
            .get(name)
            .ok_or(CommandError::InvalidCommand)?;
        if name == crate::model::ROW_VERSION
            || (!crate::storage::value_has_type(value, &column.kind)
                && !(column.optional && *value == Value::Null))
        {
            return Err(CommandError::InvalidCommand);
        }
    }
    for name in &incoming.removed_fields {
        if incoming.changes.get(name) != Some(&Value::Null)
            || !definition.columns.get(name).is_some_and(|c| c.optional)
        {
            return Err(CommandError::InvalidCommand);
        }
    }
    bounded(&change_json(incoming))?;
    crate::storage::count_work!(rows_cloned, u64::from(current.is_some()));
    let mut result = EntityResolution {
        row: current.cloned(),
        metadata: metadata.clone(),
    };
    if metadata.tombstone.is_some() {
        result.row = None;
        // Once deleted, obsolete updates/creates are harmless page no-ops.
        return Ok(result);
    }
    if incoming.op == EntityOperation::Delete {
        result.row = None;
        result.metadata.tombstone = Some(FieldStamp {
            timestamp_ms: incoming.timestamp_ms,
            authoritative,
        });
    } else {
        crate::storage::count_work!(rows_cloned, u64::from(current.is_some()));
        let mut row = current.cloned().unwrap_or(Row {
            id: incoming.record_id.clone(),
            fields: BTreeMap::new(),
        });
        for (name, value) in &incoming.changes {
            let next = if incoming.removed_fields.contains(name) {
                None
            } else {
                Some(value)
            };
            let old = row.fields.get(name);
            let stamp = metadata.fields.get(name);
            let old_time = stamp.map_or(0, |value| value.timestamp_ms);
            let time = incoming.field_timestamps[name];
            let first = current.is_none() && stamp.is_none();
            if !first && time == old_time && old != next && !authoritative {
                return Err(CommandError::AuthorityRequired);
            }
            if first || time > old_time || (time == old_time && (old == next || authoritative)) {
                if let Some(value) = next {
                    row.fields.insert(name.clone(), value.clone());
                } else {
                    row.fields.remove(name);
                }
                result.metadata.fields.insert(
                    name.clone(),
                    FieldStamp {
                        timestamp_ms: time,
                        authoritative: authoritative
                            || (time == old_time && stamp.is_some_and(|s| s.authoritative)),
                    },
                );
            }
        }
        crate::storage::validate_row(&storage, &incoming.table, &row)?;
        bounded(&row_json(&row))?;
        result.row = Some(row);
    }
    result.metadata.validate()?;
    if let Some(hook) = hook {
        let input = MergeInput {
            schema,
            current,
            metadata,
            incoming,
            default: &result,
        };
        return validate_custom_resolution(&input, hook(&input)?);
    }
    Ok(result)
}

/// The same authority/schema guard serves synchronous and passive host hooks.
pub(super) fn validate_custom_resolution(
    input: &MergeInput<'_>,
    custom: EntityResolution,
) -> Result<EntityResolution, CommandError> {
    let result = input.default;
    let incoming = input.incoming;
    if custom.metadata != result.metadata || custom.row.is_some() != result.row.is_some() {
        return Err(CommandError::InvalidCommand);
    }
    if let (Some(custom), Some(default)) = (&custom.row, &result.row) {
        if custom.id != default.id {
            return Err(CommandError::InvalidCommand);
        }
        let mut storage = input.schema.storage_schema()?;
        for table in storage.tables.values_mut() {
            table.columns.remove(crate::model::ROW_VERSION);
        }
        crate::storage::validate_row(&storage, &incoming.table, custom)?;
        bounded(&row_json(custom))?;
        for name in custom.fields.keys().chain(default.fields.keys()) {
            let tied = incoming.field_timestamps.get(name).is_some_and(|time| {
                *time
                    == input
                        .metadata
                        .fields
                        .get(name)
                        .map_or(0, |s| s.timestamp_ms)
            });
            if (!incoming.changes.contains_key(name)
                || tied
                || incoming.removed_fields.contains(name))
                && custom.fields.get(name) != default.fields.get(name)
            {
                return Err(CommandError::InvalidCommand);
            }
        }
    }
    Ok(custom)
}

fn bounded(value: &Json) -> Result<(), StorageError> {
    value.canonical().map(|_| ()).map_err(|_| invalid())
}

fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn text(value: &str) -> Json {
    Json::String(value.into())
}

fn names(values: &BTreeSet<String>) -> Json {
    Json::Array(values.iter().map(|value| text(value)).collect())
}

fn exact<'a>(value: &'a Json, keys: &[&str]) -> Result<&'a BTreeMap<String, Json>, StorageError> {
    let Json::Object(map) = value else {
        return Err(invalid());
    };
    if map.len() != keys.len() || keys.iter().any(|key| !map.contains_key(*key)) {
        return Err(invalid());
    }
    Ok(map)
}

fn as_text(value: &Json) -> Result<&str, StorageError> {
    if let Json::String(value) = value {
        Ok(value)
    } else {
        Err(invalid())
    }
}

fn as_num(value: &Json) -> Result<i64, StorageError> {
    if let Json::Integer(value) = value {
        Ok(*value)
    } else {
        Err(invalid())
    }
}

fn as_bool(value: &Json) -> Result<bool, StorageError> {
    if let Json::Bool(value) = value {
        Ok(*value)
    } else {
        Err(invalid())
    }
}

fn as_names(value: &Json) -> Result<BTreeSet<String>, StorageError> {
    let Json::Array(values) = value else {
        return Err(invalid());
    };
    let values = values
        .iter()
        .map(|value| as_text(value).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    if values.windows(2).any(|p| p[0] >= p[1]) {
        return Err(invalid());
    }
    Ok(values.into_iter().collect())
}

pub(crate) fn op_name(value: EntityOperation) -> &'static str {
    match value {
        EntityOperation::Create => "create",
        EntityOperation::Update => "update",
        EntityOperation::Delete => "delete",
    }
}

fn operation(value: &Json) -> Result<EntityOperation, StorageError> {
    match as_text(value)? {
        "create" => Ok(EntityOperation::Create),
        "update" => Ok(EntityOperation::Update),
        "delete" => Ok(EntityOperation::Delete),
        _ => Err(invalid()),
    }
}

fn stamp_json(value: &FieldStamp) -> Json {
    object([
        ("timestamp_ms", Json::Integer(value.timestamp_ms)),
        ("authoritative", Json::Bool(value.authoritative)),
    ])
}

fn stamp_from_json(value: &Json) -> Result<FieldStamp, StorageError> {
    let map = exact(value, &["timestamp_ms", "authoritative"])?;
    let stamp = FieldStamp {
        timestamp_ms: as_num(&map["timestamp_ms"])?,
        authoritative: as_bool(&map["authoritative"])?,
    };
    if !valid_time(stamp.timestamp_ms) {
        return Err(invalid());
    }
    Ok(stamp)
}

pub(crate) fn validate_descriptor(value: &EntityDescriptor) -> Result<(), StorageError> {
    crate::model::check_name(&value.table).map_err(|_| invalid())?;
    if !valid_time(value.timestamp_ms)
        || (value.op != EntityOperation::Update && !value.removed_fields.is_empty())
    {
        return Err(invalid());
    }
    for field in &value.removed_fields {
        crate::model::check_name(field).map_err(|_| invalid())?;
    }
    bounded(&descriptor_json(value))
}

pub(crate) fn descriptor_json(value: &EntityDescriptor) -> Json {
    object([
        ("table", text(&value.table)),
        ("op", text(op_name(value.op))),
        ("timestamp_ms", Json::Integer(value.timestamp_ms)),
        ("removed_fields", names(&value.removed_fields)),
    ])
}

pub(crate) fn descriptor_from_json(value: &Json) -> Result<EntityDescriptor, StorageError> {
    let map = exact(value, &["table", "op", "timestamp_ms", "removed_fields"])?;
    let descriptor = EntityDescriptor {
        table: as_text(&map["table"])?.into(),
        op: operation(&map["op"])?,
        timestamp_ms: as_num(&map["timestamp_ms"])?,
        removed_fields: as_names(&map["removed_fields"])?,
    };
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

pub(crate) fn metadata_json(value: &RowMetadata) -> Json {
    object([
        (
            "fields",
            Json::Object(
                value
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), stamp_json(v)))
                    .collect(),
            ),
        ),
        (
            "tombstone",
            value
                .tombstone
                .as_ref()
                .map(stamp_json)
                .unwrap_or(Json::Null),
        ),
    ])
}

pub(crate) fn metadata_from_json(value: &Json) -> Result<RowMetadata, StorageError> {
    let map = exact(value, &["fields", "tombstone"])?;
    let Json::Object(fields) = &map["fields"] else {
        return Err(invalid());
    };
    let result = RowMetadata {
        fields: fields
            .iter()
            .map(|(k, v)| Ok((k.clone(), stamp_from_json(v)?)))
            .collect::<Result<_, StorageError>>()?,
        tombstone: if map["tombstone"] == Json::Null {
            None
        } else {
            Some(stamp_from_json(&map["tombstone"])?)
        },
    };
    result.validate()?;
    Ok(result)
}

pub(crate) fn metadata_change_json(value: &MetadataChange) -> Json {
    object([
        ("table", text(&value.table)),
        ("record_id", text(&value.record_id)),
        (
            "before",
            value
                .before
                .as_ref()
                .map(metadata_json)
                .unwrap_or(Json::Null),
        ),
        (
            "after",
            value
                .after
                .as_ref()
                .map(metadata_json)
                .unwrap_or(Json::Null),
        ),
    ])
}

pub(crate) fn metadata_change_from_json(value: &Json) -> Result<MetadataChange, StorageError> {
    let map = exact(value, &["table", "record_id", "before", "after"])?;
    let result = MetadataChange {
        table: as_text(&map["table"])?.into(),
        record_id: as_text(&map["record_id"])?.into(),
        before: if map["before"] == Json::Null {
            None
        } else {
            Some(metadata_from_json(&map["before"])?)
        },
        after: if map["after"] == Json::Null {
            None
        } else {
            Some(metadata_from_json(&map["after"])?)
        },
    };
    validate_metadata_change(&result)?;
    Ok(result)
}

pub(crate) fn validate_metadata_change(value: &MetadataChange) -> Result<(), StorageError> {
    crate::model::check_name(&value.table).map_err(|_| invalid())?;
    if value.record_id.is_empty() {
        return Err(invalid());
    }
    for metadata in value.before.iter().chain(value.after.iter()) {
        metadata.validate()?;
    }
    bounded(&metadata_change_json(value))
}

pub(crate) fn header_json(value: &EntityLedger) -> Json {
    let mut header = object([
        (
            "config",
            object([
                ("scope", text(&value.config.scope)),
                ("tables", names(&value.config.tables)),
            ]),
        ),
        ("generation", Json::Integer(value.generation)),
        ("revision", Json::Integer(value.revision)),
        (
            "checkpoint",
            value.checkpoint.as_deref().map(text).unwrap_or(Json::Null),
        ),
        ("ready", Json::Bool(value.ready)),
        ("pages", Json::Integer(value.pages)),
        ("changes", Json::Integer(value.changes)),
    ]);
    if value.ledger_format == 3
        && let Json::Object(fields) = &mut header
    {
        fields.insert("ledger_format".into(), Json::Integer(3));
    }
    header
}

pub(crate) fn header_from_json(value: &Json) -> Result<EntityLedger, StorageError> {
    let has_format = matches!(value, Json::Object(fields) if fields.contains_key("ledger_format"));
    let keys: &[&str] = if has_format {
        &[
            "config",
            "generation",
            "revision",
            "checkpoint",
            "ready",
            "pages",
            "changes",
            "ledger_format",
        ]
    } else {
        &[
            "config",
            "generation",
            "revision",
            "checkpoint",
            "ready",
            "pages",
            "changes",
        ]
    };
    let map = exact(value, keys)?;
    if has_format && map["ledger_format"] != Json::Integer(3) {
        return Err(invalid());
    }
    let config = exact(&map["config"], &["scope", "tables"])?;
    let result = EntityLedger {
        ledger_format: if has_format {
            3
        } else {
            2
        },
        config: EntityConfig {
            scope: as_text(&config["scope"])?.into(),
            tables: as_names(&config["tables"])?,
        },
        generation: as_num(&map["generation"])?,
        revision: as_num(&map["revision"])?,
        checkpoint: if map["checkpoint"] == Json::Null {
            None
        } else {
            Some(as_text(&map["checkpoint"])?.into())
        },
        ready: as_bool(&map["ready"])?,
        pages: as_num(&map["pages"])?,
        changes: as_num(&map["changes"])?,
        metadata: BTreeMap::new(),
    };
    result.validate_intrinsic()?;
    Ok(result)
}

pub(crate) fn value_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::String(v) | Value::Reference(v) => text(v),
        Value::Integer(v) | Value::Timestamp(v) => Json::Integer(*v),
        Value::Boolean(v) => Json::Bool(*v),
        Value::Float(v) => Json::Float(*v),
    }
}

fn row_json(row: &Row) -> Json {
    object([
        ("id", text(&row.id)),
        (
            "fields",
            Json::Object(
                row.fields
                    .iter()
                    .map(|(k, v)| (k.clone(), value_json(v)))
                    .collect(),
            ),
        ),
    ])
}

pub(crate) fn change_json(value: &EntityChange) -> Json {
    object([
        ("table", text(&value.table)),
        ("record_id", text(&value.record_id)),
        ("op", text(op_name(value.op))),
        (
            "changes",
            Json::Object(
                value
                    .changes
                    .iter()
                    .map(|(k, v)| (k.clone(), value_json(v)))
                    .collect(),
            ),
        ),
        ("removed_fields", names(&value.removed_fields)),
        ("timestamp_ms", Json::Integer(value.timestamp_ms)),
        (
            "field_timestamps",
            Json::Object(
                value
                    .field_timestamps
                    .iter()
                    .map(|(k, v)| (k.clone(), Json::Integer(*v)))
                    .collect(),
            ),
        ),
    ])
}

fn invalid() -> StorageError {
    StorageError::BackendFailure {
        message: "invalid entity comparison bookkeeping".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::TableDefinition,
        storage::{Column, ColumnType},
    };
    fn schema() -> SchemaDefinition {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([
                        (
                            "a".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        ),
                        (
                            "b".into(),
                            Column {
                                kind: ColumnType::Integer,
                                optional: true,
                            },
                        ),
                    ]),
                    relations: BTreeMap::new(),
                },
            )]),
        }
    }

    fn row() -> Row {
        Row {
            id: "r".into(),
            fields: BTreeMap::from([
                ("a".into(), Value::String("old".into())),
                ("b".into(), Value::Integer(1)),
            ]),
        }
    }

    fn change() -> EntityChange {
        EntityChange {
            table: "items".into(),
            record_id: "r".into(),
            op: EntityOperation::Update,
            changes: BTreeMap::from([("a".into(), Value::String("new".into()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 20,
            field_timestamps: BTreeMap::from([("a".into(), 20)]),
        }
    }

    fn stamp(time: i64) -> FieldStamp {
        FieldStamp {
            timestamp_ms: time,
            authoritative: false,
        }
    }

    #[test]
    fn si_c04_entity_candidates_count_existing_rows_and_absent_controls() {
        use crate::storage::counters;
        let current = row();
        for present in [true, false] {
            for deleting in [false, true] {
                let mut incoming = change();
                if deleting {
                    incoming.op = EntityOperation::Delete;
                    incoming.changes.clear();
                    incoming.field_timestamps.clear();
                }
                counters::reset();
                let resolved = resolve_entity(
                    &schema(),
                    present.then_some(&current),
                    &RowMetadata::default(),
                    &incoming,
                    true,
                    None,
                )
                .unwrap();
                assert_eq!(resolved.row.is_none(), deleting);
                assert_eq!(
                    counters::snapshot().rows_cloned,
                    if present {
                        if deleting {
                            1
                        } else {
                            2
                        }
                    } else {
                        0
                    },
                    "present={present} delete={deleting}"
                );
            }
        }
        let mut invalid = change();
        invalid.timestamp_ms = -1;
        counters::reset();
        assert!(
            resolve_entity(
                &schema(),
                Some(&current),
                &RowMetadata::default(),
                &invalid,
                true,
                None
            )
            .is_err()
        );
        assert_eq!(
            counters::snapshot().rows_cloned,
            0,
            "validation refuses before any candidate copy"
        );
    }

    #[test]
    fn si_y_entity_header_checks_skip_metadata_but_preserve_header_and_schema() {
        let mut value = EntityLedger::new(EntityConfig {
            scope: "scope".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        value.metadata.insert(
            ("items".into(), "r".into()),
            RowMetadata {
                fields: BTreeMap::from([("a".into(), stamp(-1))]),
                tombstone: None,
            },
        );
        assert!(value.validate(&schema()).is_err());
        value
            .validate_header_intrinsic()
            .expect("header intrinsic skips metadata");
        value
            .validate_header(&schema())
            .expect("header schema check skips metadata");
        value.revision = -1;
        assert!(value.validate_header_intrinsic().is_err());
        value.revision = 0;
        value.config.tables.insert("missing".into());
        assert!(value.validate_header(&schema()).is_err());
    }

    #[test]
    fn si_y_touched_metadata_validation_is_intrinsic_and_payload_bounded() {
        let metadata = RowMetadata {
            fields: BTreeMap::from([("a".into(), stamp(1))]),
            tombstone: None,
        };
        crate::storage::counters::reset();
        validate_metadata_entry("items", "r", &metadata).unwrap();
        assert_eq!(
            crate::storage::counters::snapshot().metadata_entries_visited,
            1
        );
        assert!(validate_metadata_entry("items", "", &metadata).is_err());
        assert!(validate_metadata_entry("", "r", &metadata).is_err());
        assert!(
            validate_metadata_entry(
                "items",
                &"r".repeat(super::super::json::MAX_BYTES),
                &metadata
            )
            .is_err()
        );
    }

    #[test]
    fn newer_fields_merge_and_equal_time_requires_authority() {
        let current = row();
        let mut meta = RowMetadata {
            fields: BTreeMap::from([("a".into(), stamp(10)), ("b".into(), stamp(30))]),
            tombstone: None,
        };
        let updated =
            resolve_entity(&schema(), Some(&current), &meta, &change(), false, None).unwrap();
        assert_eq!(
            updated.row.as_ref().unwrap().fields["a"],
            Value::String("new".into())
        );
        assert_eq!(updated.row.as_ref().unwrap().fields["b"], Value::Integer(1));
        meta.fields.insert("a".into(), stamp(20));
        assert_eq!(
            resolve_entity(&schema(), Some(&current), &meta, &change(), false, None),
            Err(CommandError::AuthorityRequired)
        );
        assert_eq!(
            resolve_entity(&schema(), Some(&current), &meta, &change(), true, None)
                .unwrap()
                .row
                .unwrap()
                .fields["a"],
            Value::String("new".into())
        );
    }

    #[test]
    fn removal_is_distinct_from_null_and_keeps_its_clock() {
        let mut incoming = change();
        incoming.changes = BTreeMap::from([("b".into(), Value::Null)]);
        incoming.field_timestamps = BTreeMap::from([("b".into(), 20)]);
        let present = resolve_entity(
            &schema(),
            Some(&row()),
            &RowMetadata::default(),
            &incoming,
            true,
            None,
        )
        .unwrap();
        assert_eq!(present.row.unwrap().fields.get("b"), Some(&Value::Null));
        incoming.removed_fields.insert("b".into());
        let absent = resolve_entity(
            &schema(),
            Some(&row()),
            &RowMetadata::default(),
            &incoming,
            true,
            None,
        )
        .unwrap();
        assert!(!absent.row.unwrap().fields.contains_key("b"));
        assert_eq!(absent.metadata.fields["b"].timestamp_ms, 20);
        incoming.removed_fields.insert("a".into());
        assert!(
            resolve_entity(
                &schema(),
                Some(&row()),
                &RowMetadata::default(),
                &incoming,
                true,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn deletion_precedence_and_missing_row_completeness() {
        let mut deleted = change();
        deleted.op = EntityOperation::Delete;
        deleted.changes.clear();
        deleted.field_timestamps.clear();
        let resolved = resolve_entity(
            &schema(),
            Some(&row()),
            &RowMetadata::default(),
            &deleted,
            true,
            None,
        )
        .unwrap();
        assert!(resolved.row.is_none());
        assert!(resolved.metadata.tombstone.as_ref().unwrap().authoritative);
        assert_eq!(
            resolve_entity(&schema(), None, &resolved.metadata, &change(), true, None).unwrap(),
            resolved
        );
        let mut incomplete = change();
        incomplete.changes.clear();
        incomplete.field_timestamps.clear();
        assert!(
            resolve_entity(
                &schema(),
                None,
                &RowMetadata::default(),
                &incomplete,
                true,
                None
            )
            .is_err()
        );
        assert!(
            resolve_entity(
                &schema(),
                None,
                &RowMetadata::default(),
                &change(),
                true,
                None
            )
            .unwrap()
            .row
            .is_some()
        );
    }

    fn bad_hook(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
        let mut result = input.default.clone();
        result
            .row
            .as_mut()
            .unwrap()
            .fields
            .insert("b".into(), Value::Integer(99));
        Ok(result)
    }

    fn custom_hook(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
        let mut result = input.default.clone();
        result
            .row
            .as_mut()
            .unwrap()
            .fields
            .insert("a".into(), Value::String("custom".into()));
        Ok(result)
    }

    #[test]
    fn hooks_are_schema_checked_and_cannot_modify_unrelated_or_tied_fields() {
        let meta = RowMetadata::default();
        assert!(
            resolve_entity(
                &schema(),
                Some(&row()),
                &meta,
                &change(),
                true,
                Some(bad_hook)
            )
            .is_err()
        );
        assert_eq!(
            resolve_entity(
                &schema(),
                Some(&row()),
                &meta,
                &change(),
                true,
                Some(custom_hook)
            )
            .unwrap()
            .row
            .unwrap()
            .fields["a"],
            Value::String("custom".into())
        );
        let meta = RowMetadata {
            fields: BTreeMap::from([("a".into(), stamp(20))]),
            tombstone: None,
        };
        assert!(
            resolve_entity(
                &schema(),
                Some(&row()),
                &meta,
                &change(),
                true,
                Some(custom_hook)
            )
            .is_err()
        );
    }

    #[test]
    fn exact_metadata_header_and_descriptor_roundtrip_and_validation() {
        let mut ledger = EntityLedger::new(EntityConfig {
            scope: "tenant:a".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        ledger.validate(&schema()).unwrap();
        assert_eq!(header_from_json(&header_json(&ledger)).unwrap(), ledger);
        let metadata = RowMetadata {
            fields: BTreeMap::from([("a".into(), stamp(12))]),
            tombstone: Some(FieldStamp {
                timestamp_ms: 15,
                authoritative: true,
            }),
        };
        assert_eq!(
            metadata_from_json(&metadata_json(&metadata)).unwrap(),
            metadata
        );
        let descriptor = EntityDescriptor {
            table: "items".into(),
            op: EntityOperation::Update,
            timestamp_ms: 20,
            removed_fields: BTreeSet::from(["b".into()]),
        };
        assert_eq!(
            descriptor_from_json(&descriptor_json(&descriptor)).unwrap(),
            descriptor
        );
        let undo = MetadataChange {
            table: "items".into(),
            record_id: "r".into(),
            before: None,
            after: Some(metadata),
        };
        assert_eq!(
            metadata_change_from_json(&metadata_change_json(&undo)).unwrap(),
            undo
        );
        ledger.config.tables.insert("foreign".into());
        assert!(ledger.validate(&schema()).is_err());
        assert!(metadata_from_json(&Json::Object(BTreeMap::new())).is_err());
    }

    #[test]
    fn timestamp_shape_types_and_oversize_candidates_reject_without_touching_inputs() {
        let current = row();
        let metadata = RowMetadata::default();
        let before = current.clone();
        for time in [-1, MAX_TIME + 1] {
            let mut candidate = change();
            candidate.timestamp_ms = time;
            candidate.field_timestamps.insert("a".into(), time);
            assert!(
                resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None)
                    .is_err()
            );
        }
        let mut candidate = change();
        candidate.field_timestamps.clear();
        assert!(
            resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None).is_err()
        );
        candidate = change();
        candidate.changes.insert("a".into(), Value::Integer(9));
        assert!(
            resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None).is_err()
        );
        candidate = change();
        candidate.changes.insert(
            "a".into(),
            Value::String("x".repeat(super::super::json::MAX_BYTES)),
        );
        assert!(
            resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None).is_err()
        );
        assert_eq!(current, before);
        assert_eq!(metadata, RowMetadata::default());
        let mut candidate = change();
        candidate.timestamp_ms = MAX_TIME;
        candidate.field_timestamps.insert("a".into(), MAX_TIME);
        assert_eq!(
            resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None)
                .unwrap()
                .metadata
                .fields["a"]
                .timestamp_ms,
            MAX_TIME
        );
    }

    #[test]
    fn removed_clocks_prevent_older_updates_and_all_declared_tables_allow_inbound_metadata() {
        let current = Row {
            id: "r".into(),
            fields: BTreeMap::from([("a".into(), Value::String("old".into()))]),
        };
        let metadata = RowMetadata {
            fields: BTreeMap::from([("b".into(), stamp(30))]),
            tombstone: None,
        };
        let mut candidate = change();
        candidate.changes = BTreeMap::from([("b".into(), Value::Integer(2))]);
        candidate.field_timestamps = BTreeMap::from([("b".into(), 20)]);
        let result =
            resolve_entity(&schema(), Some(&current), &metadata, &candidate, true, None).unwrap();
        assert_eq!(result.row, Some(current));
        assert_eq!(result.metadata, metadata);
        let mut ledger = EntityLedger::new(EntityConfig {
            scope: "scope".into(),
            tables: BTreeSet::new(),
        });
        ledger
            .metadata
            .insert(("items".into(), "r".into()), metadata);
        ledger.validate(&schema()).unwrap();
        ledger
            .metadata
            .get_mut(&("items".into(), "r".into()))
            .unwrap()
            .fields
            .insert("unknown".into(), stamp(1));
        assert!(ledger.validate(&schema()).is_err());
    }
}
