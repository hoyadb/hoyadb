//! API-0011 / TC-0036: general WebAssembly boundary, independent of host fixtures.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Facade {
    inner: hoyadb_bindings::Facade,
}

#[wasm_bindgen]
impl Facade {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: hoyadb_bindings::Facade::new(),
        }
    }

    pub fn open(&mut self, input: &str, key: Vec<u8>) -> String {
        self.inner.open(input, key)
    }

    pub fn call(&mut self, input: &str) -> String {
        self.inner.call(input)
    }

    #[wasm_bindgen(js_name = validateOpen)]
    pub fn validate_open(&self, input: &str, key_length: u32) -> String {
        self.inner.validate_open(input, key_length as usize)
    }
}

impl Default for Facade {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_name = initStorage)]
pub async fn init_storage() -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;
    if !js_sys::global().is_instance_of::<web_sys::DedicatedWorkerGlobalScope>() {
        return Err(JsValue::from_str("BackendFailure"));
    }
    if POOL.with(|pool| pool.borrow().is_some()) {
        return Ok(());
    }
    if INSTALLING.with(|busy| busy.replace(true)) {
        return Err(JsValue::from_str("Busy"));
    }
    let cfg = sqlite_wasm_vfs::sahpool::OpfsSAHPoolCfgBuilder::new()
        .directory("hoyadb-v1")
        .clear_on_init(false)
        .initial_capacity(24)
        .build();
    let result =
        sqlite_wasm_vfs::sahpool::install::<sqlite_wasm_rs::WasmOsCallback>(&cfg, true).await;
    INSTALLING.with(|busy| busy.set(false));
    match result {
        Ok(pool) => {
            POOL.with(|slot| *slot.borrow_mut() = Some(pool));
            Ok(())
        }
        Err(_) => Err(JsValue::from_str("BackendFailure")),
    }
}

thread_local! {
    static POOL: std::cell::RefCell<Option<sqlite_wasm_vfs::sahpool::OpfsSAHPoolUtil>>=const{std::cell::RefCell::new(None)};
    static INSTALLING:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};
}

#[cfg(feature = "test-controls")]
#[wasm_bindgen(js_name=testFile)]
pub fn test_file(operation: &str, name: &str, data: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    use sqlite_wasm_rs::vfs::{
        VfsFilesManager,
        transfer::{DbTransfer, ExportSource},
    };
    const LIMIT: usize = 64 * 1024 * 1024;
    if !name.starts_with("phase-seven-sdk-owned-")
        || name.len() == "phase-seven-sdk-owned-".len()
        || name.contains(['/', '\\', '\0'])
        || data.len() > LIMIT
        || (!data.is_empty() && operation != "import")
    {
        return Err(JsValue::from_str("InvalidRequest"));
    }
    POOL.with(|slot| {
        let pool = slot.borrow();
        let pool = pool
            .as_ref()
            .ok_or_else(|| JsValue::from_str("BackendFailure"))?;
        let failure = |_| JsValue::from_str("BackendFailure");
        match operation {
            "exists" => Ok(vec![u8::from(pool.contains(name).map_err(failure)?)]),
            "remove" => Ok(vec![u8::from(pool.remove(name).map_err(failure)?)]),
            "import" => {
                pool.import_db_unchecked(name, &data).map_err(failure)?;
                Ok(vec![])
            }
            "export" => {
                let mut source = pool.open_export(name).map_err(failure)?;
                let size = usize::try_from(source.size())
                    .map_err(|_| JsValue::from_str("InvalidRequest"))?;
                if size > LIMIT {
                    return Err(JsValue::from_str("InvalidRequest"));
                }
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(size)
                    .map_err(|_| JsValue::from_str("BackendFailure"))?;
                bytes.resize(size, 0);
                let mut offset = 0;
                while offset < size {
                    let read = source
                        .read_at(offset as u64, &mut bytes[offset..])
                        .map_err(failure)?;
                    if read == 0 || read > size - offset {
                        return Err(JsValue::from_str("BackendFailure"));
                    }
                    offset += read;
                }
                Ok(bytes)
            }
            _ => Err(JsValue::from_str("InvalidRequest")),
        }
    })
}

#[cfg(feature = "test-controls")]
#[wasm_bindgen(js_name=testPoolStats)]
pub fn test_pool_stats() -> Result<Vec<u32>, JsValue> {
    use sqlite_wasm_rs::vfs::VfsFilesManager;
    POOL.with(|slot| {
        let pool = slot.borrow();
        let pool = pool
            .as_ref()
            .ok_or_else(|| JsValue::from_str("BackendFailure"))?;
        let capacity =
            u32::try_from(pool.capacity()).map_err(|_| JsValue::from_str("BackendFailure"))?;
        let assigned = u32::try_from(
            pool.len()
                .map_err(|_| JsValue::from_str("BackendFailure"))?,
        )
        .map_err(|_| JsValue::from_str("BackendFailure"))?;
        Ok(vec![capacity, assigned])
    })
}
