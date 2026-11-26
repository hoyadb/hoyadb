//! API-0011: host-supplied insertion candidates keep allocation in the core.
use super::*;
use crate::{model::Write, storage::*};
use std::collections::BTreeMap;

impl<S: SyncStorage> CommandDatabase<S> {
    pub async fn apply_batch_proposed(
        &mut self,
        writes: Vec<(Write, Option<String>)>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        if self.ledger.entity.is_some() {
            return self.entity_apply_batch_proposed(writes).await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.apply_batch_proposed(writes).await?)
    }

    pub async fn insert_proposed(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, CommandError> {
        self.apply_batch_proposed(vec![(
            Write::Insert {
                table: table.into(),
                fields,
            },
            proposed_id,
        )])
        .await?
        .into_iter()
        .next()
        .and_then(|change| change.after)
        .ok_or(CommandError::Protocol)
    }

    pub async fn scope_apply_batch_proposed(
        &mut self,
        request: CommandRequest,
        writes: Vec<(Write, Option<String>)>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        Ok(self.staged(request)?.apply_batch_proposed(writes).await?)
    }

    pub async fn scope_insert_proposed(
        &mut self,
        request: CommandRequest,
        table: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, CommandError> {
        self.scope_apply_batch_proposed(
            request,
            vec![(
                Write::Insert {
                    table: table.into(),
                    fields,
                },
                proposed_id,
            )],
        )
        .await?
        .into_iter()
        .next()
        .and_then(|change| change.after)
        .ok_or(CommandError::Protocol)
    }

    pub async fn link_proposed(
        &mut self,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, CommandError> {
        if self.ledger.entity.is_some() {
            return self
                .entity_link_proposed(table, id, relation, target, fields, proposed_id)
                .await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self
            .db
            .link_proposed(table, id, relation, target, fields, proposed_id)
            .await?)
    }

    pub async fn scope_link_proposed(
        &mut self,
        request: CommandRequest,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
        input: (BTreeMap<String, Value>, Option<String>),
    ) -> Result<Row, CommandError> {
        Ok(self
            .staged(request)?
            .link_proposed(table, id, relation, target, input.0, input.1)
            .await?)
    }
}
