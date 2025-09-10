use dev_disp_comm_usb::{UsbConnectionStrategy, connect_usb, discovery::UsbDiscovery};
use dev_disp_core::{
    client::{DisplayHost, SomeScreenTransport},
    host::ConnectableDevice,
    host::DeviceDiscovery,
    host::ScreenProvider,
};
use dev_disp_provider_evdi::EvdiScreenProvider;
use log::{LevelFilter, error};
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
            error!("Display host handling completed successfully");
        }
    });

    tokio::select! {
        _ = single_thread_work => {},
        _ = ctrl_c() => {
            error!("Received Ctrl-C, shutting down");
            evdi_provider.stop();
        }
    }
}
