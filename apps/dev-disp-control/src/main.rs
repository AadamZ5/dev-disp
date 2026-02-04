use dev_disp_core::daemon::api::{
    DiscoveryId, DisplayHostId, DisplayHostRef, DisplayHostStatus, InitializationState,
};
use futures::StreamExt;
use iced::{
    Element, Font, Task, Theme, font,
    widget::{
        Column, Container, Row, button, container, rich_text, span, text,
        text::{IntoFragment, Span},
    },
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
    backend_ref: BackendRef,
    connection_state: ConnectionState,
    available_devices: Vec<DisplayHostRef>,
    connected_devices: Vec<DisplayHostRef>,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    BackendEvent(backend::Event),
    BackendCommand(backend::Command),
    ConnectDevice(DisplayHostId, DiscoveryId),
    DisconnectDevice(DisplayHostId, DiscoveryId),
}

impl UiTest {
    pub fn new() -> (Self, Task<UiAction>) {
        let (backend_ref, backend_stream) = prepare_backend();

        let this = Self {
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
        let connected_text = text(match &self.connection_state {
            ConnectionState::Disconnected => "Disconnected".to_string(),
            ConnectionState::Connecting(addr) => format!("Connecting to {}...", addr),
            ConnectionState::Connected(addr) => format!("Connected to {}", addr),
        });

        let c = Column::new().push(connected_text).padding(20).spacing(10);

        let mut available = Column::new()
            .push(text("Available Devices:").size(24))
            .padding(10)
            .spacing(10);

        for device in &self.available_devices {
            available = available.push(Container::new(simple_device_info(device, false)))
        }

        if self.available_devices.is_empty() {
            available = available.push(text("No available devices found."));
        }

        let mut connected = Column::new()
            .push(text("Connected Devices:").size(24))
            .padding(10)
            .spacing(10);

        for device in &self.connected_devices {
            connected = connected.push(Container::new(simple_device_info(device, true)))
        }

        if self.connected_devices.is_empty() {
            connected = connected.push(text("No connected devices found."));
        }

        c.push(available).push(connected)
    }

    pub fn update(&mut self, action: UiAction) {
        match action {
            UiAction::ConnectDevice(dev_id, discovery_id) => {
                log::info!(
                    "Requesting connection to device {:?} via discovery ID {:?}",
                    dev_id,
                    discovery_id
                );
                self.backend_ref
                    .send(backend::Command::ConnectDevice(dev_id, discovery_id));
            }
            UiAction::DisconnectDevice(dev_id, discovery_id) => {
                log::info!(
                    "Requesting disconnection from device {:?} via discovery ID {:?}",
                    dev_id,
                    discovery_id
                );
                self.backend_ref
                    .send(backend::Command::DisconnectDevice(dev_id, discovery_id));
            }
            UiAction::BackendEvent(e) => match e {
                backend::Event::Connected(endpoint) => {
                    self.connection_state = ConnectionState::Connected(endpoint);
                    self.backend_ref.send(backend::Command::StreamDevices);
                }
                backend::Event::Disconnected => {
                    self.connection_state = ConnectionState::Disconnected;
                }
                backend::Event::DeviceListUpdated(devices) => {
                    self.available_devices = devices.connectable_devices;
                    self.connected_devices = devices.in_use_devices;
                }
            },
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

fn simple_device_info(device: &DisplayHostRef, connected: bool) -> Container<'_, UiAction> {
    Container::new(
        Column::new()
            .push(label("Name:", text(&device.name)))
            .push(label("Discovery Method:", code_text(&device.discovery_id)))
            .push(label("Device ID:", code_text(&device.id)))
            .push(label(
                "Status:",
                text(status_to_display_string(&device.status)),
            ))
            .push(if connected {
                let mut disconnect_button = button("Disconnect");
                if device.status == DisplayHostStatus::InUse
                    || device.status == DisplayHostStatus::Unknown
                {
                    disconnect_button = disconnect_button.on_press(UiAction::DisconnectDevice(
                        device.id.clone(),
                        device.discovery_id.clone(),
                    ));
                };
                disconnect_button
            } else {
                let mut connect_button = button("Connect");
                if device.status == DisplayHostStatus::Available {
                    connect_button = connect_button.on_press(UiAction::ConnectDevice(
                        device.id.clone(),
                        device.discovery_id.clone(),
                    ));
                };
                connect_button
            })
            .spacing(5)
            .width(400),
    )
    .padding(15)
    .style(|theme: &Theme| {
        let mut style = container::Style::default();

        style.border.radius = 5.0.into();
        style.border.width = 1.0;
        style.border.color = {
            let mut bg_lighter = theme.palette().background;
            bg_lighter.r += 0.05;
            bg_lighter.g += 0.05;
            bg_lighter.b += 0.05;
            bg_lighter
        };
        style
    })
}

fn label<'a, T, C>(label: T, content: C) -> Row<'a, UiAction>
where
    T: IntoFragment<'a>,
    C: Into<Element<'a, UiAction>>,
{
    let bold_label: Span<'a> = span(label).font(Font {
        weight: font::Weight::Bold,
        ..Default::default()
    });

    Row::new()
        .push(rich_text![bold_label])
        .push(content.into())
        .spacing(5)
}

fn code_text<'a, T>(content: T) -> Container<'a, UiAction>
where
    T: IntoFragment<'a>,
{
    Container::new(text(content).size(12))
        .style(|theme: &Theme| {
            let mut bg_lighter = theme.palette().background;
            bg_lighter.r += 0.05;
            bg_lighter.g += 0.05;
            bg_lighter.b += 0.05;

            let mut style = container::Style::default().background(bg_lighter);
            let mut bg_lighter_lighter = bg_lighter;
            bg_lighter_lighter.r += 0.05;
            bg_lighter_lighter.g += 0.05;
            bg_lighter_lighter.b += 0.05;

            style.border.radius = 3.0.into();
            style.border.width = 1.0;
            style.border.color = bg_lighter_lighter;
            style
        })
        .padding(5)
}

fn status_to_display_string(status: &DisplayHostStatus) -> String {
    match status {
        DisplayHostStatus::Available => "Available".to_string(),
        DisplayHostStatus::InUse => "In Use".to_string(),
        DisplayHostStatus::Disconnecting => "Disconnecting".to_string(),
        DisplayHostStatus::Error(err) => format!("Error: {}", err),
        DisplayHostStatus::Initializing(phase) => {
            let phase_display_str = match phase {
                InitializationState::Unknown => "Unknown",
                InitializationState::Initializing => "Beginning initialization",
                InitializationState::InitializingTransport => "Initializing transport",
                InitializationState::GettingDisplayParameters => {
                    "Getting display parameters from client"
                }
                InitializationState::NotifyClientLoading => {
                    "Notifying client of loading virtual screen"
                }
                InitializationState::GettingScreen => "Preparing virtual screen",
                InitializationState::GettingEncoder => "Preparing encoder",
                InitializationState::NegotiatingCodecs => "Negotiating codecs with client",
                InitializationState::InitializingEncoder => "Initializing encoder",
                InitializationState::SettingClientCodec => "Setting client codec",
            };

            format!("Initializing: {}", phase_display_str)
        }
        DisplayHostStatus::Unknown => "Unknown".to_string(),
    }
}
