use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt};
use futures_util::FutureExt;
use log::{debug, error, info, trace, warn};

use crate::{
    client::{DisplayHost, ScreenTransport},
    host::{
        DisplayHostResult, Encoder, EncoderContentParameters, EncoderProvider, Screen,
        ScreenProvider, ScreenReadyStatus,
    },
};

const NOT_READY_DELAY: Duration = Duration::from_millis(100);

struct InitializedSystem<T, S, E> {
    screen: S,
    encoder: E,
    display_host: DisplayHost<T>,
}

/// Given all the ingredients to screen cast, handle a display host connection.
pub async fn handle_display_host<T, P, E, C>(
    screen_provider: P,
    encoder_provider: E,
    mut display_host: DisplayHost<T>,
    cancel_notification: C,
) -> DisplayHostResult<T>
where
    T: ScreenTransport + 'static,
    E: EncoderProvider + 'static,
    P: ScreenProvider + 'static,
    C: Stream<Item = ()> + Unpin + 'static,
{
    let stopped = Arc::new(AtomicBool::new(false));
    debug!("Getting background task for {display_host}...");
    let _background_stopped = stopped.clone();
    let host_name = display_host.to_string();
    let host_name_1 = host_name.clone();
    let display_host_background_task = display_host
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

    let screen_task = async move {
        let initialized_system =
            match screen_init(screen_provider, encoder_provider, display_host).await {
                Ok(system) => system,
                Err(e) => {
                    error!("Failed to initialize screen system: {}", e);
                    return Err(e);
                }
            };

        match screen_loop(initialized_system).await {
            Ok(host) => {
                debug!("Screen loop completed successfully.");
                display_host = host;
                Ok(display_host)
            }
            Err(e) => {
                error!("Screen loop ended with error: {}", e);
                Err(e)
            }
        }
    };

    let composition_task = async move { futures::join!(display_host_background_task, screen_task) };

    futures::select! {
        (_,screen_result) = composition_task.fuse() => screen_result,
        _ = cancel_notification.into_future().fuse() => {
            Err(format!("Display host handling for {} was cancelled", host_name_1))
        }
    }
}

