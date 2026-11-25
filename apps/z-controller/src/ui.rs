use std::collections::BTreeMap;
use std::f32::consts::PI;

use egui::epaint::{Mesh, Shadow, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Rect, RichText,
    Rounding, Stroke, TextStyle, Vec2,
};
use z_core::battery::BatteryStatus;
use z_core::model::{ControllerFamily, DisconnectPolicy, LogicalButton, VirtualTargetKind};
use z_core::profile::{Profile, ThemeMode};
use z_platform_windows::drivers::query_process_memory_bytes;

use crate::diagram::draw_controller_diagram;
use crate::engine::{
    EngineCommand, EngineHandle, EngineSnapshot, PhysicalSnapshot,
};

// WT logo accent constants shared across Dark and Light ThemePalette modes.
pub const LOGO_BLUE: Color32 = Color32::from_rgb(84, 185, 242); // #54B9F2
pub const LOGO_BLUE_READABLE: Color32 = Color32::from_rgb(47, 155, 232); // #2F9BE8 (light-mode thin lines & small text)
#[allow(dead_code)]
pub const LOGO_BLUE_MID: Color32 = Color32::from_rgb(124, 208, 250); // #7CD0FA
#[allow(dead_code)]
pub const LOGO_BLUE_LIGHT: Color32 = Color32::from_rgb(165, 230, 255); // #A5E6FF
pub const LOGO_PINK: Color32 = Color32::from_rgb(247, 168, 216); // #F7A8D8
#[allow(dead_code)]
pub const LOGO_PINK_LIGHT: Color32 = Color32::from_rgb(255, 199, 227); // #FFC7E3
pub const LOGO_LAVENDER: Color32 = Color32::from_rgb(216, 166, 255); // #D8A6FF

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub bg: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint_bg: Color32,
    pub widget: Color32,
    pub widget_hover: Color32,
    pub widget_active: Color32,
    /// Accent used for thin strokes, borders, and small text (#54B9F2 dark, #2F9BE8 light).
    pub accent_line: Color32,
    pub pill_blue_fill: Color32,
    pub pill_pink_fill: Color32,
    pub row_selected_fill: Color32,
    pub diagram_body: Color32,
    pub diagram_body_stroke: Color32,
    pub diagram_seam: Color32,
    pub diagram_ctrl_fill: Color32,
    pub diagram_ctrl_stroke: Color32,
    pub crosshair: Color32,
}

impl ThemePalette {
    #[must_use]
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                bg: Color32::from_rgb(10, 10, 16),        // #0A0A10
                panel: Color32::from_rgb(20, 20, 31),     // #14141F
                border: Color32::from_rgb(38, 38, 58),    // #26263A
                text: Color32::from_rgb(237, 237, 242),   // #EDEDF2
                muted: Color32::from_rgb(154, 154, 176),  // #9A9AB0
                faint_bg: Color32::from_rgb(16, 16, 26),
                widget: Color32::from_rgb(24, 24, 38),
                widget_hover: Color32::from_rgb(31, 31, 49),
                widget_active: Color32::from_rgb(24, 34, 48),
                accent_line: LOGO_BLUE,
                pill_blue_fill: Color32::from_rgb(18, 28, 40),
                pill_pink_fill: Color32::from_rgb(38, 22, 34),
                row_selected_fill: Color32::from_rgb(20, 30, 44),
                diagram_body: Color32::from_rgb(19, 19, 31),
                diagram_body_stroke: Color32::from_rgb(56, 56, 86),
                diagram_seam: Color32::from_rgb(32, 32, 50),
                diagram_ctrl_fill: Color32::from_rgb(24, 24, 40),
                diagram_ctrl_stroke: Color32::from_rgb(54, 54, 82),
                crosshair: Color32::from_rgb(28, 28, 44),
            },
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(238, 244, 250),     // #EEF4FA
                panel: Color32::from_rgb(255, 255, 255),  // #FFFFFF
                border: Color32::from_rgb(217, 228, 240), // #D9E4F0
                text: Color32::from_rgb(28, 39, 51),      // #1C2733
                muted: Color32::from_rgb(95, 113, 134),   // #5F7186
                faint_bg: Color32::from_rgb(244, 248, 252),
                widget: Color32::from_rgb(245, 248, 252),
                widget_hover: Color32::from_rgb(233, 242, 250),
                widget_active: Color32::from_rgb(220, 235, 248),
                accent_line: LOGO_BLUE_READABLE,          // #2F9BE8
                pill_blue_fill: Color32::from_rgb(235, 246, 255),
                pill_pink_fill: Color32::from_rgb(253, 241, 249),
                row_selected_fill: Color32::from_rgb(235, 245, 255),
                diagram_body: Color32::from_rgb(244, 248, 253),
                diagram_body_stroke: Color32::from_rgb(190, 206, 224),
                diagram_seam: Color32::from_rgb(217, 228, 240),
                diagram_ctrl_fill: Color32::from_rgb(255, 255, 255),
                diagram_ctrl_stroke: Color32::from_rgb(196, 211, 228),
                crosshair: Color32::from_rgb(217, 228, 240),
            },
        }
    }
}

// Consistent 4 / 8 / 12 / 16 spacing scale applied uniformly across all panels, cards, and grids.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;

const SEGOE_UI_FONT_PATH: &str = r"C:\Windows\Fonts\segoeui.ttf";
const CARD_ROUNDING: f32 = 8.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    OpenProfileEditor {
        controller_id: String,
        profile_id: String,
    },
}

pub struct AppUiState {
    pub selected_controller: Option<String>,
    pub editing_profile_id: Option<String>,
    pub draft_profile: Option<Profile>,
    pub draft_dirty: bool,
    pub selected_diagram_button: Option<LogicalButton>,
    pub last_dynamic_rect_main: Option<Rect>,
    pub last_dynamic_rect_editor: Option<Rect>,
    pub desired_compact_logical_h: Option<f32>,
    pub applied_theme: Option<ThemeMode>,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            selected_controller: None,
            editing_profile_id: None,
            draft_profile: None,
            draft_dirty: false,
            selected_diagram_button: Some(LogicalButton::South),
            last_dynamic_rect_main: None,
            last_dynamic_rect_editor: None,
            desired_compact_logical_h: None,
            applied_theme: None,
        }
    }
}

/// Computes a deterministic hash of all static (non-live-input) UI state so
/// engine snapshots that only update `physical_state` / `virtual_output` can
/// reuse cached static chrome and invalidate only the dynamic input region.
pub fn static_ui_hash(snap: &EngineSnapshot, ui: &AppUiState) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    ui.selected_controller.hash(&mut h);
    ui.editing_profile_id.hash(&mut h);
    ui.draft_dirty.hash(&mut h);
    ui.selected_diagram_button.hash(&mut h);

    if let Some(p) = &ui.draft_profile {
        p.id.hash(&mut h);
        p.name.hash(&mut h);
        (p.family as u8).hash(&mut h);
        (p.target as u8).hash(&mut h);
        p.cloak_physical.hash(&mut h);
        (p.disconnect_policy as u8).hash(&mut h);
        p.swap_ab_xy.hash(&mut h);
        p.rumble_enabled.hash(&mut h);
        p.left_deadzone.to_bits().hash(&mut h);
        p.right_deadzone.to_bits().hash(&mut h);
        for (k, v) in &p.button_remap {
            (*k as u8).hash(&mut h);
            v.map(|b| b as u8).hash(&mut h);
        }
    }

    snap.driver_status.vigem_installed.hash(&mut h);
    snap.driver_status.hidhide_installed.hash(&mut h);
    snap.driver_status.hidhide_accessible.hash(&mut h);
    snap.driver_status.reboot_recommended.hash(&mut h);
    snap.last_status_message.hash(&mut h);
    (snap.config.theme_mode as u8).hash(&mut h);
    snap.config.default_profile_id.hash(&mut h);
    snap.config.profiles.len().hash(&mut h);

    for c in &snap.controllers {
        c.canonical_id.hash(&mut h);
        c.connected.hash(&mut h);
        c.enabled.hash(&mut h);
        c.show_live_input.hash(&mut h);
        c.profile_id.hash(&mut h);
        (c.target_kind as u8).hash(&mut h);
        c.cloaked.hash(&mut h);
        c.virtual_active.hash(&mut h);
        c.battery.percent.hash(&mut h);
        (c.battery.status as u8).hash(&mut h);
        c.present_buttons_mask.hash(&mut h);
    }
    h.finish()
}

