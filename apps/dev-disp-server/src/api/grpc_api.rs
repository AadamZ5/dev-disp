use dev_disp_core::util::PinnedStream;
use futures_util::{FutureExt, StreamExt};
use proto::dev_disp_service_server::DevDispService;
use tonic::{Request, Response, Status};

use crate::api::DevDispApiFacade;

pub mod proto {
    tonic::include_proto!("dev_disp_server");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("dev_disp_service_descriptor");
}

pub struct GrpcDevDispApiFacade<T>
where
    T: DevDispApiFacade,
{
    inner: T,
}

impl<T> GrpcDevDispApiFacade<T>
where
    T: DevDispApiFacade + Send + Sync + 'static,
{
    pub fn new(facade: T) -> Self {
        Self { inner: facade }
    }
}

#[tonic::async_trait]
impl<T> DevDispService for GrpcDevDispApiFacade<T>
where
    T: DevDispApiFacade + Send + Sync + 'static,
{
    type ListenAvailableDevicesStream =
        PinnedStream<'static, Result<proto::AvailableDevicesResponse, Status>>;
    type ListenConnectedDevicesStream =
        PinnedStream<'static, Result<proto::ConnectedDevicesResponse, Status>>;

    async fn list_available_devices(
        &self,
        request: Request<proto::ListAvailableDevicesRequest>,
    ) -> std::result::Result<Response<proto::AvailableDevicesResponse>, Status> {
        let device_stats = self.inner.get_device_status().await;
        Ok(Response::new(proto::AvailableDevicesResponse {
            devices: device_stats
                .connectable_devices
                .into_iter()
                .map(|device_ref| proto::Device {
                    name: device_ref.name,
                    discovery_id: device_ref.interface_key,
                    id: device_ref.id,
                })
                .collect(),
        }))
    }

    async fn list_connected_devices(
        &self,
        request: Request<proto::ListConnectedDevicesRequest>,
    ) -> std::result::Result<Response<proto::ConnectedDevicesResponse>, Status> {
        let device_stats = self.inner.get_device_status().await;
        Ok(Response::new(proto::ConnectedDevicesResponse {
            devices: device_stats
                .in_use_devices
                .into_iter()
                .map(|device_ref| proto::Device {
                    name: device_ref.name,
                    discovery_id: device_ref.interface_key,
                    id: device_ref.id,
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

    async fn listen_connected_devices(
        &self,
        request: Request<proto::ListConnectedDevicesRequest>,
    ) -> std::result::Result<Response<Self::ListenConnectedDevicesStream>, Status> {
        let mapped_stream = self.inner.stream_device_status().map(|device_status| {
            let response = proto::ConnectedDevicesResponse {
                devices: device_status
                    .in_use_devices
                    .into_iter()
                    .map(|device_ref| proto::Device {
                        name: device_ref.name,
                        discovery_id: device_ref.interface_key,
                        id: device_ref.id,
                    })
                    .collect(),
            };
            Ok(response)
        });

        Ok(Response::new(
            mapped_stream.boxed() as Self::ListenConnectedDevicesStream
        ))
    }

    async fn listen_available_devices(
        &self,
        request: Request<proto::ListAvailableDevicesRequest>,
    ) -> std::result::Result<
        Response<PinnedStream<'static, Result<proto::AvailableDevicesResponse, Status>>>,
        Status,
    > {
        let mapped_stream = self.inner.stream_device_status().map(|device_status| {
            let response = proto::AvailableDevicesResponse {
                devices: device_status
                    .connectable_devices
                    .into_iter()
                    .map(|device_ref| proto::Device {
                        name: device_ref.name,
                        discovery_id: device_ref.interface_key,
                        id: device_ref.id,
                    })
                    .collect(),
            };
            Ok(response)
        });

        Ok(Response::new(
            mapped_stream.boxed() as Self::ListenAvailableDevicesStream
        ))
    }
}
