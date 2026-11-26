//! Private format-4 envelope for durable entity aliases over the format-3 ledger.
use super::{
    Json, SyncState,
    entity::{EntityLedger, header_from_json},
    records::{SyncLedger, decode_header, decode_packet, entity_header_row, identity_metadata_id},
};
use crate::storage::{Row, StorageError, Value, schema_conflict};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct AliasState {
    pub enabled: bool,
    pub mappings: BTreeMap<(String, String), String>,
    targets: BTreeMap<(String, String), BTreeSet<String>>,
    sources: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct StoredLedger {
    pub ledger: SyncLedger,
    pub aliases: AliasState,
}

impl StoredLedger {
    pub(crate) fn from_rows(rows: &[Row]) -> Result<Self, StorageError> {
        let header = rows
            .iter()
            .find(|row| row.id == "entity:state")
            .map(decode_entity_header)
            .transpose()?;
        if !header.as_ref().is_some_and(|(_, enabled)| *enabled) {
            return Ok(Self {
                ledger: SyncLedger::from_rows(rows)?,
                aliases: AliasState::default(),
            });
        }
        let (entity, _) = header.ok_or_else(invalid)?;
        let state = rows
            .iter()
            .find(|row| row.id == "state")
            .ok_or_else(invalid)?;
        let namespace = decode_header(&decode_packet(state)?)?.namespace;
        let mut aliases = AliasState {
            enabled: true,
            ..AliasState::default()
        };
        let mut ids = BTreeSet::new();
        let mut projected = Vec::with_capacity(rows.len());
        for row in rows {
            if !ids.insert(&row.id) {
                return Err(invalid());
            }
            // Validate the original envelope before changing or separating any packet.
            decode_packet(row)?;
            if row.id.starts_with("entity:alias:") {
                let (identity, target) =
                    validate_alias_packet(row, &namespace, &entity.config.tables)?;
                if aliases.insert(identity, target).is_some() {
                    return Err(invalid());
                }
            } else if row.id == "entity:state" {
                projected.push(entity_header_row(&entity)?);
            } else {
                crate::storage::count_work!(rows_cloned, 1);
                projected.push(row.clone());
            }
        }
        let ledger = SyncLedger::from_rows(&projected)?;
        aliases.validate(&ledger)?;
        Ok(Self {
            ledger,
            aliases,
        })
    }
}

impl AliasState {
    pub(crate) fn validate(&self, ledger: &SyncLedger) -> Result<(), StorageError> {
        if !self.enabled {
            return if self.mappings.is_empty() && self.targets.is_empty() && self.sources.is_empty()
            {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        let entity = ledger
            .entity
            .as_ref()
            .filter(|entity| entity.ledger_format == 3)
            .ok_or_else(invalid)?;
        let mut indexed = Self {
            enabled: true,
            ..Self::default()
        };
        for ((table, local), target) in &self.mappings {
            validate_values(table, local, target, &ledger.header.namespace)?;
            if !entity.config.tables.contains(table) {
                return Err(invalid());
            }
            alias_packet(table, local, target)?;
            indexed.insert((table.clone(), local.clone()), target.clone());
        }
        if indexed.targets != self.targets
            || indexed.sources != self.sources
            || self.targets.values().any(|owners| owners.len() != 1)
            || self.sources.values().any(|tables| tables.len() != 1)
        {
            return Err(invalid());
        }
        // Mark completed paths so complete validation visits each alias once.
        let mut completed = BTreeSet::new();
        for identity in self.mappings.keys() {
            let mut current = identity.clone();
            let mut path = BTreeSet::new();
            while !completed.contains(&current) {
                if !path.insert(current.clone()) {
                    return Err(invalid());
                }
                let Some(target) = self.mappings.get(&current) else {
                    break;
                };
                current.1 = target.clone();
            }
            completed.extend(path);
        }
        for record in ledger.records.values() {
            for (local, table) in &record.created {
                if self
                    .sources
                    .get(local)
                    .is_some_and(|owners| owners.iter().any(|owner| owner != table))
                {
                    return Err(invalid());
                }
                if let Some(target) = self.mappings.get(&(table.clone(), local.clone())) {
                    validate_producer(record, local, table, target)?;
                }
                if record.entity.is_some()
                    && successful(record)
                    && let Some(target) = record.mappings.get(local)
                    && target != local
                    && self.mappings.get(&(table.clone(), local.clone())) != Some(target)
                {
                    return Err(invalid());
                }
            }
            for (local, target) in &record.mappings {
                let table = record.created.get(local).ok_or_else(invalid)?;
                if self
                    .targets
                    .get(&(table.clone(), target.clone()))
                    .is_some_and(|owners| owners.iter().any(|owner| owner != local))
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_entry(
        &self,
        ledger: &SyncLedger,
        table: &str,
        local: &str,
        target: &str,
    ) -> Result<(), StorageError> {
        let entity = ledger
            .entity
            .as_ref()
            .filter(|entity| self.enabled && entity.ledger_format == 3)
            .ok_or_else(invalid)?;
        validate_values(table, local, target, &ledger.header.namespace)?;
        alias_packet(table, local, target)?;
        if !entity.config.tables.contains(table)
            || self
                .mappings
                .get(&(table.into(), local.into()))
                .is_some_and(|old| old != target)
            || self
                .targets
                .get(&(table.into(), target.into()))
                .is_some_and(|owners| owners.iter().any(|owner| owner != local))
            || self
                .sources
                .get(local)
                .is_some_and(|tables| tables.iter().any(|owner| owner != table))
        {
            return Err(invalid());
        }
        let mut current = target;
        let mut visited = BTreeSet::new();
        loop {
            if current == local || !visited.insert(current) {
                return Err(invalid());
            }
            let Some(next) = self.mappings.get(&(table.into(), current.into())) else {
                break;
            };
            current = next;
        }
        for record in ledger.records.values() {
            if let Some(owner) = record.created.get(local) {
                if owner != table {
                    return Err(invalid());
                }
                validate_producer(record, local, table, target)?;
            }
            if record.mappings.iter().any(|(source, remote)| {
                source != local
                    && remote == target
                    && record
                        .created
                        .get(source)
                        .is_some_and(|owner| owner == table)
            }) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(crate) fn insert(&mut self, identity: (String, String), target: String) -> Option<String> {
        let previous = self.remove(&identity);
        self.sources
            .entry(identity.1.clone())
            .or_default()
            .insert(identity.0.clone());
        self.targets
            .entry((identity.0.clone(), target.clone()))
            .or_default()
            .insert(identity.1.clone());
        self.mappings.insert(identity, target);
        previous
    }

    pub(crate) fn remove(&mut self, identity: &(String, String)) -> Option<String> {
        let target = self.mappings.remove(identity)?;
        let target_identity = (identity.0.clone(), target.clone());
        if let Some(owners) = self.targets.get_mut(&target_identity) {
            owners.remove(&identity.1);
            if owners.is_empty() {
                self.targets.remove(&target_identity);
            }
        }
        if let Some(tables) = self.sources.get_mut(&identity.1) {
            tables.remove(&identity.0);
            if tables.is_empty() {
                self.sources.remove(&identity.1);
            }
        }
        Some(target)
    }

    pub(crate) fn clear(&mut self) {
        self.mappings.clear();
        self.targets.clear();
        self.sources.clear();
    }

    pub(crate) fn is_target(&self, table: &str, id: &str) -> bool {
        self.enabled && self.targets.contains_key(&(table.into(), id.into()))
    }

    pub(crate) fn resolve(&self, table: Option<&str>, id: &str) -> Result<String, StorageError> {
        if !self.enabled {
            return Ok(id.into());
        }
        let Some(tables) = self.sources.get(id) else {
            return Ok(id.into());
        };
        if tables.len() != 1 {
            return Err(invalid());
        }
        let owner = tables.first().ok_or_else(invalid)?;
        if let Some(table) = table
            && table != owner
        {
            // IDs are table-scoped: another table's alias source may be this
            // table's already accepted canonical identity.
            return if self.is_target(table, id) {
                Ok(id.into())
            } else {
                Err(invalid())
            };
        }
        let mut current = id;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current) {
                return Err(invalid());
            }
            let Some(target) = self.mappings.get(&(owner.clone(), current.into())) else {
                break;
            };
            current = target;
        }
        Ok(current.into())
    }
}

pub(crate) fn stored_rows(
    ledger: &SyncLedger,
    aliases: &AliasState,
) -> Result<Vec<Row>, StorageError> {
    aliases.validate(ledger)?;
    let mut rows = ledger.to_rows()?;
    if aliases.enabled {
        let header = rows
            .iter_mut()
            .find(|row| row.id == "entity:state")
            .ok_or_else(invalid)?;
        *header = stored_entity_header(ledger.entity.as_ref().ok_or_else(invalid)?, true)?;
        for ((table, local), target) in &aliases.mappings {
            rows.push(alias_packet(table, local, target)?);
        }
    }
    Ok(rows)
}

pub(crate) fn alias_packet(table: &str, local: &str, target: &str) -> Result<Row, StorageError> {
    let namespace = source_namespace(local).ok_or_else(invalid)?;
    validate_values(table, local, target, namespace)?;
    packet(
        alias_packet_id(table, local)?,
        Json::Object(BTreeMap::from([
            ("table".into(), Json::String(table.into())),
            ("local_id".into(), Json::String(local.into())),
            ("canonical_id".into(), Json::String(target.into())),
        ])),
    )
}

pub(crate) fn validate_alias_packet(
    row: &Row,
    namespace: &str,
    tables: &BTreeSet<String>,
) -> Result<((String, String), String), StorageError> {
    let Json::Object(fields) = decode_packet(row)? else {
        return Err(invalid());
    };
    if fields.len() != 3 {
        return Err(invalid());
    }
    let text = |key| match fields.get(key) {
        Some(Json::String(value)) => Ok(value.as_str()),
        _ => Err(invalid()),
    };
    let table = text("table")?;
    let local = text("local_id")?;
    let target = text("canonical_id")?;
    validate_values(table, local, target, namespace)?;
    if !tables.contains(table) || row.id != alias_packet_id(table, local)? {
        return Err(invalid());
    }
    Ok(((table.into(), local.into()), target.into()))
}

pub(crate) fn decode_entity_header(row: &Row) -> Result<(EntityLedger, bool), StorageError> {
    if row.id != "entity:state" {
        return Err(invalid());
    }
    let mut json = decode_packet(row)?;
    let enabled = matches!(&json, Json::Object(fields) if fields.get("ledger_format") == Some(&Json::Integer(4)));
    if enabled && let Json::Object(fields) = &mut json {
        fields.insert("ledger_format".into(), Json::Integer(3));
    }
    Ok((header_from_json(&json)?, enabled))
}

pub(crate) fn stored_entity_header(
    entity: &EntityLedger,
    enabled: bool,
) -> Result<Row, StorageError> {
    let row = entity_header_row(entity)?;
    if !enabled {
        return Ok(row);
    }
    if entity.ledger_format != 3 {
        return Err(invalid());
    }
    let Json::Object(mut json) = decode_packet(&row)? else {
        return Err(invalid());
    };
    json.insert("ledger_format".into(), Json::Integer(4));
    packet(row.id, Json::Object(json))
}

pub(crate) fn alias_packet_id(table: &str, local: &str) -> Result<String, StorageError> {
    crate::model::check_name(table).map_err(|_| invalid())?;
    if source_namespace(local).is_none() {
        return Err(invalid());
    }
    let metadata_id = identity_metadata_id(table, local)?;
    Ok(metadata_id.replacen("entity:metadata:", "entity:alias:", 1))
}

fn packet(id: String, data: Json) -> Result<Row, StorageError> {
    let data = data.canonical().map_err(|_| invalid())?;
    crate::storage::count_work!(packets_encoded, 1);
    Ok(Row {
        id,
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(2)),
            ("data".into(), Value::String(data)),
        ]),
    })
}

fn source_namespace(local: &str) -> Option<&str> {
    let (namespace, counter) = local.strip_prefix("entity:")?.split_once(':')?;
    let lower_hex = |byte: u8| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte);
    (namespace.len() == 32
        && namespace.bytes().all(lower_hex)
        && counter.len() == 16
        && counter.bytes().all(lower_hex)
        && i64::from_str_radix(counter, 16).is_ok_and(|value| value > 0))
    .then_some(namespace)
}

fn validate_values(
    table: &str,
    local: &str,
    target: &str,
    namespace: &str,
) -> Result<(), StorageError> {
    crate::model::check_name(table).map_err(|_| invalid())?;
    if source_namespace(local) != Some(namespace)
        || target.is_empty()
        || target.starts_with("local:")
        || local == target
    {
        return Err(invalid());
    }
    Ok(())
}

fn successful(record: &super::records::SyncRecord) -> bool {
    record.reconciled && matches!(record.state, SyncState::Applied | SyncState::Corrected)
}

fn validate_producer(
    record: &super::records::SyncRecord,
    local: &str,
    table: &str,
    target: &str,
) -> Result<(), StorageError> {
    if !successful(record)
        || !record
            .entity
            .as_ref()
            .is_some_and(|entity| entity.table == table)
        || record.mappings.get(local).map(String::as_str) != Some(target)
    {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> StorageError {
    schema_conflict("invalid durable entity alias state")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        EntityConfig, EntityDescriptor, EntityOperation, RemoteOutcome, SyncOptions, SyncState,
        records::{SyncHeader, SyncRecord, decode_packet, identity_metadata_id},
    };

    fn local(counter: i64) -> String {
        format!("entity:{}:{counter:016x}", "0".repeat(32))
    }

    fn ledger() -> SyncLedger {
        SyncLedger {
            entity: Some(EntityLedger::new(EntityConfig {
                scope: "account".into(),
                tables: BTreeSet::from(["items".into(), "parents".into()]),
            })),
            header: SyncHeader {
                namespace: "0".repeat(32),
                sequence: 2,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        }
    }

    fn aliases() -> AliasState {
        with_mappings(BTreeMap::from([(
            ("items".into(), local(1)),
            "canonical".into(),
        )]))
    }

    fn with_mappings(mappings: BTreeMap<(String, String), String>) -> AliasState {
        let mut aliases = AliasState {
            enabled: true,
            ..AliasState::default()
        };
        for (identity, target) in mappings {
            aliases.insert(identity, target);
        }
        aliases
    }

    fn raw_packet(id: String, data: Json) -> Row {
        Row {
            id,
            fields: BTreeMap::from([
                ("format".into(), Value::Integer(2)),
                ("data".into(), Value::String(data.canonical().unwrap())),
            ]),
        }
    }

    fn alias_fixture(table: &str, source: &str, target: &str) -> Row {
        let identity = identity_metadata_id(table, source).unwrap();
        raw_packet(
            identity.replacen("entity:metadata:", "entity:alias:", 1),
            Json::Object(BTreeMap::from([
                ("table".into(), Json::String(table.into())),
                ("local_id".into(), Json::String(source.into())),
                ("canonical_id".into(), Json::String(target.into())),
            ])),
        )
    }

    fn fixture_rows(ledger: &SyncLedger, aliases: &AliasState) -> Vec<Row> {
        let mut rows = ledger.to_rows().unwrap();
        if aliases.enabled {
            let row = rows
                .iter_mut()
                .find(|row| row.id == "entity:state")
                .unwrap();
            let Json::Object(mut header) = decode_packet(row).unwrap() else {
                panic!("entity header object");
            };
            header.insert("ledger_format".into(), Json::Integer(4));
            *row = raw_packet(row.id.clone(), Json::Object(header));
        }
        rows.extend(
            aliases
                .mappings
                .iter()
                .map(|((table, source), target)| alias_fixture(table, source, target)),
        );
        rows
    }

    fn producer(source: &str, target: &str) -> SyncRecord {
        let mut record = SyncRecord {
            entity: Some(EntityDescriptor {
                table: "items".into(),
                op: EntityOperation::Create,
                timestamp_ms: 100,
                removed_fields: BTreeSet::new(),
            }),
            metadata_undo: vec![],
            key: format!("{:016x}-{}-{:016x}", 100, "0".repeat(32), 1),
            command: String::new(),
            args: Json::Object(BTreeMap::from([
                ("recordId".into(), Json::String(source.into())),
                ("changes".into(), Json::Object(BTreeMap::new())),
            ])),
            context: Json::Object(BTreeMap::from([
                ("identity".into(), Json::Null),
                (
                    "capturedAt".into(),
                    Json::String("1970-01-01T00:00:00.100Z".into()),
                ),
            ])),
            schema_version: 1,
            order: 1,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: BTreeMap::from([(source.into(), "items".into())]),
            mappings: BTreeMap::from([(source.into(), target.into())]),
            undo: vec![],
            state: SyncState::Applied,
            frozen: None,
            attempts: 1,
            due_ms: 0,
            outcome: Some(RemoteOutcome::Applied(Json::Null)),
            rolled_back: false,
            reconciled: true,
        };
        record.frozen = Some(record.envelope().canonical().unwrap());
        record
    }

    #[test]
    fn legacy_formats_round_trip_without_marker_or_packet_changes() {
        for format in [None, Some(2), Some(3)] {
            let mut expected = ledger();
            match format {
                Some(format) => expected.entity.as_mut().unwrap().ledger_format = format,
                None => expected.entity = None,
            }
            let rows = expected.to_rows().unwrap();
            let actual = StoredLedger::from_rows(&rows).unwrap();
            assert!(actual.ledger == expected);
            assert!(actual.aliases == AliasState::default());
            assert_eq!(stored_rows(&actual.ledger, &actual.aliases).unwrap(), rows);
        }
    }

    #[test]
    fn format_four_retains_mode_with_and_without_aliases() {
        for aliases in [
            aliases(),
            AliasState {
                enabled: true,
                ..AliasState::default()
            },
        ] {
            let expected = ledger();
            let rows = fixture_rows(&expected, &aliases);
            assert!(SyncLedger::from_rows(&rows).is_err());
            let actual = StoredLedger::from_rows(&rows).unwrap();
            assert!(actual.ledger == expected);
            assert!(actual.aliases == aliases);
            assert_eq!(stored_rows(&actual.ledger, &actual.aliases).unwrap(), rows);
            let header = rows.iter().find(|row| row.id == "entity:state").unwrap();
            let (projected, enabled) = decode_entity_header(header).unwrap();
            assert_eq!(projected.ledger_format, 3);
            assert!(enabled);
        }
    }

    #[test]
    fn alias_packet_matches_canonical_identity_and_exact_envelope() {
        let row = alias_packet("items", &local(1), "canonical").unwrap();
        assert_eq!(row, alias_fixture("items", &local(1), "canonical"));
        let namespace = "0".repeat(32);
        let tables = BTreeSet::from(["items".into()]);
        assert_eq!(
            validate_alias_packet(&row, &namespace, &tables).unwrap(),
            (("items".into(), local(1)), "canonical".into())
        );
        let mut corrupt = row.clone();
        corrupt.id.push('0');
        assert!(validate_alias_packet(&corrupt, &namespace, &tables).is_err());
        let mut corrupt = row.clone();
        corrupt.fields.insert("format".into(), Value::Integer(1));
        assert!(validate_alias_packet(&corrupt, &namespace, &tables).is_err());
        let mut corrupt = row.clone();
        if let Some(Value::String(data)) = corrupt.fields.get_mut("data") {
            data.push(' ');
        }
        assert!(validate_alias_packet(&corrupt, &namespace, &tables).is_err());
        let mut corrupt = row;
        let Json::Object(mut data) = decode_packet(&corrupt).unwrap() else {
            panic!()
        };
        data.insert("extra".into(), Json::Null);
        corrupt.fields.insert(
            "data".into(),
            Value::String(Json::Object(data).canonical().unwrap()),
        );
        assert!(validate_alias_packet(&corrupt, &namespace, &tables).is_err());
    }

    #[test]
    fn raw_inventory_rejects_duplicates_mixed_formats_and_unknown_packets() {
        let ledger = ledger();
        let rows = fixture_rows(&ledger, &aliases());
        for index in 0..rows.len() {
            let mut duplicate = rows.clone();
            duplicate.push(rows[index].clone());
            assert!(StoredLedger::from_rows(&duplicate).is_err());
        }
        for format in [2, 3] {
            let mut old = ledger.clone();
            old.entity.as_mut().unwrap().ledger_format = format;
            let mut mixed = old.to_rows().unwrap();
            mixed.push(alias_fixture("items", &local(1), "canonical"));
            assert!(StoredLedger::from_rows(&mixed).is_err());
        }
        let without_header = rows
            .iter()
            .filter(|row| row.id != "entity:state")
            .cloned()
            .collect::<Vec<_>>();
        assert!(StoredLedger::from_rows(&without_header).is_err());
        let mut unknown = rows;
        unknown.push(raw_packet("entity:unknown".into(), Json::Null));
        assert!(StoredLedger::from_rows(&unknown).is_err());
    }

    #[test]
    fn raw_header_projection_does_not_accept_extra_fields_or_unknown_formats() {
        let rows = fixture_rows(&ledger(), &aliases());
        let header = rows.iter().find(|row| row.id == "entity:state").unwrap();
        for format in [0, 1, 2, 5] {
            let Json::Object(mut data) = decode_packet(header).unwrap() else {
                panic!()
            };
            data.insert("ledger_format".into(), Json::Integer(format));
            assert!(
                decode_entity_header(&raw_packet(header.id.clone(), Json::Object(data))).is_err()
            );
        }
        let Json::Object(mut data) = decode_packet(header).unwrap() else {
            panic!()
        };
        data.insert("extra".into(), Json::Null);
        assert!(decode_entity_header(&raw_packet(header.id.clone(), Json::Object(data))).is_err());
        let mut old = ledger();
        old.entity.as_mut().unwrap().ledger_format = 2;
        let old_rows = old.to_rows().unwrap();
        let old_header = old_rows
            .iter()
            .find(|row| row.id == "entity:state")
            .unwrap();
        let (decoded, enabled) = decode_entity_header(old_header).unwrap();
        assert_eq!(decoded.ledger_format, 2);
        assert!(!enabled);
    }

    #[test]
    fn aliases_require_owned_generated_sources_and_configured_tables() {
        let ledger = ledger();
        let invalid_sources = [
            "canonical".into(),
            "local:x".into(),
            local(0),
            format!("entity:{}:0000000000000001", "1".repeat(32)),
            format!("entity:{}:000000000000000A", "0".repeat(32)),
            format!("entity:{}:8000000000000000", "0".repeat(32)),
        ];
        for source in invalid_sources {
            let state = with_mappings(BTreeMap::from([(
                ("items".into(), source),
                "canonical".into(),
            )]));
            assert!(state.validate(&ledger).is_err());
        }
        let mut state = with_mappings(BTreeMap::from([(
            ("missing".into(), local(1)),
            "canonical".into(),
        )]));
        assert!(state.validate(&ledger).is_err());
        // Entity IDs use the model allocator, independently of command sequence.
        state = with_mappings(BTreeMap::from([(
            ("items".into(), local(100)),
            "canonical".into(),
        )]));
        state.validate(&ledger).unwrap();
    }

    #[test]
    fn aliases_reject_invalid_targets_cycles_and_same_table_collisions() {
        let ledger = ledger();
        for target in [String::new(), "local:pending".into(), local(1)] {
            let mut state = aliases();
            state.insert(("items".into(), local(1)), target);
            assert!(state.validate(&ledger).is_err());
        }
        let mut state = aliases();
        state.insert(("items".into(), local(2)), "canonical".into());
        assert!(state.validate(&ledger).is_err());
        state = with_mappings(BTreeMap::from([
            (("items".into(), local(1)), local(2)),
            (("items".into(), local(2)), local(1)),
        ]));
        assert!(state.validate(&ledger).is_err());
        state.insert(("items".into(), local(2)), "canonical".into());
        state.validate(&ledger).unwrap();
        state = with_mappings(BTreeMap::from([
            (("items".into(), local(1)), "canonical".into()),
            (("parents".into(), local(2)), "canonical".into()),
        ]));
        state.validate(&ledger).unwrap();
        state.insert(
            ("items".into(), local(1)),
            format!("entity:{}:0000000000000001", "1".repeat(32)),
        );
        state.validate(&ledger).unwrap();
    }

    #[test]
    fn mode_requires_a_format_three_base_and_disabled_state_cannot_hold_aliases() {
        let mut ledger = ledger();
        let mut state = aliases();
        state.enabled = false;
        assert!(state.validate(&ledger).is_err());
        state.enabled = true;
        ledger.entity.as_mut().unwrap().ledger_format = 2;
        assert!(state.validate(&ledger).is_err());
        ledger.entity = None;
        assert!(state.validate(&ledger).is_err());
    }

    #[test]
    fn aliases_match_retained_successful_producers_and_survive_their_removal() {
        let mut ledger = ledger();
        let record = producer(&local(1), "canonical");
        ledger.records.insert(record.key.clone(), record);
        ledger.validate().unwrap();
        let mut state = aliases();
        state.validate(&ledger).unwrap();
        state.insert(("items".into(), local(1)), "different".into());
        assert!(state.validate(&ledger).is_err());
        state.clear();
        assert!(state.validate(&ledger).is_err());
        state = aliases();
        ledger.records.clear();
        state.validate(&ledger).unwrap();
        let rows = fixture_rows(&ledger, &state);
        assert!(StoredLedger::from_rows(&rows).unwrap().aliases == state);
    }

    #[test]
    fn identity_mappings_do_not_require_or_allow_alias_packets() {
        let mut ledger = ledger();
        let record = producer(&local(1), &local(1));
        ledger.records.insert(record.key.clone(), record);
        ledger.validate().unwrap();
        let mut state = AliasState {
            enabled: true,
            ..AliasState::default()
        };
        state.validate(&ledger).unwrap();
        state.insert(("items".into(), local(1)), local(1));
        assert!(state.validate(&ledger).is_err());
        state = with_mappings(BTreeMap::from([(("items".into(), local(2)), local(1))]));
        assert!(state.validate(&ledger).is_err());
    }

    #[test]
    fn indexed_mutations_preserve_collision_checks_through_rollback_and_clear() {
        let ledger = ledger();
        let first = ("items".into(), local(1));
        let second = ("items".into(), local(2));
        let mut state = AliasState {
            enabled: true,
            ..AliasState::default()
        };
        state
            .validate_entry(&ledger, "items", &local(1), "canonical")
            .unwrap();
        assert_eq!(state.insert(first.clone(), "canonical".into()), None);
        state
            .validate_entry(&ledger, "items", &local(1), "canonical")
            .unwrap();
        assert!(
            state
                .validate_entry(&ledger, "items", &local(1), "different")
                .is_err()
        );
        assert!(
            state
                .validate_entry(&ledger, "items", &local(2), "canonical")
                .is_err()
        );
        assert_eq!(state.remove(&first), Some("canonical".into()));
        state
            .validate_entry(&ledger, "items", &local(2), "canonical")
            .unwrap();
        state.insert(second.clone(), "canonical".into());
        assert!(
            state
                .validate_entry(&ledger, "items", &local(1), "canonical")
                .is_err()
        );
        state.remove(&second);
        state.insert(first, "canonical".into());
        state.validate(&ledger).unwrap();
        state.clear();
        assert!(state.enabled);
        assert!(state.mappings.is_empty());
        state
            .validate_entry(&ledger, "items", &local(2), "canonical")
            .unwrap();
    }

    #[test]
    fn resolution_follows_chains_preserves_unknowns_and_rejects_wrong_table_sources() {
        let mut state = aliases();
        state.insert(("items".into(), local(1)), local(2));
        state.insert(("items".into(), local(2)), "canonical".into());
        assert_eq!(
            state.resolve(Some("items"), &local(1)).unwrap(),
            "canonical"
        );
        assert_eq!(state.resolve(None, &local(1)).unwrap(), "canonical");
        assert_eq!(state.resolve(Some("items"), "unknown").unwrap(), "unknown");
        assert_eq!(state.resolve(None, "catalogue:1").unwrap(), "catalogue:1");
        assert!(state.resolve(Some("parents"), &local(1)).is_err());
        state.insert(("parents".into(), local(1)), "other".into());
        assert!(state.validate(&ledger()).is_err());
        assert!(state.resolve(None, &local(1)).is_err());
    }

    #[test]
    fn retained_unreconciled_source_cannot_be_reassigned_to_another_table() {
        let mut ledger = ledger();
        let mut record = producer(&local(1), "canonical");
        record.state = SyncState::Queued;
        record.mappings.clear();
        record.frozen = None;
        record.attempts = 0;
        record.outcome = None;
        record.reconciled = false;
        ledger.records.insert(record.key.clone(), record);
        ledger.validate().unwrap();
        let state = with_mappings(BTreeMap::from([(
            ("parents".into(), local(1)),
            "canonical".into(),
        )]));
        assert!(
            state
                .validate_entry(&ledger, "parents", &local(1), "canonical")
                .is_err()
        );
        assert!(state.validate(&ledger).is_err());
    }
}
