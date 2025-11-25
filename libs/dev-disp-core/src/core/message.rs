use crate::host::DisplayParameters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromHost<'a> {
    GetDisplayConfig,
    ScreenData(&'a [u8]),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromClient {
    UpdateDisplayConfig(DisplayParameters),
}
