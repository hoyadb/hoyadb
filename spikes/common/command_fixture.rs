//! API-0008 / TC-0032 C20: shared experimental host bridge over the real owner.
use hoyadb_core::{command::*, model::*, storage::*};
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage, StorageProtection};
use zeroize::Zeroizing;

#[path = "command_budget.rs"]
mod budget;
#[path = "command_entity.rs"]
mod entity_host;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
struct Error(&'static str);

impl From<CommandError> for Error {
    fn from(error: CommandError) -> Self {
        Self(match error {
            CommandError::Storage(error) => return error.into(),
            CommandError::InvalidCommand => "InvalidCommand",
            CommandError::InvalidTicket => "InvalidTicket",
            CommandError::Busy => "Busy",
            CommandError::Capacity => "Capacity",
            CommandError::InvalidPayload(_) => "InvalidPayload",
            CommandError::Protocol => "Protocol",
            CommandError::UnknownLocalReference => "UnknownLocalReference",
            CommandError::RollbackConflict => "RollbackConflict",
            CommandError::Closed => "Closed",
            CommandError::LimitExceeded => "LimitExceeded",
            CommandError::AuthorityRequired => "AuthorityRequired",
            CommandError::StalePull => "StalePull",
            CommandError::PullConflict => "PullConflict",
        })
    }
}

impl From<StorageError> for Error {
    fn from(error: StorageError) -> Self {
        Self(match error {
            StorageError::MigrationFailure {
                ..
            } => "MigrationFailure",
            StorageError::SchemaConflict {
                ..
            } => "SchemaConflict",
            StorageError::InvalidSchema {
                ..
            } => "InvalidSchema",
            StorageError::InvalidQuery {
                ..
            } => "InvalidQuery",
            StorageError::InvalidRow {
                ..
            } => "InvalidRow",
            StorageError::BackendFailure {
                ..
            } => "BackendFailure",
            StorageError::TableNotFound {
                ..
            } => "TableNotFound",
            StorageError::RowAlreadyExists {
                ..
            } => "RowAlreadyExists",
            StorageError::RowNotFound {
                ..
            } => "RowNotFound",
            StorageError::StoreInUse => "Busy",
        })
    }
}

fn run_once<T, E: Into<Error>>(
    future: impl Future<Output = std::result::Result<T, E>>,
) -> Result<T> {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value.map_err(Into::into),
        Poll::Pending => Err(Error("UnexpectedPending")),
    }
}

fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}

fn exact<'a>(value: &'a Json, names: &[&str]) -> Result<&'a BTreeMap<String, Json>> {
    let Json::Object(map) = value else {
        return Err(Error("InvalidRequest"));
    };
    if map.len() != names.len() || names.iter().any(|name| !map.contains_key(*name)) {
        return Err(Error("InvalidRequest"));
    }
    Ok(map)
}

fn field<'a>(map: &'a BTreeMap<String, Json>, name: &str) -> Result<&'a Json> {
    map.get(name).ok_or(Error("InvalidRequest"))
}

fn string(value: &Json) -> Result<&str> {
    if let Json::String(value) = value {
        Ok(value)
    } else {
        Err(Error("InvalidRequest"))
    }
}

fn boolean(value: &Json) -> Result<bool> {
    if let Json::Bool(value) = value {
        Ok(*value)
    } else {
        Err(Error("InvalidRequest"))
    }
}

fn array(value: &Json) -> Result<&[Json]> {
    if let Json::Array(value) = value {
        Ok(value)
    } else {
        Err(Error("InvalidRequest"))
    }
}

fn map(value: &Json) -> Result<&BTreeMap<String, Json>> {
    if let Json::Object(value) = value {
        Ok(value)
    } else {
        Err(Error("InvalidRequest"))
    }
}