pub fn apply_theme_visuals(ctx: &egui::Context, mode: ThemeMode) {
    let pal = ThemePalette::for_mode(mode);
    let mut style = (*ctx.style()).clone();
    let mut visuals = match mode {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    visuals.dark_mode = mode == ThemeMode::Dark;
    visuals.panel_fill = pal.bg;
    visuals.window_fill = pal.panel;
    visuals.extreme_bg_color = pal.bg;
    visuals.faint_bg_color = pal.faint_bg;
    visuals.code_bg_color = pal.bg;
    visuals.override_text_color = Some(pal.text);
    visuals.window_rounding = Rounding::same(8.0);
    visuals.menu_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.window_shadow = Shadow::NONE;
    visuals.popup_shadow = Shadow::NONE;

    visuals.widgets.noninteractive.bg_fill = pal.panel;
    visuals.widgets.noninteractive.weak_bg_fill = pal.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.muted);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = pal.widget;
    visuals.widgets.inactive.weak_bg_fill = pal.widget;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = pal.widget_hover;
    visuals.widgets.hovered.weak_bg_fill = pal.widget_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = pal.widget_active;
    visuals.widgets.active.weak_bg_fill = pal.widget_active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, LOGO_PINK);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    visuals.widgets.open.bg_fill = pal.widget_hover;
    visuals.widgets.open.weak_bg_fill = pal.widget_hover;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.open.rounding = Rounding::same(6.0);

    let sel_tint = if mode == ThemeMode::Dark {
        Color32::from_rgba_unmultiplied(84, 185, 242, 42)
    } else {
        Color32::from_rgba_unmultiplied(84, 185, 242, 64)
    };
    visuals.selection.bg_fill = sel_tint;
    visuals.selection.stroke = Stroke::new(1.0_f32, pal.accent_line);

    style.visuals = visuals;
    style.animation_time = 0.0;
    ctx.set_style(style);
}

pub fn configure_neon_theme_and_fonts(ctx: &egui::Context) {
    if let Ok(bytes) = std::fs::read(SEGOE_UI_FONT_PATH) {
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("segoe_ui".to_owned(), FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
        ctx.set_fonts(fonts);
    }

    let mut style = (*ctx.style()).clone();
    style.text_styles = BTreeMap::from([
        (TextStyle::Heading, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ]);
    style.animation_time = 0.0;
    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(SPACE_MD, SPACE_XS + 1.0);
    ctx.set_style(style);
    apply_theme_visuals(ctx, ThemeMode::Dark);
}

#[inline]
pub fn battery_bar_color(percent: Option<u8>) -> Color32 {
    match percent {
        Some(p) if p <= 20 => LOGO_PINK,
        _ => LOGO_BLUE,
    }
}

#[inline]
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let u = t.clamp(0.0, 1.0);
    let r = (a.r() as f32 + (b.r() as f32 - a.r() as f32) * u).round() as u8;
    let g = (a.g() as f32 + (b.g() as f32 - a.g() as f32) * u).round() as u8;
    let bl = (a.b() as f32 + (b.b() as f32 - a.b() as f32) * u).round() as u8;
    Color32::from_rgb(r, g, bl)
}

#[inline]
pub fn normalize_stick_axis(raw: i16) -> f32 {
    (raw as f32 / 32767.0).clamp(-1.0, 1.0)
}

#[inline]
pub fn normalize_trigger_axis(raw: i16) -> f32 {
    if raw <= 0 {
        0.0
    } else {
        (raw as f32 / 32767.0).clamp(0.0, 1.0)
    }
}

fn push_rounded_rect_perimeter(rect: Rect, radius: f32, out: &mut Vec<(Pos2, Vec2)>) {
    let r = radius.clamp(0.0, 0.5 * rect.width().min(rect.height()));
    const SEGS: usize = 7;
    let corners = [
        (Pos2::new(rect.left() + r, rect.top() + r), PI, 1.5 * PI),
        (Pos2::new(rect.right() - r, rect.top() + r), 1.5 * PI, 2.0 * PI),
        (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0, 0.5 * PI),
        (Pos2::new(rect.left() + r, rect.bottom() - r), 0.5 * PI, PI),
    ];
    for (center, a0, a1) in corners {
        for k in 0..=SEGS {
            let theta = a0 + (a1 - a0) * (k as f32 / SEGS as f32);
            let n = Vec2::new(theta.cos(), theta.sin());
            out.push((center + n * r, n));
        }
    }
}

fn paint_gradient_rounded_rect_stroke(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    stroke_width: f32,
    c_start: Color32,
    c_mid: Color32,
    c_end: Color32,
    alpha: u8,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 || alpha == 0 {
        return;
    }
    let mut ring = Vec::with_capacity(32);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 4 {
        return;
    }

    let hw = stroke_width * 0.5;
    let aa = 0.75_f32;
    let offsets = [-hw - aa, -hw, hw, hw + aa];
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);

    let mut mesh = Mesh::default();
    mesh.vertices.reserve(n * 4);
    mesh.indices.reserve(n * 18);

    for (p, norm) in &ring {
        let t = (((p.x - rect.left()) / w) * 0.65 + ((p.y - rect.top()) / h) * 0.35).clamp(0.0, 1.0);
        let c = if t <= 0.5 {
            lerp_color(c_start, c_mid, t * 2.0)
        } else {
            lerp_color(c_mid, c_end, (t - 0.5) * 2.0)
        };
        let core = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
        let edge = Color32::TRANSPARENT;

        for (band_idx, &off) in offsets.iter().enumerate() {
            let color = if band_idx == 1 || band_idx == 2 { core } else { edge };
            mesh.vertices.push(Vertex {
                pos: *p + *norm * off,
                uv: WHITE_UV,
                color,
            });
        }
    }

    for i in 0..n {
        let j = (i + 1) % n;
        let b0 = (i * 4) as u32;
        let b1 = (j * 4) as u32;
        for band in 0..3u32 {
            let i0 = b0 + band;
            let i1 = b0 + band + 1;
            let i2 = b1 + band;
            let i3 = b1 + band + 1;
            mesh.indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    painter.add(egui::Shape::mesh(mesh));
}

fn paint_active_card_neon_edge_and_glow(painter: &egui::Painter, rect: Rect, rounding: f32) {
    // Static layered translucent outer glow (strictly outside card edge) + 1.5px pastel blue -> lavender -> pink edge.
    for (expand, width, alpha) in [
        (4.0_f32, 4.0_f32, 12u8),
        (2.0_f32, 2.4_f32, 26u8),
        (0.9_f32, 1.4_f32, 52u8),
    ] {
        paint_gradient_rounded_rect_stroke(
            painter,
            rect.expand(expand),
            rounding + expand,
            width,
            LOGO_BLUE,
            LOGO_LAVENDER,
            LOGO_PINK,
            alpha,
        );
    }
    paint_gradient_rounded_rect_stroke(
        painter,
        rect,
        rounding,
        1.5_f32,
        LOGO_BLUE,
        LOGO_LAVENDER,
        LOGO_PINK,
        245,
    );
}

fn draw_aligned_divider(ui: &mut egui::Ui, left: f32, right: f32, pal: &ThemePalette) {
    let y = ui.cursor().top();
    ui.painter()
        .line_segment([Pos2::new(left, y), Pos2::new(right, y)], Stroke::new(1.0_f32, pal.border));
    ui.add_space(SPACE_SM);
}

fn draw_theme_toggle_button(ui: &mut egui::Ui, mode: ThemeMode, pal: &ThemePalette) -> bool {
    let size = Vec2::splat(22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let hovered = resp.hovered();
    let fill = if hovered { pal.widget_hover } else { pal.widget };
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(6.0), fill);
    p.rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(1.0_f32, if hovered { pal.accent_line } else { pal.border }),
    );
    let c = rect.center();
    match mode {
        ThemeMode::Dark => {
            // Sun glyph in dark mode (click switches to Light)
            let fg = if hovered { pal.accent_line } else { pal.text };
            p.circle_stroke(c, 3.1, Stroke::new(1.3_f32, fg));
            for k in 0..8 {
                let a = (k as f32) * (PI * 0.25);
                let dir = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + dir * 4.8, c + dir * 6.8], Stroke::new(1.25_f32, fg));
            }
        }
        ThemeMode::Light => {
            // Crescent moon glyph in light mode (click switches to Dark)
            let fg = pal.accent_line;
            p.circle_filled(c + Vec2::new(-0.5, 0.2), 5.3, fg);
            p.circle_filled(c + Vec2::new(2.2, -1.9), 4.4, fill);
        }
    }
    resp.clicked()
}

