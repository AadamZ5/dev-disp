use std::{iter::empty, pin::Pin};

use dev_disp_core::{
    client::{DisplayHost, SomeScreenTransport},
    host::{ConnectableDevice, ConnectableDeviceInfo, DeviceDiscovery, StreamingDeviceDiscovery},
    util::PinnedFuture,
};
use futures_util::{FutureExt, Stream, StreamExt};
use nusb::DeviceInfo;

use crate::usb::{
    UsbConnectionStrategy, error::UsbConnectionError,
    strategies::android_aoa::android_accessory::connect_usb_android_accessory,
};

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

/// This guy represents a USB device that we *can* connect to, but
/// have not yet.
pub struct UsbDeviceSentinel {
    device_info: nusb::DeviceInfo,
}

impl ConnectableDevice for UsbDeviceSentinel {
    type Transport = SomeScreenTransport;

    fn connect(
        self,
    ) -> PinnedFuture<
        'static,
        Result<DisplayHost<Self::Transport>, Box<dyn std::error::Error + Send + Sync>>,
    > {
        async move {
            let device_name = self.device_info.product_string().unwrap_or("Unknown");
            let device_serial = self.device_info.serial_number().unwrap_or("Unknown");

            let transport = connect_usb(
                self.device_info.clone(),
                crate::usb::UsbConnectionStrategy::AndroidAccessory,
            )
            .await?;
            Ok(dev_disp_core::client::DisplayHost::new(
                0,
                format!("{} ({})", device_name, device_serial),
                transport,
            ))
        }
        .boxed()
    }

    fn get_info(&self) -> ConnectableDeviceInfo {
        ConnectableDeviceInfo {
            name: self
                .device_info
                .product_string()
                .unwrap_or("Unknown")
                .to_string(),
            device_type: "USB".to_string(),
            id: self
                .device_info
                .serial_number()
                .unwrap_or("Unknown")
                .to_string(),
        }
    }
}

pub struct UsbDiscovery;

impl DeviceDiscovery for UsbDiscovery {
    type DeviceFacade = UsbDeviceSentinel;

    fn discover_devices(&self) -> PinnedFuture<Vec<Self::DeviceFacade>> {
        nusb_list_usb_sentinels().boxed()
    }
}

impl StreamingDeviceDiscovery for UsbDiscovery {
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceFacade>> + Send>> {
        nusb::watch_devices()
            .map(|hotplugs| hotplugs.then(|_| nusb_list_usb_sentinels()))
            .map(|st| st.boxed())
            .unwrap_or_else(|_| futures_util::stream::empty().boxed())
            .boxed()
    }
}

async fn nusb_list_usb_sentinels() -> Vec<UsbDeviceSentinel> {
    nusb::list_devices()
        .await
        .map(|dev| {
            dev.into_iter()
                .map(|device_info| UsbDeviceSentinel { device_info })
                .collect()
        })
        .unwrap_or_else(|_| empty().collect())
}
