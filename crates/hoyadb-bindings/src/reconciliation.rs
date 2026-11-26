//! API-0011: exact transport receipts and plans reuse the durable core queue.
use crate::*;
use hoyadb_core::command::*;

fn plan(value: &Json) -> Result<ReconcilePlan> {
    let f = exact(value, &["writes", "mappings"])?;
    let writes = array(&f["writes"])?
        .iter()
        .map(|v| {
            let f = map(v)?;
            match string(f.get("op").ok_or(Error("InvalidRequest"))?)? {
                "put" => {
                    let f = exact(v, &["op", "table", "id", "fields"])?;
                    Ok(ReconcileWrite::Put {
                        table: string(&f["table"])?.into(),
                        id: string(&f["id"])?.into(),
                        fields: fields(&f["fields"])?,
                    })
                }
                "remove" => {
                    let f = exact(v, &["op", "table", "id"])?;
                    Ok(ReconcileWrite::Remove {
                        table: string(&f["table"])?.into(),
                        id: string(&f["id"])?.into(),
                    })
                }
                "entity" => {
                    let f = exact(v, &["op", "change"])?;
                    let page = PullPage::from_json(&object([
                        ("checkpoint", text("binding-reconcile")),
                        ("complete", Json::Bool(true)),
                        ("changes", Json::Array(vec![f["change"].clone()])),
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
    let mappings = map(&f["mappings"])?
        .iter()
        .map(|(k, v)| Ok((k.clone(), string(v)?.into())))
        .collect::<Result<_>>()?;
    Ok(ReconcilePlan {
        writes,
        mappings,
    })
}

/// SPEC-0039 guards: exactly `[{table, id, expected: Fields|null}...]`.
fn guards(value: &Json) -> Result<Vec<CorrectionGuard>> {
    array(value)?
        .iter()
        .map(|v| {
            let f = exact(v, &["table", "id", "expected"])?;
            Ok(CorrectionGuard {
                table: string(&f["table"])?.into(),
                id: string(&f["id"])?.into(),
                expected: match &f["expected"] {
                    Json::Null => None,
                    fields_value => Some(fields(fields_value)?),
                },
            })
        })
        .collect()
}

fn policy(value: &Json) -> Result<ConflictPolicy> {
    let f = exact(value, &["default", "commands"])?;
    let reaction = |value: &Json| match string(value)? {
        "stop" => Ok(ConflictReaction::Stop),
        "discard" => Ok(ConflictReaction::Discard),
        _ => Err(Error("InvalidRequest")),
    };
    Ok(ConflictPolicy {
        default: reaction(&f["default"])?,
        commands: map(&f["commands"])?
            .iter()
            .map(|(name, value)| Ok((name.clone(), reaction(value)?)))
            .collect::<Result<_>>()?,
    })
}

fn outcome_wire(value: &RemoteOutcome) -> Json {
    match value {
        RemoteOutcome::Applied(v) => object([("status", text("applied")), ("result", boxed(v))]),
        RemoteOutcome::Corrected(v) => {
            object([("status", text("corrected")), ("result", boxed(v))])
        }
        RemoteOutcome::Rejected(v) => object([("status", text("rejected")), ("reason", text(v))]),
    }
}

/// API-0011 §1: a result is admitted only when its tagged form fits alone in
/// the `pending_reconciliations` reply that must later return it. Core Json
/// limits are wider than this binding's tagged frame, so an undeliverable
/// result is refused before the durable receipt instead of blocking that list.
pub(crate) fn deliverable(key: &str, outcome: Option<&RemoteOutcome>) -> Result<()> {
    let Some(outcome) = outcome else {
        return Ok(());
    };
    budget::success(&Json::Array(vec![object([
        ("key", text(key)),
        ("outcome", outcome_wire(outcome)),
    ])]))
}

enum Target {
    Command(String),
    Entity {
        table: String,
        id: String,
        write: String,
    },
}
impl Target {
    fn key(&self) -> &str {
        match self {
            Self::Command(v) => v,
            Self::Entity {
                write,
                ..
            } => write,
        }
    }
    fn receive(
        self,
        owner: &mut Owner,
        outcome: Option<RemoteOutcome>,
    ) -> std::result::Result<(), CommandError> {
        match self {
            Self::Command(key) => owner.db.receive(&key, outcome),
            Self::Entity {
                table,
                id,
                write,
            } => owner.db.receive_entity(&table, &id, &write, outcome),
        }
    }
}

fn wire(body: &str) -> Result<Vec<(Target, Option<RemoteOutcome>)>> {
    let value = Json::parse(body)?;
    array(&value)?
        .iter()
        .map(|v| {
            let f = map(v)?;
            let get = |name: &str| f.get(name).ok_or(Error("InvalidRequest"));
            let (target, mut keys) = if f.contains_key("writeId") {
                (
                    Target::Entity {
                        table: string(get("table")?)?.into(),
                        id: string(get("recordId")?)?.into(),
                        write: string(get("writeId")?)?.into(),
                    },
                    vec!["table", "recordId", "writeId", "status"],
                )
            } else {
                (
                    Target::Command(string(get("idempotencyKey")?)?.into()),
                    vec!["idempotencyKey", "status"],
                )
            };
            let outcome = match string(get("status")?)? {
                "applied" => {
                    keys.push("result");
                    Some(RemoteOutcome::Applied(get("result")?.clone()))
                }
                "corrected" => {
                    keys.push("result");
                    Some(RemoteOutcome::Corrected(get("result")?.clone()))
                }
                "rejected" => {
                    keys.push("reason");
                    Some(RemoteOutcome::Rejected(string(get("reason")?)?.into()))
                }
                "acknowledged" if matches!(target, Target::Entity { .. }) => None,
                _ => return Err(Error("InvalidRequest")),
            };
            exact(v, &keys)?;
            Ok((target, outcome))
        })
        .collect()
}

impl Owner {
    pub(crate) fn reconciliation_call(&mut self, op: &str, args: &Json) -> Result<Json> {
        match op {
            "set_conflict_policy" => {
                let f = exact(args, &["policy"])?;
                self.db.set_conflict_policy(policy(&f["policy"])?)?;
                Ok(Json::Null)
            }
            "reconcile_resolving" => {
                let f = exact(args, &["key", "plan", "guards"])?;
                Ok(run(self.db.reconcile_resolving(
                    string(&f["key"])?,
                    plan(&f["plan"])?,
                    guards(&f["guards"])?,
                ))?
                .to_json())
            }
            "rollback_resolving" | "discard" => {
                let f = exact(args, &["key"])?;
                let key = string(&f["key"])?;
                let result = if op == "discard" {
                    run(self.db.discard(key))
                } else {
                    run(self.db.rollback_resolving(key))
                }?;
                Ok(result.to_json())
            }
            "receive_wire" => {
                let f = exact(args, &["body"])?;
                let outcomes = wire(string(&f["body"])?)?;
                let reports = Json::Array(
                    outcomes
                        .iter()
                        .map(|(target, _)| {
                            object([
                                ("key", text(target.key())),
                                ("ok", Json::Bool(false)),
                                ("error", object([("kind", text("UnknownLocalReference"))])),
                            ])
                        })
                        .collect(),
                );
                budget::success(&reports)?;
                Ok(Json::Array(
                    outcomes
                        .into_iter()
                        .map(|(target, outcome)| {
                            let key = target.key().to_owned();
                            let received = deliverable(&key, outcome.as_ref())
                                .and_then(|()| Ok(target.receive(self, outcome)?));
                            match received {
                                Ok(()) => object([("key", text(key)), ("ok", Json::Bool(true))]),
                                Err(error) => object([
                                    ("key", text(key)),
                                    ("ok", Json::Bool(false)),
                                    ("error", object([("kind", text(error.0))])),
                                ]),
                            }
                        })
                        .collect(),
                ))
            }
            "reconciliation_context" => {
                let f = exact(args, &["key"])?;
                let (kind, created) = self
                    .db
                    .reconciliation_context(string(&f["key"])?)
                    .ok_or(Error("Protocol"))?;
                Ok(object([
                    (
                        "kind",
                        text(match kind {
                            QueuedWriteKind::Command => "command",
                            QueuedWriteKind::Entity => "entity",
                        }),
                    ),
                    (
                        "created",
                        Json::Object(
                            created
                                .into_iter()
                                .map(|(id, table)| (id, text(table)))
                                .collect(),
                        ),
                    ),
                ]))
            }
            "pending_reconciliations" => {
                exact(args, &[])?;
                Ok(Json::Array(
                    self.db
                        .pending_reconciliations()
                        .into_iter()
                        .map(|(key, outcome)| {
                            object([("key", text(key)), ("outcome", outcome_wire(&outcome))])
                        })
                        .collect(),
                ))
            }
            "pending_reconciliations_page" => {
                let f = exact(args, &["after"])?;
                let after = proposed(&f["after"])?;
                let items = self
                    .db
                    .pending_reconciliations()
                    .into_iter()
                    .map(|(key, outcome)| {
                        let entry =
                            object([("key", text(&key)), ("outcome", outcome_wire(&outcome))]);
                        (key, entry)
                    })
                    .collect();
                budget::page(items, after.as_deref(), |entries, has_more| {
                    object([
                        ("entries", Json::Array(entries)),
                        ("has_more", Json::Bool(has_more)),
                    ])
                })
            }
            "reconcile" => {
                let f = exact(args, &["key", "plan"])?;
                run(self.db.reconcile(string(&f["key"])?, plan(&f["plan"])?))?;
                Ok(Json::Null)
            }
            "reconcile_guarded" => {
                let f = exact(args, &["key", "plan", "guards"])?;
                run(self.db.reconcile_guarded(
                    string(&f["key"])?,
                    plan(&f["plan"])?,
                    guards(&f["guards"])?,
                ))?;
                Ok(Json::Null)
            }
            "forget" => {
                let f = exact(args, &["key"])?;
                self.db.forget(string(&f["key"])?)?;
                Ok(Json::Null)
            }
            "rollback" | "cancel" => {
                let f = exact(args, &["key"])?;
                let key = string(&f["key"])?;
                if op == "rollback" {
                    run(self.db.rollback(key))?;
                } else {
                    run(self.db.cancel(key))?;
                }
                Ok(Json::Null)
            }
            _ => self.command_call(op, args),
        }
    }
}
