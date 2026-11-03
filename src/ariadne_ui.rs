//! What ariadne draws inside a folder view: the pill on a file it has something to say about,
//! the card that opens from it, and the ghost a file leaves where it was moved from.

use std::path::Path;

use cosmic::iced::{Alignment, Background, Border, Color, Length, Shadow, Vector};
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

/// Shorten from the middle, so a path keeps its root and its last folder, and a file name its
/// start and extension.
pub fn clip_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 5 {
        return s.to_string();
    }
    let tail = (max - 1) / 2;
    let head = max - 1 - tail;
    let start: String = s.chars().take(head).collect();
    let end: String = s.chars().skip(n - tail).collect();
    format!("{start}…{end}")
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
                .font(cosmic::font::default())
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

/// One section of the places panel: its header slab with the count over its places as plates,
/// or the plates alone when it has no label.
pub fn place_section<'a, M: 'a>(
    label: Option<&str>,
    count: usize,
    places: Element<'a, M>,
) -> Element<'a, M> {
    let places = widget::container(places).padding([8, 8, 8, 8]);
    match label {
        Some(label) => widget::column::with_children([
            header(label, format!("{count:02}")),
            places.into(),
        ])
        .into(),
        None => places.into(),
    }
}

/// The tracks under the Drives rows, laid over them at the rows' pitch: ink for the share taken,
/// the track tone for the rest, light on the dark of the current drive. `rows` is each row's
/// share taken (None for a disk that could not be measured) and whether it is lit.
pub fn usage_tracks<'a, M: 'a>(rows: Vec<(Option<f32>, bool)>, row: u16, spacing: u16) -> Element<'a, M> {
    let Some(t) = ladder() else {
        return widget::space::horizontal().into();
    };
    let fill = |color: cosmic::iced::Color, portion: u16| -> Element<'a, M> {
        widget::container(widget::space::horizontal())
            .width(Length::FillPortion(portion))
            .height(3)
            .class(theme::Container::custom(move |_| container::Style {
                background: Some(Background::Color(color)),
                ..Default::default()
            }))
            .into()
    };
    let tracks = rows.into_iter().map(|(used, lit)| {
        let (taken, rest) = if lit {
            (t.light(), t.secondary())
        } else {
            (t.text(), t.shadow())
        };
        let track: Element<'a, M> = match used {
            Some(used) => {
                let part = (used.clamp(0.0, 1.0) * 1000.0).round() as u16;
                widget::row::with_capacity(2)
                    .push_maybe((part > 0).then(|| fill(taken, part)))
                    .push_maybe((part < 1000).then(|| fill(rest, 1000 - part)))
                    .height(3)
                    .into()
            }
            None => widget::space::vertical().height(3).into(),
        };
        // Under the name: past the icon and its gap, 6 above the plate's foot.
        widget::container(track)
            .padding([f32::from(row) - 9.0, 8.0, 6.0, 32.0])
            .height(Length::Fixed(f32::from(row)))
            .width(Length::Fill)
            .into()
    });
    widget::column::with_children(tracks).spacing(spacing).into()
}

/// The places panel, in the window and the picker alike: the sections in a scroll, and `foot`
/// (Trash) held at the bottom.
pub fn places_panel<'a, M: 'a>(
    sections: Vec<Element<'a, M>>,
    foot: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let body = widget::column::with_capacity(2)
        .push(
            widget::scrollable(widget::column::with_children(sections))
                .class(cosmic::style::iced::Scrollable::Minimal)
                .height(Length::Fill),
        )
        .push_maybe(foot);
    let panel = widget::container(body)
        .class(crate::palette::panel())
        .width(Length::Fixed(232.0))
        .height(Length::Fill);
    widget::container(panel).padding([0, 2, 2, 0]).into()
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

/// Every control on the top bar is this tall, so they share one centre line.
const BAR: f32 = 24.0;

/// Wide enough for a short sentence: search takes queries by meaning.
const SEARCH_WIDTH: f32 = 320.0;

/// A control on the top bar: a plate at rest when `plate`, otherwise nothing; an ink border on
/// hover; the one dark while pressed or while `on` (the current view). With nothing to do it
/// drops its plate and its ink goes to the disabled tone. Icons on it are symbolic, so they take
/// the same ink as its words.
pub fn bar_button(plate: bool, on: bool) -> theme::Button {
    let look = move |hovered: bool, pressed: bool, disabled: bool, theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        let Some(t) = ladder() else {
            let c = theme.cosmic();
            let (fill, ink): (Color, Color) = if on || pressed {
                (c.accent_color().into(), c.on_accent_color().into())
            } else {
                (c.button_bg_color().into(), c.on_bg_color().into())
            };
            let ink = if disabled && !on { Color { a: 0.5, ..ink } } else { ink };
            s.background = Some(Background::Color(fill));
            s.text_color = Some(ink);
            s.icon_color = Some(ink);
            return s;
        };
        if on || pressed {
            s.background = Some(Background::Color(t.accent()));
            s.text_color = Some(t.light());
            s.icon_color = Some(t.light());
            return s;
        }
        if disabled {
            s.text_color = Some(t.dim());
            s.icon_color = Some(t.dim());
            return s;
        }
        if plate {
            s.background = Some(Background::Color(t.base_dim()));
        }
        s.text_color = Some(t.text());
        s.icon_color = Some(t.text());
        if hovered {
            s.border_width = 1.0;
            s.border_color = t.text();
        }
        s
    };
    theme::Button::Custom {
        active: Box::new(move |_, theme| look(false, false, false, theme)),
        disabled: Box::new(move |theme| look(false, false, true, theme)),
        hovered: Box::new(move |_, theme| look(true, false, false, theme)),
        pressed: Box::new(move |_, theme| look(true, true, false, theme)),
    }
}

