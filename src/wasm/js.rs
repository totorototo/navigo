use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(msg: &str);
}

/// Log a warning to the browser console.
pub(super) fn warn(msg: &str) {
    console_warn(msg);
}

pub(super) fn deserialize<T>(value: JsValue, context: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    serde_wasm_bindgen::from_value(value)
        .map_err(|e| warn(&format!("navigo: {context} parse error: {e}")))
        .ok()
}

pub(super) fn serialize<T>(value: &T, context: &str) -> Option<JsValue>
where
    T: Serialize + ?Sized,
{
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| warn(&format!("navigo: {context} serialization error: {e}")))
        .ok()
}

pub(super) fn serialize_or_null<T>(value: &T, context: &str) -> JsValue
where
    T: Serialize + ?Sized,
{
    serialize(value, context).unwrap_or(JsValue::NULL)
}

pub(super) fn serialize_or_undefined<T>(value: &T, context: &str) -> JsValue
where
    T: Serialize + ?Sized,
{
    serialize(value, context).unwrap_or(JsValue::UNDEFINED)
}

pub(super) fn serialize_silent<T>(value: &T) -> Option<JsValue>
where
    T: Serialize + ?Sized,
{
    serde_wasm_bindgen::to_value(value).ok()
}
