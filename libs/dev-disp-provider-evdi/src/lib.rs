use std::{
    fmt::Display,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use dev_disp_core::{
    client::{DisplayHost, ScreenTransport},
    host::{DisplayHostResult, ScreenProvider},
};
use evdi::{
    device_node::OpenDeviceError,
    events::AwaitEventError,
    handle::RequestUpdateError,
    prelude::{DeviceConfig, DeviceNode},
};
use futures_util::FutureExt;
use log::{debug, error, info, warn};
use thiserror::Error;

const RECEIVE_INITIAL_MODE_TIMEOUT: Duration = Duration::from_secs(10);
const UPDATE_BUFFER_TIMEOUT: Duration = Duration::from_secs(5);

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
    fn handle_display_host<T>(
        &self,
        mut host: DisplayHost<T>,
    ) -> impl Future<Output = DisplayHostResult<T>>
    where
        T: ScreenTransport + 'static,
    {
        async move {
            // Handle the display-host connection here
            info!("Handling display-host: {host}");

            async fn close_dev(host: &mut DisplayHost<impl ScreenTransport>) {
                if let Err(e) = host.close().await {
                    error!("Error closing display host");
                }
            }

            let device = match get_evdi_device() {
                Ok(dev) => dev,
                Err(e) => {
                    error!("Failed to get or create evdi device: {}", e);
                    close_dev(&mut host).await;
                    return Err(HandleClientError::EvdiNoDevice(e));
                }
            };
            debug!("Using device: {device:?}");

            // TODO: Get display parameters from client
            let device_config = host.get_display_config().await;
            // let device_config = DeviceConfig::new(
            //     device_config.get_edid(),
            //     device_config.width_px,
            //     device_config.height_px,
            // );
            // TODO: Don't use sample config
            let device_config = DeviceConfig::sample();
            debug!("Using device config: {device_config:?}");

            let unconnected_handle = match device.open() {
                Ok(handle) => handle,
                Err(e) => {
                    error!("Failed to open evdi device: {}", e);
                    close_dev(&mut host).await;
                    return Err(HandleClientError::EvdiDeviceOpenFailed(e));
                }
            };
            debug!("Opened device");

            let mut handle = unconnected_handle.connect(&device_config);
            debug!("Connected to device");

            // For simplicity don't handle the mode changing after we start
            // TODO: Handle mode changes
            let mode = match handle.events.await_mode(RECEIVE_INITIAL_MODE_TIMEOUT).await {
                Ok(mode) => mode,
                Err(e) => {
                    error!("Failed to receive initial mode: {}", e);
                    close_dev(&mut host).await;
                    return Err(HandleClientError::EvdiModeChangeError(e));
                }
            };

            info!("Received initial mode: {mode:?}");

            // Redundant, but left here so you know this is default behavior
            // handle.enable_cursor_events(false);

            // For simplicity, use only one buffer. We may want to use more than one buffer so that you
            // can send the contents of one buffer while updating another.
            let buffer_id = handle.new_buffer(&mode);

            let mut drop_count = 0;
            let max_drop_count = 100;

            loop {
                if self.stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    info!("Stop flag set, exiting");
                    close_dev(&mut host).await;
                    break;
                }

                if let Err(e) = handle
                    .request_update(buffer_id, UPDATE_BUFFER_TIMEOUT)
                    .await
                {
                    warn!("Failed to request buffer update: {}", e);
                    continue;
                }
                let buf = handle.get_buffer(buffer_id).expect("Buffer exists");
                // Do something with the bytes
                let _bytes = buf.bytes();
                if let Err(_) = host.send_screen_data(_bytes).await {
                    error!("Dropped some screen data to host");
                    drop_count += 1;
                } else {
                    drop_count = 0;
                }

                if drop_count >= max_drop_count {
                    error!("Too many dropped frames, exiting");
                    close_dev(&mut host).await;
                    break;
                }
            }

            Ok(host)
        }
        .map(|res: Result<DisplayHost<T>, HandleClientError>| match res {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("Error handling client: {}", e)),
        })
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
