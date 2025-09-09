use crate::client::{DisplayHost, ScreenTransport};

pub trait ConnectableDevice<T>
where
    T: ScreenTransport,
{
    fn connect(&self) -> DisplayHost<T>;
}

pub trait DeviceDiscovery {
    type Transport: ScreenTransport;

    fn discover_devices(&self) -> Vec<DisplayHost<Self::Transport>>;
}
