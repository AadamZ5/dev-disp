use dev_disp_api::grpc::client::DevDispGrpcClient;
use dev_disp_core::util::PinnedFuture;
use futures::{FutureExt, StreamExt};

use crate::backend::{ApiFactory, DisconnectableApi, callback_api_factory};

impl DisconnectableApi for DevDispGrpcClient {
    type ConnectParam = String;

    fn on_disconnect(
        &self,
    ) -> PinnedFuture<'static, Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>> {
        // This current implementation will just declare a disconnect when there is
        // an error on the error notification channel.
        let mut error_rx = self._get_error_notification_receiver();

        async move {
            match error_rx.next().await {
                Some(_) => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "An error occurred",
                )) as _), // An error occurred
                None => Ok(()), // Channel closed without errors
            }
        }
        .boxed()
    }

    fn disconnect(
        &mut self,
    ) -> PinnedFuture<'static, Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>> {
        futures::future::ready(Ok(())).boxed()
    }
}

pub fn grpc_api_factory() -> impl ApiFactory<Api = DevDispGrpcClient, ConnectParam = String> {
    callback_api_factory(
        |last_instance: Option<DevDispGrpcClient>, param: String| async move {
            // If we have a last instance, we can try to reuse it by disconnecting first.
            if let Some(mut client) = last_instance {
                log::info!("Reusing existing gRPC client instance");
                if let Err(e) = client.disconnect().await {
                    log::warn!("Failed to disconnect existing gRPC client instance: {}", e);
                }
            }

            log::info!("Creating new gRPC client instance with endpoint: {}", param);
            DevDispGrpcClient::connect(param).await
        },
    )
}
