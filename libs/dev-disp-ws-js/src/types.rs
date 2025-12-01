use std::collections::HashMap;

use dev_disp_comm::websocket::messages::{
    DisplayParameters, EncoderPossibleConfiguration, WsMessageDeviceInfo,
};
use js_sys::Function;
use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::*;

mod serialize_function {
    use js_sys::Function;
    use serde::{Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Function, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_wasm_bindgen::preserve::deserialize::<D, Function>(deserializer)
    }

    pub fn serialize<S>(func: &Function, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde_wasm_bindgen::preserve::serialize(func, serializer)
    }
}

mod serialize_option_function {
    use js_sys::Function;
    use serde::{Deserializer, Serializer};
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

    #[allow(dead_code)]
    pub fn serialize<S>(func: &Option<Function>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match func {
            Some(f) => serde_wasm_bindgen::preserve::serialize(f, serializer),
            None => serializer.serialize_none(),
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

impl From<JsDisplayParameters> for DisplayParameters {
    fn from(val: JsDisplayParameters) -> Self {
        DisplayParameters {
            host_dev_name: val.name,
            resolution: val.resolution,
        }
    }
}

impl From<JsDisplayParameters> for WsMessageDeviceInfo {
    fn from(val: JsDisplayParameters) -> WsMessageDeviceInfo {
        WsMessageDeviceInfo {
            name: val.name,
            resolution: val.resolution,
        }
    }
}

#[derive(Tsify, Serialize, Deserialize, Clone, Debug)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct JsEncoderPossibleConfiguration {
    pub encoder_name: String,
    pub encoder_family: String,
    pub parameters: HashMap<String, String>,
}

impl From<JsEncoderPossibleConfiguration> for EncoderPossibleConfiguration {
    fn from(val: JsEncoderPossibleConfiguration) -> Self {
        EncoderPossibleConfiguration {
            encoder_name: val.encoder_name,
            encoder_family: val.encoder_family,
            parameters: val.parameters,
        }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_FN_TYPE_CONTENT: &str = r#"
export type WsNotificationFunction = (event: DevDispEvent) => void;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_REQUEST_DEVICE_INFO: &str = r#"
export type WsHandlerRequestDeviceInfo = (event: DevDispEvent) => JsDisplayParameters;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_SCREEN_DATA: &str = r#"
export type WsHandlerScreenData = (event: DevDispEvent | null) => void;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_REQUEST_DISPLAY_PARAMETERS: &str = r#"
export type WsHandlerRequestDisplayParameters = (event: DevDispEvent) => JsDisplayParameters;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_REQUEST_PREFERRED_ENCODINGS: &str = r#"
export type WsHandlerRequestPreferredEncodings = (event: JsEncoderPossibleConfiguration[]) => Promise<JsEncoderPossibleConfiguration[]>;
"#;

#[wasm_bindgen(typescript_custom_section)]
const WS_HANDLER_SET_ENCODING: &str = r#"
export type WsHandlerSetEncoding = (event: JsEncoderPossibleConfiguration) => void;
"#;

#[derive(Tsify, Deserialize, Clone, Debug)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct WsHandlers {
    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_pre_init: Option<Function>,

    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_pre_init_success: Option<Function>,

    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_protocol_init: Option<Function>,

    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_protocol_init_success: Option<Function>,

    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_core: Option<Function>,

    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_connect: Option<Function>,
    #[serde(with = "serialize_option_function", default)]
    #[tsify(type = "WsNotificationFunction", optional)]
    pub on_disconnect: Option<Function>,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsHandlerRequestDeviceInfo")]
    pub handle_request_device_info: Function,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsHandlerScreenData")]
    pub handle_screen_data: Function,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsHandlerRequestDisplayParameters")]
    pub handle_request_display_parameters: Function,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsHandlerRequestPreferredEncodings")]
    pub handle_request_preferred_encoding: Function,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsHandlerSetEncoding")]
    pub handle_set_encoding: Function,
}

#[wasm_bindgen(typescript_custom_section)]
const WS_DISPATCHER_UPDATE_DISPLAY_PARAMETERS: &str = r#"
export type WsDispatcherUpdateDisplayParameters = (event: JsDisplayParameters) => void;
"#;

#[derive(Tsify, Serialize, Deserialize, Clone, Debug)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct WsDispatchers {
    #[serde(with = "serialize_function")]
    #[tsify(type = "() => void")]
    pub close_connection: Function,

    #[serde(with = "serialize_function")]
    #[tsify(type = "WsDispatcherUpdateDisplayParameters")]
    pub update_display_parameters: Function,
}
