//! API-0011: exact representation codecs; core validates application semantics.
use crate::{Error, Result};
use hoyadb_core::{
    command::{Json, SyncOptions},
    model::*,
    storage::{Column, ColumnType},
};
use std::collections::BTreeMap;

pub(crate) fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

pub(crate) fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}

pub(crate) fn map(value: &Json) -> Result<&BTreeMap<String, Json>> {
    match value {
        Json::Object(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

pub(crate) fn exact<'a>(value: &'a Json, names: &[&str]) -> Result<&'a BTreeMap<String, Json>> {
    let map = map(value)?;
    if map.len() != names.len() || names.iter().any(|name| !map.contains_key(*name)) {
        return Err(Error("InvalidRequest"));
    }
    Ok(map)
}

pub(crate) fn string(value: &Json) -> Result<&str> {
    match value {
        Json::String(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

pub(crate) fn boolean(value: &Json) -> Result<bool> {
    match value {
        Json::Bool(value) => Ok(*value),
        _ => Err(Error("InvalidRequest")),
    }
}

pub(crate) fn array(value: &Json) -> Result<&[Json]> {
    match value {
        Json::Array(value) => Ok(value),
        _ => Err(Error("InvalidRequest")),
    }
}

pub(crate) fn unsigned(value: &Json) -> Result<u64> {
    let input = string(value)?;
    let parsed = input.parse::<u64>().map_err(|_| Error("InvalidRequest"))?;
    if parsed.to_string() != input {
        return Err(Error("InvalidRequest"));
    }
    Ok(parsed)
}

pub(crate) fn size(value: &Json) -> Result<usize> {
    let value = u32::try_from(unsigned(value)?).map_err(|_| Error("InvalidRequest"))?;
    usize::try_from(value).map_err(|_| Error("InvalidRequest"))
}

pub(crate) fn integer(value: &Json) -> Result<i64> {
    let input = string(value)?;
    let parsed = input.parse::<i64>().map_err(|_| Error("InvalidRequest"))?;
    if parsed.to_string() != input {
        return Err(Error("InvalidRequest"));
    }
    Ok(parsed)
}

pub(crate) fn wire_only(value: &Json) -> Result<()> {
    match value {
        Json::Integer(_) | Json::Float(_) => Err(Error("InvalidRequest")),
        Json::Array(values) => values.iter().try_for_each(wire_only),
        Json::Object(values) => values.values().try_for_each(wire_only),
        _ => Ok(()),
    }
}

fn column(value: &Json) -> Result<Column> {
    let f = exact(value, &["kind", "optional"])?;
    Ok(Column {
        kind: match string(&f["kind"])? {
            "String" => ColumnType::String,
            "Integer" => ColumnType::Integer,
            "Float" => ColumnType::Float,
            "Boolean" => ColumnType::Boolean,
            "Timestamp" => ColumnType::Timestamp,
            "Reference" => ColumnType::Reference,
            _ => return Err(Error("InvalidRequest")),
        },
        optional: boolean(&f["optional"])?,
    })
}

fn relation(value: &Json) -> Result<RelationDefinition> {
    let f = map(value)?;
    match string(f.get("kind").ok_or(Error("InvalidRequest"))?)? {
        "ToOne" => {
            let f = exact(value, &["kind", "table", "foreign_key", "optional"])?;
            Ok(RelationDefinition::ToOne {
                table: string(&f["table"])?.into(),
                foreign_key: string(&f["foreign_key"])?.into(),
                optional: boolean(&f["optional"])?,
            })
        }
        "HasMany" => {
            let f = exact(value, &["kind", "table", "foreign_key"])?;
            Ok(RelationDefinition::HasMany {
                table: string(&f["table"])?.into(),
                foreign_key: string(&f["foreign_key"])?.into(),
            })
        }
        "ManyToMany" => {
            let f = exact(
                value,
                &[
                    "kind",
                    "table",
                    "through",
                    "source_relation",
                    "target_relation",
                ],
            )?;
            Ok(RelationDefinition::ManyToMany {
                table: string(&f["table"])?.into(),
                through: string(&f["through"])?.into(),
                source_relation: string(&f["source_relation"])?.into(),
                target_relation: string(&f["target_relation"])?.into(),
            })
        }
        _ => Err(Error("InvalidRequest")),
    }
}

fn table(value: &Json) -> Result<TableDefinition> {
    let f = exact(value, &["columns", "relations"])?;
    Ok(TableDefinition {
        columns: map(&f["columns"])?
            .iter()
            .map(|(name, value)| Ok((name.clone(), column(value)?)))
            .collect::<Result<_>>()?,
        relations: map(&f["relations"])?
            .iter()
            .map(|(name, value)| Ok((name.clone(), relation(value)?)))
            .collect::<Result<_>>()?,
    })
}

pub(crate) fn schema(value: &Json) -> Result<SchemaDefinition> {
    let f = exact(value, &["version", "tables"])?;
    Ok(SchemaDefinition {
        version: u32::try_from(unsigned(&f["version"])?).map_err(|_| Error("InvalidRequest"))?,
        tables: map(&f["tables"])?
            .iter()
            .map(|(name, value)| Ok((name.clone(), table(value)?)))
            .collect::<Result<_>>()?,
    })
}

pub(crate) fn schema_wire(value: &SchemaDefinition) -> Json {
    fn relation_wire(value: &RelationDefinition) -> Json {
        match value {
            RelationDefinition::ToOne {
                table,
                foreign_key,
                optional,
            } => object([
                ("kind", text("ToOne")),
                ("table", text(table)),
                ("foreign_key", text(foreign_key)),
                ("optional", Json::Bool(*optional)),
            ]),
            RelationDefinition::HasMany {
                table,
                foreign_key,
            } => object([
                ("kind", text("HasMany")),
                ("table", text(table)),
                ("foreign_key", text(foreign_key)),
            ]),
            RelationDefinition::ManyToMany {
                table,
                through,
                source_relation,
                target_relation,
            } => object([
                ("kind", text("ManyToMany")),
                ("table", text(table)),
                ("through", text(through)),
                ("source_relation", text(source_relation)),
                ("target_relation", text(target_relation)),
            ]),
        }
    }
    object([
        ("version", text(value.version)),
        (
            "tables",
            Json::Object(
                value
                    .tables
                    .iter()
                    .map(|(name, table)| {
                        (
                            name.clone(),
                            object([
                                (
                                    "columns",
                                    Json::Object(
                                        table
                                            .columns
                                            .iter()
                                            .map(|(name, column)| {
                                                (
                                                    name.clone(),
                                                    object([
                                                        (
                                                            "kind",
                                                            text(match column.kind {
                                                                ColumnType::String => "String",
                                                                ColumnType::Integer => "Integer",
                                                                ColumnType::Float => "Float",
                                                                ColumnType::Boolean => "Boolean",
                                                                ColumnType::Timestamp => {
                                                                    "Timestamp"
                                                                }
                                                                ColumnType::Reference => {
                                                                    "Reference"
                                                                }
                                                            }),
                                                        ),
                                                        ("optional", Json::Bool(column.optional)),
                                                    ]),
                                                )
                                            })
                                            .collect(),
                                    ),
                                ),
                                (
                                    "relations",
                                    Json::Object(
                                        table
                                            .relations
                                            .iter()
                                            .map(|(name, value)| {
                                                (name.clone(), relation_wire(value))
                                            })
                                            .collect(),
                                    ),
                                ),
                            ]),
                        )
                    })
                    .collect(),
            ),
        ),
    ])
}

pub(crate) fn options(value: &Json) -> Result<SyncOptions> {
    let f = exact(
        value,
        &[
            "capacity",
            "history",
            "initial_delay_ms",
            "multiplier",
            "max_delay_ms",
        ],
    )?;
    Ok(SyncOptions {
        capacity: size(&f["capacity"])?,
        history: size(&f["history"])?,
        initial_delay_ms: integer(&f["initial_delay_ms"])?,
        multiplier: u32::try_from(unsigned(&f["multiplier"])?)
            .map_err(|_| Error("InvalidRequest"))?,
        max_delay_ms: integer(&f["max_delay_ms"])?,
    })
}

pub(crate) fn migrations(value: &Json) -> Result<Vec<Migration>> {
    array(value)?
        .iter()
        .map(|v| {
            let f = exact(v, &["from_version", "to", "steps"])?;
            let steps = array(&f["steps"])?
                .iter()
                .map(|v| {
                    let f = map(v)?;
                    let op = string(f.get("op").ok_or(Error("InvalidRequest"))?)?;
                    Ok(match op {
                        "AddTable" => {
                            let f = exact(v, &["op", "table", "definition"])?;
                            MigrationStep::AddTable {
                                table: string(&f["table"])?.into(),
                                definition: table(&f["definition"])?,
                            }
                        }
                        "DropTable" => {
                            let f = exact(v, &["op", "table"])?;
                            MigrationStep::DropTable {
                                table: string(&f["table"])?.into(),
                            }
                        }
                        "AddColumn" => {
                            let f = exact(v, &["op", "table", "name", "column", "default"])?;
                            let d = map(&f["default"])?;
                            let default =
                                match string(d.get("kind").ok_or(Error("InvalidRequest"))?)? {
                                    "none" => {
                                        exact(&f["default"], &["kind"])?;
                                        None
                                    }
                                    "some" => {
                                        let d = exact(&f["default"], &["kind", "value"])?;
                                        Some(crate::data::cell(&d["value"])?)
                                    }
                                    _ => return Err(Error("InvalidRequest")),
                                };
                            MigrationStep::AddColumn {
                                table: string(&f["table"])?.into(),
                                name: string(&f["name"])?.into(),
                                column: column(&f["column"])?,
                                default,
                            }
                        }
                        "DropColumn" => {
                            let f = exact(v, &["op", "table", "name"])?;
                            MigrationStep::DropColumn {
                                table: string(&f["table"])?.into(),
                                name: string(&f["name"])?.into(),
                            }
                        }
                        "RenameColumn" => {
                            let f = exact(v, &["op", "table", "from", "to"])?;
                            MigrationStep::RenameColumn {
                                table: string(&f["table"])?.into(),
                                from: string(&f["from"])?.into(),
                                to: string(&f["to"])?.into(),
                            }
                        }
                        "Backfill" => {
                            let f = exact(v, &["op", "table", "column", "value"])?;
                            MigrationStep::Backfill {
                                table: string(&f["table"])?.into(),
                                column: string(&f["column"])?.into(),
                                value: crate::data::cell(&f["value"])?,
                            }
                        }
                        "SetRelations" => {
                            let f = exact(v, &["op", "table", "relations"])?;
                            MigrationStep::SetRelations {
                                table: string(&f["table"])?.into(),
                                relations: map(&f["relations"])?
                                    .iter()
                                    .map(|(k, v)| Ok((k.clone(), relation(v)?)))
                                    .collect::<Result<_>>()?,
                            }
                        }
                        _ => return Err(Error("InvalidRequest")),
                    })
                })
                .collect::<Result<_>>()?;
            Ok(Migration {
                from_version: u32::try_from(unsigned(&f["from_version"])?)
                    .map_err(|_| Error("InvalidRequest"))?,
                to: schema(&f["to"])?,
                steps,
            })
        })
        .collect()
}
