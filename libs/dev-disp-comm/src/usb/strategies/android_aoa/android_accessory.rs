use std::time::Duration;

use log::{debug, info};
use nusb::{
    Device, DeviceInfo, Interface,
    descriptors::TransferType,
    list_devices,
    transfer::{
        Bulk, ControlIn, ControlOut, ControlType, Direction, In, Out, Recipient, TransferError,
    },
};

use crate::usb::{
    error::UsbConnectionError, strategies::android_aoa::transport::AndroidAoaScreenHostTransport,
};

pub const USB_ACCESSORY_VENDOR_ID: u16 = 0x18D1;
pub const USB_ACCESSORY_DEVICE_ID: u16 = 0x2D00;
pub const USB_ACCESSORY_DEVICE_ID_ADB_DEBUG: u16 = 0x2D01;

pub const ACCESSORY_GET_PROTOCOL: u8 = 0x33;
pub const ACCESSORY_START: u8 = 0x35;

pub const ACCESSORY_RE_ENUMERATE_RETRY_COUNT: u32 = 10;

pub const DEV_DISP_DESCRIPTION: &str = "Device Display Host";
pub const DEV_DISP_MANUFACTURER: &str = "Device Display";
pub const DEV_DISP_MODEL: &str = "Screen Provider";

pub async fn connect_usb_android_accessory(
    target_device_info: DeviceInfo,
) -> Result<AndroidAoaScreenHostTransport, UsbConnectionError> {
    // Specific implementation for connecting via Android Accessory protocol
    // This would involve sending the appropriate control transfers
    // and managing the USB connection lifecycle.

    info!("Using target device: {:?}", target_device_info);

    // Connect to the device
    let target_device = target_device_info
        .open()
        .await
        .map_err(|_| UsbConnectionError::ConnectionFailed)?;
    info!("Opened device: {:?}", target_device);

    let target_device_serial = target_device_info.serial_number();
    info!("Device serial: {target_device_serial:?}");

    // Version response is 2 bytes, one u16 value
    let version_bytelen = size_of::<u16>();

    let response = target_device
        .control_in(
            ControlIn {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: ACCESSORY_GET_PROTOCOL,
                index: 0,
                value: 0,
                length: version_bytelen as u16,
            },
            Duration::from_secs(1),
        )
        .await
        .expect("Failed to send control transfer");
    debug!("Control transfer response: {:?}", response);

    if response.len() != version_bytelen {
        eprintln!("Unexpected response length: {}", response.len());
        return Err(UsbConnectionError::StrategyFailed);
    }

    let version = u16::from_le_bytes([response[0], response[1]]);
    info!("Accessory protocol version: {}", version);

    // Assuming device is good, lets enter accessory mode
    async fn send_string(handle: &Device, index: u16, string: &str) -> Result<(), TransferError> {
        let bytes = string.as_bytes();
        let mut data = Vec::with_capacity(bytes.len() + 1);
        data.extend_from_slice(bytes);
        data.push(0); // Null-terminate the string

        handle
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request: 52, // ACCESSORY_SEND_STRING
                    value: 0,
                    index,
                    data: &data,
                },
                Duration::from_secs(1),
            )
            .await
    }

    send_string(&target_device, 0, DEV_DISP_MANUFACTURER)
        .await
        .map_err(|_| UsbConnectionError::StrategyFailed)?;
    send_string(&target_device, 1, DEV_DISP_MODEL)
        .await
        .map_err(|_| UsbConnectionError::StrategyFailed)?;
    send_string(&target_device, 2, DEV_DISP_DESCRIPTION)
        .await
        .map_err(|_| UsbConnectionError::StrategyFailed)?;

    // Start accessory mode
    target_device
        .control_out(
            ControlOut {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: ACCESSORY_START,
                value: 0,
                index: 0,
                data: &[],
            },
            Duration::from_secs(1),
        )
        .await
        .expect("Failed to start accessory mode");

    // At this point, we need to re-search for the android device in AOA (accessory) mode.
    drop(target_device);

    // TODO: We may implement a better retry/strategy here that uses udev or nusb stream
    // to trigger immediate connection events with a timeout, rather than a blind sleep
    // and retry.

    let mut retries_left = ACCESSORY_RE_ENUMERATE_RETRY_COUNT;
    let wait_time = Duration::from_secs(1);
    let wait_str = format!("{}s", wait_time.as_secs());

    let mut target_device: Option<(Device, DeviceInfo)> = None;

    while retries_left > 0 {
        retries_left -= 1;

        debug!(
            "Waiting {wait_str} for device to re-enumerate in accessory mode... ({retries_left} retries left)"
        );
        futures_timer::Delay::new(wait_time).await;

        let accessory_device_info = list_devices().await.ok().and_then(|mut dev_list| {
            dev_list.find(|device_info| {
                let this_device_serial = device_info.serial_number();

                if let Some(serial) = target_device_serial {
                    if this_device_serial.is_none()
                        || this_device_serial.is_some_and(|s| s != serial)
                    {
                        return false;
                    }
                }

                device_info.vendor_id() == USB_ACCESSORY_VENDOR_ID
                    && (device_info.product_id() == USB_ACCESSORY_DEVICE_ID
                        || device_info.product_id() == USB_ACCESSORY_DEVICE_ID_ADB_DEBUG)
            })
        });

        if let Some(device) = accessory_device_info {
            debug!("Found device in accessory mode: {:?}", device);
            let accessory_handle = device
                .open()
                .await
                .map_err(|_| UsbConnectionError::ConnectionFailed)?;

            target_device = Some((accessory_handle, device));
            break;
        }
    }

    let (target_device, target_device_info) = target_device.ok_or_else(|| {
        eprintln!("Could not find device in accessory mode after retries");
        UsbConnectionError::StrategyFailed
    })?;

    // Claim the interface
    let ifc = target_device
        .claim_interface(0)
        .await
        .map_err(|_| UsbConnectionError::StrategyFailed)?;
    debug!("Claimed interface: {:?}", ifc);

    let (bulk_out_ep, bulk_in_ep) = find_bulk_endpoints(&ifc).ok_or_else(|| {
        eprintln!("Could not find bulk endpoints on interface");
        UsbConnectionError::StrategyFailed
    })?;

    let bulk_out = ifc
        .endpoint::<Bulk, Out>(bulk_out_ep)
        .map_err(|_| UsbConnectionError::StrategyFailed)?;

    let bulk_in = ifc
        .endpoint::<Bulk, In>(bulk_in_ep)
        .map_err(|_| UsbConnectionError::StrategyFailed)?;

    Ok(AndroidAoaScreenHostTransport::new(
        target_device,
        target_device_info,
        ifc,
        bulk_in,
        bulk_out,
    ))
}

/// Helper function to find the first bulk IN and OUT endpoints on an interface.
fn find_bulk_endpoints(interface: &Interface) -> Option<(u8, u8)> {
    let mut out_endpoint = None;
    let mut in_endpoint = None;

    // The interface descriptor contains information about the endpoints.
    let current_setting = if let Some(desc) = interface.descriptor() {
        desc
    } else {
        return None;
    };

    for ep in current_setting.endpoints() {
        match (ep.transfer_type(), ep.direction()) {
            // Endpoint direction is from the perspective of the host.
            // OUT is for Host -> Device communication.
            (TransferType::Bulk, Direction::Out) => {
                out_endpoint = Some(ep.address());
            }
            // IN is for Device -> Host communication.
            (TransferType::Bulk, Direction::In) => {
                in_endpoint = Some(ep.address());
            }
            _ => {}
        }
    }

    if let (Some(out_ep), Some(in_ep)) = (out_endpoint, in_endpoint) {
        Some((out_ep, in_ep))
    } else {
        None
    }
}

pub fn android_ifc_fd_to_transport(dev: Device, dev_info: DeviceInfo, ifc: Interface) {}
