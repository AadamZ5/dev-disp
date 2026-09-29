use dev_disp_core::host::DisplayParameters;
use futures::{channel::mpsc, SinkExt};
use iced::{
    color,
    widget::{column, row},
    window::Id,
    Border, Element, Task,
};
use log::{debug, error, info};
use triple_buffer::{Input, Output};

use crate::worker::{WorkerCommand, WorkerEvent};

pub struct ScreenHostApp {
    producer_endpoint: String,
    window_size: Option<iced::Size>,
    canvas_size: Option<iced::Size>,
    window_id: Option<iced::window::Id>,
    display_buffer: Option<iced::widget::image::Handle>,
    worker_buffer: Option<Output<Vec<u8>>>,
    worker_command_tx: Option<mpsc::Sender<WorkerCommand>>,
    connect_url: String,
}

/// Cloneable input things that represent user input events.
///
/// Iced's text box needs a cloneable message... idk why
#[derive(Debug, Clone)]
pub enum InputEvent {
    SetConnectUrl(String),
    Connect,
}

#[derive(Debug)]
pub enum Message {
    WorkerEvent(WorkerEvent),

    ChangeCanvasSize(iced::Size),
    WindowIdChange(Option<iced::window::Id>),
    WindowSizeChange(iced::Size),

    Input(InputEvent),
}

impl ScreenHostApp {
    fn create_canvas(&self, size: Option<iced::Size>) -> iced::Element<'_, Message> {
        let size = if let Some(size) = size {
            size
        } else {
            return iced::widget::space().into();
        };

        if let Some(display_buffer) = &self.display_buffer {
            return iced::widget::Container::new(
                iced::widget::image(display_buffer)
                    .width(size.width)
                    .height(size.height)
                    .content_fit(iced::ContentFit::Contain),
            )
            .center(iced::Length::Shrink)
            .into();
        } else {
            return iced::widget::space().into();
        }
    }

    fn create_toolbar(&self) -> iced::Element<'_, Message> {
        let widget_window_info = iced::widget::text(format!(
            "Window ID: {} ({}x{})",
            self.window_id
                .map_or_else(|| "?".to_string(), |id| id.to_string()),
            self.window_size.map_or_default(|s| s.width),
            self.window_size.map_or_default(|s| s.height)
        ));

        let widget_url_textbox: Element<'_, InputEvent> =
            iced::widget::text_input("URL", &self.connect_url)
                .on_input(|value| InputEvent::SetConnectUrl(value))
                .into();
        let widget_url_textbox = widget_url_textbox.map(Message::Input);

        let widget_connect_button: Element<'_, InputEvent> = iced::widget::button("Connect")
            .on_press(InputEvent::Connect)
            .into();
        let widget_connect_button = widget_connect_button.map(Message::Input);

        let toolbar_contents = iced::widget::container(
            row![
                widget_window_info,
                widget_url_textbox,
                widget_connect_button
            ]
            .spacing(10)
            .height(25)
            .align_y(iced::alignment::Vertical::Center),
        )
        .align_left(iced::Length::Fill)
        .padding(4)
        .style(|_theme| iced::widget::container::Style::default().background(color!(50, 50, 75)));

        column![toolbar_contents, iced::widget::rule::horizontal(1),].into()
    }
}

impl ScreenHostApp {
    pub fn new(producer_endpoint: String) -> (Self, Task<Message>) {
        let red = [255, 0, 0, 255];
        let mut backing_buffer = Vec::new();
        for _ in 0..(100 * 100) {
            backing_buffer.extend_from_slice(&red);
        }

        let this = Self {
            producer_endpoint,
            window_size: None,
            canvas_size: None,
            window_id: None,
            display_buffer: None,
            worker_buffer: None,
            worker_command_tx: None,
            connect_url: String::new(),
        };

        let get_window_id = iced::window::latest().map(|id| Message::WindowIdChange(id));

        (this, get_window_id)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowIdChange(window_id) => {
                self.window_id = window_id;
                if let Some(window_id) = &self.window_id {
                    return iced::window::size(*window_id).map(Message::WindowSizeChange);
                } else {
                    return Task::none();
                }
            }
            Message::ChangeCanvasSize(size) => {
                self.canvas_size = Some(size);
            }
            Message::WindowSizeChange(size) => {
                self.window_size = Some(size);

                // Canvas only implicitly sized on first window sizing
                if self.canvas_size.is_none() {
                    // In the future, this may be an async operation?
                    return Task::done(Message::ChangeCanvasSize(size));
                }
            }

            Message::Input(event) => match event {
                InputEvent::SetConnectUrl(url) => self.connect_url = url,
                InputEvent::Connect => {
                    if let Some(worker_command_tx) = self.worker_command_tx.as_mut() {
                        let mut worker_command_tx = worker_command_tx.clone();
                        let connect_url = self.connect_url.clone();
                        return Task::future(async move {
                            match worker_command_tx
                                .send(WorkerCommand::Connect(connect_url))
                                .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("Failed to send connect command to worker: {:?}", e);
                                }
                            }
                        })
                        .discard();
                    } else {
                        error!("Worker command channel is not available");
                    }
                }
            },
            Message::WorkerEvent(event) => match event {
                WorkerEvent::Ready(sender) => {
                    self.worker_command_tx = Some(sender);
                }
                WorkerEvent::Error(worker_error) => {
                    error!("Worker encountered an error: {:?}", worker_error);
                }
                WorkerEvent::Connecting(url) => {
                    info!("Worker is connecting to {}", url);
                }
                WorkerEvent::Connected(url) => {
                    info!("Worker connected to {}", url);
                }
                WorkerEvent::PrepareBuffer(output) => {
                    self.worker_buffer = Some(output);
                }
                WorkerEvent::Disconnected => {
                    // Idk
                }
                WorkerEvent::RequestDisplayParameters => {
                    if let Some(worker_command_tx) = self.worker_command_tx.as_mut() {
                        // Send display parameters to the worker

                        let mut worker_command_tx = worker_command_tx.clone();
                        let display_parameters = DisplayParameters {
                            host_dev_name: "Desktop Client".to_string(),
                            resolution: self
                                .window_size
                                .map(|size| unsafe {
                                    (
                                        size.width.to_int_unchecked(),
                                        size.height.to_int_unchecked(),
                                    )
                                })
                                .unwrap_or((800, 600)),
                        };

                        return Task::future(async move {
                            worker_command_tx
                                .send(WorkerCommand::SetDisplayParameters(display_parameters))
                                .await
                                .unwrap_or_else(|err| {
                                    error!("Failed to send display parameters: {:?}", err);
                                });
                        })
                        .discard();
                    }
                }
            },
        }

        Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        // Implement the view logic here

        column![self.create_toolbar(), self.create_canvas(self.canvas_size),].into()
    }
}
