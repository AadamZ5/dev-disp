use iced::{
    Theme,
    widget::{Container, container, text, text::IntoFragment},
};

use crate::application::UiAction;

pub fn code_text<'a, T>(content: T) -> Container<'a, UiAction>
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
