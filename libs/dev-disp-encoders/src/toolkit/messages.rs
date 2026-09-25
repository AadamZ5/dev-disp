use serde::{Deserialize, Serialize};

use crate::toolkit::encoder::CodecOption;
use dev_disp_core::host::ScreenContentParameters;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodecNegotiationServer {
    RequestPreferredEncodings(ScreenContentParameters, Vec<CodecOption>),
    RequestSetEncoding(CodecOption),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodecNegotiationClient {
    ResponsePreferredEncodings(Vec<CodecOption>),
    ResponseSetEncoding(bool),
}
