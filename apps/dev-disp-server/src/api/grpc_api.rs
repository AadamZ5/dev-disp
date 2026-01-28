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
        request: Request<proto::Device>,
    ) -> std::result::Result<Response<proto::ConnectDeviceResponse>, Status> {
        let device = request.into_inner();
        self.inner
            .initialize_device(device.discovery_id, device.id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(proto::ConnectDeviceResponse {
            success: true,
            message: "".to_string(),
        }))
    }

    async fn disconnect_device(
        &self,
        request: Request<proto::Device>,
    ) -> std::result::Result<Response<proto::DisconnectDeviceResponse>, Status> {
        let device = request.into_inner();
        self.inner
            .disconnect_device(device.discovery_id, device.id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(proto::DisconnectDeviceResponse {
            success: true,
            message: "".to_string(),
        }))
    }
}
