//! String-only bindings keep mathematical integers in the shared exact JSON format.

use crate::{Assignment, PreparedModel};
use wasm_bindgen::prelude::*;

/// One validated executable, reusable across independent candidate checks.
#[wasm_bindgen(js_name = PreparedModel)]
pub struct WasmModel {
    model: PreparedModel,
}

#[wasm_bindgen(js_class = PreparedModel)]
impl WasmModel {
    /// Load the same versioned artifact accepted by the native checker.
    #[wasm_bindgen(constructor)]
    pub fn new(prepared_json: &str) -> Result<WasmModel, JsValue> {
        let model = PreparedModel::from_json(prepared_json).map_err(js_error)?;
        Ok(Self { model })
    }

    #[wasm_bindgen(js_name = setFloatTolerance)]
    pub fn set_float_tolerance(&mut self, absolute: f64, relative: f64) -> Result<(), JsValue> {
        let tolerance =
            crate::FloatTolerance::new(absolute, relative).map_err(JsValue::from_str)?;
        self.model.set_float_tolerance(tolerance);
        Ok(())
    }

    #[wasm_bindgen(js_name = floatTolerance)]
    pub fn float_tolerance(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.model.float_tolerance())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Check typed assignment JSON and return the shared CheckReport as JSON.
    pub fn check(&self, assignment_json: &str) -> Result<String, JsValue> {
        let assignment: Assignment = serde_json::from_str(assignment_json)
            .map_err(|error| JsValue::from_str(&format!("invalid assignment JSON: {error}")))?;
        let report = self.model.check(&assignment).map_err(js_error)?;
        serde_json::to_string(&report).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn js_error(error: impl serde::Serialize) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&error).unwrap_or_else(|error| error.to_string()))
}
