use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use futures::{channel::oneshot, join};
use futures_util::FutureExt;
use log::{debug, error, info, warn};

use crate::{
    client::{DisplayHost, ScreenTransport},
    host::{
        DisplayHostResult, Encoder, EncoderProvider, RawEncoder, Screen, ScreenProvider,
        ScreenReadyStatus,
    },
};

const NOT_READY_DELAY: Duration = Duration::from_millis(100);

pub async fn handle_display_host<T, P, E>(
    screen_provider: P,
    encoder_provider: E,
    mut host: DisplayHost<T>,
) -> DisplayHostResult<T>
where
    T: ScreenTransport + 'static,
    E: EncoderProvider + 'static,
    P: ScreenProvider + 'static,
{
    let stopped = Arc::new(AtomicBool::new(false));

    debug!("Getting background task for {host}...");
    let background_stopped = stopped.clone();
    let host_name = host.to_string();
    let background_task = host
        .get_background_task()
        .map(|r| r.map_err(|e| e.to_string()))
        .then(move |r| {
            debug!(
                "Background task for {host_name} finished with result: {:?}",
                r
            );
            //background_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
            futures::future::ready(r)
        })
        .boxed_local();

    let screen_task_stopped = stopped.clone();
    let screen_task = async move {
        // Handle the display-host connection here
        info!("Handling display-host: {host}");

        async fn close_dev(host: &mut DisplayHost<impl ScreenTransport>) {
            if let Err(_) = host.close().await {
                error!("Error closing display host");
            }
        }

        debug!("Initializing with transport...");
        // Initialize the transport
        if let Err(e) = host.initialize().await {
            error!("Failed to initialize transport: {}", e);
            close_dev(&mut host).await;
            return Err((host, "Failed to initialize transport".to_string()));
        }
        debug!("Initialized transport");

        debug!("Getting display parameters...");
        let display_params = match host.get_display_config().await {
            Err(e) => {
                error!("Failed to get display parameters: {}", e);
                close_dev(&mut host).await;
                return Err((host, "Failed to get display parameters".to_string()));
            }
            Ok(display_params) => display_params,
        };
        debug!("Got display parameters: {:?}", display_params);

        match host.notify_loading_screen().await {
            Err(e) => warn!(
                "Couldn't notify {host} of loading screen provider, will continue anyways: {}",
                e
            ),
            Ok(_) => debug!("Notified {host} of loading screen..."),
        }

        debug!("Creating virtual screen...");
        let screen = match screen_provider.get_screen(display_params).await {
            Err(e) => {
                error!("Failed to create virtual screen: {}", e);
                close_dev(&mut host).await;
                return Err((host, "Failed to create virtual screen".to_string()));
            }
            Ok(screen) => screen,
        };
        debug!("Created virtual screen.");

        debug!("Creating encoder...");
        let mut encoder = match encoder_provider.create_encoder() {
            Err(e) => {
                error!("Failed to create encoder: {}", e);
                close_dev(&mut host).await;
                return Err((host, "Failed to create encoder".to_string()));
            }
            Ok(encoder) => encoder,
        };
        debug!("Created encoder.");

        debug!("Getting format parameters...");
        let format_params = screen.get_format_parameters();
        debug!("Got format parameters: {:?}", format_params);

        debug!("Initializing encoder...");
        let encoder_init_result = encoder
            .init(crate::host::EncoderParameters {
                width: format_params.width,
                height: format_params.height,
                bitrate: 1000000, // TODO: Make this configurable
                fps: 60,          // TODO: Make this configurable
                input_parameters: format_params,
            })
            .await;
        if let Err(e) = encoder_init_result {
            error!("Failed to initialize encoder: {}", e);
            close_dev(&mut host).await;
            return Err((host, "Failed to initialize encoder".to_string()));
        };
        debug!("Initialized encoder.");

        debug!("Starting screen loop...");
        let result = screen_loop(screen, host, encoder, screen_task_stopped.clone()).await;
        debug!("Screen loop finished.");
        result
    }
    .boxed_local();

    let (_, screen_result) = join!(background_task, screen_task);

    screen_result
}

async fn screen_loop<S, T, E>(
    mut screen: S,
    mut host: DisplayHost<T>,
    mut encoder: E,
    stop_flag: Arc<AtomicBool>,
) -> Result<DisplayHost<T>, (DisplayHost<T>, String)>
where
    S: Screen,
    T: ScreenTransport,
    E: Encoder,
{
    let mut bad_transmission_start: Option<Instant> = None;
    let mut bad_transmission_count = 0u32;

    let mut err: Option<String> = None;

    loop {
        match screen.get_ready().await {
            Ok(status) => match status {
                ScreenReadyStatus::Finished => {
                    info!("Virtual screen has finished");
                    stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
                ScreenReadyStatus::NotReady => {
                    futures_timer::Delay::new(NOT_READY_DELAY).await;
                    if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        info!("Screen task stop flag set, exiting not-ready wait loop");
                        break;
                    }
                }
                ScreenReadyStatus::Ready => {
                    if let Some(data) = screen.get_bytes() {
                        // TODO: Allow some sort of encoding here!
                        let now = Instant::now();
                        let encoded_data = match encoder.encode(data).await {
                            Ok(ed) => ed,
                            Err(e) => {
                                error!("Failed to encode screen data: {}", e);
                                err = Some("Failed to encode screen data".to_string());
                                break;
                            }
                        };
                        let encode_time = now.elapsed();
                        let send_result = host.send_screen_data(encoded_data).await;
                        let send_time = now.elapsed();
                        if let Err(e) = send_result {
                            error!("Error during transmission to screen host: {}", e);
                            let bad_transmission_elapsed =
                                if let Some(start) = bad_transmission_start {
                                    start.elapsed()
                                } else {
                                    bad_transmission_start = Some(Instant::now());
                                    Duration::ZERO
                                };
                            bad_transmission_count += 1;

                            if bad_transmission_elapsed >= Duration::from_secs(5)
                                && bad_transmission_count >= 5
                            {
                                error!(
                                    "Too many bad transmissions ({} errors in {}ms), closing connection",
                                    bad_transmission_count,
                                    bad_transmission_elapsed.as_millis()
                                );
                                stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                                err =
                                    Some("Too many bad transmissions to display host".to_string());
                                break;
                            }
                        } else {
                            bad_transmission_start = None;
                            bad_transmission_count = 0;
                            let kbs = encoded_data.len() as f64 / 1024.0 / send_time.as_secs_f64();
                            debug!(
                                "Sent {} bytes to display host in {}ms ({:.2} KB/s, encode time: {}ms, send time: {}ms)",
                                encoded_data.len(),
                                send_time.as_millis(),
                                kbs,
                                encode_time.as_millis(),
                                (send_time - encode_time).as_millis()
                            );
                        }
                    } else {
                        error!("Bytes were missing after declared ready!");
                    }
                }
            },
            Err(e) => {
                error!("Virtual screen error: {}", e);
                err = Some("Virtual screen runtime error".to_string());
            }
        }
    }

    if let Err(e) = host.close().await {
        error!("Error closing display host: {}", e);
    }

    if let Err(e) = screen.close().await {
        error!("Error closing virtual screen: {}", e);
    }

    if let Some(e) = err {
        return Err((host, e));
    } else {
        return Ok(host);
    }
}
