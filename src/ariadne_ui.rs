//! What ariadne draws inside a folder view: the pill on a file it has something to say about,
//! the card that opens from it, and the ghost a file leaves where it was moved from.

use std::path::Path;

use cosmic::iced::{Alignment, Background, Border, Length, Shadow, Vector};
use cosmic::iced::widget::container;
use cosmic::{Element, theme, widget};

use crate::ariadne::{InboxEntry, InboxVerb, Mark, Move, Proposal, folder_name};
use crate::palette::ladder;
use crate::tab::Message;

pub fn ago(at: f64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let min = ((now - at) / 60.0).max(0.0).round() as u64;
    match min {
        0 => "just now".to_string(),
        1..=59 => format!("{min} min ago"),
        _ => format!("{} h ago", min / 60),
    }
}

/// Short enough for a pill or a tile: long names end in an ellipsis.
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>().trim_end())
    }
}

fn pct(share: Option<f32>) -> String {
    share.map(|s| format!("{:.0}%", s * 100.0)).unwrap_or_default()
}

/// A small plate: `shadow` at rest with an ink border, or `urgent` for new things.
fn plate_style(urgent: bool) -> theme::Button {
    let look = move |_focused: bool, theme: &theme::Theme| {
        let cosmic = theme.cosmic();
        let mut s = widget::button::Style::new();
        match ladder() {
            Some(t) if urgent => {
                s.background = Some(Background::Color(t.urgent()));
                s.text_color = Some(t.light());
                s.icon_color = Some(t.light());
            }
            Some(t) => {
                s.background = Some(Background::Color(t.base_dim()));
                s.text_color = Some(t.text());
                s.icon_color = Some(t.text());
            }
            None => {
                s.background = Some(Background::Color(cosmic.accent_color().into()));
                s.text_color = Some(cosmic.on_accent_color().into());
            }
        }
        s.border_radius = 0.0.into();
        s
    };
    theme::Button::Custom {
        active: Box::new(look),
        disabled: Box::new(move |theme| look(false, theme)),
        hovered: Box::new(look),
        pressed: Box::new(look),
    }
}

fn card_style() -> theme::Container<'static> {
    theme::Container::custom(|theme| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.highlight())),
            text_color: Some(t.text()),
            border: Border {
                color: t.text(),
                width: 1.0,
                radius: 0.0.into(),
            },
            shadow: Shadow {
                color: t.shadow(),
                offset: Vector::new(2.0, 2.0),
                blur_radius: 0.0,
            },
            ..Default::default()
        },
        None => container::Style {
            background: Some(Background::Color(theme.cosmic().primary_container_color().into())),
            border: Border {
                color: theme.cosmic().primary_container_divider().into(),
                width: 1.0,
                radius: theme.cosmic().radius_s().into(),
            },
            ..Default::default()
        },
    })
}

/// Text in the instrument register under the palette, a caption without it.
pub fn small_text<'a, M: 'a>(text: &str, tone: crate::yorha::Tone) -> Element<'a, M> {
    if ladder().is_some() {
        crate::yorha::micro(text, tone)
    } else {
        widget::text::caption(text.to_string()).into()
    }
}

/// The pill on a file, one line of micro caps on a plate. Pressing it opens the card.
pub fn pill<'a>(mark: &Mark, path: &Path) -> Element<'a, Message> {
    use crate::yorha::Tone;
    let open = Message::AriadneCard(Some(path.to_path_buf()));
    let (label, urgent) = match mark {
        Mark::Reading => ("reading ▮".to_string(), false),
        Mark::Waiting(e) => (
            format!("→ {} {}", clip(&folder_name(&e.folder), 7), pct(e.share)),
            false,
        ),
        Mark::Copy(_) => ("copy".to_string(), false),
        Mark::New(_) => ("new".to_string(), true),
    };
    let tone = if urgent { Tone::Light } else { Tone::Ink };
    let button = widget::button::custom(small_text(&label, tone))
        .padding([3, 6])
        .class(plate_style(urgent));
    match mark {
        Mark::Reading => button.into(),
        _ => button.on_press(open).into(),
    }
}

