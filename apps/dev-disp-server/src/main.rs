use std::process::exit;

use dev_disp_comm_usb::{UsbConnectionStrategy, connect_usb, discovery::UsbDiscovery};
use dev_disp_core::{
    client::{DisplayHost, SomeScreenTransport},
    host::ConnectableDevice,
    host::DeviceDiscovery,
    host::ScreenProvider,
};
use dev_disp_provider_evdi::EvdiScreenProvider;
use futures_util::FutureExt;
use log::{LevelFilter, error, info, warn};
use tokio::{signal::ctrl_c, task::LocalSet};

const SAMSUNG_SERIAL: &str = "RFCT71HTZNL";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let evdi_provider = EvdiScreenProvider::new();
    let evdi_provider_1 = evdi_provider.clone();

    let local_set = LocalSet::new();
    let single_thread_work = local_set.run_until(async move {
        let logic = tokio::task::spawn_local(async move {
            let usb_discovery = UsbDiscovery {};

            // TODO: Implement some UI for picking this stuff
            let sammy_accessory = usb_discovery
                .discover_devices()
                .await
                .into_iter()
                .find(|dev| dev.get_info().id == SAMSUNG_SERIAL)
                .expect("Could not find Samsung device");

            let display = sammy_accessory
                .connect()
                .await
                .expect("Failed to connect to device");

            let display = display.to_some_transport();

            let evdi_provider = evdi_provider_1;
            let handle_result = evdi_provider.handle_display_host(display).await;

            if let Err(e) = handle_result {
                error!("Error handling display host: {}", e);
            } else {
                info!("Display host handling completed successfully");
            }
        });

        let ctrl_c_listener = tokio::task::spawn_local(async move {
            ctrl_c().await.expect("Failed to listen for Ctrl-C");
            warn!("Received Ctrl-C, shutting down");
            evdi_provider.stop();

            ctrl_c().await.expect("Failed to listen for second Ctrl-C");
            error!("Received second Ctrl-C, forcing exit");
            exit(1);
        });

        let res = futures_util::select_biased! {
            logic_result = logic.fuse() => logic_result,
            ctrl_c_result = ctrl_c_listener.fuse() => ctrl_c_result,
        };

        if let Err(e) = res {
            error!("Error in main logic: {}", e);
        }
    });

    single_thread_work.await;
    info!("Exiting");
}
