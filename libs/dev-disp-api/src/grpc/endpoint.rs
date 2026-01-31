use dev_disp_core::{
    daemon::{api::DevDispApi, endpoint::DevDispApiEndpoint},
    util::PinnedFuture,
};
use futures_util::FutureExt;
use log::{error, info};
use tonic::transport::Server;

use crate::grpc::{
    self, proto::dev_disp_service_server::DevDispServiceServer, server::GrpcDevDispApiAdapter,
};

pub struct DevDispGrpcEndpoint;

impl DevDispApiEndpoint for DevDispGrpcEndpoint {
    fn serve_api<D>(&mut self, api: D) -> PinnedFuture<'static, ()>
    where
        D: DevDispApi + Send + Sync + 'static,
    {
        async move {
            let grpc_api = GrpcDevDispApiAdapter::new(api);

            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(grpc::proto::FILE_DESCRIPTOR_SET)
                .build_v1()
                .unwrap();

            // TODO: Make address configurable!
            let addr = "[::1]:50051".parse().unwrap();

            info!("Starting to serve grpc API on {}", addr);
            match Server::builder()
                .add_service(DevDispServiceServer::new(grpc_api))
                .add_service(reflection)
                .serve(addr)
                .await
            {
                Ok(_) => info!("Finished serving grpc API"),
                Err(e) => error!("gRPC server error: {}", e),
            }
        }
        .boxed()
    }
}
