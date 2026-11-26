//! API-0005 / TC-0029 R11: SQL selects keys, then decodes only consulted cells.
use super::{
    SqliteStorage, backend, decode_value, query, require_query_tables, require_table, row_exists,
};
use hoyadb_core::storage::*;
use rusqlite::{Connection, params, params_from_iter};
use std::future::{Future, ready};

fn field_value(
    connection: &Connection,
    table: &str,
    id: &str,
    name: &str,
) -> Result<Option<Value>, StorageError> {
    let mut statement=connection.prepare("SELECT kind,text_value,int_value FROM __hoya_fields WHERE table_name=?1 AND row_id=?2 AND name=?3").map_err(backend)?;
    let mut cursor = statement.query(params![table, id, name]).map_err(backend)?;
    let Some(row) = cursor.next().map_err(backend)? else {
        return Ok(None);
    };
    decode_value(
        row.get_ref(0).map_err(backend)?,
        row.get_ref(1).map_err(backend)?,
        row.get_ref(2).map_err(backend)?,
    )
    .map(Some)
}

fn cell(
    connection: &Connection,
    source_table: &str,
    source_id: &str,
    field: &ResolvedField,
    validate: bool,
) -> Result<Value, StorageError> {
    let related_id = if let Some(key) = &field.foreign_key {
        let Some(Value::Reference(id)) = field_value(connection, source_table, source_id, key)?
        else {
            return Ok(Value::Null);
        };
        if !row_exists(connection, &field.table, &id)? {
            return Ok(Value::Null);
        };
        Some(id)
    } else {
        None
    };
    let id = related_id.as_deref().unwrap_or(source_id);
    if field.name == "id" {
        return Ok(Value::String(id.into()));
    }
    let value = field_value(connection, &field.table, id, &field.name)?.unwrap_or(Value::Null);
    if !validate
        || value_has_type(&value, &field.column.kind)
        || value == Value::Null && field.column.optional
    {
        Ok(value)
    } else {
        Err(StorageError::InvalidRow {
            table: field.table.clone(),
            id: id.into(),
            message: "invalid requested projected cell".into(),
        })
    }
}

impl SqliteStorage {
    fn projected_rows(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> Result<Vec<ProjectedRow>, StorageError> {
        let plan = query::build(schema, query)?;
        let requested: Vec<_> = fields
            .iter()
            .map(|path| resolve_field(schema, &query.table, path))
            .collect::<Result<_, _>>()?;
        require_query_tables(&self.connection, schema, query)?;
        for field in &requested {
            require_table(&self.connection, &field.table)?;
        }
        let consulted: Vec<_> = query
            .predicates
            .iter()
            .map(|p| &p.field)
            .chain(query.order.iter().map(|o| &o.field))
            .map(|path| resolve_field(schema, &query.table, path))
            .collect::<Result<_, _>>()?;
        let order = query
            .order
            .as_ref()
            .map(|o| resolve_field(schema, &query.table, &o.field))
            .transpose()?;
        let ids = plan.execute(&self.connection, || {
            self.connection
                .prepare(&plan.sql)
                .map_err(backend)?
                .query_map(params_from_iter(plan.parameters.iter()), |row| {
                    row.get::<_, String>(0)
                })
                .map_err(backend)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(backend)
        })?;
        ids.into_iter()
            .map(|id| {
                // SQL determines membership/window. These narrow reads reject malformed
                // physical inputs without materializing or validating unrelated fields.
                for field in &consulted {
                    cell(&self.connection, &query.table, &id, field, false)?;
                }
                let values = fields
                    .iter()
                    .zip(&requested)
                    .map(|(path, field)| {
                        Ok((
                            path.clone(),
                            cell(&self.connection, &query.table, &id, field, true)?,
                        ))
                    })
                    .collect::<Result<_, StorageError>>()?;
                let order_key = order
                    .as_ref()
                    .map(|field| {
                        let value = cell(&self.connection, &query.table, &id, field, false)?;
                        Ok::<_, StorageError>(if value_has_type(&value, &field.column.kind) {
                            value
                        } else {
                            Value::Null
                        })
                    })
                    .transpose()?;
                Ok(ProjectedRow {
                    id,
                    values,
                    order_key,
                })
            })
            .collect()
    }
}

impl ObservationStorage for SqliteStorage {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(self.projected_rows(schema, query, fields))
    }
}
