//! SPEC-0025 / API-0003: adapters execute bound predicates, joins, order and limit.
use hoyadb_core::storage::{
    ColumnType, Direction, Operator, Query, ResolvedField, Schema, StorageError, Value,
    resolve_field, validate_query, validate_schema,
};
use rusqlite::types::Value as SqlValue;
use std::collections::BTreeMap;

pub(crate) struct Plan {
    pub sql: String,
    pub parameters: Vec<SqlValue>,
    operand_sets: Vec<Vec<Value>>,
}

struct Builder {
    operand_sets: Vec<Vec<Value>>,
    parameters: Vec<SqlValue>,
    joins: Vec<String>,
    fields: BTreeMap<String, Field>,
}

#[derive(Clone)]
struct Field {
    value: String,
    payload: &'static str,
    null: String,
    kind: String,
    present_target: Option<String>,
}

impl Builder {
    fn bind(&mut self, value: SqlValue) -> String {
        self.parameters.push(value);
        format!("?{}", self.parameters.len())
    }

    fn field(&mut self, resolved: ResolvedField, path: &str) -> Field {
        if let Some(field) = self.fields.get(path) {
            return field.clone();
        }
        let suffix = self.fields.len();
        let (owner, present_target) = if let Some(foreign_key) = resolved.foreign_key {
            let fk = format!("k{suffix}");
            let target = format!("t{suffix}");
            let fk_name = self.bind(SqlValue::Text(foreign_key));
            let target_table = self.bind(SqlValue::Text(resolved.table));
            self.joins.push(format!("LEFT JOIN __hoya_fields {fk} ON {fk}.table_name=r.table_name AND {fk}.row_id=r.id AND {fk}.name={fk_name} AND {fk}.kind=6"));
            self.joins.push(format!("LEFT JOIN __hoya_rows {target} ON {target}.table_name={target_table} AND {target}.id={fk}.text_value"));
            (target.clone(), Some(format!("{target}.id IS NOT NULL")))
        } else {
            ("r".into(), None)
        };
        let field = if resolved.name == "id" {
            Field {
                value: format!("{owner}.id"),
                payload: "text_value",
                null: "0".into(),
                kind: "1".into(),
                present_target,
            }
        } else {
            let alias = format!("f{suffix}");
            let name = self.bind(SqlValue::Text(resolved.name));
            self.joins.push(format!("LEFT JOIN __hoya_fields {alias} ON {alias}.table_name={owner}.table_name AND {alias}.row_id={owner}.id AND {alias}.name={name}"));
            let (tag, payload) = match resolved.column.kind {
                ColumnType::String => (1, "text_value"),
                ColumnType::Integer => (2, "int_value"),
                ColumnType::Boolean => (3, "int_value"),
                ColumnType::Float => (4, "int_value"),
                ColumnType::Timestamp => (5, "int_value"),
                ColumnType::Reference => (6, "text_value"),
            };
            Field {
                value: format!("{alias}.{payload}"),
                payload,
                null: if resolved.column.optional {
                    format!("({alias}.kind IS NULL OR {alias}.kind=7)")
                } else {
                    "0".into()
                },
                kind: format!("{alias}.kind={tag}"),
                present_target,
            }
        };
        self.fields.insert(path.into(), field.clone());
        field
    }

    fn compare(&mut self, field: &Field, operator: &Operator, value: &Value) -> String {
        if matches!(value, Value::Null) {
            return match operator {
                Operator::Eq | Operator::In => field.null.clone(),
                Operator::Ne => field.kind.clone(),
                _ => "0".into(),
            };
        }
        let value = self.bind(scalar(value));
        let op = match operator {
            Operator::Eq | Operator::In => "=",
            Operator::Ne => "<>",
            Operator::Lt => "<",
            Operator::Le => "<=",
            Operator::Gt => ">",
            Operator::Ge => ">=",
        };
        let comparison = format!("({} AND {} {op} {value})", field.kind, field.value);
        if matches!(operator, Operator::Ne) {
            format!("({} OR {comparison})", field.null)
        } else {
            comparison
        }
    }
}

