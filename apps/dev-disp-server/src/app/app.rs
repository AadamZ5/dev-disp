use crate::util::BroadcastSink;
use arc_swap::ArcSwap;
use dev_disp_core::{
    client::ScreenTransport,
    core::{SystemState, handle_display_host},
    daemon::api::{
        DevDispApi, DeviceCollectionStatus, DiscoveryId, DiscoveryRef, DisplayHostId,
        DisplayHostRef, DisplayHostStatus, InitializationState,
    },
    host::{
        ConnectableDevice, DeviceDiscovery, EncoderProvider, PollingDeviceDiscovery,
        ScreenProvider, StreamingDeviceDiscovery,
    },
    util::{PinnedFuture, PinnedLocalFuture, PinnedStream},
};
use futures_util::{FutureExt, StreamExt};
use log::{debug, error, info, warn};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::{
        RwLock, broadcast,
        mpsc::{self, error::SendError},
    },
    task::JoinSet,
};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream, errors::BroadcastStreamRecvError};

#[derive(Debug, Clone)]
pub struct ReadyDeviceRef {
    pub name: String,
    pub discovery_id: String,
    pub id: String,
    take_tx: mpsc::Sender<()>,
}

impl ReadyDeviceRef {
    pub fn new(name: String, discovery_id: String, id: String) -> (Self, mpsc::Receiver<()>) {
        let (take_tx, take_rx) = mpsc::channel(1);
        (
            Self {
                name,
                discovery_id,
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
    pub discovery_id: String,
    pub id: String,
    pub status: Arc<ArcSwap<SystemState>>,
    // TODO: current status atomic slot!
    disconnect_tx: mpsc::Sender<()>,
    status_tx: broadcast::Sender<SystemState>,
}

impl InUseDeviceRef {
    pub fn new(
        name: String,
        discovery_id: String,
        id: String,
        status: Arc<ArcSwap<SystemState>>,
    ) -> (Self, mpsc::Receiver<()>) {
        let (disconnect_tx, disconnect_rx) = mpsc::channel(1);
        let (status_tx, _) = broadcast::channel(16);
        (
            Self {
                name,
                discovery_id,
                id,
                disconnect_tx,
                status_tx,
                status,
            },
            disconnect_rx,
        )
    }

    pub async fn disconnect(&self) -> Result<(), SendError<()>> {
        let _ = self.status_tx.send(SystemState::Stopped);
        // TODO: Actually wait for the disconnection to complete!
        self.disconnect_tx.send(()).await
    }

    pub fn listen_status(&self) -> BroadcastStream<SystemState> {
        BroadcastStream::new(self.status_tx.subscribe())
    }

    pub fn get_current_status(&self) -> SystemState {
        // Note that this is driven by handler code in the app.
        **self.status.load()
    }
}

#[derive(Debug, Clone)]
struct DiscoveryMethod {
    pub id: DiscoveryId,
    pub name: String,
    pub description: Option<String>,
    // TODO: Use this!
    stop_discovery_tx: mpsc::Sender<()>,
}

impl DiscoveryMethod {
    pub fn new(
        id: DiscoveryId,
        name: String,
        description: Option<String>,
    ) -> (Self, mpsc::Receiver<()>) {
        let (stop_discovery_tx, stop_discovery_rx) = mpsc::channel(1);
        let this = Self {
            id,
            name,
            description,
            stop_discovery_tx,
        };

        (this, stop_discovery_rx)
    }
}

/// App keeps track of the current available devices, and in-use devices.
#[derive(Debug, Clone)]
pub struct App<S, E>
where
    S: ScreenProvider + Clone + Send + 'static,
    E: EncoderProvider + Clone + Send + 'static,
{
    screen_provider: S,
    encoder_provider: E,
    available_devices: Arc<RwLock<HashMap<DiscoveryId, HashMap<DisplayHostId, ReadyDeviceRef>>>>,
    in_use_devices: Arc<RwLock<HashMap<DiscoveryId, HashMap<DisplayHostId, InUseDeviceRef>>>>,
    discovery_methods: Arc<RwLock<HashMap<DiscoveryId, DiscoveryMethod>>>,
    devices_change_tx: broadcast::Sender<()>,
}

impl<S, E> App<S, E>
where
    S: ScreenProvider + Clone + Send + 'static,
    E: EncoderProvider + Clone + Send + 'static,
{
    pub fn new(screen_provider: S, encoder_provider: E) -> Self {
        let (devices_change_tx, _) = broadcast::channel(128);
        Self {
            screen_provider,
            encoder_provider,
            available_devices: Arc::new(RwLock::new(HashMap::new())),
            in_use_devices: Arc::new(RwLock::new(HashMap::new())),
            discovery_methods: Arc::new(RwLock::new(HashMap::new())),
            devices_change_tx,
        }
    }

    pub async fn shutdown(&self) {
        info!("Shutting down app");

        // TODO: Stop all discovery methods

        // Stop devices
        let available_devices = self.available_devices.read().await;
        let in_use_devices = self.in_use_devices.read().await;
        info!(
            "Final device status: {} available devices, {} in-use devices",
            available_devices
                .values()
                .map(|map| map.len())
                .sum::<usize>(),
            in_use_devices.values().map(|map| map.len()).sum::<usize>()
        );

        // Close all in-use devices
        let mut disconnect_tasks = JoinSet::new();
        for (_, devices_map) in in_use_devices.iter() {
            for (_, device_ref) in devices_map.iter() {
                let device_ref = device_ref.clone();
                disconnect_tasks.spawn(async move {
                    info!("Disconnecting device '{}'", device_ref.name);
                    match device_ref.disconnect().await {
                        Ok(_) => {
                            info!("Device '{}' disconnected successfully", device_ref.name);
                        }
                        Err(e) => {
                            error!("Error disconnecting device '{}': {}", device_ref.name, e);
                        }
                    }
                });
            }
        }

        // Make sure we don't keep read locks alive, disconnect tasks will want
        // to write to the in-use devices list.
        drop(available_devices);
        drop(in_use_devices);

        while let Some(res) = disconnect_tasks.join_next().await {
            match res {
                Ok(_) => {}
                Err(e) => {
                    error!("Error in device disconnection task: {}", e);
                }
            }
        }

        info!("App shutdown complete");
    }

    /// Given a device discovery instance, listen to the devices it discovers and hold
    /// them in the available devices list.
    ///
    /// ### Implementation note:
    ///
    /// A lot of logic is defined in this single function because we can't quite pass
    /// the generics around easily. We play nice with Rust's monomorphization rules, and
    /// leave the entire generic logic here inside this function.
    ///
    /// The alternative would be to box a lot of things, which wouldn't be super performant.
    /// However, performance doesn't quite matter at this stage. But... it does matter at
    /// the display handling stage, so we keep that part generic and monomorphized. We
    /// cannot go generic -> boxed -> generic again (as far as I know), so we keep the
    /// generics here.
    pub fn setup_discovery<D, C, T>(
        &self,
        discovery: D,
        discovery_id: DiscoveryId,
    ) -> PinnedLocalFuture<'static, ()>
    where
        D: StreamingDeviceDiscovery<DeviceCandidate = C>,
        C: ConnectableDevice<Transport = T> + Send + 'static,
        T: ScreenTransport + 'static,
    {
        let discovery_name = discovery.get_display_name();
        let discovery_description = discovery.get_description();
        let discovery = discovery.into_stream();
        let available_devices = self.available_devices.clone();
        let in_use_devices = self.in_use_devices.clone();
        let screen_provider = self.screen_provider.clone();
        let encoder_provider = self.encoder_provider.clone();
        let devices_change_tx = self.devices_change_tx.clone();
        let discovery_methods = self.discovery_methods.clone();

        // TODO: Handle the indentation party below (make functions to reduce indentation)

        // Discover devices, and enter them into the available devices list.
        async move {
            let discovery_id = discovery_id;
            let screen_provider = screen_provider;
            let encoder_provider = encoder_provider;
            let devices_change_tx = devices_change_tx;
            let discovery_methods = discovery_methods;

            // Submit the discovery info first
            let (discovery_method_ref, mut stop_discovery_rx) = DiscoveryMethod::new(
                discovery_id.clone(),
                discovery_name.clone(),
                discovery_description,
            );
            let mut discovery_methods_guard = discovery_methods.write().await;
            discovery_methods_guard.insert(discovery_id.clone(), discovery_method_ref);
            drop(discovery_methods_guard);

            let stop_discover_fut = stop_discovery_rx.recv().boxed();
            let mut discovery = discovery.take_until(stop_discover_fut);

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
                        info.id.clone(),
                    );

                    entry.insert(device_ref.id.clone(), device_ref);

                    let screen_provider_clone = screen_provider.clone();
                    let encoder_provider_clone = encoder_provider.clone();
                    let available_devices = available_devices.clone();
                    let in_use_devices = in_use_devices.clone();
                    let discovery_id = discovery_id.clone();
                    let devices_change_tx_clone = devices_change_tx.clone();

                    // Spawn a task to handle if/when this device is taken.
                    tokio::task::spawn_local(async move {
                        let info = info;
                        let device = device;
                        let screen_provider = screen_provider_clone;
                        let encoder_provider = encoder_provider_clone;
                        let available_devices = available_devices;
                        let in_use_devices = in_use_devices;
                        let discovery_id = discovery_id;
                        let device_change_tx = devices_change_tx_clone;
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

                        let status_slot = Arc::new(ArcSwap::from_pointee(SystemState::Unknown));
                        let (in_use_device_ref, cancel_rx) = InUseDeviceRef::new(
                            info.name.clone(),
                            discovery_id.clone(),
                            info.id.clone(),
                            status_slot.clone(),
                        );

                        let device_status_tx = in_use_device_ref.status_tx.clone();

                        in_use_devices
                            .write()
                            .await
                            .entry(discovery_id.clone())
                            .or_insert_with(HashMap::new)
                            .insert(info.id.clone(), in_use_device_ref);

                        match device_change_tx.send(()) {
                            Ok(a) => {
                                debug!("Notified {} device-list change listeners", a);
                            }
                            Err(_) => debug!("Failed to notify device change listeners"),
                        }

                        let device_status_tx_clone = device_status_tx.clone();
                        let device_change_tx_clone = device_change_tx.clone();
                        let device_name = info.name.clone();
                        let device_name_clone = device_name.clone();

                        // Fork off screen handling on a new thread with its own runtime.
                        // This is done because some transport or screen implementations
                        // may behave badly if run on the same runtime as the main server.
                        // In other words, do not let screen-handling code get interefered
                        // by other async tasks in the main server runtime.
                        std::thread::spawn(move || {
                            let local_set = tokio::task::LocalSet::new();
                            let rt = tokio::runtime::Runtime::new()
                                .expect("Failed to create Tokio runtime");
                            local_set.block_on(&rt, async move {
                                let device_status_tx = device_status_tx_clone;
                                let device_name = device_name_clone;
                                let device_change_tx = device_change_tx_clone;
                                match device.connect().await {
                                    Ok(display) => {
                                        info!("Device '{}' initiated successfully", device_name);
                                        let handle_result = handle_display_host(
                                            screen_provider,
                                            encoder_provider,
                                            display,
                                            ReceiverStream::new(cancel_rx),
                                            BroadcastSink::new(device_status_tx),
                                        )
                                        .await;

                                        if let Err(e) = handle_result {
                                            error!("Error handling display host: {}", e);
                                        } else {
                                            info!("Display host handling completed successfully");
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to initiate device '{}': {}",
                                            device_name, e
                                        );
                                    }
                                };

                                // After above is done, remove from in-use devices
                                in_use_devices
                                    .write()
                                    .await
                                    .entry(discovery_id.clone())
                                    .and_modify(|devices_map| {
                                        devices_map.remove(&info.id);
                                    });

                                debug!(
                                    "Device '{}' disconnected and removed from in-use list",
                                    device_name
                                );

                                match device_change_tx.send(()) {
                                    Ok(a) => {
                                        debug!("Notified {} device-list change listeners", a);
                                    }
                                    Err(_) => {
                                        debug!("Failed to notify device change listeners")
                                    }
                                }
                            })
                        });

                        // This task will remain to listen for device updates
                        let mut status_rx = BroadcastStream::new(device_status_tx.subscribe());
                        while let Some(status_res) = status_rx.next().await {
                            match status_res {
                                Ok(status) => {
                                    debug!("Device '{}' status update: {:?}", device_name, status);
                                    status_slot.store(Arc::new(status));
                                    match device_change_tx.send(()) {
                                        Ok(a) => {
                                            debug!("Notified {} device-list change listeners", a);
                                        }
                                        Err(_) => {
                                            debug!("Failed to notify device change listeners")
                                        }
                                    }
                                }
                                Err(e) => match e {
                                    BroadcastStreamRecvError::Lagged(n) => {
                                        warn!(
                                            "Device '{}' status update lagged by {} messages",
                                            device_name, n
                                        );
                                    }
                                },
                            }
                        }
                    });
                }

                info!(
                    "Discovered {} device(s) on interface '{}'",
                    entry.len(),
                    discovery_name
                );

                match devices_change_tx.send(()) {
                    Ok(a) => {
                        debug!("Notified {} device-list change listeners", a);
                    }
                    Err(_) => debug!("Failed to notify device change listeners"),
                }
            }
        }
        .boxed_local()
    }

