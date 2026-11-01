// SPDX-License-Identifier: GPL-3.0-only

//! The map view: folders as a squarified treemap sized by disk use, with meaning drawn on top.

use cosmic::iced::mouse;
use cosmic::iced::widget::canvas::{self, Frame, LineDash, Path, Stroke, Text};
use cosmic::iced::{Color, Length, Pixels, Point, Rectangle, Size, alignment};
use cosmic::{Element, Renderer, Theme};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::time::{Duration, Instant};

const WORLD: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1600.0,
    height: 1000.0,
};
const ZOOM: Duration = Duration::from_millis(520);
const TRAVEL: Duration = Duration::from_millis(1100);
const RING: Duration = Duration::from_millis(900);
const FADE: Duration = Duration::from_millis(300);
const DASH: &[f32] = &[6.0, 4.0];

/// One entry of the daemon's `GET /sizes` tree. A node with kids is a folder.
#[derive(Deserialize, Clone, Debug)]
pub struct Node {
    pub path: PathBuf,
    #[serde(default)]
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub kids: Vec<Node>,
}

/// Meaning drawn on top of the treemap.
#[derive(Clone, Debug)]
pub enum Overlay {
    /// A file that belongs elsewhere: a curved arrow from `from` to `to`.
    Arrow { from: PathBuf, to: PathBuf },
    /// Two folders that could be one: dashed borders joined by a dashed line.
    Merge { a: PathBuf, b: PathBuf },
    /// A collection: a line round each path.
    Outline { paths: Vec<PathBuf> },
}

/// A move to animate: a dot runs from the old folder to the new file, then a ring fades out.
#[derive(Clone, Debug)]
pub struct Travel {
    pub from: PathBuf,
    pub to: PathBuf,
    pub started: Instant,
}

#[derive(Clone, Debug)]
pub enum Event {
    /// A folder was clicked: zoom to it with `Map::zoom_to`.
    Zoom(PathBuf),
    /// The focused folder, or a file in it, was clicked again.
    Open(PathBuf),
    Hover(Option<PathBuf>),
    /// The wheel turned: pass the new view to `Map::wheel`.
    Wheel(Rectangle),
}

struct Item {
    path: PathBuf,
    name: String,
    size: u64,
    rect: Rectangle,
    head: f32,
    depth: usize,
    dir: bool,
    parent: Option<usize>,
}

struct Tween {
    from: Rectangle,
    to: Rectangle,
    started: Instant,
}

