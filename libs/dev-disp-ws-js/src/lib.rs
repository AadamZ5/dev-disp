use std::{net::SocketAddr, str::FromStr};

use js_sys::Function;
use serde::Deserialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use web_sys::console;

mod deserialize_function {
    use js_sys::Function;
    use serde::{de::Error, Deserializer};
    use wasm_bindgen::{JsCast, JsValue};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Function, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_wasm_bindgen::preserve::deserialize::<D, Function>(deserializer)
    }
}

#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct WsHandlers {
    #[serde(with = "deserialize_function")]
    pub on_request_pre_init: Function,

    #[serde(with = "deserialize_function")]
    pub on_request_device_info: Function,

    #[serde(with = "deserialize_function")]
    pub on_request_protocol_init: Function,

    #[serde(with = "deserialize_function")]
    pub on_core: Function,
}

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    alert(&format!("Hello, {}!", name));
}

#[wasm_bindgen]
pub fn test_handlers(handlers: &WsHandlers) {
    let _ = handlers.on_request_pre_init;
    let _ = handlers.on_request_device_info;
    let _ = handlers.on_request_protocol_init;
    let _ = handlers.on_core;

    console::log_1(&"Handlers received successfully".into());
}

#[wasm_bindgen]
pub fn connect_ws(address: &str, handlers: &WsHandlers) -> Result<Function, JsError> {
    // First, parse the given address
    let parsed_address = SocketAddr::from_str(address)
        .map_err(|e| JsError::new(&format!("Invalid address: {}", e)))?;

    // Create cancel channels
    let (cancel_tx, cancel_rx) = futures::channel::oneshot::channel();

    let mut cancel_token = Some(cancel_tx);

    // Return a closure that when called, will cancel the connection.
    let cancel_closure = Closure::wrap(Box::new(move || {
        // If this has already been called, do nothing.
        if let Some(token) = cancel_token.take() {
            console::log_1(&"Closing websocket connection".into());
            let _ = token.send(());
        } else {
            console::warn_1(&"Websocket connection already closed".into());
        }
    }) as Box<dyn FnMut()>);

    Ok(cancel_closure.into_js_value().into())
}