async fn screen_init<T, P, E>(
    screen_provider: P,
    encoder_provider: E,
    mut display_host: DisplayHost<T>,
) -> Result<InitializedSystem<T, P::ScreenType, E::EncoderType>, String>
where
    T: ScreenTransport,
    E: EncoderProvider,
    P: ScreenProvider,
{
    // Handle the display-host connection here
    info!("Handling display-host: {display_host}");

    async fn close_dev(host: &mut DisplayHost<impl ScreenTransport>) {
        if let Err(_) = host.close().await {
            error!("Error closing display host");
        }
    }

    debug!("Initializing with transport...");
    // Initialize the transport
    if let Err(e) = display_host.initialize().await {
        error!("Failed to initialize transport: {}", e);
        close_dev(&mut display_host).await;
        return Err("Failed to initialize transport".to_string());
    }
    debug!("Initialized transport");

    debug!("Getting display parameters...");
    let display_params = match display_host.get_display_config().await {
        Err(e) => {
            error!("Failed to get display parameters: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to get display parameters".to_string());
        }
        Ok(display_params) => display_params,
    };
    debug!("Got display parameters: {:?}", display_params);

    match display_host.notify_loading_screen().await {
        Err(e) => warn!(
            "Couldn't notify {display_host} of loading screen provider, will continue anyways: {}",
            e
        ),
        Ok(_) => debug!("Notified {display_host} of loading screen..."),
    }

    debug!("Creating virtual screen...");
    let screen = match screen_provider.get_screen(display_params).await {
        Err(e) => {
            error!("Failed to create virtual screen: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to create virtual screen".to_string());
        }
        Ok(screen) => screen,
    };
    debug!("Created virtual screen.");

    debug!("Creating encoder...");
    let mut encoder = match encoder_provider.create_encoder().await {
        Err(e) => {
            error!("Failed to create encoder: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to create encoder".to_string());
        }
        Ok(encoder) => encoder,
    };
    debug!("Created encoder.");

    debug!("Getting format parameters...");
    let format_params = screen.get_format_parameters();
    debug!("Got format parameters: {:?}", format_params);

    let encoder_parameters = EncoderContentParameters {
        // Note here, formatting to the same width/height as the screen
        width: format_params.width,
        height: format_params.height,

        bitrate: 1000000, // TODO: Make this configurable?
        fps: 60,          // TODO: Make this configurable?
        encoder_input_parameters: format_params,
    };

    let supported_configurations = match encoder.get_supported_configurations(&encoder_parameters) {
        Err(e) => {
            error!("Failed to get supported encoder configurations: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to get supported encoder configurations".to_string());
        }
        Ok(configs) => configs,
    };

    if supported_configurations.is_empty() {
        error!("No supported encoder configurations available");
        close_dev(&mut display_host).await;
        return Err("No supported encoder configurations available".to_string());
    }

    let preferred_configurations = match display_host
        .get_preferred_encodings(supported_configurations)
        .await
    {
        Err(e) => {
            error!(
                "Failed to get preferred encoder configurations from host: {}",
                e
            );
            close_dev(&mut display_host).await;
            return Err("Failed to get preferred encoder configurations".to_string());
        }
        Ok(configs) => configs,
    };

    debug!(
        "Got supported {} encoder configurations: {:#?}",
        preferred_configurations.len(),
        preferred_configurations
    );

    debug!("Initializing encoder...");
    let encoder_init_result = encoder
        .init(encoder_parameters, Some(preferred_configurations))
        .await;
    let initialized_codec = match encoder_init_result {
        Err(e) => {
            error!("Failed to initialize encoder: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to initialize encoder".to_string());
        }
        Ok(config) => config,
    };
    debug!(
        "Initialized encoder with {}.",
        initialized_codec.encoder_name
    );

    debug!("Setting encoding on host...");
    if let Err(e) = display_host.set_encoding(initialized_codec).await {
        error!("Failed to set encoding on host: {}", e);
        close_dev(&mut display_host).await;
        return Err("Failed to set encoding on host".to_string());
    }
    debug!("Set encoding on host.");

    Ok(InitializedSystem {
        screen,
        encoder,
        display_host,
    })
}

async fn screen_loop<S, T, E>(
    initialized_system: InitializedSystem<T, S, E>,
) -> DisplayHostResult<T>
where
    S: Screen,
    T: ScreenTransport,
    E: Encoder,
{
    let mut bad_transmission_start: Option<Instant> = None;
    let mut bad_transmission_count = 0u32;

    let mut err: Option<String> = None;
    let InitializedSystem {
        mut screen,
        display_host: mut host,
        mut encoder,
    } = initialized_system;

    loop {
        match screen.get_ready().await {
            Ok(status) => match status {
                ScreenReadyStatus::Finished => {
                    info!("Virtual screen has finished");
                    break;
                }
                ScreenReadyStatus::NotReady => {
                    futures_timer::Delay::new(NOT_READY_DELAY).await;
                }
                ScreenReadyStatus::Ready => {
                    if let Some(data) = screen.get_bytes() {
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
                                err =
                                    Some("Too many bad transmissions to display host".to_string());
                                break;
                            }
                        } else {
                            bad_transmission_start = None;
                            bad_transmission_count = 0;
                            let kbs = encoded_data.len() as f64 / 1024.0 / send_time.as_secs_f64();
                            trace!(
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
                break;
            }
        }
    }

    if let Err(e) = host.close().await {
        error!("Error closing display host: {}", e);
    }

    if let Err(e) = screen.close().await {
        error!("Error closing virtual screen: {}", e);
    }

    if let Some(e) = err { Err(e) } else { Ok(host) }
}