fn draw_collapse_chevron_button(ui: &mut egui::Ui, expanded: bool, pal: &ThemePalette) -> bool {
    let size = Vec2::splat(20.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let hovered = resp.hovered();
    let p = ui.painter();
    p.rect_filled(
        rect,
        Rounding::same(5.0),
        if hovered { pal.widget_hover } else { pal.bg },
    );
    p.rect_stroke(
        rect,
        Rounding::same(5.0),
        Stroke::new(1.0_f32, if hovered { pal.accent_line } else { pal.border }),
    );
    let c = rect.center();
    let st = Stroke::new(1.5_f32, if hovered { pal.text } else { pal.muted });
    if expanded {
        // Chevron down (expanded)
        p.line_segment([Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.5)], st);
        p.line_segment([Pos2::new(c.x, c.y + 2.5), Pos2::new(c.x + 4.0, c.y - 2.0)], st);
    } else {
        // Chevron left (<) when collapsed
        p.line_segment([Pos2::new(c.x + 2.0, c.y - 4.0), Pos2::new(c.x - 2.5, c.y)], st);
        p.line_segment([Pos2::new(c.x - 2.5, c.y), Pos2::new(c.x + 2.0, c.y + 4.0)], st);
    }
    resp.clicked()
}

fn draw_status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

fn draw_battery_bar(
    ui: &mut egui::Ui,
    battery: &z_core::battery::BatteryReading,
    pal: &ThemePalette,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        ui.label(RichText::new("Battery:").small().color(pal.muted));
        let bar_size = Vec2::new(124.0, 8.0);
        let (rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(4.0), pal.bg);
        p.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, pal.border));

        let fraction = match (battery.percent, battery.status) {
            (Some(pct), _) => (pct as f32 / 100.0).clamp(0.0, 1.0),
            (None, BatteryStatus::Wired) => 1.0,
            _ => 0.0,
        };
        if fraction > 0.0 {
            let inner = rect.shrink(1.0);
            let fill_w = (inner.width() * fraction).max(4.0).min(inner.width());
            let fill_rect = Rect::from_min_size(inner.min, Vec2::new(fill_w, inner.height()));
            p.rect_filled(
                fill_rect,
                Rounding::same(3.0),
                battery_bar_color(battery.percent),
            );
        }

        ui.label(RichText::new(battery.format_badge()).small().color(pal.text));
    });
}

#[allow(dead_code)]
pub fn draw_compact_main_ui(
    ctx: &egui::Context,
    state: &mut AppUiState,
    engine: &EngineHandle,
) -> Vec<UiAction> {
    let snap = engine.snapshot();
    draw_compact_main_snapshot(ctx, state, &snap, |cmd| engine.send(cmd))
}

pub fn draw_compact_main_snapshot(
    ctx: &egui::Context,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    mut send: impl FnMut(EngineCommand),
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if state.selected_controller.is_none()
        || !snap
            .controllers
            .iter()
            .any(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
    {
        state.selected_controller = snap.controllers.first().map(|c| c.canonical_id.clone());
    }

    let mode = snap.config.theme_mode;
    if state.applied_theme != Some(mode) {
        apply_theme_visuals(ctx, mode);
        state.applied_theme = Some(mode);
    }
    let pal = ThemePalette::for_mode(mode);

    state.last_dynamic_rect_main = None;
    let ds = &snap.driver_status;
    let has_driver_alert = !ds.vigem_installed
        || !ds.hidhide_installed
        || ds.reboot_recommended
        || ds.hidhide_error.is_some();
    let status_bar_h = if has_driver_alert { 68.0_f32 } else { 36.0_f32 };

    // Bottom status bar (driver readiness, HidHide lock warnings, live RAM readout — NO popups)
    egui::TopBottomPanel::bottom("compact_status_bar")
        .exact_height(status_bar_h)
        .frame(
            egui::Frame::none()
                .fill(pal.panel)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            let sr = ctx.screen_rect();
            let top_y = ui.max_rect().top() - SPACE_SM;
            ui.painter().line_segment(
                [Pos2::new(sr.left(), top_y), Pos2::new(sr.right(), top_y)],
                Stroke::new(1.0_f32, pal.border),
            );
            draw_status_bar(ui, snap, has_driver_alert, &pal, &mut send);
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(pal.bg)
                .inner_margin(Margin {
                    left: SPACE_MD,
                    right: SPACE_MD,
                    top: SPACE_SM,
                    bottom: 2.0,
                }),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Controllers").color(pal.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_XS + 2.0;
                    if ui.small_button("Refresh").clicked() {
                        send(EngineCommand::RefreshStatus);
                    }
                    if draw_theme_toggle_button(ui, mode, &pal) {
                        send(EngineCommand::SetThemeMode(mode.toggled()));
                    }
                });
            });

            if snap.controllers.is_empty() {
                ui.add_space(SPACE_XS + 2.0);
                let empty_resp = egui::Frame::none()
                    .fill(pal.panel)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("No physical controllers detected.").color(pal.text),
                        );
                        ui.add_space(SPACE_XS);
                        ui.label(
                            RichText::new("Connect a controller via Bluetooth or USB.")
                                .small()
                                .color(pal.muted),
                        );
                    });
                state.desired_compact_logical_h =
                    Some((empty_resp.response.rect.bottom() + SPACE_SM + status_bar_h).ceil());
                return;
            }

            let mut content_bottom_y: Option<f32> = None;
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // Inner 6px inset keeps active cards' 4px static outer glow unclipped while locking single-column vertical flow.
                    let stack_resp = egui::Frame::none()
                        .inner_margin(Margin::symmetric(SPACE_XS + 2.0, SPACE_XS + 2.0))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let avail_col = ui.available_width();
                            ui.set_max_width(avail_col);
                            let card_inner_w = (avail_col - SPACE_MD * 2.0).max(200.0);
                            let total_cards = snap.controllers.len();
                            for (idx, ctrl) in snap.controllers.iter().enumerate() {
                                let is_selected =
                                    state.selected_controller.as_deref() == Some(&ctrl.canonical_id);
                                let card_fill = if is_selected {
                                    pal.widget
                                } else {
                                    pal.panel
                                };
                                let card_stroke = if ctrl.virtual_active {
                                    Stroke::NONE
                                } else {
                                    Stroke::new(
                                        1.0_f32,
                                        if is_selected { pal.accent_line } else { pal.border },
                                    )
                                };

                                let card_resp = egui::Frame::none()
                                    .fill(card_fill)
                                    .rounding(CARD_ROUNDING)
                                    .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM))
                                    .stroke(card_stroke)
                                    .show(ui, |ui| {
                                        ui.set_width(card_inner_w);
                                        ui.spacing_mut().item_spacing.y = SPACE_XS;

                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                                            let dot = if ctrl.connected {
                                                LOGO_BLUE
                                            } else {
                                                pal.muted
                                            };
                                            draw_status_dot(ui, dot);
                                            let title_resp = ui.add(
                                                egui::Label::new(
                                                    RichText::new(&ctrl.name)
                                                        .strong()
                                                        .color(pal.text),
                                                )
                                                .truncate()
                                                .sense(egui::Sense::click()),
                                            );
                                            if title_resp.clicked() {
                                                state.selected_controller =
                                                    Some(ctrl.canonical_id.clone());
                                            }
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if draw_collapse_chevron_button(
                                                        ui,
                                                        ctrl.show_live_input,
                                                        &pal,
                                                    ) {
                                                        send(EngineCommand::SetDeviceShowLiveInput {
                                                            key: ctrl.key.clone(),
                                                            show_live_input: !ctrl.show_live_input,
                                                        });
                                                    }
                                                },
                                            );
                                        });

                                    ui.label(
                                        RichText::new(format!(
                                            "{} | {} | {:04X}:{:04X}",
                                            ctrl.family.label(),
                                            ctrl.transport.label(),
                                            ctrl.key.vid,
                                            ctrl.key.pid
                                        ))
                                        .small()
                                        .color(pal.muted),
                                    );

                                    draw_battery_bar(ui, &ctrl.battery, &pal);

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        let mut enabled = ctrl.enabled;
                                        if ui.checkbox(&mut enabled, "Active").changed() {
                                            send(EngineCommand::SetDeviceEnabled {
                                                key: ctrl.key.clone(),
                                                enabled,
                                            });
                                        }
                                        if ctrl.virtual_active {
                                            egui::Frame::none()
                                                .fill(pal.pill_blue_fill)
                                                .rounding(Rounding::same(4.0))
                                                .stroke(Stroke::new(1.0_f32, pal.accent_line))
                                                .inner_margin(Margin::symmetric(SPACE_SM, 2.0))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "-> {}",
                                                            ctrl.target_kind.label()
                                                        ))
                                                        .small()
                                                        .color(pal.text),
                                                    );
                                                });
                                        }
                                        let cloak_dot =
                                            if ctrl.cloaked { LOGO_BLUE } else { pal.muted };
                                        draw_status_dot(ui, cloak_dot);
                                        ui.label(
                                            RichText::new(if ctrl.cloaked {
                                                "Cloaked"
                                            } else {
                                                "Visible"
                                            })
                                            .small()
                                            .color(if ctrl.cloaked {
                                                pal.text
                                            } else {
                                                pal.muted
                                            }),
                                        );
                                    });

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        ui.spacing_mut().button_padding =
                                            Vec2::new(SPACE_SM, SPACE_XS);
                                        ui.label(
                                            RichText::new("Profile:").small().color(pal.muted),
                                        );
                                        let edit_text_w = ui.fonts(|f| {
                                            f.layout_no_wrap(
                                                "Edit Profile".to_owned(),
                                                TextStyle::Small.resolve(ui.style()),
                                                pal.text,
                                            )
                                            .size()
                                            .x
                                        });
                                        let edit_btn_w = (edit_text_w + SPACE_SM * 2.0 + 4.0).ceil();
                                        let combo_w =
                                            (ui.available_width() - edit_btn_w - SPACE_SM).max(120.0);
                                        egui::ComboBox::from_id_salt((
                                            "ctrl_prof",
                                            &ctrl.canonical_id,
                                        ))
                                        .width(combo_w)
                                        .truncate()
                                        .selected_text(
                                            RichText::new(&ctrl.profile_name)
                                                .small()
                                                .color(pal.text),
                                        )
                                        .show_ui(ui, |ui| {
                                            for p in &snap.config.profiles {
                                                let item_resp = ui.selectable_label(
                                                    ctrl.profile_id == p.id,
                                                    &p.name,
                                                );
                                                if item_resp.clicked() {
                                                    send(EngineCommand::SelectDeviceProfile {
                                                        key: ctrl.key.clone(),
                                                        profile_id: p.id.clone(),
                                                    });
                                                    state.selected_controller =
                                                        Some(ctrl.canonical_id.clone());
                                                    state.editing_profile_id = Some(p.id.clone());
                                                    state.draft_profile = Some(p.clone());
                                                    state.draft_dirty = false;
                                                }
                                            }
                                        });

                                        let edit_btn = egui::Button::new(
                                            RichText::new("Edit Profile").small().color(pal.text),
                                        )
                                        .min_size(Vec2::new(edit_btn_w, 0.0))
                                        .fill(pal.pill_blue_fill)
                                        .stroke(Stroke::new(1.0_f32, pal.accent_line))
                                        .rounding(Rounding::same(6.0));
                                        if ui.add(edit_btn).clicked() {
                                            state.selected_controller =
                                                Some(ctrl.canonical_id.clone());
                                            state.editing_profile_id =
                                                Some(ctrl.profile_id.clone());
                                            state.draft_profile = Some(
                                                snap.config.profile_by_id(&ctrl.profile_id).clone(),
                                            );
                                            state.draft_dirty = false;
                                            actions.push(UiAction::OpenProfileEditor {
                                                controller_id: ctrl.canonical_id.clone(),
                                                profile_id: ctrl.profile_id.clone(),
                                            });
                                        }
                                    });

                                    if ctrl.show_live_input {
                                        let live_resp = ui.scope(|ui| {
                                            draw_card_live_status(
                                                ui,
                                                ctrl.family,
                                                ctrl.present_buttons_mask,
                                                &ctrl.physical_state,
                                                card_inner_w,
                                                &pal,
                                            );
                                        });
                                        let lr = live_resp.response.rect;
                                        state.last_dynamic_rect_main = Some(
                                            state
                                                .last_dynamic_rect_main
                                                .map_or(lr, |prev| prev.union(lr)),
                                        );
                                    }
                                });

                            if ctrl.virtual_active {
                                paint_active_card_neon_edge_and_glow(
                                    ui.painter(),
                                    card_resp.response.rect,
                                    CARD_ROUNDING,
                                );
                            }

                            if idx + 1 < total_cards {
                                ui.add_space(SPACE_LG);
                            }
                        }
                    });
                    content_bottom_y = Some(stack_resp.response.rect.bottom());
            });
            if let Some(cb) = content_bottom_y {
                state.desired_compact_logical_h = Some((cb + 2.0 + status_bar_h).ceil());
            }
        });

    actions
}

