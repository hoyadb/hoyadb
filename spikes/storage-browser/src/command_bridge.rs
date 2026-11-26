//! API-0008 / TC-0032 C20: one shared Rust command fixture in the OPFS worker.
#[path = "../../common/command_fixture.rs"]
mod fixture;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct CommandFixture {
    inner: fixture::CommandFixture,
}

impl Default for CommandFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl CommandFixture {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: fixture::CommandFixture::new(),
        }
    }

    pub fn rpc(&mut self, input: &str) -> String {
        self.inner.dispatch(input)
    }
}
