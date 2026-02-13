use dev_disp_core::{
    daemon::api::{
        DevDispApi, DeviceCollectionStatus, DiscoveryId, DisplayHostId, DisplayHostRef,
        DisplayHostStatus, InitializationState,
    },
    util::{PinnedFuture, PinnedStream},
};
use futures::{Stream, stream};
use futures_util::FutureExt;
use futures_util::StreamExt;

use crate::grpc::proto::{
    self, ConnectDeviceRequest, DisconnectDeviceRequest, ListAvailableDevicesRequest,
    ListConnectedDevicesRequest, StreamDevicesRequest,
    dev_disp_service_client::DevDispServiceClient,
};

use async_broadcast::{Receiver, Sender};

#[derive(Clone, Debug)]
pub struct DevDispGrpcClient {
    inner: DevDispServiceClient<tonic::transport::Channel>,
    /// Sender to notify about client errors
    client_error_tx: Sender<()>,
    /// Unused receiver to keep the channel alive
    _client_error_rx: Receiver<()>,
}

impl DevDispGrpcClient {
    pub async fn connect<E>(endpoint: E) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        E: TryInto<tonic::transport::Endpoint>,
        E::Error: std::error::Error + Send + Sync + 'static,
    {
        let client = DevDispServiceClient::connect(endpoint).await?;
        let (client_error_tx, _client_error_rx) = async_broadcast::broadcast(16);
        Ok(DevDispGrpcClient {
            inner: client,
            client_error_tx,
            _client_error_rx,
        })
    }

    /// This returns a special broadcast receiver that we use to communicate with
    /// any listener that this client has encountered an error during transport.
    ///
    /// This is not a super elegant solution, but the gRPC tonic library has no
    /// exposed way to know when a client is disconnected or non functioning. So
    /// we will use a conservative method of assuming that if there is an error
    /// in any case, we are disconnected or non-functioning correctly.
    ///
    /// The purpose of this channel is not to deliver the error object either,
    /// rather just to have a separate notification channel that we can use to
    /// trigger disconnects.
    pub fn _get_error_notification_receiver(&self) -> PinnedStream<'static, ()> {
        self._client_error_rx.clone().boxed()
    }
}

impl DevDispApi for DevDispGrpcClient {
    fn get_devices(
        &self,
    ) -> PinnedFuture<
        'static,
        Result<DeviceCollectionStatus, Box<dyn std::error::Error + Send + Sync>>,
    > {
        let inner = self.inner.clone();
        let error_tx = self.client_error_tx.clone();

        async move {
            let mut inner = inner;
            let mut inner2 = inner.clone();

            let (available_response, connected_response) = futures::join!(
                inner.list_available_devices(ListAvailableDevicesRequest {}),
                inner2.list_connected_devices(ListConnectedDevicesRequest {})
            );

            match (available_response, connected_response) {
                (Ok(available_res), Ok(connected_res)) => {
                    let available_devices = available_res.into_inner().devices;
                    let connected_devices = connected_res.into_inner().devices;
                    // Combine or process the devices as needed
                    Ok(DeviceCollectionStatus {
                        connectable_devices: available_devices
                            .into_iter()
                            .map(|d| DisplayHostRef {
                                name: d.name,
                                discovery_id: d.discovery_id,
                                id: d.id,
                                status: d.status.unwrap_or_default().into(),
                            })
                            .collect(),
                        in_use_devices: connected_devices
                            .into_iter()
                            .map(|d| DisplayHostRef {
                                name: d.name,
                                discovery_id: d.discovery_id,
                                id: d.id,
                                status: d.status.unwrap_or_default().into(),
                            })
                            .collect(),
                    })
                }
                (Err(e), _) | (_, Err(e)) => {
                    // If either request fails, we consider the client to be in an error
                    // state. This will likely reset the connection.
                    let _ = error_tx.broadcast_direct(()).await;
                    Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        }
        .boxed()
    }

    fn stream_devices(&self) -> PinnedStream<'static, DeviceCollectionStatus> {
        let mut inner = self.inner.clone();
        let error_tx = self.client_error_tx.clone();

        async move {
            match inner.stream_devices(StreamDevicesRequest {}).await {
                Ok(response) => {
                    let error_tx_clone = error_tx.clone();
                    let stream = response.into_inner().filter_map(move |res| {
                        let error_tx_clone = error_tx_clone.clone();
                        async move {
                            let error_tx = error_tx_clone;
                            match res {
                                Ok(msg) => {
                                    let available_devices = msg.available_devices;
                                    let connected_devices = msg.connected_devices;

                                    DeviceCollectionStatus {
                                        connectable_devices: available_devices
                                            .into_iter()
                                            .map(|d| DisplayHostRef {
                                                name: d.name,
                                                discovery_id: d.discovery_id,
                                                id: d.id,
                                                status: d.status.unwrap_or_default().into(),
                                            })
                                            .collect(),
                                        in_use_devices: connected_devices
                                            .into_iter()
                                            .map(|d| DisplayHostRef {
                                                name: d.name,
                                                discovery_id: d.discovery_id,
                                                id: d.id,
                                                status: d.status.unwrap_or_default().into(),
                                            })
                                            .collect(),
                                    }
                                    .into()
                                }
                                Err(_) => {
                                    let _ = error_tx.broadcast_direct(()).await;
                                    None
                                }
                            }
                        }
                    });
                    stream.boxed()
                }
                Err(e) => {
                    error_tx.broadcast_direct(()).await.ok();
                    log::error!("Failed to start device status stream: {}", e);
                    stream::empty().boxed()
                }
            }
        }
        .flatten_stream()
        .boxed()
    }

    fn initialize_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>> {
        let mut inner = self.inner.clone();
        let error_tx = self.client_error_tx.clone();

        async move {
            let request = tonic::Request::new(ConnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match inner.connect_device(request).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    error_tx.broadcast_direct(()).await.ok();
                    Err(format!("gRPC error: {}", e))
                }
            }
        }
        .boxed()
    }