pub struct Map {
    items: Vec<Item>,
    index: HashMap<PathBuf, usize>,
    focus: usize,
    view: Rectangle,
    tween: Option<Tween>,
    overlays: Vec<Overlay>,
    travels: Vec<Travel>,
    now: Instant,
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            index: HashMap::new(),
            focus: 0,
            view: WORLD,
            tween: None,
            overlays: Vec::new(),
            travels: Vec::new(),
            now: Instant::now(),
        }
    }

    /// Replaces the tree. A refresh that still holds the focus keeps the view and any zoom
    /// under way.
    pub fn set_tree(&mut self, root: Node) {
        let focus = self.focus().map(FsPath::to_path_buf);
        self.items.clear();
        self.index.clear();
        self.lay(root, WORLD, 0, None);
        match focus.and_then(|path| self.index.get(&path).copied()) {
            Some(i) => {
                self.focus = i;
                let to = self.frame(i);
                if let Some(tween) = &mut self.tween {
                    tween.to = to;
                }
            }
            None => {
                self.focus = 0;
                self.view = WORLD;
                self.tween = None;
            }
        }
    }

    pub fn focus(&self) -> Option<&FsPath> {
        self.items.get(self.focus).map(|item| item.path.as_path())
    }

    /// Zooms to a folder, or to the folder holding a file.
    pub fn zoom_to(&mut self, path: &FsPath) {
        let Some(&i) = self.index.get(path) else {
            return;
        };
        let item = &self.items[i];
        self.focus = if item.dir {
            i
        } else {
            item.parent.unwrap_or(0)
        };
        self.animate(self.frame(self.focus));
    }

    pub fn zoom_out(&mut self) {
        if let Some(parent) = self.items.get(self.focus).and_then(|item| item.parent) {
            self.focus = parent;
            self.animate(self.frame(parent));
        }
    }

    pub fn fit(&mut self) {
        self.focus = 0;
        self.animate(WORLD);
    }

    /// Takes the view sent by `Event::Wheel`.
    pub fn wheel(&mut self, view: Rectangle) {
        self.tween = None;
        if view.width >= WORLD.width && view.height >= WORLD.height {
            self.view = WORLD;
        } else if view.width > 4.0 {
            self.view = view;
        }
        // the focus follows the deepest folder that still fills the view
        let centre = self.view.center();
        self.focus = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.dir
                    && item.rect.contains(centre)
                    && item.rect.width >= self.view.width * 0.5
                    && item.rect.height >= self.view.height * 0.5
            })
            .map(|(i, _)| i)
            .last()
            .unwrap_or(0);
    }

    pub fn set_overlays(&mut self, overlays: Vec<Overlay>) {
        self.overlays = overlays;
    }

    pub fn travel(&mut self, travel: Travel) {
        self.travels.push(travel);
    }

    /// Advances the zoom tween and drops finished travels. Call on every frame while
    /// `is_animating` is true.
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
        if let Some(tween) = &self.tween {
            let t = progress(now, tween.started, ZOOM);
            self.view = mix(tween.from, tween.to, ease(t));
            if t >= 1.0 {
                self.tween = None;
            }
        }
        self.travels
            .retain(|travel| now.saturating_duration_since(travel.started) < TRAVEL + RING);
    }

    pub fn is_animating(&self) -> bool {
        self.tween.is_some() || !self.travels.is_empty()
    }

    pub fn view<'a, M: 'a>(&'a self, on: impl Fn(Event) -> M + 'a) -> Element<'a, M> {
        canvas::Canvas::new(Painter {
            map: self,
            on: Box::new(on),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn lay(&mut self, node: Node, rect: Rectangle, depth: usize, parent: Option<usize>) {
        let i = self.items.len();
        let dir = !node.kids.is_empty();
        let name = if node.name.is_empty() {
            node.path.file_name().map_or_else(
                || node.path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        } else {
            node.name
        };
        let pad = 5f32.min(rect.width * 0.03).min(rect.height * 0.03);
        let head = if depth > 0 && dir {
            20f32.min(rect.height * 0.14)
        } else {
            0.0
        };
        self.index.insert(node.path.clone(), i);
        self.items.push(Item {
            path: node.path,
            name,
            size: node.size,
            rect,
            head,
            depth,
            dir,
            parent,
        });
        if !dir {
            return;
        }
        let inner = Rectangle {
            x: rect.x + pad,
            y: rect.y + pad + head,
            width: rect.width - 2.0 * pad,
            height: rect.height - 2.0 * pad - head,
        };
        if inner.width < 1.0 || inner.height < 1.0 {
            return;
        }
        let total: f64 = node.kids.iter().map(|kid| kid.size as f64).sum();
        let floor = (total * 0.01).max(1.0);
        let weights: Vec<f64> = node
            .kids
            .iter()
            .map(|kid| (kid.size as f64).max(floor))
            .collect();
        let rects = squarify(&weights, inner);
        for (kid, rect) in node.kids.into_iter().zip(rects) {
            self.lay(kid, rect, depth + 1, Some(i));
        }
    }

    fn frame(&self, i: usize) -> Rectangle {
        match self.items.get(i) {
            Some(item) if i > 0 => item.rect.expand(item.rect.width.max(item.rect.height) * 0.05),
            _ => WORLD,
        }
    }

    fn animate(&mut self, to: Rectangle) {
        self.tween = Some(Tween {
            from: self.view,
            to,
            started: Instant::now(),
        });
    }

    fn hit(&self, at: Point) -> Option<usize> {
        // items are in pre-order, so the last one that contains the point is the deepest
        self.items.iter().rposition(|item| item.rect.contains(at))
    }

    fn click(&self, path: &FsPath) -> Option<Event> {
        let &i = self.index.get(path)?;
        let item = &self.items[i];
        let folder = if item.dir {
            i
        } else {
            item.parent.unwrap_or(0)
        };
        Some(if folder != self.focus {
            Event::Zoom(self.items[folder].path.clone())
        } else {
            Event::Open(item.path.clone())
        })
    }

    fn screen(&self, path: &FsPath, xf: &Xf) -> Option<Rectangle> {
        self.index.get(path).map(|&i| xf.rect(self.items[i].rect))
    }

    /// As `screen`, or the nearest folder above `path` the map draws: the tree stops a few
    /// levels down, and a move into a deep folder should still land somewhere visible.
    fn screen_near(&self, path: &FsPath, xf: &Xf) -> Option<Rectangle> {
        path.ancestors().find_map(|p| self.screen(p, xf))
    }

    fn paint(&self, frame: &mut Frame, pal: &Palette, hover: Option<&FsPath>, size: Size) {
        let xf = Xf::new(self.view, size);
        let field = Rectangle::new(Point::ORIGIN, size);

        // a moved file stays hidden until its dot lands, then fades in
        let mut arrivals: HashMap<usize, f32> = HashMap::new();
        for travel in &self.travels {
            if let Some(&i) = self.index.get(&travel.to) {
                arrivals.insert(i, progress(self.now, travel.started + TRAVEL, FADE));
            }
        }

        for (i, item) in self.items.iter().enumerate().skip(1) {
            let s = xf.rect(item.rect);
            if s.width < 1.2 || s.height < 1.2 || !s.intersects(&field) {
                continue;
            }
            let alpha = arrivals.get(&i).copied().unwrap_or(1.0);
            if alpha <= 0.0 {
                continue;
            }
            let shade = &pal.shades[(item.depth - 1) % pal.shades.len()];
            let s = snap(s);
            let hovered = hover == Some(item.path.as_path());
            if item.dir {
                let focused = i == self.focus;
                frame.fill_rectangle(s.position(), s.size(), shade.fill.scale_alpha(alpha));
                let head = (item.head * xf.k).min(s.height);
                if focused && head >= 2.0 {
                    frame.fill_rectangle(s.position(), Size::new(s.width, head), pal.accent);
                }
                let edge = if focused {
                    pal.accent
                } else if hovered {
                    pal.ink
                } else {
                    pal.line
                };
                outline(frame, s, edge.scale_alpha(alpha));
                if head >= 13.0 {
                    let ink = if focused { pal.on_accent } else { shade.ink };
                    let y = s.y + head.min(40.0) / 2.0;
                    let size = human(item.size);
                    if 12.0 + guess(&item.name, 12.0) <= s.width {
                        label(frame, &item.name, Point::new(s.x + 6.0, y), 12.0, ink, false);
                        if 30.0 + guess(&item.name, 12.0) + guess(&size, 11.0) <= s.width {
                            let at = Point::new(s.x + s.width - 6.0, y);
                            label(frame, &size, at, 11.0, ink.scale_alpha(0.7), true);
                        }
                    }
                }
            } else {
                let body = Rectangle {
                    width: (s.width - 1.0).max(1.0),
                    height: (s.height - 1.0).max(1.0),
                    ..s
                };
                frame.fill_rectangle(body.position(), body.size(), shade.fill.scale_alpha(alpha));
                let edge = if hovered { pal.ink } else { pal.file_line };
                outline(frame, body, edge.scale_alpha(alpha));
                if s.width >= guess(&item.name, 11.0) + 10.0 && s.height >= 18.0 {
                    let at = Point::new(s.x + 5.0, s.y + 9.0);
                    label(frame, &item.name, at, 11.0, shade.ink.scale_alpha(0.8 * alpha), false);
                }
            }
        }

        let line = Stroke::default().with_color(pal.accent).with_width(1.0);
        let dashed = Stroke {
            line_dash: LineDash {
                segments: DASH,
                offset: 0,
            },
            ..line
        };
        for overlay in &self.overlays {
            match overlay {
                Overlay::Arrow { from, to } => {
                    if let (Some(a), Some(b)) = (self.screen_near(from, &xf), self.screen_near(to, &xf))
                        && a != b
                    {
                        arrow(frame, a.center(), b.center(), line);
                    }
                }
                Overlay::Merge { a, b } => {
                    if let (Some(a), Some(b)) = (self.screen(a, &xf), self.screen(b, &xf)) {
                        // stroke_rectangle ignores the dash, a path keeps it
                        for r in [a, b] {
                            let r = snap(r.expand(3.0));
                            let corner = Point::new(r.x + 0.5, r.y + 0.5);
                            let size = Size::new(r.width - 1.0, r.height - 1.0);
                            frame.stroke(&Path::rectangle(corner, size), dashed);
                        }
                        frame.stroke(&Path::line(a.center(), b.center()), dashed);
                    }
                }
                Overlay::Outline { paths } => {
                    for path in paths {
                        if let Some(r) = self.screen(path, &xf) {
                            outline(frame, snap(r.expand(2.0)), pal.accent);
                        }
                    }
                }
            }
        }

        for travel in &self.travels {
            let (Some(a), Some(b)) = (
                self.screen_near(&travel.from, &xf),
                self.screen_near(&travel.to, &xf),
            ) else {
                continue;
            };
            if a == b || self.now < travel.started {
                continue;
            }
            let (a, b) = (a.center(), b.center());
            let c = bend(a, b);
            let t = progress(self.now, travel.started, TRAVEL);
            if t < 1.0 {
                let e = ease(t);
                frame.stroke(&curve(a, c, b, e), line.with_color(pal.accent.scale_alpha(0.9)));
                let dot = along(a, c, b, e);
                frame.fill(&Path::circle(dot, 5.0), pal.accent);
                frame.stroke(
                    &Path::circle(dot, 9.0),
                    line.with_color(pal.accent.scale_alpha(0.4)),
                );
            } else {
                let q = progress(self.now, travel.started + TRAVEL, RING);
                let fade = pal.accent.scale_alpha(1.0 - q);
                frame.stroke(&curve(a, c, b, 1.0), line.with_color(fade));
                frame.stroke(&Path::circle(b, 6.0 + q * 18.0), line.with_color(fade));
            }
        }
    }
}

struct Painter<'a, M> {
    map: &'a Map,
    on: Box<dyn Fn(Event) -> M + 'a>,
}

#[derive(Default)]
struct Pointer {
    hover: Option<PathBuf>,
    pressed: Option<PathBuf>,
}

impl<M> canvas::Program<M, Theme, Renderer> for Painter<'_, M> {
    type State = Pointer;

    fn update(
        &self,
        state: &mut Pointer,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        let canvas::Event::Mouse(event) = event else {
            return None;
        };
        let xf = Xf::new(self.map.view, bounds.size());
        let at = cursor.position_in(bounds).map(|p| xf.world(p));
        let hit = at
            .and_then(|p| self.map.hit(p))
            .map(|i| self.map.items[i].path.clone());
        match event {
            mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft => {
                if hit == state.hover {
                    return None;
                }
                state.hover.clone_from(&hit);
                Some(canvas::Action::publish((self.on)(Event::Hover(hit))))
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                at?;
                state.pressed = hit;
                Some(canvas::Action::capture())
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                let pressed = state.pressed.take()?;
                if hit.as_ref() != Some(&pressed) {
                    return None;
                }
                let event = self.map.click(&pressed)?;
                Some(canvas::Action::publish((self.on)(event)).and_capture())
            }
            mouse::Event::WheelScrolled { delta } => {
                let at = at?;
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y * 0.16,
                    mouse::ScrollDelta::Pixels { y, .. } => y * 0.0016,
                };
                let factor = (-y).exp();
                let seen = xf.seen;
                let view = Rectangle {
                    x: at.x - (at.x - seen.x) * factor,
                    y: at.y - (at.y - seen.y) * factor,
                    width: seen.width * factor,
                    height: seen.height * factor,
                };
                Some(canvas::Action::publish((self.on)(Event::Wheel(view))).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Pointer,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let pal = Palette::new(theme);
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), pal.bg);
        self.map
            .paint(&mut frame, &pal, state.hover.as_deref(), bounds.size());
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Pointer,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.hover.is_some() && cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

struct Shade {
    fill: Color,
    ink: Color,
}

struct Palette {
    bg: Color,
    shades: [Shade; 3],
    line: Color,
    file_line: Color,
    accent: Color,
    on_accent: Color,
    ink: Color,
}

impl Palette {
    fn new(theme: &Theme) -> Self {
        let c = theme.cosmic();
        Self {
            bg: c.bg_color().into(),
            // depth steps through primary container, component and background
            shades: [
                Shade {
                    fill: c.primary_container_color().into(),
                    ink: c.on_primary_container_color().into(),
                },
                Shade {
                    fill: c.bg_component_color().into(),
                    ink: c.on_bg_component_color().into(),
                },
                Shade {
                    fill: c.bg_color().into(),
                    ink: c.on_bg_color().into(),
                },
            ],
            line: c.bg_divider().into(),
            file_line: c.small_widget_divider().into(),
            accent: c.accent_color().into(),
            on_accent: c.on_accent_color().into(),
            ink: c.on_bg_color().into(),
        }
    }
}

/// Maps the view rectangle onto the canvas, keeping its aspect. The spare room is spent on the
/// map around it rather than on empty field.
struct Xf {
    k: f32,
    /// The part of the world on the canvas.
    seen: Rectangle,
}

impl Xf {
    fn new(view: Rectangle, size: Size) -> Self {
        let k = (size.width / view.width).min(size.height / view.height);
        let (width, height) = (size.width / k, size.height / k);
        let x = settle(view.center().x - width / 2.0, width, WORLD.x, WORLD.width);
        let y = settle(view.center().y - height / 2.0, height, WORLD.y, WORLD.height);
        Self {
            k,
            seen: Rectangle {
                x,
                y,
                width,
                height,
            },
        }
    }

    fn rect(&self, r: Rectangle) -> Rectangle {
        Rectangle {
            x: (r.x - self.seen.x) * self.k,
            y: (r.y - self.seen.y) * self.k,
            width: r.width * self.k,
            height: r.height * self.k,
        }
    }

    fn world(&self, p: Point) -> Point {
        Point::new(p.x / self.k + self.seen.x, p.y / self.k + self.seen.y)
    }
}

/// Keeps a span of `len` starting at `at` inside the world span, or centres it when it is wider.
fn settle(at: f32, len: f32, start: f32, world: f32) -> f32 {
    if len >= world {
        start + (world - len) / 2.0
    } else {
        at.clamp(start, start + world - len)
    }
}

fn squarify(weights: &[f64], rect: Rectangle) -> Vec<Rectangle> {
    let mut out = vec![Rectangle::new(Point::ORIGIN, Size::ZERO); weights.len()];
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return out;
    }
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]));
    let scale = f64::from(rect.width) * f64::from(rect.height) / total;
    let (mut x, mut y) = (f64::from(rect.x), f64::from(rect.y));
    let (mut w, mut h) = (f64::from(rect.width), f64::from(rect.height));
    let mut rest = &order[..];
    while !rest.is_empty() {
        let short = w.min(h);
        let mut n = 1;
        let mut best = worst(&rest[..1], weights, short, scale);
        while n < rest.len() {
            let next = worst(&rest[..=n], weights, short, scale);
            if next > best {
                break;
            }
            best = next;
            n += 1;
        }
        let (row, tail) = rest.split_at(n);
        rest = tail;
        let area = row.iter().map(|&i| weights[i]).sum::<f64>() * scale;
        if w >= h {
            let cw = area / h;
            let mut cy = y;
            for &i in row {
                let rh = weights[i] * scale / cw;
                out[i] = Rectangle {
                    x: x as f32,
                    y: cy as f32,
                    width: cw as f32,
                    height: rh as f32,
                };
                cy += rh;
            }
            x += cw;
            w -= cw;
        } else {
            let ch = area / w;
            let mut cx = x;
            for &i in row {
                let rw = weights[i] * scale / ch;
                out[i] = Rectangle {
                    x: cx as f32,
                    y: y as f32,
                    width: rw as f32,
                    height: ch as f32,
                };
                cx += rw;
            }
            y += ch;
            h -= ch;
        }
    }
    out
}

