use super::proto::{self, dev_disp_service_server::DevDispService};
use dev_disp_core::{
    daemon::api::{DevDispApi, DisplayHostStatus},
    util::PinnedStream,
};
use futures_util::StreamExt;
use tonic::{Request, Response, Status};

pub struct GrpcDevDispApiAdapter<T>
where
    T: DevDispApi,
{
    inner: T,
}

impl<T> GrpcDevDispApiAdapter<T>
where
    T: DevDispApi + Send + Sync + 'static,
{
    pub fn new(facade: T) -> Self {
        Self { inner: facade }
    }
}

#[tonic::async_trait]
impl<T> DevDispService for GrpcDevDispApiAdapter<T>
where
    T: DevDispApi + Send + Sync + 'static,
{
    type StreamDevicesStream = PinnedStream<'static, Result<proto::StreamDevicesResponse, Status>>;

    async fn list_available_devices(
        &self,
        _request: Request<proto::ListAvailableDevicesRequest>,
    ) -> std::result::Result<Response<proto::AvailableDevicesResponse>, Status> {
        let device_stats = self
            .inner
            .get_devices()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::AvailableDevicesResponse {
            devices: device_stats
                .connectable_devices
                .into_iter()
                .map(|device_ref| proto::Device {
                    name: device_ref.name,
                    discovery_id: device_ref.discovery_id,
                    id: device_ref.id,
                    status: Some(device_ref.status.into()),
                })
                .collect(),
        }))
    }

    async fn list_connected_devices(
        &self,
        _request: Request<proto::ListConnectedDevicesRequest>,
    ) -> std::result::Result<Response<proto::ConnectedDevicesResponse>, Status> {
        let device_stats = self
            .inner
            .get_devices()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::ConnectedDevicesResponse {
            devices: device_stats
                .in_use_devices
                .into_iter()
                .map(|device_ref| proto::Device {
                    name: device_ref.name,
                    discovery_id: device_ref.discovery_id,
                    id: device_ref.id,
                    status: Some(device_ref.status.into()),
                })
                .collect(),
        }))
    }

    async fn connect_device(
        &self,
        request: Request<proto::ConnectDeviceRequest>,
    ) -> std::result::Result<Response<proto::ConnectDeviceResponse>, Status> {
        let req = request.into_inner();
        self.inner
            .initialize_device(req.discovery_id, req.device_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(proto::ConnectDeviceResponse {
            success: true,
            message: "".to_string(),
        }))
    }

    async fn disconnect_device(
        &self,
        request: Request<proto::DisconnectDeviceRequest>,
    ) -> std::result::Result<Response<proto::DisconnectDeviceResponse>, Status> {
        let req = request.into_inner();
        self.inner
            .disconnect_device(req.discovery_id, req.device_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(proto::DisconnectDeviceResponse {
            success: true,
            message: "".to_string(),
        }))
    }

    async fn stream_devices(
        &self,
        _request: Request<proto::StreamDevicesRequest>,
    ) -> std::result::Result<Response<Self::StreamDevicesStream>, Status> {
        let stream = self.inner.stream_devices().map(|device_stats| {
            Ok(proto::StreamDevicesResponse {
                available_devices: device_stats
                    .connectable_devices
                    .into_iter()
                    .map(|device_ref| proto::Device {
                        name: device_ref.name,
                        discovery_id: device_ref.discovery_id,
                        id: device_ref.id,
                        status: Some(device_ref.status.into()),
                    })
                    .collect(),
                connected_devices: device_stats
                    .in_use_devices
                    .into_iter()
                    .map(|device_ref| proto::Device {
                        name: device_ref.name,
                        discovery_id: device_ref.discovery_id,
                        id: device_ref.id,
                        status: Some(device_ref.status.into()),
                    })
                    .collect(),
            })
        });

        Ok(Response::new(stream.boxed()))
    }

    async fn list_discovery_methods(
        &self,
        _request: Request<proto::ListDiscoveryMethodsRequest>,
    ) -> std::result::Result<Response<proto::ListDiscoveryMethodsResponse>, Status> {
        let methods = self
            .inner
            .get_discovery_methods()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::ListDiscoveryMethodsResponse {
            discovery_methods: methods
                .into_iter()
                .map(|method_ref| proto::DiscoveryMethod {
                    id: method_ref.id,
                    name: method_ref.name,
                    description: method_ref.description,
                })
                .collect(),
        }))
    }
}

impl From<DisplayHostStatus> for proto::DeviceStatus {
    fn from(status: DisplayHostStatus) -> Self {
        match status {
            DisplayHostStatus::Available
            | DisplayHostStatus::Connecting
            | DisplayHostStatus::InUse
            | DisplayHostStatus::Unreachable
            | DisplayHostStatus::Disconnecting
            | DisplayHostStatus::Unknown => proto::DeviceStatus {
                status: proto::Status::from(status) as i32,
                error_message: None,
            },
            DisplayHostStatus::Error(e) => proto::DeviceStatus {
                status: proto::Status::Error as i32,
                error_message: Some(e),
            },
        }
    }
}

impl From<DisplayHostStatus> for proto::Status {
    fn from(value: DisplayHostStatus) -> Self {
        match value {
            DisplayHostStatus::Available => proto::Status::Available,
            DisplayHostStatus::Connecting => proto::Status::Connecting,
            DisplayHostStatus::InUse => proto::Status::InUse,
            DisplayHostStatus::Unreachable => proto::Status::Unreachable,
            DisplayHostStatus::Disconnecting => proto::Status::Disconnecting,
            DisplayHostStatus::Error(_) => proto::Status::Error,
            DisplayHostStatus::Unknown => proto::Status::Unknown,
        }
    }
}
