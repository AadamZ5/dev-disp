use crate::{
    backend::{ApiFactory, Command, DisconnectableApi, Event},
    util::{UnwrapOrLog, UnwrapOrLogMsg},
};
use dev_disp_core::daemon::api::{DevDispApi, DiscoveryId, DisplayHostId};
use futures::{SinkExt, StreamExt, channel::mpsc::Sender};

/// TODO: Refactor to allow a different backend client to be easily swapped!
#[derive(Debug)]
pub struct BackendState<T>
where
    T: ApiFactory,
{
    want_connected: bool,
    factory: T,
    backend_api: Option<T::Api>,
    event_tx: Sender<Event>,
}

impl<T> BackendState<T>
where
    T: ApiFactory,
{
    pub fn new(factory: T, event_tx: Sender<Event>) -> Self {
        Self {
            want_connected: false,
            factory,
            backend_api: None::<T::Api>,
            event_tx,
        }
    }

    pub async fn send_event(&mut self, event: Event) {
        if let Err(e) = self.event_tx.send(event).await {
            log::error!("Failed to send event to frontend: {}", e);
        }
    }

    pub async fn process_command(&mut self, command: Command) -> Option<Event> {
        match command {
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
impl<T> BackendState<T>
where
    T: ApiFactory,
{
    pub async fn connect(
        &mut self,
        endpoint: T::ConnectParam,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.backend_api.is_some() {
            log::warn!("Already connected to a backend");
            return Ok(());
        }

        let last_instance = self.backend_api.take();
        let backend = self.factory.create_api(last_instance, endpoint).await?;

        self.backend_api = Some(backend);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DisconnectionError> {
        if let Some(mut backend) = self.backend_api.as_mut() {
            backend
                .disconnect()
                .await
                .map_err(|e| DisconnectionError::ClientError(e))?;
        } else {
            log::warn!("Not connected to any backend");
            return Err(DisconnectionError::NotConnected);
        }

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

        let mut streaming_events = self.event_tx.clone();
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