/// The count on a folder tile. `waiting`: files that wait inside it, a solid plate in the one
/// dark (dark means engaged). Otherwise its Tidy ideas, an ink outline on the panel tone.
pub fn count<'a>(n: usize, waiting: bool) -> Element<'a, Message> {
    use crate::yorha::Tone;
    let tone = if waiting { Tone::Light } else { Tone::Ink };
    widget::container(small_text(&format!("{n:02}"), tone))
        .padding([2, 5])
        .class(theme::Container::custom(move |theme| match ladder() {
            Some(t) if waiting => container::Style {
                background: Some(Background::Color(t.accent())),
                ..Default::default()
            },
            Some(t) => container::Style {
                background: Some(Background::Color(t.highlight())),
                border: Border {
                    color: t.text(),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            },
            None => container::Style {
                background: Some(Background::Color(theme.cosmic().accent_color().into())),
                text_color: Some(theme.cosmic().on_accent_color().into()),
                border: Border {
                    radius: theme.cosmic().radius_xl().into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        }))
        .into()
}

/// A path as the window says it: under home it starts with `~`.
pub fn short(path: &Path) -> String {
    match path.strip_prefix(crate::home_dir()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// A popup sheet, as the panels behind it: the header slab, then the body on the panel tone.
/// It floats over the grid, so it also gets a 1px ink frame and the 2px drop shadow.
pub fn sheet<'a, M: 'a>(label: &str, value: String, body: Vec<Element<'a, M>>) -> Element<'a, M> {
    widget::container(widget::column::with_children([
        header(label, value),
        widget::container(widget::column::with_children(body).spacing(8))
            .padding([10, 12, 12, 12])
            .into(),
    ]))
    .width(Length::Fixed(SHEET_WIDTH))
    .class(card_style())
    .into()
}

/// Wide enough for a 30 character name in the body register.
const SHEET_WIDTH: f32 = 320.0;

/// A file's name as the first line of a sheet, cut to the sheet's width.
pub fn name_line<'a, M: 'a>(path: &Path) -> Element<'a, M> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    crate::palette::body(clip(&name, 34)).into()
}

/// The buttons at the foot of a sheet.
pub fn actions<'a, M: 'a>(buttons: Vec<Element<'a, M>>) -> Element<'a, M> {
    widget::row::with_children(buttons)
        .spacing(8)
        .padding([4, 0, 0, 0])
        .into()
}

/// A folder to pick on a sheet: a plate row with the name left and the share right.
fn choice<'a>(folder: &Path, share: f32, press: Message) -> Element<'a, Message> {
    widget::button::custom(
        widget::row::with_children([
            crate::palette::body(clip(&folder_name(folder), 24)).into(),
            widget::space::horizontal().into(),
            crate::palette::body(pct(Some(share)))
                .class(crate::palette::secondary())
                .into(),
        ])
        .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .on_press(press)
    .class(crate::palette::plate_button())
    .into()
}

/// The card for a file ariadne has something to say about.
pub fn card<'a>(mark: &Mark, path: &Path, more: bool) -> Element<'a, Message> {
    let verb = |v| Message::Inbox(path.to_path_buf(), v);
    let mut body = vec![name_line(path)];
    let (label, when) = match mark {
        Mark::Reading => ("reading", String::new()),
        Mark::Waiting(e) => ("arrival", ago(e.at)),
        Mark::Copy(e) => ("copy", ago(e.at)),
        Mark::New(m) => ("new", ago(m.at)),
    };
    match mark {
        Mark::Reading => {}
        Mark::Waiting(e) => {
            body.push(section("move to"));
            body.push(field("Folder", clip(&folder_name(&e.folder), 24)));
            body.push(field("Where", clip(&short(e.folder.parent().unwrap_or(&e.folder)), 24)));
            body.push(field("Sure", format!("{} {}", pct(e.share), by(e))));
            if let Some(s) = &e.source {
                body.push(field("From", clip(s, 24)));
            }
            body.push(actions(vec![
                crate::palette::act("Move").on_press(verb(InboxVerb::Move)).into(),
                crate::palette::quiet("Keep").on_press(verb(InboxVerb::Keep)).into(),
                crate::palette::quiet(if more { "Less" } else { "More" })
                    .on_press(Message::AriadneMore)
                    .into(),
            ]));
            if more && !e.alternatives.is_empty() {
                body.push(section("or"));
                for alt in &e.alternatives {
                    body.push(choice(
                        &alt.folder,
                        alt.share,
                        Message::AriadneMoveTo(path.to_path_buf(), alt.folder.clone()),
                    ));
                }
            }
        }
        Mark::Copy(e) => {
            let twin = e.dup_of.clone().unwrap_or_default();
            let home = twin.parent().map(Path::to_path_buf).unwrap_or_default();
            body.push(section("already filed"));
            body.push(field("Folder", clip(&folder_name(&home), 24)));
            body.push(field("Where", clip(&short(home.parent().unwrap_or(&home)), 24)));
            if let Some(s) = &e.source {
                body.push(field("From", clip(s, 24)));
            }
            body.push(actions(vec![
                crate::palette::act("Drop copy")
                    .on_press(Message::AriadneDropCopy(path.to_path_buf()))
                    .into(),
                crate::palette::quiet("Keep").on_press(verb(InboxVerb::Keep)).into(),
            ]));
        }
        Mark::New(m) => {
            let from = m.from.parent().map(Path::to_path_buf).unwrap_or_default();
            body.push(field("From", clip(&short(&from), 24)));
            body.push(field("Sure", pct(m.share)));
            body.push(actions(vec![
                crate::palette::act("OK")
                    .on_press(Message::AriadneSeen(path.to_path_buf()))
                    .into(),
                crate::palette::quiet("Undo").on_press(verb(InboxVerb::Undo)).into(),
            ]));
        }
    }
    sheet(label, when, body)
}

/// "ariadne noticed": one sure idea about this folder, at the bottom of the view.
pub fn noticed<'a>(p: &Proposal, name: &str) -> Element<'a, Message> {
    let mark: Element<'a, Message> = if ladder().is_some() {
        widget::container(crate::palette::body("NOTICED"))
            .padding([3, 8])
            .class(crate::palette::slab())
            .into()
    } else {
        widget::icon::from_name("view-reveal-symbolic").size(16).into()
    };
    let mut row = widget::row::with_capacity(6)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(mark)
        .push(crate::palette::body(p.noticed()).width(Length::Fill));
    if p.kind == "group" {
        row = row.push(
            widget::text_input("", name.to_string())
                .on_input(Message::NoticeName)
                .width(Length::Fixed(160.0)),
        );
    }
    row = row
        .push(crate::palette::act(p.action()).on_press(Message::NoticeApply))
        .push(crate::palette::quiet("Not now").on_press(Message::NoticeLater))
        .push(crate::palette::quiet("Never").on_press(Message::NoticeNever));
    widget::container(row)
        .padding([6, 12, 6, 6])
        .width(Length::Fill)
        .class(card_style())
        .into()
}

/// Under the items when several files here wait for a yes: their count, Move all, Keep all,
/// and Next, which puts the cursor on the next one (Alt+N).
pub fn waiting_bar<'a>(n: usize) -> Element<'a, Message> {
    let mark: Element<'a, Message> = widget::container(crate::palette::body(format!("WAITING  {n:02}")))
        .padding([3, 8])
        .class(crate::palette::slab())
        .into();
    widget::container(
        widget::row::with_children([
            mark,
            widget::space::horizontal().into(),
            crate::palette::act("Move all")
                .on_press(Message::AriadneAll(InboxVerb::Move))
                .into(),
            crate::palette::quiet("Keep all")
                .on_press(Message::AriadneAll(InboxVerb::Keep))
                .into(),
            crate::palette::quiet("Next").on_press(Message::AriadneNext).into(),
            small_text("alt+n", crate::yorha::Tone::Secondary),
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 12, 6, 6])
    .width(Length::Fill)
    .class(card_style())
    .into()
}

/// A panel's header slab: caps label flush left, value flush right, light ink on the one dark.
pub fn header<'a, M: 'a>(label: &str, value: String) -> Element<'a, M> {
    widget::container(
        widget::row::with_children([
            crate::palette::body(label.to_uppercase()).into(),
            widget::space::horizontal().into(),
            crate::palette::body(value).into(),
        ])
        .align_y(Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .class(crate::palette::slab())
    .into()
}

/// The light header of a detail panel: a plate with a solid square and the caps label in ink.
pub fn plate_header<'a, M: 'a>(label: &str, value: String) -> Element<'a, M> {
    let square = widget::container(widget::space::horizontal().width(8).height(8)).class(
        theme::Container::custom(|_| container::Style {
            background: ladder().map(|t| Background::Color(t.solid())),
            ..Default::default()
        }),
    );
    widget::container(
        widget::row::with_children([
            square.into(),
            crate::palette::body(label.to_uppercase()).into(),
            widget::space::horizontal().into(),
            crate::palette::body(value).into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .class(theme::Container::custom(|_| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.base_dim())),
            text_color: Some(t.text()),
            ..Default::default()
        },
        None => container::Style::default(),
    }))
    .into()
}

/// A label and its value on one row: the label in secondary ink, the value right-aligned.
pub fn field<'a, M: 'a>(label: &str, value: String) -> Element<'a, M> {
    widget::row::with_children([
        crate::palette::body(label.to_string())
            .class(crate::palette::secondary())
            .into(),
        widget::space::horizontal().into(),
        crate::palette::body(value).into(),
    ])
    .align_y(Alignment::Center)
    .into()
}

/// A tag plate: caps on the plate, with the gold category edge on the left when it names a kind.
pub fn tag<'a, M: 'a>(text: &str, category: bool) -> Element<'a, M> {
    let label = widget::container(crate::yorha::micro(text, crate::yorha::Tone::Ink))
        .padding([2, 6])
        .class(theme::Container::custom(|_| match ladder() {
            Some(t) => container::Style {
                background: Some(Background::Color(t.base_dim())),
                text_color: Some(t.text()),
                ..Default::default()
            },
            None => container::Style::default(),
        }));
    if !category {
        return label.into();
    }
    let edge = widget::container(widget::space::vertical().width(3).height(Length::Fill)).class(
        theme::Container::custom(|_| container::Style {
            background: ladder().map(|t| Background::Color(t.gold())),
            ..Default::default()
        }),
    );
    widget::row::with_children([edge.into(), label.into()])
        .height(Length::Shrink)
        .into()
}

/// A picture of the thing in a light matte with a 1px ink frame (the viewport rule: foreign
/// colours, a thumbnail, are legal inside the frame).
pub fn matte<'a, M: 'a>(content: Element<'a, M>, size: f32) -> Element<'a, M> {
    widget::container(content)
        .center(Length::Fixed(size))
        .class(theme::Container::custom(|_| match ladder() {
            Some(t) => container::Style {
                background: Some(Background::Color(t.base_dim())),
                border: Border {
                    color: t.text(),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            },
            None => container::Style::default(),
        }))
        .into()
}

/// The title lockup: the name large in caps with its ghost behind it, and a secondary line.
pub fn lockup<'a, M: 'a>(word: &str, sub: String) -> Element<'a, M> {
    crate::yorha::title_lockup(&clip(word, 12), &clip(&sub, 30))
}

/// A section label inside a panel: the micro caption, then a rule in the plate tone to the edge.
pub fn section<'a, M: 'a>(label: &str) -> Element<'a, M> {
    let rule = widget::container(widget::space::horizontal().height(1))
        .width(Length::Fill)
        .class(theme::Container::custom(|_| container::Style {
            background: ladder().map(|t| Background::Color(t.shadow())),
            ..Default::default()
        }));
    widget::row::with_children([
        crate::yorha::micro(label, crate::yorha::Tone::Secondary),
        rule.into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// A recessed input well: a 1px ink frame, a micro label in front of what it holds.
pub fn well<'a, M: 'a>(label: &str, content: Element<'a, M>) -> Element<'a, M> {
    widget::container(
        widget::row::with_children([
            crate::yorha::micro(label, crate::yorha::Tone::Secondary),
            content,
        ])
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([0, 8])
    .width(Length::Fill)
    .class(theme::Container::custom(|_| match ladder() {
        Some(t) => container::Style {
            border: Border {
                color: t.text(),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        },
        None => container::Style::default(),
    }))
    .into()
}

pub fn summary_id() -> widget::Id {
    widget::Id::new("ariadne-summary")
}

/// The field for a folder's summary; Enter or a click outside saves it.
pub fn summary_editor<'a>(text: &str) -> Element<'a, Message> {
    sheet(
        "summary",
        "enter saves".to_string(),
        vec![
            widget::text_input("what lives here", text.to_string())
                .id(summary_id())
                .on_input(Message::SummaryInput)
                .on_submit(|_| Message::SummarySave)
                .width(Length::Fill)
                .into(),
        ],
    )
}

/// An icon the noticed card is about: an ink frame around it.
pub fn lit<'a>(icon: Element<'a, Message>) -> Element<'a, Message> {
    widget::container(icon)
        .class(theme::Container::custom(|theme| container::Style {
            border: Border {
                color: ladder().map_or(theme.cosmic().accent_color().into(), |t| t.text()),
                width: 2.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }))
        .into()
}

fn by(e: &InboxEntry) -> String {
    e.by.clone().unwrap_or_default()
}

/// Where a file was before ariadne moved it: its icon faded, where it went, and Undo.
pub fn ghost<'a>(m: &Move, icon: widget::icon::Handle, width: f32, height: f32) -> Element<'a, Message> {
    let name = m
        .to
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    use crate::yorha::Tone;
    let dest = clip(&m.to.parent().map(folder_name).unwrap_or_default(), 10);
    let name = clip(&name, 13);
    widget::container(
        widget::column::with_children([
            widget::icon::icon(icon).size(48).into(),
            widget::text::caption(name)
                .class(crate::palette::secondary())
                .wrapping(cosmic::iced::widget::text::Wrapping::None)
                .center()
                .into(),
            small_text(&format!("→ {dest}"), Tone::Secondary),
            widget::button::custom(small_text("undo", Tone::Ink))
                .padding([3, 8])
                .on_press(Message::Inbox(m.to.clone(), InboxVerb::Undo))
                .class(crate::palette::plate_button())
                .into(),
        ])
        .spacing(4)
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .padding(4)
    .class(theme::Container::custom(|_| match ladder() {
        Some(t) => container::Style {
            border: Border {
                color: t.secondary(),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        },
        None => container::Style::default(),
    }))
    .into()
}
