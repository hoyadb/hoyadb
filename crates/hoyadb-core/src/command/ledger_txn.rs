//! SPEC-0037 R9: synchronous first-image ledger transaction.
use super::{CommandError, durable::AliasState, entity::*, records::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

type Identity = (String, String);

/// Scalar entity state only. The metadata map is always edited by identity.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct EntityHeader {
    pub ledger_format: u8,
    pub config: EntityConfig,
    pub generation: i64,
    pub revision: i64,
    pub checkpoint: Option<String>,
    pub ready: bool,
    pub pages: i64,
    pub changes: i64,
}

impl EntityHeader {
    fn from(entity: &EntityLedger) -> Self {
        let EntityLedger {
            ledger_format,
            config,
            generation,
            revision,
            checkpoint,
            ready,
            pages,
            changes,
            metadata: _,
        } = entity;
        Self {
            ledger_format: *ledger_format,
            config: config.clone(),
            generation: *generation,
            revision: *revision,
            checkpoint: checkpoint.clone(),
            ready: *ready,
            pages: *pages,
            changes: *changes,
        }
    }
    fn install(self, entity: &mut EntityLedger) {
        let Self {
            ledger_format,
            config,
            generation,
            revision,
            checkpoint,
            ready,
            pages,
            changes,
        } = self;
        entity.ledger_format = ledger_format;
        entity.config = config;
        entity.generation = generation;
        entity.revision = revision;
        entity.checkpoint = checkpoint;
        entity.ready = ready;
        entity.pages = pages;
        entity.changes = changes;
    }
}

pub(super) struct LedgerTxn<'a> {
    ledger: Option<&'a mut SyncLedger>,
    exclusive: bool,
    prior_header: SyncHeader,
    prior_entity: Option<EntityHeader>,
    prior_metadata_len: usize,
    first_records: BTreeMap<String, Option<SyncRecord>>,
    first_metadata: BTreeMap<Identity, Option<RowMetadata>>,
    aliases: Option<&'a mut AliasState>,
    prior_alias_enabled: bool,
    first_aliases: BTreeMap<Identity, Option<String>>,
}

impl Deref for LedgerTxn<'_> {
    type Target = SyncLedger;
    fn deref(&self) -> &Self::Target {
        self.ledger.as_deref().expect("active ledger transaction")
    }
}

