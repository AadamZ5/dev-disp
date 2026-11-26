use std::sync::atomic::AtomicBool;

use dev_disp_core::host::DisplayParameters;
use futures::{channel::mpsc, SinkExt};
use iced::{
    color,
    widget::{column, row},
    Element, Task,
};
use keepawake::KeepAwake;
use log::{debug, error, info, warn};

use crate::{
    renderer::{VideoData, VideoFeed},
    worker::{WorkerCommand, WorkerEvent, WorkerRenderContext},
};

pub enum WorkerConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct ScreenHostApp {
    /// The entire inner window size
    window_size: Option<iced::Size>,
    /// The size of the canvas that we will draw to
    canvas_size: Option<iced::Size>,
    /// The window ID of "this" window
    window_id: Option<iced::window::Id>,
    /// The worker connection state (connected/disconnected)
    worker_state: WorkerConnectionState,
    /// The worker render context, established during connection
    worker_render_context: Option<WorkerRenderContext>,
    /// Flag to indicate if a new frame was received into the
    /// triple-buffer, used during [ScreenHostApp::create_canvas].
    ///
    /// This is slightly different than the triple-buffer's own `updated` flag,
    /// since we can't mutate during the draw call happening in `create_canvas`.
    video_updated: AtomicBool,
    /// A handle to dispatch worker commands
    worker_command_tx: Option<mpsc::Sender<WorkerCommand>>,
    /// The URL to connect to
    connect_url: String,
    /// The [KeepAwake] guard to keep the screen on,
    /// set only when a connection is established.
    keepawake: Option<KeepAwake>,
    /// If we should fullscreen
    fullscreen: bool,
    /// If this client is selected and going to be used to display video.
    ///
    /// This is set when the worker begins initializing video parameters
    /// before video starts, and reset when the connection is finished.
    selected: bool,
}

/// Cloneable input things that represent user input events.
///
/// Iced's text box needs a cloneable message... idk why
#[derive(Debug, Clone)]
pub enum InputEvent {
    SetConnectUrl(String),
    Connect,
    Disconnect,
    SetFullScreen(bool),
}

/// An event for the [ScreenHostApp]
#[derive(Debug)]
pub enum Message {
    /// An event specifically from the worker thread
    WorkerEvent(WorkerEvent),
    /// The window ID change message, usually only occuring on
    /// window creation.
    WindowIdChange(Option<iced::window::Id>),
    /// The window size changed
    WindowSizeChange(iced::Size),
    /// Input events that require clone, moved into their own envelope.
    Input(InputEvent),
    /// Animation frame tick
    AnimationFrame,
}

impl ScreenHostApp {
    /// Creates the video rendering widget, rendering video data if any is available.
    /// Relies on [Self::video_updated] to know if we need to dispatch a redraw to the
    /// shader. Uses the [VideoFeed] shader program to render already-decoded data.
    fn create_canvas(&self, size: Option<iced::Size>) -> iced::Element<'_, Message> {
        let size = if let Some(size) = size {
            size
        } else {
            return iced::widget::space().into();
        };

        let update_available = self
            .video_updated
            .swap(false, std::sync::atomic::Ordering::Relaxed);

        let video_data = match (
            self.worker_render_context.as_ref(),
            update_available,
            &self.worker_state,
            self.selected,
        ) {
            // TODO: Restrict frame data against worker connected/in-use state!
            (Some(ctx), true, WorkerConnectionState::Connected, true) => VideoData::Frame {
                width: ctx.comitted_size.0,
                height: ctx.comitted_size.1,
                buf: ctx.buffer.output_buffer(),
            },
            (Some(_), false, WorkerConnectionState::Connected, true) => VideoData::NoUpdate,
            (Some(_), _, _, _) => VideoData::None,
            (None, _, _, _) => VideoData::None,
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
        // Window info label
        let widget_window_info = iced::widget::text(format!(
            "Window ID: {} ({}x{})",
            self.window_id
                .map_or_else(|| "?".to_string(), |id| id.to_string()),
            self.window_size.map_or_default(|s| s.width),
            self.window_size.map_or_default(|s| s.height)
        ));

        // The negotiated virtual screen size coming from the screen producer
        let casting_size_text = if let Some(size) = self.worker_render_context.as_ref() {
            format!("Screen: {}x{}", size.comitted_size.0, size.comitted_size.1)
        } else {
            "Screen: ?x?".into()
        };
        let casting_size_info = iced::widget::text(casting_size_text);

        // Connection Status
        let status_text = match &self.worker_state {
            WorkerConnectionState::Connected => "Connected",
            WorkerConnectionState::Disconnected => "Disconnected",
            WorkerConnectionState::Connecting => "Connecting",
        };
        let widget_connection_status = iced::widget::text(status_text);

        // URL input text box
        let widget_url_textbox: Element<'_, InputEvent> =
            iced::widget::text_input("URL", &self.connect_url)
                .on_input_maybe(
                    if matches!(self.worker_state, WorkerConnectionState::Disconnected) {
                        Some(InputEvent::SetConnectUrl)
                    } else {
                        None
                    },
                )
                .into();
        let widget_url_textbox = widget_url_textbox.map(Message::Input);

        // Connect/disconnect button
        let widget_connect_button: Element<'_, InputEvent> = match &self.worker_state {
            WorkerConnectionState::Connected => {
                { iced::widget::button("Disconnect").on_press(InputEvent::Disconnect) }.into()
            }
            WorkerConnectionState::Disconnected => {
                { iced::widget::button("Connect").on_press(InputEvent::Connect) }.into()
            }
            WorkerConnectionState::Connecting => {
                { iced::widget::button("Cancel").on_press(InputEvent::Disconnect) }.into()
            }
        };
        let widget_connect_button = widget_connect_button.map(Message::Input);

