use std::{pin::Pin, time::Duration};

use dev_disp_core::{
    client::{DisplayHostInfo, ScreenTransport, TransportError},
    util::PinnedFuture,
};
use futures_util::{FutureExt, future};
use log::debug;
use nusb::{
    Device, DeviceInfo, Endpoint, Interface,
    transfer::{Buffer, Bulk, In, Out},
};

const USB_TIMEOUT: Duration = Duration::from_millis(200);

pub struct AndroidAoaScreenHostTransport {
    dev_info: DeviceInfo,
    dev: Device,
    ifc: Interface,
    bulk_in: Endpoint<Bulk, In>,
    bulk_out: Endpoint<Bulk, Out>,
    out_buffer: Option<Buffer>,
}

impl AndroidAoaScreenHostTransport {
    pub fn new(
        device: Device,
        device_info: DeviceInfo,
        ifc: Interface,
        bulk_in: Endpoint<Bulk, In>,
        bulk_out: Endpoint<Bulk, Out>,
    ) -> Self {
        Self {
            dev: device,
            dev_info: device_info,
            ifc,
            bulk_in,
            bulk_out,
            out_buffer: None,
        }
    }

    pub fn into_device(self) -> Device {
        self.dev
    }

    pub fn device_info(&self) -> &DeviceInfo {
        &self.dev_info
    }
}

impl ScreenTransport for AndroidAoaScreenHostTransport {
    fn initialize<'s>(&'s mut self) -> PinnedFuture<'s, Result<(), TransportError>> {
        let data = "test-data".as_bytes();

        let mut out_buffer = Buffer::new(data.len());
        out_buffer
            .extend_fill(data.len(), 0)
            .copy_from_slice(&data[..data.len()]);

        debug!(
            "Sending {} bytes of screen data to USB device (buffer size {})",
            data.len(),
            out_buffer.capacity()
        );

        async move {
            self.bulk_out.submit(out_buffer);
            let completion = self.bulk_out.next_complete().await;
            self.out_buffer.replace(completion.buffer);
            completion
                .status
                .map_err(|e| TransportError::Other(Box::new(e)))
        }
        .boxed()
    }

    fn get_display_config(&mut self) -> PinnedFuture<'_, Result<DisplayHostInfo, TransportError>> {
        future::ready(Ok(DisplayHostInfo::new(1920, 1080, vec![]))).boxed()
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>> {
        self.dev.reset().into_future().map(|_| Ok(())).boxed()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 's>> {
        // TODO: Don't do this below, use compression!
        // Use test data for now
        let data = "test-data".as_bytes();

        let mut out_buffer = self
            .out_buffer
            .take()
            .and_then(|buffer| {
                if buffer.len() >= data.len() {
                    Some(buffer)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.bulk_out.allocate(data.len()));
        out_buffer.clear();

        debug!(
            "Sending {} bytes of screen data to USB device (buffer size {}/{})",
            data.len(),
            out_buffer.len(),
            out_buffer.capacity()
        );
        out_buffer
            .extend_fill(data.len(), 0)
            .copy_from_slice(&data[..data.len()]);

        async move {
            self.bulk_out.submit(out_buffer);
            let completion = self.bulk_out.next_complete().await;
            self.out_buffer.replace(completion.buffer);
            completion
                .status
                .map_err(|e| TransportError::Other(Box::new(e)))
        }
        .boxed()
    }
}