/// A word on a plate on the top bar. Without `press` it rests disabled.
pub fn bar_text<'a, M: Clone + 'a>(label: String, press: Option<M>) -> Element<'a, M> {
    widget::button::custom(widget::container(crate::palette::body(label)).center_y(Length::Fixed(BAR)))
        .padding([0, 8])
        .on_press_maybe(press)
        .class(bar_button(true, false))
        .into()
}

/// An icon on the top bar, in a square of the bar's height.
pub fn bar_icon<'a, M: Clone + 'static>(
    icon: widget::icon::Handle,
    class: theme::Button,
    press: Option<M>,
) -> Element<'a, M> {
    widget::button::custom(widget::container(widget::icon(icon).size(16)).center(Length::Fixed(BAR)))
        .padding(0)
        .on_press_maybe(press)
        .class(class)
        .into()
}

/// An icon and a word on the top bar.
pub fn bar_pair<'a, M: Clone + 'static>(
    icon: widget::icon::Handle,
    label: String,
    class: theme::Button,
    press: M,
) -> Element<'a, M> {
    widget::button::custom(
        widget::row::with_children([
            widget::icon(icon).size(16).into(),
            crate::palette::body(label).into(),
        ])
        .spacing(8)
        .height(Length::Fixed(BAR))
        .align_y(Alignment::Center),
    )
    .padding([0, 8])
    .on_press(press)
    .class(class)
    .into()
}

/// The top bar's own pictograms, drawn in whatever ink the button gives its icon.
pub enum Glyph {
    Grid,
    List,
    Map,
    Tidy,
    Info,
    Tick,
    Dash,
    Shut,
    Open,
    Arrow,
}

pub fn glyph(g: Glyph) -> widget::icon::Handle {
    let svg: &'static str = match g {
        Glyph::Grid => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 2H7V7H2Z M9 2H14V7H9Z M2 9H7V14H2Z M9 9H14V14H9Z"/></svg>"#
        }
        Glyph::List => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M1 3H3V5H1Z M5 3H15V5H5Z M1 7H3V9H1Z M5 7H15V9H5Z M1 11H3V13H1Z M5 11H15V13H5Z"/></svg>"#
        }
        Glyph::Map => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M1 2H8V14H1Z M9 2H15V9H9Z M9 10H12V14H9Z M13 10H15V14H13Z"/></svg>"#
        }
        Glyph::Tidy => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M1 3H15V5H1Z M1 7H11V9H1Z M1 11H7V13H1Z"/></svg>"#
        }
        Glyph::Info => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill-rule="evenodd" d="M1 1H15V15H1Z M2.5 2.5V13.5H13.5V2.5Z"/><path d="M7 4H9V6H7Z M7 7H9V12H7Z"/></svg>"#
        }
        Glyph::Tick => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 8.5L4 6.5L6.5 9L12 3.5L14 5.5L6.5 13Z"/></svg>"#
        }
        Glyph::Dash => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3 7H13V9H3Z"/></svg>"#
        }
        Glyph::Shut => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M5 3L11 8L5 13Z"/></svg>"#
        }
        Glyph::Open => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3 5H13L8 11Z"/></svg>"#
        }
        Glyph::Arrow => {
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M1 7H10V4L15 8L10 12V9H1Z"/></svg>"#
        }
    };
    widget::icon::from_svg_bytes(svg.as_bytes()).symbolic(true)
}

/// What a tick box holds: a tick, nothing, or a dash for a group with only some ticked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ticked {
    On,
    Off,
    Some,
}

