//! API-0005 / SPEC-0027: narrow owned cells for adapter-owned observations.
use super::{Query, Schema, SchemaStorage, StorageError, Value};
use std::{collections::BTreeMap, future::Future};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedRow {
    pub id: String,
    pub order_key: Option<Value>,
    pub values: BTreeMap<String, Value>,
}

pub trait ObservationStorage: SchemaStorage {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send;
}
