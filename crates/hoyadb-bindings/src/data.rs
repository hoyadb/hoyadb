//! API-0011 §3: lossless tagged data and lazy core query construction.
use crate::{Error, Result, codec::*};
use hoyadb_core::{
    command::Json,
    model::{QueryBuilder, RelationDefinition, SchemaDefinition, Write},
    storage::{CommittedChange, Direction, FiniteFloat, Operator, Row, Value},
};
use std::collections::BTreeMap;

fn float(input: &str) -> Result<FiniteFloat> {
    if input.trim() != input || input.is_empty() {
        return Err(Error("InvalidRequest"));
    }
    let number = if input.contains(['.', 'e', 'E']) {
        input.into()
    } else {
        format!("{input}.0")
    };
    match Json::parse(&number).map_err(|_| Error("InvalidRequest"))? {
        Json::Float(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

pub(crate) fn boxed(value: &Json) -> Json {
    let (kind, value) = match value {
        Json::Null => return object([("kind", text("null"))]),
        Json::Bool(v) => ("boolean", Json::Bool(*v)),
        Json::String(v) => ("string", text(v)),
        Json::Integer(v) => ("integer", text(v)),
        Json::Float(v) => ("float", text(v.get())),
        Json::Array(v) => ("array", Json::Array(v.iter().map(boxed).collect())),
        Json::Object(v) => (
            "object",
            Json::Object(v.iter().map(|(k, v)| (k.clone(), boxed(v))).collect()),
        ),
    };
    object([("kind", text(kind)), ("value", value)])
}

pub(crate) fn unbox(value: &Json) -> Result<Json> {
    fn decode(value: &Json) -> Result<Json> {
        let f = map(value)?;
        let kind = string(f.get("kind").ok_or(Error("InvalidRequest"))?)?;
        if kind == "null" {
            exact(value, &["kind"])?;
            return Ok(Json::Null);
        }
        let f = exact(value, &["kind", "value"])?;
        let v = &f["value"];
        Ok(match kind {
            "boolean" => Json::Bool(boolean(v)?),
            "string" => text(string(v)?),
            "integer" => Json::Integer(integer(v)?),
            "float" => Json::Float(float(string(v)?)?),
            "array" => Json::Array(array(v)?.iter().map(decode).collect::<Result<_>>()?),
            "object" => Json::Object(
                map(v)?
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), decode(v)?)))
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(Error("InvalidRequest")),
        })
    }
    let value = decode(value)?;
    value.canonical()?;
    Ok(value)
}

pub(crate) fn cell(value: &Json) -> Result<Value> {
    let f = map(value)?;
    match string(f.get("kind").ok_or(Error("InvalidRequest"))?)? {
        "timestamp" => {
            let f = exact(value, &["kind", "value"])?;
            Ok(Value::Timestamp(integer(&f["value"])?))
        }
        "reference" => {
            let f = exact(value, &["kind", "value"])?;
            Ok(Value::Reference(string(&f["value"])?.into()))
        }
        _ => Ok(match unbox(value)? {
            Json::Null => Value::Null,
            Json::Bool(v) => Value::Boolean(v),
            Json::String(v) => Value::String(v),
            Json::Integer(v) => Value::Integer(v),
            Json::Float(v) => Value::Float(v),
            _ => return Err(Error("InvalidRequest")),
        }),
    }
}

pub(crate) fn cell_wire(value: &Value) -> Json {
    match value {
        Value::Timestamp(v) => object([("kind", text("timestamp")), ("value", text(v))]),
        Value::Reference(v) => object([("kind", text("reference")), ("value", text(v))]),
        Value::Null => boxed(&Json::Null),
        Value::Boolean(v) => boxed(&Json::Bool(*v)),
        Value::String(v) => boxed(&text(v)),
        Value::Integer(v) => boxed(&Json::Integer(*v)),
        Value::Float(v) => boxed(&Json::Float(*v)),
    }
}

pub(crate) fn fields(value: &Json) -> Result<BTreeMap<String, Value>> {
    map(value)?
        .iter()
        .map(|(k, v)| Ok((k.clone(), cell(v)?)))
        .collect()
}

pub(crate) fn row_wire(row: &Row) -> Json {
    object([
        ("id", text(&row.id)),
        (
            "fields",
            Json::Object(
                row.fields
                    .iter()
                    .map(|(k, v)| (k.clone(), cell_wire(v)))
                    .collect(),
            ),
        ),
    ])
}

