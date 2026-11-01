//! The system palette as a COSMIC theme. The colours are read from a TOML file at runtime
//! (default `~/.config/yorha/palette.toml`), so the window follows the palette and this source
//! carries no colour of its own.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::SystemTime;

use cosmic::cosmic_theme::palette::{Srgb, Srgba};
use cosmic::cosmic_theme::{Component, CornerRadii, ThemeBuilder};
use cosmic::iced::{Background, Border, Color, Shadow, Vector};
use cosmic::iced::widget::container;
use cosmic::widget::icon;
use cosmic::{theme, widget};
use serde::Deserialize;

pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("yorha")
        .join("palette.toml")
}

#[derive(Deserialize)]
struct File {
    roles: HashMap<String, String>,
    #[serde(default)]
    extras: HashMap<String, String>,
    #[serde(default)]
    ramp: HashMap<String, String>,
    #[serde(default)]
    derived: HashMap<String, String>,
}

/// The tones the window spends, each with its job in the design manifest.
#[derive(Clone, Copy, Debug)]
pub struct Tones {
    /// the field
    base: Srgba,
    /// panel bodies
    highlight: Srgba,
    /// row plates, resting buttons, wells, dividers inside panels
    shadow: Srgba,
    /// the one dark: selection, current tab, header slabs
    accent: Srgba,
    /// ink
    text: Srgba,
    /// light ink on the dark
    highlight_2: Srgba,
    /// secondary text, unfocused selection
    border_mid: Srgba,
    /// disabled text
    text_dim: Srgba,
    /// new and blocked
    urgent: Srgba,
    /// the category edge
    gold: Srgba,
    /// drop shadows and rails
    text_faint: Srgba,
    /// ornament on the field, light plates
    base_dim: Srgba,
    /// rail staples and triads
    dot: Srgba,
    /// solid ink marks: the cursor lozenge, bullets, the header plate square
    selected: Srgba,
}

/// The tones of the palette theme in use, for the fork's own widgets; None under any other theme.
static LADDER: RwLock<Option<Tones>> = RwLock::new(None);

pub fn ladder() -> Option<Tones> {
    *LADDER.read().unwrap()
}

pub fn clear() {
    *LADDER.write().unwrap() = None;
}

impl Tones {
    pub fn highlight(&self) -> Color {
        self.highlight.into()
    }
    pub fn shadow(&self) -> Color {
        self.shadow.into()
    }
    pub fn accent(&self) -> Color {
        self.accent.into()
    }
    pub fn text(&self) -> Color {
        self.text.into()
    }
    pub fn light(&self) -> Color {
        self.highlight_2.into()
    }
    pub fn secondary(&self) -> Color {
        self.border_mid.into()
    }
    pub fn urgent(&self) -> Color {
        self.urgent.into()
    }
    pub fn faint(&self) -> Color {
        self.text_faint.into()
    }
    pub fn base_dim(&self) -> Color {
        self.base_dim.into()
    }
    pub fn dot(&self) -> Color {
        self.dot.into()
    }
    pub fn solid(&self) -> Color {
        self.selected.into()
    }
    pub fn gold(&self) -> Color {
        self.gold.into()
    }
    pub fn dim(&self) -> Color {
        self.text_dim.into()
    }
}

/// A panel body on the field: the panel tone with the game's offset drop shadow (round 2: 2px in
/// the plate-shadow tone).
pub fn panel() -> theme::Container<'static> {
    theme::Container::custom(|theme| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.highlight())),
            text_color: Some(t.text()),
            shadow: Shadow {
                color: t.shadow(),
                offset: Vector::new(2.0, 2.0),
                blur_radius: 0.0,
            },
            ..Default::default()
        },
        None => container::Style {
            background: Some(Background::Color(theme.cosmic().bg_color().into())),
            ..Default::default()
        },
    })
}

/// A header slab: the one dark with light ink.
pub fn slab() -> theme::Container<'static> {
    theme::Container::custom(|theme| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.accent())),
            text_color: Some(t.light()),
            icon_color: Some(t.light()),
            ..Default::default()
        },
        None => container::Style {
            text_color: Some(theme.cosmic().on_bg_color().into()),
            ..Default::default()
        },
    })
}

