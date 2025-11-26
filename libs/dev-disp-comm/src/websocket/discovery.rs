use dev_disp_core::{
    host::{ConnectableDevice, DeviceDiscovery},
    util::PinnedFuture,
};

use crate::websocket::ws_transport::WsTransport;

pub struct WsDeviceSentinal {}

impl ConnectableDevice for WsDeviceSentinal {
    type Transport = WsTransport;

    fn connect(
        self,
    ) -> dev_disp_core::util::PinnedFuture<
        'static,
        Result<
            dev_disp_core::client::DisplayHost<Self::Transport>,
            Box<dyn std::error::Error + Send + Sync>,
        >,
    > {
        todo!()
    }

    fn get_info(&self) -> dev_disp_core::host::ConnectableDeviceInfo {
        todo!()
    }
}

pub struct WsDiscovery {}

impl DeviceDiscovery for WsDiscovery {
    type DeviceFacade = WsDeviceSentinal;

    fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceFacade>> {
        async { vec![] }
    }
}
