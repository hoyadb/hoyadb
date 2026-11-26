//! TC-0019/TC-0020: unchanged shared assertions against fresh memory stores.
use hoyadb_storage_memory::MemoryStorage;

fn fresh() -> MemoryStorage {
    MemoryStorage::default()
}

#[path = "../../../tests/conformance/storage.rs"]
mod conformance;
#[path = "../../../tests/conformance/data_model.rs"]
mod model_conformance;
#[path = "../../../tests/conformance/schema_storage.rs"]
mod schema_conformance;
#[path = "../../../tests/conformance/typed_storage.rs"]
mod typed_conformance;

#[path = "../../../tests/conformance/projection.rs"]
mod projection_conformance;

#[path = "../../../tests/conformance/reactivity.rs"]
mod reactive_conformance;
