pub mod error;
pub mod strategies;

#[cfg(feature = "host")]
pub mod discovery;
#[cfg(feature = "host")]
pub mod transport;

pub enum UsbConnectionStrategy {
    /// Android Accessory mode, or AOA
    AndroidAccessory,
}
