use std::time::Duration;

use evdi::{
    device_node::OpenDeviceError, events::AwaitEventError, handle::RequestUpdateError,
    prelude::DeviceConfig,
};
use log::{debug, error, info};
use thiserror::Error;

use crate::{client::DevDispClient, core::get_device::NoDeviceError};

const RECEIVE_INITIAL_MODE_TIMEOUT: Duration = Duration::from_secs(3);
const UPDATE_BUFFER_TIMEOUT: Duration = Duration::from_millis(500);

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

pub async fn handle_client(client: DevDispClient) -> Result<(), HandleClientError> {
    // Handle the client connection here
    info!("Handling client: {client}");

    let device = crate::core::get_device().map_err(|e| HandleClientError::EvdiNoDevice(e))?;
    debug!("Using device: {device:?}");

    // TODO: Get screen parameters from client (create helper fn)
    // so we can generate a EDID blob and give it to evdi
    // For now, just use sample EDID

    let device_config = DeviceConfig::sample();
    debug!("Using device config: {device_config:?}");

    let unconnected_handle = device
        .open()
        .map_err(|e| HandleClientError::EvdiDeviceOpenFailed(e))?;
    debug!("Opened device");

    let mut handle = unconnected_handle.connect(&device_config);
    debug!("Connected to device");

    // For simplicity don't handle the mode changing after we start
    // TODO: Handle mode changes
    let mode = handle
        .events
        .await_mode(RECEIVE_INITIAL_MODE_TIMEOUT)
        .await
        .map_err(|e| HandleClientError::EvdiModeChangeError(e))?;

    info!("Received initial mode: {mode:?}");

    // Redundant, but left here so you know this is default behavior
    // handle.enable_cursor_events(false);

    // For simplicity, use only one buffer. We may want to use more than one buffer so that you
    // can send the contents of one buffer while updating another.
    let buffer_id = handle.new_buffer(&mode);

    loop {
        handle
            .request_update(buffer_id, UPDATE_BUFFER_TIMEOUT)
            .await
            .map_err(|e| HandleClientError::EvdiRequestUpdateError(e))?;
        let buf = handle.get_buffer(buffer_id).expect("Buffer exists");
        // Do something with the bytes
        let _bytes = buf.bytes();
        info!("Got buffer update, {} bytes", buf.bytes().len());
    }

    Ok(())
}
