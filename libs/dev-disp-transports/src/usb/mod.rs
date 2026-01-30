pub mod error;
pub mod strategies;

#[cfg(feature = "usb-host")]
pub mod discovery;

pub enum UsbConnectionStrategy {
    /// Android Accessory mode, or AOA
    AndroidAccessory,
}