    /// Convenience function to setup a non-streaming discovery.
    ///
    /// Given a device discovery instance, poll it at the given interval, and hold
    /// discovered devices in the available devices list.
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

    /// Get a snapshot of the available devices.
    pub async fn get_available_devices(
        &self,
    ) -> HashMap<DiscoveryId, HashMap<DisplayHostId, ReadyDeviceRef>> {
        let read_guard = self.available_devices.read().await;
        read_guard.clone()
    }

    /// Get a snapshot of the in-use devices.
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
    ) -> PinnedFuture<'static, Result<(), ()>> {
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
    ) -> PinnedFuture<'static, Result<(), ()>> {
        let in_use_devices = self.in_use_devices.clone();
        async move {
            let read_guard = in_use_devices.read().await;
            let device = read_guard
                .get(&from_discovery_id)
                .and_then(|devices_map| devices_map.get(&device_id))
                .cloned()
                .ok_or(())?;

            device.disconnect().await.map_err(|_| ())?;

            Ok(())
        }
        .boxed()
    }
}

impl<S, E> DevDispApi for App<S, E>
where
    S: ScreenProvider + Clone + Send + 'static,
    E: EncoderProvider + Clone + Send + 'static,
{
    fn get_devices(
        &self,
    ) -> PinnedFuture<
        'static,
        Result<DeviceCollectionStatus, Box<dyn std::error::Error + Send + Sync>>,
    > {
        let available_devices = self.available_devices.clone();
        let in_use_devices = self.in_use_devices.clone();

        async move {
            let (available_guard, in_use_guard) =
                tokio::join!(available_devices.read(), in_use_devices.read());

            let connectable_devices = available_guard
                .iter()
                .flat_map(|(_, devices_map)| devices_map.values().cloned())
                .map(|device_ref| DisplayHostRef {
                    name: device_ref.name,
                    discovery_id: device_ref.discovery_id,
                    id: device_ref.id,
                    status: DisplayHostStatus::Available,
                })
                .collect();

            let in_use_devices = in_use_guard
                .iter()
                .flat_map(|(_, devices_map)| devices_map.values().cloned())
                .map(|device_ref| {
                    let status = device_status_from_system_state(&device_ref.get_current_status());
                    DisplayHostRef {
                        name: device_ref.name,
                        discovery_id: device_ref.discovery_id,
                        id: device_ref.id,
                        status,
                    }
                })
                .collect();

            Ok(DeviceCollectionStatus {
                connectable_devices,
                in_use_devices,
            })
        }
        .boxed()
    }

    fn stream_devices(&self) -> PinnedStream<'static, DeviceCollectionStatus> {
        let rx = self.devices_change_tx.clone().subscribe();
        let update_notifications = BroadcastStream::new(rx);
        // Create a fake initial emission to trigger an initial update
        let update_notifications =
            futures_util::stream::once(async { Ok::<(), _>(()) }).chain(update_notifications);

        let available_devices = self.available_devices.clone();
        let in_use_devices = self.in_use_devices.clone();

        update_notifications
            .then(move |_| {
                let available_devices = available_devices.clone();
                let in_use_devices = in_use_devices.clone();
                async move {
                    let (available_guard, in_use_guard) =
                        tokio::join!(available_devices.read(), in_use_devices.read());

                    let connectable_devices = available_guard
                        .iter()
                        .flat_map(|(_, devices_map)| devices_map.values().cloned())
                        .map(|device_ref| DisplayHostRef {
                            name: device_ref.name,
                            discovery_id: device_ref.discovery_id,
                            id: device_ref.id,
                            status: DisplayHostStatus::Available,
                        })
                        .collect();

                    let in_use_devices = in_use_guard
                        .iter()
                        .flat_map(|(_, devices_map)| devices_map.values().cloned())
                        .map(|device_ref| {
                            let status =
                                device_status_from_system_state(&device_ref.get_current_status());
                            DisplayHostRef {
                                name: device_ref.name,
                                discovery_id: device_ref.discovery_id,
                                id: device_ref.id,
                                status,
                            }
                        })
                        .collect();

                    DeviceCollectionStatus {
                        connectable_devices,
                        in_use_devices,
                    }
                }
            })
            .boxed()
    }

    fn initialize_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>> {
        self.initialize_device(discovery_id, device_id)
            .map(|res| res.map_err(|_| "Failed to initialize device".to_string()))
            .boxed()
    }

    fn disconnect_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>> {
        self.disconnect_device(discovery_id, device_id)
            .map(|res| res.map_err(|_| "Failed to disconnect device".to_string()))
            .boxed()
    }

    fn get_discovery_methods(
        &self,
    ) -> PinnedFuture<'static, Result<Vec<DiscoveryRef>, Box<dyn std::error::Error + Send + Sync>>>
    {
        let discovery_methods = self.discovery_methods.clone();
        async move {
            let read_guard = discovery_methods.read().await;
            let methods = read_guard
                .values()
                .map(|method| DiscoveryRef {
                    id: method.id.clone(),
                    name: method.name.clone(),
                    description: method.description.clone(),
                })
                .collect();

            Ok(methods)
        }
        .boxed()
    }
}

