use std::{process::exit, time::Duration};

use dev_disp_comm::{usb::discovery::UsbDiscovery, websocket::discovery::WsDiscovery};
use dev_disp_core::{
    client::ScreenTransport,
    core::{get_default_config_path_for, handle_display_host},
    host::{ConnectableDevice, DeviceDiscovery, ScreenProvider, StreamingDeviceDiscovery},
};
use dev_disp_encoders::ffmpeg::{FfmpegEncoderProvider, config_file::FfmpegConfiguration};
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::{FutureExt, StreamExt, stream};
use log::{LevelFilter, error, info, trace, warn};
use tokio::{net::TcpListener, signal::ctrl_c, task::LocalSet};
use tokio_util::compat::TokioAsyncWriteCompatExt;

use crate::util::default_path_read_or_write_default_config_for;

mod util;

const SAMSUNG_SERIAL: &str = "RFCT71HTZNL";

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

    let ws_discovery = WsDiscovery::new();

    let local_set = LocalSet::new();
    let single_thread_work = local_set.run_until(async move {
        let logic_1 = tokio::task::spawn_local(sammy_implementation(evdi_provider_1, UsbDiscovery));

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
            trace!("Could not find Samsung device");
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

        let provider_1 = provider.clone();

        let _ = tokio::task::spawn_local(async move {
            let ffmpeg_config = match get_default_config_path_for::<FfmpegConfiguration>() {
                Ok(path) => {
                    match util::read_configuration_or_write_default_for::<FfmpegConfiguration>(
                        &path,
                    )
                    .await
                    {
                        Ok(config) => config,
                        Err(e) => {
                            error!(
                                "Failed to read or write FFmpeg configuration at {:?}: {:?}",
                                path, e
                            );
                            FfmpegConfiguration::default()
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get default FFmpeg configuration path: {}", e);
                    FfmpegConfiguration::default()
                }
            };

            let handle_result = handle_display_host(
                provider_1,
                FfmpegEncoderProvider::new(ffmpeg_config),
                display,
            )
            .await;

            if let Err((_, e)) = handle_result {
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

    // TODO: Make this configuration hot-reloadable with a file watcher!
    let ffmpeg_config = default_path_read_or_write_default_config_for::<FfmpegConfiguration>()
        .await
        .map_err(|e| {
            error!("Failed to read or write FFmpeg configuration: {}", e);
            e
        })
        .unwrap_or_default();

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

            let provider_1 = provider.clone();

            let ffmpeg_config = ffmpeg_config.clone();
            let _ = tokio::task::spawn_local(async move {
                let handle_result = handle_display_host(
                    provider_1,
                    FfmpegEncoderProvider::new(ffmpeg_config),
                    display,
                )
                .await;

                if let Err((_, e)) = handle_result {
                    error!("Error handling display host: {}", e);
                } else {
                    info!("Display host handling completed successfully");
                }
            });
        }
    }
}
