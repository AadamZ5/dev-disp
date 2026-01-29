use std::process::exit;

use dev_disp_comm::usb::discovery::UsbDiscovery;
use dev_disp_core::host::{EncoderProvider, ScreenProvider};
use dev_disp_encoders::ffmpeg::{FfmpegEncoderProvider, config_file::FfmpegConfiguration};
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::FutureExt;
use log::{LevelFilter, error, info, warn};
use tokio::{signal::ctrl_c, task::LocalSet};
use tonic::transport::Server;

use crate::{
    api::{
        DevDispApiFacade,
        grpc_api::{
            self, GrpcDevDispApiFacade, proto::dev_disp_service_server::DevDispServiceServer,
        },
    },
    app::App,
    config::default_path_read_or_write_default_config_for,
};

mod api;
mod app;
mod config;
mod websocket;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("evdi", LevelFilter::Warn)
        .filter_module("tracing", LevelFilter::Warn)
        .filter_module(
            "nusb::platform::linux_usbfs::enumeration",
            LevelFilter::Warn,
        )
        .init();

    let screen_provider = get_screen_provider().await;
    let encoder_provider = get_encoder_provider().await;
    let app = App::new(screen_provider.clone(), encoder_provider);
    spawn_grpc_api(app.clone());

    let local_set = LocalSet::new();
    // TODO: Is this single-threaded work necessary?
    let single_thread_work = local_set.run_until(async move {
        let (ws_discovery, ws_listen) = websocket::create_websocket_and_bg_task().await;
        let listen = tokio::task::spawn_local(ws_listen).map(|res| {
            if let Err(e) = res {
                error!("Error setting up websocket listen task: {}", e);
            } else if let Ok(Err(e)) = res {
                error!("Error accepting WebSocket connections: {}", e);
            }
        });

        let app_ws_discovery =
            tokio::task::spawn_local(app.setup_discovery(ws_discovery, "websocket".to_string()));

        let running_tasks = async move {
            _ = tokio::join!(listen, app_ws_discovery);
        }
        .shared();

        let app_clone = app.clone();
        let running_tasks_clone = running_tasks.clone();
        // TODO: Figure out a better way to handle sig-int/shutdown
        let ctrl_c_listener = tokio::task::spawn(async move {
            let app = app_clone;
            let running_tasks = running_tasks_clone;

            ctrl_c().await.expect("Failed to listen for Ctrl-C");
            warn!("Received Ctrl-C, shutting down");

            // App shutdown represents the completion of shutdown behavior, and then
            // the completion of running_tasks we care about
            let app_shutdown = async move {
                app.shutdown().await;
            }
            .then(|_| running_tasks);

            let next_ctrl_c = async move {
                ctrl_c().await.expect("Failed to listen for second Ctrl-C");
                error!("Received second Ctrl-C, forcing exit");
                exit(1);
            };

            tokio::select! {
                _ = app_shutdown => {
                    info!("Shutdown complete");
                }
                _ = next_ctrl_c => {
                    // This will never be reached because of the exit(1) above.
                }
            };
        });

        futures_util::select! {
            _ = running_tasks.fuse() => {
                info!("All running tasks completed");
            }
            _ =
            ctrl_c_listener.fuse() => {
                info!("Ctrl-C listener completed");
            }
        };

        info!("Main logic finished");
    });

    single_thread_work.await;
    info!("Exiting");
}

async fn get_screen_provider() -> impl ScreenProvider + Clone + 'static {
    let evdi_provider = EvdiScreenProvider::new();
    evdi_provider
}

async fn get_encoder_provider() -> impl EncoderProvider + Clone + 'static {
    // TODO: Make this configuration hot-reloadable with a file watcher!
    let ffmpeg_config = default_path_read_or_write_default_config_for::<FfmpegConfiguration>()
        .await
        .map_err(|e| {
            error!("Failed to read or write FFmpeg configuration: {}", e);
            e
        })
        .unwrap_or_default();
    let ffmpeg_provider = FfmpegEncoderProvider::new(ffmpeg_config);
    ffmpeg_provider
}

fn spawn_grpc_api<T>(api_facade: T)
where
    T: DevDispApiFacade + Send + Sync + 'static,
{
    let grpc_api = GrpcDevDispApiFacade::new(api_facade);

    tokio::spawn(async move {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(grpc_api::proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();

        Server::builder()
            .add_service(DevDispServiceServer::new(grpc_api))
            .add_service(reflection)
            .serve("[::1]:50051".parse().unwrap())
            .await
            .unwrap();
    });
}
