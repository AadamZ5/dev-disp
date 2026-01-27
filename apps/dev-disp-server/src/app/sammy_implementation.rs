use std::time::Duration;

use dev_disp_core::{
    client::ScreenTransport,
    core::{get_default_config_path_for, handle_display_host},
    host::{ConnectableDevice, DeviceDiscovery, ScreenProvider},
};
use dev_disp_encoders::ffmpeg::{FfmpegEncoderProvider, config_file::FfmpegConfiguration};
use log::{error, info, trace};

const SAMSUNG_SERIAL: &str = "RFCT71HTZNL";

pub async fn sammy_implementation<P, D, C, T>(provider: P, discovery: D)
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
