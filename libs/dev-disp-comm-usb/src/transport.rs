use std::pin::Pin;

use dev_disp_core::{
    client::{DisplayHostInfo, ScreenTransport, TransportError},
    util::PinnedFuture,
};
use futures_util::{FutureExt, future};
use log::debug;
use nusb::{Device, DeviceInfo, Interface};

/// Some USB device that is ready to receive screen data
pub struct UsbScreenHostTransport {
    dev_info: DeviceInfo,
    dev: Device,
    ifc: Interface,
}

impl UsbScreenHostTransport {
    pub fn new(device: Device, device_info: DeviceInfo, ifc: Interface) -> Self {
        Self {
            dev: device,
            dev_info: device_info,
            ifc,
        }
    }

    pub fn into_device(self) -> Device {
        self.dev
    }

    pub fn device_info(&self) -> &DeviceInfo {
        &self.dev_info
    }
}

impl ScreenTransport for UsbScreenHostTransport {
    fn initialize<'s>(&'s mut self) -> PinnedFuture<'s, Result<(), TransportError>> {
        todo!()
    }

    fn get_display_config(&mut self) -> PinnedFuture<'_, Result<DisplayHostInfo, TransportError>> {
        let ifc = self.ifc.clone();

        async move {};

        future::ready(Ok(DisplayHostInfo::new(1920, 1080, vec![]))).boxed()
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>> {
        self.dev.reset().into_future().map(|_| Ok(())).boxed()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 's>> {
        let len = data.len();
        async move {
            // TODO: Implement the sending data!
            debug!("Sending {} bytes of screen data to USB device", len);
            Ok(())
        }
        .boxed()
    }
}
