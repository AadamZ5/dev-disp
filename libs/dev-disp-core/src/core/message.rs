use crate::host::DisplayParameters;
use serde::{Deserialize, Serialize};

/// A message coming from the data source, aka where the screen
/// data is provided (ex: the "host" laptop)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromSource<'a> {
    /// A request for the client device's current display parameters
    GetDisplayParametersRequest,
    /// A command do put the given screen data.
    ///
    /// TODO: Allow region updates, or other metadata about the update
    /// TODO: Encode compression type! Or bundle in a library!
    PutScreenData(&'a [u8]),
}

/// A message coming from the client, where the screen data is painted
/// (ex: a mobile phone presenting screen data from a laptop)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DevDispMessageFromClient {
    DisplayParametersUpdate(DisplayParameters),
}