#[allow(dead_code)]
pub fn draw_profile_editor_ui(
    ctx: &egui::Context,
    state: &mut AppUiState,
    engine: &EngineHandle,
) {
    let snap = engine.snapshot();
    draw_profile_editor_snapshot(ctx, state, &snap, |cmd| engine.send(cmd));
}

pub fn draw_profile_editor_snapshot(
    ctx: &egui::Context,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    mut send: impl FnMut(EngineCommand),
) {
    let mode = snap.config.theme_mode;
    if state.applied_theme != Some(mode) {
        apply_theme_visuals(ctx, mode);
        state.applied_theme = Some(mode);
    }
    let pal = ThemePalette::for_mode(mode);

    if state.selected_controller.is_none() {
        state.selected_controller = snap.controllers.first().map(|c| c.canonical_id.clone());
    }
    if state.editing_profile_id.is_none() || !state.draft_dirty && state.draft_profile.is_none() {
        let pid = snap
            .controllers
            .iter()
            .find(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
            .map(|c| c.profile_id.clone())
            .unwrap_or_else(|| snap.config.default_profile_id.clone());
        state.editing_profile_id = Some(pid.clone());
        state.draft_profile = Some(snap.config.profile_by_id(&pid).clone());
    }

    let live_ctrl = snap
        .controllers
        .iter()
        .find(|c| {
            Some(&c.canonical_id) == state.selected_controller.as_ref()
                && state.editing_profile_id.as_deref() == Some(c.profile_id.as_str())
        })
        .or_else(|| {
            snap.controllers
                .iter()
                .find(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
        })
        .or_else(|| {
            snap.controllers.iter().find(|c| {
                c.physical_state.buttons_mask != 0
                    || c.physical_state.left_x != 0
                    || c.physical_state.left_y != 0
            })
        })
        .or_else(|| snap.controllers.first())
        .cloned();

    // Top Editor Toolbar
    egui::TopBottomPanel::top("editor_toolbar")
        .exact_height(46.0)
        .frame(
            egui::Frame::none()
                .fill(pal.panel)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                ui.label(RichText::new("Editing Profile:").color(pal.muted));
                let cur_id = state
                    .editing_profile_id
                    .clone()
                    .unwrap_or_else(|| snap.config.default_profile_id.clone());
                let cur_name = state
                    .draft_profile
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("Select");
                egui::ComboBox::from_id_salt("editor_profile_selector")
                    .width(230.0)
                    .selected_text(RichText::new(cur_name).color(pal.text))
                    .show_ui(ui, |ui| {
                        for p in &snap.config.profiles {
                            if ui.selectable_label(p.id == cur_id, &p.name).clicked() {
                                state.editing_profile_id = Some(p.id.clone());
                                state.draft_profile = Some(p.clone());
                                state.draft_dirty = false;
                            }
                        }
                    });

                if ui.button("+ New").clicked() {
                    let n = snap.config.profiles.len() + 1;
                    let mut new_prof = Profile::builtin_xbox360();
                    new_prof.id = format!("custom-{n}");
                    new_prof.name = format!("Custom Profile {n}");
                    state.editing_profile_id = Some(new_prof.id.clone());
                    state.draft_profile = Some(new_prof.clone());
                    state.draft_dirty = false;
                    send(EngineCommand::UpsertProfile(new_prof));
                }

                if snap.config.profiles.len() > 1 {
                    if let Some(id) = &state.editing_profile_id {
                        if !Profile::is_builtin_id(id) && ui.button("Delete").clicked() {
                            send(EngineCommand::DeleteProfile(id.clone()));
                            let fallback = snap.config.default_profile_id.clone();
                            state.editing_profile_id = Some(fallback.clone());
                            state.draft_profile = Some(snap.config.profile_by_id(&fallback).clone());
                            state.draft_dirty = false;
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(prof) = state.draft_profile.as_ref() {
                        let btn_text = if state.draft_dirty {
                            "Apply & Save *"
                        } else {
                            "Apply & Save"
                        };
                        let accent = if state.draft_dirty { LOGO_PINK } else { pal.accent_line };
                        let save_btn =
                            egui::Button::new(RichText::new(btn_text).strong().color(pal.text))
                                .fill(if state.draft_dirty { pal.pill_pink_fill } else { pal.pill_blue_fill })
                                .stroke(Stroke::new(1.0_f32, accent))
                                .rounding(Rounding::same(6.0));
                        if ui.add(save_btn).clicked() {
                            send(EngineCommand::UpsertProfile(prof.clone()));
                            state.draft_dirty = false;
                        }
                    }
                });
            });
            let sr = ctx.screen_rect();
            let bot_y = ui.max_rect().bottom() + SPACE_SM;
            ui.painter().line_segment(
                [Pos2::new(sr.left(), bot_y), Pos2::new(sr.right(), bot_y)],
                Stroke::new(1.0_f32, pal.border),
            );
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(pal.bg)
                .inner_margin(Margin::same(SPACE_MD)),
        )
        .show(ctx, |ui| {
            let Some(prof) = state.draft_profile.as_mut() else {
                ui.label(RichText::new("Select a profile to edit.").color(pal.muted));
                return;
            };

            let total_w = ui.available_width();
            let total_h = ui.available_height();
            let left_w = ((total_w - SPACE_MD) * 0.56).clamp(480.0, 560.0);
            let right_w = (total_w - SPACE_MD - left_w).max(280.0);

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_MD;

                // Left Column: Compact Settings + Full x360ce-style Controller Diagram + Selected Input Bar
                egui::Frame::none()
                    .fill(pal.panel)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::same(SPACE_MD))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                        ui.set_width(left_w - SPACE_MD * 2.0);
                        ui.set_min_height((total_h - SPACE_MD * 2.0).max(400.0));
                        ui.spacing_mut().item_spacing.y = SPACE_SM;

                        egui::Grid::new("editor_compact_settings")
                            .num_columns(2)
                            .spacing([SPACE_MD, SPACE_XS + 2.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Profile Name:").small().color(pal.muted));
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(egui::TextEdit::singleline(&mut prof.name).desired_width(210.0))
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                    ui.add_space(SPACE_SM);
                                    ui.label(RichText::new("Layout:").small().color(pal.muted));
                                    for (fam, lbl) in [
                                        (ControllerFamily::Xbox, "Xbox"),
                                        (ControllerFamily::Sony, "PS"),
                                        (ControllerFamily::Nintendo, "Switch"),
                                    ] {
                                        if ui.selectable_label(prof.family == fam, lbl).clicked() {
                                            prof.family = fam;
                                            state.draft_dirty = true;
                                        }
                                    }
                                });
                                ui.end_row();

                                ui.label(RichText::new("Virtual Output:").small().color(pal.muted));
                                ui.horizontal(|ui| {
                                    for (kind, text) in [
                                        (VirtualTargetKind::Xbox360, "Xbox 360"),
                                        (VirtualTargetKind::DualShock4, "DualShock 4"),
                                    ] {
                                        let is_target = prof.target == kind;
                                        let r = ui.radio_value(&mut prof.target, kind, text);
                                        if is_target {
                                            ui.painter().rect_stroke(
                                                r.rect.expand(2.0),
                                                Rounding::same(4.0),
                                                Stroke::new(1.0_f32, pal.accent_line),
                                            );
                                        }
                                        if r.changed() {
                                            state.draft_dirty = true;
                                        }
                                    }
                                    ui.separator();
                                    if ui
                                        .checkbox(&mut prof.cloak_physical, "Cloak")
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                    if ui
                                        .checkbox(&mut prof.rumble_enabled, "Rumble")
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                });
                                ui.end_row();

                                ui.label(RichText::new("Options:").small().color(pal.muted));
                                ui.horizontal(|ui| {
                                    if ui
                                        .checkbox(&mut prof.swap_ab_xy, "Swap A/B & X/Y")
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                    ui.separator();
                                    if ui
                                        .radio_value(
                                            &mut prof.disconnect_policy,
                                            DisconnectPolicy::KeepNeutral,
                                            "Keep Neutral",
                                        )
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                    if ui
                                        .radio_value(
                                            &mut prof.disconnect_policy,
                                            DisconnectPolicy::DropVirtual,
                                            "Drop",
                                        )
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                });
                                ui.end_row();

                                ui.label(RichText::new("Stick Deadzone:").small().color(pal.muted));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("L").small().color(pal.muted));
                                    let mut l_pct = (prof.left_deadzone * 100.0).round();
                                    if ui
                                        .add(egui::Slider::new(&mut l_pct, 0.0..=50.0).suffix("%"))
                                        .changed()
                                    {
                                        prof.left_deadzone = l_pct / 100.0;
                                        state.draft_dirty = true;
                                    }
                                    ui.label(RichText::new("R").small().color(pal.muted));
                                    let mut r_pct = (prof.right_deadzone * 100.0).round();
                                    if ui
                                        .add(egui::Slider::new(&mut r_pct, 0.0..=50.0).suffix("%"))
                                        .changed()
                                    {
                                        prof.right_deadzone = r_pct / 100.0;
                                        state.draft_dirty = true;
                                    }
                                });
                                ui.end_row();
                            });

                        let phys_ref = live_ctrl.as_ref().map(|c| &c.physical_state);
                        let diag_rect = draw_controller_diagram(
                            ui,
                            prof.family,
                            prof,
                            phys_ref,
                            &mut state.selected_diagram_button,
                            &pal,
                        );
                        state.last_dynamic_rect_editor = Some(diag_rect.expand(4.0));

                        // Selected Input quick-remap bar synced directly with diagram click
                        let sel_btn = state
                            .selected_diagram_button
                            .unwrap_or(LogicalButton::South);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            ui.label(
                                RichText::new("Selected Input:")
                                    .small()
                                    .color(pal.muted),
                            );
                            ui.label(
                                RichText::new(sel_btn.family_label(prof.family))
                                    .strong()
                                    .color(pal.text),
                            );
                            ui.label(RichText::new("->").color(pal.muted));
                            draw_remap_combobox(
                                ui,
                                prof,
                                sel_btn,
                                "selected_diagram_remap",
                                180.0,
                                &pal,
                                &mut state.draft_dirty,
                            );
                        });
                        });
                    });

                // Right Column: Synchronized Text Mapping List (exact same mapping data in text form)
                egui::Frame::none()
                    .fill(pal.panel)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::same(SPACE_MD))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                        ui.set_width(right_w - SPACE_MD * 2.0);
                        ui.set_min_height((total_h - SPACE_MD * 2.0).max(400.0));
                        ui.spacing_mut().item_spacing.y = SPACE_SM;

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Input Mappings").strong().color(pal.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Reset All").clicked() {
                                        prof.button_remap.clear();
                                        state.draft_dirty = true;
                                    }
                                },
                            );
                        });

                        let div = ui.min_rect();
                        draw_aligned_divider(ui, div.left(), div.right(), &pal);

                        let present_mask = live_ctrl
                            .as_ref()
                            .map_or(0, |c| c.present_buttons_mask | c.physical_state.buttons_mask);
                        let visible_buttons = visible_mapping_buttons(prof, present_mask);

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let row_w = ui.available_width();
                            for src in visible_buttons {
                                let is_sel = state.selected_diagram_button == Some(src);
                                let is_phys = live_ctrl.as_ref().is_some_and(|c| {
                                    (c.physical_state.buttons_mask & src.bit()) != 0
                                });
                                let row_fill = if is_sel {
                                    pal.row_selected_fill
                                } else {
                                    pal.bg
                                };
                                let row_stroke = if is_phys {
                                    Stroke::new(1.0_f32, LOGO_PINK)
                                } else if is_sel {
                                    Stroke::new(1.0_f32, pal.accent_line)
                                } else {
                                    Stroke::new(1.0_f32, pal.border)
                                };

                                egui::Frame::none()
                                    .fill(row_fill)
                                    .stroke(row_stroke)
                                    .rounding(6.0)
                                    .inner_margin(Margin::symmetric(SPACE_SM, SPACE_XS))
                                    .show(ui, |ui| {
                                        ui.set_min_width(
                                            (row_w - SPACE_SM * 2.0 - 12.0).max(200.0),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                                            let dot_col = if is_phys {
                                                LOGO_PINK
                                            } else if prof.button_remap.contains_key(&src) {
                                                LOGO_LAVENDER
                                            } else {
                                                pal.border
                                            };
                                            draw_status_dot(ui, dot_col);
                                            let lbl_resp = ui.add(
                                                egui::Label::new(
                                                    RichText::new(src.family_label(prof.family))
                                                        .small()
                                                        .color(if is_sel || is_phys {
                                                            pal.text
                                                        } else {
                                                            pal.muted
                                                        }),
                                                )
                                                .sense(egui::Sense::click()),
                                            );
                                            if lbl_resp.clicked() {
                                                state.selected_diagram_button = Some(src);
                                            }
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    draw_remap_combobox(
                                                        ui,
                                                        prof,
                                                        src,
                                                        ("text_remap", src),
                                                        148.0,
                                                        &pal,
                                                        &mut state.draft_dirty,
                                                    );
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(SPACE_XS);
                            }
                        });
                        });
                    });
            });
        });
}