impl<'a> LedgerTxn<'a> {
    pub(super) fn new(ledger: &'a mut SyncLedger, exclusive: bool) -> Self {
        Self {
            prior_header: ledger.header.clone(),
            prior_entity: ledger.entity.as_ref().map(EntityHeader::from),
            prior_metadata_len: ledger
                .entity
                .as_ref()
                .map_or(0, |entity| entity.metadata.len()),
            ledger: Some(ledger),
            exclusive,
            first_records: BTreeMap::new(),
            first_metadata: BTreeMap::new(),
            aliases: None,
            prior_alias_enabled: false,
            first_aliases: BTreeMap::new(),
        }
    }
    pub(super) fn with_aliases(
        ledger: &'a mut SyncLedger,
        aliases: &'a mut AliasState,
        exclusive: bool,
    ) -> Self {
        let mut txn = Self::new(ledger, exclusive);
        txn.prior_alias_enabled = aliases.enabled;
        txn.aliases = Some(aliases);
        txn
    }
    pub(super) fn aliases(&self) -> Option<&AliasState> {
        self.aliases.as_deref()
    }
    pub(super) fn enable_aliases(&mut self) -> Result<(), CommandError> {
        if self.prior_entity.is_some() || self.has_live_recovery() {
            return Err(CommandError::InvalidCommand);
        }
        let aliases = self.aliases.as_deref_mut().ok_or(CommandError::Protocol)?;
        if !aliases.mappings.is_empty() {
            return Err(CommandError::Protocol);
        }
        aliases.enabled = true;
        Ok(())
    }
    pub(super) fn put_alias(
        &mut self,
        table: String,
        local: String,
        target: String,
    ) -> Result<(), CommandError> {
        let Some(aliases) = self.aliases() else {
            return Ok(());
        };
        if !aliases.enabled || local == target {
            return Ok(());
        }
        aliases.validate_entry(self, &table, &local, &target)?;
        let identity = (table, local);
        let before = aliases.mappings.get(&identity).cloned();
        self.first_aliases.entry(identity.clone()).or_insert(before);
        self.aliases
            .as_deref_mut()
            .expect("alias state retained")
            .insert(identity, target);
        Ok(())
    }
    pub(super) fn changed_aliases(&self) -> BTreeSet<Identity> {
        self.first_aliases
            .iter()
            .filter(|(id, before)| {
                self.aliases().and_then(|state| state.mappings.get(*id)) != before.as_ref()
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
    pub(super) fn prior_alias_enabled(&self) -> bool {
        self.prior_alias_enabled
    }
    pub(super) fn stored_rows(
        &self,
    ) -> Result<Vec<crate::storage::Row>, crate::storage::StorageError> {
        match self.aliases() {
            Some(aliases) => super::durable::stored_rows(self, aliases),
            None => self.to_rows(),
        }
    }
    pub(super) fn validate(&self) -> Result<(), crate::storage::StorageError> {
        if !self.exclusive {
            self.deref().validate()?;
            if let Some(aliases) = self.aliases() {
                aliases.validate(self)?;
            }
            return Ok(());
        }
        self.validate_bounded()?;
        if let Some(aliases) = self.aliases() {
            for (table, local) in self.first_aliases.keys() {
                if let Some(target) = aliases.mappings.get(&(table.clone(), local.clone())) {
                    aliases.validate_entry(self, table, local, target)?;
                }
            }
        }
        for identity in self.first_metadata.keys() {
            if let Some(metadata) = self
                .entity
                .as_ref()
                .and_then(|entity| entity.metadata.get(identity))
            {
                validate_metadata_entry(&identity.0, &identity.1, metadata)?;
            }
        }
        Ok(())
    }
    pub(super) fn serialization_check(&self) -> Result<(), crate::storage::StorageError> {
        if self.exclusive {
            self.validate()
        } else {
            self.stored_rows().map(|_| ())
        }
    }
    fn current(&mut self) -> &mut SyncLedger {
        self.ledger
            .as_deref_mut()
            .expect("active ledger transaction")
    }
    fn capture_record(&mut self, key: &str) {
        if !self.first_records.contains_key(key) {
            let before = self.records().get(key).cloned();
            self.first_records.insert(key.to_owned(), before);
        }
    }
    fn capture_metadata(&mut self, identity: &Identity) {
        if !self.first_metadata.contains_key(identity) {
            crate::storage::count_work!(metadata_entries_visited, 1);
            let before = self
                .entity
                .as_ref()
                .and_then(|entity| entity.metadata.get(identity))
                .cloned();
            self.first_metadata.insert(identity.clone(), before);
        }
    }

    pub(super) fn header_mut(&mut self) -> &mut SyncHeader {
        &mut self.current().header
    }
    pub(super) fn entity_header(&self) -> Option<EntityHeader> {
        self.entity.as_ref().map(EntityHeader::from)
    }
    pub(super) fn edit_entity_header(
        &mut self,
        edit: impl FnOnce(&mut EntityHeader) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        let mut header = self.entity_header().ok_or(CommandError::InvalidCommand)?;
        edit(&mut header)?;
        header.install(self.current().entity.as_mut().expect("known entity header"));
        Ok(())
    }
    pub(super) fn adopt_entity(&mut self, entity: EntityLedger) -> Result<(), CommandError> {
        if self.entity.is_some() || !entity.metadata.is_empty() {
            return Err(CommandError::InvalidCommand);
        }
        self.current().entity = Some(entity);
        Ok(())
    }
    pub(super) fn record_mut(&mut self, key: &str) -> Option<&mut SyncRecord> {
        if !self.records().contains_key(key) {
            return None;
        }
        self.capture_record(key);
        self.current().records.get_mut(key)
    }
    pub(super) fn insert_record(&mut self, key: String, record: SyncRecord) -> Option<SyncRecord> {
        self.capture_record(&key);
        self.current().records.insert(key, record)
    }
    pub(super) fn remove_record(&mut self, key: &str) -> Option<SyncRecord> {
        if !self.records().contains_key(key) {
            return None;
        }
        self.capture_record(key);
        self.current().records.remove(key)
    }
    fn records(&self) -> &BTreeMap<String, SyncRecord> {
        &self.deref().records
    }
    pub(super) fn metadata_mut(&mut self, identity: &Identity) -> Option<&mut RowMetadata> {
        if !self.entity.as_ref()?.metadata.contains_key(identity) {
            return None;
        }
        self.capture_metadata(identity);
        self.current().entity.as_mut()?.metadata.get_mut(identity)
    }
    pub(super) fn put_metadata(
        &mut self,
        identity: Identity,
        value: RowMetadata,
    ) -> Result<Option<RowMetadata>, CommandError> {
        if self.entity.is_none() {
            return Err(CommandError::InvalidCommand);
        }
        self.capture_metadata(&identity);
        Ok(self
            .current()
            .entity
            .as_mut()
            .expect("known entity ledger")
            .metadata
            .insert(identity, value))
    }
    pub(super) fn remove_metadata(&mut self, identity: &Identity) -> Option<RowMetadata> {
        if !self.entity.as_ref()?.metadata.contains_key(identity) {
            return None;
        }
        self.capture_metadata(identity);
        self.current().entity.as_mut()?.metadata.remove(identity)
    }
    pub(super) fn prior_record(&self, key: &str) -> Option<&SyncRecord> {
        self.first_records
            .get(key)
            .map_or_else(|| self.records().get(key), Option::as_ref)
    }
    pub(super) fn prior_metadata(&self, identity: &Identity) -> Option<&RowMetadata> {
        self.first_metadata.get(identity).map_or_else(
            || self.entity.as_ref()?.metadata.get(identity),
            Option::as_ref,
        )
    }
    pub(super) fn prior_records(&self) -> Vec<&SyncRecord> {
        self.records()
            .keys()
            .chain(self.first_records.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|key| self.prior_record(key))
            .collect()
    }
    pub(super) fn prior_metadata_len(&self) -> usize {
        self.prior_metadata_len
    }
    pub(super) fn prior_header(&self) -> &SyncHeader {
        &self.prior_header
    }
    pub(super) fn prior_entity_header(&self) -> Option<&EntityHeader> {
        self.prior_entity.as_ref()
    }
    #[cfg(any(test, feature = "work-counters"))]
    pub(super) fn record_keys(&self) -> BTreeSet<String> {
        self.first_records.keys().cloned().collect()
    }
    #[cfg(any(test, feature = "work-counters"))]
    pub(super) fn metadata_keys(&self) -> BTreeSet<Identity> {
        self.first_metadata.keys().cloned().collect()
    }
    pub(super) fn changed_records(&self) -> BTreeSet<String> {
        self.first_records
            .iter()
            .filter(|(key, prior)| self.records().get(*key) != prior.as_ref())
            .map(|(key, _)| key.clone())
            .collect()
    }
    pub(super) fn changed_metadata(&self) -> BTreeSet<Identity> {
        self.first_metadata
            .iter()
            .filter(|(identity, prior)| {
                self.entity
                    .as_ref()
                    .and_then(|entity| entity.metadata.get(*identity))
                    != prior.as_ref()
            })
            .map(|(identity, _)| identity.clone())
            .collect()
    }
    pub(super) fn changed(&self) -> bool {
        self.header != self.prior_header
            || self.entity_header() != self.prior_entity
            || !self.changed_records().is_empty()
            || !self.changed_metadata().is_empty()
            || self
                .aliases()
                .is_some_and(|state| state.enabled != self.prior_alias_enabled)
            || !self.changed_aliases().is_empty()
    }
    pub(super) fn commit(mut self) -> &'a mut SyncLedger {
        self.ledger.take().expect("active ledger transaction")
    }
}

impl Drop for LedgerTxn<'_> {
    fn drop(&mut self) {
        let Some(ledger) = self.ledger.take() else {
            return;
        };
        if let Some(aliases) = self.aliases.as_deref_mut() {
            aliases.enabled = self.prior_alias_enabled;
            for (identity, previous) in std::mem::take(&mut self.first_aliases) {
                match previous {
                    Some(target) => {
                        aliases.insert(identity, target);
                    }
                    None => {
                        aliases.remove(&identity);
                    }
                }
            }
        }
        for (key, before) in std::mem::take(&mut self.first_records) {
            match before {
                Some(record) => {
                    ledger.records.insert(key, record);
                }
                None => {
                    ledger.records.remove(&key);
                }
            }
        }
        if let Some(entity) = ledger.entity.as_mut() {
            for (identity, before) in std::mem::take(&mut self.first_metadata) {
                match before {
                    Some(metadata) => {
                        entity.metadata.insert(identity, metadata);
                    }
                    None => {
                        entity.metadata.remove(&identity);
                    }
                }
            }
        }
        ledger.header = self.prior_header.clone();
        match self.prior_entity.take() {
            Some(header) => header.install(
                ledger
                    .entity
                    .as_mut()
                    .expect("transaction retains existing entity shell"),
            ),
            None => ledger.entity = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Json;
    use crate::storage::counters;

    fn metadata(value: i64) -> RowMetadata {
        RowMetadata {
            fields: BTreeMap::from([(
                "body".into(),
                FieldStamp {
                    timestamp_ms: value,
                    authoritative: false,
                },
            )]),
            tombstone: None,
        }
    }
    fn record(key: &str) -> SyncRecord {
        SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: key.into(),
            command: "change-v1".into(),
            args: Json::Null,
            context: Json::Null,
            schema_version: 1,
            order: 1,
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
    fn ledger(m: usize) -> SyncLedger {
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "txn".into(),
            tables: BTreeSet::from(["items".into()]),
        });
        for i in 0..m {
            entity
                .metadata
                .insert(("items".into(), format!("r{i:04}")), metadata(1));
        }
        SyncLedger {
            header: SyncHeader {
                namespace: "ab".repeat(16),
                sequence: 2,
                last_ms: 1,
                options: SyncOptions::default(),
            },
            entity: Some(entity),
            records: BTreeMap::from([("old".into(), record("old"))]),
        }
    }
    #[test]
    fn si_y01_transaction_first_images_restore_repeated_touches() {
        let mut ledger = ledger(10);
        let before = ledger.clone();
        let identity = ("items".into(), "r0000".into());
        {
            let mut txn = LedgerTxn::new(&mut ledger, true);
            txn.record_mut("old").unwrap().due_ms = 5;
            txn.remove_record("old");
            txn.insert_record("old".into(), record("old"));
            txn.insert_record("new".into(), record("new"));
            txn.remove_record("new");
            txn.put_metadata(identity.clone(), metadata(2)).unwrap();
            txn.remove_metadata(&identity);
            txn.put_metadata(identity.clone(), metadata(3)).unwrap();
            txn.header_mut().sequence = 10;
            txn.edit_entity_header(|header| {
                header.pages = 1;
                header.checkpoint = Some("cp".into());
                Ok(())
            })
            .unwrap();
            assert_eq!(
                txn.record_keys(),
                BTreeSet::from(["old".into(), "new".into()])
            );
            assert_eq!(txn.metadata_keys(), BTreeSet::from([identity.clone()]));
            assert_eq!(txn.prior_record("old"), before.records.get("old"));
            assert_eq!(
                txn.prior_metadata(&identity),
                before.entity.as_ref().unwrap().metadata.get(&identity)
            );
        }
        assert_eq!(ledger, before);
    }
    #[test]
    fn si_y01_transaction_panic_restores_and_commit_keeps_state() {
        let mut ledger = ledger(10);
        let before = ledger.clone();
        let identity = ("items".into(), "r0000".into());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut txn = LedgerTxn::new(&mut ledger, true);
            txn.record_mut("old").unwrap().due_ms = 9;
            txn.metadata_mut(&identity).unwrap().tombstone = Some(FieldStamp {
                timestamp_ms: 2,
                authoritative: false,
            });
            panic!("task-owned planner fault");
        }));
        assert!(result.is_err());
        assert_eq!(ledger, before);
        let mut txn = LedgerTxn::new(&mut ledger, true);
        txn.header_mut().last_ms = 2;
        assert!(txn.changed());
        let committed = txn.commit();
        assert_eq!(committed.header.last_ms, 2);
    }
    #[test]
    fn si_y01_transaction_clones_first_image_once_and_compares_touched_only() {
        for m in [10, 1000] {
            let mut ledger = ledger(m);
            let identity = ("items".into(), "r0000".into());
            counters::reset();
            let mut txn = LedgerTxn::new(&mut ledger, true);
            txn.metadata_mut(&identity)
                .unwrap()
                .fields
                .get_mut("body")
                .unwrap()
                .timestamp_ms = 2;
            txn.metadata_mut(&identity)
                .unwrap()
                .fields
                .get_mut("body")
                .unwrap()
                .timestamp_ms = 3;
            assert_eq!(counters::snapshot().ledger_entries_cloned, 1);
            counters::reset();
            assert_eq!(txn.changed_metadata(), BTreeSet::from([identity]));
            assert_eq!(counters::snapshot().metadata_entries_visited, 1);
        }
    }
    #[test]
    fn si_y01_transaction_net_noop_and_entity_adoption_restore() {
        let mut ledger = ledger(10);
        {
            let mut txn = LedgerTxn::new(&mut ledger, true);
            txn.insert_record("new".into(), record("new"));
            txn.remove_record("new");
            assert!(!txn.changed());
        }
        ledger.entity = None;
        {
            let mut txn = LedgerTxn::new(&mut ledger, true);
            txn.adopt_entity(EntityLedger::new(EntityConfig {
                scope: "new".into(),
                tables: BTreeSet::new(),
            }))
            .unwrap();
            assert!(txn.changed());
        }
        assert!(ledger.entity.is_none());
    }
}
