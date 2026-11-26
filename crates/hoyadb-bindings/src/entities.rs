//! API-0011: one-shot captured entity contexts and owned pull/merge handles.
use crate::{codec::*, data::*, *};
use hoyadb_core::{
    command::*,
    model::{RelationDefinition, Write},
    storage::{Operator, Row, Value},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct State {
    context: Arc<Mutex<Option<EntityContext>>>,
    pulls: BTreeMap<u64, PullRequest>,
    merges: BTreeMap<u64, (MergeSession, u64)>,
}
struct ContextGuard(Arc<Mutex<Option<EntityContext>>>);
impl Drop for ContextGuard {
    fn drop(&mut self) {
        if let Ok(mut value) = self.0.lock() {
            *value = None;
        }
    }
}
fn context(value: &Json) -> Result<EntityContext> {
    let f = exact(value, &["identity", "now_ms"])?;
    Ok(EntityContext {
        identity: unbox(&f["identity"])?,
        now_ms: integer(&f["now_ms"])?,
    })
}
fn names(value: &Json) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for name in array(value)? {
        if !result.insert(string(name)?.to_owned()) {
            return Err(Error("InvalidRequest"));
        }
    }
    Ok(result)
}
fn config(value: &Json) -> Result<EntityConfig> {
    let f = exact(value, &["scope", "tables"])?;
    Ok(EntityConfig {
        scope: string(&f["scope"])?.into(),
        tables: names(&f["tables"])?,
    })
}

