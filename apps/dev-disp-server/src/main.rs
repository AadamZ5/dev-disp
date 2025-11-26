use std::{process::exit, time::Duration};

use dev_disp_comm::{usb::discovery::UsbDiscovery, websocket::discovery::WsDiscovery};
use dev_disp_core::{
    client::ScreenTransport,
    core::handle_display_host,
    host::{ConnectableDevice, DeviceDiscovery, ScreenProvider, StreamingDeviceDiscovery},
};
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::{
    FutureExt, StreamExt,
    stream::{self, empty},
};
use log::{LevelFilter, error, info, warn};
use tokio::{net::TcpListener, signal::ctrl_c, task::LocalSet};
use tokio_util::compat::TokioAsyncWriteCompatExt;

const SAMSUNG_SERIAL: &str = "RFCT71HTZNL";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let evdi_provider = EvdiScreenProvider::new();
    let evdi_provider_1 = evdi_provider.clone();

    let ws_discovery = WsDiscovery::new();

    let local_set = LocalSet::new();
    let single_thread_work = local_set.run_until(async move {
        let logic_1 =
            tokio::task::spawn_local(sammy_implementation(evdi_provider_1, UsbDiscovery {}));

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

async fn sammy_implementation<P, D, C, T>(provider: P, discovery: D)
where
    P: ScreenProvider,
    D: DeviceDiscovery<DeviceCandidate = C>,
    C: ConnectableDevice<Transport = T>,
    T: ScreenTransport + 'static,
{
    let mut first_iter = true;

    return;

    loop {
        if first_iter {
            first_iter = false;
        } else {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        // TODO: Implement some UI for picking this stuff
        let sammy_accessory_result = discovery
            .discover_devices()
            .await
            .into_iter()
            .find(|dev| dev.get_info().id == SAMSUNG_SERIAL);

        if sammy_accessory_result.is_none() {
            error!("Could not find Samsung device");
            continue;
        }

        let sammy_accessory =
            sammy_accessory_result.expect("Sammy accessory was None after checking!");

        let connect_result = sammy_accessory.connect().await;

        if let Err(e) = connect_result {
            error!("Failed to connect to device: {}", e);
            continue;
        }

        let display =
            connect_result.expect("Sammy accessory was an error after checking that it wasnt!");

        let display = display.to_some_transport();

        let provider_1 = provider.clone();

        let _ = tokio::task::spawn_local(async move {
            let handle_result = handle_display_host(provider_1, display).await;

            if let Err(e) = handle_result {
                error!("Error handling display host: {}", e);
            } else {
                info!("Display host handling completed successfully");
            }
        })
        .await;
    }
}

async fn accept_all<P, D, C, T>(provider: P, discovery: D)
where
    P: ScreenProvider,
    D: StreamingDeviceDiscovery<DeviceCandidate = C>,
    C: ConnectableDevice<Transport = T>,
    T: ScreenTransport + 'static,
{
    let mut discovery = discovery.into_stream();
    while let Some(devices) = discovery.next().await {
        info!("Discovered {} device(s)", devices.len());
        for device in devices {
            let connect_result = device.connect().await;

            if let Err(e) = connect_result {
                error!("Failed to connect to device: {}", e);
                continue;
            }

            let display =
                connect_result.expect("Device was an error after checking that it wasnt!");

            let display = display.to_some_transport();

            let provider_1 = provider.clone();

            let _ = tokio::task::spawn_local(async move {
                let handle_result = handle_display_host(provider_1, display).await;

                if let Err(e) = handle_result {
                    error!("Error handling display host: {}", e);
                } else {
                    info!("Display host handling completed successfully");
                }
            });
        }
    }
}
