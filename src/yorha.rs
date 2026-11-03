//! The desktop's drawn chrome (rails, cursor, grid, ornament, tracked caps, title lockup).
//!
//! Ports of the quickshell primitives in `~/.config/quickshell/yorha/` (round 2 numbers, see
//! `PRIMITIVES.md` of the 2026-10-04 audition), keeping their QML names. Every mark lands on
//! whole device pixels the way the QML does with `Math.round(v * dpr) / dpr`: the window's scale
//! factor only reaches `Widget::draw` (a canvas `Program` never sees it), so each primitive is one
//! small widget that paints into a canvas `Cache`. None of them handles an event or asks for a
//! cursor, so in a `stack` the layers under them keep their hover and clicks.
//!
//! Tones come from `crate::palette::ladder()`. Under any other theme every primitive draws
//! nothing and takes no space.

use std::cell::RefCell;
use std::sync::Mutex;

use cosmic::iced::advanced::graphics::geometry::Renderer as _;
use cosmic::iced::advanced::graphics::text::Paragraph;
use cosmic::iced::advanced::text::{self, Paragraph as _};
use cosmic::iced::advanced::widget::{Tree, tree};
use cosmic::iced::advanced::{Layout, Renderer as _, Widget, layout, mouse, renderer};
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Cache, Frame, Path, Stroke, Text};
use cosmic::iced::widget::{Space, column};
use cosmic::iced::{Color, Font, Length, Pixels, Point, Rectangle, Size, Vector};
use cosmic::{Element, Renderer, Theme};

use crate::palette::{Tones, ladder};

/// Rail height and period, `Config.chrome.railHeight` and `railPeriod`.
const RAIL_HEIGHT: f32 = 11.0;
const RAIL_PERIOD: f32 = 38.0;
/// The cursor lozenge's design box.
const LOZENGE: Size = Size::new(21.0, 8.0);
/// The lozenge's width, for the margin that holds it.
pub const LOZENGE_WIDTH: f32 = LOZENGE.width;
/// Grid period and ink opacity, `Config.chrome.grid` and `gridOpacity` (round 2).
const GRID: f32 = 7.0;
const GRID_INK: f32 = 0.04;
/// Gaps between the ornament's hairlines in logical px, cycled: `DiagonalOrnament.gaps`.
const GAPS: [f32; 8] = [36.0, 106.0, 24.0, 144.0, 60.0, 132.0, 48.0, 96.0];
/// Line box of tracked caps, in em.
const LEADING: f32 = 1.4;
/// Monaco's advance in em, used only when the font cannot be measured.
const ADVANCE: f32 = 0.6;

/// What a piece of text is, never how dark it should look (`Label.tone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// text on paper
    Ink,
    /// captions, units, labels beside values
    Secondary,
    /// disabled, empty, zero, unknown `--`
    Disabled,
    /// on a dark slab or an urgent plate
    Light,
    /// urgent text on paper
    Urgent,
}

impl Tone {
    fn color(self, t: &Tones) -> Color {
        match self {
            Self::Ink => t.text(),
            Self::Secondary => t.secondary(),
            Self::Disabled => t.dim(),
            Self::Light => t.light(),
            Self::Urgent => t.urgent(),
        }
    }
}

/// The frame rail: a 1px rule in ink, a 6x2 staple hanging off it every 38px at +2, and a
/// triad of 2x2 squares between staples (+21 and +25 on the near row, +23 three px further).
/// Full width, 11 tall. `under`: rule on top with the ornament below it (the rail under a
/// title); otherwise mirrored (the rail over a footer). Never on a strip.
pub fn dot_rail<'a, M: 'a>(under: bool) -> Element<'a, M> {
    ink(
        |t| Paint::Rail {
            under,
            rule: t.text(),
            dot: t.dot(),
        },
        Length::Fill,
        Length::Fixed(RAIL_HEIGHT),
    )
}

