use iced::{
    Element, Font, font,
    widget::{
        Row, rich_text, span,
        text::{IntoFragment, Span},
    },
};

use crate::application::UiAction;

pub fn label<'a, T, C>(label: T, content: C) -> Row<'a, UiAction>
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
