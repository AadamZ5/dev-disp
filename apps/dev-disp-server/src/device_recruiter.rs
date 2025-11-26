use std::any::Any;

use dev_disp_core::host::StreamingDeviceDiscovery;

/// Device recruiter holds many device sources, and emits devices
/// that the user chooses to recruit
///
/// TODO
pub struct DeviceRecruiter {
    device_sources: Vec<Box<dyn Any>>,
}

impl DeviceRecruiter {}
