use dev_disp_core::client::DisplayHost;

use crate::api::DeviceRef;

// pub fn device_ref_from_display_host<T>(display_host: DisplayHost<T>) -> DeviceRef
// where
//     T: dev_disp_core::client::SomeScreenTransport,
// {
//     DeviceRef {
//         name: display_host.name,
//         interface_key: display_host.transport.get_interface_key().to_string(),
//         interface_display: display_host.transport.get_interface_display().to_string(),
//         id: format!("{}", display_host.client_id),
//         serial: display_host.transport.get_serial().map(|s| s.to_string()),
//     }
// }
