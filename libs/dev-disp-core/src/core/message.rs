use std::fmt::Display;

use crate::host::{DisplayParameters, EncoderPossibleConfiguration};
use serde::{Deserialize, Serialize};

/// A message coming from the data source, aka where the screen
/// data is provided (ex: the "host" laptop)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromSource<'a> {
    /// A request for the client device's current display parameters
    GetDisplayParametersRequest,

    /// A request to get the preferred encoding from a set of possible configurations.
    GetPreferredEncodingRequest(Vec<EncoderPossibleConfiguration>),

    SetEncoding(EncoderPossibleConfiguration),

    /// A command do put the given screen data.
    ///
    /// TODO: Allow region updates, or other metadata about the update
    /// TODO: Encode compression type! Or bundle in a library!
    PutScreenData(&'a [u8]),
}

impl Display for DevDispMessageFromSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevDispMessageFromSource::GetDisplayParametersRequest => {
                write!(f, "GetDisplayParametersRequest")
            }
            DevDispMessageFromSource::GetPreferredEncodingRequest(configs) => {
                write!(
                    f,
                    "GetPreferredEncodingRequest ({} configurations)",
                    configs.len()
                )
            }
            DevDispMessageFromSource::PutScreenData(data) => {
                write!(f, "PutScreenData ({} bytes)", data.len())
            }
            DevDispMessageFromSource::SetEncoding(config) => {
                write!(f, "SetEncoding ({})", config.encoder_name)
            }
        }
    }
}

/// A message coming from the client, where the screen data is painted
/// (ex: a mobile phone presenting screen data from a laptop)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromClient {
    /// Response to GetPreferredEncodingRequest message
    EncodingPreferenceResponse(Vec<EncoderPossibleConfiguration>),
    /// Response to SetEncoding message, true if successful
    SetEncodingResponse(bool),
    /// Update with the current display parameters of the client device
    DisplayParametersUpdate(DisplayParameters),
}

impl Display for DevDispMessageFromClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevDispMessageFromClient::DisplayParametersUpdate(params) => {
                write!(f, "DisplayParametersUpdate ({})", params)
            }
            DevDispMessageFromClient::EncodingPreferenceResponse(configs) => {
                write!(
                    f,
                    "EncodingPreferenceResponse ({} configurations)",
                    configs.len()
                )
            }
            DevDispMessageFromClient::SetEncodingResponse(success) => {
                write!(f, "SetEncodingResponse (success: {})", success)
            }
        }
    }
}