pub(crate) fn rows_wire(rows: &[Row]) -> Json {
    Json::Array(rows.iter().map(row_wire).collect())
}

pub(crate) fn changes_wire(changes: &[CommittedChange]) -> Json {
    Json::Array(
        changes
            .iter()
            .map(|c| {
                object([
                    ("table", text(&c.table)),
                    ("id", text(&c.id)),
                    (
                        "before",
                        c.before.as_ref().map(row_wire).unwrap_or(Json::Null),
                    ),
                    (
                        "after",
                        c.after.as_ref().map(row_wire).unwrap_or(Json::Null),
                    ),
                ])
            })
            .collect(),
    )
}

pub(crate) fn proposed(value: &Json) -> Result<Option<String>> {
    if value == &Json::Null {
        Ok(None)
    } else {
        Ok(Some(string(value)?.into()))
    }
}

pub(crate) fn query(
    definition: &SchemaDefinition,
    value: &Json,
    make: impl FnOnce(&str, Option<(&str, &str, &str)>) -> QueryBuilder,
) -> Result<QueryBuilder> {
    let f = exact(value, &["table", "predicates", "order", "limit", "source"])?;
    let table = string(&f["table"])?;
    let source = if f["source"] == Json::Null {
        None
    } else {
        let source = exact(&f["source"], &["table", "id", "relation"])?;
        let name = string(&source["table"])?;
        let relation = string(&source["relation"])?;
        let Some(declaration) = definition
            .tables
            .get(name)
            .and_then(|t| t.relations.get(relation))
        else {
            return Err(Error("InvalidQuery"));
        };
        let (RelationDefinition::ToOne {
            table: target,
            ..
        }
        | RelationDefinition::HasMany {
            table: target,
            ..
        }
        | RelationDefinition::ManyToMany {
            table: target,
            ..
        }) = declaration;
        if target != table {
            return Err(Error("InvalidQuery"));
        }
        Some((name, string(&source["id"])?, relation))
    };
    let mut q = make(table, source);
    for p in array(&f["predicates"])? {
        let p = exact(p, &["field", "operator", "values"])?;
        let name = string(&p["field"])?;
        let values = array(&p["values"])?
            .iter()
            .map(cell)
            .collect::<Result<Vec<_>>>()?;
        let op = match string(&p["operator"])? {
            "Eq" => Operator::Eq,
            "Ne" => Operator::Ne,
            "Lt" => Operator::Lt,
            "Le" => Operator::Le,
            "Gt" => Operator::Gt,
            "Ge" => Operator::Ge,
            "In" => Operator::In,
            _ => return Err(Error("InvalidRequest")),
        };
        q = if op == Operator::In {
            q.where_in(name, values)
        } else {
            let [value] = values.as_slice() else {
                return Err(Error("InvalidRequest"));
            };
            q.where_(name, op, value.clone())
        };
    }
    if f["order"] != Json::Null {
        let o = exact(&f["order"], &["field", "direction"])?;
        q = q.order_by(
            string(&o["field"])?,
            match string(&o["direction"])? {
                "Asc" => Direction::Asc,
                "Desc" => Direction::Desc,
                _ => return Err(Error("InvalidRequest")),
            },
        );
    }
    if f["limit"] != Json::Null {
        q = q.limit(size(&f["limit"])?);
    }
    Ok(q)
}

pub(crate) fn writes(value: &Json) -> Result<Vec<(Write, Option<String>)>> {
    array(value)?
        .iter()
        .map(|v| {
            let f = map(v)?;
            match string(f.get("op").ok_or(Error("InvalidRequest"))?)? {
                "insert" => {
                    let f = exact(v, &["op", "table", "fields", "proposed_id"])?;
                    Ok((
                        Write::Insert {
                            table: string(&f["table"])?.into(),
                            fields: fields(&f["fields"])?,
                        },
                        proposed(&f["proposed_id"])?,
                    ))
                }
                "replace" => {
                    let f = exact(v, &["op", "table", "id", "fields"])?;
                    Ok((
                        Write::Replace {
                            table: string(&f["table"])?.into(),
                            id: string(&f["id"])?.into(),
                            fields: fields(&f["fields"])?,
                        },
                        None,
                    ))
                }
                "remove" => {
                    let f = exact(v, &["op", "table", "id"])?;
                    Ok((
                        Write::Remove {
                            table: string(&f["table"])?.into(),
                            id: string(&f["id"])?.into(),
                        },
                        None,
                    ))
                }
                _ => Err(Error("InvalidRequest")),
            }
        })
        .collect()
}
