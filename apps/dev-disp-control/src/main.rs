use dev_disp_core::daemon::api::DeviceRef;
use futures::StreamExt;
use iced::{
    Task,
    widget::{Button, Column, Container, Text},
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
    available_devices: Vec<DeviceRef>,
    connected_devices: Vec<DeviceRef>,
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
            available_devices: Vec::new(),
            connected_devices: Vec::new(),
        };

        (
            this,
            Task::batch([
                Task::stream(backend_stream.map(UiAction::BackendEvent)),
                Task::done(UiAction::BackendCommand(backend::Command::Connect(
                    "http://[::1]:50051".to_string(),
                ))),
            ]),
        )
    }

    pub fn view(&self) -> Column<UiAction> {
        let connected_text = Text::new(match &self.connection_state {
            ConnectionState::Disconnected => "Disconnected".to_string(),
            ConnectionState::Connecting(addr) => format!("Connecting to {}...", addr),
            ConnectionState::Connected(addr) => format!("Connected to {}", addr),
        });

        let c = Column::new().push(connected_text).padding(20).spacing(10);

        let mut available = Column::new()
            .push(Text::new("Available Devices:").size(24))
            .padding(10)
            .spacing(10);

        for device in &self.available_devices {
            available = available.push(Container::new(simple_device_info(device)))
        }

        let mut connected = Column::new()
            .push(Text::new("Connected Devices:").size(24))
            .padding(10)
            .spacing(10);

        for device in &self.connected_devices {
            connected = connected.push(Container::new(simple_device_info(device)))
        }

        c.push(available).push(connected)
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
                        self.backend_ref.send(backend::Command::StreamDevices);
                    }
                    backend::Event::Disconnected => {
                        self.connection_state = ConnectionState::Disconnected;
                    }
                    backend::Event::DeviceListUpdated(device) => {
                        log::info!("Device list updated: {:?}", device);
                        self.available_devices = device.connectable_devices;
                        self.connected_devices = device.in_use_devices;
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
        .filter_module("h2::codec", LevelFilter::Warn)
        .init();

    iced::application(UiTest::new, UiTest::update, UiTest::view).run()
}

fn simple_device_info(device: &DeviceRef) -> Container<'static, UiAction> {
    Container::new(
        Column::new()
            .push(Text::new(format!("Name: {}", device.name)))
            .push(Text::new(format!("Discovery ID: {}", device.interface_key)))
            .push(Text::new(format!(
                "Discovery Display: {}",
                device.interface_display
            )))
            .push(Text::new(format!("ID: {}", device.id))),
    )
}
