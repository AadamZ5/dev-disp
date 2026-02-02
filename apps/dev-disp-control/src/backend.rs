use crate::util::UnwrapOrLog;
use dev_disp_api::grpc::client::DevDispGrpcClient;
use dev_disp_core::daemon::api::DeviceRef;
use futures::{
    SinkExt, Stream, StreamExt,
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
    DeviceListUpdated(DeviceRef),
}

#[derive(Debug, Clone)]
pub enum Command {
    Connect(String),
    Disconnect,
    // TODO:
    // RefreshDeviceList,
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
}

impl BackendWorkerState {
    pub fn new(recv: Receiver<Command>) -> Self {
        Self {
            backend_api: None,
            recv,
        }
    }

    pub async fn process_command(&mut self, command: Command) -> Option<Event> {
        match command {
            Command::Connect(endpoint) => self
                .connect(endpoint.clone())
                .await
                .unwrap_or_log("Failed to connect to backend")
                .map(|_| Event::Connected(endpoint)),
            Command::Disconnect => self
                .disconnect()
                .await
                .unwrap_or_log("Failed to disconnect from backend")
                .map(|_| Event::Disconnected),
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
        if self.backend_api.is_some() {
            return Err(ConnectionError::AlreadyConnected);
        }

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
}

fn run_backend(recv: Receiver<Command>) -> impl Stream<Item = Event> {
    let state = BackendWorkerState::new(recv);

    // TODO: An unfolding stream will not let us parallelize tasks easily.
    // We will need a polled-future-sender approach later on. Probably just
    // use the `iced::task::Sipper` thing.
    stream::unfold(state, |mut state| async {
        match state.recv.next().await {
            Some(cmd) => Some((state.process_command(cmd).await, state)),
            // If None, we are all finnished
            None => None,
        }
    })
    .filter_map(|event| async move { event })
}
