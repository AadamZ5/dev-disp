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

/// A trait for something that can connect to a device and provide a host for screen
/// extension.
pub trait ConnectableDevice: Sized {
    /// The underlying type of transport this device uses.
    type Transport: ScreenTransport;

    /// Connect to the device and return a DisplayHost for it.
    ///
    /// The returned future resolves to a DisplayHost that can be used to manage
    /// the connected device.
    fn connect(
        self,
    ) -> PinnedFuture<
        'static,
        Result<DisplayHost<Self::Transport>, Box<dyn std::error::Error + Send + Sync>>,
    >;

    /// Get information about this connectable device.
    fn get_info(&self) -> ConnectableDeviceInfo;
}

/// A trait for something that can discover connectable devices.
pub trait DeviceDiscovery {
    type DeviceCandidate: ConnectableDevice;

    /// Discover available devices.
    ///
    /// The returned future resolves to a list of available device candidates.
    fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceCandidate>>;

    /// Get a display name for this discovery service
    fn get_display_name(&self) -> String;
}

/// Something that can discover devices and asynchronously automatically provide updates
/// about new devices as they become available.
pub trait StreamingDeviceDiscovery: DeviceDiscovery {
    /// Convert this discovery into a stream of device lists.
    ///
    /// Each item yielded by the stream is a list of currently available devices.
    /// The stream should yield a new list whenever the set of available devices changes.
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceCandidate>> + Send>>;
}

/// Helper struct that wraps a DeviceDiscovery and polls it at regular intervals
/// to provide a streaming device discovery.
pub struct PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery,
{
    inner: D,
    interval: Duration,
    sleep_factory: Box<SleepFactory>,
}

/// A factory function that creates a sleep future for the given duration.
/// I need to get me one of these
pub type SleepFactory = fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

impl<D> PollingDeviceDiscovery<D>
where
    D: DeviceDiscovery,
{
    pub fn new(inner: D, interval: Duration, sleep_factory: SleepFactory) -> Self {
        Self {
            inner,
            interval,
            sleep_factory: Box::new(sleep_factory),
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

    fn get_display_name(&self) -> String {
        self.inner.get_display_name()
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
