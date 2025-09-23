use std::fmt::Display;

use nusb::transfer::TransferError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UsbConnectionError {
    DeviceNotFound,
    ConnectionFailed,
    TransferError(TransferError),
    StrategyFailed,
}

impl Display for UsbConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsbConnectionError::DeviceNotFound => write!(f, "USB device not found"),
            UsbConnectionError::ConnectionFailed => write!(f, "Failed to connect to USB device"),
            UsbConnectionError::TransferError(e) => write!(f, "USB transfer error: {}", e),
            UsbConnectionError::StrategyFailed => write!(f, "USB connection strategy failed"),
        }
    }
}
