//! API-0005 / TC-0029 R11: narrow projected keys and requested cells.
use super::SnapshotStorage;
use crate::storage::*;

impl SnapshotStorage {
    fn projected_cell(&self, source: &Row, field: &ResolvedField) -> Result<Value, StorageError> {
        let target = if let Some(key) = &field.foreign_key {
            let Some(Value::Reference(id)) = source.fields.get(key) else {
                return Ok(Value::Null);
            };
            let Some(row) = self.require_table(&field.table)?.get(id) else {
                return Ok(Value::Null);
            };
            row
        } else {
            source
        };
        if field.name == "id" {
            return Ok(Value::String(target.id.clone()));
        }
        match target.fields.get(&field.name) {
            None | Some(Value::Null) if field.column.optional => Ok(Value::Null),
            Some(value) if value_has_type(value, &field.column.kind) => Ok(value.clone()),
            _ => Err(StorageError::InvalidRow {
                table: field.table.clone(),
                id: target.id.clone(),
                message: "invalid requested projected cell".into(),
            }),
        }
    }
}

impl ObservationStorage for SnapshotStorage {
    async fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> Result<Vec<ProjectedRow>, StorageError> {
        validate_query(schema, query)?;
        let resolved: Vec<_> = fields
            .iter()
            .map(|path| resolve_field(schema, &query.table, path))
            .collect::<Result<_, _>>()?;
        self.require_table(&query.table)?;
        for field in &resolved {
            self.require_table(&field.table)?;
        }
        let order = query
            .order
            .as_ref()
            .map(|o| resolve_field(schema, &query.table, &o.field))
            .transpose()?;
        self.select_rows(schema, query)?
            .into_iter()
            .map(|row| {
                let values = fields
                    .iter()
                    .zip(&resolved)
                    .map(|(path, field)| Ok((path.clone(), self.projected_cell(row, field)?)))
                    .collect::<Result<_, StorageError>>()?;
                Ok(ProjectedRow {
                    id: row.id.clone(),
                    order_key: order
                        .as_ref()
                        .map(|field| self.field(row, field).unwrap_or(Value::Null)),
                    values,
                })
            })
            .collect()
    }
}
