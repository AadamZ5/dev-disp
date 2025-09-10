use std::{pin::Pin, time::Duration};

use futures_core::Stream;
use futures_util::{FutureExt, StreamExt, stream::unfold};

use crate::{
    client::{DisplayHost, ScreenTransport},
    util::PinnedFuture,
};

#[derive(Debug, Clone)]
pub struct ConnectableDeviceInfo {
    pub name: String,
    pub device_type: String,
    pub id: String,
}

pub trait ConnectableDevice {
    type Transport: ScreenTransport;

    fn connect(
        self,
    ) -> PinnedFuture<Result<DisplayHost<Self::Transport>, Box<dyn std::error::Error + Send + Sync>>>;

    fn get_info(&self) -> ConnectableDeviceInfo;
}

pub trait DeviceDiscovery {
    type DeviceFacade: ConnectableDevice;

    fn discover_devices(&self) -> PinnedFuture<Vec<Self::DeviceFacade>>;
}

pub trait StreamingDeviceDiscovery: DeviceDiscovery {
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceFacade>> + Send>>;
}

pub struct PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery,
{
    inner: D,
    interval: Duration,
    sleep_factory: SleepFactory,
}

/// A factory function that creates a sleep future for the given duration.
/// I need to get me one of these
pub type SleepFactory = fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send>>;

impl<D> PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery,
{
    pub fn new(inner: D, interval: Duration, sleep_factory: SleepFactory) -> Self {
        Self {
            inner,
            interval,
            sleep_factory,
        }
    }
}

impl<D> DeviceDiscovery for PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery,
{
    type DeviceFacade = D::DeviceFacade;

    fn discover_devices(&self) -> PinnedFuture<Vec<Self::DeviceFacade>> {
        self.inner.discover_devices()
    }
}

impl<D> StreamingDeviceDiscovery for PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery + Send + 'static,
{
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceFacade>> + Send>> {
        let initial = self.inner.discover_devices();

        let poll_stream = unfold(self, |this| async move {
            (this.sleep_factory)(this.interval).await;
            let devices = this.inner.discover_devices().await;
            Some((devices, this))
        })
        .boxed();

        // Compose the initial result with the polling stream
        let composed = initial.into_stream().chain(poll_stream);

        Box::pin(composed)
    }
}
