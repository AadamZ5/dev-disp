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
    pub description: Option<String>,
}

pub trait ConnectableDevice: Sized {
    type Transport: ScreenTransport;

    fn connect(
        self,
    ) -> PinnedFuture<
        'static,
        Result<DisplayHost<Self::Transport>, Box<dyn std::error::Error + Send + Sync>>,
    >;

    fn get_info(&self) -> ConnectableDeviceInfo;
}

pub trait DeviceDiscovery {
    type DeviceCandidate: ConnectableDevice;

    fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceCandidate>>;
}

pub trait StreamingDeviceDiscovery: DeviceDiscovery {
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceCandidate>> + Send>>;
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
    type DeviceCandidate = D::DeviceCandidate;

    fn discover_devices(&'_ self) -> PinnedFuture<'_, Vec<Self::DeviceCandidate>> {
        self.inner.discover_devices()
    }
}

impl<D> StreamingDeviceDiscovery for PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery + Send + 'static,
    <D as DeviceDiscovery>::DeviceCandidate: Send + 'static,
{
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceCandidate>> + Send>> {
        let discovery_stream = async move {
            let initial_discovery = self.inner.discover_devices().await;

            let poll_stream = unfold(self, |this| async move {
                (this.sleep_factory)(this.interval).await;
                let devices = this.inner.discover_devices().await;
                Some((devices, this))
            });

            futures_util::stream::once(async move { initial_discovery }).chain(poll_stream)
        }
        .flatten_stream();

        Box::pin(discovery_stream)
    }
}

// TODO: Below is an attempt to generalize the device discovery, but there are issues
// with traites with GATs and dyn-compatibility. There may be a way to fix this. Needs
// further investigation.

// pub struct GenericDeviceDiscovery {
//     inner: Box<dyn StreamingDeviceDiscovery<DeviceFacade = GenericDeviceFacade>>,
// }

// pub struct GenericDeviceFacade {
//     inner: Box<dyn ConnectableDevice<Transport = SomeScreenTransport>>,
// }

// impl ConnectableDevice for GenericDeviceFacade {
//     type Transport = SomeScreenTransport;

//     fn connect(
//         self,
//     ) -> PinnedFuture<
//         'static,
//         Result<DisplayHost<Self::Transport>, Box<dyn std::error::Error + Send + Sync>>,
//     > {
//         (self.inner).connect()
//     }

//     fn get_info(&self) -> ConnectableDeviceInfo {
//         self.inner.get_info()
//     }
// }

// impl DeviceDiscovery for GenericDeviceDiscovery {
//     type DeviceFacade = GenericDeviceFacade;

//     fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceFacade>> {}
// }
