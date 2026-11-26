//! SPEC-0026 / TC-0026 D01/D02/D13: declarations and canonical schema identity.
use crate::storage::{self, Column, StorageError};
use std::collections::BTreeMap;

pub const RESERVED_PREFIX: &str = "__hoya_";

pub const ROW_VERSION: &str = "__hoya_schema_version";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaDefinition {
    pub version: u32,
    pub tables: BTreeMap<String, TableDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TableDefinition {
    pub columns: BTreeMap<String, Column>,
    pub relations: BTreeMap<String, RelationDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationDefinition {
    ToOne {
        table: String,
        foreign_key: String,
        optional: bool,
    },
    HasMany {
        table: String,
        foreign_key: String,
    },
    ManyToMany {
        table: String,
        through: String,
        source_relation: String,
        target_relation: String,
    },
}

pub(crate) fn invalid(message: &str) -> StorageError {
    StorageError::InvalidSchema {
        message: message.into(),
    }
}

pub(crate) fn check_name(name: &str) -> Result<(), StorageError> {
    if name.is_empty() || name == "id" || name.contains('.') || name.starts_with(RESERVED_PREFIX) {
        return Err(invalid("invalid or reserved application name"));
    }
    Ok(())
}

impl SchemaDefinition {
    pub(crate) fn normalized(&self) -> Result<Self, StorageError> {
        if self.version == 0 {
            return Err(invalid("schema version must be positive"));
        }
        let mut normalized = self.clone();
        for (name, table) in &mut normalized.tables {
            check_name(name)?;
            for name in table.columns.keys().chain(table.relations.keys()) {
                check_name(name)?;
            }
            let mut reference_targets = BTreeMap::new();
            for relation in table.relations.values() {
                if let RelationDefinition::ToOne {
                    table: target,
                    foreign_key,
                    optional,
                } = relation
                {
                    check_name(foreign_key)?;
                    let mapping = (target.clone(), *optional);
                    if let Some(previous) =
                        reference_targets.insert(foreign_key.clone(), mapping.clone())
                        && previous != mapping
                    {
                        return Err(invalid(
                            "shared reference aliases disagree on target or optionality",
                        ));
                    }
                    let column = Column {
                        kind: storage::ColumnType::Reference,
                        optional: *optional,
                    };
                    if let Some(previous) =
                        table.columns.insert(foreign_key.clone(), column.clone())
                        && previous != column
                    {
                        return Err(invalid(
                            "to-one reference declaration conflicts with column",
                        ));
                    }
                }
            }
            if table
                .relations
                .keys()
                .any(|key| table.columns.contains_key(key))
            {
                return Err(invalid("relation shadows column"));
            }
        }
        for (source, table) in &normalized.tables {
            for relation in table.relations.values() {
                match relation {
                    RelationDefinition::ToOne {
                        table,
                        ..
                    } => {
                        if !normalized.tables.contains_key(table) {
                            return Err(invalid("undeclared to-one target"));
                        }
                    }
                    RelationDefinition::HasMany {
                        table,
                        foreign_key,
                    } => {
                        check_name(foreign_key)?;
                        let target = normalized
                            .tables
                            .get(table)
                            .ok_or_else(|| invalid("undeclared has-many target"))?;
                        if !target.relations.values().any(|relation| matches!(relation,
                            RelationDefinition::ToOne { table, foreign_key: key, .. } if table == source && key == foreign_key)) {
                            return Err(invalid("has-many requires a declared inverse to-one key"));
                        }
                    }
                    RelationDefinition::ManyToMany {
                        table,
                        through,
                        source_relation,
                        target_relation,
                    } => {
                        if !normalized.tables.contains_key(table) {
                            return Err(invalid("undeclared many-to-many target"));
                        }
                        let through = normalized
                            .tables
                            .get(through)
                            .ok_or_else(|| invalid("undeclared through table"))?;
                        let source_key = match through.relations.get(source_relation) {
                            Some(RelationDefinition::ToOne {
                                table,
                                foreign_key,
                                ..
                            }) if table == source => foreign_key,
                            _ => return Err(invalid("invalid through source relation")),
                        };
                        let target_key = match through.relations.get(target_relation) {
                            Some(RelationDefinition::ToOne {
                                table: target,
                                foreign_key,
                                ..
                            }) if target == table => foreign_key,
                            _ => return Err(invalid("invalid through target relation")),
                        };
                        if source_relation == target_relation || source_key == target_key {
                            return Err(invalid("many-to-many requires distinct through keys"));
                        }
                    }
                }
            }
        }
        Ok(normalized)
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        self.normalized().map(|_| ())
    }

    pub fn storage_schema(&self) -> Result<storage::Schema, StorageError> {
        let normalized = self.normalized()?;
        let mut tables = BTreeMap::new();
        for (name, table) in normalized.tables {
            let mut columns = table.columns;
            columns.insert(
                ROW_VERSION.into(),
                Column {
                    kind: storage::ColumnType::Integer,
                    optional: false,
                },
            );
            let relations = table
                .relations
                .into_iter()
                .map(|(name, relation)| {
                    (
                        name,
                        match relation {
                            RelationDefinition::ToOne {
                                table,
                                foreign_key,
                                ..
                            } => storage::Relation::ToOne {
                                table,
                                foreign_key,
                            },
                            RelationDefinition::HasMany {
                                ..
                            } => storage::Relation::HasMany,
                            RelationDefinition::ManyToMany {
                                ..
                            } => storage::Relation::ManyToMany,
                        },
                    )
                })
                .collect();
            tables.insert(
                name,
                storage::TableSchema {
                    columns,
                    relations,
                },
            );
        }
        let schema = storage::Schema {
            tables,
        };
        storage::validate_schema(&schema)?;
        Ok(schema)
    }

    pub fn canonical(&self) -> Result<String, StorageError> {
        let normalized = self.normalized()?;
        let mut output = String::new();
        fn token(output: &mut String, value: &str) {
            use std::fmt::Write;
            write!(output, "{}:{value}", value.len()).unwrap();
        }
        token(&mut output, "hoyadb-model-1");
        token(&mut output, &normalized.version.to_string());
        token(&mut output, &normalized.tables.len().to_string());
        for (name, table) in normalized.tables {
            token(&mut output, &name);
            token(&mut output, &table.columns.len().to_string());
            for (name, column) in table.columns {
                token(&mut output, &name);
                token(&mut output, kind_name(&column.kind));
                token(
                    &mut output,
                    if column.optional {
                        "1"
                    } else {
                        "0"
                    },
                );
            }
            token(&mut output, &table.relations.len().to_string());
            for (name, relation) in table.relations {
                token(&mut output, &name);
                match relation {
                    RelationDefinition::ToOne {
                        table,
                        foreign_key,
                        optional,
                    } => {
                        for value in [
                            "ToOne",
                            &table,
                            &foreign_key,
                            if optional {
                                "1"
                            } else {
                                "0"
                            },
                        ] {
                            token(&mut output, value);
                        }
                    }
                    RelationDefinition::HasMany {
                        table,
                        foreign_key,
                    } => {
                        for value in ["HasMany", &table, &foreign_key] {
                            token(&mut output, value);
                        }
                    }
                    RelationDefinition::ManyToMany {
                        table,
                        through,
                        source_relation,
                        target_relation,
                    } => {
                        for value in [
                            "ManyToMany",
                            &table,
                            &through,
                            &source_relation,
                            &target_relation,
                        ] {
                            token(&mut output, value);
                        }
                    }
                }
            }
        }
        Ok(output)
    }

    pub fn from_canonical(input: &str) -> Result<Self, StorageError> {
        let mut reader = Reader(input);
        if reader.token()? != "hoyadb-model-1" {
            return Err(invalid("unsupported schema encoding"));
        }
        let version = reader
            .token()?
            .parse::<u32>()
            .map_err(|_| invalid("invalid schema version"))?;
        let mut tables = BTreeMap::new();
        for _ in 0..reader.count()? {
            let name = reader.token()?.to_owned();
            let mut columns = BTreeMap::new();
            for _ in 0..reader.count()? {
                let name = reader.token()?.to_owned();
                let kind = match reader.token()? {
                    "String" => storage::ColumnType::String,
                    "Integer" => storage::ColumnType::Integer,
                    "Float" => storage::ColumnType::Float,
                    "Boolean" => storage::ColumnType::Boolean,
                    "Timestamp" => storage::ColumnType::Timestamp,
                    "Reference" => storage::ColumnType::Reference,
                    _ => return Err(invalid("invalid column kind")),
                };
                let optional = reader.flag()?;
                if columns
                    .insert(
                        name,
                        Column {
                            kind,
                            optional,
                        },
                    )
                    .is_some()
                {
                    return Err(invalid("duplicate column"));
                }
            }
            let mut relations = BTreeMap::new();
            for _ in 0..reader.count()? {
                let name = reader.token()?.to_owned();
                let relation = match reader.token()? {
                    "ToOne" => RelationDefinition::ToOne {
                        table: reader.token()?.into(),
                        foreign_key: reader.token()?.into(),
                        optional: reader.flag()?,
                    },
                    "HasMany" => RelationDefinition::HasMany {
                        table: reader.token()?.into(),
                        foreign_key: reader.token()?.into(),
                    },
                    "ManyToMany" => RelationDefinition::ManyToMany {
                        table: reader.token()?.into(),
                        through: reader.token()?.into(),
                        source_relation: reader.token()?.into(),
                        target_relation: reader.token()?.into(),
                    },
                    _ => return Err(invalid("invalid relation kind")),
                };
                if relations.insert(name, relation).is_some() {
                    return Err(invalid("duplicate relation"));
                }
            }
            if tables
                .insert(
                    name,
                    TableDefinition {
                        columns,
                        relations,
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate table"));
            }
        }
        let definition = Self {
            version,
            tables,
        };
        if !reader.0.is_empty() || definition.canonical()? != input {
            return Err(invalid("noncanonical schema definition"));
        }
        Ok(definition)
    }
}

fn kind_name(kind: &storage::ColumnType) -> &'static str {
    match kind {
        storage::ColumnType::String => "String",
        storage::ColumnType::Integer => "Integer",
        storage::ColumnType::Float => "Float",
        storage::ColumnType::Boolean => "Boolean",
        storage::ColumnType::Timestamp => "Timestamp",
        storage::ColumnType::Reference => "Reference",
    }
}

struct Reader<'a>(&'a str);

impl<'a> Reader<'a> {
    fn token(&mut self) -> Result<&'a str, StorageError> {
        let (length, tail) = self
            .0
            .split_once(':')
            .ok_or_else(|| invalid("missing schema frame"))?;
        if length.is_empty() || !length.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("invalid schema frame length"));
        }
        let length = length
            .parse::<usize>()
            .map_err(|_| invalid("schema frame length overflow"))?;
        let value = tail
            .get(..length)
            .ok_or_else(|| invalid("truncated or invalid UTF-8 schema frame"))?;
        self.0 = &tail[length..];
        Ok(value)
    }

    fn count(&mut self) -> Result<usize, StorageError> {
        let count = self
            .token()?
            .parse::<usize>()
            .map_err(|_| invalid("invalid schema entry count"))?;
        if count > self.0.len() / 2 {
            return Err(invalid("truncated schema entries"));
        }
        Ok(count)
    }

    fn flag(&mut self) -> Result<bool, StorageError> {
        match self.token()? {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(invalid("invalid optional flag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ColumnType, Relation};

    fn schema() -> SchemaDefinition {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                (
                    "customers".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "name".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::from([(
                            "orders".into(),
                            RelationDefinition::HasMany {
                                table: "orders".into(),
                                foreign_key: "customer_id".into(),
                            },
                        )]),
                    },
                ),
                (
                    "orders".into(),
                    TableDefinition {
                        columns: BTreeMap::new(),
                        relations: BTreeMap::from([(
                            "customer".into(),
                            RelationDefinition::ToOne {
                                table: "customers".into(),
                                foreign_key: "customer_id".into(),
                                optional: true,
                            },
                        )]),
                    },
                ),
            ]),
        }
    }

    #[test]
    fn d01_implicit_keys_and_bookkeeping_compile_without_changing_declaration() {
        let definition = schema();
        assert_eq!(definition.validate(), Ok(()));
        let compiled = definition.storage_schema().unwrap();
        assert_eq!(
            compiled.tables["orders"].columns["customer_id"],
            Column {
                kind: ColumnType::Reference,
                optional: true
            }
        );
        assert_eq!(
            compiled.tables["orders"].columns[ROW_VERSION],
            Column {
                kind: ColumnType::Integer,
                optional: false
            }
        );
        assert!(
            matches!(&compiled.tables["orders"].relations["customer"], Relation::ToOne { table, foreign_key } if table=="customers" && foreign_key=="customer_id")
        );
        assert!(
            !definition.tables["orders"]
                .columns
                .contains_key(ROW_VERSION)
        );
        let mut explicit = definition.clone();
        explicit.tables.get_mut("orders").unwrap().columns.insert(
            "customer_id".into(),
            Column {
                kind: ColumnType::Reference,
                optional: true,
            },
        );
        assert_eq!(explicit.canonical(), definition.canonical());
    }

    #[test]
    fn d02_invalid_versions_names_and_reference_mappings_reject() {
        let mut variants = Vec::new();
        let mut bad = schema();
        bad.version = 0;
        variants.push(bad);
        for name in ["", "id", "bad.name", "__hoya_queue"] {
            let mut bad = schema();
            bad.tables.insert(name.into(), TableDefinition::default());
            variants.push(bad);
            let mut bad = schema();
            bad.tables.get_mut("orders").unwrap().columns.insert(
                name.into(),
                Column {
                    kind: ColumnType::String,
                    optional: true,
                },
            );
            variants.push(bad);
        }
        let mut bad = schema();
        bad.tables.get_mut("orders").unwrap().columns.insert(
            "customer_id".into(),
            Column {
                kind: ColumnType::String,
                optional: true,
            },
        );
        variants.push(bad);
        let mut bad = schema();
        bad.tables.remove("customers");
        variants.push(bad);
        let mut bad = schema();
        bad.tables.get_mut("customers").unwrap().relations.insert(
            "wrong".into(),
            RelationDefinition::HasMany {
                table: "orders".into(),
                foreign_key: "missing".into(),
            },
        );
        variants.push(bad);
        for bad in variants {
            assert!(matches!(
                bad.validate(),
                Err(StorageError::InvalidSchema { .. })
            ));
        }
    }

    #[test]
    fn tc0027_shared_reference_aliases_must_agree_on_target() {
        let mut definition = schema();
        definition
            .tables
            .insert("suppliers".into(), TableDefinition::default());
        definition
            .tables
            .get_mut("orders")
            .unwrap()
            .relations
            .insert(
                "buyer".into(),
                RelationDefinition::ToOne {
                    table: "customers".into(),
                    foreign_key: "customer_id".into(),
                    optional: true,
                },
            );
        assert_eq!(definition.validate(), Ok(()));
        definition
            .tables
            .get_mut("orders")
            .unwrap()
            .relations
            .insert(
                "buyer".into(),
                RelationDefinition::ToOne {
                    table: "suppliers".into(),
                    foreign_key: "customer_id".into(),
                    optional: true,
                },
            );
        assert!(matches!(
            definition.validate(),
            Err(StorageError::InvalidSchema { .. })
        ));
    }

    #[test]
    fn d13_canonical_codec_roundtrips_all_relation_operands_and_exact_names() {
        let mut definition = schema();
        definition.tables.get_mut("orders").unwrap().columns.insert(
            "引用\0:12🌿".into(),
            Column {
                kind: ColumnType::Float,
                optional: true,
            },
        );
        definition.tables.insert(
            "through".into(),
            TableDefinition {
                columns: BTreeMap::new(),
                relations: BTreeMap::from([
                    (
                        "order".into(),
                        RelationDefinition::ToOne {
                            table: "orders".into(),
                            foreign_key: "order_id".into(),
                            optional: false,
                        },
                    ),
                    (
                        "customer".into(),
                        RelationDefinition::ToOne {
                            table: "customers".into(),
                            foreign_key: "customer_id".into(),
                            optional: false,
                        },
                    ),
                ]),
            },
        );
        definition
            .tables
            .get_mut("orders")
            .unwrap()
            .relations
            .insert(
                "members".into(),
                RelationDefinition::ManyToMany {
                    table: "customers".into(),
                    through: "through".into(),
                    source_relation: "order".into(),
                    target_relation: "customer".into(),
                },
            );
        let encoded = definition.canonical().unwrap();
        let decoded = SchemaDefinition::from_canonical(&encoded).unwrap();
        assert_eq!(decoded.canonical(), Ok(encoded.clone()));
        assert_eq!(decoded.storage_schema(), definition.storage_schema());
        for bad in [
            String::new(),
            format!("{encoded}0:"),
            encoded[..encoded.len() - 1].to_owned(),
            encoded.replacen("hoyadb-model-1", "hoyadb-model-9", 1),
            encoded.replacen("14:hoyadb-model-1", "014:hoyadb-model-1", 1),
        ] {
            assert!(
                matches!(
                    SchemaDefinition::from_canonical(&bad),
                    Err(StorageError::InvalidSchema { .. })
                ),
                "accepted malformed encoding"
            );
        }
        let mut distinct = definition.clone();
        distinct.tables.get_mut("orders").unwrap().columns.insert(
            "12:引用\0🌿".into(),
            Column {
                kind: ColumnType::Float,
                optional: true,
            },
        );
        assert_ne!(distinct.canonical().unwrap(), encoded);
    }
}