fn worst(row: &[usize], weights: &[f64], short: f64, scale: f64) -> f64 {
    let areas = row.iter().map(|&i| weights[i] * scale);
    let sum: f64 = areas.clone().sum();
    let max = areas.clone().fold(f64::MIN, f64::max);
    let min = areas.fold(f64::MAX, f64::min);
    let short = short * short;
    (short * max / (sum * sum)).max(sum * sum / (short * min))
}

fn progress(now: Instant, start: Instant, length: Duration) -> f32 {
    (now.saturating_duration_since(start).as_secs_f32() / length.as_secs_f32()).min(1.0)
}

fn ease(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn mix(a: Rectangle, b: Rectangle, t: f32) -> Rectangle {
    Rectangle {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        width: a.width + (b.width - a.width) * t,
        height: a.height + (b.height - a.height) * t,
    }
}

/// Rounds a rectangle to whole pixels so 1 px lines stay sharp.
fn snap(r: Rectangle) -> Rectangle {
    let (x, y) = (r.x.round(), r.y.round());
    Rectangle {
        x,
        y,
        width: (r.x + r.width).round() - x,
        height: (r.y + r.height).round() - y,
    }
}

fn outline(frame: &mut Frame, r: Rectangle, color: Color) {
    if r.width < 2.0 || r.height < 2.0 {
        return;
    }
    frame.stroke_rectangle(
        Point::new(r.x + 0.5, r.y + 0.5),
        Size::new(r.width - 1.0, r.height - 1.0),
        Stroke::default().with_color(color).with_width(1.0),
    );
}

fn label(frame: &mut Frame, content: &str, at: Point, size: f32, color: Color, right: bool) {
    frame.fill_text(Text {
        content: content.to_string(),
        position: at,
        color,
        size: Pixels(size),
        font: if right {
            cosmic::font::mono()
        } else {
            cosmic::font::default()
        },
        align_x: if right {
            alignment::Horizontal::Right.into()
        } else {
            alignment::Horizontal::Left.into()
        },
        align_y: alignment::Vertical::Center,
        ..Text::default()
    });
}

/// A rough text width, enough to decide whether a label fits.
fn guess(content: &str, size: f32) -> f32 {
    content.chars().count() as f32 * size * 0.6
}

fn human(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        let gb = mb / 1024.0;
        if gb >= 10.0 {
            format!("{gb:.0} GB")
        } else {
            format!("{gb:.1} GB")
        }
    } else {
        format!("{} MB", mb.round().max(1.0))
    }
}

