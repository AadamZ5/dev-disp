use std::{
    fmt::Display,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use dev_disp_core::{
    client::{DisplayHost, ScreenTransport},
    host::{DisplayHostResult, DisplayParameters, Screen, ScreenProvider, ScreenReadyStatus},
    util::PinnedLocalFuture,
};
use evdi::{
    buffer::{Buffer as EvdiBuffer, BufferId},
    device_node::OpenDeviceError,
    events::{AwaitEventError, Mode},
    handle::{Handle as EvdiHandle, RequestUpdateError},
    prelude::{DeviceConfig, DeviceNode},
};
use futures::stream;
use futures_util::FutureExt;
use log::{debug, error, info, warn};
use thiserror::Error;

use crate::edid_from_display_params;

const RECEIVE_INITIAL_MODE_TIMEOUT: Duration = Duration::from_secs(10);
const UPDATE_BUFFER_TIMEOUT: Duration = Duration::from_secs(5);
const BUFFER_NOT_AVAIL_DELAY: Duration = Duration::from_millis(750);
const SEND_BUFFER_TIMEOUT: Duration = Duration::from_millis(20000);
const SEND_BUFFER_TIMEOUT_MAX_COUNT: usize = 20;

#[derive(Error, Debug)]
pub enum HandleClientError {
    #[error("Unknown error while handling client")]
    Unknown,
    #[error("Failed to get or create evdi device")]
    EvdiNoDevice(NoDeviceError),
    #[error("Failed to open evdi device")]
    EvdiDeviceOpenFailed(OpenDeviceError),
    #[error("Failed to receive mode from evdi device")]
    EvdiModeChangeError(AwaitEventError),
    #[error("No buffer update was received in time")]
    EvdiRequestUpdateError(RequestUpdateError),
}

#[derive(Debug, Clone)]
pub struct EvdiScreenProvider {
    stop_flag: Arc<AtomicBool>,
}

impl EvdiScreenProvider {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl ScreenProvider for EvdiScreenProvider {
    type ScreenType = EvdiScreen;

    async fn get_screen(&self, params: DisplayParameters) -> Result<Self::ScreenType, String> {
        info!("Getting an EVDI screen for params {params}");

        let edid = edid_from_display_params(&params);

        let device = match get_evdi_device() {
            Ok(dev) => dev,
            Err(e) => {
                error!("Failed to get or create evdi device: {}", e);
                return Err(HandleClientError::EvdiNoDevice(e).to_string());
            }
        };

        let device_config = DeviceConfig::new(edid, params.resolution.0, params.resolution.1);
        debug!("Using device config: {device_config:?}");

        let unconnected_handle = match device.open() {
            Ok(handle) => handle,
            Err(e) => {
                error!("Failed to open evdi device: {}", e);
                return Err(HandleClientError::EvdiDeviceOpenFailed(e).to_string());
            }
        };
        debug!("Opened EVDI device");

        let mut handle = unconnected_handle.connect(&device_config);
        debug!("Connected to EVDI device");

        // For simplicity don't handle the mode changing after we start
        // TODO: Handle mode changes in EvdiScreen!
        let mode = match handle.events.await_mode(RECEIVE_INITIAL_MODE_TIMEOUT).await {
            Ok(mode) => mode,
            Err(e) => {
                error!("Failed to receive initial EVDI device mode: {}", e);
                return Err(HandleClientError::EvdiModeChangeError(e).to_string());
            }
        };

        info!("Received initial EVDI device mode: {mode:?}");

        // Redundant, but left here so you know this is default behavior
        // handle.enable_cursor_events(false);

        // For simplicity, use only one buffer. We may want to use more than one buffer so that you
        // can send the contents of one buffer while updating another.

        Ok(EvdiScreen::new(handle, mode))
    }
}

pub struct EvdiScreen {
    drop_count: u8,
    stop_flag: AtomicBool,
    handle: EvdiHandle,
    buffer_id: BufferId,
    bytes: Option<EvdiBuffer>,
}

const EMPTY_BYTES: [u8; 0] = [0; 0];

impl EvdiScreen {
    pub fn new(mut handle: EvdiHandle, mode: Mode) -> Self {
        let buffer_id = handle.new_buffer(&mode);

        Self {
            drop_count: 0,
            stop_flag: false.into(),
            handle,
            buffer_id,
            bytes: None,
        }
    }
}

impl Screen for EvdiScreen {
    async fn get_ready(&mut self) -> Result<ScreenReadyStatus, String> {
        if self.stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            info!("Stop flag set, exiting");
            return Ok(ScreenReadyStatus::Finished);
        }

        if let Err(e) = self
            .handle
            .request_update(self.buffer_id, UPDATE_BUFFER_TIMEOUT)
            .await
        {
            warn!("Failed to request buffer update from EVDI: {}", e);
            return Ok(ScreenReadyStatus::NotReady);
        }

        Ok(ScreenReadyStatus::Ready)
    }
    fn get_bytes(&self) -> Option<&[u8]> {
        let buf = match self.handle.get_buffer(self.buffer_id) {
            Some(buf) => buf,
            None => {
                warn!("EVDI buffer not available yet");
                return None;
            }
        };

        let bytes = buf.bytes();
        let count = bytes.len();

        debug!("Buffer retrieved with {count} bytes");

        Some(bytes)
    }

    fn close(self) -> PinnedLocalFuture<'static, Result<(), String>>
    where
        Self: Sized,
    {
        async move {
            info!("Closing EVDI screen");
            self.handle.disconnect();
            Ok(())
        }
        .boxed_local()
    }
}

#[derive(Error, Debug)]
pub struct NoDeviceError;

impl Display for NoDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No evdi device node found and failed to create one")
    }
}

pub fn get_evdi_device() -> Result<DeviceNode, NoDeviceError> {
    DeviceNode::get()
        .or_else(|| {
            debug!("Failed to get an existing device node, will try to create one");
            if DeviceNode::add() {
                debug!("Successfully added a device node");
                DeviceNode::get().or_else(|| {
                    error!("Added a device node but still can't get it!");
                    None
                })
            } else {
                error!("Failed to add a device node, do you have superuser permissions?");
                None
            }
        })
        .ok_or(NoDeviceError)
}
