use std::process::exit;

use dev_disp_encoders::ffmpeg::{FfmpegEncoderProvider, config_file::FfmpegConfiguration};
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::FutureExt;
use log::{LevelFilter, error, info, warn};
use tokio::{signal::ctrl_c, task::LocalSet};

use crate::{app::App, config::default_path_read_or_write_default_config_for};

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

    let evdi_provider = EvdiScreenProvider::new();

    // TODO: Make this configuration hot-reloadable with a file watcher!
    let ffmpeg_config = default_path_read_or_write_default_config_for::<FfmpegConfiguration>()
        .await
        .map_err(|e| {
            error!("Failed to read or write FFmpeg configuration: {}", e);
            e
        })
        .unwrap_or_default();
    let ffmpeg_provider = FfmpegEncoderProvider::new(ffmpeg_config);

    let app = App::new(evdi_provider.clone(), ffmpeg_provider);

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
            Ok(())
        });

        let app_ws_discovery =
            tokio::task::spawn_local(app.setup_discovery(ws_discovery, "websocket".to_string()));

        let ctrl_c_listener = tokio::task::spawn_local(async move {
            ctrl_c().await.expect("Failed to listen for Ctrl-C");
            warn!("Received Ctrl-C, shutting down");
            evdi_provider.stop();

            ctrl_c().await.expect("Failed to listen for second Ctrl-C");
            error!("Received second Ctrl-C, forcing exit");
            exit(1);
        });

        let res = futures_util::select_biased! {
            listen_result = listen.fuse() => listen_result,
            app_ws_discovery_result = app_ws_discovery.fuse() => app_ws_discovery_result,
            ctrl_c_result = ctrl_c_listener.fuse() => ctrl_c_result,
        };

        if let Err(e) = res {
            error!("Error in main logic: {}", e);
        }

        info!("Main logic finished");
    });

    single_thread_work.await;
    info!("Exiting");
}
