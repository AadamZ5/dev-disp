use futures::StreamExt;
use iced::{
    Task,
    widget::{Button, Column},
};
use log::LevelFilter;

use crate::backend::{BackendRef, prepare_backend};

mod backend;
mod util;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectionState {
    Disconnected,
    Connecting(String),
    Connected(String),
}

#[derive(Debug)]
struct UiTest {
    counter: i64,
    backend_ref: BackendRef,
    connection_state: ConnectionState,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    Increment,
    Decrement,
    BackendEvent(backend::Event),
    BackendCommand(backend::Command),
}

impl UiTest {
    pub fn new() -> (Self, Task<UiAction>) {
        let (backend_ref, backend_stream) = prepare_backend();

        let this = Self {
            counter: 0,
            backend_ref,
            connection_state: ConnectionState::Disconnected,
        };

        (
            this,
            Task::stream(backend_stream.map(UiAction::BackendEvent)),
        )
    }

    pub fn view(&self) -> Column<UiAction> {
        let is_connecting = matches!(self.connection_state, ConnectionState::Connecting(_));

        let button = if self.connection_state == ConnectionState::Disconnected || is_connecting {
            let button = Button::new("Connect Backend");
            if !is_connecting {
                button.on_press(UiAction::BackendCommand(backend::Command::Connect(
                    "http://[::1]:50051".to_string(),
                )))
            } else {
                button
            }
        } else {
            Button::new("Disconnect Backend")
                .on_press(UiAction::BackendCommand(backend::Command::Disconnect))
        };

        Column::new().push(button).padding(20).spacing(10)
    }

    pub fn update(&mut self, action: UiAction) {
        match action {
            UiAction::Increment => self.counter += 1,
            UiAction::Decrement => self.counter -= 1,
            UiAction::BackendEvent(e) => {
                log::info!("Received backend event: {:?}", e);
                match e {
                    backend::Event::Connected(endpoint) => {
                        self.connection_state = ConnectionState::Connected(endpoint);
                    }
                    backend::Event::Disconnected => {
                        self.connection_state = ConnectionState::Disconnected;
                    }
                    backend::Event::DeviceListUpdated(device) => {
                        log::info!("Device list updated: {:?}", device);
                    }
                }
            }
            UiAction::BackendCommand(cmd) => {
                log::info!("Sending backend command: {:?}", cmd);
                match &cmd {
                    backend::Command::Connect(addr) => {
                        self.connection_state = ConnectionState::Connecting(addr.clone());
                    }
                    _ => {}
                };
                self.backend_ref.send(cmd);
            }
        }
    }
}

pub fn main() -> iced::Result {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("tracing", LevelFilter::Warn)
        .filter_module("cosmic_text", LevelFilter::Warn)
        .filter_module("wgpu_core", LevelFilter::Warn)
        .filter_module("naga", LevelFilter::Warn)
        .init();

    iced::application(UiTest::new, UiTest::update, UiTest::view).run()
}