fn aliased_writes(value: &Json) -> Result<Vec<AliasedWrite>> {
    array(value)?
        .iter()
        .map(|value| {
            let fields = map(value)?;
            let op = string(fields.get("op").ok_or(Error("InvalidRequest"))?)?;
            let keys: &[&str] = match op {
                "insert" => &[
                    "op",
                    "table",
                    "fields",
                    "proposed_id",
                    "alias",
                    "references",
                ],
                "replace" | "patch" => &["op", "table", "id", "fields", "references"],
                "remove" => &["op", "table", "id"],
                _ => return Err(Error("InvalidRequest")),
            };
            exact(value, keys)?;
            let alias = fields.get("alias").map(proposed).transpose()?.flatten();
            let references = fields
                .get("references")
                .map(|value| {
                    map(value)?
                        .iter()
                        .map(|(field, alias)| Ok((field.clone(), string(alias)?.into())))
                        .collect::<Result<_>>()
                })
                .transpose()?
                .unwrap_or_default();
            let mut base: BTreeMap<_, _> = fields
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "alias" | "references"))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            if op == "patch" {
                base.insert("op".into(), text("replace"));
            }
            let base = Json::Object(base);
            let (write, proposed_id) = writes(&Json::Array(vec![base]))?.remove(0);
            Ok(AliasedWrite {
                write,
                patch: op == "patch",
                proposed_id,
                alias,
                references,
            })
        })
        .collect()
}
fn row(value: &Json) -> Result<Option<Row>> {
    if value == &Json::Null {
        return Ok(None);
    }
    let f = exact(value, &["id", "fields"])?;
    Ok(Some(Row {
        id: string(&f["id"])?.into(),
        fields: fields(&f["fields"])?,
    }))
}
fn metadata(value: &RowMetadata) -> Json {
    let stamp = |v: &FieldStamp| {
        object([
            ("timestamp_ms", text(v.timestamp_ms)),
            ("authoritative", Json::Bool(v.authoritative)),
        ])
    };
    object([
        (
            "fields",
            Json::Object(
                value
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), stamp(v)))
                    .collect(),
            ),
        ),
        (
            "tombstone",
            value.tombstone.as_ref().map(stamp).unwrap_or(Json::Null),
        ),
    ])
}
fn incoming(value: &EntityChange) -> Result<Json> {
    let json = PullPage {
        checkpoint: String::new(),
        complete: true,
        changes: vec![value.clone()],
    }
    .to_json()?;
    Ok(array(&map(&json)?["changes"])?[0].clone())
}
fn step(value: MergeStep, external: u64) -> Json {
    match value {
        MergeStep::Offer {
            step,
            table,
            record_id,
            ..
        } => object([
            ("session", text(external)),
            ("step", text(step)),
            ("kind", text("offer")),
            ("table", text(table)),
            ("record_id", text(record_id)),
        ]),
        MergeStep::Ready {
            step,
            ..
        } => object([
            ("session", text(external)),
            ("step", text(step)),
            ("kind", text("ready")),
        ]),
    }
}
impl Owner {
    fn set_context(&mut self, value: &Json) -> Result<ContextGuard> {
        let value = context(value)?;
        *self
            .entities
            .context
            .lock()
            .map_err(|_| Error("Protocol"))? = Some(value);
        Ok(ContextGuard(self.entities.context.clone()))
    }
    fn pull(&self, value: &Json) -> Result<PullRequest> {
        self.entities
            .pulls
            .get(&handle(value)?)
            .copied()
            .ok_or(Error("InvalidHandle"))
    }
    fn merge(&self, value: &Json) -> Result<(MergeSession, u64)> {
        self.entities
            .merges
            .get(&handle(value)?)
            .copied()
            .ok_or(Error("InvalidHandle"))
    }
    fn retire_merge(&mut self, id: u64) {
        if let Some((session, pull)) = self.entities.merges.remove(&id) {
            let _ = self.db.merge_abort(session);
            self.entities.pulls.remove(&pull);
        }
    }
    fn fetch_id(&self, table: &str, id: &str) -> Result<Vec<Row>> {
        let query = self
            .db
            .query(table)
            .where_("id", Operator::Eq, Value::String(id.into()));
        run(query.fetch(&self.db))
    }
    // These are response-size bounds only. Core still normalizes IDs, validates
    // relations and performs the sole allocation/write/merge transaction.
    fn budget_link(
        &self,
        (table, id, relation, target): (&str, &str, &str, &str),
        fields: &BTreeMap<String, Value>,
        proposed: Option<&str>,
        request: Option<CommandRequest>,
    ) -> Result<()> {
        let definition = self.db.schema();
        let Some(RelationDefinition::ManyToMany {
            through,
            source_relation,
            target_relation,
            ..
        }) = definition
            .tables
            .get(table)
            .and_then(|t| t.relations.get(relation))
        else {
            return Err(Error("InvalidQuery"));
        };
        let through_definition = definition
            .tables
            .get(through)
            .ok_or(Error("InvalidQuery"))?;
        let fk = |relation: &str| match through_definition.relations.get(relation) {
            Some(RelationDefinition::ToOne {
                foreign_key,
                ..
            }) => Ok(foreign_key.clone()),
            _ => Err(Error("InvalidQuery")),
        };
        let (source, destination, mut bound) = if request.is_none() {
            (
                self.db.normalize_entity_target(through, id)?,
                self.db.normalize_entity_target(through, target)?,
                self.db.normalize_entity_fields(through, fields.clone())?,
            )
        } else {
            (id.to_owned(), target.to_owned(), fields.clone())
        };
        let source_key = fk(source_relation)?;
        let target_key = fk(target_relation)?;
        bound.insert(source_key.clone(), Value::Reference(source.clone()));
        bound.insert(target_key.clone(), Value::Reference(destination.clone()));
        budget::row(proposed, &bound)?;
        let query = self
            .db
            .query(through)
            .where_(&source_key, Operator::Eq, Value::Reference(source))
            .where_(&target_key, Operator::Eq, Value::Reference(destination));
        let existing = match request {
            Some(request) => run(self.db.scope_fetch(request, &query))?,
            None => run(query.fetch(&self.db))?,
        };
        for row in existing {
            budget::row(Some(&row.id), &row.fields)?;
        }
        Ok(())
    }
    fn normalized_entity_batch(
        &self,
        writes: &[(Write, Option<String>)],
    ) -> Result<Vec<(Write, Option<String>)>> {
        writes
            .iter()
            .map(|(write, proposed)| {
                let write = match write {
                    Write::Insert {
                        table,
                        fields,
                    } => Write::Insert {
                        table: table.clone(),
                        fields: self.db.normalize_entity_fields(table, fields.clone())?,
                    },
                    Write::Replace {
                        table,
                        id,
                        fields,
                    } => Write::Replace {
                        table: table.clone(),
                        id: self.db.normalize_entity_target(table, id)?,
                        fields: self.db.normalize_entity_fields(table, fields.clone())?,
                    },
                    Write::Remove {
                        table,
                        id,
                    } => Write::Remove {
                        table: table.clone(),
                        id: self.db.normalize_entity_target(table, id)?,
                    },
                };
                Ok((write, proposed.clone()))
            })
            .collect::<Result<Vec<_>>>()
    }
    fn budget_entity_batch(&self, writes: &[(Write, Option<String>)]) -> Result<()> {
        let normalized = self.normalized_entity_batch(writes)?;
        budget::batch(&normalized, |table, id| self.fetch_id(table, id))
    }
    pub(crate) fn entity_call(&mut self, op: &str, args: &Json) -> Result<Json> {
        match op {
            "entity_configure" | "entity_configure_durable" | "entity_reconfigure" => {
                let f = exact(args, &["config"])?;
                let config = config(&f["config"])?;
                let slot = self.entities.context.clone();
                let provider = move || {
                    slot.lock()
                        .map_err(|_| CommandError::Protocol)?
                        .take()
                        .ok_or(CommandError::InvalidCommand)
                };
                if op == "entity_configure" {
                    self.db.configure_entities(config, provider)?;
                } else if op == "entity_configure_durable" {
                    self.db.configure_entities_durable(config, provider)?;
                } else {
                    run(self.db.reconfigure_entities(config, provider))?;
                    self.entities.pulls.clear();
                    self.entities.merges.clear();
                }
                Ok(Json::Null)
            }
            "resolve_entity_id" => {
                let f = exact(args, &["table", "id"])?;
                Ok(text(self.db.resolve_entity_id(
                    string(&f["table"])?,
                    string(&f["id"])?,
                )?))
            }
            "entity_batch" => {
                let f = exact(args, &["writes", "context"])?;
                let writes = writes(&f["writes"])?;
                self.budget_entity_batch(&writes)?;
                let _context = self.set_context(&f["context"])?;
                Ok(changes_wire(&run(self.db.apply_batch_proposed(writes))?))
            }
            "entity_batch_aliased" => {
                let f = exact(args, &["writes", "context"])?;
                let writes = aliased_writes(&f["writes"])?;
                // Conservative complete-reply sizing only: core resolves actual aliases,
                // validates their graph/schema and owns the sole ID allocation/commit.
                let maximum = "entity:ffffffffffffffffffffffffffffffff:ffffffffffffffff";
                let mut aliases = BTreeMap::new();
                let mut bounded = Vec::new();
                let mut lower_bound = budget::AliasedReplyLowerBound::default();
                let mut known: BTreeMap<(String, String), Option<Row>> = BTreeMap::new();
                for item in &writes {
                    if let Some(alias) = &item.alias {
                        aliases.insert(alias.clone(), maximum.into());
                    }
                    let mut write = item.write.clone();
                    if let Write::Insert {
                        fields,
                        ..
                    }
                    | Write::Replace {
                        fields,
                        ..
                    } = &mut write
                    {
                        for field in item.references.keys() {
                            fields.insert(field.clone(), Value::Reference(maximum.into()));
                        }
                    }
                    let mut normalized =
                        self.normalized_entity_batch(&[(write, item.proposed_id.clone())])?;
                    let (mut write, proposed) = normalized.remove(0);
                    if let Write::Replace {
                        table,
                        id,
                        fields,
                    } = &mut write
                        && item.patch
                    {
                        let current = match known.get(&(table.clone(), id.clone())) {
                            Some(value) => value.clone(),
                            None => self.fetch_id(table, id)?.into_iter().next(),
                        }
                        .ok_or(Error("RowNotFound"))?;
                        let mut replacement = current.fields;
                        replacement.extend(fields.clone());
                        *fields = replacement;
                    }
                    lower_bound.after(&write, proposed.as_deref())?;
                    match &write {
                        Write::Insert {
                            table,
                            fields,
                        } => {
                            if let Some(id) = &proposed {
                                known.insert(
                                    (table.clone(), id.clone()),
                                    Some(Row {
                                        id: id.clone(),
                                        fields: fields.clone(),
                                    }),
                                );
                            }
                        }
                        Write::Replace {
                            table,
                            id,
                            fields,
                        } => {
                            known.insert(
                                (table.clone(), id.clone()),
                                Some(Row {
                                    id: id.clone(),
                                    fields: fields.clone(),
                                }),
                            );
                        }
                        Write::Remove {
                            table,
                            id,
                        } => {
                            known.insert((table.clone(), id.clone()), None);
                        }
                    }
                    bounded.push((write, proposed));
                }
                budget::aliased_batch(&bounded, |table, id| self.fetch_id(table, id), &aliases)?;
                let _context = self.set_context(&f["context"])?;
                let result = run(self.db.apply_entity_batch_aliased(writes))?;
                Ok(object([
                    ("changes", changes_wire(&result.changes)),
                    (
                        "aliases",
                        Json::Object(
                            result
                                .aliases
                                .into_iter()
                                .map(|(alias, id)| (alias, text(id)))
                                .collect(),
                        ),
                    ),
                ]))
            }
            "patch" => {
                let f = exact(args, &["table", "id", "fields", "context"])?;
                let table = string(&f["table"])?;
                let id = string(&f["id"])?;
                let fields = fields(&f["fields"])?;
                let normalized_id = self.db.normalize_entity_target(table, id)?;
                let normalized_fields = self.db.normalize_entity_fields(table, fields.clone())?;
                let before = self.fetch_id(table, &normalized_id)?.into_iter().next();
                let mut bound = before.map(|row| row.fields).unwrap_or_default();
                bound.extend(normalized_fields);
                budget::row(Some(&normalized_id), &bound)?;
                let _context = self.set_context(&f["context"])?;
                Ok(row_wire(&run(self.db.patch(table, id, fields))?))
            }
            "link" | "scope_link" | "entity_link" => {
                let f = match op {
                    "scope_link" => exact(
                        args,
                        &[
                            "request",
                            "table",
                            "id",
                            "relation",
                            "target",
                            "fields",
                            "proposed_id",
                        ],
                    ),
                    "entity_link" => exact(
                        args,
                        &["table", "id", "relation", "target", "fields", "context"],
                    ),
                    _ => exact(
                        args,
                        &["table", "id", "relation", "target", "fields", "proposed_id"],
                    ),
                }?;
                let request = if op == "scope_link" {
                    Some(
                        *self
                            .requests
                            .get(&handle(&f["request"])?)
                            .ok_or(Error("InvalidHandle"))?,
                    )
                } else {
                    None
                };
                if let Some(request) = request {
                    self.db.scope_args(request)?;
                }
                let table = string(&f["table"])?;
                let id = string(&f["id"])?;
                let relation = string(&f["relation"])?;
                let target = string(&f["target"])?;
                let fields = fields(&f["fields"])?;
                let proposed = if op == "entity_link" {
                    None
                } else {
                    proposed(&f["proposed_id"])?
                };
                self.budget_link(
                    (table, id, relation, target),
                    &fields,
                    proposed.as_deref(),
                    request,
                )?;
                let _context = if op == "entity_link" {
                    Some(self.set_context(&f["context"])?)
                } else {
                    None
                };
                let row = match request {
                    Some(request) => run(self.db.scope_link_proposed(
                        request,
                        table,
                        id,
                        relation,
                        target,
                        (fields, proposed),
                    ))?,
                    None => run(self
                        .db
                        .link_proposed(table, id, relation, target, fields, proposed))?,
                };
                Ok(row_wire(&row))
            }
            "unlink" | "scope_unlink" | "entity_unlink" => {
                let f = match op {
                    "scope_unlink" => {
                        exact(args, &["request", "table", "id", "relation", "target"])
                    }
                    "entity_unlink" => {
                        exact(args, &["table", "id", "relation", "target", "context"])
                    }
                    _ => exact(args, &["table", "id", "relation", "target"]),
                }?;
                let _context = if op == "entity_unlink" {
                    Some(self.set_context(&f["context"])?)
                } else {
                    None
                };
                let table = string(&f["table"])?;
                let id = string(&f["id"])?;
                let relation = string(&f["relation"])?;
                let target = string(&f["target"])?;
                let count = if op == "scope_unlink" {
                    let request = *self
                        .requests
                        .get(&handle(&f["request"])?)
                        .ok_or(Error("InvalidHandle"))?;
                    run(self.db.scope_unlink(request, table, id, relation, target))?
                } else {
                    run(self.db.unlink(table, id, relation, target))?
                };
                Ok(text(count))
            }
            "sync_progress" => {
                exact(args, &[])?;
                Ok(observations::progress_wire(self.db.sync_progress()))
            }
            "begin_pull" => {
                exact(args, &[])?;
                let id = allocate()?;
                let batch = self.db.begin_pull()?;
                let value = object([
                    ("request", text(id)),
                    ("since", batch.since.map(text).unwrap_or(Json::Null)),
                    ("scope", text(batch.scope)),
                    ("generation", text(batch.generation)),
                ]);
                if let Err(error) = budget::success(&value) {
                    self.db.abort_pull(batch.request)?;
                    return Err(error);
                }
                self.entities.pulls.insert(id, batch.request);
                Ok(value)
            }
            "apply_pull" => {
                let f = exact(args, &["request", "page"])?;
                let id = handle(&f["request"])?;
                let request = self.pull(&f["request"])?;
                let page = PullPage::from_json(&f["page"])?;
                let result = run(self.db.apply_pull(request, page));
                if result.is_ok() || matches!(result, Err(Error("StalePull"))) {
                    self.entities.pulls.remove(&id);
                    self.entities.merges.retain(|_, (_, pull)| *pull != id);
                }
                result?;
                Ok(Json::Null)
            }
            "abort_pull" => {
                let f = exact(args, &["request"])?;
                let id = handle(&f["request"])?;
                self.db.abort_pull(self.pull(&f["request"])?)?;
                self.entities.pulls.remove(&id);
                self.entities.merges.retain(|_, (_, pull)| *pull != id);
                Ok(Json::Null)
            }
            "merge_prepare" => {
                let f = exact(args, &["request", "page", "tables"])?;
                let pull = handle(&f["request"])?;
                let request = self.pull(&f["request"])?;
                let page = PullPage::from_json(&f["page"])?;
                let tables = names(&f["tables"])?;
                let id = allocate()?;
                let value = match run(self.db.merge_prepare(request, page, tables)) {
                    Ok(value) => value,
                    Err(error) => {
                        if matches!(error, Error("StalePull")) {
                            self.entities.pulls.remove(&pull);
                        }
                        return Err(error);
                    }
                };
                let session = match &value {
                    MergeStep::Offer {
                        session,
                        ..
                    }
                    | MergeStep::Ready {
                        session,
                        ..
                    } => *session,
                };
                self.entities.merges.insert(id, (session, pull));
                let value = step(value, id);
                if let Err(error) = budget::success(&value) {
                    self.retire_merge(id);
                    return Err(error);
                }
                Ok(value)
            }
            "merge_input" => {
                let f = exact(args, &["session", "step", "part"])?;
                let id = handle(&f["session"])?;
                let (session, _) = self.merge(&f["session"])?;
                let value = self.db.merge_input(session, size(&f["step"])? as u64)?;
                let value = match string(&f["part"])? {
                    "current" => value.current.as_ref().map(row_wire).unwrap_or(Json::Null),
                    "metadata" => metadata(&value.metadata),
                    "incoming" => incoming(&value.incoming)?,
                    "default" => object([
                        (
                            "row",
                            value
                                .default
                                .row
                                .as_ref()
                                .map(row_wire)
                                .unwrap_or(Json::Null),
                        ),
                        ("metadata", metadata(&value.default.metadata)),
                    ]),
                    _ => return Err(Error("InvalidRequest")),
                };
                if let Err(error) = budget::success(&value) {
                    self.retire_merge(id);
                    return Err(error);
                }
                Ok(value)
            }
            "merge_reply" => {
                let f = exact(args, &["session", "step", "row"])?;
                let id = handle(&f["session"])?;
                let (session, _) = self.merge(&f["session"])?;
                let index = size(&f["step"])? as u64;
                self.db.merge_input(session, index)?;
                let row = match row(&f["row"]) {
                    Ok(row) => row,
                    Err(error) => {
                        self.retire_merge(id);
                        return Err(error);
                    }
                };
                let result = run(self.db.merge_reply(session, index, row));
                if result.is_err() && self.db.merge_input(session, index).is_err() {
                    self.retire_merge(id);
                }
                let value = step(result?, id);
                if let Err(error) = budget::success(&value) {
                    self.retire_merge(id);
                    return Err(error);
                }
                Ok(value)
            }
            "merge_commit" | "merge_abort" => {
                let f = exact(args, &["session"])?;
                let id = handle(&f["session"])?;
                let (session, pull) = self.merge(&f["session"])?;
                let result = if op == "merge_commit" {
                    run(self.db.merge_commit(session))
                } else {
                    self.db.merge_abort(session).map_err(Into::into)
                };
                if result.is_ok() || matches!(result, Err(Error("StalePull"))) {
                    self.entities.merges.remove(&id);
                    self.entities.pulls.remove(&pull);
                }
                result?;
                Ok(Json::Null)
            }
            "reset" => {
                let f = exact(args, &["mode"])?;
                let mode = match string(&f["mode"])? {
                    "preserve_recovery" => ResetMode::PreserveRecovery,
                    "discard_recovery" => ResetMode::DiscardRecovery,
                    _ => return Err(Error("InvalidRequest")),
                };
                run(self.db.reset(mode))?;
                self.entities.pulls.clear();
                self.entities.merges.clear();
                *self
                    .entities
                    .context
                    .lock()
                    .map_err(|_| Error("Protocol"))? = None;
                self.requests.clear();
                self.push = None;
                self.subscriptions.clear();
                self.events = None;
                Ok(Json::Null)
            }
            _ => self.observation_call(op, args),
        }
    }
}
