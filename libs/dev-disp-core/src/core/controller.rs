//! This module handles core controller logic. This is **the** business logic of the application once a screen host has been initiated.
//!
//! It orchestrates the interaction between the screen, encoder, and display host, managing the lifecycle and state transitions.

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use futures::{Sink, SinkExt, Stream, StreamExt};
use futures_util::FutureExt;
use log::{debug, error, info, trace, warn};

use crate::{
    client::{DisplayHost, ScreenTransport},
    coding::encoder::EncoderContentParameters,
    host::{DisplayHostResult, Screen, ScreenProvider, ScreenReadyStatus},
};

const NOT_READY_DELAY: Duration = Duration::from_millis(100);

/// Bespoke context object used in each phase of the controller casting session
#[derive(Debug)]
struct InitializedSystem<T, S, St> {
    screen: S,
    display_host: DisplayHost<T>,
    status_sink: St,
}

/// Useful system state for what is currently happening in the business logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemState {
    #[default]
    Unknown,
    /// Very beginning of controller initialization
    Initializing,
    /// Initializing the transport layer for the screen.
    InitializingTransport,
    /// Getting the display parameters from the screen host (remote device)
    GettingDisplayParameters,
    /// Notifying the client that the virt screen is loading.
    NotifyClientLoading,
    /// Acquiring the screen from the screen provider.
    GettingScreen,
    GettingEncoder, // TODO: Reconsider
    /// Negotiating codec configuration between transport and client
    NegotiatingCodecs,
    InitializingEncoder, // TODO: Reconsider
    SettingClientCodec,  // TODO: Reconsider
    /// Running the screen casting session.
    Running,
    /// The screen casting session has been stopped.
    Stopped,
}

/// Given all the ingredients to screen cast, handle a display host connection.
pub async fn handle_display_host<T, P, C, St>(
    screen_provider: P,
    mut display_host: DisplayHost<T>,
    cancel_notification: C,
    status_sink: St,
) -> DisplayHostResult<T>
where
    T: ScreenTransport + 'static,
    P: ScreenProvider + 'static,
    C: Stream<Item = ()> + Unpin + 'static,
    St: Sink<SystemState> + Unpin + 'static,
{
    let stopped = Arc::new(AtomicBool::new(false));
    debug!("Getting background task for {display_host}...");
    let _background_stopped = stopped.clone();
    let host_name = display_host.to_string();
    let host_name_1 = host_name.clone();
    let display_host_background_task = display_host
        .background_task()
        .map(|r| r.map_err(|e| e.to_string()))
        .boxed_local();

    let screen_task = async move {
        let initialized_system = match screen_init(screen_provider, display_host, status_sink).await
        {
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
    }
    .boxed_local();

    let composition_task = async move {
        let mut screen_task = screen_task.fuse();
        let mut background_task = display_host_background_task.fuse();

        // Compose the tasks in such a way that if the background task finishes, we
        // still continue to poll the screen task. But... if the screen task finishes,
        // we do *not* continue polling the background task.
        loop {
            futures::select! {
                background_result = background_task => {
                    debug!(
                        "Background task for {host_name} finished with result: {:?}",
                        background_result
                    );
                    // And no-op, `.fuse()` will keep that future pending forever after
                    // it completes.
                },
                screen_result = screen_task => {
                    return screen_result;
                }
            }
        }
    };

    futures::select! {
        screen_result = composition_task.fuse() => screen_result,
        _ = cancel_notification.into_future().fuse() => { // TODO: Pass cancellation notification into loop fn.
            Err(format!("Display host handling for {} was cancelled", host_name_1))
        }
    }
}

async fn screen_init<T, P, St>(
    screen_provider: P,
    mut display_host: DisplayHost<T>,
    mut status_sink: St,
) -> Result<InitializedSystem<T, P::ScreenType, St>, String>
where
    T: ScreenTransport,
    P: ScreenProvider,
    St: Sink<SystemState> + Unpin + 'static,
{
    // Handle the display-host connection here
    info!("Handling display-host: {display_host}");

    async fn close_dev(host: &mut DisplayHost<impl ScreenTransport>) {
        if let Err(_) = host.close().await {
            error!("Error closing display host");
        }
    }

    debug!("Initializing with transport...");
    match status_sink.send(SystemState::InitializingTransport).await {
        Err(_) => warn!("Failed to send initializing status"),
        _ => {}
    };
    // Initialize the transport
    if let Err(e) = display_host.initialize().await {
        error!("Failed to initialize transport: {}", e);
        close_dev(&mut display_host).await;
        return Err("Failed to initialize transport".to_string());
    }
    debug!("Initialized transport");

    debug!("Getting display parameters...");
    match status_sink
        .send(SystemState::GettingDisplayParameters)
        .await
    {
        Err(_) => warn!("Failed to send getting display parameters status"),
        _ => {}
    };
    // Get display params
    let display_params = match display_host.get_display_config().await {
        Err(e) => {
            error!("Failed to get display parameters: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to get display parameters".to_string());
        }
        Ok(display_params) => display_params,
    };
    debug!("Got display parameters: {:?}", display_params);

    match status_sink.send(SystemState::NotifyClientLoading).await {
        Err(_) => warn!("Failed to send notify client loading status"),
        _ => {}
    };

    match display_host.notify_loading_screen().await {
        Err(e) => warn!(
            "Couldn't notify {display_host} of loading screen provider, will continue anyways: {}",
            e
        ),
        Ok(_) => debug!("Notified {display_host} of loading screen..."),
    }

    debug!("Creating virtual screen...");
    match status_sink.send(SystemState::GettingScreen).await {
        Err(_) => warn!("Failed to send getting screen status"),
        _ => {}
    };
    // Get the virtual screen
    let screen = match screen_provider.get_screen(display_params).await {
        Err(e) => {
            error!("Failed to create virtual screen: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to create virtual screen".to_string());
        }
        Ok(screen) => screen,
    };
    debug!("Created virtual screen.");

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

    match status_sink.send(SystemState::NegotiatingCodecs).await {
        Err(_) => warn!("Failed to send negotiating codecs status"),
        _ => {}
    };

    // TODO: Return basic info about the codec here so we can log it
    match display_host
        .setup_encoding_config(&encoder_parameters)
        .await
    {
        Err(e) => {
            error!("Failed to negotiate and setup encoder: {}", e);
            close_dev(&mut display_host).await;
            return Err("Failed to negotiate and setup encoder".to_string());
        }
        Ok(_) => {}
    };

    debug!("Setup encoding configuration completed");

    Ok(InitializedSystem {
        screen,
        display_host,
        status_sink,
    })
}

async fn screen_loop<S, T, St>(
    initialized_system: InitializedSystem<T, S, St>,
) -> DisplayHostResult<T>
where
    S: Screen,
    T: ScreenTransport,
    St: Sink<SystemState> + Unpin + 'static,
{
    let mut bad_transmission_start: Option<Instant> = None;
    let mut bad_transmission_count = 0u32;

    let mut err: Option<String> = None;
    let InitializedSystem {
        mut screen,
        display_host: mut host,
        mut status_sink,
    } = initialized_system;

    match status_sink.send(SystemState::Running).await {
        Err(_) => warn!("Failed to send running status"),
        _ => {}
    };

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
                        let encoded_data = match host.encode(data).await {
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
