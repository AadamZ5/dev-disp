use dev_disp_comm_usb::{UsbConnectionStrategy, connect_usb};
use dev_disp_core::{
    client::{DisplayHost, SomeScreenTransport},
    host::ScreenProvider,
};
use dev_disp_provider_evdi::EvdiScreenProvider;
use log::{LevelFilter, error};

use std::process::exit;

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

    let dev_title = sammy_accessory
        .device_info()
        .product_string()
        .unwrap_or("Unknown");
    let dev_serial = sammy_accessory
        .device_info()
        .serial_number()
        .unwrap_or("Unknown");

    let dev_string = format!("{} ({})", dev_title, dev_serial);

    // Example dynamic transport
    let dyn_sammy = SomeScreenTransport::new(Box::new(sammy_accessory));

    let display = DisplayHost::new(1, dev_string, dyn_sammy);

    let evdi_provider = EvdiScreenProvider {};
    let handled = evdi_provider.handle_display_host(display).await;

    if let Err(e) = handled {
        error!("Error handling display host: {}", e);
        exit(1);
    }
}
