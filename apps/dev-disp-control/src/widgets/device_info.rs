use dev_disp_core::daemon::api::{DisplayHostRef, DisplayHostStatus};
use iced::{
    Theme,
    widget::{Column, Container, button, container, text},
};

use crate::{
    application::UiAction,
    backend::BackendCommand,
    util::status_to_display_string,
    widgets::{code_text, label},
};

pub fn simple_device_info(device: &DisplayHostRef, connected: bool) -> Container<'_, UiAction> {
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
                    disconnect_button = disconnect_button.on_press(UiAction::BackendCommand(
                        BackendCommand::DisconnectDevice(
                            device.id.clone(),
                            device.discovery_id.clone(),
                        ),
                    ));
                };
                disconnect_button
            } else {
                let mut connect_button = button("Connect");
                if device.status == DisplayHostStatus::Available {
                    connect_button = connect_button.on_press(UiAction::BackendCommand(
                        BackendCommand::InitializeDevice(
                            device.id.clone(),
                            device.discovery_id.clone(),
                        ),
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
