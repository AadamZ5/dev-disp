use dev_disp_core::{
    daemon::api::{
        DevDispApi, DeviceCollectionStatus, DiscoveryId, DisplayHostId, DisplayHostRef,
        DisplayHostStatus,
    },
    util::{PinnedFuture, PinnedStream},
};
use futures::stream;
use futures_util::FutureExt;
use futures_util::StreamExt;

use crate::grpc::proto::{
    self, ConnectDeviceRequest, DisconnectDeviceRequest, ListAvailableDevicesRequest,
    ListConnectedDevicesRequest, StreamDevicesRequest,
    dev_disp_service_client::DevDispServiceClient,
};

#[derive(Clone, Debug)]
pub struct DevDispGrpcClient {
    inner: DevDispServiceClient<tonic::transport::Channel>,
}

impl DevDispGrpcClient {
    pub async fn connect<E>(endpoint: E) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        E: TryInto<tonic::transport::Endpoint>,
        E::Error: std::error::Error + Send + Sync + 'static,
    {
        let client = DevDispServiceClient::connect(endpoint).await?;
        Ok(Self { inner: client })
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
                    Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        }
        .boxed()
    }

    fn stream_devices(&self) -> PinnedStream<'static, DeviceCollectionStatus> {
        let mut inner = self.inner.clone();

        async move {
            match inner.stream_devices(StreamDevicesRequest {}).await {
                Ok(response) => {
                    let stream = response.into_inner().filter_map(|res| async move {
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
                            Err(_) => None,
                        }
                    });
                    stream.boxed()
                }
                Err(e) => {
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

        async move {
            let request = tonic::Request::new(ConnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match inner.connect_device(request).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("gRPC error: {}", e)),
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

        async move {
            let request = tonic::Request::new(DisconnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match inner.disconnect_device(request).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("gRPC error: {}", e)),
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
            Some(proto::Status::Connecting) => DisplayHostStatus::Connecting,
            Some(proto::Status::InUse) => DisplayHostStatus::InUse,
            Some(proto::Status::Unreachable) => DisplayHostStatus::Unreachable,
            Some(proto::Status::Disconnecting) => DisplayHostStatus::Disconnecting,
            Some(proto::Status::Error) => {
                DisplayHostStatus::Error(status.error_message.unwrap_or_default())
            }
            Some(proto::Status::Unknown) | None => DisplayHostStatus::Unknown,
        }
    }
}
