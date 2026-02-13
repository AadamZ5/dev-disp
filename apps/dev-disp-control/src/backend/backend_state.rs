use crate::{
    backend::{ApiFactory, BackendEvent, DisconnectableApi},
    util::{MyStreamExt, UnwrapOrLog},
};
use dev_disp_core::{
    daemon::api::{DevDispApi, DeviceCollectionStatus, DiscoveryId, DisplayHostId},
    util::{PinnedFuture, PinnedStream},
};
use futures::{
    FutureExt, StreamExt,
    future::{self},
    stream::{self, AbortHandle, SelectAll, abortable},
};
use log::info;

pub trait Api: DevDispApi + DisconnectableApi {}

/// Backend actions that affect the internal backend task's state.
#[derive(Debug, Clone)]
pub enum BackendAction<T, A> {
    /// Connect with the given backend
    Connect(T),
    /// The backend has connected to the endpoint.
    Connected(A, String),
    /// Ask the backend to disconnect from the current API.
    Disconnect,
    /// The backend has disconnected from the endpoint.
    /// This can happen either because the frontend requested
    /// a disconnect, or because the connection failed somehow.
    Disconnected,
    /// The backend has requested a retry to connect after the elapsed duration.
    RetryConnectElapsed,

    /// Ask the backend to start streaming device updates.
    StreamDevices,
    /// The device list has been updated with the contained data.
    DeviceListUpdated(DeviceCollectionStatus),
    /// Ask the backend to stop streaming device updates.
    StreamDevicesStop,
    /// The device stream stopped for some reason.
    DeviceStreamEnded,
    /// Attempt to initialize the specified device.
    InitializeDevice(DisplayHostId, DiscoveryId),
    /// Attempt to stop using the specified device.
    DisconnectDevice(DisplayHostId, DiscoveryId),
    /// A no-op event that can be used when futures need to throw away side-effects
    NoOp,
    /// Emits when the backend loop exits. This signals *no more* reconnectivity,
    /// and the backend is essentially dead.
    BackendLoopExit,
}

pub enum EventType<T, A> {
    InternalState(BackendAction<T, A>),
    ToFrontend(BackendEvent),
}

impl<T, A> From<BackendAction<T, A>> for EventType<T, A> {
    fn from(action: BackendAction<T, A>) -> Self {
        EventType::InternalState(action)
    }
}

impl<T, A> From<BackendEvent> for EventType<T, A> {
    fn from(event: BackendEvent) -> Self {
        EventType::ToFrontend(event)
    }
}

#[derive(Debug)]
pub struct BackendState<T>
where
    T: ApiFactory,
{
    want_connected: bool,
    factory: T,
    streaming_handle: Option<AbortHandle>,
    backend_api: Option<T::Api>,
}

