//! SPEC-0026 / API-0004: optional atomic schema lifecycle capability.
use super::*;
use crate::model::{RESERVED_PREFIX, ROW_VERSION, SchemaDefinition};

pub const MODEL_TABLE: &str = "__hoya_model";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelState {
    pub version: u32,
    pub definition: String,
    pub next_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelCatalog {
    pub state: Option<ModelState>,
    pub tables: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaMutation {
    CreateTable(String),
    DropTable(String),
    Write(Mutation),
    Validate {
        schema: Schema,
        version: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaPlan {
    pub expected: Option<ModelState>,
    pub next: ModelState,
    pub operations: Vec<SchemaMutation>,
    pub schema: Schema,
}

pub trait SchemaStorage: TypedTransactionalStorage {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send;
    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send;
    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}

fn backend(message: &str) -> StorageError {
    StorageError::BackendFailure {
        message: message.into(),
    }
}

pub fn schema_conflict(message: &str) -> StorageError {
    StorageError::SchemaConflict {
        message: message.into(),
    }
}

pub fn model_definition(state: &ModelState) -> Result<SchemaDefinition, StorageError> {
    let definition = SchemaDefinition::from_canonical(&state.definition)
        .map_err(|_| backend("invalid canonical stored schema"))?;
    if state.version == 0 || state.version != definition.version || state.next_id < 1 {
        return Err(backend("inconsistent stored schema state"));
    }
    Ok(definition)
}

pub fn model_state_row(state: &ModelState) -> Result<Row, StorageError> {
    model_definition(state)?;
    Ok(Row {
        id: "state".into(),
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(1)),
            ("version".into(), Value::Integer(i64::from(state.version))),
            ("definition".into(), Value::String(state.definition.clone())),
            ("next_id".into(), Value::Integer(state.next_id)),
        ]),
    })
}

pub fn model_state_from_rows(rows: &[Row]) -> Result<ModelState, StorageError> {
    let [row] = rows else {
        return Err(backend("schema metadata requires exactly one row"));
    };
    if row.id != "state"
        || row.fields.len() != 4
        || row.fields.get("format") != Some(&Value::Integer(1))
    {
        return Err(backend("unsupported schema metadata shape"));
    }
    let (
        Some(Value::Integer(version)),
        Some(Value::String(definition)),
        Some(Value::Integer(next_id)),
    ) = (
        row.fields.get("version"),
        row.fields.get("definition"),
        row.fields.get("next_id"),
    )
    else {
        return Err(backend("invalid schema metadata types"));
    };
    let state = ModelState {
        version: u32::try_from(*version).map_err(|_| backend("invalid schema version"))?,
        definition: definition.clone(),
        next_id: *next_id,
    };
    model_definition(&state)?;
    Ok(state)
}

pub fn model_metadata_schema() -> TableSchema {
    TableSchema {
        columns: [
            ("format", ColumnType::Integer),
            ("version", ColumnType::Integer),
            ("definition", ColumnType::String),
            ("next_id", ColumnType::Integer),
        ]
        .into_iter()
        .map(|(name, kind)| {
            (
                name.into(),
                Column {
                    kind,
                    optional: false,
                },
            )
        })
        .collect(),
        relations: BTreeMap::new(),
    }
}

pub fn checked_model_catalog(
    state: Option<ModelState>,
    mut tables: Vec<String>,
) -> Result<ModelCatalog, StorageError> {
    if tables.iter().any(|name| name.starts_with(RESERVED_PREFIX)) {
        return Err(backend("unknown reserved bookkeeping"));
    }
    tables.sort();
    if tables.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(backend("duplicate logical table"));
    }
    if let Some(state) = &state {
        let definition = model_definition(state)?;
        if tables != definition.tables.keys().cloned().collect::<Vec<_>>() {
            return Err(backend("managed inventory differs from schema"));
        }
    }
    Ok(ModelCatalog {
        state,
        tables,
    })
}

pub fn validate_model_inventory(
    schema: &Schema,
    version: u32,
    tables: &BTreeMap<String, Vec<Row>>,
) -> Result<(), StorageError> {
    validate_schema(schema)?;
    if version == 0 || !tables.keys().eq(schema.tables.keys()) {
        return Err(backend("invalid schema candidate inventory"));
    }
    for (table, rows) in tables {
        for row in rows {
            super::count_work!(inventory_rows_validated, 1);
            validate_row(schema, table, row)?;
            if row.fields.get(ROW_VERSION) != Some(&Value::Integer(i64::from(version))) {
                return Err(backend("inconsistent managed row version"));
            }
        }
    }
    Ok(())
}