fn draw_remap_combobox(
    ui: &mut egui::Ui,
    prof: &mut Profile,
    src: LogicalButton,
    salt: impl std::hash::Hash,
    width: f32,
    pal: &ThemePalette,
    dirty: &mut bool,
) {
    let current = prof.button_remap.get(&src).copied();
    let label = match current {
        None => "Default (Pass)".to_string(),
        Some(None) => "Unbound".to_string(),
        Some(Some(dst)) => dst.family_label(prof.family).to_string(),
    };
    egui::ComboBox::from_id_salt(salt)
        .width(width)
        .selected_text(RichText::new(label).small().color(pal.text))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), "Default (Pass)")
                .clicked()
            {
                prof.button_remap.remove(&src);
                *dirty = true;
            }
            if ui
                .selectable_label(current == Some(None), "Unbound")
                .clicked()
            {
                prof.button_remap.insert(src, None);
                *dirty = true;
            }
            for dst in LogicalButton::VIRTUAL_TARGETS {
                if ui
                    .selectable_label(
                        current == Some(Some(dst)),
                        dst.family_label(prof.family),
                    )
                    .clicked()
                {
                    prof.button_remap.insert(src, Some(dst));
                    *dirty = true;
                }
            }
        });
}

fn draw_status_bar_row(ui: &mut egui::Ui, snap: &EngineSnapshot, pal: &ThemePalette) {
    ui.spacing_mut().item_spacing.x = SPACE_SM;
    let ds = &snap.driver_status;
    draw_status_dot(ui, if ds.vigem_installed { LOGO_BLUE } else { LOGO_PINK });
    ui.label(
        RichText::new(if ds.vigem_installed {
            "ViGEmBus: OK"
        } else {
            "ViGEmBus: Missing"
        })
        .small()
        .color(pal.text),
    );
    ui.separator();

    let (hh_text, hh_dot) = if !ds.hidhide_installed {
        ("HidHide: Missing", LOGO_PINK)
    } else if ds.hidhide_accessible {
        ("HidHide: Ready", LOGO_BLUE)
    } else {
        ("HidHide: Locked", LOGO_PINK)
    };
    draw_status_dot(ui, hh_dot);
    ui.label(RichText::new(hh_text).small().color(pal.text));

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let (ws, priv_b) = query_process_memory_bytes();
        let ws_mb = ws as f64 / (1024.0 * 1024.0);
        let priv_mb = priv_b as f64 / (1024.0 * 1024.0);
        ui.label(
            RichText::new(format!("RAM: {ws_mb:.1} / {priv_mb:.1} MB"))
                .small()
                .color(pal.muted),
        );
    });
}

