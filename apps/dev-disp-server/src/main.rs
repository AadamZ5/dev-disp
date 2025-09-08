use dev_disp_comm_usb::{UsbConnectionStrategy, connect_usb};
use dev_disp_core::{client::DisplayHost, host::ScreenProvider};
use dev_disp_provider_evdi::EvdiScreenProvider;
use log::{LevelFilter, debug, error, info};

use std::{process::exit, thread, time::Duration};

const USB_SAMSUNG_VENDOR_ID: u16 = 0x04E8;
const USB_SAMSUNG_PRODUCT_ID: u16 = 0x6860;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let sammy_accessory = connect_usb(
        USB_SAMSUNG_VENDOR_ID,
        USB_SAMSUNG_PRODUCT_ID,
        UsbConnectionStrategy::AndroidAccessory,
    )
    .await
    .expect("Couldn't get the sammy device in accessory mode");

    let display = DisplayHost::new(1, "Samsung Galaxy S9+".to_string(), sammy_accessory);

    let evdi_provider = EvdiScreenProvider {};
    let handled = evdi_provider.handle_display_host(display).await;

    if let Err(e) = handled {
        error!("Error handling display host: {}", e);
        exit(1);
    }

    handled
        .unwrap()
        .into_transport()
        .into_device()
        .reset()
        .await
        .expect("Failed to reset device");
    info!("Device reset successfully, exiting...");
}
