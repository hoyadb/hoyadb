//! API-0011: host scopes hold checked tickets, never a borrowed Rust engine.
use crate::{codec::*, data::*, *};
use hoyadb_core::{
    command::*,
    storage::{Operator, Value},
};
use std::collections::{BTreeSet, VecDeque};

pub(crate) struct PendingPush {
    envelopes: VecDeque<(String, Json)>,
    due: Option<i64>,
}

fn definition(value: &Json) -> Result<CommandDefinition> {
    let f = exact(value, &["name", "arguments", "reference_paths"])?;
    let arguments = map(&f["arguments"])?
        .iter()
        .map(|(k, v)| {
            let a = exact(v, &["kind", "optional"])?;
            let kind = match string(&a["kind"])? {
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
                k.clone(),
                Argument {
                    kind,
                    optional: boolean(&a["optional"])?,
                },
            ))
        })
        .collect::<Result<_>>()?;
    let reference_paths = array(&f["reference_paths"])?
        .iter()
        .map(|v| {
            array(v)?
                .iter()
                .map(|v| {
                    let f = map(v)?;
                    if f.len() != 1 {
                        return Err(Error("InvalidRequest"));
                    }
                    if let Some(v) = f.get("key") {
                        Ok(PathPart::Key(string(v)?.into()))
                    } else {
                        Ok(PathPart::Index(size(
                            f.get("index").ok_or(Error("InvalidRequest"))?,
                        )?))
                    }
                })
                .collect::<Result<_>>()
        })
        .collect::<Result<_>>()?;
    Ok(CommandDefinition {
        name: string(&f["name"])?.into(),
        arguments,
        reference_paths,
    })
}

