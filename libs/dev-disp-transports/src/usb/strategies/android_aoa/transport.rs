use std::{pin::Pin, time::Duration};

use dev_disp_core::{
    client::{ScreenTransport, TransportError},
    host::DisplayParameters,
    util::PinnedFuture,
};
use futures_util::{FutureExt, future};
use log::debug;
use nusb::{
    Device, DeviceInfo, Endpoint, Interface,
    transfer::{Buffer, Bulk, In, Out},
};

use crate::usb::strategies::android_aoa::protocol::{Message, MessageToAndroid};

const USB_TIMEOUT: Duration = Duration::from_millis(200);

/// The Android AOA Screen Host Transport
///
/// This facilitates communication to an Android device
/// running corresponding software, via AOA (Android
/// Open Accessory) mode.
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
        let mut data = [0u8; 512];
        let data_size = match MessageToAndroid::GetScreenInfo(Message { id: 0, payload: () })
            .serialize_into(&mut data)
        {
            Ok(size) => size,
            Err(e) => return future::err(TransportError::Other(Box::new(e))).boxed(),
        };

        let mut out_buffer = Buffer::new(data.len());
        out_buffer
            .extend_fill(data_size, 0)
            .copy_from_slice(&data[..data_size]);

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

    fn get_display_config(
        &mut self,
    ) -> PinnedFuture<'_, Result<DisplayParameters, TransportError>> {
        future::ready(Ok(DisplayParameters {
            host_dev_name: self
                .dev_info
                .serial_number()
                .unwrap_or("Unknown")
                .to_string(),
            resolution: (1920, 1080),
        }))
        .boxed()
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>> {
        self.dev.reset().into_future().map(|_| Ok(())).boxed()
    }

    fn get_preferred_encodings(
        &mut self,
        _configurations: Vec<dev_disp_core::host::EncoderPossibleConfiguration>,
    ) -> PinnedFuture<
        '_,
        Result<Vec<dev_disp_core::host::EncoderPossibleConfiguration>, TransportError>,
    > {
        todo!("Not implemented yet for Android AOA transport")
    }

    fn set_encoding(
        &mut self,
        _configuration: dev_disp_core::host::EncoderPossibleConfiguration,
    ) -> PinnedFuture<'_, Result<(), TransportError>> {
        todo!("Not implemented yet for Android AOA transport")
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 's>>
    where
        'a: 's,
    {
        let screen_update = MessageToAndroid::ScreenUpdate(Message {
            id: 0,
            payload: data.to_vec(),
        });
        let heaped_data = match screen_update.serialize() {
            Ok(vec) => vec,
            Err(e) => return future::err(TransportError::Other(Box::new(e))).boxed(),
        };

        let mut out_buffer = self
            .out_buffer
            .take()
            .and_then(|buffer| {
                if buffer.len() >= heaped_data.len() {
                    Some(buffer)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.bulk_out.allocate(heaped_data.len()));
        out_buffer.clear();

        out_buffer
            .extend_fill(heaped_data.len(), 0)
            .copy_from_slice(&heaped_data[..heaped_data.len()]);

        debug!(
            "Sending {} bytes of screen data to USB device (buffer size {}/{})",
            heaped_data.len(),
            out_buffer.len(),
            out_buffer.capacity()
        );

        let data_len = heaped_data.len();

        async move {
            let now = std::time::Instant::now();
            self.bulk_out.submit(out_buffer);
            let completion = self.bulk_out.next_complete().await;
            let elapsed = now.elapsed();
            let kb_s = (data_len as f64 / 1024.0) / (elapsed.as_secs_f64());
            debug!(
                "Sent {} bytes of screen data to USB device in {}ms ({}kb/s)",
                data_len,
                elapsed.as_millis(),
                kb_s
            );
            self.out_buffer.replace(completion.buffer);
            completion
                .status
                .map_err(|e| TransportError::Other(Box::new(e)))
        }
        .boxed()
    }
}
