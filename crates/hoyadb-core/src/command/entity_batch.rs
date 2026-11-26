//! SPEC-0048: temporary batch aliases never become stored identities.
use super::*;
use crate::{model::Write, storage::*};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct AliasedWrite {
    pub write: Write,
    /// Only valid with Write::Replace: merge supplied fields and retain partial intent.
    pub patch: bool,
    pub proposed_id: Option<String>,
    pub alias: Option<String>,
    pub references: BTreeMap<String, String>,
}

pub struct AliasedBatchResult {
    pub changes: Vec<CommittedChange>,
    pub aliases: BTreeMap<String, String>,
}

impl<S: SyncStorage> CommandDatabase<S> {
    pub async fn apply_entity_batch_aliased(
        &mut self,
        writes: Vec<AliasedWrite>,
    ) -> Result<AliasedBatchResult, CommandError> {
        self.entity_batch_staged(writes).await
    }

    pub(super) fn validate_batch_aliases(
        &self,
        writes: &[AliasedWrite],
    ) -> Result<(), CommandError> {
        let mut names = BTreeSet::new();
        for item in writes {
            if item.patch && !matches!(item.write, Write::Replace { .. }) {
                return Err(CommandError::InvalidCommand);
            }
            if let Some(alias) = &item.alias {
                let Write::Insert {
                    table,
                    ..
                } = &item.write
                else {
                    return Err(CommandError::InvalidCommand);
                };
                if alias.is_empty() || !self.is_entity_table(table) || !names.insert(alias) {
                    return Err(CommandError::InvalidCommand);
                }
            }
        }
        Ok(())
    }

    pub(super) fn resolve_batch_references(
        &self,
        write: &mut Write,
        references: &BTreeMap<String, String>,
        aliases: &BTreeMap<String, (String, String)>,
    ) -> Result<(), CommandError> {
        if references.is_empty() {
            return Ok(());
        }
        let (table, fields) = match write {
            Write::Insert {
                table,
                fields,
            }
            | Write::Replace {
                table,
                fields,
                ..
            } => (table, fields),
            Write::Remove {
                ..
            } => return Err(CommandError::InvalidCommand),
        };
        let definition = self
            .db
            .db
            .schema
            .tables
            .get(table)
            .ok_or(CommandError::InvalidCommand)?;
        for (field, alias) in references {
            let (target, id) = aliases.get(alias).ok_or(CommandError::InvalidCommand)?;
            if fields.contains_key(field)
                || !definition
                    .columns
                    .get(field)
                    .is_some_and(|column| column.kind == ColumnType::Reference)
                || definition.relations.values().any(|relation| {
                    matches!(relation,
                    crate::model::RelationDefinition::ToOne { table, foreign_key, .. }
                    if foreign_key == field && table != target)
                })
            {
                return Err(CommandError::InvalidCommand);
            }
            fields.insert(field.clone(), Value::Reference(id.clone()));
        }
        Ok(())
    }
}