/// The game's selection cursor, 21x8: a kite pointing right with a dot punched near its blunt
/// end and a colon of two 2px squares after the tip. It sits 32px left of the row it marks.
/// `ghost` is the unfocused list's placeholder: kite in the panel tone, dot in the shadow, no
/// colon.
pub fn cursor_lozenge<'a, M: 'a>(ghost: bool) -> Element<'a, M> {
    ink(
        |t| Paint::Lozenge {
            ink: if ghost { t.highlight() } else { t.solid() },
            hole: if ghost { t.shadow() } else { t.highlight() },
            colon: !ghost,
        },
        Length::Fixed(LOZENGE.width),
        Length::Fixed(LOZENGE.height),
    )
}

/// The ground grid: 1px ink lines at 4% on the top and left of every 7px cell. Fills its
/// space; make it the last layer of a `stack` so it lies over panels and slabs alike.
pub fn grid_texture<'a, M: 'a>() -> Element<'a, M> {
    ink(
        |t| Paint::Grid {
            ink: Color {
                a: GRID_INK,
                ..t.text()
            },
        },
        Length::Fill,
        Length::Fill,
    )
}

/// The ground ornament: sparse 45 degree hairlines at uneven gaps and two large arcs in
/// `base_dim`. Fills its space; make it the first layer of a `stack` so panels cover it.
pub fn diagonal_ornament<'a, M: 'a>() -> Element<'a, M> {
    ink(
        |t| Paint::Ornament { ink: t.base_dim() },
        Length::Fill,
        Length::Fill,
    )
}

/// `text` tracked by `track` em at `size` px, as it comes (no case change), sized exactly to
/// its content: `n` advances plus `n - 1` gaps wide, `1.4 * size` tall.
pub fn caps<'a, M: 'a>(text: &str, size: f32, track: f32, tone: Tone) -> Element<'a, M> {
    tracked(text, size, track, tone, false)
}

/// The instrument register: 9px caps tracked 0.30em (gauge labels, axes, tags, key hints).
pub fn micro<'a, M: 'a>(text: &str, tone: Tone) -> Element<'a, M> {
    caps(&text.to_uppercase(), 9.0, 0.30, tone)
}

/// The page title: 24px caps tracked 0.15em, in ink.
pub fn title<'a, M: 'a>(text: &str) -> Element<'a, M> {
    caps(&text.to_uppercase(), 24.0, 0.15, Tone::Ink)
}

/// The title lockup of a detail panel: the word as a title with its ghost copy behind it
/// (+0.16em right, +0.13em down, `text_faint`), and `sub` under it in the body register,
/// secondary. 8px of room above the word, 4px between the word and `sub`.
pub fn title_lockup<'a, M: 'a>(word: &str, sub: &str) -> Element<'a, M> {
    if ladder().is_none() {
        return Space::new().into();
    }
    column![
        tracked(&word.to_uppercase(), 24.0, 0.15, Tone::Ink, true),
        caps(sub, 12.0, 0.03, Tone::Secondary),
    ]
    .spacing(4)
    .padding([8, 0, 0, 0])
    .into()
}

fn tracked<'a, M: 'a>(text: &str, size: f32, track: f32, tone: Tone, ghost: bool) -> Element<'a, M> {
    // Fluent wraps every argument in bidi isolates; each would take a cell of its own here.
    let text: String = text
        .chars()
        .filter(|c| !('\u{2066}'..='\u{2069}').contains(c))
        .collect();
    let font = cosmic::font::default();
    let advance = advance(font) * size;
    let gap = size * track;
    let n = text.chars().count() as f32;
    let width = if n > 0.0 {
        (n * advance + (n - 1.0) * gap).ceil()
    } else {
        0.0
    };
    let height = (size * LEADING).round();
    ink(
        move |t| Paint::Caps {
            text,
            font,
            size,
            step: advance + gap,
            height,
            ink: tone.color(t),
            ghost: ghost.then(|| {
                (
                    t.faint(),
                    Vector::new((size * 0.16).round(), (size * 0.13).round()),
                )
            }),
        },
        Length::Fixed(width),
        Length::Fixed(height),
    )
}

