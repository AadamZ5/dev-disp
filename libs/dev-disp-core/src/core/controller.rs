use core::error;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use futures::{StreamExt, channel::oneshot};
use futures_util::{FutureExt, stream::FuturesUnordered};
use log::{debug, error, info, warn};

use crate::{
    client::{DisplayHost, ScreenTransport},
    host::{DisplayHostResult, Screen, ScreenProvider, ScreenReadyStatus},
};

const NOT_READY_DELAY: Duration = Duration::from_millis(100);

pub fn handle_display_host<T, P>(
    provider: P,
    mut host: DisplayHost<T>,
) -> impl Future<Output = DisplayHostResult<T>>
where
    T: ScreenTransport + 'static,
    P: ScreenProvider + 'static,
{
    let mut tasks = FuturesUnordered::new();
    let mut stopped = Arc::new(AtomicBool::new(false));

    let (device_tx, device_rx) = oneshot::channel();

    debug!("Spawning background task for {host}...");
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
            background_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
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
        let init_result = host.initialize().await;
        if let Err(e) = init_result {
            error!("Failed to initialize transport: {}", e);
            close_dev(&mut host).await;
            return Err("Failed to initialize transport".to_string());
        }
        debug!("Initialized transport");

        debug!("Getting display parameters...");
        let display_params_result = host.get_display_config().await;
        if let Err(e) = display_params_result {
            error!("Failed to get display parameters: {}", e);
            close_dev(&mut host).await;
            return Err("Failed to get display parameters".to_string());
        }
        let display_params = display_params_result.unwrap();
        debug!("Got display parameters: {:?}", display_params);

        match host.notify_loading_screen().await {
            Err(e) => warn!(
                "Couldn't notify {host} of loading screen provider, will continue anyways: {}",
                e
            ),
            Ok(_) => debug!("Notified {host} of loading screen..."),
        }

        debug!("Creating virtual screen...");
        let screen_result = provider.get_screen(display_params).await;
        if let Err(e) = screen_result {
            error!("Failed to create virtual screen: {}", e);
            close_dev(&mut host).await;
            return Err("Failed to create virtual screen".to_string());
        }
        let mut screen = screen_result.unwrap();

        let mut bad_transmission_start: Option<Instant> = None;
        let mut bad_transmission_count = 0u32;

        loop {
            match screen.get_ready().await {
                Ok(status) => match status {
                    ScreenReadyStatus::Finished => {
                        info!("Virtual screen has finished");
                        close_dev(&mut host).await;
                        screen_task_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                    ScreenReadyStatus::NotReady => {
                        futures_timer::Delay::new(NOT_READY_DELAY).await;
                        if (screen_task_stopped.load(std::sync::atomic::Ordering::SeqCst)) {
                            info!("Screen task stop flag set, exiting not-ready wait loop");
                            break;
                        }
                    }
                    ScreenReadyStatus::Ready => {
                        info!("Screen data ready!");
                        if let Some(data) = screen.get_bytes() {
                            // TODO: Allow some sort of encoding here!
                            let now = Instant::now();
                            let send_result = host.send_screen_data(data).await;
                            let elapsed = now.elapsed();
                            if let Err(e) = send_result {
                                error!("Error during transmission to screen host: {}", e);
                                let bad_transmission_elapsed = if let Some(start) = bad_transmission_start {
                                    start.elapsed()
                                } else {
                                    bad_transmission_start = Some(Instant::now());
                                    Duration::ZERO
                                };
                                bad_transmission_count += 1;

                                if bad_transmission_elapsed >= Duration::from_secs(5) && bad_transmission_count >= 5 {
                                    error!(
                                        "Too many bad transmissions ({} errors in {}ms), closing connection",
                                        bad_transmission_count, bad_transmission_elapsed.as_millis()
                                    );
                                    close_dev(&mut host).await;
                                    screen_task_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if let Err(e) = screen.close().await {
                                        error!("Error closing virtual screen: {}", e);
                                    }
                                    return Err("Too many bad transmissions to display host".to_string());
                                }

                            } else {
                                bad_transmission_start = None;
                                bad_transmission_count = 0;
                                let kbs = data.len() as f64 / 1024.0 / elapsed.as_secs_f64();
                                debug!(
                                    "Sent {} bytes to display host in {}ms ({:.2} KB/s)",
                                    data.len(),
                                    elapsed.as_millis(),
                                    kbs
                                );
                            }
                        } else {
                            error!("Bytes were missing after declared ready!");
                        }
                    }
                },
                Err(e) => {
                    error!("Virtual screen error: {}", e);
                    close_dev(&mut host).await;
                    if let Err(e) = screen.close().await {
                        error!("Error closing virtual screen: {}", e);
                    }
                    return Err("Virtual screen runtime error".to_string());
                }
            }
        }

        if let Err(e) = screen.close().await {
            error!("Error closing virtual screen: {}", e);
        }

        Ok(host)
    }
    .map(|res: Result<DisplayHost<T>, String>| match res {
        Ok(v) => {
            debug!("Display host handling finished successfully");
            device_tx
                .send(v)
                .map_err(|_| "Could not send device back to caller".to_string())
        }
        Err(e) => Err(format!("Error handling client: {}", e)),
    })
    .boxed_local();

    tasks.push(background_task);
    tasks.push(screen_task);

    async move {
        while let Some(_) = tasks.next().await {
            // Don't care about results!
        }

        device_rx
            .await
            .map_err(|_| "Could not receive device from task".to_string())
    }
}