impl<T> BackendState<T>
where
    T: ApiFactory,
{
    pub fn new(factory: T) -> Self {
        Self {
            want_connected: false,
            factory,
            streaming_handle: None,
            backend_api: None::<T::Api>,
        }
    }

    pub fn update(
        &mut self,
        action: BackendAction<T::ConnectParam, T::Api>,
    ) -> PinnedStream<'static, EventType<T::ConnectParam, T::Api>> {
        match action {
            BackendAction::Connect(connect_param) => {
                let connect_param_clone = connect_param.clone();
                self.factory
                    .create_api(self.backend_api.take(), connect_param)
                    .map(move |res| {
                        match res {
                            Ok(api) => {
                                BackendAction::Connected(api, connect_param_clone.to_string())
                            }
                            Err(e) => {
                                log::error!("Failed to connect to backend API: {}", e);
                                BackendAction::Disconnected
                            }
                        }
                        .into()
                    })
                    .into_stream()
                    .boxed()
            }
            BackendAction::Disconnect => self
                .disconnect()
                .map(|res| {
                    res.unwrap_or_log("Failed to disconnect from backend");
                    BackendAction::Disconnected.into()
                })
                .into_stream()
                .boxed(),
            BackendAction::StreamDevices => {
                if let Some(handle) = self.streaming_handle.take() {
                    handle.abort();
                }
                match self.stream_devices() {
                    Ok(device_stream) => {
                        let (stream, handle) = abortable(device_stream);
                        let stream = stream.map(|collection| {
                            EventType::InternalState(BackendAction::DeviceListUpdated(collection))
                        });
                        self.streaming_handle = Some(handle);
                        stream.boxed()
                    }
                    Err(_) => {
                        stream::once(future::ready(BackendAction::DeviceStreamEnded.into())).boxed()
                    }
                }
            }
            BackendAction::StreamDevicesStop => {
                if let Some(handle) = self.streaming_handle.take() {
                    handle.abort();
                }
                stream::once(future::ready(BackendAction::DeviceStreamEnded.into())).boxed()
            }
            BackendAction::InitializeDevice(host_id, discovery_id) => self
                .initialize_device(host_id, discovery_id)
                .into_stream()
                .no_output()
                .boxed(),
            BackendAction::DisconnectDevice(host_id, discovery_id) => self
                .disconnect_device(host_id, discovery_id)
                .into_stream()
                .no_output()
                .boxed(),
            BackendAction::Connected(api, display_endpoint) => {
                info!(
                    "Successfully connected to backend API at endpoint: {}",
                    display_endpoint
                );
                self.backend_api = Some(api);
                let ready_stream = stream::once(future::ready(
                    BackendEvent::Connected(display_endpoint).into(),
                ))
                .boxed();
                let watch_disconnect_stream = self
                    .listen_backend_disconnect()
                    .into_stream()
                    .map(|_| BackendEvent::Disconnected.into())
                    .boxed();
                // Simultaneously dispatch the connected event to the frontend, and start watching for disconnects.
                SelectAll::from_iter([ready_stream, watch_disconnect_stream]).boxed()
            }
            BackendAction::Disconnected => {
                self.backend_api.take();
                stream::once(future::ready(BackendEvent::Disconnected.into())).boxed()
            }
            BackendAction::RetryConnectElapsed => todo!(),
            BackendAction::DeviceListUpdated(collection) => stream::once(future::ready(
                BackendEvent::DeviceListUpdated(collection).into(),
            ))
            .boxed(),
            BackendAction::DeviceStreamEnded => todo!(),
            BackendAction::NoOp => stream::empty().boxed(),
            BackendAction::BackendLoopExit => {
                log::error!("Backend loop has exited. No further events will be emitted.");
                stream::empty().boxed()
            }
        }
    }

    pub fn listen_backend_disconnect(&mut self) -> impl Future<Output = ()> + Send + 'static {
        let disconnect_fut = self.backend_api.as_mut().map(|api| api.on_disconnect());

        async move {
            if let Some(disconnect_fut) = disconnect_fut {
                match disconnect_fut.await {
                    Ok(_) => {
                        log::info!("Backend disconnect detected");
                    }
                    Err(e) => {
                        log::error!("Backend disconnect detected: {}", e);
                    }
                }
            } else {
                log::warn!("listen_backend_disconnect called but no backend API is connected");
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
    fn disconnect(&mut self) -> PinnedFuture<'static, Result<(), DisconnectionError>> {
        let backend_disconncet_fut = self.backend_api.as_mut().map(|api| api.disconnect());

        async move {
            if let Some(backend_disconncet_fut) = backend_disconncet_fut {
                backend_disconncet_fut
                    .await
                    .map_err(|e| DisconnectionError::ClientError(e))?;
            } else {
                log::warn!("Not connected to any backend");
                return Err(DisconnectionError::NotConnected);
            }

            Ok(())
        }
        .boxed()
    }

    fn stream_devices(&self) -> Result<PinnedStream<'static, DeviceCollectionStatus>, ()> {
        log::info!("Starting device status streaming from backend");
        let backend_api = match &self.backend_api {
            Some(api) => api,
            None => {
                log::error!("Attempted to stream devices without a connected backend");
                return Err(());
            }
        };

        let device_stream = backend_api.stream_devices();
        Ok(device_stream)
    }

    fn initialize_device(
        &mut self,
        dev_id: DisplayHostId,
        discovery_id: DiscoveryId,
    ) -> PinnedFuture<'static, Result<(), ()>> {
        log::info!(
            "Requesting device connection to device {:?} via discovery ID {:?}",
            dev_id,
            discovery_id
        );
        let backend_api = match &self.backend_api {
            Some(api) => api,
            None => {
                log::error!("Attempted to connect to device without a connected backend");
                return futures::future::ready(Err(())).boxed();
            }
        };

        backend_api
            .initialize_device(discovery_id, dev_id)
            .map(|res| {
                res.unwrap_or_log("Failed to initialize device");
                Ok(())
            })
            .boxed()
    }

    fn disconnect_device(
        &mut self,
        dev_id: DisplayHostId,
        discovery_id: DiscoveryId,
    ) -> PinnedFuture<'static, Result<(), ()>> {
        log::info!(
            "Requesting device disconnection from device {:?} via discovery ID {:?}",
            dev_id,
            discovery_id
        );
        let backend_api = match &self.backend_api {
            Some(api) => api,
            None => {
                log::error!("Attempted to disconnect from device without a connected backend");
                return futures::future::ready(Err(())).boxed();
            }
        };

        backend_api
            .disconnect_device(discovery_id, dev_id)
            .map(|res| {
                res.unwrap_or_log("Failed to disconnect device");
                Ok(())
            })
            .boxed()
    }
}