/// The advance of `font` in em, measured once per font on a run of capitals.
fn advance(font: Font) -> f32 {
    static SEEN: Mutex<Vec<(Font, f32)>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap();
    if let Some((_, em)) = seen.iter().find(|(f, _)| *f == font) {
        return *em;
    }
    const RUN: &str = "MMMMMMMMMM";
    const SIZE: f32 = 100.0;
    let width = Paragraph::with_text(text::Text {
        content: RUN,
        bounds: Size::INFINITE,
        size: Pixels(SIZE),
        line_height: text::LineHeight::default(),
        font,
        align_x: text::Alignment::Left,
        align_y: Vertical::Top,
        shaping: text::Shaping::Basic,
        wrapping: text::Wrapping::None,
        ellipsize: text::Ellipsize::None,
    })
    .min_bounds()
    .width;
    let em = width / (SIZE * RUN.len() as f32);
    let em = if em > 0.0 { em } else { ADVANCE };
    seen.push((font, em));
    em
}

/// One primitive, painted with the palette's tones, or empty space when there is no palette.
fn ink<'a, M: 'a>(
    paint: impl FnOnce(&Tones) -> Paint,
    width: Length,
    height: Length,
) -> Element<'a, M> {
    match ladder() {
        Some(t) => Element::new(Ink {
            paint: paint(&t),
            width,
            height,
        }),
        None => Space::new().into(),
    }
}

/// What a primitive paints, with its colours resolved.
#[derive(Clone, Debug, PartialEq)]
enum Paint {
    Rail {
        under: bool,
        rule: Color,
        dot: Color,
    },
    Lozenge {
        ink: Color,
        hole: Color,
        colon: bool,
    },
    Grid {
        ink: Color,
    },
    Ornament {
        ink: Color,
    },
    Caps {
        text: String,
        font: Font,
        size: f32,
        /// advance plus tracking, logical px
        step: f32,
        height: f32,
        ink: Color,
        /// the lockup's ghost copy: its ink and offset
        ghost: Option<(Color, Vector)>,
    },
}

/// Device pixel arithmetic for one draw: `scale` device px per logical px, and `shift`, the
/// logical offset (under one device px) that puts the widget's origin on a device pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Px {
    scale: f32,
    shift: Vector,
}

impl Px {
    fn new(scale: f32, origin: Point) -> Self {
        let shift = |v: f32| {
            let at = v * scale;
            // a hair of slack so float noise on a whole px does not push it a px further
            ((at - 1e-3).ceil() - at) / scale
        };
        Self {
            scale,
            shift: Vector::new(shift(origin.x), shift(origin.y)),
        }
    }

    /// A logical length as whole device px.
    fn dev(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// The point `x`, `y` device px from the snapped origin.
    fn at(&self, x: f32, y: f32) -> Point {
        Point::new(self.shift.x + x / self.scale, self.shift.y + y / self.scale)
    }

    /// A size of `w` by `h` device px.
    fn size(&self, w: f32, h: f32) -> Size {
        Size::new(w / self.scale, h / self.scale)
    }
}

impl Paint {
    /// How far the paint may spill past the widget's bounds, logical px.
    fn spill(&self) -> Vector {
        match self {
            // rounding to device px can add up to one px at the far edges
            Self::Rail { .. } | Self::Lozenge { .. } => Vector::new(1.0, 1.0),
            Self::Grid { .. } | Self::Ornament { .. } => Vector::new(0.0, 0.0),
            Self::Caps { ghost, .. } => {
                ghost.map_or(Vector::new(1.0, 1.0), |(_, at)| at + Vector::new(1.0, 1.0))
            }
        }
    }

