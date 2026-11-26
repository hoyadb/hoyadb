//! API-0009 / TC-0034: configured ordinary writes use the single durable queue.
use super::{
    entity::*,
    execution::StagedScope,
    owner::{initial_guards, normalize_references, utc_time},
    records::*,
    *,
};
use crate::{model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

impl<S: SyncStorage> CommandDatabase<S> {
    /// Resolve an application reference using the configured table's exact core policy.
    pub fn normalize_entity_target(&self, table: &str, id: &str) -> Result<String, CommandError> {
        if self.is_entity_table(table) {
            self.entity_target_in(table, id)
        } else {
            Ok(id.into())
        }
    }
    /// Resolve a retained entity reference without changing rows or sync state.
    /// Durable alias retention is enabled only by `configure_entities_durable`.
    pub fn resolve_entity_id(&self, table: &str, id: &str) -> Result<String, CommandError> {
        if id.is_empty() || !self.db.db.schema.tables.contains_key(table) {
            return Err(CommandError::InvalidCommand);
        }
        self.normalize_entity_target(table, id)
    }
    /// Normalize a candidate's references without reading storage or mutating state.
    pub fn normalize_entity_fields(
        &self,
        table: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<BTreeMap<String, Value>, CommandError> {
        if self.is_entity_table(table) {
            self.entity_fields(table, fields)
        } else {
            Ok(fields)
        }
    }

    pub fn configure_entities(
        &mut self,
        config: EntityConfig,
        provider: impl FnMut() -> Result<EntityContext, CommandError> + Send + 'static,
    ) -> Result<(), CommandError> {
        self.configure_entity_mode(config, provider, false)
    }

    /// Opt in when adopting entity sync; existing format-2/3 ledgers never convert.
    pub fn configure_entities_durable(
        &mut self,
        config: EntityConfig,
        provider: impl FnMut() -> Result<EntityContext, CommandError> + Send + 'static,
    ) -> Result<(), CommandError> {
        self.configure_entity_mode(config, provider, true)
    }

    fn configure_entity_mode(
        &mut self,
        config: EntityConfig,
        provider: impl FnMut() -> Result<EntityContext, CommandError> + Send + 'static,
        durable: bool,
    ) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let entity = EntityLedger::new(config.clone());
        entity.validate(&self.db.db.schema)?;
        if let Some(existing) = &self.ledger.entity {
            // SPEC-0049: either entry verifies the retained mode; neither converts it.
            if existing.config != config || durable != self.aliases.enabled {
                return Err(CommandError::InvalidCommand);
            }
        } else {
            if self.ledger.has_live_recovery() {
                return Err(CommandError::Protocol);
            }
            self.refuse_reserved_local_rows(&config)?;
            self.transact(self.db.db.state.clone(), &permit, |next| {
                next.adopt_entity(entity)?;
                if durable {
                    next.enable_aliases()?;
                }
                Ok(((), vec![], vec![]))
            })?;
        }
        self.entity_provider = Some(Box::new(provider));
        Ok(())
    }

    /// API-0009: `local:` IDs are reserved for command creations. Without live
    /// recovery no retained producer can still map or admit one, so the entity
    /// path would reject every write addressing such a row; adoption refuses it
    /// first. One bounded limit-1 identity-range query per configured table,
    /// polled once: this entry is synchronous, so an adapter whose read suspends
    /// keeps only the later per-write rejection.
    fn refuse_reserved_local_rows(&self, config: &EntityConfig) -> Result<(), CommandError> {
        for table in &config.tables {
            let query = self
                .db
                .query(table)
                .where_("id", Operator::Ge, Value::String("local:".into()))
                .where_("id", Operator::Lt, Value::String("local;".into()))
                .limit(1);
            let fetch = std::pin::pin!(self.db.fetch(&query));
            if let std::task::Poll::Ready(rows) =
                fetch.poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
                && !rows?.is_empty()
            {
                return Err(CommandError::Protocol);
            }
        }
        Ok(())
    }

    pub async fn reconfigure_entities(
        &mut self,
        config: EntityConfig,
        provider: impl FnMut() -> Result<EntityContext, CommandError> + Send + 'static,
    ) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let previous = self
            .ledger
            .entity
            .as_ref()
            .ok_or(CommandError::InvalidCommand)?;
        if previous.generation <= 1
            || !previous.is_pristine()
            || !self.ledger.records.is_empty()
            || !self.aliases.mappings.is_empty()
        {
            return Err(CommandError::Protocol);
        }
        let mut entity = previous.clone();
        entity.config = config;
        entity.validate(&self.db.db.schema)?;
        // SPEC-0037 R6.6: one limit-1 query per table decides emptiness.
        for table in self.db.db.schema.tables.keys() {
            if !self
                .db
                .fetch(&self.db.query(table).limit(1))
                .await?
                .is_empty()
            {
                return Err(CommandError::Protocol);
            }
        }
        self.transact(self.db.db.state.clone(), &permit, |next| {
            next.edit_entity_header(|header| {
                header.config = entity.config;
                Ok(())
            })?;
            Ok(((), vec![], vec![]))
        })?;
        self.entity_provider = Some(Box::new(provider));
        self.pull = None;
        self.merge = None;
        Ok(())
    }

    pub fn set_merge_hook(&mut self, table: &str, hook: MergeHook) -> Result<(), CommandError> {
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        if !self.db.db.schema.tables.contains_key(table) {
            return Err(CommandError::InvalidCommand);
        }
        self.merge_hooks.insert(table.into(), hook);
        Ok(())
    }

    pub async fn patch(
        &mut self,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let mut scope = StagedScope::begin(&self.db.db, permit).await?;
        let id = if self.is_entity_table(table) {
            self.normalize_entity_target(table, id)?
        } else {
            id.into()
        };
        let query = scope.reader(&self.db.db.storage)?.query(table).where_(
            "id",
            Operator::Eq,
            Value::String(id.clone()),
        );
        let before = query
            .fetch(&scope.reader(&self.db.db.storage)?)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| StorageError::RowNotFound {
                table: table.into(),
                id: id.clone(),
            })?;
        let supplied = if self.is_entity_table(table) {
            self.entity_fields(table, fields)?
        } else {
            fields
        };
        let mut replacement = before.fields.clone();
        replacement.extend(supplied.clone());
        let context = self.capture_entity_context(self.is_entity_table(table))?;
        let row = scope
            .enter(&self.db.db.storage)?
            .replace(table, &id, replacement)
            .await?;
        let change = CommittedChange {
            table: table.into(),
            id,
            before: Some(before),
            after: Some(row.clone()),
        };
        if self.is_entity_table(table) && scope.journal()?.is_empty() {
            scope
                .enter(&self.db.db.storage)?
                .journal
                .as_mut()
                .expect("staging journal")
                .push(change.clone());
        }
        let changes = vec![(change, Some(supplied))];
        self.finish_entity_scope(scope, changes, context).await?;
        Ok(row)
    }

    pub(super) async fn entity_replace(
        &mut self,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        let configured = self.is_entity_table(table);
        let id = if configured {
            self.normalize_entity_target(table, id)?
        } else {
            id.into()
        };
        let fields = if configured {
            self.entity_fields(table, fields)?
        } else {
            fields
        };
        self.entity_apply_batch(vec![Write::Replace {
            table: table.into(),
            id: id.clone(),
            fields: fields.clone(),
        }])
        .await?;
        // Inputs were normalized and validated before the single commit; a no-op
        // replacement has no adapter change row, but still returns its value.
        Ok(Row {
            id,
            fields,
        })
    }

    pub(super) async fn entity_apply_batch(
        &mut self,
        writes: Vec<Write>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        self.entity_apply_batch_proposed(writes.into_iter().map(|write| (write, None)).collect())
            .await
    }

    pub(super) async fn entity_apply_batch_proposed(
        &mut self,
        writes: Vec<(Write, Option<String>)>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        Ok(self
            .entity_batch_staged(
                writes
                    .into_iter()
                    .map(|(write, proposed_id)| AliasedWrite {
                        write,
                        patch: false,
                        proposed_id,
                        alias: None,
                        references: BTreeMap::new(),
                    })
                    .collect(),
            )
            .await?
            .changes)
    }

    pub(super) async fn entity_batch_staged(
        &mut self,
        writes: Vec<AliasedWrite>,
    ) -> Result<AliasedBatchResult, CommandError> {
        for item in &writes {
            if item.proposed_id.is_some()
                && match &item.write {
                    Write::Insert {
                        table,
                        ..
                    } => self.is_entity_table(table),
                    _ => true,
                }
            {
                return Err(CommandError::InvalidCommand);
            }
        }
        self.validate_batch_aliases(&writes)?;
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let needs_context = writes.iter().any(|item| {
            self.is_entity_table(match &item.write {
                Write::Insert {
                    table,
                    ..
                }
                | Write::Replace {
                    table,
                    ..
                }
                | Write::Remove {
                    table,
                    ..
                } => table,
            })
        });
        let context = self.capture_entity_context(needs_context)?;
        let mut scope = StagedScope::begin(&self.db.db, permit).await?;
        let mut requested = vec![];
        let mut result = vec![];
        let mut aliases = BTreeMap::new();
        for AliasedWrite {
            mut write,
            patch,
            proposed_id,
            alias,
            references,
        } in writes
        {
            self.resolve_batch_references(&mut write, &references, &aliases)?;
            let mut partial = None;
            let write = match write {
                Write::Insert {
                    table,
                    fields,
                } if self.is_entity_table(&table) => {
                    let mut db = scope.enter(&self.db.db.storage)?;
                    let counter = db.state.next_id;
                    db.state.next_id = counter.checked_add(1).ok_or(CommandError::LimitExceeded)?;
                    let id = format!("entity:{}:{counter:016x}", self.ledger.header.namespace);
                    let old = db.generator.take();
                    db.generator = Some(std::sync::Arc::new(std::sync::Mutex::new(Box::new(
                        move || id.clone(),
                    ))));
                    let fields = self.entity_fields(&table, fields)?;
                    let outcome = db
                        .apply_batch(vec![Write::Insert {
                            table,
                            fields,
                        }])
                        .await;
                    db.generator = old;
                    let changes = outcome?;
                    for change in &changes {
                        requested.push((change.clone(), None));
                    }
                    if let Some(alias) = alias {
                        let change = changes.first().ok_or(CommandError::Protocol)?;
                        aliases.insert(alias, (change.table.clone(), change.id.clone()));
                    }
                    result.extend(changes);
                    continue;
                }
                Write::Insert {
                    table,
                    fields,
                } => Write::Insert {
                    table,
                    fields,
                },
                Write::Replace {
                    table,
                    id,
                    fields,
                } => {
                    let configured = self.is_entity_table(&table);
                    let id = if configured {
                        self.normalize_entity_target(&table, &id)?
                    } else {
                        id
                    };
                    let mut fields = if configured {
                        self.entity_fields(&table, fields)?
                    } else {
                        fields
                    };
                    if patch {
                        partial = Some(fields.clone());
                        let current = scope
                            .reader(&self.db.db.storage)?
                            .query(&table)
                            .where_("id", Operator::Eq, Value::String(id.clone()))
                            .fetch(&scope.reader(&self.db.db.storage)?)
                            .await?
                            .into_iter()
                            .next()
                            .ok_or_else(|| StorageError::RowNotFound {
                                table: table.clone(),
                                id: id.clone(),
                            })?;
                        let mut replacement = current.fields;
                        replacement.extend(fields);
                        fields = replacement;
                    }
                    Write::Replace {
                        table,
                        id,
                        fields,
                    }
                }
                Write::Remove {
                    table,
                    id,
                } => {
                    let configured = self.is_entity_table(&table);
                    let id = if configured {
                        self.normalize_entity_target(&table, &id)?
                    } else {
                        id
                    };
                    Write::Remove {
                        table,
                        id,
                    }
                }
            };
            let explicit_replace = if let Write::Replace {
                table,
                id,
                fields,
            } = &write
                && self.is_entity_table(table)
            {
                let before = scope
                    .reader(&self.db.db.storage)?
                    .query(table)
                    .where_("id", Operator::Eq, Value::String(id.clone()))
                    .fetch(&scope.reader(&self.db.db.storage)?)
                    .await?
                    .into_iter()
                    .next();
                Some(CommittedChange {
                    table: table.clone(),
                    id: id.clone(),
                    before,
                    after: Some(Row {
                        id: id.clone(),
                        fields: fields.clone(),
                    }),
                })
            } else {
                None
            };
            let mut changes = scope
                .enter(&self.db.db.storage)?
                .apply_batch_proposed(vec![(write, proposed_id)])
                .await?;
            if changes.is_empty()
                && let Some(change) = explicit_replace
            {
                scope
                    .enter(&self.db.db.storage)?
                    .journal
                    .as_mut()
                    .expect("staging journal")
                    .push(change.clone());
                changes.push(change);
            }
            for change in &changes {
                requested.push((change.clone(), partial.clone()));
            }
            result.extend(changes);
        }
        self.finish_entity_scope(scope, requested, context).await?;
        Ok(AliasedBatchResult {
            changes: result,
            aliases: aliases
                .into_iter()
                .map(|(alias, (_, id))| (alias, id))
                .collect(),
        })
    }

    pub(super) async fn entity_link(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        self.entity_link_proposed(source, id, relation, target, fields, None)
            .await
    }

    pub(super) async fn entity_link_proposed(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let configured = self.entity_relation(source, relation);
        if configured && proposed_id.is_some() {
            return Err(CommandError::InvalidCommand);
        }
        let context = self.capture_entity_context(configured)?;
        let mut scope = StagedScope::begin(&self.db.db, permit).await?;
        let id = if configured {
            self.entity_target_in(source, id)?
        } else {
            id.into()
        };
        let target = if configured {
            self.entity_target_in(self.relation_target(source, relation)?, target)?
        } else {
            target.into()
        };
        let fields = if configured {
            self.entity_fields(self.relation_through(source, relation)?, fields)?
        } else {
            fields
        };
        let counter = scope.state()?.next_id;
        let used = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let previous = if configured {
            let value = format!("entity:{}:{counter:016x}", self.ledger.header.namespace);
            let used = used.clone();
            scope
                .enter(&self.db.db.storage)?
                .generator
                .replace(std::sync::Arc::new(std::sync::Mutex::new(Box::new(
                    move || {
                        used.store(true, std::sync::atomic::Ordering::Relaxed);
                        value.clone()
                    },
                ))))
        } else {
            None
        };
        let result = scope
            .enter(&self.db.db.storage)?
            .link_proposed(source, &id, relation, &target, fields, proposed_id)
            .await;
        if configured {
            scope.enter(&self.db.db.storage)?.generator = previous;
        }
        let row = result?;
        if used.load(std::sync::atomic::Ordering::Relaxed) {
            scope.enter(&self.db.db.storage)?.state.next_id =
                counter.checked_add(1).ok_or(CommandError::LimitExceeded)?;
        }
        let changes = scope
            .journal()?
            .to_vec()
            .into_iter()
            .map(|c| (c, None))
            .collect();
        self.finish_entity_scope(scope, changes, context).await?;
        Ok(row)
    }

    pub(super) async fn entity_unlink(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
    ) -> Result<usize, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let configured = self.entity_relation(source, relation);
        let context = self.capture_entity_context(configured)?;
        let mut scope = StagedScope::begin(&self.db.db, permit).await?;
        let id = if configured {
            self.entity_target_in(source, id)?
        } else {
            id.into()
        };
        let target = if configured {
            self.entity_target_in(self.relation_target(source, relation)?, target)?
        } else {
            target.into()
        };
        let removed = scope
            .enter(&self.db.db.storage)?
            .unlink(source, &id, relation, &target)
            .await?;
        let changes = scope
            .journal()?
            .to_vec()
            .into_iter()
            .map(|c| (c, None))
            .collect();
        self.finish_entity_scope(scope, changes, context).await?;
        Ok(removed)
    }

    fn entity_relation(&self, source: &str, relation: &str) -> bool {
        self.db
            .db
            .schema
            .tables
            .get(source)
            .and_then(|table| table.relations.get(relation))
            .is_some_and(|relation| match relation {
                RelationDefinition::ManyToMany {
                    through,
                    ..
                } => self.is_entity_table(through),
                _ => false,
            })
    }

    pub(super) fn is_entity_table(&self, table: &str) -> bool {
        self.ledger
            .entity
            .as_ref()
            .is_some_and(|e| e.config.tables.contains(table))
    }

    fn capture_entity_context(
        &mut self,
        needed: bool,
    ) -> Result<Option<EntityContext>, CommandError> {
        if !needed {
            return Ok(None);
        }
        let context = self
            .entity_provider
            .as_mut()
            .ok_or(CommandError::InvalidCommand)?()?;
        if !(0..=253_402_300_799_999).contains(&context.now_ms) {
            return Err(CommandError::InvalidCommand);
        }
        context.identity.canonical()?;
        Ok(Some(context))
    }

    fn relation_target(&self, source: &str, relation: &str) -> Result<&str, CommandError> {
        match self
            .db
            .db
            .schema
            .tables
            .get(source)
            .and_then(|table| table.relations.get(relation))
        {
            Some(
                RelationDefinition::ToOne {
                    table,
                    ..
                }
                | RelationDefinition::HasMany {
                    table,
                    ..
                }
                | RelationDefinition::ManyToMany {
                    table,
                    ..
                },
            ) => Ok(table),
            None => Err(CommandError::InvalidCommand),
        }
    }

    fn relation_through(&self, source: &str, relation: &str) -> Result<&str, CommandError> {
        match self
            .db
            .db
            .schema
            .tables
            .get(source)
            .and_then(|table| table.relations.get(relation))
        {
            Some(RelationDefinition::ManyToMany {
                through,
                ..
            }) => Ok(through),
            _ => Err(CommandError::InvalidCommand),
        }
    }

    fn entity_target_in(&self, table: &str, id: &str) -> Result<String, CommandError> {
        let resolved = self.aliases.resolve(Some(table), id)?;
        if resolved != id || self.aliases.is_target(table, id) {
            return Ok(resolved);
        }
        self.entity_target(&resolved)
    }

    fn entity_target(&self, id: &str) -> Result<String, CommandError> {
        self.retained_target(self.aliases.resolve(None, id)?)
    }

    /// Bounded-history normalization only: no durable alias is consulted.
    fn retained_target(&self, id: String) -> Result<String, CommandError> {
        let mut value = Json::Object(BTreeMap::from([("recordId".into(), Json::String(id))]));
        normalize_references(
            &mut value,
            &[vec![PathPart::Key("recordId".into())]],
            &self.ledger,
        )?;
        match value {
            Json::Object(mut value) => match value.remove("recordId") {
                Some(Json::String(id)) => Ok(id),
                _ => Err(CommandError::InvalidCommand),
            },
            _ => Err(CommandError::InvalidCommand),
        }
    }

    /// SPEC-0049: a cell whose declared to-one relation names its target table
    /// resolves through that table only, so another table's alias source with
    /// the same literal cannot rewrite it. Undeclared cells have no known table.
    fn entity_fields(
        &self,
        table: &str,
        mut fields: BTreeMap<String, Value>,
    ) -> Result<BTreeMap<String, Value>, CommandError> {
        let definition = self.db.db.schema.tables.get(table);
        for (name, value) in &mut fields {
            if let Value::Reference(id) = value {
                let targets = definition
                    .into_iter()
                    .flat_map(|definition| definition.relations.values())
                    .filter_map(|relation| match relation {
                        RelationDefinition::ToOne {
                            table,
                            foreign_key,
                            ..
                        } if foreign_key == name => Some(table.as_str()),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                let target = (targets.len() == 1)
                    .then(|| targets.first().copied())
                    .flatten();
                *id = match target {
                    Some(target) if self.is_entity_table(target) => {
                        self.entity_target_in(target, id)?
                    }
                    Some(_) => self.retained_target(id.clone())?,
                    None => self.entity_target(id)?,
                };
            }
        }
        Ok(fields)
    }

    async fn finish_entity_scope(
        &mut self,
        scope: StagedScope,
        requested: Vec<(CommittedChange, Option<BTreeMap<String, Value>>)>,
        context: Option<EntityContext>,
    ) -> Result<(), CommandError> {
        let prepared = scope.prepare()?;
        let configured = self
            .ledger
            .entity
            .as_ref()
            .map(|entity| entity.config.tables.clone())
            .unwrap_or_default();
        let guards = initial_guards(&prepared.changes, self.db.db.state.version);
        let epoch = self.ledger_epoch;
        let (_, changes) = self.transact(prepared.state.clone(), &prepared.permit, |next| {
            // One full live closure per transaction; each enqueued record extends it.
            let mut live: Option<BTreeSet<String>> = None;
            for (change, partial) in requested {
                if !configured.contains(&change.table) {
                    continue;
                }
                let context = context.as_ref().ok_or(CommandError::InvalidCommand)?;
                if live.get_or_insert_with(|| next.live_keys()).len()
                    >= next.header.options.capacity
                {
                    return Err(CommandError::Capacity);
                }
                let op = match (&change.before, &change.after) {
                    (None, Some(_)) => EntityOperation::Create,
                    (_, None) => EntityOperation::Delete,
                    _ => EntityOperation::Update,
                };
                let mut fields = partial.unwrap_or_else(|| {
                    change
                        .after
                        .as_ref()
                        .map(|r| r.fields.clone())
                        .unwrap_or_default()
                });
                let mut removed_fields = BTreeSet::new();
                if op != EntityOperation::Delete
                    && let Some(before) = &change.before
                {
                    for name in before.fields.keys() {
                        if change
                            .after
                            .as_ref()
                            .is_some_and(|r| !r.fields.contains_key(name))
                        {
                            fields.insert(name.clone(), Value::Null);
                            removed_fields.insert(name.clone());
                        }
                    }
                }
                let mut reference_paths = vec![];
                if op != EntityOperation::Create {
                    reference_paths.push(vec![PathPart::Key("recordId".into())]);
                }
                let changes = fields
                    .iter()
                    .map(|(name, value)| {
                        if matches!(value, Value::Reference(_)) {
                            reference_paths.push(vec![
                                PathPart::Key("changes".into()),
                                PathPart::Key(name.clone()),
                            ]);
                        }
                        (name.clone(), wire_cell(value))
                    })
                    .collect();
                let mut args = Json::Object(BTreeMap::from([
                    ("recordId".into(), Json::String(change.id.clone())),
                    ("changes".into(), Json::Object(changes)),
                ]));
                let dependencies = normalize_references(&mut args, &reference_paths, next)?;
                let order = next.header.sequence;
                next.header_mut().sequence =
                    order.checked_add(1).ok_or(CommandError::LimitExceeded)?;
                next.header_mut().last_ms = next.header.last_ms.max(context.now_ms);
                let key = format!(
                    "{:016x}-{}-{order:016x}",
                    next.header.last_ms, next.header.namespace
                );
                let entity = next.entity.as_ref().ok_or(CommandError::InvalidCommand)?;
                let identity = (change.table.clone(), change.id.clone());
                crate::storage::count_work!(metadata_entries_visited, 1);
                let before = entity.metadata.get(&identity).cloned();
                let mut after = before.clone().unwrap_or_default();
                let stamp = FieldStamp {
                    timestamp_ms: context.now_ms,
                    authoritative: false,
                };
                if op == EntityOperation::Delete {
                    after.tombstone = Some(stamp);
                } else {
                    if after.tombstone.is_some() {
                        return Err(CommandError::InvalidCommand);
                    }
                    for name in fields.keys() {
                        after.fields.insert(name.clone(), stamp.clone());
                    }
                }
                next.put_metadata(identity, after.clone())?;
                let record = SyncRecord {
                    entity: Some(EntityDescriptor {
                        table: change.table.clone(),
                        op,
                        timestamp_ms: context.now_ms,
                        removed_fields,
                    }),
                    metadata_undo: vec![MetadataChange {
                        table: change.table.clone(),
                        record_id: change.id.clone(),
                        before,
                        after: Some(after),
                    }],
                    key: key.clone(),
                    command: String::new(),
                    args,
                    context: Json::Object(BTreeMap::from([
                        ("identity".into(), context.identity.clone()),
                        ("capturedAt".into(), Json::String(utc_time(context.now_ms))),
                    ])),
                    schema_version: prepared.state.version,
                    order,
                    reference_paths,
                    dependencies,
                    created: if op == EntityOperation::Create {
                        BTreeMap::from([(change.id.clone(), change.table.clone())])
                    } else {
                        BTreeMap::new()
                    },
                    mappings: BTreeMap::new(),
                    undo: vec![change],
                    state: SyncState::Queued,
                    frozen: None,
                    attempts: 0,
                    due_ms: 0,
                    outcome: None,
                    rolled_back: false,
                    reconciled: false,
                };
                record
                    .check_send_size()
                    .map_err(|_| CommandError::LimitExceeded)?;
                if next.insert_record(key.clone(), record).is_some() {
                    return Err(CommandError::Protocol);
                }
                if let Some(live) = &mut live {
                    next.extend_live(live, &key);
                }
            }
            Ok(((), guards, prepared.mutations))
        })?;
        if self.ledger_epoch != epoch {
            self.db.publish_committed(changes).await;
        }
        Ok(())
    }

    pub fn receive_entity(
        &mut self,
        table: &str,
        record_id: &str,
        write_id: &str,
        outcome: Option<RemoteOutcome>,
    ) -> Result<(), CommandError> {
        let record = self
            .ledger
            .records
            .get(write_id)
            .ok_or(CommandError::Protocol)?;
        let descriptor = record.entity.as_ref().ok_or(CommandError::Protocol)?;
        let wire = record.frozen.as_ref().ok_or(CommandError::Protocol)?;
        let Json::Object(envelope) = Json::parse(wire)? else {
            return Err(CommandError::Protocol);
        };
        if descriptor.table != table
            || envelope.get("recordId") != Some(&Json::String(record_id.into()))
        {
            return Err(CommandError::Protocol);
        }
        self.receive(write_id, outcome)
    }
}

fn wire_cell(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::String(s) | Value::Reference(s) => Json::String(s.clone()),
        Value::Integer(i) | Value::Timestamp(i) => Json::Integer(*i),
        Value::Float(f) => Json::Float(*f),
        Value::Boolean(b) => Json::Bool(*b),
    }
}

