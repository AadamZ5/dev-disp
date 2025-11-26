use core::error;
use std::time::Duration;

use futures_util::FutureExt;
use log::{debug, error, info};

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
    async move {
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

        let display_params_result = host.get_display_config().await;
        if let Err(e) = display_params_result {
            error!("Failed to get display parameters: {}", e);
            close_dev(&mut host).await;
            return Err("Failed to get display parameters".to_string());
        }
        let display_params = display_params_result.unwrap();

        let screen_result = provider.get_screen(display_params).await;
        if let Err(e) = screen_result {
            error!("Failed to create virtual screen: {}", e);
            close_dev(&mut host).await;
            return Err("Failed to create virtual screen".to_string());
        }
        let mut screen = screen_result.unwrap();

        loop {
            match screen.get_ready().await {
                Ok(status) => match status {
                    ScreenReadyStatus::Finished => {
                        info!("Virtual screen has finished");
                        close_dev(&mut host).await;
                        break;
                    }
                    ScreenReadyStatus::NotReady => {
                        futures_timer::Delay::new(NOT_READY_DELAY).await;
                    }
                    ScreenReadyStatus::Ready => {
                        info!("Screen data ready!");
                        let data = screen.get_bytes();
                        let send_result = host.send_screen_data(data).await;
                        if let Err(e) = send_result {
                            error!("Error during transmission to screen host: {}", e);
                        }
                    }
                },
                Err(e) => {
                    error!("Virtual screen error: {}", e);
                    close_dev(&mut host).await;
                    return Err("Virtual screen runtime error".to_string());
                }
            }
        }

        Ok(host)
    }
    .map(|res: Result<DisplayHost<T>, String>| match res {
        Ok(v) => Ok(v),
        Err(e) => Err(format!("Error handling client: {}", e)),
    })
}
