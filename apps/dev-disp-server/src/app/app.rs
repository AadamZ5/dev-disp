use dev_disp_core::{
    client::ScreenTransport,
    core::handle_display_host,
    host::{
        ConnectableDevice, DeviceDiscovery, EncoderProvider, PollingDeviceDiscovery,
        ScreenProvider, StreamingDeviceDiscovery,
    },
    util::{PinnedFuture, PinnedLocalFuture},
};
use futures_util::{FutureExt, StreamExt};
use log::{debug, error, info};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{
    RwLock,
    mpsc::{self, error::SendError},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::api::{DevDispApiFacade, DiscoveryId, DisplayHostId};

#[derive(Debug, Clone)]
pub struct ReadyDeviceRef {
    pub name: String,
    pub interface_key: String,
    pub interface_display: String,
    pub id: String,
    take_tx: mpsc::Sender<()>,
}

impl ReadyDeviceRef {
    pub fn new(
        name: String,
        interface_key: String,
        interface_display: String,
        id: String,
    ) -> (Self, mpsc::Receiver<()>) {
        let (take_tx, take_rx) = mpsc::channel(1);
        (
            Self {
                name,
                interface_key,
                interface_display,
                id,
                take_tx,
            },
            take_rx,
        )
    }

    pub async fn take(&self) -> Result<(), SendError<()>> {
        self.take_tx.send(()).await
    }
}

#[derive(Debug, Clone)]
pub struct InUseDeviceRef {
    pub name: String,
    pub interface_key: String,
    pub interface_display: String,
    pub id: String,
    canel_tx: mpsc::Sender<()>,
}

impl InUseDeviceRef {
    pub fn new(
        name: String,
        interface_key: String,
        interface_display: String,
        id: String,
    ) -> (Self, mpsc::Receiver<()>) {
        let (cancel_tx, cancel_rx) = mpsc::channel(1);
        (
            Self {
                name,
                interface_key,
                interface_display,
                id,
                canel_tx: cancel_tx,
            },
            cancel_rx,
        )
    }

    pub async fn cancel(&self) -> Result<(), SendError<()>> {
        self.canel_tx.send(()).await
    }
}

/// App keeps track of the current available devices, and in-use devices.
#[derive(Debug, Clone)]
pub struct App<S, E>
where
    S: ScreenProvider + Clone + 'static,
    E: EncoderProvider + Clone + 'static,
{
    screen_provider: S,
    encoder_provider: E,
    available_devices: Arc<RwLock<HashMap<DiscoveryId, HashMap<DisplayHostId, ReadyDeviceRef>>>>,
    in_use_devices: Arc<RwLock<HashMap<DiscoveryId, HashMap<DisplayHostId, InUseDeviceRef>>>>,
}

impl<S, E> App<S, E>
where
    S: ScreenProvider + Clone,
    E: EncoderProvider + Clone,
{
    pub fn new(screen_provider: S, encoder_provider: E) -> Self {
        Self {
            screen_provider,
            encoder_provider,
            available_devices: Arc::new(RwLock::new(HashMap::new())),
            in_use_devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Given a device discovery instance, listen to the devices it discovers and hold
    /// them in the available devices list.
    pub fn setup_discovery<D, C, T>(
        &self,
        discovery: D,
        discovery_id: DiscoveryId,
    ) -> PinnedLocalFuture<'static, ()>
    where
        D: StreamingDeviceDiscovery<DeviceCandidate = C>,
        C: ConnectableDevice<Transport = T> + 'static,
        T: ScreenTransport + 'static,
    {
        let discovery_display = discovery.get_display_name();
        let mut discovery = discovery.into_stream();
        let available_devices = self.available_devices.clone();
        let in_use_devices = self.in_use_devices.clone();
        let screen_provider = self.screen_provider.clone();
        let encoder_provider = self.encoder_provider.clone();

        // Discover devices, and enter them into the available devices list.
        async move {
            let discovery_id = discovery_id;
            let screen_provider = screen_provider;
            let encoder_provider = encoder_provider;
            while let Some(devices) = discovery.next().await {
                let mut write_guard = available_devices.write().await;

                let entry = write_guard
                    .entry(discovery_id.clone())
                    .or_insert_with(HashMap::new);

                entry.clear();

                for device in devices {
                    let info = device.get_info();
                    let (device_ref, mut take_rx) = ReadyDeviceRef::new(
                        info.name.clone(),
                        discovery_id.clone(),
                        discovery_display.clone(),
                        info.id.clone(),
                    );

                    entry.insert(device_ref.id.clone(), device_ref);

                    let screen_provider_clone = screen_provider.clone();
                    let encoder_provider_clone = encoder_provider.clone();
                    let available_devices = available_devices.clone();
                    let in_use_devices = in_use_devices.clone();
                    let discovery_id = discovery_id.clone();
                    let discovery_display = discovery_display.clone();

                    // Spawn a task to handle if/when this device is taken.
                    tokio::task::spawn_local(async move {
                        let info = info;
                        let device = device;
                        let screen_provider = screen_provider_clone;
                        let encoder_provider = encoder_provider_clone;
                        let available_devices = available_devices.clone();
                        let in_use_devices = in_use_devices.clone();
                        let discovery_id = discovery_id.clone();
                        let discovery_display = discovery_display.clone();
                        if take_rx.recv().await.is_none() {
                            // Device was not taken before other half dropped
                            return;
                        }
                        info!("Initiating device '{}'", info.name);

                        available_devices
                            .write()
                            .await
                            .entry(discovery_id.clone())
                            .and_modify(|devices_map| {
                                devices_map.remove(&info.id);
                            });

                        let (in_use_device_ref, cancel_rx) = InUseDeviceRef::new(
                            info.name.clone(),
                            discovery_id.clone(),
                            discovery_display.clone(),
                            info.id.clone(),
                        );

                        in_use_devices
                            .write()
                            .await
                            .entry(discovery_id.clone())
                            .or_insert_with(HashMap::new)
                            .insert(info.id.clone(), in_use_device_ref);

                        match device.connect().await {
                            Ok(display) => {
                                info!("Device '{}' initiated successfully", info.name);
                                // TODO: We should send this to another thread instead.
                                let handle_result = handle_display_host(
                                    screen_provider,
                                    encoder_provider,
                                    display,
                                    ReceiverStream::new(cancel_rx),
                                )
                                .await;

                                if let Err((_, e)) = handle_result {
                                    error!("Error handling display host: {}", e);
                                } else {
                                    info!("Display host handling completed successfully");
                                }
                            }
                            Err(e) => {
                                error!("Failed to initiate device '{}': {}", info.name, e)
                            }
                        };
                    });
                }

                info!(
                    "Discovered {} device(s) on interface '{}'",
                    entry.len(),
                    discovery_display
                );
            }
        }
        .boxed_local()
    }

    pub fn setup_discovery_polling<D, C, T>(
        &self,
        discovery: D,
        discovery_id: DiscoveryId,
        poll_interval: std::time::Duration,
    ) -> PinnedLocalFuture<'static, ()>
    where
        D: DeviceDiscovery<DeviceCandidate = C> + Send + 'static,
        C: ConnectableDevice<Transport = T> + 'static + Send,
        T: ScreenTransport + 'static,
    {
        let streaming_discovery = PollingDeviceDiscovery::new(discovery, poll_interval, |d| {
            tokio::time::sleep(d).boxed()
        });

        self.setup_discovery(streaming_discovery, discovery_id)
    }

    pub async fn get_available_devices(
        &self,
    ) -> HashMap<DiscoveryId, HashMap<DisplayHostId, ReadyDeviceRef>> {
        let read_guard = self.available_devices.read().await;
        read_guard.clone()
    }

    pub async fn get_in_use_devices(
        &self,
    ) -> HashMap<DiscoveryId, HashMap<DisplayHostId, InUseDeviceRef>> {
        let read_guard = self.in_use_devices.read().await;
        read_guard.clone()
    }

    /// Attempt to connect to an available device, using its discovery ID and device ID.
    pub fn initialize_device(
        &self,
        from_discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'_, Result<(), ()>> {
        // TODO: Better error types!
        let available_devices = self.available_devices.clone();
        async move {
            let read_guard = available_devices.read().await;
            let device = read_guard
                .get(&from_discovery_id)
                .and_then(|devices_map| devices_map.get(&device_id))
                .cloned()
                .ok_or(())?;

            device.take().await.map_err(|_| ())?;

            Ok(())
        }
        .boxed()
    }

    pub fn disconnect_device(
        &self,
        from_discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'_, Result<(), ()>> {
        let in_use_devices = self.in_use_devices.clone();
        async move {
            let read_guard = in_use_devices.read().await;
            let device = read_guard
                .get(&from_discovery_id)
                .and_then(|devices_map| devices_map.get(&device_id))
                .cloned()
                .ok_or(())?;

            device.cancel().await.map_err(|_| ())?;

            Ok(())
        }
        .boxed()
    }
}

impl<S, E> DevDispApiFacade for App<S, E>
where
    S: ScreenProvider + Clone,
    E: EncoderProvider + Clone,
{
    fn get_device_status(&self) -> PinnedFuture<'_, crate::api::DeviceCollectionStatus> {
        let available_devices = self.available_devices.clone();
        let in_use_devices = self.in_use_devices.clone();

        async move {
            let (available_guard, in_use_guard) =
                tokio::join!(available_devices.read(), in_use_devices.read());

            let connectable_devices = available_guard
                .iter()
                .flat_map(|(_, devices_map)| devices_map.values().cloned())
                .map(|device_ref| crate::api::DeviceRef {
                    name: device_ref.name,
                    interface_key: device_ref.interface_key,
                    interface_display: device_ref.interface_display,
                    id: device_ref.id,
                })
                .collect();

            let in_use_devices = in_use_guard
                .iter()
                .flat_map(|(_, devices_map)| devices_map.values().cloned())
                .map(|device_ref| crate::api::DeviceRef {
                    name: device_ref.name,
                    interface_key: device_ref.interface_key,
                    interface_display: device_ref.interface_display,
                    id: device_ref.id,
                })
                .collect();

            crate::api::DeviceCollectionStatus {
                connectable_devices,
                in_use_devices,
            }
        }
        .boxed()
    }

    fn stream_device_status(
        &self,
    ) -> dev_disp_core::util::PinnedLocalStream<'_, crate::api::DeviceCollectionStatus> {
        todo!()
    }

    fn initialize_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'_, Result<(), String>> {
        self.initialize_device(discovery_id, device_id)
            .map(|res| res.map_err(|_| "Failed to initialize device".to_string()))
            .boxed()
    }

    fn disconnect_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'_, Result<(), String>> {
        self.disconnect_device(discovery_id, device_id)
            .map(|res| res.map_err(|_| "Failed to disconnect device".to_string()))
            .boxed()
    }
}
