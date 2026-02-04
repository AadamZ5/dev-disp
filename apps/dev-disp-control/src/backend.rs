use crate::util::{UnwrapOrLog, UnwrapOrLogMsg};
use dev_disp_api::grpc::client::DevDispGrpcClient;
use dev_disp_core::daemon::api::{DevDispApi, DeviceCollectionStatus, DiscoveryId, DisplayHostId};
use futures::{
    FutureExt, SinkExt, Stream, StreamExt,
    channel::mpsc::{self, Receiver, Sender},
    stream,
};

#[derive(Debug, Clone)]
pub struct BackendRef(Sender<Command>);

impl BackendRef {
    fn new(sender: Sender<Command>) -> Self {
        Self(sender)
    }

    pub fn send(&mut self, command: Command) {
        let mut sender = self.0.clone();
        iced::futures::executor::block_on(async move {
            if let Err(e) = sender.send(command).await {
                log::error!("Failed to send command to backend: {}", e);
            }
        });
    }

    pub fn connect(&mut self, endpoint: String) {
        self.send(Command::Connect(endpoint));
    }

    pub fn disconnect(&mut self) {
        self.send(Command::Disconnect);
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(String),
    Disconnected,
    DeviceListUpdated(DeviceCollectionStatus),
}

#[derive(Debug, Clone)]
pub enum Command {
    Connect(String),
    Disconnect,
    StreamDevices,
    ConnectDevice(DisplayHostId, DiscoveryId),
    DisconnectDevice(DisplayHostId, DiscoveryId),
}

pub fn prepare_backend() -> (BackendRef, impl Stream<Item = Event>) {
    let (cmd_sender, cmd_receiver) = mpsc::channel(100);
    let backend = BackendRef::new(cmd_sender);
    let backend_future = run_backend(cmd_receiver);
    (backend, backend_future)
}

/// TODO: Refactor to allow a different backend client to be easily swapped!
#[derive(Debug)]
pub struct BackendWorkerState {
    backend_api: Option<DevDispGrpcClient>,
    recv: Receiver<Command>,
    streaming_events: Receiver<Event>,
    _streaming_events_tx: Sender<Event>,
}

impl BackendWorkerState {
    pub fn new(recv: Receiver<Command>) -> Self {
        let (event_send, event_recv) = mpsc::channel(100);

        Self {
            backend_api: None,
            recv,
            streaming_events: event_recv,
            _streaming_events_tx: event_send,
        }
    }

    pub async fn process_command(&mut self, command: Command) -> Option<Event> {
        match command {
            Command::Connect(endpoint) => self
                .connect(endpoint.clone())
                .await
                .map(|_| Event::Connected(endpoint.clone()))
                .unwrap_or_else(|e| {
                    log::error!("Failed to connect to backend at {}: {}", endpoint, e);
                    Event::Disconnected
                })
                .into(),
            Command::Disconnect => self
                .disconnect()
                .await
                .unwrap_or_log("Failed to disconnect from backend")
                .map(|_| Event::Disconnected)
                .unwrap_or(Event::Disconnected)
                .into(),
            Command::StreamDevices => {
                self.stream_devices()
                    .await
                    .unwrap_or_log_msg("Failed to start device streaming");
                None
            }
            Command::ConnectDevice(dev_id, discovery_id) => {
                self.connect_device(dev_id, discovery_id)
                    .await
                    .unwrap_or_log_msg("Failed to connect to device");
                None
            }
            Command::DisconnectDevice(dev_id, discovery_id) => {
                self.disconnect_device(dev_id, discovery_id)
                    .await
                    .unwrap_or_log_msg("Failed to disconnect from device");
                None
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ConnectionError {
    #[error("Client error: {0}")]
    ClientError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Already connected to a backend")]
    AlreadyConnected,
}

#[derive(Debug, thiserror::Error)]
enum DisconnectionError {
    #[error("Client error: {0}")]
    ClientError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Not connected to any backend")]
    NotConnected,
}

// Internal impl
impl BackendWorkerState {
    async fn connect(&mut self, endpoint: String) -> Result<(), ConnectionError> {
        let client = DevDispGrpcClient::connect(endpoint)
            .await
            .map_err(ConnectionError::ClientError)?;
        self.backend_api = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DisconnectionError> {
        if self.backend_api.is_none() {
            return Err(DisconnectionError::NotConnected);
        }

        self.backend_api = None;
        Ok(())
    }

    async fn stream_devices(&mut self) -> Result<(), ()> {
        log::info!("Starting device status streaming from backend");
        let backend_api = match &self.backend_api {
            Some(api) => api.clone(),
            None => {
                log::error!("Attempted to stream devices without a connected backend");
                return Err(());
            }
        };

        let mut streaming_events = self._streaming_events_tx.clone();
        let mut device_stream = backend_api.stream_devices();

        // TODO: We are cheating! Figure out how execute this properly within the
        // confines of the iced task system.
        tokio::spawn(async move {
            while let Some(status) = device_stream.next().await {
                log::debug!("Received device status update: {:?}", status);
                if let Err(e) = streaming_events
                    .send(Event::DeviceListUpdated(status))
                    .await
                {
                    log::error!("Failed to send device status update: {}", e);
                    break;
                }
            }
        });

        Ok(())
    }

    async fn connect_device(
        &mut self,
        dev_id: DisplayHostId,
        discovery_id: DiscoveryId,
    ) -> Result<(), ()> {
        log::info!(
            "Requesting device connection to device {:?} via discovery ID {:?}",
            dev_id,
            discovery_id
        );
        let backend_api = match &self.backend_api {
            Some(api) => api.clone(),
            None => {
                log::error!("Attempted to connect to device without a connected backend");
                return Err(());
            }
        };

        backend_api
            .initialize_device(discovery_id, dev_id)
            .await
            .map_err(|e| {
                log::error!("Failed to initialize device: {}", e);
            })
    }

    async fn disconnect_device(
        &mut self,
        dev_id: DisplayHostId,
        discovery_id: DiscoveryId,
    ) -> Result<(), ()> {
        log::info!(
            "Requesting device disconnection from device {:?} via discovery ID {:?}",
            dev_id,
            discovery_id
        );
        let backend_api = match &self.backend_api {
            Some(api) => api.clone(),
            None => {
                log::error!("Attempted to disconnect from device without a connected backend");
                return Err(());
            }
        };

        backend_api
            .disconnect_device(discovery_id, dev_id)
            .await
            .map_err(|e| {
                log::error!("Failed to disconnect device: {}", e);
            })
    }
}

fn run_backend(recv: Receiver<Command>) -> impl Stream<Item = Event> {
    let state = BackendWorkerState::new(recv);

    // TODO: An unfolding stream will not let us parallelize tasks easily.
    // We will need a polled-future-sender approach later on. Probably just
    // use the `iced::task::Sipper` thing.
    stream::unfold(state, |mut state| async {
        let result = futures::select! {
            command = state.recv.next().fuse() => match command {
                Some(cmd) => {
                    Some((state.process_command(cmd).await, state))
                },
                // If None, we are all finnished
                None => None,
            },
            streaming_event = state.streaming_events.next().fuse() => match streaming_event {
                Some(event) => {
                    Some((Some(event), state))
                },
                None => None,
            },
        };

        result
    })
    .filter_map(|event| async move { event })
}
