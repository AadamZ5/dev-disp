use std::fmt::Display;

use edid::{
    Edid, EdidDigitalBitDepth, EdidDigitalVideoInterface, EdidEstablishedTimingSupport,
    descriptors::{DigitalSyncFlags, EdidDescriptor},
};
use futures::{FutureExt, future};
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    client::DisplayHost,
    host::ScreenOutputParameters,
    util::{PinnedFuture, PinnedLocalFuture},
};

pub type DisplayHostResult<T> = Result<DisplayHost<T>, (DisplayHost<T>, String)>;

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

impl Into<Edid> for DisplayParameters {
    fn into(self) -> Edid {
        Edid {
            display_parameters: edid::EdidDisplayParameters::Digital((
                EdidDigitalBitDepth::Eight,
                EdidDigitalVideoInterface::DisplayPort,
            )),
            descriptor_1: Some(EdidDescriptor::DetailedTiming(
                edid::descriptors::EdidDetailedTimingDescriptor {
                    pixel_clock: 14850,
                    horizontal_active_pixels: self.resolution.0 as u16,
                    vertical_active_lines: self.resolution.1 as u16,

                    // I totally guessed with the rest of these values. They
                    // may not matter for our use case using a virtual display.
                    horizontal_blanking_pixels: 100,
                    vertical_blanking_lines: 25,
                    horizontal_sync_offset: 10,
                    horizontal_sync_pulse_width: 5,
                    vertical_sync_offset: 10,
                    vertical_sync_pulse_width: 5,
                    horizontal_image_size_mm: 100,
                    vertical_image_size_mm: 50,
                    horizontal_border: 0,
                    vertical_border: 0,
                    features: edid::descriptors::FeaturesMap {
                        signal_type: edid::descriptors::SignalInterfaceType::NonInterlaced,
                        stereo_mode: edid::descriptors::StereoMode::BiInterleavedLeftImageEvenLines,
                        sync_type: edid::descriptors::SyncType::Digital(DigitalSyncFlags {
                            ..Default::default()
                        }),
                    },
                },
            )),
            ..Default::default()
        }
    }
}

/// A screen provider is something that provides a screen
pub trait ScreenProvider: Clone + Send + Sync + 'static {
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
    // TODO: Should encoder types live here?
    fn get_format_parameters(&self) -> ScreenOutputParameters;

    /// Background task started before the screen is used during looping. Cannot
    /// hold onto self reference.
    fn background<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), String>> {
        debug!("Default screen background impl");
        future::ready(Ok(())).boxed()
    }

    // TODO: Better error type!
    fn get_ready(&mut self) -> impl Future<Output = Result<ScreenReadyStatus, String>>;
    fn get_bytes(&self) -> Option<&[u8]>;

    // TODO: Better error type!
    fn close(self) -> PinnedLocalFuture<'static, Result<(), String>>
    where
        // Hmm, what happens when we `Box<dyn Screen>`?
        Self: Sized,
    {
        future::ready(Ok(())).boxed()
    }
}
