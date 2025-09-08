mod error;
mod strategies;

use std::future;

use crate::{
    error::UsbConnectionError,
    strategies::android_accessory::{self, connect_usb_android_accessory},
};
use dev_disp_core::client::{DisplayHostInfo, ScreenTransport};
use futures_util::{FutureExt, Sink};
use nusb::{Device, DeviceInfo};

pub enum UsbConnectionStrategy {
    /// Android Accessory mode, or AOA
    AndroidAccessory,
}

/// Connect to a USB device using the specified strategy and return a transport
pub async fn connect_usb(
    vendor_id: u16,
    product_id: u16,
    strategy: UsbConnectionStrategy,
) -> Result<UsbScreenHostTransport, UsbConnectionError> {
    let (dev, dev_info) = match strategy {
        UsbConnectionStrategy::AndroidAccessory => {
            connect_usb_android_accessory(vendor_id, product_id).await
        }
    }?;

    Ok(UsbScreenHostTransport::new(dev, dev_info))
}

/// Some USB device that is ready to receive screen data
pub struct UsbScreenHostTransport {
    dev_info: DeviceInfo,
    dev: Device,
}

impl UsbScreenHostTransport {
    pub fn new(device: Device, device_info: DeviceInfo) -> Self {
        Self {
            dev: device,
            dev_info: device_info,
        }
    }

    pub fn into_device(self) -> Device {
        self.dev
    }
}

// TODO: Not sure if we will use this implementation or not
impl Sink<&'static [u8]> for UsbScreenHostTransport {
    type Error = ();

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: &'static [u8]) -> Result<(), Self::Error> {
        todo!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }
}

impl ScreenTransport for UsbScreenHostTransport {
    fn get_display_config(&self) -> impl Future<Output = dev_disp_core::client::DisplayHostInfo> {
        future::ready(DisplayHostInfo::new(1920, 1080, vec![]))
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        self.dev.reset().into_future().map(|_| Ok(()))
    }

    fn send_screen_data<'a>(&mut self, data: &'a [u8]) -> Result<(), Self::Error> {
        // TODO: Implement the sending data!
        Ok(())
    }
}
