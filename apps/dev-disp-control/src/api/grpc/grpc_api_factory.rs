use dev_disp_api::grpc::client::DevDispGrpcClient;
use futures::StreamExt;

use crate::backend::{ApiFactory, DisconnectableApi};

impl DisconnectableApi for DevDispGrpcClient {
    type ConnectParam = String;

    fn on_disconnect(
        &self,
    ) -> dev_disp_core::util::PinnedFuture<
        'static,
        Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
    > {
        let mut error_rx = self.get_error_notification_receiver();

        Box::pin(async move {
            match error_rx.next().await {
                Some(_) => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "An error occurred",
                )) as _), // An error occurred
                None => Ok(()), // Channel closed without errors
            }
        })
    }

    fn disconnect(
        &mut self,
    ) -> dev_disp_core::util::PinnedFuture<
        'static,
        Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
    > {
        Box::pin(async move {
            // For gRPC, we can just drop the client to disconnect.
            Ok(())
        })
    }
}

pub fn grpc_api_factory() -> impl ApiFactory<Api = DevDispGrpcClient, ConnectParam = String> {
    struct GrpcApiFactory;

    impl ApiFactory for GrpcApiFactory {
        type Api = DevDispGrpcClient;
        type ConnectParam = String;

        fn create_api(
            &self,
            _last_instance: Option<Self::Api>,
            param: Self::ConnectParam,
        ) -> dev_disp_core::util::PinnedFuture<
            'static,
            Result<Self::Api, Box<dyn std::error::Error + Send + Sync>>,
        > {
            Box::pin(async move {
                let client = DevDispGrpcClient::connect(param.clone()).await?;
                Ok(client)
            })
        }
    }

    GrpcApiFactory
}
