//! SPEC-0026 / API-0004: explicitly authored, all-or-nothing schema changes.
use super::{RelationDefinition, SchemaDefinition, TableDefinition};
use crate::storage::{Column, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    pub from_version: u32,
    pub to: SchemaDefinition,
    pub steps: Vec<MigrationStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationStep {
    AddTable {
        table: String,
        definition: TableDefinition,
    },
    DropTable {
        table: String,
    },
    AddColumn {
        table: String,
        name: String,
        column: Column,
        default: Option<Value>,
    },
    DropColumn {
        table: String,
        name: String,
    },
    RenameColumn {
        table: String,
        from: String,
        to: String,
    },
    Backfill {
        table: String,
        column: String,
        value: Value,
    },
    SetRelations {
        table: String,
        relations: BTreeMap<String, RelationDefinition>,
    },
}

fn failure(message: &str) -> crate::storage::StorageError {
    crate::storage::StorageError::MigrationFailure {
        message: message.into(),
    }
}

fn validate_nested_names(
    relations: &BTreeMap<String, RelationDefinition>,
    columns: Option<&BTreeMap<String, Column>>,
) -> Result<(), crate::storage::StorageError> {
    let mut references = BTreeMap::new();
    for (name, relation) in relations {
        super::check_name(name).map_err(|_| failure("invalid migration relation name"))?;
        if columns.is_some_and(|columns| columns.contains_key(name)) {
            return Err(failure("migration relation shadows a column"));
        }
        let names = match relation {
            RelationDefinition::ToOne {
                table,
                foreign_key,
                optional,
            } => {
                let mapping = (table.clone(), *optional);
                if let Some(previous) = references.insert(foreign_key, mapping.clone())
                    && previous != mapping
                {
                    return Err(failure("migration reference aliases disagree"));
                }
                if relations.contains_key(foreign_key)
                    || columns
                        .and_then(|columns| columns.get(foreign_key))
                        .is_some_and(|column| {
                            column
                                != &Column {
                                    kind: crate::storage::ColumnType::Reference,
                                    optional: *optional,
                                }
                        })
                {
                    return Err(failure(
                        "migration reference conflicts with a declared field",
                    ));
                }
                vec![table.as_str(), foreign_key.as_str()]
            }
            RelationDefinition::HasMany {
                table,
                foreign_key,
            } => {
                vec![table.as_str(), foreign_key.as_str()]
            }
            RelationDefinition::ManyToMany {
                table,
                through,
                source_relation,
                target_relation,
            } => {
                if source_relation == target_relation {
                    return Err(failure("migration through relations must differ"));
                }
                vec![
                    table.as_str(),
                    through.as_str(),
                    source_relation.as_str(),
                    target_relation.as_str(),
                ]
            }
        };
        for name in names {
            super::check_name(name).map_err(|_| failure("invalid migration relation operand"))?;
        }
    }
    Ok(())
}

fn validate_descriptors(
    migrations: &[Migration],
) -> Result<BTreeMap<u32, &Migration>, crate::storage::StorageError> {
    let mut graph = BTreeMap::new();
    for migration in migrations {
        if migration.from_version == 0
            || migration.to.version <= migration.from_version
            || graph.insert(migration.from_version, migration).is_some()
        {
            return Err(failure("invalid or ambiguous migration edge"));
        }
        migration
            .to
            .validate()
            .map_err(|_| failure("invalid migration target schema"))?;
        for step in &migration.steps {
            match step {
                MigrationStep::AddTable {
                    definition,
                    ..
                } => {
                    for name in definition.columns.keys() {
                        super::check_name(name)
                            .map_err(|_| failure("invalid migration column declaration"))?;
                    }
                    validate_nested_names(&definition.relations, Some(&definition.columns))?;
                }
                MigrationStep::SetRelations {
                    relations,
                    ..
                } => validate_nested_names(relations, None)?,
                _ => (),
            }
            let table = match step {
                MigrationStep::AddTable {
                    table,
                    ..
                }
                | MigrationStep::DropTable {
                    table,
                }
                | MigrationStep::AddColumn {
                    table,
                    ..
                }
                | MigrationStep::DropColumn {
                    table,
                    ..
                }
                | MigrationStep::RenameColumn {
                    table,
                    ..
                }
                | MigrationStep::Backfill {
                    table,
                    ..
                }
                | MigrationStep::SetRelations {
                    table,
                    ..
                } => table,
            };
            super::check_name(table).map_err(|_| failure("invalid migration table name"))?;
            let names = match step {
                MigrationStep::AddColumn {
                    name,
                    ..
                }
                | MigrationStep::DropColumn {
                    name,
                    ..
                } => {
                    vec![name.as_str()]
                }
                MigrationStep::RenameColumn {
                    from,
                    to,
                    ..
                } => vec![from.as_str(), to.as_str()],
                MigrationStep::Backfill {
                    column,
                    ..
                } => vec![column.as_str()],
                _ => vec![],
            };
            for name in names {
                super::check_name(name).map_err(|_| failure("invalid migration column name"))?;
            }
            if let MigrationStep::AddColumn {
                column,
                default: Some(value),
                ..
            } = step
                && !accepts(column, value)
            {
                return Err(failure("added column default has incompatible type"));
            }
        }
    }
    Ok(graph)
}

fn accepts(column: &Column, value: &Value) -> bool {
    crate::storage::value_has_type(value, &column.kind) || column.optional && value == &Value::Null
}

type Data = BTreeMap<String, Vec<crate::storage::Row>>;

fn apply_step(
    definition: &mut SchemaDefinition,
    rows: &mut Data,
    step: &MigrationStep,
) -> Result<(), crate::storage::StorageError> {
    match step {
        MigrationStep::AddTable {
            table,
            definition: added,
        } => {
            if definition.tables.contains_key(table) {
                return Err(failure("added table already exists"));
            }
            definition.tables.insert(table.clone(), added.clone());
            rows.insert(table.clone(), vec![]);
        }
        MigrationStep::DropTable {
            table,
        } => {
            if definition.tables.remove(table).is_none() {
                return Err(failure("dropped table does not exist"));
            }
            rows.remove(table);
        }
        MigrationStep::AddColumn {
            table,
            name,
            column,
            default,
        } => {
            let declared = definition
                .tables
                .get_mut(table)
                .ok_or_else(|| failure("migration table does not exist"))?;
            if declared.columns.contains_key(name) || declared.relations.contains_key(name) {
                return Err(failure("added column already exists"));
            }
            if !column.optional && default.is_none() && !rows[table].is_empty() {
                return Err(failure("required added column needs an explicit default"));
            }
            declared.columns.insert(name.clone(), column.clone());
            if let Some(value) = default {
                for row in rows.get_mut(table).unwrap() {
                    row.fields.insert(name.clone(), value.clone());
                }
            }
        }
        MigrationStep::DropColumn {
            table,
            name,
        } => {
            if definition
                .tables
                .get_mut(table)
                .ok_or_else(|| failure("migration table does not exist"))?
                .columns
                .remove(name)
                .is_none()
            {
                return Err(failure("dropped column does not exist"));
            }
            for row in rows.get_mut(table).unwrap() {
                row.fields.remove(name);
            }
        }
        MigrationStep::RenameColumn {
            table,
            from,
            to,
        } => {
            let declared = definition
                .tables
                .get_mut(table)
                .ok_or_else(|| failure("migration table does not exist"))?;
            if declared.columns.contains_key(to) || declared.relations.contains_key(to) {
                return Err(failure("renamed column destination already exists"));
            }
            let column = declared
                .columns
                .remove(from)
                .ok_or_else(|| failure("renamed column does not exist"))?;
            declared.columns.insert(to.clone(), column);
            for row in rows.get_mut(table).unwrap() {
                if let Some(value) = row.fields.remove(from) {
                    row.fields.insert(to.clone(), value);
                }
            }
        }
        MigrationStep::Backfill {
            table,
            column,
            value,
        } => {
            let declared = definition
                .tables
                .get(table)
                .and_then(|table| {
                    // A ToOne set earlier in this migration already declares its
                    // implicit key; normalization materializes it at the endpoint.
                    table.columns.get(column).cloned().or_else(|| {
                        table
                            .relations
                            .values()
                            .find_map(|relation| match relation {
                                RelationDefinition::ToOne {
                                    foreign_key,
                                    optional,
                                    ..
                                } if foreign_key == column => Some(Column {
                                    kind: crate::storage::ColumnType::Reference,
                                    optional: *optional,
                                }),
                                _ => None,
                            })
                    })
                })
                .ok_or_else(|| failure("backfill column does not exist"))?;
            if !accepts(&declared, value) {
                return Err(failure("backfill value has incompatible type"));
            }
            for row in rows.get_mut(table).unwrap() {
                row.fields.insert(column.clone(), value.clone());
            }
        }
        MigrationStep::SetRelations {
            table,
            relations,
        } => {
            definition
                .tables
                .get_mut(table)
                .ok_or_else(|| failure("migration table does not exist"))?
                .relations = relations.clone()
        }
    }
    Ok(())
}

fn lower(before: &Data, after: &Data, operations: &mut Vec<crate::storage::SchemaMutation>) {
    use crate::storage::{Mutation, SchemaMutation};
    for table in before.keys().filter(|table| !after.contains_key(*table)) {
        operations.push(SchemaMutation::DropTable(table.clone()));
    }
    for (table, rows) in after {
        if !before.contains_key(table) {
            operations.push(SchemaMutation::CreateTable(table.clone()));
        }
        let old: BTreeMap<_, _> = before
            .get(table)
            .into_iter()
            .flatten()
            .map(|row| (&row.id, row))
            .collect();
        let new: BTreeMap<_, _> = rows.iter().map(|row| (&row.id, row)).collect();
        for id in old.keys().filter(|id| !new.contains_key(*id)) {
            operations.push(SchemaMutation::Write(Mutation::Remove {
                table: table.clone(),
                id: (*id).clone(),
            }));
        }
        for (id, row) in new {
            let mutation = match old.get(id) {
                None => Some(Mutation::Insert {
                    table: table.clone(),
                    row: row.clone(),
                }),
                Some(prior) if *prior != row => Some(Mutation::Replace {
                    table: table.clone(),
                    row: row.clone(),
                }),
                _ => None,
            };
            if let Some(mutation) = mutation {
                operations.push(SchemaMutation::Write(mutation));
            }
        }
    }
}

pub(crate) async fn open_schema<S: crate::storage::SchemaStorage>(
    storage: &mut S,
    definition: &SchemaDefinition,
    migrations: &[Migration],
) -> Result<crate::storage::ModelState, crate::storage::StorageError> {
    use super::ROW_VERSION;
    use crate::storage::*;
    let graph = validate_descriptors(migrations)?;
    let catalog = storage.catalog().await?;
    let canonical = definition.canonical()?;
    let projection = definition.storage_schema()?;
    if let Some(state) = catalog.state {
        if state.version > definition.version {
            return Err(schema_conflict(
                "application schema downgrade is unsupported",
            ));
        }
        if state.version == definition.version {
            if state.definition != canonical {
                return Err(schema_conflict("schema changed without a version change"));
            }
            return Ok(state);
        }
        let mut version = state.version;
        let mut chain = Vec::new();
        while version < definition.version {
            let Some(migration) = graph.get(&version) else {
                chain.clear();
                break;
            };
            if migration.to.version > definition.version {
                return Err(failure("migration edge overshoots requested version"));
            }
            version = migration.to.version;
            chain.push(*migration);
        }
        let next = ModelState {
            version: definition.version,
            definition: canonical,
            next_id: state.next_id,
        };
        let mut operations = Vec::new();
        if chain.is_empty() {
            for table in catalog.tables {
                operations.push(SchemaMutation::DropTable(table));
            }
            for table in definition.tables.keys() {
                operations.push(SchemaMutation::CreateTable(table.clone()));
            }
        } else {
            if chain.last().unwrap().to.canonical()? != next.definition {
                return Err(failure(
                    "authored final endpoint differs from requested schema",
                ));
            }
            let mut working = model_definition(&state)?;
            let mut rows = BTreeMap::new();
            for table in catalog.tables {
                rows.insert(table.clone(), storage.scan(&table).await?);
            }
            validate_model_inventory(&working.storage_schema()?, working.version, &rows)
                .map_err(|_| failure("source rows do not match stored schema"))?;
            for migration in chain {
                let before = rows.clone();
                for step in &migration.steps {
                    apply_step(&mut working, &mut rows, step)?;
                }
                working.version = migration.to.version;
                working = working
                    .normalized()
                    .map_err(|_| failure("migration produced an invalid schema"))?;
                if working.canonical()? != migration.to.canonical()? {
                    return Err(failure(
                        "authored operations do not reach their declared endpoint",
                    ));
                }
                for table in rows.values_mut() {
                    for row in table {
                        row.fields.insert(
                            ROW_VERSION.into(),
                            Value::Integer(i64::from(working.version)),
                        );
                    }
                }
                let checkpoint = working.storage_schema()?;
                validate_model_inventory(&checkpoint, working.version, &rows)
                    .map_err(|_| failure("migration endpoint rows are invalid"))?;
                lower(&before, &rows, &mut operations);
                operations.push(SchemaMutation::Validate {
                    schema: checkpoint,
                    version: working.version,
                });
            }
        }
        storage
            .apply_schema_plan(SchemaPlan {
                expected: Some(state),
                next: next.clone(),
                operations,
                schema: projection,
            })
            .await?;
        return Ok(next);
    }
    if !catalog.tables.is_empty() && definition.version != 1 {
        return Err(schema_conflict(
            "populated unversioned stores can only adopt version one",
        ));
    }
    if catalog
        .tables
        .iter()
        .any(|name| !definition.tables.contains_key(name))
    {
        return Err(schema_conflict("unversioned store has undeclared tables"));
    }
    let mut legacy = projection.clone();
    for table in legacy.tables.values_mut() {
        table.columns.remove(ROW_VERSION);
    }
    let mut operations = Vec::new();
    for name in definition.tables.keys() {
        if !catalog.tables.contains(name) {
            operations.push(SchemaMutation::CreateTable(name.clone()));
        }
    }
    for name in &catalog.tables {
        for mut row in storage.scan(name).await? {
            validate_row(&legacy, name, &row).map_err(|_| {
                schema_conflict("unversioned rows do not match the declared schema")
            })?;
            row.fields.insert(
                ROW_VERSION.into(),
                Value::Integer(i64::from(definition.version)),
            );
            operations.push(SchemaMutation::Write(Mutation::Replace {
                table: name.clone(),
                row,
            }));
        }
    }
    let state = ModelState {
        version: definition.version,
        definition: canonical,
        next_id: 1,
    };
    storage
        .apply_schema_plan(SchemaPlan {
            expected: None,
            next: state.clone(),
            operations,
            schema: projection,
        })
        .await?;
    Ok(state)
}
