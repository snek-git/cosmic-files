// SPDX-License-Identifier: GPL-3.0-only

//! Every `yorha` primitive on the palette ground, the ornament first and the grid last.

use cosmic::app::{self, Core, Settings, Task};
use cosmic::iced::widget::{column, container, row, stack};
use cosmic::iced::{Background, Length, Size};
use cosmic::{Application, Element, executor, theme};
use cosmic_files::config::Config;
use cosmic_files::yorha::{self, Tone};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // the window's type face, as the fork sets it
    cosmic::config::COSMIC_TK
        .write()
        .unwrap()
        .interface_font
        .family = "Monaco".to_string();
    // building the palette theme is what fills `palette::ladder()`
    let config = Config::default();
    let theme = config.app_theme.theme(&config.palette);
    // YORHA_NOAA=1 turns MSAA off: with it on, a full size window draws the stack's layers out
    // of order in iced_wgpu (the ornament over the panels) and drops the middle layer's meshes
    let aa = std::env::var_os("YORHA_NOAA").is_none();
    let settings = Settings::default()
        .size(Size::new(1200.0, 750.0))
        .antialiasing(aa);
    app::run::<App>(settings, theme)?;
    Ok(())
}

pub struct App {
    core: Core,
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = theme::Theme;
    type Message = ();

    const APP_ID: &'static str = "com.system76.CosmicFilesYorhaDemo";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, theme: Self::Flags) -> (Self, Task<()>) {
        (Self { core }, cosmic::command::set_theme(theme))
    }

    fn view(&self) -> Element<'_, ()> {
        // a panel body: the theme's primary container, the palette's panel tone
        let panel = |body: Element<'static, ()>| {
            container(body)
                .padding(16)
                .class(theme::Container::Primary)
        };
        // the one dark, for light ink
        let slab = |body: Element<'static, ()>| {
            container(body)
                .padding([4, 8])
                .class(theme::Container::custom(|t| container::Style {
                    background: Some(Background::Color(t.cosmic().accent_color().into())),
                    ..Default::default()
                }))
        };

        let tones = row![
            yorha::micro("ink", Tone::Ink),
            yorha::micro("secondary", Tone::Secondary),
            yorha::micro("disabled", Tone::Disabled),
            yorha::micro("urgent", Tone::Urgent),
            slab(yorha::micro("light", Tone::Light)),
        ]
        .spacing(24)
        .align_y(cosmic::iced::Alignment::Center);

        // the lozenge lives 32px left of the row it marks, outside the panel
        let marked = |ghost: bool, label: &str| {
            row![
                yorha::cursor_lozenge(ghost),
                cosmic::iced::widget::Space::new().width(11),
                panel(yorha::caps(label, 12.0, 0.03, Tone::Ink).into()),
            ]
            .align_y(cosmic::iced::Alignment::Center)
        };

        let body = column![
            yorha::dot_rail(true),
            row![
                yorha::title_lockup("Files", "Ariadne file manager"),
                yorha::title("Board"),
            ]
            .spacing(48),
            tones,
            yorha::caps("Mixed case, tracked 0.03em", 12.0, 0.03, Tone::Ink),
            yorha::caps("WIDE CAPS 0.30", 12.0, 0.30, Tone::Secondary),
            marked(false, "focused list: solid lozenge"),
            marked(true, "focus elsewhere: ghost lozenge"),
            row![
                yorha::cursor_lozenge(false),
                yorha::cursor_lozenge(true),
                panel(row![yorha::cursor_lozenge(false), yorha::cursor_lozenge(true)].spacing(8).into()),
            ]
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center),
            cosmic::iced::widget::Space::new().height(Length::Fill),
            yorha::dot_rail(false),
        ]
        .spacing(24)
        .padding([16, 32]);

        stack![
            yorha::diagonal_ornament(),
            container(body).width(Length::Fill).height(Length::Fill),
            yorha::grid_texture(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn update(&mut self, _message: ()) -> Task<()> {
        Task::none()
    }
}
