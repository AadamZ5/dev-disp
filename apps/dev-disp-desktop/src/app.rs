use iced::{
    color,
    widget::{column, row},
    window::Id,
    Border, Task,
};
use log::debug;

use crate::worker::WorkerEvent;

pub struct ScreenHostApp {
    producer_endpoint: String,
    window_size: Option<iced::Size>,
    canvas_size: Option<iced::Size>,
    window_id: Option<iced::window::Id>,
    display_buffer: Option<iced::widget::image::Handle>,
    _backing_buffer: Vec<u8>,
}

#[derive(Debug)]
pub enum Message {
    WorkerEvent(WorkerEvent),

    ChangeCanvasSize(iced::Size),
    WindowIdChange(Option<iced::window::Id>),
    WindowSizeChange(iced::Size),
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
        let toolbar_contents = iced::widget::container(
            row![iced::widget::text(format!(
                "Window ID: {} ({}x{})",
                self.window_id
                    .map_or_else(|| "?".to_string(), |id| id.to_string()),
                self.window_size.map_or_default(|s| s.width),
                self.window_size.map_or_default(|s| s.height)
            ))]
            .spacing(10)
            .height(20)
            .align_y(iced::alignment::Vertical::Center),
        )
        .align_left(iced::Length::Fill)
        .padding(10)
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
            _backing_buffer: backing_buffer,
        };

        let get_window_id = iced::window::latest().map(|id| Message::WindowIdChange(id));

        (this, get_window_id)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if self.display_buffer.is_none() {
            self.display_buffer = Some(iced::widget::image::Handle::from_rgba(
                100,
                100,
                self._backing_buffer.clone(),
            ));
        }

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
            Message::WorkerEvent(event) => {
                debug!("App received worker event: {:?}", event);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        // Implement the view logic here

        column![self.create_toolbar(), self.create_canvas(self.canvas_size),].into()
    }
}