#[cfg(test)]
mod phase3_characterization {
    use super::*;
    use crate::command::phase3_cases::{corrupt_metadata, corrupt_record, owner, run};

    fn staged_change(invalid_row: bool) -> CommittedChange {
        CommittedChange {
            table: "items".into(),
            id: "staged".into(),
            before: None,
            after: Some(Row {
                id: "staged".into(),
                fields: BTreeMap::from([(
                    "body".into(),
                    if invalid_row {
                        Value::Integer(1)
                    } else {
                        Value::String("valid".into())
                    },
                )]),
            }),
        }
    }

    fn finish(
        db: &mut CommandDatabase<SnapshotStorage>,
        invalid_row: bool,
        missing_context: bool,
    ) -> Result<(), CommandError> {
        let permit = db.execution_owner.idle_permit().unwrap();
        let mut scope = run(StagedScope::begin(&db.db.db, permit)).unwrap();
        let change = staged_change(invalid_row);
        // A private malformed staged image pins finish's order independently
        // of the public write validator, which would reject this image earlier.
        scope
            .enter(&db.db.db.storage)
            .unwrap()
            .journal
            .as_mut()
            .unwrap()
            .push(change.clone());
        let requested = if missing_context {
            vec![(change, None)]
        } else {
            vec![]
        };
        run(db.finish_entity_scope(scope, requested, None))
    }