/// A strip along the window edge: the plate tone.
pub fn strip() -> theme::Container<'static> {
    theme::Container::custom(|theme| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.shadow())),
            text_color: Some(t.text()),
            ..Default::default()
        },
        None => container::Style {
            background: Some(Background::Color(theme.cosmic().bg_component_color().into())),
            ..Default::default()
        },
    })
}

/// A row plate on a panel (round 2: the lighter `base_dim`).
pub fn row_plate() -> theme::Container<'static> {
    theme::Container::custom(|theme| match ladder() {
        Some(t) => container::Style {
            background: Some(Background::Color(t.base_dim())),
            text_color: Some(t.text()),
            ..Default::default()
        },
        None => container::Style {
            background: Some(Background::Color(theme.cosmic().bg_component_color().into())),
            ..Default::default()
        },
    })
}

/// The window's default text size: the law's 12px under the palette, COSMIC's 14 otherwise.
pub fn text_size() -> f32 {
    if ladder().is_some() { 12.0 } else { 14.0 }
}

/// Body text. Under the palette it is the law's data size, 12px (type is 9 / 12 / 24 and
/// nothing between 13 and 23); otherwise COSMIC's own 14px.
pub fn body<'a>(
    text: impl Into<std::borrow::Cow<'a, str>> + 'a,
) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    let t = widget::text::body(text);
    if ladder().is_some() {
        t.size(12.0)
            .line_height(cosmic::iced::widget::text::LineHeight::Absolute(17.0.into()))
    } else {
        t
    }
}

/// Secondary text: captions, dates, sizes.
pub fn secondary() -> theme::Text {
    match ladder() {
        Some(t) => theme::Text::Color(t.secondary()),
        None => theme::Text::Default,
    }
}

/// A button that does the thing: the panel tone with an ink border, the one dark while pressed.
/// `quiet` is the button beside it (Keep, Skip, Not now): no plate, the border shows on hover.
pub fn button(quiet: bool) -> theme::Button {
    let look = move |hovered: bool, pressed: bool, theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        let Some(t) = ladder() else {
            let c = theme.cosmic();
            s.background = Some(Background::Color(c.button_bg_color().into()));
            s.text_color = Some(c.on_bg_color().into());
            return s;
        };
        if pressed {
            s.background = Some(Background::Color(t.accent()));
            s.text_color = Some(t.light());
            s.icon_color = Some(t.light());
            return s;
        }
        if !quiet {
            s.background = Some(Background::Color(t.highlight()));
        }
        s.text_color = Some(t.text());
        s.icon_color = Some(t.text());
        if !quiet || hovered {
            s.border_width = 1.0;
            s.border_color = t.text();
        }
        s
    };
    theme::Button::Custom {
        active: Box::new(move |_, theme| look(false, false, theme)),
        disabled: Box::new(move |theme| look(false, false, theme)),
        hovered: Box::new(move |_, theme| look(true, false, theme)),
        pressed: Box::new(move |_, theme| look(true, true, theme)),
    }
}

/// The button that does the thing on a card, a row or the Tidy list.
pub fn act<'a, M>(label: impl Into<std::borrow::Cow<'a, str>>) -> widget::button::TextButton<'a, M> {
    widget::button::text(label).class(button(false))
}

/// The buttons beside it: Keep, Skip, Not now, More.
pub fn quiet<'a, M>(label: impl Into<std::borrow::Cow<'a, str>>) -> widget::button::TextButton<'a, M> {
    widget::button::text(label).class(button(true))
}

/// A row to pick on a sheet: a plate at rest, an ink border on hover, the one dark while pressed.
pub fn plate_button() -> theme::Button {
    let look = move |hovered: bool, pressed: bool, theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        let Some(t) = ladder() else {
            let c = theme.cosmic();
            s.background = Some(Background::Color(c.button_bg_color().into()));
            s.text_color = Some(c.on_bg_color().into());
            return s;
        };
        let (fill, ink) = if pressed {
            (t.accent(), t.light())
        } else {
            (t.base_dim(), t.text())
        };
        s.background = Some(Background::Color(fill));
        s.text_color = Some(ink);
        s.icon_color = Some(ink);
        if hovered && !pressed {
            s.border_width = 1.0;
            s.border_color = t.text();
        }
        s
    };
    theme::Button::Custom {
        active: Box::new(move |_, theme| look(false, false, theme)),
        disabled: Box::new(move |theme| look(false, false, theme)),
        hovered: Box::new(move |_, theme| look(true, false, theme)),
        pressed: Box::new(move |_, theme| look(true, true, theme)),
    }
}

