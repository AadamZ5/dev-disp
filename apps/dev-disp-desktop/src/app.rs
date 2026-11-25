use std::sync::atomic::AtomicBool;

use dev_disp_core::host::DisplayParameters;
use futures::{channel::mpsc, SinkExt};
use iced::{
    color,
    widget::shader,
    widget::{column, row},
    Element, Task,
};
use log::{debug, error, info, warn};

use crate::{
    renderer::{VideoData, VideoFeed},
    worker::{Worker, WorkerCommand, WorkerEvent, WorkerRenderContext},
};

pub enum WorkerConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct ScreenHostApp {
    producer_endpoint: String,
    window_size: Option<iced::Size>,
    canvas_size: Option<iced::Size>,
    window_id: Option<iced::window::Id>,
    image_handle: Option<iced::widget::image::Handle>,
    worker_state: WorkerConnectionState,
    worker_render_context: Option<WorkerRenderContext>,
    video_update: AtomicBool,
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
    Disconnect,
}

#[derive(Debug)]
pub enum Message {
    WorkerEvent(WorkerEvent),

    ChangeCanvasSize(iced::Size),
    WindowIdChange(Option<iced::window::Id>),
    WindowSizeChange(iced::Size),

    Input(InputEvent),

    AnimationFrame,
}

impl ScreenHostApp {
    fn create_canvas(&self, size: Option<iced::Size>) -> iced::Element<'_, Message> {
        let size = if let Some(size) = size {
            size
        } else {
            return iced::widget::space().into();
        };

        let update_available = self
            .video_update
            .swap(false, std::sync::atomic::Ordering::Relaxed);

        let video_data = match (
            self.worker_render_context.as_ref(),
            update_available,
            &self.worker_state,
        ) {
            // TODO: Restrict frame data against worker connected/in-use state!
            (Some(ctx), true, _) => VideoData::Frame {
                width: ctx.comitted_size.0,
                height: ctx.comitted_size.1,
                buf: ctx.buffer.output_buffer(),
            },
            (Some(_), false, WorkerConnectionState::Connected) => VideoData::NoUpdate,
            (Some(_), _, _) => VideoData::NoUpdate,
            (None, _, _) => VideoData::None,
        };

        return iced::widget::Container::new(
            iced::widget::shader(VideoFeed::new(video_data))
                .width(size.width)
                .height(size.height),
        )
        .center(iced::Length::Shrink)
        .into();
    }

    fn create_toolbar(&self) -> iced::Element<'_, Message> {
        let widget_window_info = iced::widget::text(format!(
            "Window ID: {} ({}x{})",
            self.window_id
                .map_or_else(|| "?".to_string(), |id| id.to_string()),
            self.window_size.map_or_default(|s| s.width),
            self.window_size.map_or_default(|s| s.height)
        ));

        // Show the worker's dispatched size info if we have it
        let casting_size_info = if let Some(size) = self.worker_render_context.as_ref() {
            iced::widget::text(format!(
                "Canvas: {}x{}",
                size.comitted_size.0, size.comitted_size.1
            ))
        } else {
            iced::widget::text("Canvas: ?x?")
        };

        let widget_url_textbox: Element<'_, InputEvent> =
            iced::widget::text_input("URL", &self.connect_url)
                .on_input(|value| InputEvent::SetConnectUrl(value))
                .into();
        let widget_url_textbox = widget_url_textbox.map(Message::Input);

        let widget_connect_button: Element<'_, InputEvent> = match &self.worker_state {
            WorkerConnectionState::Connected => {
                { iced::widget::button("Disconnect").on_press(InputEvent::Disconnect) }.into()
            }
            WorkerConnectionState::Disconnected => {
                { iced::widget::button("Connect").on_press(InputEvent::Connect) }.into()
            }
            WorkerConnectionState::Connecting => { iced::widget::button("Connecting") }.into(),
        };
        let widget_connect_button = widget_connect_button.map(Message::Input);

        let toolbar_contents = iced::widget::container(
            row![
                widget_window_info,
                casting_size_info,
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
        let this = Self {
            producer_endpoint,
            window_size: None,
            canvas_size: None,
            window_id: None,
            image_handle: None,
            worker_render_context: None,
            worker_state: WorkerConnectionState::Disconnected,
            video_update: AtomicBool::new(false),
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
            Message::AnimationFrame => {
                // Handle animation frame updates here if needed
                let video_update = if let Some(ctx) = self.worker_render_context.as_mut() {
                    ctx.buffer.update()
                } else {
                    false
                };

                self.video_update
                    .store(video_update, std::sync::atomic::Ordering::Relaxed);
            }
            Message::ChangeCanvasSize(size) => {
                self.canvas_size = Some(size);
            }
            Message::WindowSizeChange(size) => {
                self.window_size = Some(size);
                self.canvas_size = Some(size);
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
                InputEvent::Disconnect => {
                    if let Some(worker_command_tx) = self.worker_command_tx.as_mut() {
                        let mut worker_command_tx = worker_command_tx.clone();
                        return Task::future(async move {
                            let _ = worker_command_tx.send(WorkerCommand::Disconnect).await;
                        })
                        .discard();
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
                    self.worker_state = WorkerConnectionState::Connecting;
                }
                WorkerEvent::Connected(url) => {
                    info!("Worker connected to {}", url);
                    self.worker_state = WorkerConnectionState::Connected;
                }
                WorkerEvent::PrepareRender(render_context) => {
                    info!(
                        "Preparing render with context {:?}",
                        render_context.comitted_size
                    );
                    self.worker_render_context = Some(render_context);
                }
                WorkerEvent::Disconnected => {
                    self.worker_state = WorkerConnectionState::Disconnected;
                }
                WorkerEvent::RequestDisplayParameters => {
                    debug!("Worker requested display parameters");
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
                            debug!(
                                "Sending display parameters to worker: {:?}",
                                display_parameters
                            );

                            worker_command_tx
                                .send(WorkerCommand::SetDisplayParameters(display_parameters))
                                .await
                                .unwrap_or_else(|err| {
                                    error!("Failed to send display parameters: {:?}", err);
                                });
                        })
                        .discard();
                    } else {
                        warn!("Worker requested display parameters, but the command channel is not available to send them!");
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
