use dev_disp_encoders::toolkit::codecs::{
    Av1Parameters, Codec, H264Parameters, HevcParameters, RawParameters, Vp09Parameters,
};
use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const TS_VIRTUAL_SCREEN_PIXEL_FORMAT: &'static str = r#"
export type VirtualScreenPixelFormat = "Rgb888" | "Bgr888" | "Rgba8888" | "Bgra8888" | "Argb8888" | "Abgr8888";
"#;

#[wasm_bindgen(typescript_custom_section)]
const TS_CODEC_PARAMETER_TYPES: &'static str = r#"
export type RawParameters = {pixelFormat: VirtualScreenPixelFormat, width: number, height: number, stride: number};
export type Av1Parameters = {profile: number, level: number, constraint_flags: number, bit_depth: number};
export type Vp09Parameters = {profile: string, level: number, bit_depth: number};
export type HevcParameters = {profile: number, compatibility: number, level: number, tier: string, constraints: number};
export type H264Parameters = {profile: number, level: number, constraint_flags: number};
export type Vp8Parameters = null;

export type JsCodecParameters = RawParameters | Av1Parameters | Vp8Parameters | Vp09Parameters | HevcParameters | H264Parameters;
"#;

// Need to mirror the Codec type from the core lib
#[derive(Tsify, Serialize, Deserialize, Clone, Debug)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase", tag = "codecFamily", content = "parameters")]
pub enum JsCodec {
    Raw(#[tsify(type = "RawParameters")] RawParameters),
    Av1(#[tsify(type = "Av1Parameters")] Av1Parameters),
    Vp8(#[tsify(type = "Vp8Parameters")] Option<()>),
    Vp09(#[tsify(type = "Vp09Parameters")] Vp09Parameters),
    Hevc(#[tsify(type = "HevcParameters")] HevcParameters),
    H264(#[tsify(type = "H264Parameters")] H264Parameters),
}

impl From<JsCodec> for Codec {
    fn from(js_codec: JsCodec) -> Self {
        match js_codec {
            JsCodec::Raw(params) => Codec::Raw(params),
            JsCodec::Av1(params) => Codec::Av1(params),
            JsCodec::Vp8(_params) => Codec::Vp8,
            JsCodec::Vp09(params) => Codec::Vp09(params),
            JsCodec::Hevc(params) => Codec::Hevc(params),
            JsCodec::H264(params) => Codec::H264(params),
        }
    }
}
impl From<Codec> for JsCodec {
    fn from(codec: Codec) -> Self {
        match codec {
            Codec::Av1(params) => JsCodec::Av1(params),
            Codec::Vp8 => JsCodec::Vp8(None),
            Codec::Vp09(params) => JsCodec::Vp09(params),
            Codec::Hevc(params) => JsCodec::Hevc(params),
            Codec::H264(params) => JsCodec::H264(params),
            Codec::Raw(params) => JsCodec::Raw(params),
        }
    }
}
