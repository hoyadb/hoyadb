//! API-0003 / SPEC-0025: portable typed storage inputs and commit snapshots.
use super::{Row, Storage, StorageError, TransactionalStorage, Value};
use std::collections::BTreeMap;
use std::future::Future;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiniteFloat(u64);

impl FiniteFloat {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then(|| {
            Self(if value == 0.0 {
                0
            } else {
                value.to_bits()
            })
        })
    }

    pub fn get(&self) -> f64 {
        f64::from_bits(self.0)
    }
}

impl Ord for FiniteFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get().total_cmp(&other.get())
    }
}

impl PartialOrd for FiniteFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub kind: ColumnType,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relation {
    ToOne {
        table: String,
        foreign_key: String,
    },
    HasMany,
    ManyToMany,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TableSchema {
    pub columns: BTreeMap<String, Column>,
    pub relations: BTreeMap<String, Relation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Schema {
    pub tables: BTreeMap<String, TableSchema>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicate {
    pub field: String,
    pub operator: Operator,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderBy {
    pub field: String,
    pub direction: Direction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub table: String,
    pub predicates: Vec<Predicate>,
    pub order: Option<OrderBy>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedChange {
    pub table: String,
    pub id: String,
    pub before: Option<Row>,
    pub after: Option<Row>,
}

/// SPEC-0037 R8.0: proof, created only inside hoyadb-core, that a query runs
/// for a `CommandDatabase` that owns an `Exclusive` store and passed R5 open
/// validation, so every stored row of `schema` satisfies `validate_row`.
///
/// It cannot be constructed, copied or kept outside the core:
///
/// ```compile_fail,E0451
/// # use hoyadb_core::storage::*;
/// # fn forge<'a>(schema: &'a Schema, query: &'a Query) -> ValidatedQuery<'a> {
/// ValidatedQuery { schema, query }
/// # }
/// ```
///
/// Nor can a raw caller reach an adapter's validated path without one:
///
/// ```compile_fail,E0624
/// # use hoyadb_core::storage::*;
/// # async fn forge(store: SnapshotStorage, schema: Schema, query: Query) {
/// store.query_validated(ValidatedQuery::new(&schema, &query)).await;
/// # }
/// ```
pub struct ValidatedQuery<'a> {
    schema: &'a Schema,
    query: &'a Query,
}

impl<'a> ValidatedQuery<'a> {
    pub(crate) fn new(schema: &'a Schema, query: &'a Query) -> Self {
        Self {
            schema,
            query,
        }
    }

    pub fn schema(&self) -> &'a Schema {
        self.schema
    }

    pub fn query(&self) -> &'a Query {
        self.query
    }
}

pub trait TypedStorage: Storage {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send;

    /// SPEC-0037 R8.1: a query in a validated exclusive owner. An adapter may
    /// serve it with a bounded ordered path; every other adapter keeps
    /// today's `query`.
    #[doc(hidden)]
    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.query(proof.schema(), proof.query())
    }
}

pub trait TypedTransactionalStorage: TypedStorage + TransactionalStorage {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<super::Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send;
}

/// A checked own or direct-to-one field; adapters retain execution ownership.
#[derive(Clone, Debug)]
pub struct ResolvedField {
    pub table: String,
    pub name: String,
    pub column: Column,
    pub foreign_key: Option<String>,
}

fn invalid_schema(message: &str) -> StorageError {
    StorageError::InvalidSchema {
        message: message.into(),
    }
}

fn invalid_query(message: &str) -> StorageError {
    StorageError::InvalidQuery {
        message: message.into(),
    }
}

fn table<'a>(schema: &'a Schema, name: &str) -> Result<&'a TableSchema, StorageError> {
    schema
        .tables
        .get(name)
        .ok_or_else(|| StorageError::TableNotFound {
            table: name.into(),
        })
}

