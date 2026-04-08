use iced::widget::{button, container, text_input};
use iced::Color;

use crate::app::Launcher;
use crate::config::{get, parse_color};

pub fn container_style(_theme: &iced::Theme) -> container::Style {
    let s = &get().style;
    container::Style {
        background: Some(iced::Background::Color(parse_color(&s.container_background))),
        border: iced::Border {
            color: parse_color(&s.container_border),
            width: 1.0,
            radius: iced::border::Radius {
                top_left: s.container_radius,
                top_right: s.container_radius,
                bottom_left: s.container_radius,
                bottom_right: s.container_radius,
            },
        },
        ..Default::default()
    }
}

pub fn input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let s = &get().style;
    text_input::Style {
        background: iced::Background::Color(parse_color(&s.input_background)),
        border: iced::Border {
            color: match status {
                text_input::Status::Focused { .. } => parse_color(&s.input_border_focused),
                text_input::Status::Hovered => parse_color(&s.input_border_hover),
                _ => parse_color(&s.input_border_idle),
            },
            width: 1.0,
            radius: s.input_radius.into(),
        },
        value: parse_color(&s.text_color),
        placeholder: parse_color(&s.placeholder_color),
        selection: parse_color(&s.input_border_focused),
        icon: parse_color(&s.text_color),
    }
}

pub fn button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let s = &get().style;
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Hovered => parse_color(&s.button_hover),
            button::Status::Pressed => parse_color(&s.button_pressed),
            _ => parse_color(&s.button_background),
        })),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: s.button_radius.into(),
        },
        text_color: parse_color(&s.text_color),
        ..Default::default()
    }
}

pub fn button_style_selected(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let s = &get().style;
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Pressed => parse_color(&s.button_selected_hover),
            _ => parse_color(&s.button_selected_background),
        })),
        border: iced::Border {
            color: parse_color(&s.button_selected_border),
            width: 1.0,
            radius: s.button_radius.into(),
        },
        text_color: parse_color(&s.text_color),
        ..Default::default()
    }
}

pub fn window_style(_: &Launcher, _theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: parse_color(&get().style.text_color),
    }
}
