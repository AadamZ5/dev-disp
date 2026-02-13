use dev_disp_core::daemon::api::DisplayHostRef;
use futures::StreamExt;
use iced::{
    Task,
    widget::{Column, Container, text},
};

use crate::{
    backend::{self, ApiFactory, BackendRef},
    widgets::simple_device_info,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting(String),
    Connected(String),
}

#[derive(Debug)]
pub struct DevDispApplication<T>
where
    T: ApiFactory,
{
    backend_ref: BackendRef<T::ConnectParam>,
    connection_state: ConnectionState,
    available_devices: Vec<DisplayHostRef>,
    connected_devices: Vec<DisplayHostRef>,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    BackendEvent(backend::BackendEvent),
    BackendCommand(backend::BackendCommand),
}

impl<T> DevDispApplication<T>
where
    T: ApiFactory + 'static + Send,
    T::Api: 'static + Send,
{
    pub fn new(
        api_factory: T,
        initial_connect_param: Option<T::ConnectParam>,
    ) -> (Self, Task<UiAction>) {
        let mut tasks = Vec::new();

        let (backend_ref, backend_event_stream) = backend::run_backend(api_factory);
        tasks.push(Task::stream(
            backend_event_stream.map(UiAction::BackendEvent),
        ));

        if let Some(connect_param) = initial_connect_param {
            let backend_ref = backend_ref.clone();
            tasks.push(
                Task::future(async move {
                    backend_ref.connect(connect_param).await;
                })
                .discard(),
            );
        }

        let this = Self {
            backend_ref,
            connection_state: ConnectionState::Disconnected,
            available_devices: Vec::new(),
            connected_devices: Vec::new(),
        };

        (this, Task::batch(tasks))
    }

    pub fn view(&self) -> Column<UiAction> {
        let connected_text = text(match &self.connection_state {
            ConnectionState::Disconnected => "Disconnected".to_string(),
            ConnectionState::Connecting(addr) => format!("Connecting to {}...", addr),
            ConnectionState::Connected(addr) => format!("Connected to {}", addr),
        });

        let c = Column::new()
            .push(connected_text)
            .padding(20)
            .spacing(10)
            .push(text("Available Devices:").size(24));

        let mut available = Column::new().padding(10).spacing(10);

        for device in &self.available_devices {
            available = available.push(Container::new(simple_device_info(device, false)))
        }

        if self.available_devices.is_empty() {
            available = available.push(text("No available devices found."));
        }

        let available_scroll = iced::widget::scrollable(available)
            .height(iced::Length::FillPortion(1))
            .width(iced::Length::Fill);

        let c = c
            .push(available_scroll)
            .push(text("Connected Devices:").size(24));

        let mut connected = Column::new().padding(10).spacing(10);

        for device in &self.connected_devices {
            connected = connected.push(Container::new(simple_device_info(device, true)))
        }

        if self.connected_devices.is_empty() {
            connected = connected.push(text("No connected devices found."));
        }

        let connected_scroll = iced::widget::scrollable(connected)
            .height(iced::Length::FillPortion(1))
            .width(iced::Length::Fill);

        c.push(connected_scroll)
    }

    pub fn update(&mut self, action: UiAction) -> Task<UiAction> {
        match action {
            UiAction::BackendEvent(e) => match e {
                backend::BackendEvent::Connected(endpoint_display) => {
                    self.connection_state = ConnectionState::Connected(endpoint_display);
                    Task::done(UiAction::BackendCommand(
                        backend::BackendCommand::StreamDevices,
                    ))
                }
                backend::BackendEvent::Disconnected => {
                    self.connection_state = ConnectionState::Disconnected;
                    self.available_devices.clear();
                    self.connected_devices.clear();
                    Task::none()
                }
                backend::BackendEvent::DeviceListUpdated(device_collection_status) => {
                    self.available_devices = device_collection_status.connectable_devices;
                    self.connected_devices = device_collection_status.in_use_devices;
                    Task::none()
                }
            },
            UiAction::BackendCommand(cmd) => Task::future(self.backend_ref.send(cmd)).discard(),
        }
    }
}
