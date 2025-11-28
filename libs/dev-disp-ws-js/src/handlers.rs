use dev_disp_comm::websocket::messages::DisplayParameters;
use js_sys::Function;
use serde::Deserialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

mod deserialize_function {
    use js_sys::Function;
    use serde::Deserializer;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Function, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_wasm_bindgen::preserve::deserialize::<D, Function>(deserializer)
    }
}

mod deserialize_option_function {
    use js_sys::Function;
    use serde::Deserializer;
    use wasm_bindgen::{JsCast, JsValue};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Function>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_wasm_bindgen::preserve::deserialize::<D, JsValue>(deserializer)?;
        if value.is_undefined() || value.is_null() {
            Ok(None)
        } else {
            let func: Function = value.dyn_into().map_err(|e| {
                serde::de::Error::custom(format!(
                    "Expected a function, got a {}",
                    e.js_typeof().as_string().unwrap_or_default()
                ))
            })?;
            Ok(Some(func))
        }
    }
}

#[wasm_bindgen]
pub struct DevDispEvent {
    #[wasm_bindgen(getter_with_clone)]
    pub error: Option<JsValue>,
    #[wasm_bindgen(getter_with_clone)]
    pub data: Option<JsValue>,
}

#[derive(Tsify, Deserialize, Clone, Debug)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct JsDisplayParameters {
    pub name: String,
    pub resolution: (u32, u32),
}

impl Into<DisplayParameters> for JsDisplayParameters {
    fn into(self) -> DisplayParameters {
        DisplayParameters {
            host_dev_name: self.name,
            resolution: self.resolution,
        }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_FN_TYPE_CONTENT: &str = r#"
export type WsNotificationFunction = (event: DevDispEvent) => void;
"#;

// TODO: Define a return type
#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_REQUEST_DEVICE_INFO: &str = r#"
export type WsHandlerRequestDeviceInfo = (event: DevDispEvent) => object;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_SCREEN_DATA: &str = r#"
export type WsHandlerScreenData = (event: DevDispEvent) => void;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_REQUEST_DISPLAY_PARAMETERS: &str = r#"
export type WsHandlerRequestDisplayParameters = (event: DevDispEvent) => JsDisplayParameters;
"#;

#[derive(Tsify, Deserialize, Clone, Debug)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct WsHandlers {
    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_pre_init: Option<Function>,

    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_pre_init_success: Option<Function>,

    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_protocol_init: Option<Function>,

    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_protocol_init_success: Option<Function>,

    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_core: Option<Function>,

    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_connect: Option<Function>,
    #[serde(with = "deserialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_disconnect: Option<Function>,

    #[serde(with = "deserialize_function")]
    #[tsify(type = "WsHandlerRequestDeviceInfo")]
    pub handle_request_device_info: Function,

    #[serde(with = "deserialize_function")]
    #[tsify(type = "WsHandlerScreenData")]
    pub handle_screen_data: Function,

    #[serde(with = "deserialize_function")]
    #[tsify(type = "WsHandlerRequestDisplayParameters")]
    pub handle_request_display_parameters: Function,
}