pub(crate) fn build(schema: &Schema, query: &Query) -> Result<Plan, StorageError> {
    validate_schema(schema)?;
    validate_query(schema, query)?;
    let mut builder = Builder {
        parameters: vec![],
        operand_sets: vec![],
        joins: vec![],
        fields: BTreeMap::new(),
    };
    // Keep query metadata/ordinary operands far below both pinned builds' bind
    // limits; oversized sets use constant-shape, connection-local subqueries.
    let oversized = query
        .predicates
        .iter()
        .fold(0usize, |n, p| n.saturating_add(p.values.len()))
        > 4096;
    let table = builder.bind(SqlValue::Text(query.table.clone()));
    let mut filters = vec![format!("r.table_name={table}")];
    for predicate in &query.predicates {
        let field = builder.field(
            resolve_field(schema, &query.table, &predicate.field)?,
            &predicate.field,
        );
        let comparison = if matches!(predicate.operator, Operator::In) {
            let non_null: Vec<_> = predicate
                .values
                .iter()
                .filter(|v| **v != Value::Null)
                .cloned()
                .collect();
            let mut alternatives = Vec::new();
            if !non_null.is_empty() {
                let operands = if oversized {
                    let set_id = builder.operand_sets.len();
                    builder.operand_sets.push(non_null);
                    // Only internal set indices and payload column literals enter SQL.
                    format!(
                        "SELECT {} FROM temp.__hoya_query_operands WHERE set_id={set_id}",
                        field.payload
                    )
                } else {
                    non_null
                        .iter()
                        .map(|v| builder.bind(scalar(v)))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                alternatives.push(format!(
                    "({} AND {} IN ({operands}))",
                    field.kind, field.value
                ));
            }
            if predicate.values.contains(&Value::Null) {
                alternatives.push(field.null.clone());
            }
            if alternatives.is_empty() {
                "0".into()
            } else {
                format!("({})", alternatives.join(" OR "))
            }
        } else {
            builder.compare(&field, &predicate.operator, &predicate.values[0])
        };
        filters.push(if let Some(present) = &field.present_target {
            format!("({present} AND ({comparison}))")
        } else {
            comparison
        });
    }
    let order = if let Some(order) = &query.order {
        let field = builder.field(
            resolve_field(schema, &query.table, &order.field)?,
            &order.field,
        );
        let direction = match order.direction {
            Direction::Asc => "ASC",
            Direction::Desc => "DESC",
        };
        format!(
            "CASE WHEN {} THEN {} END {direction},r.id ASC",
            field.kind, field.value
        )
    } else {
        "r.id ASC".into()
    };
    let limit = if let Some(limit) = query.limit {
        let bound = builder.bind(SqlValue::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
        format!(" LIMIT {bound}")
    } else {
        String::new()
    };
    Ok(Plan {
        sql: format!(
            "SELECT r.id FROM __hoya_rows r {} WHERE {} ORDER BY {order}{limit}",
            builder.joins.join(" "),
            filters.join(" AND ")
        ),
        parameters: builder.parameters,
        operand_sets: builder.operand_sets,
    })
}

fn scalar(value: &Value) -> SqlValue {
    match value {
        Value::String(v) | Value::Reference(v) => SqlValue::Text(v.clone()),
        Value::Integer(v) | Value::Timestamp(v) => SqlValue::Integer(*v),
        Value::Boolean(v) => SqlValue::Integer(i64::from(*v)),
        Value::Float(v) => SqlValue::Real(v.get()),
        Value::Null => SqlValue::Null,
    }
}

impl Plan {
    // SPEC-0028 / TC-0031: operand cardinality cannot grow SQL expression
    // depth or bind count. Temp storage is MEMORY (configured by open), never
    // part of the persisted schema or OPFS handle pool. No caller SQL runs here.
    pub(crate) fn execute<T>(
        &self,
        connection: &rusqlite::Connection,
        operation: impl FnOnce() -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        if self.operand_sets.is_empty() {
            return operation();
        }
        connection.execute_batch("CREATE TEMP TABLE IF NOT EXISTS __hoya_query_operands(set_id INTEGER NOT NULL,kind INTEGER NOT NULL,text_value TEXT,int_value INTEGER); CREATE INDEX IF NOT EXISTS temp.__hoya_query_operands_set ON __hoya_query_operands(set_id); DELETE FROM temp.__hoya_query_operands").map_err(super::backend)?;
        let result = (|| {
            let mut statement=connection.prepare("INSERT INTO temp.__hoya_query_operands(set_id,kind,text_value,int_value) VALUES (?1,?2,?3,?4)").map_err(super::backend)?;
            for (set_id, values) in self.operand_sets.iter().enumerate() {
                for value in values {
                    let (kind, text, number) = super::encode_value(value);
                    statement
                        .execute(rusqlite::params![set_id as i64, kind, text, number])
                        .map_err(super::backend)?;
                }
            }
            drop(statement);
            operation()
        })();
        let cleanup = connection
            .execute("DELETE FROM temp.__hoya_query_operands", [])
            .map_err(super::backend);
        match (result, cleanup) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(_)) => Ok(value),
        }
    }
}