/// A 16px tick box: the one dark with a light tick or dash, or the plate with an ink frame when
/// empty.
///
/// It acts on the mouse press; the keyboard works it through the panel's own cursor. The cursor
/// (`ring`) or the focus draws a 1px ring of the one dark around it, 2px out from the box.
pub fn tick<'a, M: Clone + 'a>(state: Ticked, press: M, ring: bool) -> Element<'a, M> {
    // The box: the shadow tone with an ink edge when off, the one dark when on or partly on.
    let tone = move |theme: &theme::Theme| {
        let Some(t) = ladder() else {
            let c = theme.cosmic();
            return cosmic::iced::widget::container::Style {
                background: Some(Background::Color(match state {
                    Ticked::Off => c.button_bg_color().into(),
                    _ => c.accent_color().into(),
                })),
                icon_color: Some(c.on_accent_color().into()),
                ..Default::default()
            };
        };
        let off = state == Ticked::Off;
        cosmic::iced::widget::container::Style {
            background: Some(Background::Color(if off { t.shadow() } else { t.accent() })),
            border: cosmic::iced::Border {
                color: t.text(),
                width: if off { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            icon_color: Some(t.light()),
            ..Default::default()
        }
    };
    let mark: Element<'a, M> = match state {
        Ticked::On => widget::icon(glyph(Glyph::Tick)).size(12).into(),
        Ticked::Some => widget::icon(glyph(Glyph::Dash)).size(12).into(),
        Ticked::Off => widget::space::horizontal().width(12).height(12).into(),
    };
    let boxed = widget::container(mark)
        .center(Length::Fixed(16.0))
        .class(theme::Container::custom(tone));
    // The button is the box plus a 3px frame; focus draws a 1px ring at its edge, a 2px gap
    // from the box so it shows around a dark box too, and the ring never moves the box.
    let cursor = ring;
    let ring = move |focused: bool, theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        if focused || cursor {
            s.border_width = 1.0;
            s.border_color = match ladder() {
                Some(t) => t.accent(),
                None => theme.cosmic().accent_color().into(),
            };
        }
        s
    };
    widget::button::custom(boxed)
        .padding(3)
        .width(Length::Fixed(22.0))
        .height(Length::Fixed(22.0))
        .on_press_down(press)
        .class(theme::Button::Custom {
            active: Box::new(ring),
            disabled: Box::new(move |theme| ring(false, theme)),
            hovered: Box::new(ring),
            pressed: Box::new(ring),
        })
        .into()
}

/// The `+12 more` latch under a short list: light ink on the secondary plate with a drop shadow.
pub fn more<'a, M: Clone + 'a>(label: String, press: M) -> Element<'a, M> {
    let look = |theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        match ladder() {
            Some(t) => {
                s.background = Some(Background::Color(t.secondary()));
                s.text_color = Some(t.light());
            }
            None => {
                s.background = Some(Background::Color(theme.cosmic().button_bg_color().into()));
                s.text_color = Some(theme.cosmic().on_bg_color().into());
            }
        }
        s
    };
    let button = widget::button::custom(crate::palette::body(label))
        .padding([0, 4])
        .height(Length::Fixed(16.0))
        .on_press(press)
        .class(theme::Button::Custom {
            active: Box::new(move |_, theme| look(theme)),
            disabled: Box::new(move |theme| look(theme)),
            hovered: Box::new(move |_, theme| look(theme)),
            pressed: Box::new(move |_, theme| look(theme)),
        });
    // A button has no shadow tone of its own, so the plate's drop shadow sits on a frame.
    widget::container(button)
        .class(theme::Container::custom(|_| match ladder() {
            Some(t) => container::Style {
                shadow: Shadow {
                    color: t.faint(),
                    offset: Vector::new(2.0, 2.0),
                    blur_radius: 0.0,
                },
                ..Default::default()
            },
            None => container::Style::default(),
        }))
        .into()
}

/// A press area with no look of its own: a chevron, or a header row to fold.
pub fn flat<'a, M: Clone + 'a>(content: impl Into<Element<'a, M>>, press: M) -> widget::Button<'a, M> {
    // Keyboard focus draws a 1px ink frame; nothing else changes.
    let look = |focused: bool, _: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        if let Some(t) = ladder() {
            s.text_color = Some(t.text());
            s.icon_color = Some(t.solid());
            if focused {
                s.border_width = 1.0;
                s.border_color = t.text();
            }
        }
        s
    };
    widget::button::custom(content)
        .padding(0)
        .on_press(press)
        .class(theme::Button::Custom {
            active: Box::new(look),
            disabled: Box::new(move |theme| look(false, theme)),
            hovered: Box::new(look),
            pressed: Box::new(look),
        })
}