/// The sidebar: plates at rest, an ink border on hover, the one dark on the current place.
pub fn nav_look() -> theme::SegmentedButton {
    use widget::segmented_button::{Appearance, ItemAppearance, ItemStatusAppearance};
    theme::SegmentedButton::Custom(Box::new(|_| {
        let Some(t) = ladder() else {
            return Appearance::default();
        };
        let item = |border: Border| ItemAppearance { border };
        let status = |bg: Color, ink: Color, edge: Border| ItemStatusAppearance {
            background: Some(Background::Color(bg)),
            first: item(edge),
            middle: item(edge),
            last: item(edge),
            text_color: ink,
        };
        let none = Border::default();
        let ink = Border {
            color: t.text(),
            width: 1.0,
            radius: 0.0.into(),
        };
        Appearance {
            background: None,
            border: none,
            active_width: 0.0,
            active: status(t.accent(), t.light(), none),
            inactive: status(t.base_dim(), t.text(), none),
            hover: status(t.base_dim(), t.text(), ink),
            pressed: status(t.accent(), t.light(), none),
        }
    }))
}

/// A list row: a plate at rest, the border promotes to ink on hover, the one dark when selected.
pub fn row_appearance(mut s: widget::button::Style, selected: bool, highlighted: bool) -> widget::button::Style {
    if let Some(t) = ladder() {
        if !selected {
            s.background = Some(Background::Color(t.base_dim()));
            s.text_color = Some(t.text());
            s.icon_color = Some(t.text());
        }
        if highlighted && !selected {
            s.border_width = 1.0;
            s.border_color = t.text();
        }
        s.border_radius = 0.0.into();
    }
    s
}

/// The middle of the selection's arrival flash: the fill steps from `accent` to `border_mid`.
pub fn flash(mut s: widget::button::Style, selected: bool, peak: bool) -> widget::button::Style {
    if let Some(t) = ladder()
        && selected
        && peak
    {
        s.background = Some(Background::Color(t.secondary()));
    }
    s
}

fn color(table: &HashMap<String, String>, key: &str) -> Option<Srgba> {
    let rgb = Srgb::<u8>::from_str(table.get(key)?.trim()).ok()?;
    Some(Srgba::from(rgb.into_format::<f32>()))
}

fn tones(path: &Path) -> Option<Tones> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| log::warn!("palette {}: {err}", path.display()))
        .ok()?;
    let file: File = toml::from_str(&text)
        .map_err(|err| log::warn!("palette {}: {err}", path.display()))
        .ok()?;
    let role = |k| color(&file.roles, k);
    Some(Tones {
        base: role("base")?,
        highlight: role("highlight")?,
        shadow: role("shadow")?,
        accent: role("accent")?,
        text: role("text")?,
        highlight_2: color(&file.derived, "highlight_2").or(role("highlight"))?,
        border_mid: color(&file.ramp, "border_mid").or(role("accent"))?,
        text_dim: color(&file.derived, "text_dim").or(role("shadow"))?,
        urgent: role("urgent")?,
        gold: color(&file.extras, "gold").or(role("urgent"))?,
        text_faint: color(&file.derived, "text_faint").or(role("shadow"))?,
        base_dim: color(&file.derived, "base_dim").or(role("shadow"))?,
        dot: color(&file.ramp, "dot").or(role("accent"))?,
        selected: color(&file.ramp, "selected").or(role("text"))?,
    })
}

/// A component in the palette's terms: a plate at rest, the border promotes to ink on hover,
/// the one dark while pressed or selected.
fn plate(c: &mut Component, t: &Tones, base: Srgba, on: Srgba) {
    c.base = base;
    c.on = on;
    c.hover = base;
    c.pressed = t.accent;
    c.selected = t.accent;
    c.selected_text = t.highlight_2;
    c.focus = t.text;
    c.divider = t.shadow;
    c.border = t.text;
    c.disabled = base;
    c.on_disabled = t.text_dim;
    c.disabled_border = t.text_dim;
}

