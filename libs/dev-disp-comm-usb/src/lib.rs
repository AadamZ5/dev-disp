pub mod discovery;
mod error;
pub mod protocol;
mod strategies;
pub mod transport;

use crate::{
    error::UsbConnectionError,
    strategies::android_aoa::android_accessory::connect_usb_android_accessory,
    transport::UsbScreenHostTransport,
};
use dev_disp_core::client::SomeScreenTransport;
use nusb::DeviceInfo;

pub enum UsbConnectionStrategy {
    /// Android Accessory mode, or AOA
    AndroidAccessory,
}

async fn find_usb_device(
    vendor_id: u16,
    product_id: u16,
) -> Result<DeviceInfo, UsbConnectionError> {
    nusb::list_devices()
        .await
        .map_err(|_| UsbConnectionError::ConnectionFailed)?
        .find(|device| device.vendor_id() == vendor_id && device.product_id() == product_id)
        .ok_or(UsbConnectionError::DeviceNotFound)
}

/// Connect to a USB device using the specified strategy and return a transport
pub async fn connect_usb(
    device_info: DeviceInfo,
    strategy: UsbConnectionStrategy,
) -> Result<SomeScreenTransport, UsbConnectionError> {
    let transport = match strategy {
        UsbConnectionStrategy::AndroidAccessory => connect_usb_android_accessory(device_info)
            .await
            .map(SomeScreenTransport::new),
    }?;

    Ok(transport)
}