fn draw_status_bar(
    ui: &mut egui::Ui,
    snap: &EngineSnapshot,
    has_driver_alert: bool,
    pal: &ThemePalette,
    send: &mut impl FnMut(EngineCommand),
) {
    let ds = &snap.driver_status;
    if !has_driver_alert {
        ui.horizontal_centered(|ui| draw_status_bar_row(ui, snap, pal));
        return;
    }

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = SPACE_XS + 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_SM;
            ui.spacing_mut().button_padding = Vec2::new(SPACE_SM, SPACE_XS);
            if !ds.vigem_installed || !ds.hidhide_installed {
                ui.label(
                    RichText::new("Required drivers missing")
                        .small()
                        .color(pal.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let install_btn = egui::Button::new(
                        RichText::new("Install Missing Drivers")
                            .small()
                            .color(pal.text),
                    )
                    .fill(pal.pill_pink_fill)
                    .stroke(Stroke::new(1.0_f32, LOGO_PINK))
                    .rounding(Rounding::same(6.0));
                    if ui.add(install_btn).clicked() {
                        send(EngineCommand::InstallDrivers);
                    }
                });
            } else if ds.reboot_recommended {
                draw_status_dot(ui, LOGO_PINK);
                ui.label(
                    RichText::new("Reboot recommended to finish driver setup")
                        .small()
                        .color(pal.muted),
                );
            } else if let Some(err) = &ds.hidhide_error {
                draw_status_dot(ui, LOGO_PINK);
                ui.add(
                    egui::Label::new(RichText::new(err).small().color(pal.muted)).truncate(),
                );
            }
        });
        ui.horizontal(|ui| draw_status_bar_row(ui, snap, pal));
    });
}

fn extra_buttons_for_device(family: ControllerFamily, active_mask: u32) -> Vec<LogicalButton> {
    let mut extras = Vec::new();
    for btn in LogicalButton::EXTRA {
        let exposed = (active_mask & btn.bit()) != 0;
        let family_standard_extra = match (family, btn) {
            (ControllerFamily::Nintendo, LogicalButton::Misc1) => true,
            (ControllerFamily::Sony, LogicalButton::Misc1 | LogicalButton::Touchpad) => true,
            (ControllerFamily::Xbox, LogicalButton::Misc1) => true,
            _ => false,
        };
        if exposed || family_standard_extra {
            extras.push(btn);
        }
    }
    extras
}

fn visible_mapping_buttons(profile: &Profile, active_mask: u32) -> Vec<LogicalButton> {
    let mut out = Vec::with_capacity(26);
    out.extend_from_slice(&LogicalButton::STANDARD);
    for btn in LogicalButton::EXTRA {
        let exposed = (active_mask & btn.bit()) != 0;
        let remapped = profile.button_remap.contains_key(&btn);
        let family_default = match (profile.family, btn) {
            (ControllerFamily::Nintendo, LogicalButton::Misc1) => true,
            (ControllerFamily::Sony, LogicalButton::Misc1 | LogicalButton::Touchpad) => true,
            (ControllerFamily::Xbox, LogicalButton::Misc1) => true,
            _ => matches!(
                btn,
                LogicalButton::RightPaddle1
                    | LogicalButton::LeftPaddle1
                    | LogicalButton::RightPaddle2
                    | LogicalButton::LeftPaddle2
            ),
        };
        if exposed || remapped || family_default {
            out.push(btn);
        }
    }
    out
}

const CARD_STANDARD_ROW1: [LogicalButton; 8] = [
    LogicalButton::South,
    LogicalButton::East,
    LogicalButton::West,
    LogicalButton::North,
    LogicalButton::LeftShoulder,
    LogicalButton::RightShoulder,
    LogicalButton::LeftStick,
    LogicalButton::RightStick,
];

const CARD_STANDARD_ROW2: [LogicalButton; 7] = [
    LogicalButton::DpadUp,
    LogicalButton::DpadDown,
    LogicalButton::DpadLeft,
    LogicalButton::DpadRight,
    LogicalButton::Back,
    LogicalButton::Start,
    LogicalButton::Guide,
];

fn draw_card_live_status(
    ui: &mut egui::Ui,
    family: ControllerFamily,
    present_mask: u32,
    phys: &PhysicalSnapshot,
    card_inner_w: f32,
    pal: &ThemePalette,
) {
    let div_left = ui.cursor().left();
    draw_aligned_divider(ui, div_left, div_left + card_inner_w, pal);

    let (lt_lbl, rt_lbl) = match family {
        ControllerFamily::Sony => ("L2", "R2"),
        ControllerFamily::Nintendo => ("ZL", "ZR"),
        ControllerFamily::Xbox | ControllerFamily::Generic => ("LT", "RT"),
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        draw_mini_stick_pad(
            ui,
            "L",
            normalize_stick_axis(phys.left_x),
            normalize_stick_axis(phys.left_y),
            pal,
        );
        draw_mini_stick_pad(
            ui,
            "R",
            normalize_stick_axis(phys.right_x),
            normalize_stick_axis(phys.right_y),
            pal,
        );
        ui.vertical(|ui| {
            ui.add_space(SPACE_XS);
            draw_mini_trigger_row(ui, lt_lbl, normalize_trigger_axis(phys.left_trigger), pal);
            ui.add_space(SPACE_XS);
            draw_mini_trigger_row(ui, rt_lbl, normalize_trigger_axis(phys.right_trigger), pal);
        });
    });

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS;
        for btn in CARD_STANDARD_ROW1 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, btn.family_chip(family), pressed, 32.0, pal);
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS;
        for btn in CARD_STANDARD_ROW2 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, btn.family_chip(family), pressed, 32.0, pal);
        }
    });

    let extras = extra_buttons_for_device(family, present_mask | phys.buttons_mask);
    for chunk in extras.chunks(8) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_XS;
            for &btn in chunk {
                let pressed = (phys.buttons_mask & btn.bit()) != 0;
                draw_mini_button_pill(ui, btn.family_chip(family), pressed, 36.0, pal);
            }
        });
    }
}