/// The theme for the palette at `path`, or None when the file is missing or incomplete.
pub fn theme(path: &Path) -> Option<theme::Theme> {
    let t = tones(path)?;
    let zero = [0.0; 4];
    let mut c = ThemeBuilder::light()
        .bg_color(t.base)
        .primary_container_bg(t.highlight)
        .accent(t.accent.color)
        .text_tint(t.text.color)
        .neutral_tint(t.border_mid.color)
        .destructive(t.urgent.color)
        .warning(t.gold.color)
        .success(t.accent.color)
        .corner_radii(CornerRadii {
            radius_0: zero,
            radius_xs: zero,
            radius_s: zero,
            radius_m: zero,
            radius_l: zero,
            radius_xl: zero,
        })
        .build();

    c.name = "palette".to_string();
    let clear = Srgba::new(t.base.red, t.base.green, t.base.blue, 0.0);
    plate(&mut c.button, &t, t.shadow, t.text);
    plate(&mut c.icon_button, &t, clear, t.text);
    plate(&mut c.text_button, &t, clear, t.text);
    plate(&mut c.list_button, &t, clear, t.text);
    plate(&mut c.link_button, &t, clear, t.text);
    // The suggested action is a plate with an ink border, never the dark: dark means engaged.
    plate(&mut c.accent_button, &t, t.shadow, t.text);
    plate(&mut c.accent, &t, t.accent, t.highlight_2);
    c.accent.hover = t.accent;
    plate(&mut c.success, &t, t.accent, t.highlight_2);
    plate(&mut c.destructive, &t, t.urgent, t.highlight_2);
    plate(&mut c.destructive_button, &t, t.urgent, t.highlight_2);
    plate(&mut c.warning, &t, t.gold, t.text);
    plate(&mut c.warning_button, &t, t.gold, t.text);
    c.is_dark = false;

    // The containers are not public fields: set them through the theme's own serde form.
    let mut value = serde_json::to_value(&c).ok()?;
    for (key, ground, transparent) in [
        ("background", t.base, false),
        ("primary", t.highlight, false),
        ("secondary", t.highlight, false),
        ("transparent_background", t.base, true),
        ("transparent_primary", t.highlight, true),
        ("transparent_secondary", t.highlight, true),
    ] {
        let mut container = match key.trim_start_matches("transparent_") {
            "background" => c.background(transparent).clone(),
            "primary" => c.primary(transparent).clone(),
            _ => c.secondary(transparent).clone(),
        };
        container.base = ground;
        container.on = t.text;
        container.divider = t.shadow;
        container.small_widget = t.shadow;
        plate(&mut container.component, &t, t.shadow, t.text);
        value[key] = serde_json::to_value(container).ok()?;
    }
    let c: cosmic::cosmic_theme::Theme = serde_json::from_value(value)
        .map_err(|err| log::warn!("palette theme: {err}"))
        .ok()?;

    let mut theme = theme::Theme::custom(Arc::new(c));
    theme.theme_type.prefer_dark(Some(false));
    *LADDER.write().unwrap() = Some(t);
    Some(theme)
}

/// Under the palette theme: `font` everywhere and icons from `icons`. Under any other theme: the
/// desktop's own font and icon theme.
pub fn dress(font: &str, icons: &str) {
    let on = ladder().is_some();
    let tk = cosmic::config::CosmicTk::default();
    let pick = |plain: String| if on { font.to_string() } else { plain };
    let mut live = cosmic::config::COSMIC_TK.write().unwrap();
    live.interface_font.family = pick(tk.interface_font.family);
    live.monospace_font.family = pick(tk.monospace_font.family);
    drop(live);
    let theme = if on { icons.to_string() } else { cosmic::config::icon_theme() };
    if theme != cosmic::icon_theme::default() {
        cosmic::icon_theme::set_default(theme);
        crate::mime_icon::forget();
    }
}

/// Icons by name prefer SVG, which skips a bitmap icon theme; under the palette take what the theme has.
pub fn prefer_svg() -> bool {
    ladder().is_none()
}

/// The tone curve of the launcher's TintIcon (snek's pick C, 2026-10-09): coverage floor, mid-tone
/// gamma, and a per-icon contrast stretch.
const FLOOR: f32 = 0.2;
const GAMMA: f32 = 1.6;