fn system_state_to_init_state(state: &SystemState) -> Option<InitializationState> {
    match state {
        SystemState::Unknown => Some(InitializationState::Unknown),
        SystemState::Initializing => Some(InitializationState::Initializing),
        SystemState::InitializingTransport => Some(InitializationState::InitializingTransport),
        SystemState::GettingDisplayParameters => {
            Some(InitializationState::GettingDisplayParameters)
        }
        SystemState::NotifyClientLoading => Some(InitializationState::NotifyClientLoading),
        SystemState::GettingScreen => Some(InitializationState::GettingScreen),
        SystemState::GettingEncoder => Some(InitializationState::GettingEncoder),
        SystemState::NegotiatingCodecs => Some(InitializationState::NegotiatingCodecs),
        SystemState::InitializingEncoder => Some(InitializationState::InitializingEncoder),
        SystemState::SettingClientCodec => Some(InitializationState::SettingClientCodec),
        SystemState::Running | SystemState::Stopped => None,
    }
}

fn device_status_from_system_state(state: &SystemState) -> DisplayHostStatus {
    match system_state_to_init_state(state) {
        Some(init_state) => DisplayHostStatus::Initializing(init_state),
        None => match state {
            SystemState::Running => DisplayHostStatus::InUse,
            SystemState::Stopped => DisplayHostStatus::Disconnecting,
            _ => DisplayHostStatus::Unknown,
        },
    }
}