        // Fullscreen button
        let fullscreen_text = if self.fullscreen {
            "X Fullscreen"
        } else {
            "Fullscreen"
        };
        let widget_fullscreen_button: Element<'_, InputEvent> =
            iced::widget::button(fullscreen_text)
                .on_press(InputEvent::SetFullScreen(!self.fullscreen))
                .into();
        let widget_fullscreen_button = widget_fullscreen_button.map(Message::Input);

        let toolbar_contents = iced::widget::container(
            row![
                widget_window_info,
                casting_size_info,
                widget_connection_status,
                widget_url_textbox,
                widget_connect_button,
                widget_fullscreen_button
            ]
            .spacing(10)
            .height(30)
            .align_y(iced::alignment::Vertical::Center),
        )
        .align_left(iced::Length::Fill)
        .padding(4)
        .style(|_theme| iced::widget::container::Style::default().background(color!(50, 50, 75)));

        column![toolbar_contents, iced::widget::rule::horizontal(1),].into()
    }

    fn upsert_keepawake(&mut self) -> Result<bool, keepawake::Error> {
        if self.keepawake.is_none() {
            self.keepawake = Some(
                keepawake::Builder::default()
                    .display(true)
                    .idle(true)
                    .sleep(true)
                    .reason("vdex streaming")
                    .app_name("vdex")
                    .create()?,
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn remove_keepawake(&mut self) {
        self.keepawake = None;
    }

    fn update_window_settings(&self) -> Task<Message> {
        let window_id = if let Some(id) = &self.window_id {
            id
        } else {
            error!("Can't update window settings without ID");
            return Task::none();
        };

        let window_mode = if self.fullscreen {
            iced::window::Mode::Fullscreen
        } else {
            iced::window::Mode::Windowed
        };
        iced::window::set_mode(*window_id, window_mode)
    }
}

impl ScreenHostApp {
    pub fn new() -> (Self, Task<Message>) {
        let this = Self {
            window_size: None,
            canvas_size: None,
            window_id: None,
            worker_render_context: None,
            worker_state: WorkerConnectionState::Disconnected,
            video_updated: AtomicBool::new(false),
            worker_command_tx: None,
            connect_url: String::new(),
            keepawake: None,
            fullscreen: false,
            selected: false,
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

                self.video_updated
                    .store(video_update, std::sync::atomic::Ordering::Relaxed);
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
                InputEvent::SetFullScreen(fs) => {
                    self.fullscreen = fs;
                    return self.update_window_settings();
                }
            },
            Message::WorkerEvent(event) => match event {
                WorkerEvent::Ready(sender) => {
                    self.worker_command_tx = Some(sender);
                }
                WorkerEvent::Error(worker_error) => {
                    error!("Worker encountered an error: {:?}", worker_error);
                    self.remove_keepawake();
                    self.selected = false;
                }
                WorkerEvent::Connecting(url) => {
                    info!("Worker is connecting to {}", url);
                    self.worker_render_context = None;
                    self.selected = false;
                    self.worker_state = WorkerConnectionState::Connecting;
                    match self.upsert_keepawake() {
                        Ok(_) => {}
                        Err(e) => error!("Failed to install keep-awake: {:?}", e),
                    }
                }
                WorkerEvent::Connected(url) => {
                    info!("Worker connected to {}", url);
                    self.worker_state = WorkerConnectionState::Connected;
                    match self.upsert_keepawake() {
                        Ok(_) => {}
                        Err(e) => error!("Failed to install keep-awake: {:?}", e),
                    }
                }
                WorkerEvent::PrepareRender(render_context) => {
                    info!(
                        "Preparing render with context {:?}",
                        render_context.comitted_size
                    );
                    self.worker_render_context = Some(render_context);
                }
                WorkerEvent::Selected => {
                    self.selected = true;
                }
                WorkerEvent::Disconnected => {
                    self.worker_state = WorkerConnectionState::Disconnected;
                    self.remove_keepawake();
                    self.selected = false;
                    self.worker_render_context = None;
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
