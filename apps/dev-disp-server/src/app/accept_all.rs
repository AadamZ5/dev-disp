use dev_disp_core::{
    client::ScreenTransport,
    core::handle_display_host,
    host::{ConnectableDevice, ScreenProvider, StreamingDeviceDiscovery},
};
use dev_disp_encoders::ffmpeg::{FfmpegEncoderProvider, config_file::FfmpegConfiguration};
use log::{error, info};

use crate::util::default_path_read_or_write_default_config_for;

pub async fn accept_all<P, D, C, T>(provider: P, discovery: D)
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