pub fn validate_schema_plan(plan: &SchemaPlan) -> Result<(), StorageError> {
    if model_definition(&plan.next)?.storage_schema()? != plan.schema {
        return Err(backend("schema plan definition and projection disagree"));
    }
    if let Some(expected) = &plan.expected {
        model_definition(expected)?;
        if plan.next.version < expected.version
            || plan.next.next_id < expected.next_id
            || (plan.next.version == expected.version
                && plan.next.definition != expected.definition)
        {
            return Err(schema_conflict(
                "schema plan cannot downgrade or redefine an existing version",
            ));
        }
    }
    for op in &plan.operations {
        match op {
            SchemaMutation::CreateTable(name) | SchemaMutation::DropTable(name) => {
                crate::model::check_name(name)?
            }
            SchemaMutation::Write(mutation) => {
                let (table, row) = match mutation {
                    Mutation::Insert {
                        table,
                        row,
                    }
                    | Mutation::Replace {
                        table,
                        row,
                    } => (table, Some(row)),
                    Mutation::Remove {
                        table,
                        ..
                    } => (table, None),
                };
                crate::model::check_name(table)?;
                if let Some(row) = row
                    && row
                        .fields
                        .keys()
                        .any(|name| name.starts_with(RESERVED_PREFIX) && name != ROW_VERSION)
                {
                    return Err(crate::model::invalid("reserved schema-plan row field"));
                }
            }
            SchemaMutation::Validate {
                schema,
                version,
            } => {
                if *version == 0 {
                    return Err(backend("invalid checkpoint version"));
                }
                validate_schema(schema)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TableDefinition;

    fn state() -> ModelState {
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        ModelState {
            version: 1,
            definition: schema.canonical().unwrap(),
            next_id: 1,
        }
    }

    #[test]
    fn d13_metadata_shape_is_exact_and_never_defaults_on_corruption() {
        let state = state();
        let row = model_state_row(&state).unwrap();
        assert_eq!(
            model_state_from_rows(std::slice::from_ref(&row)),
            Ok(state.clone())
        );
        let mut variants = vec![Vec::new(), vec![row.clone(), row.clone()]];
        let mut bad = row.clone();
        bad.id = "other".into();
        variants.push(vec![bad]);
        for (name, value) in [
            ("extra", Value::Null),
            ("format", Value::Integer(2)),
            ("version", Value::Integer(2)),
            ("next_id", Value::Integer(0)),
            ("definition", Value::String("bad".into())),
        ] {
            let mut bad = row.clone();
            bad.fields.insert(name.into(), value);
            variants.push(vec![bad]);
        }
        let mut bad = row.clone();
        bad.fields.remove("version");
        variants.push(vec![bad]);
        for rows in variants {
            assert!(matches!(
                model_state_from_rows(&rows),
                Err(StorageError::BackendFailure { .. })
            ));
        }
    }

    #[test]
    fn d14_plan_and_inventory_require_matching_definition_and_row_versions() {
        let state = state();
        let schema = model_definition(&state).unwrap().storage_schema().unwrap();
        assert_eq!(
            checked_model_catalog(Some(state.clone()), vec!["items".into()]),
            Ok(ModelCatalog {
                state: Some(state.clone()),
                tables: vec!["items".into()]
            })
        );
        assert!(checked_model_catalog(Some(state.clone()), vec!["unknown".into()]).is_err());
        assert!(checked_model_catalog(None, vec!["__hoya_queue".into()]).is_err());
        let tables = BTreeMap::from([(
            "items".into(),
            vec![Row {
                id: "x".into(),
                fields: BTreeMap::from([(ROW_VERSION.into(), Value::Integer(1))]),
            }],
        )]);
        assert_eq!(validate_model_inventory(&schema, 1, &tables), Ok(()));
        assert!(validate_model_inventory(&schema, 2, &tables).is_err());
        let plan = SchemaPlan {
            expected: Some(state.clone()),
            next: state,
            operations: vec![],
            schema: schema.clone(),
        };
        assert_eq!(validate_schema_plan(&plan), Ok(()));
        let mut bad = plan.clone();
        bad.schema.tables.clear();
        assert!(validate_schema_plan(&bad).is_err());
        let mut bad = plan;
        bad.next.next_id = 0;
        assert!(validate_schema_plan(&bad).is_err());
    }
}