/// The control point that bends a route a quarter of its length to one side.
fn bend(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    Point::new((a.x + b.x) / 2.0 - dy * 0.25, (a.y + b.y) / 2.0 + dx * 0.25)
}

fn along(a: Point, c: Point, b: Point, t: f32) -> Point {
    let u = 1.0 - t;
    Point::new(
        u * u * a.x + 2.0 * u * t * c.x + t * t * b.x,
        u * u * a.y + 2.0 * u * t * c.y + t * t * b.y,
    )
}

/// The route from `a` to `b` drawn up to `t`.
fn curve(a: Point, c: Point, b: Point, t: f32) -> Path {
    Path::new(|p| {
        p.move_to(a);
        for n in 1..=32 {
            p.line_to(along(a, c, b, t * n as f32 / 32.0));
        }
    })
}

fn arrow(frame: &mut Frame, a: Point, b: Point, stroke: Stroke<'_>) {
    let c = bend(a, b);
    frame.stroke(&curve(a, c, b, 1.0), stroke);
    let angle = (b.y - c.y).atan2(b.x - c.x);
    let wing = |turn: f32| {
        Point::new(
            b.x - 9.0 * (angle + turn).cos(),
            b.y - 9.0 * (angle + turn).sin(),
        )
    };
    let head = Path::new(|p| {
        p.move_to(wing(-0.45));
        p.line_to(b);
        p.line_to(wing(0.45));
    });
    frame.stroke(&head, stroke);
}