    fn draw(&self, frame: &mut Frame<Renderer>, px: &Px, size: Size) {
        match self {
            Self::Rail { under, rule, dot } => rail(frame, px, size, *under, *rule, *dot),
            Self::Lozenge { ink, hole, colon } => lozenge(frame, px, *ink, *hole, *colon),
            Self::Grid { ink } => grid(frame, px, size, *ink),
            Self::Ornament { ink } => ornament(frame, px, size, *ink),
            Self::Caps {
                text,
                font,
                size: em,
                step,
                height,
                ink,
                ghost,
            } => {
                if let Some((faint, at)) = ghost {
                    letters(frame, px, text, *font, *em, *step, *height, *faint, *at);
                }
                letters(
                    frame,
                    px,
                    text,
                    *font,
                    *em,
                    *step,
                    *height,
                    *ink,
                    Vector::new(0.0, 0.0),
                );
            }
        }
    }
}

/// `DotRail.paintRail`, with no phase.
fn rail(frame: &mut Frame<Renderer>, px: &Px, size: Size, under: bool, rule: Color, dot: Color) {
    let hd = px.dev(RAIL_HEIGHT);
    let wd = (size.width * px.scale).ceil();
    let pd = px.dev(RAIL_PERIOD).max(1.0);
    // y of a mark `mh` tall at `oy` from the rule, in device px
    let y = |oy: f32, mh: f32| {
        if under {
            px.dev(oy)
        } else {
            (hd - (oy + mh) * px.scale).round()
        }
    };
    let two = px.dev(2.0);
    let mut k = 0.0;
    while k * pd < wd {
        let x = |off: f32| k * pd + px.dev(off);
        frame.fill_rectangle(px.at(x(2.0), y(1.0, 2.0)), px.size(px.dev(6.0), two), dot);
        frame.fill_rectangle(px.at(x(21.0), y(5.0, 2.0)), px.size(two, two), dot);
        frame.fill_rectangle(px.at(x(25.0), y(5.0, 2.0)), px.size(two, two), dot);
        frame.fill_rectangle(px.at(x(23.0), y(8.0, 2.0)), px.size(two, two), dot);
        k += 1.0;
    }
    let ry = if under { 0.0 } else { hd - 1.0 };
    frame.fill_rectangle(px.at(0.0, ry), px.size(wd, 1.0), rule);
}

/// `CursorLozenge.paintLozenge`: the kite and its dot keep their antialiasing, the colon lands
/// on whole device px.
fn lozenge(frame: &mut Frame<Renderer>, px: &Px, ink: Color, hole: Color, colon: bool) {
    let sx = px.dev(LOZENGE.width) / px.scale / LOZENGE.width;
    let sy = px.dev(LOZENGE.height) / px.scale / LOZENGE.height;
    let p = |x: f32, y: f32| Point::new(px.shift.x + x * sx, px.shift.y + y * sy);
    let kite = Path::new(|b| {
        b.move_to(p(0.0, 4.0));
        b.line_to(p(5.0, 0.8));
        b.line_to(p(16.0, 4.0));
        b.line_to(p(5.0, 7.2));
        b.close();
    });
    frame.fill(&kite, ink);
    frame.fill(&Path::circle(p(4.6, 4.0), 1.1 * sx.min(sy)), hole);
    if colon {
        let x = px.dev(18.0 * sx);
        let size = px.size(px.dev(2.0 * sx), px.dev(2.0 * sy));
        frame.fill_rectangle(px.at(x, 0.0), size, ink);
        frame.fill_rectangle(px.at(x, px.dev(6.0 * sy)), size, ink);
    }
}

/// `GridTexture.paintGrid`. The QML fades the whole canvas, so a crossing is inked once; here
/// the vertical lines stop at each horizontal one for the same result.
fn grid(frame: &mut Frame<Renderer>, px: &Px, size: Size, ink: Color) {
    let pd = px.dev(GRID).max(2.0);
    let wd = (size.width * px.scale).floor();
    let hd = (size.height * px.scale).floor();
    let mut j = 0.0;
    while j * pd < hd {
        frame.fill_rectangle(px.at(0.0, j * pd), px.size(wd, 1.0), ink);
        j += 1.0;
    }
    let mut i = 0.0;
    while i * pd < wd {
        let mut j = 0.0;
        while j * pd < hd {
            let top = j * pd + 1.0;
            let bottom = ((j + 1.0) * pd).min(hd);
            if bottom > top {
                frame.fill_rectangle(px.at(i * pd, top), px.size(1.0, bottom - top), ink);
            }
            j += 1.0;
        }
        i += 1.0;
    }
}

/// `DiagonalOrnament` in its sparse form with two arcs: `\` hairlines from the top edge to the
/// bottom at the cycled gaps, a circle of 0.8w about (0.95w, 0.6w) and one of 1.0w about
/// (0.05w, 0.85h), all 1px and antialiased.
fn ornament(frame: &mut Frame<Renderer>, px: &Px, size: Size, ink: Color) {
    let (w, h) = (size.width, size.height);
    let p = |x: f32, y: f32| Point::new(px.shift.x + x, px.shift.y + y);
    let lines = Path::new(|b| {
        let mut x = -h + GAPS[0];
        let mut i = 0;
        while x < w {
            b.move_to(p(x + 0.5, 0.0));
            b.line_to(p(x + h + 0.5, h));
            i = (i + 1) % GAPS.len();
            x += GAPS[i];
        }
    });
    let arcs = Path::new(|b| {
        b.circle(p(w * 0.95, w * 0.6), w * 0.8);
        b.circle(p(w * 0.05, h * 0.85), w);
    });
    let stroke = Stroke::default().with_color(ink).with_width(1.0);
    frame.stroke(&lines, stroke);
    frame.stroke(&arcs, stroke);
}

/// One glyph per character, each on a whole device px, `step` apart.
#[allow(clippy::too_many_arguments)]
fn letters(
    frame: &mut Frame<Renderer>,
    px: &Px,
    text: &str,
    font: Font,
    size: f32,
    step: f32,
    height: f32,
    color: Color,
    at: Vector,
) {
    for (i, ch) in text.chars().enumerate() {
        if ch.is_whitespace() {
            continue;
        }
        frame.fill_text(Text {
            content: ch.to_string(),
            position: px.at(px.dev(i as f32 * step + at.x), px.dev(at.y)),
            color,
            size: Pixels(size),
            line_height: text::LineHeight::Absolute(Pixels(height)),
            font,
            align_y: Vertical::Top,
            shaping: text::Shaping::Basic,
            ..Text::default()
        });
    }
}

/// A primitive as a widget: a fixed or filling box that paints `paint` into a cache and
/// handles nothing.
struct Ink {
    paint: Paint,
    width: Length,
    height: Length,
}

/// The cache, and what it holds: the paint, the device grid it was drawn on.
#[derive(Default)]
struct State {
    cache: Cache<Renderer>,
    drawn: RefCell<Option<(Paint, Px)>>,
}

impl<M> Widget<M, Theme, Renderer> for Ink {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }
        let state = tree.state.downcast_ref::<State>();
        let px = Px::new(style.scale_factor as f32, bounds.position());
        // the cache only follows its bounds: clear it when the paint or the device grid moves
        {
            let mut drawn = state.drawn.borrow_mut();
            let fresh = drawn
                .as_ref()
                .is_some_and(|(paint, at)| *paint == self.paint && *at == px);
            if !fresh {
                state.cache.clear();
                *drawn = Some((self.paint.clone(), px));
            }
        }
        let spill = self.paint.spill();
        let clip = Rectangle::new(
            Point::ORIGIN,
            Size::new(bounds.width + spill.x, bounds.height + spill.y),
        );
        let size = bounds.size();
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            let geometry = state
                .cache
                .draw_with_bounds(renderer, clip, |frame| self.paint.draw(frame, &px, size));
            renderer.draw_geometry(geometry);
        });
    }
}
