use futures::StreamExt;
use iced::{application, Subscription};
use log::{debug, error, LevelFilter};
use tokio::runtime::LocalOptions;

use crate::{
    app::{Message, ScreenHostApp},
    util::TokioRuntime,
    worker::{worker_main, WorkerError, WorkerEvent},
};

mod adapter;
mod app;
mod util;
mod worker;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("tracing", LevelFilter::Warn)
        .filter_module("cosmic_text", LevelFilter::Warn)
        .filter_module("wgpu_core", LevelFilter::Warn)
        .filter_module("naga", LevelFilter::Warn)
        .filter_module("h2::codec", LevelFilter::Warn)
        .filter_module("wgpu_hal", LevelFilter::Warn)
        .filter_module("iced_wgpu", LevelFilter::Warn)
        .filter_module("iced", LevelFilter::Warn)
        .filter_module("winit", LevelFilter::Warn)
        .filter_module("sctk", LevelFilter::Warn)
        .filter_module("zbus", LevelFilter::Warn)
        .init();

    let app_result = application(
        || ScreenHostApp::new("ws://localhost:56789".to_string()),
        ScreenHostApp::update,
        ScreenHostApp::view,
    )
    .window(iced::window::Settings {
        size: (800, 600).into(),
        ..Default::default()
    })
    .subscription(application_subscriptions)
    .run();

    println!("{:?}", app_result);
}

fn application_subscriptions(_state: &ScreenHostApp) -> Subscription<Message> {
    Subscription::batch(vec![
        spawn_background(_state),
        iced::window::resize_events()
            .map(|(_id, size)| Message::WindowSizeChange(size))
            .into(),
    ])
}

fn spawn_background(_state: &ScreenHostApp) -> Subscription<Message> {
    Subscription::run(|| {
        let (mut event_tx, event_rx) = futures::channel::mpsc::channel::<WorkerEvent>(100);

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_time()
                .worker_threads(2)
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to build worker runtime: {:?}", e);
                    let _ = event_tx.try_send(WorkerEvent::Error(WorkerError::RuntimeFailed));
                    return;
                }
            };

            match runtime.block_on(worker_main(TokioRuntime, event_tx.clone())) {
                Ok(_) => {}
                Err(e) => {
                    error!(
                        "Worker runtime failed while `block_on` worker_main: {:?}",
                        e
                    );
                    let _ = event_tx.try_send(WorkerEvent::Error(WorkerError::RuntimeFailed));
                }
            }
        });

        event_rx.map(|event| {
            debug!("Received worker event: {:?}", event);
            Message::WorkerEvent(event)
        })
    })
}