    fn disconnect_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>> {
        let mut inner = self.inner.clone();
        let error_tx = self.client_error_tx.clone();

        async move {
            let request = tonic::Request::new(DisconnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match inner.disconnect_device(request).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    error_tx.broadcast_direct(()).await.ok();
                    Err(format!("gRPC error: {}", e))
                }
            }
        }
        .boxed()
    }

    fn get_discovery_methods(
        &self,
    ) -> PinnedFuture<
        'static,
        Result<
            Vec<dev_disp_core::daemon::api::DiscoveryRef>,
            Box<dyn std::error::Error + Send + Sync>,
        >,
    > {
        let inner = self.inner.clone();
        async move {
            todo!();
        }
        .boxed()
    }
}

impl From<proto::DeviceStatus> for DisplayHostStatus {
    fn from(status: proto::DeviceStatus) -> Self {
        match proto::Status::from_i32(status.status) {
            Some(proto::Status::Available) => DisplayHostStatus::Available,
            Some(proto::Status::Initializing) => DisplayHostStatus::Initializing(
                proto::InitializationPhase::from_i32(
                    status.initialization_phase.unwrap_or_default() as i32,
                )
                .unwrap_or(proto::InitializationPhase::InitializationUnknown)
                .into(),
            ),
            Some(proto::Status::InUse) => DisplayHostStatus::InUse,
            Some(proto::Status::Disconnecting) => DisplayHostStatus::Disconnecting,
            Some(proto::Status::Error) => {
                DisplayHostStatus::Error(status.error_message.unwrap_or_default())
            }
            Some(proto::Status::Unknown) | None => DisplayHostStatus::Unknown,
        }
    }
}

impl From<proto::InitializationPhase> for InitializationState {
    fn from(phase: proto::InitializationPhase) -> Self {
        match phase {
            proto::InitializationPhase::InitializationUnknown => InitializationState::Unknown,
            proto::InitializationPhase::InitializingBegin => InitializationState::Initializing,
            proto::InitializationPhase::InitializingTransport => {
                InitializationState::InitializingTransport
            }
            proto::InitializationPhase::GettingDisplayParams => {
                InitializationState::GettingDisplayParameters
            }
            proto::InitializationPhase::NotifyClientLoading => {
                InitializationState::NotifyClientLoading
            }
            proto::InitializationPhase::GettingScreen => InitializationState::GettingScreen,
            proto::InitializationPhase::GettingEncoder => InitializationState::GettingEncoder,
            proto::InitializationPhase::NegotiatingCodecs => InitializationState::NegotiatingCodecs,
            proto::InitializationPhase::InitializingEncoder => {
                InitializationState::InitializingEncoder
            }
            proto::InitializationPhase::SettingClientCodec => {
                InitializationState::SettingClientCodec
            }
        }
    }
}