fn integer(value: &Json) -> Result<i64> {
    match Json::parse(string(value)?).map_err(|_| Error("InvalidRequest"))? {
        Json::Integer(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

fn unsigned(value: &Json) -> Result<u64> {
    let value = string(value)?;
    let parsed = value.parse::<u64>().map_err(|_| Error("InvalidRequest"))?;
    if value != parsed.to_string() {
        return Err(Error("InvalidRequest"));
    }
    Ok(parsed)
}

fn handle(value: &Json) -> Result<u64> {
    let id = unsigned(value)?;
    if id == 0 {
        Err(Error("InvalidHandle"))
    } else {
        Ok(id)
    }
}

fn no_numbers(value: &Json) -> bool {
    match value {
        Json::Integer(_) | Json::Float(_) => false,
        Json::Array(values) => values.iter().all(no_numbers),
        Json::Object(values) => values.values().all(no_numbers),
        _ => true,
    }
}

fn float(value: &str) -> Result<FiniteFloat> {
    let number = if value.contains(['.', 'e', 'E']) {
        value.to_owned()
    } else {
        format!("{value}.0")
    };
    match Json::parse(&number).map_err(|_| Error("InvalidRequest"))? {
        Json::Float(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

fn unbox(value: &Json) -> Result<Json> {
    let map = map(value)?;
    let kind = string(field(map, "kind")?)?;
    if kind == "null" {
        exact(value, &["kind"])?;
        return Ok(Json::Null);
    }
    exact(value, &["kind", "value"])?;
    let value = field(map, "value")?;
    let value = match kind {
        "boolean" => Json::Bool(boolean(value)?),
        "string" => Json::String(string(value)?.into()),
        "integer" => Json::Integer(integer(value)?),
        "float" => Json::Float(float(string(value)?)?),
        "array" => Json::Array(array(value)?.iter().map(unbox).collect::<Result<_>>()?),
        "object" => Json::Object(
            map_value(value)?
                .iter()
                .map(|(k, v)| Ok((k.clone(), unbox(v)?)))
                .collect::<Result<_>>()?,
        ),
        _ => return Err(Error("InvalidRequest")),
    };
    value.canonical().map_err(|_| Error("LimitExceeded"))?;
    Ok(value)
}

fn map_value(value: &Json) -> Result<&BTreeMap<String, Json>> {
    map(value)
}

fn boxed(value: &Json) -> Json {
    match value {
        Json::Null => object([("kind", text("null"))]),
        Json::Bool(value) => object([("kind", text("boolean")), ("value", Json::Bool(*value))]),
        Json::String(value) => object([("kind", text("string")), ("value", text(value))]),
        Json::Integer(value) => object([("kind", text("integer")), ("value", text(value))]),
        Json::Float(value) => object([("kind", text("float")), ("value", text(value.get()))]),
        Json::Array(values) => object([
            ("kind", text("array")),
            ("value", Json::Array(values.iter().map(boxed).collect())),
        ]),
        Json::Object(values) => object([
            ("kind", text("object")),
            (
                "value",
                Json::Object(values.iter().map(|(k, v)| (k.clone(), boxed(v))).collect()),
            ),
        ]),
    }
}

fn cell(value: &Json) -> Result<Value> {
    let fields = map(value)?;
    match string(field(fields, "kind")?)? {
        "timestamp" => {
            exact(value, &["kind", "value"])?;
            Ok(Value::Timestamp(integer(field(fields, "value")?)?))
        }
        "reference" => {
            exact(value, &["kind", "value"])?;
            Ok(Value::Reference(string(field(fields, "value")?)?.into()))
        }
        _ => Ok(match unbox(value)? {
            Json::Null => Value::Null,
            Json::Bool(v) => Value::Boolean(v),
            Json::String(v) => Value::String(v),
            Json::Integer(v) => Value::Integer(v),
            Json::Float(v) => Value::Float(v),
            _ => return Err(Error("InvalidRequest")),
        }),
    }
}

fn cell_wire(value: &Value) -> Json {
    match value {
        Value::Timestamp(value) => object([("kind", text("timestamp")), ("value", text(value))]),
        Value::Reference(value) => object([("kind", text("reference")), ("value", text(value))]),
        Value::Null => boxed(&Json::Null),
        Value::String(value) => boxed(&Json::String(value.clone())),
        Value::Integer(value) => boxed(&Json::Integer(*value)),
        Value::Boolean(value) => boxed(&Json::Bool(*value)),
        Value::Float(value) => boxed(&Json::Float(*value)),
    }
}

fn row_fields(value: &Json) -> Result<BTreeMap<String, Value>> {
    map(value)?
        .iter()
        .map(|(k, v)| Ok((k.clone(), cell(v)?)))
        .collect()
}

fn row_wire(row: &Row) -> Json {
    object([
        ("id", text(&row.id)),
        (
            "fields",
            Json::Object(
                row.fields
                    .iter()
                    .map(|(k, v)| (k.clone(), cell_wire(v)))
                    .collect(),
            ),
        ),
    ])
}

fn rows_wire(rows: &[Row]) -> Json {
    Json::Array(rows.iter().map(row_wire).collect())
}

fn changes_wire(changes: &[CommittedChange]) -> Json {
    Json::Array(
        changes
            .iter()
            .map(|change| {
                object([
                    ("table", text(&change.table)),
                    ("id", text(&change.id)),
                    (
                        "before",
                        change.before.as_ref().map(row_wire).unwrap_or(Json::Null),
                    ),
                    (
                        "after",
                        change.after.as_ref().map(row_wire).unwrap_or(Json::Null),
                    ),
                ])
            })
            .collect(),
    )
}

fn options(value: &Json) -> Result<SyncOptions> {
    let f = exact(
        value,
        &[
            "capacity",
            "history",
            "initial_delay_ms",
            "multiplier",
            "max_delay_ms",
        ],
    )?;
    Ok(SyncOptions {
        capacity: usize::try_from(unsigned(field(f, "capacity")?)?)
            .map_err(|_| Error("InvalidRequest"))?,
        history: usize::try_from(unsigned(field(f, "history")?)?)
            .map_err(|_| Error("InvalidRequest"))?,
        initial_delay_ms: integer(field(f, "initial_delay_ms")?)?,
        multiplier: u32::try_from(unsigned(field(f, "multiplier")?)?)
            .map_err(|_| Error("InvalidRequest"))?,
        max_delay_ms: integer(field(f, "max_delay_ms")?)?,
    })
}

fn definition(value: &Json) -> Result<CommandDefinition> {
    let f = exact(value, &["name", "arguments", "reference_paths"])?;
    let arguments = map(field(f, "arguments")?)?
        .iter()
        .map(|(name, value)| {
            let fields = exact(value, &["kind", "optional"])?;
            let kind = match string(field(fields, "kind")?)? {
                "Any" => ArgumentType::Any,
                "String" => ArgumentType::String,
                "Integer" => ArgumentType::Integer,
                "Float" => ArgumentType::Float,
                "Boolean" => ArgumentType::Boolean,
                "Array" => ArgumentType::Array,
                "Object" => ArgumentType::Object,
                _ => return Err(Error("InvalidRequest")),
            };
            Ok((
                name.clone(),
                Argument {
                    kind,
                    optional: boolean(field(fields, "optional")?)?,
                },
            ))
        })
        .collect::<Result<_>>()?;
    let reference_paths = array(field(f, "reference_paths")?)?
        .iter()
        .map(|path| {
            array(path)?
                .iter()
                .map(|part| {
                    let parts = map(part)?;
                    if parts.len() != 1 {
                        return Err(Error("InvalidRequest"));
                    }
                    if let Some(key) = parts.get("key") {
                        Ok(PathPart::Key(string(key)?.into()))
                    } else {
                        Ok(PathPart::Index(
                            usize::try_from(unsigned(field(parts, "index")?)?)
                                .map_err(|_| Error("InvalidRequest"))?,
                        ))
                    }
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<_>>()?;
    Ok(CommandDefinition {
        name: string(field(f, "name")?)?.into(),
        arguments,
        reference_paths,
    })
}

fn query(db: &CommandDatabase<SqliteStorage>, value: &Json) -> Result<QueryBuilder> {
    let fields = map(value)?;
    let has_source = fields.contains_key("source");
    let names = if has_source {
        vec!["table", "predicates", "order", "limit", "source"]
    } else {
        vec!["table", "predicates", "order", "limit"]
    };
    let f = exact(value, &names)?;
    let table = string(field(f, "table")?)?;
    let mut query = if has_source {
        let source = exact(field(f, "source")?, &["table", "id", "relation"])?;
        db.related(
            string(field(source, "table")?)?,
            string(field(source, "id")?)?,
            string(field(source, "relation")?)?,
        )
    } else {
        db.query(table)
    };
    for predicate in array(field(f, "predicates")?)? {
        let p = exact(predicate, &["field", "operator", "values"])?;
        let name = string(field(p, "field")?)?;
        let values = array(field(p, "values")?)?
            .iter()
            .map(cell)
            .collect::<Result<Vec<_>>>()?;
        let op = match string(field(p, "operator")?)? {
            "Eq" => Operator::Eq,
            "Ne" => Operator::Ne,
            "Lt" => Operator::Lt,
            "Le" => Operator::Le,
            "Gt" => Operator::Gt,
            "Ge" => Operator::Ge,
            "In" => Operator::In,
            _ => return Err(Error("InvalidRequest")),
        };
        if op == Operator::In {
            query = query.where_in(name, values);
        } else {
            let [value] = values.as_slice() else {
                return Err(Error("InvalidRequest"));
            };
            query = query.where_(name, op, value.clone());
        }
    }
    if field(f, "order")? != &Json::Null {
        let order = exact(field(f, "order")?, &["field", "direction"])?;
        query = query.order_by(
            string(field(order, "field")?)?,
            match string(field(order, "direction")?)? {
                "Asc" => Direction::Asc,
                "Desc" => Direction::Desc,
                _ => return Err(Error("InvalidRequest")),
            },
        );
    }
    if field(f, "limit")? != &Json::Null {
        query = query.limit(
            usize::try_from(unsigned(field(f, "limit")?)?).map_err(|_| Error("InvalidRequest"))?,
        );
    }
    Ok(query)
}

fn writes(value: &Json) -> Result<Vec<Write>> {
    array(value)?
        .iter()
        .map(|value| {
            let f = map(value)?;
            let op = string(field(f, "op")?)?;
            let table = string(field(f, "table")?)?.into();
            Ok(match op {
                "insert" => {
                    exact(value, &["op", "table", "fields"])?;
                    Write::Insert {
                        table,
                        fields: row_fields(field(f, "fields")?)?,
                    }
                }
                "replace" => {
                    exact(value, &["op", "table", "id", "fields"])?;
                    Write::Replace {
                        table,
                        id: string(field(f, "id")?)?.into(),
                        fields: row_fields(field(f, "fields")?)?,
                    }
                }
                "remove" => {
                    exact(value, &["op", "table", "id"])?;
                    Write::Remove {
                        table,
                        id: string(field(f, "id")?)?.into(),
                    }
                }
                _ => return Err(Error("InvalidRequest")),
            })
        })
        .collect()
}

fn plan(value: &Json) -> Result<ReconcilePlan> {
    let f = exact(value, &["writes", "mappings"])?;
    let writes = array(field(f, "writes")?)?
        .iter()
        .map(|value| {
            let f = map(value)?;
            match string(field(f, "op")?)? {
                "put" => {
                    exact(value, &["op", "table", "id", "fields"])?;
                    Ok(ReconcileWrite::Put {
                        table: string(field(f, "table")?)?.into(),
                        id: string(field(f, "id")?)?.into(),
                        fields: row_fields(field(f, "fields")?)?,
                    })
                }
                "remove" => {
                    exact(value, &["op", "table", "id"])?;
                    Ok(ReconcileWrite::Remove {
                        table: string(field(f, "table")?)?.into(),
                        id: string(field(f, "id")?)?.into(),
                    })
                }
                "entity" => {
                    exact(value, &["op", "change"])?;
                    let page = PullPage::from_json(&object([
                        ("checkpoint", text("reconcile-codec")),
                        ("complete", Json::Bool(true)),
                        ("changes", Json::Array(vec![field(f, "change")?.clone()])),
                    ]))?;
                    Ok(ReconcileWrite::Entity {
                        change: page
                            .changes
                            .into_iter()
                            .next()
                            .ok_or(Error("InvalidRequest"))?,
                    })
                }
                _ => Err(Error("InvalidRequest")),
            }
        })
        .collect::<Result<_>>()?;
    let mappings = map(field(f, "mappings")?)?
        .iter()
        .map(|(k, v)| Ok((k.clone(), string(v)?.into())))
        .collect::<Result<_>>()?;
    Ok(ReconcilePlan {
        writes,
        mappings,
    })
}

fn outcome(value: &Json) -> Result<RemoteOutcome> {
    let f = map(value)?;
    match string(field(f, "state")?)? {
        "applied" => {
            exact(value, &["state", "result"])?;
            Ok(RemoteOutcome::Applied(unbox(field(f, "result")?)?))
        }
        "corrected" => {
            exact(value, &["state", "result"])?;
            Ok(RemoteOutcome::Corrected(unbox(field(f, "result")?)?))
        }
        "rejected" => {
            exact(value, &["state", "reason"])?;
            Ok(RemoteOutcome::Rejected(string(field(f, "reason")?)?.into()))
        }
        _ => Err(Error("InvalidRequest")),
    }
}

fn outcome_wire(value: &RemoteOutcome) -> Json {
    match value {
        RemoteOutcome::Applied(value) => {
            object([("state", text("applied")), ("result", boxed(value))])
        }
        RemoteOutcome::Corrected(value) => {
            object([("state", text("corrected")), ("result", boxed(value))])
        }
        RemoteOutcome::Rejected(value) => {
            object([("state", text("rejected")), ("reason", text(value))])
        }
    }
}

fn app_outcome_wire(value: &CommandOutcome) -> Json {
    match value {
        CommandOutcome::Queued => object([("state", text("queued"))]),
        CommandOutcome::Sent => object([("state", text("sent"))]),
        CommandOutcome::Acknowledged => object([("state", text("acknowledged"))]),
        CommandOutcome::Applied(value) => outcome_wire(&RemoteOutcome::Applied(value.clone())),
        CommandOutcome::Corrected(value) => outcome_wire(&RemoteOutcome::Corrected(value.clone())),
        CommandOutcome::Rejected(reason) => outcome_wire(&RemoteOutcome::Rejected(reason.clone())),
    }
}

fn state_name(state: &SyncState) -> &'static str {
    match state {
        SyncState::Queued => "queued",
        SyncState::Sent => "sent",
        SyncState::Acknowledged => "acknowledged",
        SyncState::Applied => "applied",
        SyncState::Corrected => "corrected",
        SyncState::Rejected => "rejected",
        SyncState::Cancelled => "cancelled",
    }
}

fn status_wire(value: QueueStatus) -> Json {
    object([
        ("pending", text(value.pending)),
        ("sending", text(value.sending)),
        ("awaiting_outcome", text(value.awaiting_outcome)),
        ("live", text(value.live)),
        ("blocked", Json::Bool(value.blocked)),
        (
            "commands",
            Json::Array(
                value
                    .commands
                    .into_iter()
                    .map(|entry| {
                        let mut value = object([
                            ("key", text(entry.key)),
                            ("state", text(state_name(&entry.state))),
                            (
                                "blocked_on",
                                Json::Array(entry.blocked_on.into_iter().map(text).collect()),
                            ),
                            ("awaiting_local", Json::Bool(entry.awaiting_local)),
                            ("rolled_back", Json::Bool(entry.rolled_back)),
                        ]);
                        if entry.kind == QueuedWriteKind::Entity {
                            let Json::Object(fields) = &mut value else {
                                unreachable!()
                            };
                            fields.insert("kind".into(), text("entity"));
                            fields.insert(
                                "table".into(),
                                entry.table.map(text).unwrap_or(Json::Null),
                            );
                            fields.insert(
                                "record_id".into(),
                                entry.record_id.map(text).unwrap_or(Json::Null),
                            );
                        }
                        value
                    })
                    .collect(),
            ),
        ),
    ])
}

fn progress_wire(value: SyncProgress) -> Json {
    object([
        (
            "phase",
            text(match value.phase {
                ProgressPhase::NotStarted => "not_started",
                ProgressPhase::Bootstrapping => "bootstrapping",
                ProgressPhase::Ready => "ready",
            }),
        ),
        ("ready", Json::Bool(value.ready)),
        ("pages", text(value.pages)),
        ("changes", text(value.changes)),
        ("tombstones", text(value.tombstones)),
    ])
}

enum WireTarget {
    Command(String),
    Entity {
        table: String,
        record_id: String,
        write_id: String,
    },
}

impl WireTarget {
    fn key(&self) -> &str {
        match self {
            Self::Command(key) => key,
            Self::Entity {
                write_id,
                ..
            } => write_id,
        }
    }
    fn receive(
        self,
        db: &mut CommandDatabase<SqliteStorage>,
        outcome: Option<RemoteOutcome>,
    ) -> std::result::Result<(), CommandError> {
        match self {
            Self::Command(key) => db.receive(&key, outcome),
            Self::Entity {
                table,
                record_id,
                write_id,
            } => db.receive_entity(&table, &record_id, &write_id, outcome),
        }
    }
}

fn http_outcomes(body: &str) -> Result<Vec<(WireTarget, Option<RemoteOutcome>)>> {
    let value = Json::parse(body).map_err(|_| Error("InvalidPayload"))?;
    array(&value)?
        .iter()
        .map(|value| {
            let f = map(value)?;
            let (target, mut keys) = if f.contains_key("writeId") {
                (
                    WireTarget::Entity {
                        table: string(field(f, "table")?)?.into(),
                        record_id: string(field(f, "recordId")?)?.into(),
                        write_id: string(field(f, "writeId")?)?.into(),
                    },
                    vec!["table", "recordId", "writeId", "status"],
                )
            } else {
                (
                    WireTarget::Command(string(field(f, "idempotencyKey")?)?.into()),
                    vec!["idempotencyKey", "status"],
                )
            };
            let outcome = match string(field(f, "status")?)? {
                "applied" => {
                    keys.push("result");
                    Some(RemoteOutcome::Applied(field(f, "result")?.clone()))
                }
                "corrected" => {
                    keys.push("result");
                    Some(RemoteOutcome::Corrected(field(f, "result")?.clone()))
                }
                "rejected" => {
                    keys.push("reason");
                    Some(RemoteOutcome::Rejected(string(field(f, "reason")?)?.into()))
                }
                "acknowledged" if matches!(&target, WireTarget::Entity { .. }) => None,
                _ => return Err(Error("InvalidRequest")),
            };
            exact(value, &keys)?;
            Ok((target, outcome))
        })
        .collect()
}

fn schema(version: u32) -> SchemaDefinition {
    let required = |kind| Column {
        kind,
        optional: false,
    };
    let mut orders = TableDefinition {
        columns: BTreeMap::from([
            ("label".into(), required(ColumnType::String)),
            ("total".into(), required(ColumnType::Integer)),
            ("ratio".into(), required(ColumnType::Float)),
            ("active".into(), required(ColumnType::Boolean)),
            ("placed_at".into(), required(ColumnType::Timestamp)),
            (
                "notes".into(),
                Column {
                    kind: ColumnType::String,
                    optional: true,
                },
            ),
        ]),
        relations: BTreeMap::from([
            (
                "customer".into(),
                RelationDefinition::ToOne {
                    table: "customers".into(),
                    foreign_key: "customer_id".into(),
                    optional: true,
                },
            ),
            (
                "items".into(),
                RelationDefinition::HasMany {
                    table: "items".into(),
                    foreign_key: "order_id".into(),
                },
            ),
        ]),
    };
    if version == 2 {
        orders.columns.insert(
            "extra".into(),
            Column {
                kind: ColumnType::String,
                optional: true,
            },
        );
    }
    SchemaDefinition {
        version,
        tables: BTreeMap::from([
            (
                "customers".into(),
                TableDefinition {
                    columns: BTreeMap::from([("country".into(), required(ColumnType::String))]),
                    relations: BTreeMap::from([(
                        "orders".into(),
                        RelationDefinition::HasMany {
                            table: "orders".into(),
                            foreign_key: "customer_id".into(),
                        },
                    )]),
                },
            ),
            ("orders".into(), orders),
            (
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([
                        ("label".into(), required(ColumnType::String)),
                        ("quantity".into(), required(ColumnType::Integer)),
                    ]),
                    relations: BTreeMap::from([(
                        "order".into(),
                        RelationDefinition::ToOne {
                            table: "orders".into(),
                            foreign_key: "order_id".into(),
                            optional: false,
                        },
                    )]),
                },
            ),
        ]),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    Data(Subscription),
    Sync(SyncSubscription),
}

struct Owner {
    path: String,
    db: CommandDatabase<SqliteStorage>,
    protection: StorageProtection,
    requests: BTreeMap<u64, Option<CommandRequest>>,
    subscriptions: BTreeMap<u64, Token>,
    events: Option<VecDeque<Json>>,
    push: Option<PendingPush>,
    context: Arc<Mutex<Option<EntityContext>>>,
    pulls: BTreeMap<u64, entity_host::PendingPull>,
}

struct PendingPush {
    envelopes: VecDeque<(String, Json)>,
    next_due_ms: Option<i64>,
}

// Test-only delivery ownership: select complete values without consuming a
// rejected value. An empty Some queue preserves a cancelled snapshot boundary.
fn prefix<T>(
    queue: &mut VecDeque<T>,
    wire: impl Fn(&T) -> Json,
    frame: impl Fn(Vec<Json>, bool) -> Json,
) -> Result<Json> {
    let mut selected = vec![];
    for item in queue.iter() {
        selected.push(wire(item));
        if budget::success(&frame(selected.clone(), queue.len() > selected.len())).is_err() {
            selected.pop();
            if selected.is_empty() {
                return Err(Error("LimitExceeded"));
            }
            break;
        }
    }
    let count = selected.len();
    let result = frame(selected, queue.len() > count);
    budget::success(&result)?;
    for _ in 0..count {
        queue.pop_front();
    }
    Ok(result)
}

impl Owner {
    fn request(&self, value: &Json) -> Result<CommandRequest> {
        self.requests
            .get(&handle(value)?)
            .copied()
            .flatten()
            .ok_or(Error("InvalidHandle"))
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn allocate() -> Result<u64> {
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1)
    })
    .map_err(|_| Error("LimitExceeded"))
}

pub struct CommandFixture {
    owners: BTreeMap<u64, Owner>,
    #[cfg(not(target_arch = "wasm32"))]
    root: Result<std::path::PathBuf>,
}

impl Default for CommandFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandFixture {
    pub fn new() -> Self {
        Self {
            owners: BTreeMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            root: std::env::var("HOYADB_PHASE4_ROOT")
                .map_err(|_| Error("InvalidRequest"))
                .and_then(|root| validate_root(&root)),
        }
    }

    pub fn dispatch(&mut self, input: &str) -> String {
        let result = (|| {
            let input = Json::parse(input).map_err(|_| Error("InvalidPayload"))?;
            if !no_numbers(&input) {
                return Err(Error("InvalidRequest"));
            }
            let frame = exact(&input, &["op", "owner", "args"])?;
            let op = string(field(frame, "op")?)?;
            let args = field(frame, "args")?;
            map(args)?;
            if op == "open" || op == "open_secure" {
                if field(frame, "owner")? != &Json::Null {
                    return Err(Error("InvalidRequest"));
                }
                return self.open(args, op == "open_secure");
            }
            let owner = handle(field(frame, "owner")?)?;
            if op == "close" {
                exact(args, &[])?;
                self.owners.remove(&owner).ok_or(Error("InvalidHandle"))?;
                return Ok(Json::Null);
            }
            let owner = self.owners.get_mut(&owner).ok_or(Error("InvalidHandle"))?;
            operation(owner, op, args)
        })();
        let response = match result {
            Ok(value) => object([("ok", Json::Bool(true)), ("value", value)]),
            Err(error) => object([
                ("ok", Json::Bool(false)),
                ("error", object([("kind", text(error.0))])),
            ]),
        };
        response
            .canonical()
            .unwrap_or_else(|_| "{\"error\":{\"kind\":\"LimitExceeded\"},\"ok\":false}".into())
    }

    fn open(&mut self, args: &Json, protected: bool) -> Result<Json> {
        let names: &[&str] = if protected {
            &[
                "path",
                "namespace",
                "options",
                "version",
                "authored",
                "key_hex",
            ]
        } else {
            &["path", "namespace", "options", "version", "authored"]
        };
        let f = exact(args, names)?;
        let key = if protected {
            let hex = string(field(f, "key_hex")?)?;
            if hex.len() != 64
                || !hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(Error("InvalidRequest"));
            }
            let mut bytes = Zeroizing::new([0; 32]);
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                    .map_err(|_| Error("InvalidRequest"))?;
            }
            Some(EncryptionKey::from_bytes(*bytes))
        } else {
            None
        };
        let path = self.fixture_path(string(field(f, "path")?)?)?;
        if self.owners.values().any(|owner| owner.path == path) {
            return Err(Error("InvalidRequest"));
        }
        let namespace = string(field(f, "namespace")?)?;
        if namespace.len() != 32
            || !namespace
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error("InvalidRequest"));
        }
        let mut seed = [0; 16];
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&namespace[i * 2..i * 2 + 2], 16)
                .map_err(|_| Error("InvalidRequest"))?;
        }
        let version = match string(field(f, "version")?)? {
            "1" => 1,
            "2" => 2,
            _ => return Err(Error("InvalidRequest")),
        };
        let authored = boolean(field(f, "authored")?)?;
        let options = options(field(f, "options")?)?;
        let migrations = if version == 2 && authored {
            vec![Migration {
                from_version: 1,
                to: schema(2),
                steps: vec![MigrationStep::AddColumn {
                    table: "orders".into(),
                    name: "extra".into(),
                    column: Column {
                        kind: ColumnType::String,
                        optional: true,
                    },
                    default: None,
                }],
            }]
        } else {
            vec![]
        };
        let token = allocate()?;
        let store = if protected {
            SqliteStorage::open_with_options(
                &path,
                SqliteOpenOptions {
                    encryption_key: key,
                },
            )?
        } else {
            SqliteStorage::open_unprotected(&path)?
        };
        let protection = store.storage_protection();
        let db = run_once(CommandDatabase::open(
            store,
            schema(version),
            migrations,
            seed,
            options,
        ))?;
        self.owners.insert(
            token,
            Owner {
                path,
                db,
                protection,
                requests: BTreeMap::new(),
                subscriptions: BTreeMap::new(),
                events: None,
                push: None,
                context: Arc::new(Mutex::new(None)),
                pulls: BTreeMap::new(),
            },
        );
        Ok(text(token))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_root(value: &str) -> Result<std::path::PathBuf> {
    let root = std::path::PathBuf::from(value);
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error("InvalidRequest"))?;
    let (pid, nonce) = name
        .strip_prefix("hoya-phase-four-")
        .and_then(|name| name.split_once('-'))
        .ok_or(Error("InvalidRequest"))?;
    let parsed = pid.parse::<u32>().map_err(|_| Error("InvalidRequest"))?;
    if parsed == 0
        || pid != parsed.to_string()
        || nonce.len() != 32
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !root.is_absolute()
    {
        return Err(Error("InvalidRequest"));
    }
    let metadata = std::fs::symlink_metadata(&root).map_err(|_| Error("InvalidRequest"))?;
    let canonical = std::fs::canonicalize(&root).map_err(|_| Error("InvalidRequest"))?;
    let temp = std::fs::canonicalize(std::env::temp_dir()).map_err(|_| Error("InvalidRequest"))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || canonical.to_str() != Some(value)
        || !canonical.starts_with(&temp)
        || canonical == temp
    {
        return Err(Error("InvalidRequest"));
    }
    Ok(canonical)
}

impl CommandFixture {
    fn fixture_path(&self, path: &str) -> Result<String> {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error("InvalidRequest"))?;
        if !name.starts_with("phase-four-")
            || !name.ends_with(".sqlite")
            || path.contains('\0')
            || path.starts_with("file:")
        {
            return Err(Error("InvalidRequest"));
        }

        #[cfg(target_arch = "wasm32")]
        {
            if path != name || path.contains('/') {
                return Err(Error("InvalidRequest"));
            }
            Ok(path.into())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let root = self.root.as_ref().map_err(|error| *error)?;
            if root.join(name).to_str() != Some(path) {
                return Err(Error("InvalidRequest"));
            }
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(Error("InvalidRequest"));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(Error("InvalidRequest")),
            }
            Ok(path.into())
        }
    }
}

fn operation(owner: &mut Owner, op: &str, args: &Json) -> Result<Json> {
    match op {
        "protection" => {
            exact(args, &[])?;
            let protection = owner.protection;
            Ok(object([
                ("encrypted", Json::Bool(protection.encrypted)),
                (
                    "cipher",
                    text(if protection.encrypted {
                        "SQLCipher 4.19.0"
                    } else {
                        "SQLite unprotected"
                    }),
                ),
                (
                    "backup_exclusion",
                    text(if cfg!(target_arch = "wasm32") {
                        "unsupported"
                    } else {
                        "host-managed"
                    }),
                ),
            ]))
        }
        "diagnostics" => {
            exact(args, &[])?;
            let value = owner.db.diagnostics();
            Ok(object([
                ("schema_version", text(value.schema_version)),
                ("pending", text(value.pending)),
                ("sending", text(value.sending)),
                ("awaiting_outcome", text(value.awaiting_outcome)),
                ("live", text(value.live)),
                ("blocked", Json::Bool(value.blocked)),
                ("ready", Json::Bool(value.ready)),
                ("pages", text(value.pages)),
                ("changes", text(value.changes)),
                ("tombstones", text(value.tombstones)),
            ]))
        }
        "request" => {
            exact(args, &[])?;
            let token = allocate()?;
            let request = owner.db.request()?;
            owner.requests.insert(token, Some(request));
            Ok(text(token))
        }
        "cancel_request" => {
            let f = exact(args, &["request"])?;
            let token = handle(field(f, "request")?)?;
            let request = owner.request(field(f, "request")?)?;
            owner.db.cancel_request(request)?;
            owner.requests.insert(token, None);
            Ok(Json::Null)
        }
        "begin" => {
            let f = exact(
                args,
                &["request", "definition", "value", "identity", "now_ms"],
            )?;
            let token = handle(field(f, "request")?)?;
            let request = owner.request(field(f, "request")?)?;
            let definition = definition(field(f, "definition")?)?;
            let value = unbox(field(f, "value")?)?;
            let identity = match field(f, "identity")? {
                Json::Null => None,
                value => Some(unbox(value)?),
            };
            let now = integer(field(f, "now_ms")?)?;
            match run_once(owner.db.begin(request, definition, value, identity, now)) {
                Ok(admitted) => Ok(Json::Bool(admitted)),
                Err(error) => {
                    // Repeating begin on an active ticket is rejected by core,
                    // but does not cancel its existing scope. Keep that handle
                    // addressable for a later finish or explicit abort.
                    if owner.db.scope_args(request).is_err() {
                        owner.requests.insert(token, None);
                    }
                    Err(error)
                }
            }
        }
        "scope_args" => {
            let f = exact(args, &["request"])?;
            Ok(boxed(
                owner.db.scope_args(owner.request(field(f, "request")?)?)?,
            ))
        }
        "scope_context" => {
            let f = exact(args, &["request"])?;
            let ctx = owner
                .db
                .scope_context(owner.request(field(f, "request")?)?)?;
            Ok(object([
                ("identity", boxed(ctx.identity())),
                ("now_ms", text(ctx.now())),
            ]))
        }
        "scope_fetch" => {
            let f = exact(args, &["request", "query"])?;
            let request = owner.request(field(f, "request")?)?;
            let q = query(&owner.db, field(f, "query")?)?;
            Ok(rows_wire(&run_once(owner.db.scope_fetch(request, &q))?))
        }
        "scope_insert" => {
            let f = exact(args, &["request", "table", "fields"])?;
            let request = owner.request(field(f, "request")?)?;
            let fields = row_fields(field(f, "fields")?)?;
            owner.db.scope_args(request)?;
            budget::row(None, &fields)?;
            Ok(row_wire(&run_once(owner.db.scope_insert(
                request,
                string(field(f, "table")?)?,
                fields,
            ))?))
        }
        "scope_replace" => {
            let f = exact(args, &["request", "table", "id", "fields"])?;
            let request = owner.request(field(f, "request")?)?;
            let fields = row_fields(field(f, "fields")?)?;
            let id = string(field(f, "id")?)?;
            owner.db.scope_args(request)?;
            budget::row(Some(id), &fields)?;
            Ok(row_wire(&run_once(owner.db.scope_replace(
                request,
                string(field(f, "table")?)?,
                id,
                fields,
            ))?))
        }
        "scope_remove" => {
            let f = exact(args, &["request", "table", "id"])?;
            let request = owner.request(field(f, "request")?)?;
            Ok(Json::Bool(run_once(owner.db.scope_remove(
                request,
                string(field(f, "table")?)?,
                string(field(f, "id")?)?,
            ))?))
        }
        "scope_batch" => {
            let f = exact(args, &["request", "writes"])?;
            let request = owner.request(field(f, "request")?)?;
            let writes = writes(field(f, "writes")?)?;
            owner.db.scope_args(request)?;
            budget::batch(&writes, |table, id| {
                let q = owner
                    .db
                    .query(table)
                    .where_("id", Operator::Eq, Value::String(id.into()));
                run_once(owner.db.scope_fetch(request, &q))
            })?;
            Ok(changes_wire(&run_once(
                owner.db.scope_apply_batch(request, writes),
            )?))
        }
        "finish" => {
            let f = exact(args, &["request", "result"])?;
            let token = handle(field(f, "request")?)?;
            let request = owner.request(field(f, "request")?)?;
            let result = unbox(field(f, "result")?)?;
            owner.db.scope_args(request)?;
            budget::finish(&result)?;
            let result = run_once(owner.db.finish(request, result));
            owner.requests.insert(token, None);
            let result = result?;
            Ok(object([
                ("key", text(result.key)),
                ("result", boxed(&result.result)),
            ]))
        }
        "abort" => {
            let f = exact(args, &["request"])?;
            let token = handle(field(f, "request")?)?;
            let request = owner.request(field(f, "request")?)?;
            owner.db.abort(request)?;
            owner.requests.insert(token, None);
            Ok(Json::Null)
        }
        "fetch" => {
            let f = exact(args, &["query"])?;
            let q = query(&owner.db, field(f, "query")?)?;
            Ok(rows_wire(&run_once(q.fetch(&owner.db))?))
        }
        "direct_batch" => {
            let f = exact(args, &["writes"])?;
            let writes = writes(field(f, "writes")?)?;
            budget::batch(&writes, |table, id| {
                let q = owner
                    .db
                    .query(table)
                    .where_("id", Operator::Eq, Value::String(id.into()));
                run_once(q.fetch(&owner.db))
            })?;
            Ok(changes_wire(&run_once(owner.db.apply_batch(writes))?))
        }
        "observe_data" => {
            let f = exact(args, &["query", "mode", "columns"])?;
            let q = query(&owner.db, field(f, "query")?)?;
            let columns = array(field(f, "columns")?)?
                .iter()
                .map(|v| string(v).map(str::to_owned))
                .collect::<Result<_>>()?;
            let token = allocate()?;
            let subscription = match string(field(f, "mode")?)? {
                "rows" => run_once(owner.db.observe(&q))?,
                "count" => run_once(owner.db.observe_count(&q))?,
                "columns" => run_once(owner.db.observe_with_columns(&q, columns))?,
                _ => return Err(Error("InvalidRequest")),
            };
            owner.subscriptions.insert(token, Token::Data(subscription));
            Ok(text(token))
        }
        "observe_status" => {
            exact(args, &[])?;
            let token = allocate()?;
            let subscription = owner.db.observe_queue_status()?;
            owner.subscriptions.insert(token, Token::Sync(subscription));
            Ok(text(token))
        }
        "observe_outcome" => {
            let f = exact(args, &["key"])?;
            let token = allocate()?;
            let subscription = owner.db.observe_outcome(string(field(f, "key")?)?)?;
            owner.subscriptions.insert(token, Token::Sync(subscription));
            Ok(text(token))
        }
        "unsubscribe" => {
            let f = exact(args, &["subscription"])?;
            let token = handle(field(f, "subscription")?)?;
            if let Some(events) = &mut owner.events {
                events.retain(|event| {
                    map(event).expect("owned event")["subscription"] != text(token)
                });
            }
            Ok(Json::Bool(match owner.subscriptions.remove(&token) {
                Some(Token::Data(token)) => owner.db.unsubscribe(token),
                Some(Token::Sync(token)) => owner.db.unsubscribe_sync(token),
                None => false,
            }))
        }
        "queue_status" => {
            exact(args, &[])?;
            Ok(status_wire(owner.db.queue_status()))
        }
        "drain" => {
            let f = exact(args, &["now_ms"])?;
            let now = unsigned(field(f, "now_ms")?)?;
            if owner.events.is_none() {
                let mut events = vec![];
                for event in owner.db.drain_events(now) {
                    let (core, kind, value) = match event {
                        CommandEvent::Data(event) => {
                            let value = match event.value {
                                Ok(ObservationValue::Rows(rows)) => {
                                    object([("kind", text("rows")), ("value", rows_wire(&rows))])
                                }
                                Ok(ObservationValue::Count(count)) => {
                                    object([("kind", text("count")), ("value", text(count))])
                                }
                                Err(error) => object([
                                    ("kind", text("error")),
                                    ("value", object([("kind", text(Error::from(error).0))])),
                                ]),
                            };
                            (Token::Data(event.subscription), "data", value)
                        }
                        CommandEvent::Status {
                            subscription,
                            value,
                        } => (Token::Sync(subscription), "status", status_wire(value)),
                        CommandEvent::Outcome {
                            subscription,
                            value,
                        } => (
                            Token::Sync(subscription),
                            "outcome",
                            app_outcome_wire(&value),
                        ),
                        CommandEvent::Progress {
                            subscription,
                            value,
                        } => (Token::Sync(subscription), "progress", progress_wire(value)),
                    };
                    if let Some((token, _)) = owner
                        .subscriptions
                        .iter()
                        .find(|(_, token)| **token == core)
                    {
                        events.push(object([
                            ("subscription", text(token)),
                            ("kind", text(kind)),
                            ("value", value),
                        ]));
                    }
                }
                owner.events = Some(events.into());
            }
            let events = owner.events.as_mut().expect("captured snapshot");
            let page = prefix(events, Clone::clone, |events, has_more| {
                object([
                    ("events", Json::Array(events)),
                    ("has_more", Json::Bool(has_more)),
                ])
            })?;
            if events.is_empty() {
                owner.events = None;
            }
            Ok(page)
        }
        "prepare_push" => {
            let f = exact(args, &["now_ms", "jitter_basis_points"])?;
            let now = integer(field(f, "now_ms")?)?;
            let jitter = u16::try_from(unsigned(field(f, "jitter_basis_points")?)?)
                .map_err(|_| Error("InvalidRequest"))?;
            if owner.push.is_none() {
                let batch = owner.db.prepare_push(now, jitter)?;
                let envelopes = batch
                    .envelopes
                    .into_iter()
                    .map(|envelope| {
                        let decoded =
                            Json::parse(&envelope).map_err(|_| Error("InvalidPayload"))?;
                        let fields = map(&decoded)?;
                        let key = string(
                            fields
                                .get("idempotencyKey")
                                .or_else(|| fields.get("writeId"))
                                .ok_or(Error("InvalidRequest"))?,
                        )?
                        .to_owned();
                        Ok((key, text(envelope)))
                    })
                    .collect::<Result<_>>()?;
                owner.push = Some(PendingPush {
                    envelopes,
                    next_due_ms: batch.next_due_ms,
                });
            }
            let sent = owner
                .db
                .queue_status()
                .commands
                .into_iter()
                .filter(|entry| entry.state == SyncState::Sent)
                .map(|entry| entry.key)
                .collect::<BTreeSet<_>>();
            let pending = owner.push.as_mut().expect("captured push");
            pending.envelopes.retain(|(key, _)| sent.contains(key));
            let due = pending.next_due_ms;
            let page = prefix(
                &mut pending.envelopes,
                |(_, value)| value.clone(),
                |envelopes, has_more| {
                    object([
                        ("envelopes", Json::Array(envelopes)),
                        ("next_due_ms", due.map(text).unwrap_or(Json::Null)),
                        ("has_more", Json::Bool(has_more)),
                    ])
                },
            )?;
            if pending.envelopes.is_empty() {
                owner.push = None;
            }
            Ok(page)
        }

        "receive" => {
            let f = exact(args, &["key", "outcome"])?;
            let outcome = if field(f, "outcome")? == &Json::Null {
                None
            } else {
                Some(outcome(field(f, "outcome")?)?)
            };
            owner.db.receive(string(field(f, "key")?)?, outcome)?;
            Ok(Json::Null)
        }
        "receive_wire" => {
            let f = exact(args, &["body"])?;
            let outcomes = http_outcomes(string(field(f, "body")?)?)?;
            budget::wire_reports(
                &outcomes
                    .iter()
                    .map(|(target, _)| target.key().to_owned())
                    .collect::<Vec<_>>(),
            )?;
            Ok(Json::Array(
                outcomes
                    .into_iter()
                    .map(|(target, outcome)| {
                        let key = target.key().to_owned();
                        match target.receive(&mut owner.db, outcome) {
                            Ok(()) => object([("key", text(key)), ("ok", Json::Bool(true))]),
                            Err(error) => object([
                                ("key", text(key)),
                                ("ok", Json::Bool(false)),
                                ("error", object([("kind", text(Error::from(error).0))])),
                            ]),
                        }
                    })
                    .collect(),
            ))
        }
        "pending_reconciliations" => {
            exact(args, &[])?;
            Ok(Json::Array(
                owner
                    .db
                    .pending_reconciliations()
                    .into_iter()
                    .map(|(key, outcome)| {
                        object([("key", text(key)), ("outcome", outcome_wire(&outcome))])
                    })
                    .collect(),
            ))
        }
        "reconcile" => {
            let f = exact(args, &["key", "plan"])?;
            run_once(
                owner
                    .db
                    .reconcile(string(field(f, "key")?)?, plan(field(f, "plan")?)?),
            )?;
            Ok(Json::Null)
        }
        "rollback" => {
            let f = exact(args, &["key"])?;
            run_once(owner.db.rollback(string(field(f, "key")?)?))?;
            Ok(Json::Null)
        }
        "cancel" => {
            let f = exact(args, &["key"])?;
            run_once(owner.db.cancel(string(field(f, "key")?)?))?;
            Ok(Json::Null)
        }
        "forget" => {
            let f = exact(args, &["key"])?;
            owner.db.forget(string(field(f, "key")?)?)?;
            Ok(Json::Null)
        }
        _ => entity_host::operation(owner, op, args),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use hoyadb_core::command::Json;

    static FIXTURES: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Real {
        fixture: CommandFixture,
        owner: String,
        directory: std::path::PathBuf,
    }

    impl Real {
        fn new() -> Self {
            let directory = std::fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!(
                    "hoya-phase-four-{}-{:032x}",
                    std::process::id(),
                    allocate().unwrap()
                ));
            std::fs::create_dir(&directory).unwrap();
            let mut fixture = CommandFixture::new();
            fixture.root = validate_root(directory.to_str().unwrap());
            let value = object([
                ("op", text("open")),
                ("owner", Json::Null),
                (
                    "args",
                    object([
                        (
                            "path",
                            text(directory.join("phase-four-bounds.sqlite").to_str().unwrap()),
                        ),
                        ("namespace", text("0123456789abcdef0123456789abcdef")),
                        (
                            "options",
                            object([
                                ("capacity", text("1000")),
                                ("history", text("1000")),
                                ("initial_delay_ms", text("1000")),
                                ("multiplier", text("2")),
                                ("max_delay_ms", text("60000")),
                            ]),
                        ),
                        ("version", text("1")),
                        ("authored", Json::Bool(false)),
                    ]),
                ),
            ])
            .canonical()
            .unwrap();
            let reply = Json::parse(&fixture.dispatch(&value)).unwrap();
            let owner = string(&map(&reply).unwrap()["value"]).unwrap().to_owned();
            Self {
                fixture,
                owner,
                directory,
            }
        }

        fn db(&mut self) -> &mut CommandDatabase<SqliteStorage> {
            &mut self
                .fixture
                .owners
                .get_mut(&self.owner.parse().unwrap())
                .unwrap()
                .db
        }

        fn raw(&mut self, op: &str, args: Json) -> Json {
            let input = object([
                ("op", text(op)),
                ("owner", text(&self.owner)),
                ("args", args),
            ])
            .canonical()
            .unwrap();
            Json::parse(&self.fixture.dispatch(&input)).unwrap()
        }

        fn ok(&mut self, op: &str, args: Json) -> Json {
            let reply = self.raw(op, args);
            assert_eq!(map(&reply).unwrap()["ok"], Json::Bool(true), "{reply:?}");
            map(&reply).unwrap()["value"].clone()
        }

        fn enqueue(&mut self, args_label: &str, row_label: Option<&str>) -> String {
            let db = self.db();
            let request = db.request().unwrap();
            let definition = CommandDefinition {
                name: "bounds-v1".into(),
                arguments: BTreeMap::from([(
                    "label".into(),
                    Argument {
                        kind: ArgumentType::String,
                        optional: false,
                    },
                )]),
                reference_paths: vec![],
            };
            assert!(
                run_once(db.begin(
                    request,
                    definition,
                    object([("label", text(args_label))]),
                    Some(Json::Null),
                    1000
                ))
                .unwrap()
            );
            if let Some(label) = row_label {
                run_once(db.scope_insert(request, "orders", blob(label))).unwrap();
            }
            run_once(db.finish(request, Json::Null)).unwrap().key
        }

        fn rows(&mut self) -> Vec<Row> {
            let db = self.db();
            run_once(db.query("orders").fetch(db)).unwrap()
        }
    }

    impl Drop for Real {
        fn drop(&mut self) {
            self.fixture.owners.clear();
            std::fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    fn blob(label: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("label".into(), Value::String(label.into())),
            ("total".into(), Value::Integer(1)),
            (
                "ratio".into(),
                Value::Float(FiniteFloat::new(1.25).unwrap()),
            ),
            ("active".into(), Value::Boolean(true)),
            ("placed_at".into(), Value::Timestamp(1000)),
        ])
    }

    fn orders_query() -> Json {
        object([
            ("table", text("orders")),
            ("predicates", Json::Array(vec![])),
            ("order", Json::Null),
            ("limit", Json::Null),
        ])
    }

    fn entity_config(real: &mut Real) {
        real.ok(
            "entity_configure",
            object([
                ("scope", text("synthetic:phase-five")),
                ("tables", Json::Array(vec![text("customers")])),
            ]),
        );
    }

    fn entity_context(writes: Json) -> Json {
        object([
            ("writes", writes),
            ("identity", boxed(&Json::Null)),
            ("now_ms", text("1000")),
        ])
    }

    fn customer_insert(country: &str) -> Json {
        object([
            ("op", text("insert")),
            ("table", text("customers")),
            ("fields", object([("country", boxed(&text(country)))])),
        ])
    }

    fn upload(real: &mut Real, request: &Json, page: &PullPage) {
        let body = page.to_json().unwrap().canonical().unwrap();
        real.ok(
            "pull_chunk",
            object([
                ("request", request.clone()),
                ("offset", text("0")),
                ("chunk", text(body)),
            ]),
        );
    }

    fn customer_page(checkpoint: &str, complete: bool, country: &str) -> PullPage {
        PullPage {
            checkpoint: checkpoint.into(),
            complete,
            changes: vec![EntityChange {
                table: "customers".into(),
                record_id: "server:customer".into(),
                op: EntityOperation::Create,
                changes: BTreeMap::from([("country".into(), Value::String(country.into()))]),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 1000,
                field_timestamps: BTreeMap::from([("country".into(), 1000)]),
            }],
        }
    }

    #[test]
    fn e01_e03_shared_entity_mutations_generate_and_correlate_real_mixed_envelopes() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        entity_config(&mut real);
        let changes = real.ok(
            "entity_batch",
            entity_context(Json::Array(vec![customer_insert("東京\0🌿")])),
        );
        let id = string(&map(&array(&changes).unwrap()[0]).unwrap()["id"])
            .unwrap()
            .to_owned();
        assert!(id.starts_with("entity:0123456789abcdef0123456789abcdef:"));
        let row = real.ok(
            "entity_patch",
            object([
                ("table", text("customers")),
                ("id", text(&id)),
                ("fields", object([("country", boxed(&text("changed")))])),
                ("identity", boxed(&Json::Null)),
                ("now_ms", text("1001")),
            ]),
        );
        assert_eq!(
            map(&map(&row).unwrap()["fields"]).unwrap()["country"],
            boxed(&text("changed"))
        );
        let command = real.enqueue("mixed command", None);
        let batch = real.ok(
            "prepare_push",
            object([("now_ms", text("1001")), ("jitter_basis_points", text("0"))]),
        );
        let envelopes = array(&map(&batch).unwrap()["envelopes"]).unwrap();
        assert_eq!(
            envelopes.len(),
            2,
            "unresolved same-entity patch waits for producer"
        );
        let entity = envelopes
            .iter()
            .map(|v| Json::parse(string(v).unwrap()).unwrap())
            .find(|v| map(v).unwrap().contains_key("writeId"))
            .unwrap();
        let entity = map(&entity).unwrap();
        assert!(!entity.contains_key("idempotencyKey"));
        let reports = real.ok(
            "receive_wire",
            object([(
                "body",
                text(
                    Json::Array(vec![
                        object([
                            ("idempotencyKey", text(command)),
                            ("status", text("applied")),
                            ("result", Json::Null),
                        ]),
                        object([
                            ("table", entity["table"].clone()),
                            ("recordId", entity["recordId"].clone()),
                            ("writeId", entity["writeId"].clone()),
                            ("status", text("applied")),
                            ("result", object([("id", text(&id))])),
                        ]),
                    ])
                    .canonical()
                    .unwrap(),
                ),
            )]),
        );
        assert!(
            array(&reports)
                .unwrap()
                .iter()
                .all(|report| map(report).unwrap()["ok"] == Json::Bool(true))
        );
        assert_eq!(real.db().queue_status().awaiting_outcome, 0);
    }

    #[test]
    fn e13_e18_shared_chunked_pull_progress_and_reset_invalidate_checked_handles() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        entity_config(&mut real);
        real.ok("observe_progress", object([]));
        let batch = real.ok("begin_pull", object([]));
        assert_eq!(map(&batch).unwrap()["since"], Json::Null);
        let request = map(&batch).unwrap()["request"].clone();
        let invalid = real.raw(
            "pull_chunk",
            object([
                ("request", request.clone()),
                ("offset", text("1")),
                ("chunk", text("bad")),
            ]),
        );
        assert_eq!(map(&invalid).unwrap()["ok"], Json::Bool(false));
        upload(
            &mut real,
            &request,
            &customer_page("opaque checkpoint / 1", false, "first"),
        );
        real.ok("apply_pull", object([("request", request.clone())]));
        let progress = real.ok("sync_progress", object([]));
        assert_eq!(map(&progress).unwrap()["phase"], text("bootstrapping"));
        assert_eq!(map(&progress).unwrap()["pages"], text("1"));
        let next = real.ok("begin_pull", object([]));
        assert_eq!(map(&next).unwrap()["since"], text("opaque checkpoint / 1"));
        let stale = map(&next).unwrap()["request"].clone();
        upload(
            &mut real,
            &stale,
            &customer_page("opaque checkpoint / 2", true, "second"),
        );
        real.ok("reset", object([("mode", text("preserve"))]));
        assert_eq!(
            map(&real.raw("apply_pull", object([("request", stale)]))).unwrap()["ok"],
            Json::Bool(false)
        );
        let events = real.ok("drain", object([("now_ms", text("2000"))]));
        assert_eq!(map(&events).unwrap()["events"], Json::Array(vec![]));
        assert_eq!(
            map(&real.ok("sync_progress", object([]))).unwrap()["pages"],
            text("0")
        );
        assert_eq!(
            map(&real.ok("begin_pull", object([]))).unwrap()["since"],
            Json::Null
        );
    }

    #[test]
    fn e16_e17_shared_reset_preserves_recovery_or_explicitly_discards_it() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        entity_config(&mut real);
        real.ok(
            "entity_batch",
            entity_context(Json::Array(vec![customer_insert("queued")])),
        );
        assert_eq!(
            map(&real.raw("reset", object([("mode", text("preserve"))]))).unwrap()["ok"],
            Json::Bool(false)
        );
        assert_eq!(real.db().queue_status().live, 1);
        real.ok("reset", object([("mode", text("discard"))]));
        assert_eq!(real.db().queue_status().live, 0);
        let db = real.db();
        assert!(
            run_once(db.query("customers").fetch(db))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn e04_http_receipt_is_entity_only_and_exactly_correlated() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        let legacy = real.raw(
            "receive_wire",
            object([(
                "body",
                text("[{\"idempotencyKey\":\"unknown\",\"status\":\"acknowledged\"}]"),
            )]),
        );
        assert_eq!(
            map(&legacy).unwrap()["ok"],
            Json::Bool(false),
            "legacy HTTP grammar still rejects receipt-only commands before member dispatch"
        );
        entity_config(&mut real);
        real.ok(
            "entity_batch",
            entity_context(Json::Array(vec![customer_insert("receipt")])),
        );
        let batch = real.ok(
            "prepare_push",
            object([("now_ms", text("1000")), ("jitter_basis_points", text("0"))]),
        );
        let envelope =
            Json::parse(string(&array(&map(&batch).unwrap()["envelopes"]).unwrap()[0]).unwrap())
                .unwrap();
        let fields = map(&envelope).unwrap();
        let receipt = object([
            ("table", fields["table"].clone()),
            ("recordId", fields["recordId"].clone()),
            ("writeId", fields["writeId"].clone()),
            ("status", text("acknowledged")),
        ]);
        let reports = real.ok(
            "receive_wire",
            object([(
                "body",
                text(Json::Array(vec![receipt]).canonical().unwrap()),
            )]),
        );
        assert_eq!(
            map(&array(&reports).unwrap()[0]).unwrap()["ok"],
            Json::Bool(true)
        );
        let retry = real.ok(
            "prepare_push",
            object([
                ("now_ms", text("60000")),
                ("jitter_basis_points", text("0")),
            ]),
        );
        assert_eq!(map(&retry).unwrap()["envelopes"], Json::Array(vec![]));
        assert_eq!(real.db().queue_status().awaiting_outcome, 1);
    }

    #[test]
    fn e06_shared_typed_correction_uses_authoritative_clock_before_later_pull() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        entity_config(&mut real);
        let changes = real.ok(
            "entity_batch",
            entity_context(Json::Array(vec![customer_insert("optimistic1000")])),
        );
        let id = string(&map(&array(&changes).unwrap()[0]).unwrap()["id"])
            .unwrap()
            .to_owned();
        let batch = real.ok(
            "prepare_push",
            object([("now_ms", text("1000")), ("jitter_basis_points", text("0"))]),
        );
        let envelope =
            Json::parse(string(&array(&map(&batch).unwrap()["envelopes"]).unwrap()[0]).unwrap())
                .unwrap();
        let write = map(&envelope).unwrap()["writeId"].clone();
        real.ok(
            "receive_entity",
            object([
                ("table", text("customers")),
                ("record_id", text(&id)),
                ("write_id", write.clone()),
                (
                    "outcome",
                    object([("state", text("corrected")), ("result", boxed(&Json::Null))]),
                ),
            ]),
        );
        let mut page = customer_page("correction-wrapper", true, "authoritative25");
        let change = &mut page.changes[0];
        change.record_id = id.clone();
        change.op = EntityOperation::Update;
        change.timestamp_ms = 25;
        change.field_timestamps.insert("country".into(), 25);
        let encoded = page.to_json().unwrap();
        let change = array(&map(&encoded).unwrap()["changes"]).unwrap()[0].clone();
        real.ok(
            "reconcile",
            object([
                ("key", write),
                (
                    "plan",
                    object([
                        (
                            "writes",
                            Json::Array(vec![object([("op", text("entity")), ("change", change)])]),
                        ),
                        (
                            "mappings",
                            Json::Object(BTreeMap::from([(id.clone(), text(&id))])),
                        ),
                    ]),
                ),
            ]),
        );
        let mut later = customer_page("after-correction", true, "later30");
        let change = &mut later.changes[0];
        change.record_id = id.clone();
        change.op = EntityOperation::Update;
        change.timestamp_ms = 30;
        change.field_timestamps.insert("country".into(), 30);
        let request = map(&real.ok("begin_pull", object([]))).unwrap()["request"].clone();
        upload(&mut real, &request, &later);
        real.ok("apply_pull", object([("request", request)]));
        let db = real.db();
        let row = run_once(
            db.query("customers")
                .where_("id", Operator::Eq, Value::String(id))
                .fetch(db),
        )
        .unwrap();
        assert_eq!(
            row[0].fields["country"],
            Value::String("later30".into()),
            "explicit clock25 must not be overwritten by optimistic clock1000 promotion"
        );
    }

    #[test]
    fn e15_chunk_bounds_and_cross_owner_handles_leave_progress_unchanged() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut a = Real::new();
        let mut b = Real::new();
        entity_config(&mut a);
        entity_config(&mut b);
        let request = map(&a.ok("begin_pull", object([]))).unwrap()["request"].clone();
        assert_eq!(
            map(&b.raw("apply_pull", object([("request", request.clone())]))).unwrap()["ok"],
            Json::Bool(false)
        );
        let too_large = a.raw(
            "pull_chunk",
            object([
                ("request", request.clone()),
                ("offset", text("0")),
                ("chunk", text("x".repeat(262145))),
            ]),
        );
        assert_eq!(map(&too_large).unwrap()["ok"], Json::Bool(false));
        for index in 0..4 {
            a.ok(
                "pull_chunk",
                object([
                    ("request", request.clone()),
                    ("offset", text(index * 262144)),
                    ("chunk", text(" ".repeat(262144))),
                ]),
            );
        }
        assert_eq!(
            map(&a.raw(
                "pull_chunk",
                object([
                    ("request", request.clone()),
                    ("offset", text("1048576")),
                    ("chunk", text(" "))
                ])
            ))
            .unwrap()["ok"],
            Json::Bool(false)
        );
        assert_eq!(
            map(&a.raw("apply_pull", object([("request", request.clone())]))).unwrap()["ok"],
            Json::Bool(false)
        );
        assert_eq!(a.db().sync_progress().pages, 0);
        assert_eq!(b.db().sync_progress().pages, 0);
        a.ok("abort_pull", object([("request", request)]));
        let request = map(&a.ok("begin_pull", object([]))).unwrap()["request"].clone();
        upload(
            &mut a,
            &request,
            &customer_page("valid-after-abort", true, "valid"),
        );
        a.ok("apply_pull", object([("request", request)]));
        assert_eq!(a.db().sync_progress().pages, 1);
        assert_eq!(b.db().sync_progress().pages, 0);
    }

    fn call(fixture: &mut CommandFixture, op: &str, owner: &str, args: Json) -> Json {
        let request = object([("op", text(op)), ("owner", text(owner)), ("args", args)])
            .canonical()
            .unwrap();
        let response = Json::parse(&fixture.dispatch(&request)).unwrap();
        let fields = map(&response).unwrap();
        assert_eq!(fields["ok"], Json::Bool(true), "{response:?}");
        fields["value"].clone()
    }

    #[test]
    fn s01_s10_secure_rpc_validates_key_before_creation_and_reports_protection() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut host = Real::new();
        let path = host.directory.join("phase-four-security.sqlite");
        let frame = |key: Json| {
            object([
                ("op", text("open_secure")),
                ("owner", Json::Null),
                (
                    "args",
                    object([
                        ("path", text(path.to_str().unwrap())),
                        ("namespace", text("0123456789abcdef0123456789abcdef")),
                        (
                            "options",
                            object([
                                ("capacity", text("1000")),
                                ("history", text("1000")),
                                ("initial_delay_ms", text("1000")),
                                ("multiplier", text("2")),
                                ("max_delay_ms", text("60000")),
                            ]),
                        ),
                        ("version", text("1")),
                        ("authored", Json::Bool(false)),
                        ("key_hex", key),
                    ]),
                ),
            ])
            .canonical()
            .unwrap()
        };
        for invalid in [
            text(""),
            text("1".repeat(63)),
            text("1".repeat(65)),
            text("AA".repeat(32)),
            text("gg".repeat(32)),
            Json::Null,
        ] {
            let reply = Json::parse(&host.fixture.dispatch(&frame(invalid))).unwrap();
            assert_eq!(map(&reply).unwrap()["ok"], Json::Bool(false));
            assert!(
                !path.exists(),
                "invalid key must fail before filesystem creation"
            );
        }
        let reply = Json::parse(&host.fixture.dispatch(&frame(text("11".repeat(32))))).unwrap();
        let fields = map(&reply).unwrap();
        assert_eq!(
            fields["ok"],
            Json::Bool(true),
            "valid supplied key must open protected store"
        );
        host.owner = string(&fields["value"]).unwrap().into();
        let capability = host.ok("protection", object([]));
        let fields = exact(&capability, &["encrypted", "cipher", "backup_exclusion"]).unwrap();
        assert_eq!(fields["encrypted"], Json::Bool(true));
        assert_eq!(fields["cipher"], text("SQLCipher 4.19.0"));
        assert_eq!(fields["backup_exclusion"], text("host-managed"));
        host.enqueue(
            "protected-rpc-args-canary",
            Some("protected-rpc-row-canary"),
        );
        host.ok("close", object([]));
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.starts_with(b"SQLite format 3"));
        for canary in [
            b"protected-rpc-args-canary".as_slice(),
            b"protected-rpc-row-canary".as_slice(),
        ] {
            assert!(!raw.windows(canary.len()).any(|window| window == canary));
        }
    }

    #[test]
    fn s12_diagnostics_rpc_has_only_bounded_allowlisted_state() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut host = Real::new();
        let key = host.enqueue("private-diagnostic-canary", Some("private-row-canary"));
        let value = host.ok("diagnostics", object([]));
        let fields = exact(
            &value,
            &[
                "schema_version",
                "pending",
                "sending",
                "awaiting_outcome",
                "live",
                "blocked",
                "ready",
                "pages",
                "changes",
                "tombstones",
            ],
        )
        .unwrap();
        assert_eq!(fields["schema_version"], text("1"));
        assert_eq!(fields["pending"], text("1"));
        assert_eq!(fields["sending"], text("0"));
        assert_eq!(fields["awaiting_outcome"], text("0"));
        assert_eq!(fields["live"], text("1"));
        assert_eq!(fields["blocked"], Json::Bool(false));
        assert_eq!(fields["ready"], Json::Bool(false));
        for count in ["pages", "changes", "tombstones"] {
            assert_eq!(fields[count], text("0"));
        }
        let wire = value.canonical().unwrap();
        for forbidden in [
            &key,
            "private-diagnostic-canary",
            "private-row-canary",
            "orders",
            "namespace",
            "checkpoint",
            "identity",
        ] {
            assert!(!wire.contains(forbidden));
        }
        assert_eq!(
            map(&host.raw("diagnostics", object([("extra", Json::Null)]))).unwrap()["ok"],
            Json::Bool(false)
        );
        let batch = host.db().prepare_push(1000, 0).unwrap();
        assert!(
            batch.envelopes[0].contains("private-diagnostic-canary"),
            "explicit retry data stays complete"
        );
    }

    #[test]
    fn h03_tagged_values_preserve_all_numbers_and_reject_unsafe_coercion() {
        for value in [
            Json::Integer(i64::MIN),
            Json::Integer(i64::MAX),
            Json::Float(FiniteFloat::new(42.0).unwrap()),
            Json::Float(FiniteFloat::new(f64::MAX).unwrap()),
            Json::Float(FiniteFloat::new(f64::from_bits(1)).unwrap()),
            Json::String("東京\0😀".into()),
            Json::Object(BTreeMap::from([(
                "nested".into(),
                Json::Array(vec![Json::Bool(false), Json::Null]),
            )])),
        ] {
            assert_eq!(unbox(&boxed(&value)).unwrap(), value);
            assert!(no_numbers(&boxed(&value)));
        }
        for value in [
            object([
                ("kind", text("integer")),
                ("value", Json::Integer(9_007_199_254_740_992)),
            ]),
            object([
                ("kind", text("integer")),
                ("value", text("9223372036854775808")),
            ]),
            object([("kind", text("integer")), ("value", text("1.0"))]),
            object([("kind", text("float")), ("value", text("NaN"))]),
        ] {
            assert!(unbox(&value).is_err());
        }
    }

    #[test]
    fn h01_h03_shared_rpc_opens_real_owner_and_rejects_unsafe_wire() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "hoya-phase-four-{}-{}",
                std::process::id(),
                "0".repeat(32)
            ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory
            .join("phase-four-rpc.sqlite")
            .to_str()
            .unwrap()
            .to_owned();
        let mut fixture = CommandFixture::new();
        fixture.root = validate_root(directory.to_str().unwrap());
        let request = format!(
            r#"{{"op":"open","owner":null,"args":{{"path":{},"namespace":"0123456789abcdef0123456789abcdef","options":{{"capacity":"1000","history":"100","initial_delay_ms":"1000","multiplier":"2","max_delay_ms":"60000"}},"version":"1","authored":false}}}}"#,
            Json::String(path).canonical().unwrap()
        );
        let reply = Json::parse(&fixture.dispatch(&request)).unwrap();
        let Json::Object(reply) = reply else {
            panic!("object reply required")
        };
        assert_eq!(reply["ok"], Json::Bool(true));
        let Json::String(owner) = &reply["value"] else {
            panic!("owner string required")
        };
        let request = format!(r#"{{"op":"request","owner":"{owner}","args":{{}}}}"#);
        let reply = Json::parse(&fixture.dispatch(&request)).unwrap();
        let Json::Object(reply) = reply else {
            panic!("object reply required")
        };
        assert_eq!(reply["ok"], Json::Bool(true));
        let request_id = string(&reply["value"]).unwrap().to_owned();
        let definition = object([
            ("name", text("probe-v1")),
            (
                "arguments",
                object([(
                    "amount",
                    object([("kind", text("Integer")), ("optional", Json::Bool(false))]),
                )]),
            ),
            ("reference_paths", Json::Array(vec![])),
        ]);
        let values = Json::Object(BTreeMap::from([("amount".into(), Json::Integer(i64::MAX))]));
        let admitted = call(
            &mut fixture,
            "begin",
            owner,
            object([
                ("request", text(&request_id)),
                ("definition", definition.clone()),
                ("value", boxed(&values)),
                ("identity", boxed(&Json::Null)),
                ("now_ms", text("100")),
            ]),
        );
        assert_eq!(admitted, Json::Bool(true));
        // A repeated begin is invalid, but must not retire an already-active
        // request: its staging remains addressable and can still abort/finish.
        let duplicate = object([
            ("op", text("begin")),
            ("owner", text(owner)),
            (
                "args",
                object([
                    ("request", text(&request_id)),
                    ("definition", definition.clone()),
                    ("value", boxed(&values)),
                    ("identity", boxed(&Json::Null)),
                    ("now_ms", text("100")),
                ]),
            ),
        ])
        .canonical()
        .unwrap();
        assert!(fixture.dispatch(&duplicate).contains("InvalidTicket"));
        assert_eq!(
            call(
                &mut fixture,
                "scope_args",
                owner,
                object([("request", text(&request_id))])
            ),
            boxed(&values)
        );
        call(
            &mut fixture,
            "abort",
            owner,
            object([("request", text(&request_id))]),
        );
        let request_id = string(&call(&mut fixture, "request", owner, object([])))
            .unwrap()
            .to_owned();
        assert_eq!(
            call(
                &mut fixture,
                "begin",
                owner,
                object([
                    ("request", text(&request_id)),
                    ("definition", definition),
                    ("value", boxed(&values)),
                    ("identity", boxed(&Json::Null)),
                    ("now_ms", text("100")),
                ])
            ),
            Json::Bool(true)
        );

        let fields = Json::Object(BTreeMap::from([
            ("label".into(), cell_wire(&Value::String("東京\0😀".into()))),
            ("total".into(), cell_wire(&Value::Integer(i64::MAX))),
            (
                "ratio".into(),
                cell_wire(&Value::Float(FiniteFloat::new(42.0).unwrap())),
            ),
            ("active".into(), cell_wire(&Value::Boolean(false))),
            ("placed_at".into(), cell_wire(&Value::Timestamp(i64::MIN))),
        ]));
        let inserted = call(
            &mut fixture,
            "scope_insert",
            owner,
            object([
                ("request", text(&request_id)),
                ("table", text("orders")),
                ("fields", fields.clone()),
            ]),
        );
        assert_eq!(map(&inserted).unwrap()["fields"], fields);
        let query = object([
            ("table", text("orders")),
            ("predicates", Json::Array(vec![])),
            ("order", Json::Null),
            ("limit", Json::Null),
        ]);
        assert_eq!(
            call(
                &mut fixture,
                "fetch",
                owner,
                object([("query", query.clone())])
            ),
            Json::Array(vec![])
        );
        assert_eq!(
            call(
                &mut fixture,
                "scope_fetch",
                owner,
                object([("request", text(&request_id)), ("query", query.clone())])
            ),
            Json::Array(vec![inserted.clone()])
        );
        let result = call(
            &mut fixture,
            "finish",
            owner,
            object([
                ("request", text(&request_id)),
                ("result", boxed(&Json::Null)),
            ]),
        );
        let key = string(&map(&result).unwrap()["key"]).unwrap().to_owned();
        assert_eq!(
            call(&mut fixture, "fetch", owner, object([("query", query)])),
            Json::Array(vec![inserted])
        );
        let push = call(
            &mut fixture,
            "prepare_push",
            owner,
            object([("now_ms", text("100")), ("jitter_basis_points", text("0"))]),
        );
        let envelopes = array(&map(&push).unwrap()["envelopes"]).unwrap();
        assert_eq!(envelopes.len(), 1);
        assert!(
            string(&envelopes[0])
                .unwrap()
                .contains("9223372036854775807")
        );
        let body = format!(
            "[{{\"idempotencyKey\":\"{key}\",\"status\":\"applied\",\"result\":-9223372036854775808}}]"
        );
        call(
            &mut fixture,
            "receive_wire",
            owner,
            object([("body", text(body))]),
        );
        let pending = call(&mut fixture, "pending_reconciliations", owner, object([]));
        let pending = array(&pending).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            map(&pending[0]).unwrap()["outcome"],
            object([
                ("state", text("applied")),
                ("result", boxed(&Json::Integer(i64::MIN)))
            ])
        );
        let unsafe_wire =
            format!(r#"{{"op":"begin","owner":"{owner}","args":{{"request":9007199254740992}}}}"#);
        let reply = Json::parse(&fixture.dispatch(&unsafe_wire)).unwrap();
        let Json::Object(reply) = reply else {
            panic!("object reply required")
        };
        assert_eq!(reply["ok"], Json::Bool(false));
        let reply = fixture.dispatch(&format!(
            r#"{{"op":"close","owner":"{owner}","args":{{}}}}"#
        ));
        assert!(reply.contains("\"ok\":true"));
        drop(fixture);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bounded_finish_rejects_before_commit_and_retains_explicit_abort() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        let request = real.ok("request", object([]));
        assert_eq!(
            real.ok(
                "begin",
                object([
                    ("request", request.clone()),
                    (
                        "definition",
                        object([
                            ("name", text("result-v1")),
                            ("arguments", object([])),
                            ("reference_paths", Json::Array(vec![]))
                        ])
                    ),
                    ("value", boxed(&object([]))),
                    ("identity", boxed(&Json::Null)),
                    ("now_ms", text("1000"))
                ])
            ),
            Json::Bool(true)
        );
        let token = real.fixture.owners[&real.owner.parse().unwrap()]
            .request(&request)
            .unwrap();
        run_once(real.db().scope_insert(token, "orders", blob("staged"))).unwrap();
        let frame = |result: &str| {
            object([
                ("op", text("finish")),
                ("owner", text(&real.owner)),
                (
                    "args",
                    object([
                        ("request", request.clone()),
                        ("result", boxed(&text(result))),
                    ]),
                ),
            ])
            .canonical()
            .unwrap()
        };
        let padding = "x".repeat(1_048_576 - frame("").len());
        let input = frame(&padding);
        assert_eq!(input.len(), 1_048_576);
        let reply = real.fixture.dispatch(&input);
        assert!(reply.contains("LimitExceeded"));
        assert!(
            real.rows().is_empty(),
            "a rejected reply must not hide committed rows"
        );
        assert_eq!(
            real.db().queue_status().live,
            0,
            "a rejected reply must not hide an enqueued command"
        );
        assert!(
            real.db().scope_args(token).is_ok(),
            "preflight retains staging for explicit abort"
        );
        real.ok("abort", object([("request", request)]));
        assert_eq!(real.enqueue("small", Some("after abort")).len(), 66);
        assert_eq!(
            real.rows()[0].id,
            "local:1",
            "failed finish cannot advance the committed counter"
        );
    }

    #[test]
    fn bounded_drain_preserves_complete_events_and_snapshot_cancellation() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        let label = "x".repeat(300000);
        real.enqueue("row", Some(&label));
        let mut tokens = vec![];
        for _ in 0..4 {
            tokens.push(real.ok(
                "observe_data",
                object([
                    ("query", orders_query()),
                    ("mode", text("rows")),
                    ("columns", Json::Array(vec![])),
                ]),
            ));
        }
        let mut events = vec![];
        let mut pages = 0;
        loop {
            let page = real.ok("drain", object([("now_ms", text("1000"))]));
            let page = map(&page).unwrap();
            events.extend(array(&page["events"]).unwrap().iter().cloned());
            pages += 1;
            if page["has_more"] == Json::Bool(false) {
                break;
            }
        }
        assert!(pages > 1);
        assert_eq!(events.len(), 4);
        for (event, token) in events.iter().zip(&tokens) {
            assert_eq!(map(event).unwrap()["subscription"], *token);
            let data = &map(event).unwrap()["value"];
            let row = &array(&map(data).unwrap()["value"]).unwrap()[0];
            assert_eq!(
                map(&map(row).unwrap()["fields"]).unwrap()["label"],
                boxed(&text(&label))
            );
        }
        // A later snapshot's last page is emptied by cancellation. Continuing it
        // must not consume newly registered core events from another snapshot.
        for token in tokens {
            real.ok("unsubscribe", object([("subscription", token)]));
        }
        let mut current = vec![];
        for _ in 0..4 {
            current.push(real.ok(
                "observe_data",
                object([
                    ("query", orders_query()),
                    ("mode", text("rows")),
                    ("columns", Json::Array(vec![])),
                ]),
            ));
        }
        let first = real.ok("drain", object([("now_ms", text("1001"))]));
        assert_eq!(map(&first).unwrap()["has_more"], Json::Bool(true));
        for token in current {
            real.ok("unsubscribe", object([("subscription", token)]));
        }
        let count = real.ok(
            "observe_data",
            object([
                ("query", orders_query()),
                ("mode", text("count")),
                ("columns", Json::Array(vec![])),
            ]),
        );
        let continuation = real.ok("drain", object([("now_ms", text("1002"))]));
        assert_eq!(map(&continuation).unwrap()["events"], Json::Array(vec![]));
        assert_eq!(map(&continuation).unwrap()["has_more"], Json::Bool(false));
        let next = real.ok("drain", object([("now_ms", text("1003"))]));
        assert_eq!(array(&map(&next).unwrap()["events"]).unwrap().len(), 1);
        real.ok("unsubscribe", object([("subscription", count)]));
        // One oversized row snapshot stays retained until its token is removed.
        for _ in 0..3 {
            real.enqueue("more", Some(&label));
        }
        let token = real.ok(
            "observe_data",
            object([
                ("query", orders_query()),
                ("mode", text("rows")),
                ("columns", Json::Array(vec![])),
            ]),
        );
        for _ in 0..2 {
            assert!(
                real.raw("drain", object([("now_ms", text("1010"))]))
                    .canonical()
                    .unwrap()
                    .contains("LimitExceeded")
            );
        }
        real.ok("unsubscribe", object([("subscription", token)]));
        let after = real.ok("drain", object([("now_ms", text("1011"))]));
        assert_eq!(map(&after).unwrap()["events"], Json::Array(vec![]));
    }

    #[test]
    fn bounded_push_preserves_frozen_order_deadline_and_excludes_cancelled_cache() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        let label = "x".repeat(280000);
        for _ in 0..4 {
            real.enqueue(&label, None);
        }
        let expected = real.db().prepare_push(1000, 0).unwrap().envelopes;
        let mut actual = vec![];
        let mut pages = 0;
        loop {
            let page = real.ok(
                "prepare_push",
                object([
                    (
                        "now_ms",
                        text(if pages == 0 {
                            "2000"
                        } else {
                            "9000"
                        }),
                    ),
                    ("jitter_basis_points", text("0")),
                ]),
            );
            let page = map(&page).unwrap();
            assert_eq!(page["next_due_ms"], text("4000"));
            actual.extend(
                array(&page["envelopes"])
                    .unwrap()
                    .iter()
                    .map(|value| string(value).unwrap().to_owned()),
            );
            pages += 1;
            if page["has_more"] == Json::Bool(false) {
                break;
            }
        }
        assert!(pages > 1);
        assert_eq!(actual, expected);
        assert_eq!(real.db().queue_status().sending, 4);
        let first = real.ok(
            "prepare_push",
            object([("now_ms", text("4000")), ("jitter_basis_points", text("0"))]),
        );
        assert_eq!(map(&first).unwrap()["has_more"], Json::Bool(true));
        let key = Json::parse(expected.last().unwrap()).unwrap();
        let key = string(&map(&key).unwrap()["idempotencyKey"])
            .unwrap()
            .to_owned();
        run_once(real.db().cancel(&key)).unwrap();
        let last = real.ok(
            "prepare_push",
            object([("now_ms", text("4000")), ("jitter_basis_points", text("0"))]),
        );
        assert_eq!(map(&last).unwrap()["envelopes"], Json::Array(vec![]));
        assert_eq!(map(&last).unwrap()["has_more"], Json::Bool(false));
    }

    #[test]
    fn bounded_before_image_batch_rejects_before_any_application_mutation() {
        let _guard = FIXTURES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut real = Real::new();
        let label = "x".repeat(300000);
        for _ in 0..4 {
            real.enqueue("row", Some(&label));
        }
        let rows = real.rows();
        let writes = Json::Array(
            rows.iter()
                .map(|row| {
                    object([
                        ("op", text("remove")),
                        ("table", text("orders")),
                        ("id", text(&row.id)),
                    ])
                })
                .collect(),
        );
        let error = real.raw("direct_batch", object([("writes", writes)]));
        assert!(error.canonical().unwrap().contains("LimitExceeded"));
        assert!(
            real.rows() == rows,
            "oversized before-images cannot be removed before a failed reply"
        );
    }
}
