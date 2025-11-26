use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::client::DisplayHost;

pub type DisplayHostResult<T> = Result<DisplayHost<T>, String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DisplayParameters {
    pub host_dev_name: String,
    pub resolution: (u32, u32),
}

impl Display for DisplayParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.host_dev_name;
        let w = self.resolution.0;
        let h = self.resolution.1;
        write!(f, "{name} ({w}x{h})")
    }
}

/// A screen provider is something that provides a screen
pub trait ScreenProvider {
    type ScreenType: Screen;

    // TODO: Better error type!
    fn get_screen(
        &self,
        params: DisplayParameters,
    ) -> impl Future<Output = Result<Self::ScreenType, String>>;
}

pub enum ScreenReadyStatus {
    Finished,
    NotReady,
    Ready,
}

/// A screen is something that provides visual data bytes to be given
/// to a client
pub trait Screen {
    // TODO: Should formatter types live here?

    // TODO: Better error type!
    fn get_ready(&mut self) -> impl Future<Output = Result<ScreenReadyStatus, String>>;
    fn get_bytes(&self) -> Option<&[u8]>;
}
