//! SPEC-0027 / API-0005: projected incremental state, explicit owned delivery.
use super::{Database, QueryBuilder, RelationDefinition, Write, database::public_row};
use crate::storage::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subscription(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservationValue {
    Rows(Vec<Row>),
    Count(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub subscription: Subscription,
    pub value: Result<ObservationValue, StorageError>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObservationStats {
    pub groups: usize,
    pub observers: usize,
    pub projected_queries: usize,
    pub candidate_updates: usize,
    pub materialized_rows: usize,
}

#[derive(Clone)]
struct Listener {
    columns: Option<BTreeSet<String>>,
    pending_count: Option<usize>,
    delivered_count: Option<(usize, u64)>,
    count_valid: bool,
    row_pending: bool,
}

struct Group {
    builder: QueryBuilder,
    entries: BTreeMap<String, Value>,
    /// SPEC-0037 R8.3: the same entries in visible order, so a window takes k
    /// and an update moves only the affected ids.
    ordered: BTreeSet<(OrderKey, String)>,
    listeners: BTreeMap<Subscription, Listener>,
    dirty: bool,
}

/// An order value under `compare` (Null lowest), reversed for Desc; ids break
/// ties ascending as the second tuple element.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OrderKey {
    value: Value,
    descending: bool,
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let ordering = compare(&self.value, &other.value);
        if self.descending {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Group {
    fn new(builder: QueryBuilder, entries: BTreeMap<String, Value>) -> Self {
        let mut group = Self {
            builder,
            entries: BTreeMap::new(),
            ordered: BTreeSet::new(),
            listeners: BTreeMap::new(),
            dirty: false,
        };
        group.replace_entries(entries);
        group
    }

    fn key(&self, value: Value) -> OrderKey {
        OrderKey {
            value,
            descending: self
                .builder
                .query
                .order
                .as_ref()
                .is_some_and(|order| order.direction == Direction::Desc),
        }
    }

    fn replace_entries(&mut self, entries: BTreeMap<String, Value>) {
        self.ordered = entries
            .iter()
            .map(|(id, value)| (self.key(value.clone()), id.clone()))
            .collect();
        self.entries = entries;
    }

    fn remove_entry(&mut self, id: &str) {
        if let Some(value) = self.entries.remove(id) {
            count_work!(ordered_ops, 1);
            self.ordered.remove(&(self.key(value), id.to_owned()));
        }
    }

    fn insert_entry(&mut self, id: String, value: Value) {
        count_work!(ordered_ops, 1);
        self.ordered.insert((self.key(value.clone()), id.clone()));
        self.entries.insert(id, value);
    }
}

pub struct ReactiveDatabase<S> {
    pub(crate) db: Database<S>,
    groups: Vec<Group>,
    queue: Vec<Notification>,
    stats: ObservationStats,
    clock: u64,
}

/// Keep detached groups owned across reads that may suspend. A cancelled
/// publication has consumed its change batch, so unfinished groups must rebuild
/// from storage before publishing again, including groups not yet visited.
struct Publication<'a, S> {
    owner: &'a mut ReactiveDatabase<S>,
    groups: Vec<Group>,
    next: usize,
}

impl<S> Drop for Publication<'_, S> {
    fn drop(&mut self) {
        let unfinished = &mut self.groups[self.next..];
        for group in unfinished {
            group.dirty = true;
            for listener in group.listeners.values_mut() {
                listener.pending_count = None;
                // Invalidate equality, but keep the last delivery deadline.
                listener.count_valid = false;
            }
        }
        // update_group queues events only after its final await. Earlier
        // completed snapshots remain valid historical events and are retained.
        self.owner.groups = std::mem::take(&mut self.groups);
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn error(message: &str) -> StorageError {
    StorageError::BackendFailure {
        message: message.into(),
    }
}

fn ids_query(table: &str, ids: impl IntoIterator<Item = String>) -> Query {
    Query {
        table: table.into(),
        predicates: vec![Predicate {
            field: "id".into(),
            operator: Operator::In,
            values: ids.into_iter().map(Value::String).collect(),
        }],
        order: None,
        limit: None,
    }
}

fn equal(field: &str, values: Vec<Value>) -> Predicate {
    Predicate {
        field: field.into(),
        operator: Operator::In,
        values,
    }
}

fn normalize(mut builder: QueryBuilder) -> QueryBuilder {
    let rank = |op: &Operator| match op {
        Operator::Eq => 0,
        Operator::Ne => 1,
        Operator::Lt => 2,
        Operator::Le => 3,
        Operator::Gt => 4,
        Operator::Ge => 5,
        Operator::In => 6,
    };
    for p in &mut builder.query.predicates {
        if p.operator == Operator::In {
            p.values.sort();
            p.values.dedup();
        }
    }
    builder.query.predicates.sort_by(|a, b| {
        a.field
            .cmp(&b.field)
            .then(rank(&a.operator).cmp(&rank(&b.operator)))
            .then(a.values.cmp(&b.values))
    });
    builder.query.predicates.dedup();
    builder
}

fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    }
}

// TC-0041 SI-Q03: group entries examined by each visible window (tests only).
#[cfg(test)]
thread_local! {
    static EXAMINED: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn examined(_entries: usize) {
    #[cfg(test)]
    EXAMINED.with(|examined| examined.borrow_mut().push(_entries));
}

fn visible(group: &Group) -> Vec<String> {
    let selected: Vec<String> = group
        .ordered
        .iter()
        .take(group.builder.query.limit.unwrap_or(usize::MAX))
        .map(|(_, id)| id.clone())
        .collect();
    examined(selected.len());
    selected
}

fn net(changes: Vec<CommittedChange>) -> Vec<CommittedChange> {
    let mut result: BTreeMap<(String, String), CommittedChange> = BTreeMap::new();
    for change in changes {
        let key = (change.table.clone(), change.id.clone());
        if let Some(prior) = result.get_mut(&key) {
            prior.after = change.after;
        } else {
            result.insert(key, change);
        }
    }
    result
        .into_values()
        .filter(|c| c.before != c.after)
        .collect()
}

impl<S: ObservationStorage> Database<S> {
    pub fn into_reactive(mut self) -> ReactiveDatabase<S> {
        self.journal = Some(vec![]);
        ReactiveDatabase {
            db: self,
            groups: vec![],
            queue: vec![],
            stats: ObservationStats::default(),
            clock: 0,
        }
    }
}

impl<S> Deref for ReactiveDatabase<S> {
    type Target = Database<S>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl<S: ObservationStorage> ReactiveDatabase<S> {
    pub fn into_storage(self) -> S {
        self.db.into_storage()
    }

    pub fn observation_stats(&self) -> ObservationStats {
        ObservationStats {
            groups: self.groups.len(),
            observers: self.groups.iter().map(|g| g.listeners.len()).sum(),
            ..self.stats
        }
    }

    pub fn unsubscribe(&mut self, token: Subscription) -> bool {
        let mut found = false;
        for group in &mut self.groups {
            found |= group.listeners.remove(&token).is_some();
        }
        self.groups.retain(|g| !g.listeners.is_empty());
        self.queue.retain(|n| n.subscription != token);
        found
    }

    pub fn drain_notifications(&mut self, now_ms: u64) -> Vec<Notification> {
        self.clock = self.clock.max(now_ms);
        let mut events = std::mem::take(&mut self.queue);
        for event in &events {
            if event.value.is_err() {
                for group in &mut self.groups {
                    if let Some(listener) = group.listeners.get_mut(&event.subscription) {
                        listener.count_valid = false;
                    }
                }
            }
            if let Ok(ObservationValue::Count(count)) = event.value {
                for group in &mut self.groups {
                    if let Some(listener) = group.listeners.get_mut(&event.subscription) {
                        listener.delivered_count = Some((count, self.clock));
                        listener.count_valid = true;
                    }
                }
            }
        }
        for group in &mut self.groups {
            for (token, listener) in &mut group.listeners {
                if let Some(count) = listener.pending_count {
                    if listener.count_valid
                        && listener
                            .delivered_count
                            .is_some_and(|(prior, _)| prior == count)
                    {
                        listener.pending_count = None;
                        continue;
                    }
                    if listener
                        .delivered_count
                        .is_none_or(|(_, time)| self.clock.saturating_sub(time) >= 16)
                    {
                        listener.pending_count = None;
                        listener.delivered_count = Some((count, self.clock));
                        listener.count_valid = true;
                        events.push(Notification {
                            subscription: *token,
                            value: Ok(ObservationValue::Count(count)),
                        });
                    }
                }
            }
        }
        events
    }

    async fn project(
        &mut self,
        query: &Query,
        fields: &[String],
    ) -> Result<Vec<ProjectedRow>, StorageError> {
        self.stats.projected_queries += 1;
        self.db
            .storage
            .project(&self.db.projection, query, fields)
            .await
    }

    async fn resolve(&mut self, builder: &QueryBuilder) -> Result<Query, StorageError> {
        let mut query = builder.query.clone();
        query.limit = None;
        let Some((source, id, name)) = &builder.navigation else {
            self.db.check_public_query(&query)?;
            return Ok(query);
        };
        let relation = self
            .db
            .schema
            .tables
            .get(source)
            .ok_or_else(|| StorageError::TableNotFound {
                table: source.clone(),
            })?
            .relations
            .get(name)
            .ok_or_else(|| StorageError::InvalidQuery {
                message: "undeclared relation".into(),
            })?
            .clone();
        query.table = match &relation {
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
            } => table.clone(),
        };
        self.db.check_public_query(&query)?;
        let source_fields = match &relation {
            RelationDefinition::ToOne {
                foreign_key,
                ..
            } => vec![foreign_key.clone()],
            _ => vec![],
        };
        let sources = self
            .project(&ids_query(source, [id.clone()]), &source_fields)
            .await?;
        let Some(row) = sources.first() else {
            query.predicates.push(equal("id", vec![]));
            return Ok(query);
        };
        match relation {
            RelationDefinition::ToOne {
                foreign_key,
                ..
            } => {
                let ids = match row.values.get(&foreign_key) {
                    Some(Value::Reference(id)) => vec![Value::String(id.clone())],
                    _ => vec![],
                };
                query.predicates.push(equal("id", ids));
            }
            RelationDefinition::HasMany {
                foreign_key,
                ..
            } => query
                .predicates
                .push(equal(&foreign_key, vec![Value::Reference(id.clone())])),
            RelationDefinition::ManyToMany {
                through,
                source_relation,
                target_relation,
                ..
            } => {
                let relations = &self.db.schema.tables[&through].relations;
                let fk = |name: &str| match &relations[name] {
                    RelationDefinition::ToOne {
                        foreign_key,
                        ..
                    } => foreign_key.clone(),
                    _ => unreachable!("validated relation"),
                };
                let source_key = fk(&source_relation);
                let target_key = fk(&target_relation);
                let membership = Query {
                    table: through,
                    predicates: vec![equal(&source_key, vec![Value::Reference(id.clone())])],
                    order: None,
                    limit: None,
                };
                let targets = self
                    .project(&membership, std::slice::from_ref(&target_key))
                    .await?
                    .into_iter()
                    .filter_map(|r| match r.values.get(&target_key) {
                        Some(Value::Reference(id)) => Some(Value::String(id.clone())),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                query
                    .predicates
                    .push(equal("id", targets.into_iter().collect()));
            }
        }
        Ok(query)
    }

    async fn keys(&mut self, query: &Query) -> Result<BTreeMap<String, Value>, StorageError> {
        Ok(self
            .project(query, &[])
            .await?
            .into_iter()
            .map(|row| {
                let key = row
                    .order_key
                    .unwrap_or_else(|| Value::String(row.id.clone()));
                (row.id, key)
            })
            .collect())
    }

    async fn rows(&mut self, table: &str, ids: &[String]) -> Result<Vec<Row>, StorageError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut rows: BTreeMap<_, _> = self
            .db
            .storage
            .query(&self.db.projection, &ids_query(table, ids.iter().cloned()))
            .await?
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            let row = rows
                .remove(id)
                .ok_or_else(|| error("observed row disappeared"))?;
            result.push(public_row(row, self.db.schema.version)?);
        }
        self.stats.materialized_rows += result.len();
        Ok(result)
    }

    async fn observe(
        &mut self,
        builder: &QueryBuilder,
        columns: Option<Vec<String>>,
    ) -> Result<Subscription, StorageError> {
        let builder = normalize(builder.clone());
        let query = self.resolve(&builder).await?;
        let columns = columns.map(|c| c.into_iter().collect::<BTreeSet<_>>());
        if let Some(columns) = &columns {
            for name in columns {
                if name != "id"
                    && !self.db.schema.tables[&query.table]
                        .columns
                        .contains_key(name)
                {
                    return Err(StorageError::InvalidQuery {
                        message: "unknown observed column".into(),
                    });
                }
            }
        }
        // Recovery is explicit: never create a second cache or register against
        // a stale candidate set while an equivalent group is dirty.
        if self.groups.iter().any(|g| g.builder == builder && g.dirty) {
            return Err(error("observation cache requires refresh"));
        }
        // Registration can fail without changing the shared cache or consuming a handle.
        let index = self
            .groups
            .iter()
            .position(|g| g.builder == builder && !g.dirty);
        let fresh = if index.is_none() {
            Some(self.keys(&query).await?)
        } else {
            None
        };
        let fresh = fresh.map(|entries| Group::new(builder.clone(), entries));
        let selected = visible(
            fresh
                .as_ref()
                .unwrap_or_else(|| &self.groups[index.unwrap()]),
        );
        let value = if columns.is_none() {
            ObservationValue::Count(selected.len())
        } else {
            ObservationValue::Rows(self.rows(&query.table, &selected).await?)
        };
        let token = Subscription(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| error("subscription IDs exhausted"))?,
        );
        let index = if let Some(index) = index {
            index
        } else {
            self.groups.push(fresh.unwrap());
            self.groups.len() - 1
        };
        self.groups[index].listeners.insert(
            token,
            Listener {
                columns,
                pending_count: None,
                delivered_count: None,
                count_valid: true,
                row_pending: false,
            },
        );
        self.queue.push(Notification {
            subscription: token,
            value: Ok(value),
        });
        Ok(token)
    }

    fn target_table(&self, builder: &QueryBuilder) -> String {
        if let Some((source, _, name)) = &builder.navigation {
            match &self.db.schema.tables[source].relations[name] {
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
                } => table.clone(),
            }
        } else {
            builder.query.table.clone()
        }
    }

    fn structural(&self, builder: &QueryBuilder, changes: &[CommittedChange]) -> bool {
        let Some((source, id, name)) = &builder.navigation else {
            return false;
        };
        if changes.iter().any(|c| &c.table == source && &c.id == id) {
            return true;
        }
        if let RelationDefinition::ManyToMany {
            through,
            source_relation,
            ..
        } = &self.db.schema.tables[source].relations[name]
        {
            let RelationDefinition::ToOne {
                foreign_key,
                ..
            } = &self.db.schema.tables[through].relations[source_relation]
            else {
                unreachable!()
            };
            return changes.iter().any(|c| {
                &c.table == through
                    && [c.before.as_ref(), c.after.as_ref()]
                        .into_iter()
                        .flatten()
                        .any(|r| r.fields.get(foreign_key) == Some(&Value::Reference(id.clone())))
            });
        }
        false
    }

    async fn affected(
        &mut self,
        table: &str,
        builder: &QueryBuilder,
        changes: &[CommittedChange],
    ) -> Result<BTreeSet<String>, StorageError> {
        let mut ids: BTreeSet<_> = changes
            .iter()
            .filter(|c| c.table == table)
            .map(|c| c.id.clone())
            .collect();
        let paths = builder
            .query
            .predicates
            .iter()
            .map(|p| &p.field)
            .chain(builder.query.order.iter().map(|o| &o.field));
        let mut dependencies: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for path in paths {
            let resolved = resolve_field(&self.db.projection, table, path)?;
            if let Some(fk) = resolved.foreign_key {
                for change in changes.iter().filter(|c| c.table == resolved.table) {
                    let consulted_changed = if resolved.name == "id" {
                        change.before.is_some() != change.after.is_some()
                    } else {
                        change
                            .before
                            .as_ref()
                            .and_then(|r| r.fields.get(&resolved.name))
                            != change
                                .after
                                .as_ref()
                                .and_then(|r| r.fields.get(&resolved.name))
                            || change.before.is_some() != change.after.is_some()
                    };
                    if consulted_changed {
                        dependencies
                            .entry((resolved.table.clone(), fk.clone()))
                            .or_default()
                            .insert(change.id.clone());
                    }
                }
            }
        }
        for ((_, fk), targets) in dependencies {
            let query = Query {
                table: table.into(),
                predicates: vec![equal(
                    &fk,
                    targets.into_iter().map(Value::Reference).collect(),
                )],
                order: None,
                limit: None,
            };
            ids.extend(self.project(&query, &[]).await?.into_iter().map(|r| r.id));
        }
        Ok(ids)
    }

    async fn update_group(
        &mut self,
        group: &mut Group,
        changes: &[CommittedChange],
        explicit_retry: bool,
    ) -> Result<(), StorageError> {
        let table = self.target_table(&group.builder);
        let old = visible(group);
        let recovering = group.dirty;
        // Fresh keys are read before the group changes, so a failed read leaves
        // the group untouched (it is then marked dirty and fully recomputed).
        if recovering || self.structural(&group.builder, changes) {
            let query = self.resolve(&group.builder).await?;
            let entries = self.keys(&query).await?;
            group.replace_entries(entries);
        } else {
            let affected = self.affected(&table, &group.builder, changes).await?;
            if affected.is_empty() {
                if !group
                    .listeners
                    .values()
                    .any(|listener| listener.row_pending)
                {
                    return Ok(());
                }
            } else {
                self.stats.candidate_updates += affected.len();
                let mut query = self.resolve(&group.builder).await?;
                query.predicates.push(equal(
                    "id",
                    affected.iter().cloned().map(Value::String).collect(),
                ));
                let fresh = self.keys(&query).await?;
                for id in &affected {
                    group.remove_entry(id);
                }
                for (id, value) in fresh {
                    group.insert_entry(id, value);
                }
            }
        }
        let selected = visible(group);
        let selected_set: BTreeSet<_> = selected.iter().collect();
        let membership = old.iter().collect::<BTreeSet<_>>() != selected_set;
        let notify: Vec<_> = group
            .listeners
            .iter()
            .filter_map(|(token, listener)| {
                listener.columns.as_ref().and_then(|columns| {
                    let changed = changes.iter().any(|c| {
                        c.table == table
                            && selected_set.contains(&c.id)
                            && old.contains(&c.id)
                            && columns.iter().any(|field| {
                                field != "id"
                                    && c.before.as_ref().and_then(|r| r.fields.get(field))
                                        != c.after.as_ref().and_then(|r| r.fields.get(field))
                            })
                    });
                    (membership || changed || recovering || listener.row_pending).then_some(*token)
                })
            })
            .collect();
        // Candidate validity is independent of full-row delivery. A failed
        // materialization cannot poison count observers sharing these keys.
        group.dirty = false;
        for listener in group.listeners.values_mut().filter(|l| l.columns.is_none()) {
            listener.pending_count = Some(selected.len());
        }
        let rows = if notify.is_empty() {
            Ok(vec![])
        } else {
            self.rows(&table, &selected).await
        };
        for token in notify {
            let listener = group.listeners.get_mut(&token).expect("selected listener");
            match &rows {
                Ok(rows) => {
                    listener.row_pending = false;
                    self.queue.push(Notification {
                        subscription: token,
                        value: Ok(ObservationValue::Rows(rows.clone())),
                    });
                }
                Err(error) => {
                    if explicit_retry || !listener.row_pending {
                        self.queue.push(Notification {
                            subscription: token,
                            value: Err(error.clone()),
                        });
                    }
                    listener.row_pending = true;
                }
            }
        }
        Ok(())
    }

    async fn publish(&mut self, changes: Vec<CommittedChange>, dirty_only: bool) {
        let changes = net(changes);
        let groups = std::mem::take(&mut self.groups);
        let mut publication = Publication {
            owner: self,
            groups,
            next: 0,
        };
        while publication.next < publication.groups.len() {
            let group = &mut publication.groups[publication.next];
            let rows_pending = group
                .listeners
                .values()
                .any(|listener| listener.row_pending);
            if dirty_only && !group.dirty && !rows_pending {
                publication.next += 1;
                continue;
            }
            if changes.is_empty() && !group.dirty && !rows_pending {
                publication.next += 1;
                continue;
            }
            if let Err(error) = publication
                .owner
                .update_group(group, &changes, dirty_only)
                .await
            {
                if !group.dirty || dirty_only {
                    for token in group.listeners.keys() {
                        publication.owner.queue.push(Notification {
                            subscription: *token,
                            value: Err(error.clone()),
                        });
                    }
                }
                for listener in group.listeners.values_mut() {
                    listener.pending_count = None;
                    // Recovery invalidates equality, while preserving the deadline.
                    listener.count_valid = false;
                }
                group.dirty = true;
            }
            publication.next += 1;
        }
    }

    pub(crate) fn reset_observations(&mut self) {
        self.groups.clear();
        self.queue.clear();
        self.stats = ObservationStats::default();
    }

    pub(crate) async fn publish_committed(&mut self, changes: Vec<CommittedChange>) {
        self.publish(changes, false).await;
    }

    pub async fn refresh_notifications(&mut self) {
        self.publish(vec![], true).await;
    }

    async fn finish<T>(&mut self, result: Result<T, StorageError>) -> Result<T, StorageError> {
        let changes = std::mem::take(self.db.journal.as_mut().expect("reactive journal"));
        if !changes.is_empty() {
            self.publish(changes, false).await;
        }
        result
    }

    pub async fn insert(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        let result = self.db.insert(table, fields).await;
        self.finish(result).await
    }

    pub async fn insert_proposed(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, StorageError> {
        let result = self.db.insert_proposed(table, fields, proposed_id).await;
        self.finish(result).await
    }

    pub async fn apply_batch_proposed(
        &mut self,
        writes: Vec<(Write, Option<String>)>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        let result = self.db.apply_batch_proposed(writes).await;
        self.finish(result).await
    }

    pub async fn link_proposed(
        &mut self,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, StorageError> {
        let result = self
            .db
            .link_proposed(table, id, relation, target, fields, proposed_id)
            .await;
        self.finish(result).await
    }

    pub async fn replace(
        &mut self,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        let result = self.db.replace(table, id, fields).await;
        self.finish(result).await
    }

    pub async fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError> {
        let result = self.db.remove(table, id).await;
        self.finish(result).await
    }

    pub async fn apply_batch(
        &mut self,
        writes: Vec<Write>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        let result = self.db.apply_batch(writes).await;
        self.finish(result).await
    }

    pub async fn link(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        let result = self.db.link(source, id, relation, target, fields).await;
        self.finish(result).await
    }

    pub async fn unlink(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
    ) -> Result<usize, StorageError> {
        let result = self.db.unlink(source, id, relation, target).await;
        self.finish(result).await
    }
}

impl QueryBuilder {
    pub async fn observe<S: ObservationStorage>(
        &self,
        db: &mut ReactiveDatabase<S>,
    ) -> Result<Subscription, StorageError> {
        db.observe(self, Some(vec![])).await
    }

    pub async fn observe_count<S: ObservationStorage>(
        &self,
        db: &mut ReactiveDatabase<S>,
    ) -> Result<Subscription, StorageError> {
        db.observe(self, None).await
    }

    pub async fn observe_with_columns<S: ObservationStorage>(
        &self,
        db: &mut ReactiveDatabase<S>,
        columns: Vec<String>,
    ) -> Result<Subscription, StorageError> {
        db.observe(self, Some(columns)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::{CommandDatabase, records::SyncOptions},
        model::{SchemaDefinition, TableDefinition},
        storage::{SnapshotStorage, counters},
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("reactive operation suspended"),
        }
    }

    fn rank(value: i64) -> BTreeMap<String, Value> {
        BTreeMap::from([("rank".into(), Value::Integer(value))])
    }

    /// The window algorithm before SPEC-0037 R8.3: sort every entry by
    /// `compare`, reverse for Desc, break ties by ascending id, take the limit.
    fn sorted_window(builder: &QueryBuilder, entries: &BTreeMap<String, Value>) -> Vec<String> {
        let descending = builder
            .query
            .order
            .as_ref()
            .is_some_and(|order| order.direction == Direction::Desc);
        let mut keys: Vec<_> = entries.iter().collect();
        keys.sort_by(|(left_id, left), (right_id, right)| {
            let ordering = compare(left, right);
            (if descending {
                ordering.reverse()
            } else {
                ordering
            })
            .then(left_id.cmp(right_id))
        });
        keys.into_iter()
            .take(builder.query.limit.unwrap_or(usize::MAX))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// TC-0041 SI-Q04: the ordered set yields exactly the old sorted window
    /// after every update, over mixed kinds, Null, both directions and limits.
    #[test]
    fn si_q04_ordered_set_equals_the_sorted_window_after_every_update() {
        let pool = [
            Value::Null,
            Value::Integer(-1),
            Value::Integer(5),
            Value::String("a".into()),
            Value::String("b".into()),
            Value::Float(FiniteFloat::new(-0.0).unwrap()),
            Value::Float(FiniteFloat::new(2.5).unwrap()),
            Value::Boolean(true),
        ];
        let mut seed = 0x2545_f491_u64;
        let mut next = move |bound: usize| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            (seed >> 33) as usize % bound
        };
        for direction in [Direction::Asc, Direction::Desc] {
            for limit in [None, Some(0), Some(1), Some(3)] {
                let mut builder = QueryBuilder {
                    query: Query {
                        table: "items".into(),
                        predicates: vec![],
                        order: Some(OrderBy {
                            field: "x".into(),
                            direction: direction.clone(),
                        }),
                        limit,
                    },
                    navigation: None,
                };
                builder.query.limit = limit;
                let mut group = Group::new(builder.clone(), BTreeMap::new());
                for step in 0..400 {
                    let id = format!("id:{:02}", next(24));
                    // An update is a removal of the stored old key, then a reinsert.
                    group.remove_entry(&id);
                    if next(4) != 0 {
                        group.insert_entry(id, pool[next(pool.len())].clone());
                    }
                    assert_eq!(
                        visible(&group),
                        sorted_window(&builder, &group.entries),
                        "{direction:?} {limit:?} step {step}"
                    );
                    assert_eq!(group.ordered.len(), group.entries.len());
                }
            }
        }
    }

    /// TC-0041 SI-Q03: one update to a limit-10 group touches only the affected
    /// id in the ordered set and examines exactly the visible window, at any N.
    #[test]
    fn si_q03_single_row_update_on_a_limit_10_group_is_bounded() {
        let mut measured = Vec::new();
        for n in [10_i64, 1000] {
            let schema = SchemaDefinition {
                version: 1,
                tables: BTreeMap::from([(
                    "items".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "rank".into(),
                            Column {
                                kind: ColumnType::Integer,
                                optional: false,
                            },
                        )]),
                        ..Default::default()
                    },
                )]),
            };
            let mut db = run(CommandDatabase::open(
                SnapshotStorage::default(),
                schema,
                vec![],
                [0x33; 16],
                SyncOptions::default(),
            ))
            .unwrap();
            let mut ids = Vec::new();
            for index in 0..n + 10 {
                ids.push(run(db.insert("items", rank(index))).unwrap().id);
            }
            let query = db
                .query("items")
                .order_by("rank", Direction::Desc)
                .limit(10);
            let token = run(db.observe(&query)).unwrap();
            db.drain_notifications(0);
            EXAMINED.with(|examined| examined.borrow_mut().clear());
            counters::reset();
            // Move one visible row below the window.
            run(db.replace("items", ids.last().unwrap(), rank(-1))).unwrap();
            let events = db.drain_notifications(1);
            assert!(events.iter().any(|event| event.subscription == token));
            let examined = EXAMINED.with(|examined| examined.borrow().clone());
            measured.push((counters::snapshot().ordered_ops, examined));
        }
        for (ordered_ops, examined) in &measured {
            assert!(*ordered_ops <= 2, "ordered_ops {measured:?}");
            assert!(
                examined.iter().all(|entries| *entries == 10),
                "examined {measured:?}"
            );
        }
        assert_eq!(measured[0], measured[1], "work depends on N");
    }
}