fn draw_mini_stick_pad(ui: &mut egui::Ui, tag: &str, x: f32, y: f32, pal: &ThemePalette) {
    let size = 44.0_f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter();
    let center = rect.center();
    let radius = 19.0_f32;

    p.circle_filled(center, radius, pal.bg);
    p.circle_stroke(center, radius, Stroke::new(1.0_f32, pal.border));
    p.line_segment(
        [
            Pos2::new(center.x - radius + 2.0, center.y),
            Pos2::new(center.x + radius - 2.0, center.y),
        ],
        Stroke::new(1.0_f32, pal.crosshair),
    );
    p.line_segment(
        [
            Pos2::new(center.x, center.y - radius + 2.0),
            Pos2::new(center.x, center.y + radius - 2.0),
        ],
        Stroke::new(1.0_f32, pal.crosshair),
    );
    p.text(
        rect.min + Vec2::new(2.0, 0.0),
        Align2::LEFT_TOP,
        tag,
        FontId::new(9.5, FontFamily::Proportional),
        pal.muted,
    );

    let mag = (x * x + y * y).sqrt();
    let travel = radius - 4.5_f32;
    let clamped_scale = if mag > 1.0 { 1.0 / mag } else { 1.0 };
    let dot_pos = Pos2::new(
        center.x + x * clamped_scale * travel,
        center.y + y * clamped_scale * travel,
    );
    if mag > 0.08 {
        p.circle_filled(
            dot_pos,
            6.5,
            Color32::from_rgba_unmultiplied(LOGO_PINK.r(), LOGO_PINK.g(), LOGO_PINK.b(), 64),
        );
        p.circle_filled(dot_pos, 3.8, LOGO_PINK);
    } else {
        p.circle_filled(dot_pos, 3.0, LOGO_BLUE);
    }
}

fn draw_mini_trigger_row(ui: &mut egui::Ui, label: &str, frac: f32, pal: &ThemePalette) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS + 2.0;
        ui.label(
            RichText::new(label)
                .font(FontId::new(10.5, FontFamily::Proportional))
                .color(pal.muted),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::new(112.0, 7.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(3.5), pal.bg);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.0_f32, pal.border));
        let u = frac.clamp(0.0, 1.0);
        if u > 0.01 {
            let inner = rect.shrink(1.0);
            let fw = (inner.width() * u).max(3.0).min(inner.width());
            p.rect_filled(
                Rect::from_min_size(inner.min, Vec2::new(fw, inner.height())),
                Rounding::same(2.5),
                LOGO_PINK,
            );
        }
        ui.label(
            RichText::new(format!("{:.0}%", u * 100.0))
                .font(FontId::new(10.5, FontFamily::Proportional))
                .color(if u > 0.01 { pal.text } else { pal.muted }),
        );
    });
}