    #[test]
    fn si_y06_finish_entity_scope_ledger_precedes_staged_image_but_not_context() {
        let _owners = TEST_OWNERS.lock().unwrap();
        for exclusive in [false, true] {
            let (mut db, key) = owner();
            db.db.db.validated_exclusive = exclusive;
            let invalid_image = finish(&mut db, true, false).unwrap_err();
            let ledger_error = corrupt_record(&mut db.ledger, &key);
            assert_ne!(invalid_image, ledger_error);
            let prior = db.ledger.clone();
            assert_eq!(finish(&mut db, true, false), Err(ledger_error.clone()));
            assert_eq!(finish(&mut db, false, false), Err(ledger_error));
            assert_eq!(
                finish(&mut db, true, true),
                Err(CommandError::InvalidCommand)
            );
            assert_eq!(db.ledger, prior);
            assert!(run(db.query("items").fetch(&db)).unwrap().is_empty());
        }
    }

    #[test]
    fn si_y06_finish_entity_scope_unproven_metadata_error_precedes_staged_image() {
        let _owners = TEST_OWNERS.lock().unwrap();
        let (mut db, _) = owner();
        db.db.db.validated_exclusive = false;
        let invalid_image = finish(&mut db, true, false).unwrap_err();
        let ledger_error = corrupt_metadata(&mut db.ledger);
        assert_ne!(invalid_image, ledger_error);
        let prior = db.ledger.clone();
        assert_eq!(finish(&mut db, true, false), Err(ledger_error));
        assert_eq!(db.ledger, prior);
        assert!(run(db.query("items").fetch(&db)).unwrap().is_empty());
    }
}