/// An icon in the secondary ink, for marks that only show: the arrow in a row.
pub fn quiet_icon<'a, M: 'a>(g: Glyph, size: u16) -> Element<'a, M> {
    widget::container(widget::icon(glyph(g)).size(size))
        .class(theme::Container::custom(|_| container::Style {
            icon_color: ladder().map(|t| t.secondary()),
            ..Default::default()
        }))
        .into()
}

/// A short name to edit inline, on a 16px line: ink on the plate tone, framed in ink while
/// typing.
pub fn name_field<'a, M: Clone + 'static>(
    value: String,
    on_input: impl Fn(String) -> M + 'a,
) -> widget::TextInput<'a, M> {
    let look = move |focused: bool, theme: &theme::Theme| {
        let c = theme.cosmic();
        let (ink, hint, fill, on_fill, well): (Color, Color, Color, Color, Color) = match ladder() {
            Some(t) => (t.text(), t.secondary(), t.accent(), t.light(), t.shadow()),
            None => (
                c.on_bg_color().into(),
                c.on_bg_color().into(),
                c.accent_color().into(),
                c.on_accent_color().into(),
                c.button_bg_color().into(),
            ),
        };
        widget::text_input::Appearance {
            background: Background::Color(well),
            border_radius: 0.0.into(),
            border_offset: None,
            border_width: if focused { 1.0 } else { 0.0 },
            border_color: ink,
            label_color: hint,
            placeholder_color: hint,
            selected_text_color: on_fill,
            icon_color: Some(ink),
            text_color: Some(ink),
            selected_fill: fill,
        }
    };
    widget::text_input("", value)
        .on_input(on_input)
        .style(theme::TextInput::Custom {
            active: Box::new(move |theme| look(false, theme)),
            error: Box::new(move |theme| look(false, theme)),
            hovered: Box::new(move |theme| look(false, theme)),
            focused: Box::new(move |theme| look(true, theme)),
            disabled: Box::new(move |theme| look(false, theme)),
        })
        // The renderer's own default is COSMIC's sans; the window's text face is the font set.
        .font(cosmic::font::default())
        .size(12.0)
        .line_height(cosmic::iced::widget::text::LineHeight::Absolute(16.0.into()))
        .padding([0, 4])
        .width(Length::Fill)
}

/// The search field's text: ink on the well, the hint in secondary ink, the selection in the
/// one dark.
fn field_style() -> theme::TextInput {
    let look = |theme: &theme::Theme| {
        let c = theme.cosmic();
        let (ink, hint, fill, on_fill): (Color, Color, Color, Color) = match ladder() {
            Some(t) => (t.text(), t.secondary(), t.accent(), t.light()),
            None => (
                c.on_bg_color().into(),
                c.on_bg_color().into(),
                c.accent_color().into(),
                c.on_accent_color().into(),
            ),
        };
        widget::text_input::Appearance {
            background: Background::Color(Color::TRANSPARENT),
            border_radius: 0.0.into(),
            border_offset: None,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            label_color: hint,
            placeholder_color: hint,
            selected_text_color: on_fill,
            icon_color: Some(ink),
            text_color: Some(ink),
            selected_fill: fill,
        }
    };
    theme::TextInput::Custom {
        active: Box::new(look),
        error: Box::new(look),
        hovered: Box::new(look),
        focused: Box::new(look),
        disabled: Box::new(look),
    }
}

/// The search field on the top bar, always open: a well like the location's under the palette,
/// COSMIC's search input otherwise. Without `on_input` it shows the term and takes no typing.
pub fn search_field<'a, M: Clone + 'static>(
    id: widget::Id,
    placeholder: String,
    term: &'a str,
    on_input: Option<impl Fn(String) -> M + 'a>,
    on_clear: M,
) -> Element<'a, M> {
    if ladder().is_none() {
        let mut input = widget::text_input::search_input(placeholder, term)
            .id(id)
            .on_clear(on_clear)
            .width(Length::Fixed(SEARCH_WIDTH));
        if let Some(on_input) = on_input {
            input = input.on_input(on_input);
        }
        return input.into();
    }
    let mut input = widget::text_input(placeholder, term)
        .id(id)
        .style(field_style())
        // The renderer's own default is COSMIC's sans; the window's text face is the font set.
        .font(cosmic::font::default())
        .size(12.0)
        .line_height(cosmic::iced::widget::text::LineHeight::Absolute(16.0.into()))
        .padding([4, 0])
        .width(Length::Fill);
    if let Some(on_input) = on_input {
        input = input.on_input(on_input);
    }
    widget::container(well("search", input.into()))
        .width(Length::Fixed(SEARCH_WIDTH))
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
                .font(cosmic::font::default())
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
