use std::sync::Arc;

use dev_disp_core::{
    daemon::api::{DevDispApi, DeviceCollectionStatus, DeviceRef, DiscoveryId, DisplayHostId},
    util::{PinnedFuture, PinnedStream},
};
use futures_locks::RwLock;
use futures_util::FutureExt;

use crate::grpc::proto::{
    ConnectDeviceRequest, DisconnectDeviceRequest, ListAvailableDevicesRequest,
    ListConnectedDevicesRequest, dev_disp_service_client::DevDispServiceClient,
};

pub struct DevDispGrpcClient {
    inner: Arc<RwLock<DevDispServiceClient<tonic::transport::Channel>>>,
}

impl DevDispApi for DevDispGrpcClient {
    fn get_device_status(
        &self,
    ) -> PinnedFuture<
        'static,
        Result<DeviceCollectionStatus, Box<dyn std::error::Error + Send + Sync>>,
    > {
        let inner = self.inner.clone();

        async move {
            let mut guard = inner.write().await;

            // Can't paralellize here :/
            let available_response = guard
                .list_available_devices(ListAvailableDevicesRequest {})
                .await;
            let connected_response = guard
                .list_connected_devices(ListConnectedDevicesRequest {})
                .await;

            match (available_response, connected_response) {
                (Ok(available_res), Ok(connected_res)) => {
                    let available_devices = available_res.into_inner().devices;
                    let connected_devices = connected_res.into_inner().devices;
                    // Combine or process the devices as needed
                    Ok(DeviceCollectionStatus {
                        connectable_devices: available_devices
                            .into_iter()
                            .map(|d| DeviceRef {
                                name: d.name,
                                interface_key: d.discovery_id,
                                interface_display: d.discovery_display,
                                id: d.id,
                            })
                            .collect(),
                        in_use_devices: connected_devices
                            .into_iter()
                            .map(|d| DeviceRef {
                                name: d.name,
                                interface_key: d.discovery_id,
                                interface_display: d.discovery_display,
                                id: d.id,
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

    fn stream_device_status(&self) -> PinnedStream<'static, DeviceCollectionStatus> {
        // TODO: Understand how to stream from gRPC
        todo!()
    }

    fn initialize_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>> {
        let inner = self.inner.clone();

        async move {
            let mut guard = inner.write().await;

            let request = tonic::Request::new(ConnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match guard.connect_device(request).await {
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
        let inner = self.inner.clone();

        async move {
            let mut guard = inner.write().await;

            let request = tonic::Request::new(DisconnectDeviceRequest {
                discovery_id,
                device_id,
            });

            match guard.disconnect_device(request).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("gRPC error: {}", e)),
            }
        }
        .boxed()
    }
}
