use dev_disp_core::{
    host::{ConnectableDevice, DeviceDiscovery},
    util::PinnedFuture,
};
use futures_util::FutureExt;

use crate::websocket::ws_transport::WsTransport;

pub struct WsDeviceCandidate {}

impl ConnectableDevice for WsDeviceCandidate {
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
    type DeviceCandidate = WsDeviceCandidate;

    fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceCandidate>> {
        async { vec![] }.boxed()
    }
}
