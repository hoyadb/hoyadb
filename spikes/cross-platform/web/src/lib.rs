//! SPEC-0017 / REQ-0036: isolated browser boundary, not a public database API.
use hoyadb_cross_platform_spike::{Notification, Order, Store};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Default)]
pub struct WebStore {
    store: Store,
}

#[wasm_bindgen]
impl WebStore {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    // Decimal text avoids generated i64 coercion wrapping before validation.
    pub fn insert(
        &mut self,
        id: &str,
        status: &str,
        total: &str,
        paid: bool,
    ) -> Result<(), JsValue> {
        let total_cents = total
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("total must be signed i64 decimal text"))?;
        self.store.insert(Order {
            id: id.into(),
            status: status.into(),
            total_cents,
            paid,
        });
        Ok(())
    }

    pub fn query(&self, status: &str) -> WebRows {
        WebRows {
            rows: self.store.query_status(status),
        }
    }

    pub fn observe(&mut self, status: &str) -> Result<u64, JsValue> {
        self.store
            .observe(status)
            .ok_or_else(|| JsValue::from_str("subscription tokens exhausted"))
    }

    pub fn unobserve(&mut self, token: u64) -> bool {
        self.store.unobserve(token)
    }

    pub fn drain(&mut self) -> WebEvents {
        WebEvents {
            events: self.store.take_notifications(),
        }
    }
}

#[wasm_bindgen]
pub struct WebRows {
    rows: Vec<Order>,
}

#[wasm_bindgen]
impl WebRows {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row(&self, index: usize) -> Option<WebOrder> {
        self.rows.get(index).cloned().map(|row| WebOrder {
            row,
        })
    }
}

#[wasm_bindgen]
pub struct WebOrder {
    row: Order,
}

#[wasm_bindgen]
impl WebOrder {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.row.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.row.status.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn total_cents(&self) -> i64 {
        self.row.total_cents
    }

    #[wasm_bindgen(getter)]
    pub fn paid(&self) -> bool {
        self.row.paid
    }
}

#[wasm_bindgen]
pub struct WebEvents {
    events: Vec<Notification>,
}

#[wasm_bindgen]
impl WebEvents {
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn token(&self, index: usize) -> Option<u64> {
        self.events.get(index).map(|event| event.subscription)
    }

    pub fn count(&self, index: usize) -> Option<usize> {
        self.events.get(index).map(|event| event.count)
    }
}