pub(crate) fn status_wire(value: QueueStatus) -> Json {
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
                    .map(|e| {
                        object([
                            (
                                "kind",
                                text(match e.kind {
                                    QueuedWriteKind::Command => "command",
                                    QueuedWriteKind::Entity => "entity",
                                }),
                            ),
                            ("table", e.table.map(text).unwrap_or(Json::Null)),
                            ("record_id", e.record_id.map(text).unwrap_or(Json::Null)),
                            ("key", text(e.key)),
                            (
                                "state",
                                text(match e.state {
                                    SyncState::Queued => "queued",
                                    SyncState::Sent => "sent",
                                    SyncState::Acknowledged => "acknowledged",
                                    SyncState::Applied => "applied",
                                    SyncState::Corrected => "corrected",
                                    SyncState::Rejected => "rejected",
                                    SyncState::Cancelled => "cancelled",
                                    SyncState::Discarded => "discarded",
                                }),
                            ),
                            (
                                "blocked_on",
                                Json::Array(e.blocked_on.into_iter().map(text).collect()),
                            ),
                            ("awaiting_local", Json::Bool(e.awaiting_local)),
                            (
                                "awaiting_key",
                                e.awaiting_key.map(text).unwrap_or(Json::Null),
                            ),
                            ("rolled_back", Json::Bool(e.rolled_back)),
                            ("attempts", text(e.attempts)),
                            ("next_due_ms", e.next_due_ms.map(text).unwrap_or(Json::Null)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

pub(crate) fn outcome(value: &Json) -> Result<Option<RemoteOutcome>> {
    if value == &Json::Null {
        return Ok(None);
    }
    let f = map(value)?;
    Ok(Some(
        match string(f.get("status").ok_or(Error("InvalidRequest"))?)? {
            "applied" => {
                let f = exact(value, &["status", "result"])?;
                RemoteOutcome::Applied(unbox(&f["result"])?)
            }
            "corrected" => {
                let f = exact(value, &["status", "result"])?;
                RemoteOutcome::Corrected(unbox(&f["result"])?)
            }
            "rejected" => {
                let f = exact(value, &["status", "reason"])?;
                RemoteOutcome::Rejected(string(&f["reason"])?.into())
            }
            _ => return Err(Error("InvalidRequest")),
        },
    ))
}

pub(crate) fn prefix<T>(
    pending: &mut VecDeque<T>,
    mut encode: impl FnMut(&T) -> Json,
    frame: impl Fn(Vec<Json>, bool) -> Json,
) -> Result<Json> {
    let mut values = Vec::new();
    let mut count = 0;
    for value in pending.iter() {
        values.push(encode(value));
        if budget::success(&frame(values.clone(), count + 1 < pending.len())).is_err() {
            values.pop();
            if count == 0 {
                return Err(Error("LimitExceeded"));
            }
            break;
        }
        count += 1;
    }
    let value = frame(values, count < pending.len());
    budget::success(&value)?;
    pending.drain(..count);
    Ok(value)
}

impl Owner {
    fn request(&self, value: &Json) -> Result<CommandRequest> {
        self.requests
            .get(&handle(value)?)
            .copied()
            .ok_or(Error("InvalidHandle"))
    }

    pub(crate) fn command_call(&mut self, op: &str, args: &Json) -> Result<Json> {
        match op {
            "request" => {
                exact(args, &[])?;
                let id = allocate()?;
                let request = self.db.request()?;
                self.requests.insert(id, request);
                Ok(text(id))
            }
            "cancel_request" => {
                let f = exact(args, &["request"])?;
                let id = handle(&f["request"])?;
                self.db.cancel_request(self.request(&f["request"])?)?;
                self.requests.remove(&id);
                Ok(Json::Null)
            }
            "begin" => {
                let f = exact(args, &["request", "definition", "value", "context"])?;
                let id = handle(&f["request"])?;
                let request = self.request(&f["request"])?;
                let definition = definition(&f["definition"])?;
                let value = unbox(&f["value"])?;
                let context = exact(&f["context"], &["identity", "now_ms"])?;
                let identity = unbox(&context["identity"])?;
                let now = integer(&context["now_ms"])?;
                match run(self
                    .db
                    .begin(request, definition, value, Some(identity), now))
                {
                    Ok(value) => Ok(Json::Bool(value)),
                    Err(error) => {
                        if self.db.scope_args(request).is_err() {
                            self.requests.remove(&id);
                        }
                        Err(error)
                    }
                }
            }
            "scope_args" | "scope_context" => {
                let f = exact(args, &["request"])?;
                let request = self.request(&f["request"])?;
                if op == "scope_args" {
                    Ok(boxed(self.db.scope_args(request)?))
                } else {
                    let c = self.db.scope_context(request)?;
                    Ok(object([
                        ("identity", boxed(c.identity())),
                        ("now_ms", text(c.now())),
                    ]))
                }
            }
            "scope_fetch" => {
                let f = exact(args, &["request", "query"])?;
                let request = self.request(&f["request"])?;
                let q = query(
                    self.db.schema(),
                    &f["query"],
                    |table, source| match source {
                        Some((table, id, relation)) => self.db.related(table, id, relation),
                        None => self.db.query(table),
                    },
                )?;
                Ok(rows_wire(&run(self.db.scope_fetch(request, &q))?))
            }
            "scope_insert" => {
                let f = exact(args, &["request", "table", "fields", "proposed_id"])?;
                let request = self.request(&f["request"])?;
                self.db.scope_args(request)?;
                let fields = fields(&f["fields"])?;
                let id = proposed(&f["proposed_id"])?;
                budget::row(id.as_deref(), &fields)?;
                Ok(row_wire(&run(self.db.scope_insert_proposed(
                    request,
                    string(&f["table"])?,
                    fields,
                    id,
                ))?))
            }
            "scope_replace" => {
                let f = exact(args, &["request", "table", "id", "fields"])?;
                let request = self.request(&f["request"])?;
                self.db.scope_args(request)?;
                let fields = fields(&f["fields"])?;
                let id = string(&f["id"])?;
                budget::row(Some(id), &fields)?;
                Ok(row_wire(&run(self.db.scope_replace(
                    request,
                    string(&f["table"])?,
                    id,
                    fields,
                ))?))
            }
            "scope_remove" => {
                let f = exact(args, &["request", "table", "id"])?;
                let request = self.request(&f["request"])?;
                Ok(Json::Bool(run(self.db.scope_remove(
                    request,
                    string(&f["table"])?,
                    string(&f["id"])?,
                ))?))
            }
            "scope_batch" => {
                let f = exact(args, &["request", "writes"])?;
                let request = self.request(&f["request"])?;
                self.db.scope_args(request)?;
                let writes = writes(&f["writes"])?;
                budget::batch(&writes, |table, id| {
                    let q =
                        self.db
                            .query(table)
                            .where_("id", Operator::Eq, Value::String(id.into()));
                    run(self.db.scope_fetch(request, &q))
                })?;
                Ok(changes_wire(&run(self
                    .db
                    .scope_apply_batch_proposed(request, writes))?))
            }
            "finish" => {
                let f = exact(args, &["request", "result"])?;
                let id = handle(&f["request"])?;
                let request = self.request(&f["request"])?;
                self.db.scope_args(request)?;
                let result = unbox(&f["result"])?;
                budget::success(&object([
                    ("key", text("0".repeat(66))),
                    ("result", boxed(&result)),
                ]))?;
                let result = run(self.db.finish(request, result));
                self.requests.remove(&id);
                let result = result?;
                Ok(object([
                    ("key", text(result.key)),
                    ("result", boxed(&result.result)),
                ]))
            }
            "abort" => {
                let f = exact(args, &["request"])?;
                let id = handle(&f["request"])?;
                self.db.abort(self.request(&f["request"])?)?;
                self.requests.remove(&id);
                Ok(Json::Null)
            }
            "queue_status" => {
                exact(args, &[])?;
                Ok(status_wire(self.db.queue_status()))
            }
            "queue_status_page" => {
                let f = exact(args, &["after"])?;
                let after = proposed(&f["after"])?;
                let Json::Object(mut status) = status_wire(self.db.queue_status()) else {
                    return Err(Error("Protocol"));
                };
                let Some(Json::Array(commands)) = status.remove("commands") else {
                    return Err(Error("Protocol"));
                };
                let items = commands
                    .into_iter()
                    .map(|entry| Ok((string(&map(&entry)?["key"])?.to_owned(), entry)))
                    .collect::<Result<Vec<_>>>()?;
                budget::page(items, after.as_deref(), |commands, has_more| {
                    let mut value = status.clone();
                    value.insert("commands".into(), Json::Array(commands));
                    value.insert("has_more".into(), Json::Bool(has_more));
                    Json::Object(value)
                })
            }
            "prepare_push" => {
                let f = exact(args, &["now_ms", "jitter_basis_points"])?;
                let now = integer(&f["now_ms"])?;
                let jitter = u16::try_from(unsigned(&f["jitter_basis_points"])?)
                    .map_err(|_| Error("InvalidRequest"))?;
                if self.push.is_none() {
                    let batch = self.db.prepare_push(now, jitter)?;
                    let envelopes = batch
                        .envelopes
                        .into_iter()
                        .map(|value| {
                            let decoded = Json::parse(&value)?;
                            let f = map(&decoded)?;
                            let key = string(
                                f.get("idempotencyKey")
                                    .or_else(|| f.get("writeId"))
                                    .ok_or(Error("Protocol"))?,
                            )?
                            .into();
                            Ok((key, text(value)))
                        })
                        .collect::<Result<_>>()?;
                    self.push = Some(PendingPush {
                        envelopes,
                        due: batch.next_due_ms,
                    });
                }
                let sent = self
                    .db
                    .queue_status()
                    .commands
                    .into_iter()
                    .filter(|e| e.state == SyncState::Sent)
                    .map(|e| e.key)
                    .collect::<BTreeSet<_>>();
                let pending = self.push.as_mut().expect("captured batch");
                pending.envelopes.retain(|(key, _)| sent.contains(key));
                let due = pending.due;
                let result = prefix(
                    &mut pending.envelopes,
                    |(_, v)| v.clone(),
                    |envelopes, has_more| {
                        object([
                            ("envelopes", Json::Array(envelopes)),
                            ("next_due_ms", due.map(text).unwrap_or(Json::Null)),
                            ("has_more", Json::Bool(has_more)),
                        ])
                    },
                )?;
                if pending.envelopes.is_empty() {
                    self.push = None;
                }
                Ok(result)
            }
            "request_redelivery" => {
                let f = exact(args, &["key", "now_ms"])?;
                self.db
                    .request_redelivery(string(&f["key"])?, integer(&f["now_ms"])?)?;
                Ok(Json::Null)
            }
            "receive" => {
                let f = exact(args, &["key", "outcome"])?;
                let key = string(&f["key"])?;
                let outcome = outcome(&f["outcome"])?;
                reconciliation::deliverable(key, outcome.as_ref())?;
                self.db.receive(key, outcome)?;
                Ok(Json::Null)
            }
            "receive_entity" => {
                let f = exact(args, &["table", "record_id", "write_id", "outcome"])?;
                let write = string(&f["write_id"])?;
                let outcome = outcome(&f["outcome"])?;
                reconciliation::deliverable(write, outcome.as_ref())?;
                self.db.receive_entity(
                    string(&f["table"])?,
                    string(&f["record_id"])?,
                    write,
                    outcome,
                )?;
                Ok(Json::Null)
            }
            _ => self.entity_call(op, args),
        }
    }
}