static TINTED: LazyLock<Mutex<HashMap<(PathBuf, u16, [u8; 3]), icon::Handle>>> =
    LazyLock::new(Default::default);

/// An icon redrawn in one ink, as the launcher draws app icons: its own darkness becomes the ink's
/// coverage, so no foreign hue reaches the window. Bitmaps are scaled by whole steps with nearest
/// pixels, so a 32px icon stays crisp at 64. Unchanged under any other theme.
/// `light` draws it in the light ink instead, for an icon on the selection's dark fill.
pub fn tint_ink(handle: icon::Handle, size: u16, light: bool) -> icon::Handle {
    let Some(t) = ladder() else {
        return handle;
    };
    let path = match &handle.data {
        icon::Data::Image(cosmic::iced::advanced::image::Handle::Path(_, p)) => p.clone(),
        icon::Data::Svg(svg) => match svg.data() {
            cosmic::iced::advanced::svg::Data::Path(p) => p.clone(),
            _ => return handle,
        },
        _ => return handle,
    };
    let ink: Srgb<u8> = if light { t.highlight_2.color } else { t.text.color }.into_format::<u8>();
    let key = (path, size, [ink.red, ink.green, ink.blue]);
    if let Some(h) = TINTED.lock().unwrap().get(&key) {
        return h.clone();
    }
    let Some(rgba) = raster(&key.0, size) else {
        return handle;
    };
    let tinted = ink_over(rgba, key.2);
    let h = icon::from_raster_pixels(tinted.width(), tinted.height(), tinted.into_raw());
    TINTED.lock().unwrap().insert(key, h.clone());
    h
}

fn raster(path: &Path, size: u16) -> Option<image::RgbaImage> {
    let size = u32::from(size.max(1));
    if path.extension().is_some_and(|e| e == "svg") {
        let data = std::fs::read(path).ok()?;
        let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
        let mut pix = resvg::tiny_skia::Pixmap::new(size, size)?;
        let s = tree.size();
        let k = size as f32 / s.width().max(s.height());
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pix.as_mut());
        // tiny-skia is premultiplied; undo it so the tint reads true luminance
        let mut img = image::RgbaImage::from_raw(size, size, pix.take())?;
        for p in img.pixels_mut() {
            let a = p[3] as f32 / 255.0;
            if a > 0.0 {
                for c in 0..3 {
                    p[c] = (p[c] as f32 / a).min(255.0) as u8;
                }
            }
        }
        return Some(img);
    }
    let img = image::open(path).ok()?.into_rgba8();
    let step = (size / img.width().max(1)).max(1);
    Some(if step > 1 {
        image::imageops::resize(&img, img.width() * step, img.height() * step, image::imageops::FilterType::Nearest)
    } else {
        img
    })
}

fn ink_over(mut img: image::RgbaImage, ink: [u8; 3]) -> image::RgbaImage {
    let lum = |p: &image::Rgba<u8>| {
        (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0
    };
    // the icon's own darkest and lightest opaque parts span the range, on an 8 by 8 sample
    let (w, h) = img.dimensions();
    let (mut lo, mut hi) = (1.0f32, 0.0f32);
    for y in 0..8 {
        for x in 0..8 {
            let p = img.get_pixel((x * 2 + 1) * w / 16, (y * 2 + 1) * h / 16);
            if p[3] > 127 {
                lo = lo.min(lum(p));
                hi = hi.max(lum(p));
            }
        }
    }
    let stretch = hi - lo > 0.15;
    for p in img.pixels_mut() {
        let mut l = lum(p);
        if stretch {
            l = ((l - lo) / (hi - lo)).clamp(0.0, 1.0);
        }
        let cov = FLOOR + (1.0 - FLOOR) * (1.0 - l).powf(GAMMA);
        let a = (p[3] as f32 * cov).round() as u8;
        *p = image::Rgba([ink[0], ink[1], ink[2], a]);
    }
    img
}

/// A path as typed in Settings, with a leading `~` meaning home.
pub fn expand(input: &str) -> PathBuf {
    match input.strip_prefix("~/") {
        Some(rest) => dirs::home_dir().unwrap_or_default().join(rest),
        None => PathBuf::from(input),
    }
}

/// When the palette file last changed, for following edits without a restart.
pub fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