pub fn validate_schema(schema: &Schema) -> Result<(), StorageError> {
    for definition in schema.tables.values() {
        for name in definition.columns.keys().chain(definition.relations.keys()) {
            if name.is_empty() || name.contains('.') || name == "id" {
                return Err(invalid_schema("invalid column or relation name"));
            }
        }
        for (name, relation) in &definition.relations {
            if definition.columns.contains_key(name) {
                return Err(invalid_schema("relation shadows column"));
            }
            if let Relation::ToOne {
                table,
                foreign_key,
            } = relation
            {
                if !schema.tables.contains_key(table) {
                    return Err(invalid_schema("undeclared relation target"));
                }
                if !matches!(
                    definition.columns.get(foreign_key),
                    Some(Column {
                        kind: ColumnType::Reference,
                        ..
                    })
                ) {
                    return Err(invalid_schema(
                        "to-one foreign key must be a declared reference",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn resolve_field(
    schema: &Schema,
    source: &str,
    path: &str,
) -> Result<ResolvedField, StorageError> {
    let source_table = table(schema, source)?;
    let (target, name, foreign_key) = if let Some((relation, name)) = path.split_once('.') {
        if name.contains('.') {
            return Err(invalid_query("only one relation edge is supported"));
        }
        match source_table.relations.get(relation) {
            Some(Relation::ToOne {
                table,
                foreign_key,
            }) => (table.as_str(), name, Some(foreign_key.clone())),
            _ => return Err(invalid_query("unknown or unsupported relation path")),
        }
    } else {
        (source, path, None)
    };
    let column = if name == "id" {
        Column {
            kind: ColumnType::String,
            optional: false,
        }
    } else {
        table(schema, target)?
            .columns
            .get(name)
            .cloned()
            .ok_or_else(|| invalid_query("undeclared field"))?
    };
    Ok(ResolvedField {
        table: target.into(),
        name: name.into(),
        column,
        foreign_key,
    })
}

/// Logical kinds never coerce; optional omission is handled by the engine.
pub fn value_has_type(value: &Value, kind: &ColumnType) -> bool {
    matches!(
        (value, kind),
        (Value::String(_), ColumnType::String)
            | (Value::Integer(_), ColumnType::Integer)
            | (Value::Float(_), ColumnType::Float)
            | (Value::Boolean(_), ColumnType::Boolean)
            | (Value::Timestamp(_), ColumnType::Timestamp)
            | (Value::Reference(_), ColumnType::Reference)
    )
}

pub fn validate_query(schema: &Schema, query: &Query) -> Result<(), StorageError> {
    validate_schema(schema)?;
    table(schema, &query.table)?;
    for predicate in &query.predicates {
        let field = resolve_field(schema, &query.table, &predicate.field)?;
        if predicate.operator != Operator::In && predicate.values.len() != 1 {
            return Err(invalid_query("scalar predicate requires one operand"));
        }
        for value in &predicate.values {
            if !value_has_type(value, &field.column.kind)
                && !(value == &Value::Null && field.column.optional)
            {
                return Err(invalid_query("operand does not match declared field type"));
            }
        }
    }
    if let Some(order) = &query.order {
        resolve_field(schema, &query.table, &order.field)?;
    }
    Ok(())
}

pub fn validate_row(schema: &Schema, name: &str, row: &Row) -> Result<(), StorageError> {
    super::count_work!(rows_validated, 1);
    let definition = table(schema, name)?;
    let invalid = |message: &str| StorageError::InvalidRow {
        table: name.into(),
        id: row.id.clone(),
        message: message.into(),
    };
    for field in row.fields.keys() {
        if !definition.columns.contains_key(field) {
            return Err(invalid("unknown row field"));
        }
    }
    for (field, column) in &definition.columns {
        match row.fields.get(field) {
            None | Some(Value::Null) if column.optional => (),
            Some(value) if value_has_type(value, &column.kind) => (),
            _ => return Err(invalid("missing or incompatible required field")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Schema {
        Schema {
            tables: BTreeMap::from([
                (
                    "orders".into(),
                    TableSchema {
                        columns: BTreeMap::from([
                            (
                                "name".into(),
                                Column {
                                    kind: ColumnType::String,
                                    optional: false,
                                },
                            ),
                            (
                                "customer_id".into(),
                                Column {
                                    kind: ColumnType::Reference,
                                    optional: true,
                                },
                            ),
                        ]),
                        relations: BTreeMap::from([
                            (
                                "customer".into(),
                                Relation::ToOne {
                                    table: "customers".into(),
                                    foreign_key: "customer_id".into(),
                                },
                            ),
                            ("items".into(), Relation::HasMany),
                        ]),
                    },
                ),
                (
                    "customers".into(),
                    TableSchema {
                        columns: BTreeMap::from([(
                            "country".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: true,
                            },
                        )]),
                        relations: BTreeMap::new(),
                    },
                ),
            ]),
        }
    }

    #[test]
    fn finite_float_preserves_all_finite_values_and_canonicalizes_zero() {
        for value in [
            0.0,
            -0.0,
            1.0,
            1.25,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
        ] {
            let stored = FiniteFloat::new(value).expect("finite value must construct");
            assert_eq!(stored.get(), value);
            if value == 0.0 {
                assert_eq!(stored.get().to_bits(), 0);
            }
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(FiniteFloat::new(value).is_none());
        }
    }

    #[test]
    fn supplied_schema_and_paths_validate_without_engine_access() {
        let mut schema = schema();
        assert_eq!(validate_schema(&schema), Ok(()));
        let field = resolve_field(&schema, "orders", "customer.country").unwrap();
        assert_eq!(field.table, "customers");
        assert_eq!(field.foreign_key.as_deref(), Some("customer_id"));
        assert!(field.column.optional);
        for path in [
            "missing",
            "customer.missing",
            "customer.country.name",
            "items.name",
        ] {
            assert!(matches!(
                resolve_field(&schema, "orders", path),
                Err(StorageError::InvalidQuery { .. })
            ));
        }
        schema.tables.get_mut("orders").unwrap().columns.insert(
            "bad.name".into(),
            Column {
                kind: ColumnType::String,
                optional: false,
            },
        );
        assert!(matches!(
            validate_schema(&schema),
            Err(StorageError::InvalidSchema { .. })
        ));
    }

    #[test]
    fn query_and_row_validation_use_exact_types_and_coordinates() {
        let schema = schema();
        let mut query = Query {
            table: "orders".into(),
            predicates: vec![Predicate {
                field: "name".into(),
                operator: Operator::Eq,
                values: vec![Value::String("a".into())],
            }],
            order: None,
            limit: Some(0),
        };
        assert_eq!(validate_query(&schema, &query), Ok(()));
        query.predicates[0].values = vec![Value::Integer(1)];
        assert!(matches!(
            validate_query(&schema, &query),
            Err(StorageError::InvalidQuery { .. })
        ));
        let row = Row {
            id: "a".into(),
            fields: BTreeMap::from([("name".into(), Value::String("a".into()))]),
        };
        assert_eq!(validate_row(&schema, "orders", &row), Ok(()));
        let invalid = Row {
            id: "bad".into(),
            fields: BTreeMap::new(),
        };
        assert!(
            matches!(validate_row(&schema,"orders",&invalid),Err(StorageError::InvalidRow{table,id,..}) if table=="orders"&&id=="bad")
        );
    }
}
