use dev_disp_core::daemon::api::DevDispApi;
use log::LevelFilter;

use crate::{application::DevDispApplication, backend::DisconnectableApi};

mod api;
mod application;
mod backend;
mod util;
mod widgets;

pub fn main() -> iced::Result {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("tracing", LevelFilter::Warn)
        .filter_module("cosmic_text", LevelFilter::Warn)
        .filter_module("wgpu_core", LevelFilter::Warn)
        .filter_module("naga", LevelFilter::Warn)
        .filter_module("h2::codec", LevelFilter::Warn)
        .init();

    iced::application(
        application_init,
        DevDispApplication::update,
        DevDispApplication::view,
    )
    .run()
}

fn application_init() -> (
    DevDispApplication<
        impl backend::ApiFactory<
            ConnectParam = String,
            Api = impl DevDispApi + DisconnectableApi + 'static + Send,
        >
        + 'static
        + Send,
    >,
    iced::Task<crate::application::UiAction>,
) {
    let api_factory = api::grpc::grpc_api_factory();
    let initial_connect_param = Some("http://[::1]:50051".to_string());

    DevDispApplication::new(api_factory, initial_connect_param)
}
