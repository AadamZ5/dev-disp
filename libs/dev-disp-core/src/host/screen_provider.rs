use std::fmt::Display;

use edid::{
    Edid, EdidDigitalBitDepth, EdidDigitalVideoInterface,
    descriptors::{DigitalSyncFlags, EdidDescriptor},
};
use futures::{FutureExt, future};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    client::DisplayHost,
    util::{PinnedFuture, PinnedLocalFuture},
};

/// Well-known virtual screen pixel formats we can handle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VirtualScreenPixelFormat {
    #[default]
    Rgb888,
    Bgr888,
    Rgba8888,
    Bgra8888,
    Argb8888,
    Abgr8888,
}

/// Parameters explaining what kind of screen data the virtual screen subsystem is producing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFormatParameters {
    /// Our intermediate pixel format representation.
    pub format: VirtualScreenPixelFormat,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Stride in bytes
    ///
    /// This is the amount of bytes between the start of one row of pixels and the start of the next row.
    /// This may be more than width * bytes_per_pixel due to padding or alignment requirements.
    ///
    /// In this example, notice the padding at the end of each row to align to the stride:
    /// ```text
    /// [ <----- ... Stride bytes ... -----> ]
    ///
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row 1
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row 2
    /// ...
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row h
    ///
    /// [ <-- Pixel Data ---> ][ < Padding > ]
    /// ```
    /// The padding at the end of each row helps align the pixel data to memory boundaries.
    /// The stride represents the total number of bytes between the start of one row and the start
    /// of the next row, including any padding.
    pub stride: u32,

    /// Any additional meta data associated with the screen output
    pub meta_data: Option<HashMap<String, String>>,
}

/// Represents the content parameters we will be attempting to transmit. The
/// [Self::width] and [Self::height] represent the dimensions of the content we will be transmitting, which
/// is usually the same as those present in [Self::encoder_input_parameters]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContentParameters {
    /// TODO: Will this ever differ from [virtual_screen_source_parameters.width]
    pub width: u32,
    /// TODO: Will this ever differ from [virtual_screen_source_parameters.height]
    pub height: u32,
    pub bitrate: u32,
    pub fps: u32,
    /// The parameters of the virtual screen generated output data, that will
    /// be sent to the encoder.
    pub virtual_screen_format_parameters: ScreenFormatParameters,
}

// TODO: Change `DisplayHost<T>` to be a type-changed result type like `FinishedDisplayHost` (no <T>) that allows the transport to de-initialize properly
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

impl From<DisplayParameters> for Edid {
    fn from(params: DisplayParameters) -> Self {
        Edid {
            display_parameters: edid::EdidDisplayParameters::Digital((
                EdidDigitalBitDepth::Eight,
                EdidDigitalVideoInterface::DisplayPort,
            )),
            descriptor_1: Some(EdidDescriptor::DetailedTiming(
                edid::descriptors::EdidDetailedTimingDescriptor {
                    pixel_clock: 14850,
                    horizontal_active_pixels: params.resolution.0 as u16,
                    vertical_active_lines: params.resolution.1 as u16,

                    // I totally guessed with the rest of these values. They
                    // may not matter for our use case using a virtual display.
                    // TODO: Properly calculate these values using a timing calculator!
                    // https://edidcraft.com/?tab=timing-calculator-tab
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
    fn get_format_parameters(&self) -> ScreenFormatParameters;

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