fn draw_mini_button_pill(
    ui: &mut egui::Ui,
    label: &str,
    pressed: bool,
    width: f32,
    pal: &ThemePalette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), egui::Sense::hover());
    let p = ui.painter();
    if pressed {
        p.rect_stroke(
            rect.expand(1.2),
            Rounding::same(4.5),
            Stroke::new(
                1.5_f32,
                Color32::from_rgba_unmultiplied(LOGO_PINK.r(), LOGO_PINK.g(), LOGO_PINK.b(), 80),
            ),
        );
        p.rect_filled(rect, Rounding::same(3.5), pal.pill_pink_fill);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.5_f32, LOGO_PINK));
    } else {
        p.rect_filled(rect, Rounding::same(3.5), pal.bg);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.0_f32, pal.border));
    }
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.0, FontFamily::Proportional),
        if pressed { pal.text } else { pal.muted },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ControllerSnapshot;
    use z_core::backend::DriverStatus;
    use z_core::battery::BatteryReading;
    use z_core::model::{ControllerFamily, HardwareKey, Transport, VirtualTargetKind};
    use z_core::profile::AppConfig;

    fn sample_snapshot() -> EngineSnapshot {
        EngineSnapshot {
            controllers: vec![
                ControllerSnapshot {
                    key: HardwareKey::new(0x057E, 0x2009, "NSP-1"),
                    canonical_id: "057e:2009:NSP-1".to_string(),
                    sdl_id: Some(1),
                    name: "Nintendo Switch Pro Controller".to_string(),
                    family: ControllerFamily::Nintendo,
                    transport: Transport::Bluetooth,
                    connected: true,
                    enabled: true,
                    show_live_input: true,
                    profile_id: "xbox-360".to_string(),
                    profile_name: "Xbox 360".to_string(),
                    target_kind: VirtualTargetKind::Xbox360,
                    cloaked: true,
                    virtual_active: true,
                    virtual_serial: Some(1),
                    battery: BatteryReading {
                        percent: Some(86),
                        status: BatteryStatus::OnBattery,
                        source: "sdl3",
                    },
                    present_buttons_mask: LogicalButton::Misc1.bit()
                        | LogicalButton::RightPaddle1.bit(),
                    physical_state: PhysicalSnapshot {
                        buttons_mask: LogicalButton::South.bit() | LogicalButton::Misc1.bit(),
                        left_x: 24000,
                        left_y: -21000,
                        right_x: 0,
                        right_y: 0,
                        left_trigger: 0,
                        right_trigger: 22384,
                    },
                    virtual_output: crate::engine::VirtualOutputSnapshot::None,
                    instance_id: None,
                },
                ControllerSnapshot {
                    key: HardwareKey::new(0x054C, 0x0CE6, "DS5-2"),
                    canonical_id: "054c:0ce6:DS5-2".to_string(),
                    sdl_id: Some(2),
                    name: "DualSense Wireless Controller".to_string(),
                    family: ControllerFamily::Sony,
                    transport: Transport::Usb,
                    connected: true,
                    enabled: false,
                    show_live_input: true,
                    profile_id: "dualsense".to_string(),
                    profile_name: "DualSense".to_string(),
                    target_kind: VirtualTargetKind::DualShock4,
                    cloaked: false,
                    virtual_active: false,
                    virtual_serial: None,
                    battery: BatteryReading {
                        percent: Some(18),
                        status: BatteryStatus::OnBattery,
                        source: "sony-hid",
                    },
                    present_buttons_mask: LogicalButton::Touchpad.bit() | LogicalButton::Misc1.bit(),
                    physical_state: PhysicalSnapshot::default(),
                    virtual_output: crate::engine::VirtualOutputSnapshot::None,
                    instance_id: None,
                },
            ],
            config: AppConfig::default(),
            driver_status: DriverStatus {
                vigem_installed: true,
                hidhide_installed: true,
                hidhide_accessible: true,
                hidhide_error: None,
                reboot_recommended: false,
            },
            last_status_message: None,
        }
    }

    #[test]
    fn battery_bar_uses_pink_at_or_below_twenty_and_blue_above() {
        assert_eq!(battery_bar_color(Some(0)), LOGO_PINK);
        assert_eq!(battery_bar_color(Some(20)), LOGO_PINK);
        assert_eq!(battery_bar_color(Some(21)), LOGO_BLUE);
        assert_eq!(battery_bar_color(Some(100)), LOGO_BLUE);
        assert_eq!(battery_bar_color(None), LOGO_BLUE);
    }

    #[test]
    fn card_live_status_covers_all_standard_and_extra_buttons() {
        let mut seen: Vec<LogicalButton> = CARD_STANDARD_ROW1
            .into_iter()
            .chain(CARD_STANDARD_ROW2)
            .collect();
        seen.extend_from_slice(&LogicalButton::EXTRA);
        assert_eq!(seen.len(), LogicalButton::ALL.len());
        for b in LogicalButton::ALL {
            assert!(seen.contains(&b));
        }

        let nintendo_extras = extra_buttons_for_device(ControllerFamily::Nintendo, 0);
        assert!(nintendo_extras.contains(&LogicalButton::Misc1));
        let sony_extras = extra_buttons_for_device(ControllerFamily::Sony, 0);
        assert!(sony_extras.contains(&LogicalButton::Touchpad));
        assert!(sony_extras.contains(&LogicalButton::Misc1));
        let xbox_extras =
            extra_buttons_for_device(ControllerFamily::Xbox, LogicalButton::RightPaddle1.bit());
        assert!(xbox_extras.contains(&LogicalButton::Misc1));
        assert!(xbox_extras.contains(&LogicalButton::RightPaddle1));
    }

    #[test]
    fn compact_main_and_profile_editor_render_all_families_cleanly() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let snap = sample_snapshot();
        let mut ui_state = AppUiState::default();

        let compact_raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(392.0, 580.0))),
            ..Default::default()
        };
        let compact_out = ctx.run(compact_raw, |c| {
            let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
        });
        assert!(!ctx.tessellate(compact_out.shapes, 1.0).is_empty());

        for pid in ["xbox-360", "dualsense", "switch-pro"] {
            ui_state.editing_profile_id = Some(pid.to_string());
            ui_state.draft_profile = Some(snap.config.profile_by_id(pid).clone());
            let ed_raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(980.0, 660.0))),
                ..Default::default()
            };
            let ed_out = ctx.run(ed_raw, |c| {
                draw_profile_editor_snapshot(c, &mut ui_state, &snap, |_| {});
            });
            assert!(!ctx.tessellate(ed_out.shapes, 1.0).is_empty());
        }
    }

    fn find_text_center(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<Pos2> {
        fn scan(s: &egui::Shape, needle: &str) -> Option<Pos2> {
            match s {
                egui::Shape::Vec(v) => v.iter().rev().find_map(|c| scan(c, needle)),
                egui::Shape::Text(ts) if ts.galley.text() == needle => {
                    Some(ts.pos + ts.galley.size() * 0.5)
                }
                _ => None,
            }
        }
        shapes.iter().rev().find_map(|c| scan(&c.shape, needle))
    }

    #[test]
    fn clicking_profile_selector_emits_select_device_profile_command() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let snap = sample_snapshot();
        let mut ui_state = AppUiState::default();
        let mut emitted = Vec::new();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0)));
        let mut t = 0.0_f64;

        let mut step = |events: Vec<egui::Event>, emitted: &mut Vec<EngineCommand>| {
            t += 0.05;
            ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(t),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| {
                        emitted.push(cmd);
                    });
                },
            )
            .shapes
        };

        // 1. Render compact window and locate card 1's ComboBox ("Xbox 360") in painted shapes.
        let shapes_idle = step(vec![], &mut emitted);
        let combo_pos = find_text_center(&shapes_idle, "Xbox 360")
            .expect("expected 'Xbox 360' ComboBox text in compact view");

        // 2. Real pointer press + release on ComboBox button opens popup.
        let _ = step(
            vec![
                egui::Event::PointerMoved(combo_pos),
                egui::Event::PointerButton {
                    pos: combo_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut emitted,
        );
        let shapes_open = step(
            vec![egui::Event::PointerButton {
                pos: combo_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut emitted,
        );
        let shapes_popup = step(vec![], &mut emitted);

        // 3. Locate "DualSense" option inside topmost popup layer and click it.
        let item_pos = find_text_center(&shapes_popup, "DualSense")
            .or_else(|| find_text_center(&shapes_open, "DualSense"))
            .expect("expected 'DualSense' item inside opened ComboBox popup");

        let _ = step(
            vec![
                egui::Event::PointerMoved(item_pos),
                egui::Event::PointerButton {
                    pos: item_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut emitted,
        );
        let _ = step(
            vec![egui::Event::PointerButton {
                pos: item_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut emitted,
        );

        assert!(
            emitted.iter().any(|c| matches!(
                c,
                EngineCommand::SelectDeviceProfile { profile_id, .. } if profile_id == "dualsense"
            )),
            "expected SelectDeviceProfile {{ profile_id: \"dualsense\" }} from ComboBox click, got {emitted:?}"
        );
    }

    #[test]
    fn collapse_icon_toggle_emits_command_and_collapses_card_height() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.controllers.truncate(1);
        let mut ui_state = AppUiState::default();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(396.0, 601.0)));

        // Expanded render: locate "Edit Profile" bottom & header chevron position (`top_right` of card)
        let mut emitted = Vec::new();
        let out_exp = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.05),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| {
                    emitted.push(cmd);
                });
            },
        );
        let exp_dyn = ui_state
            .last_dynamic_rect_main
            .expect("expected live input section rect when show_live_input=true");
        assert!(exp_dyn.height() > 90.0);

        // Locate chevron toggle button in the card header (top-right corner, aligned with controller name)
        let name_center =
            find_text_center(&out_exp.shapes, "Nintendo Switch Pro Controller").unwrap();
        fn find_square_button_near_y(shapes: &[egui::epaint::ClippedShape], target_y: f32) -> Option<Pos2> {
            fn scan(s: &egui::Shape, target_y: f32) -> Option<Pos2> {
                match s {
                    egui::Shape::Vec(v) => v.iter().rev().find_map(|c| scan(c, target_y)),
                    egui::Shape::Rect(rs)
                        if (rs.rect.width() - 20.0).abs() < 1.5
                            && (rs.rect.height() - 20.0).abs() < 1.5
                            && (rs.rect.center().y - target_y).abs() < 8.0 =>
                    {
                        Some(rs.rect.center())
                    }
                    _ => None,
                }
            }
            shapes.iter().rev().find_map(|c| scan(&c.shape, target_y))
        }
        let toggle_pos = find_square_button_near_y(&out_exp.shapes, name_center.y)
            .expect("expected 20x20 chevron toggle button on card header");
        for (i, pressed) in [(1, true), (2, false)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05 * (i + 1) as f64),
                    events: vec![
                        egui::Event::PointerMoved(toggle_pos),
                        egui::Event::PointerButton {
                            pos: toggle_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| {
                        emitted.push(cmd);
                    });
                },
            );
        }
        assert!(
            emitted.iter().any(|c| matches!(
                c,
                EngineCommand::SetDeviceShowLiveInput {
                    show_live_input: false,
                    ..
                }
            )),
            "expected SetDeviceShowLiveInput {{ show_live_input: false }}, got {emitted:?}"
        );

        // Render collapsed state: live section omitted, last_dynamic_rect_main is None,
        // and desired_compact_logical_h shrinks cleanly to hug the single collapsed card.
        snap.controllers[0].show_live_input = false;
        let out_col = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.25),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        assert!(ui_state.last_dynamic_rect_main.is_none());
        let collapsed_h = ui_state
            .desired_compact_logical_h
            .expect("desired_compact_logical_h populated on collapsed render");

        // Compare against single expanded card height (must hug visible content in both states).
        snap.controllers[0].show_live_input = true;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.30),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        let expanded_h = ui_state
            .desired_compact_logical_h
            .expect("desired_compact_logical_h populated on expanded render");
        assert!(
            collapsed_h < 250.0 && (expanded_h - collapsed_h) > 100.0,
            "collapsed height ({collapsed_h}) should hug header-only card and shrink >100px vs expanded ({expanded_h})"
        );
        let edit_pt = find_text_center(&out_col.shapes, "Edit Profile").unwrap();
        // Bottom margin of collapsed card sits tightly below Edit Profile button (no reserved dead space)
        assert!(collapsed_h - (edit_pt.y + 12.0) < 68.0);
    }

    #[test]
    fn multiple_controllers_stack_vertically_in_single_column() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        let mut third = snap.controllers[0].clone();
        third.key = HardwareKey::new(0x045E, 0x0B12, "XBOX-3");
        third.canonical_id = "045e:0b12:XBOX-3".to_string();
        third.name = "Xbox Series Pad 3".to_string();
        third.show_live_input = false;
        snap.controllers.push(third);

        let mut ui_state = AppUiState::default();
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(396.0, 900.0))),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );

        let p1 = find_text_center(&out.shapes, "Nintendo Switch Pro Controller").unwrap();
        let p2 = find_text_center(&out.shapes, "DualSense Wireless Controller").unwrap();
        let p3 = find_text_center(&out.shapes, "Xbox Series Pad 3").unwrap();

        assert!(p1.y < p2.y && p2.y < p3.y, "cards must stack strictly vertically: {p1:?} {p2:?} {p3:?}");
        assert!((p1.x - p2.x).abs() < 12.0 && p2.x < 250.0, "cards must share single column");
    }

    #[test]
    fn theme_toggle_button_left_of_refresh_emits_set_theme_mode() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.config.theme_mode = ThemeMode::Dark;
        let mut ui_state = AppUiState::default();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(396.0, 601.0)));

        let out_dark = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.05),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        let refresh_center = find_text_center(&out_dark.shapes, "Refresh").unwrap();
        let toggle_center = Pos2::new(refresh_center.x - 42.0, refresh_center.y);

        let mut emitted = Vec::new();
        for (i, pressed) in [(1, true), (2, false)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05 * (i + 1) as f64),
                    events: vec![
                        egui::Event::PointerMoved(toggle_center),
                        egui::Event::PointerButton {
                            pos: toggle_center,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| {
                        emitted.push(cmd);
                    });
                },
            );
        }
        assert!(
            emitted
                .iter()
                .any(|c| matches!(c, EngineCommand::SetThemeMode(ThemeMode::Light))),
            "expected SetThemeMode(Light) from dark mode toggle, got {emitted:?}"
        );

        // Switching to Light updates egui visuals background & clicking toggles back to Dark.
        snap.config.theme_mode = ThemeMode::Light;
        emitted.clear();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.25),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        assert!(!ctx.style().visuals.dark_mode);
        for (i, pressed) in [(6, true), (7, false)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05 * i as f64),
                    events: vec![
                        egui::Event::PointerMoved(toggle_center),
                        egui::Event::PointerButton {
                            pos: toggle_center,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| {
                        emitted.push(cmd);
                    });
                },
            );
        }
        assert!(
            emitted
                .iter()
                .any(|c| matches!(c, EngineCommand::SetThemeMode(ThemeMode::Dark))),
            "expected SetThemeMode(Dark) from light mode toggle, got {emitted:?}"
        );
    }
}
