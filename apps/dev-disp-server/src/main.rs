use std::process::exit;

use dev_disp_comm::websocket::discovery::WsDiscovery;
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::{FutureExt, StreamExt, stream};
use log::{LevelFilter, error, info, warn};
use tokio::{net::TcpListener, signal::ctrl_c, task::LocalSet};
use tokio_util::compat::TokioAsyncWriteCompatExt;

use crate::app::accept_all::accept_all;

mod api;
mod app;
mod util;

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
    let evdi_provider_1 = evdi_provider.clone();

    let local_set = LocalSet::new();
    let single_thread_work = local_set.run_until(async move {
        let ws_discovery = WsDiscovery::new();
        let listener = TcpListener::bind("0.0.0.0:56789")
            .await
            .expect("Failed to bind to TCP port 56789");
        info!("Listening for WebSocket connections on port 56789");

        let incoming_client_stream = stream::unfold(listener, |listener| async {
            let (stream, addr) = match listener.accept().await {
                Ok(pair) => pair,
                Err(e) => {
                    error!("Failed to accept incoming TCP connection: {}", e);
                    return None;
                }
            };

            info!("Accepted connection from {}", addr);

            Some((stream, listener))
        })
        .map(|stream| stream.compat_write())
        .boxed();

        let ws_listen = ws_discovery.listen(incoming_client_stream);
        let listen = tokio::task::spawn_local(ws_listen).map(|res| {
            if let Err(e) = res {
                error!("Error setting up websocket listen task: {}", e);
            } else if let Ok(Err(e)) = res {
                error!("Error accepting WebSocket connections: {}", e);
            }
            Ok(())
        });

        let logic_2 = tokio::task::spawn_local(accept_all(evdi_provider.clone(), ws_discovery));

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
            logic_2_result = logic_2.fuse() => logic_2_result,
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
