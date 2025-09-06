use dev_disp_core::{client::DevDispClient, core::handle_client};
use log::LevelFilter;
use std::process::exit;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let dumb_client = DevDispClient::new(42, "DumbClient".to_string());

    if let Err(e) = handle_client(dumb_client).await {
        eprintln!("Error handling client: {e}");
        exit(1);
    }
}
