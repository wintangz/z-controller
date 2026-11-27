use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::sync::OnceLock;

use egui::epaint::{Mesh, PathShape, Shadow, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Rect, RichText,
    Rounding, Sense, Stroke, TextStyle, Vec2,
};
use z_core::battery::BatteryStatus;
use z_core::driver::{DriverEntryStatus, DriverId, DriverState};
use z_core::model::{ControllerFamily, DisconnectPolicy, LogicalButton, VirtualTargetKind};
use z_core::profile::{Profile, ThemeAccent, ThemeMode, ThemePreset};
use z_platform_windows::drivers::{query_process_memory_bytes, remedy_for_entry};

use crate::diagram::draw_controller_diagram;
use crate::engine::{
    EngineCommand, EngineHandle, EngineSnapshot, PhysicalSnapshot,
};
use crate::logo_mark::masthead_logo_texture;
use crate::theme_engine::{
    MonochromeLadder, STATUS_GREEN, STATUS_RED, ThemePalette, contrast_ratio, lerp_color,
};

// Family masthead functional caption (mirrors y-storage pattern).
pub const MASTHEAD_CAPTION: &str = "Virtual pad & cloak";

const HEADER_CONTROL_H: f32 = 28.0;
const CARD_ROUNDING: f32 = 8.0;
const RADIUS_CONTROL: f32 = 6.0;
const RADIUS_PILL: f32 = 4.0;
const PILL_BADGE_H: f32 = 18.0;
const RADIUS_BADGE_PILL: f32 = PILL_BADGE_H * 0.5;

// Consistent 4 / 8 / 12 / 16 spacing scale applied uniformly across all panels, cards, and grids.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;

const SEGOE_UI_FONT_PATH: &str = r"C:\Windows\Fonts\segoeui.ttf";
const SEGOE_UI_SEMIBOLD_PATH: &str = r"C:\Windows\Fonts\seguisb.ttf";
const SEGOE_UI_BOLD_PATH: &str = r"C:\Windows\Fonts\segoeuib.ttf";
const CONSOLA_FONT_PATH: &str = r"C:\Windows\Fonts\consola.ttf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    OpenProfileEditor {
        controller_id: String,
        profile_id: String,
    },
    BrowseDriverFolder {
        driver: DriverId,
    },
}

pub struct AppUiState {
    pub selected_controller: Option<String>,
    pub editing_profile_id: Option<String>,
    pub draft_profile: Option<Profile>,
    pub draft_dirty: bool,
    pub selected_diagram_button: Option<LogicalButton>,
    pub pending_install_confirm: Option<DriverId>,
    pub pending_reboot_confirm: Option<DriverId>,
    pub missing_vigem_notice_controller: Option<String>,
    pub drivers_section_override: Option<bool>,
    pub last_all_drivers_working: Option<bool>,
    pub last_dynamic_rect_main: Option<Rect>,
    pub last_dynamic_rect_editor: Option<Rect>,
    pub desired_compact_logical_h: Option<f32>,
    pub applied_theme: Option<ThemeMode>,
    pub applied_accent: Option<ThemeAccent>,
    pub theme_picker_open: bool,
    pub custom_hex_input: String,
    pub custom_rgb: [u8; 3],
    pub custom_hex_error: Option<String>,
}

impl Default for AppUiState {
    fn default() -> Self {
        let default_rgb = ThemeAccent::default().base_rgb();
        Self {
            selected_controller: None,
            editing_profile_id: None,
            draft_profile: None,
            draft_dirty: false,
            selected_diagram_button: Some(LogicalButton::South),
            pending_install_confirm: None,
            pending_reboot_confirm: None,
            missing_vigem_notice_controller: None,
            drivers_section_override: None,
            last_all_drivers_working: None,
            last_dynamic_rect_main: None,
            last_dynamic_rect_editor: None,
            desired_compact_logical_h: None,
            applied_theme: None,
            applied_accent: None,
            theme_picker_open: false,
            custom_hex_input: ThemeAccent::default().hex_string(),
            custom_rgb: default_rgb,
            custom_hex_error: None,
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
    ui.pending_install_confirm.hash(&mut h);
    ui.pending_reboot_confirm.hash(&mut h);
    ui.missing_vigem_notice_controller.hash(&mut h);
    ui.drivers_section_override.hash(&mut h);
    ui.last_all_drivers_working.hash(&mut h);
    ui.theme_picker_open.hash(&mut h);
    ui.custom_hex_input.hash(&mut h);
    ui.custom_rgb.hash(&mut h);

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

    for entry in [&snap.driver_status.vigem, &snap.driver_status.hidhide] {
        entry.id.hash(&mut h);
        entry.state.hash(&mut h);
        entry.version.hash(&mut h);
        entry.pnp_instance_id.hash(&mut h);
        entry.detail_note.hash(&mut h);
    }
    snap.busy_driver.hash(&mut h);
    snap.driver_messages.hash(&mut h);
    snap.last_status_message.hash(&mut h);
    (snap.config.theme_mode as u8).hash(&mut h);
    snap.config.theme_accent.hash(&mut h);
    snap.config.vigem_located_dir.hash(&mut h);
    snap.config.hidhide_located_dir.hash(&mut h);
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

pub fn apply_theme_visuals(ctx: &egui::Context, mode: ThemeMode, accent: ThemeAccent) {
    let pal = ThemePalette::for_theme(mode, accent);
    let mut style = (*ctx.style()).clone();
    let mut visuals = match mode {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    visuals.dark_mode = mode == ThemeMode::Dark;
    visuals.panel_fill = pal.bg;
    visuals.window_fill = pal.panel;
    visuals.extreme_bg_color = pal.bg;
    visuals.faint_bg_color = pal.panel_alt;
    visuals.code_bg_color = pal.bg;
    visuals.override_text_color = None;
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
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    visuals.widgets.open.bg_fill = pal.widget_hover;
    visuals.widgets.open.weak_bg_fill = pal.widget_hover;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.open.rounding = Rounding::same(6.0);

    visuals.selection.bg_fill = pal.soft_fill;
    visuals.selection.stroke = Stroke::new(1.0_f32, pal.accent_line);

    style.visuals = visuals;
    style.animation_time = 0.0;
    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(SPACE_MD, SPACE_XS);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.bar_inner_margin = SPACE_SM;
    style.spacing.scroll.bar_outer_margin = 0.0;
    style.spacing.scroll.handle_min_length = 24.0;
    ctx.set_style(style);
}

pub fn configure_neon_theme_and_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let prop_regular_bytes = std::fs::read(SEGOE_UI_FONT_PATH).ok();
    if let Some(bytes) = prop_regular_bytes.as_ref() {
        fonts
            .font_data
            .insert("segoe_ui".to_owned(), FontData::from_owned(bytes.clone()));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
    }
    // DESIGN.md §3: Reserve Semibold strictly for primary card identities & modal titles.
    let prop_strong_bytes = std::fs::read(SEGOE_UI_SEMIBOLD_PATH)
        .or_else(|_| std::fs::read(SEGOE_UI_BOLD_PATH))
        .ok()
        .or(prop_regular_bytes);
    if let Some(strong_bytes) = prop_strong_bytes {
        fonts
            .font_data
            .insert("segoe_ui_strong".to_owned(), FontData::from_owned(strong_bytes));
        fonts.families.insert(
            FontFamily::Name("ProportionalStrong".into()),
            vec!["segoe_ui_strong".to_owned(), "segoe_ui".to_owned()],
        );
    }
    // DESIGN.md §3: Load Regular Consolas (`consola.ttf`) for tabular/monospace numeric readouts.
    if let Ok(mono_bytes) = std::fs::read(CONSOLA_FONT_PATH) {
        fonts
            .font_data
            .insert("consola_mono".to_owned(), FontData::from_owned(mono_bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "consola_mono".to_owned());
        }
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = BTreeMap::from([
        (TextStyle::Heading, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]);
    style.animation_time = 0.0;
    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(SPACE_MD, SPACE_XS);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.bar_inner_margin = SPACE_SM;
    style.spacing.scroll.bar_outer_margin = 0.0;
    style.spacing.scroll.handle_min_length = 24.0;
    ctx.set_style(style);
    apply_theme_visuals(ctx, ThemeMode::Dark, ThemeAccent::default());
}

#[inline]
fn strong_font(size_px: f32) -> FontId {
    FontId::new(size_px, FontFamily::Name("ProportionalStrong".into()))
}

#[inline]
pub fn battery_is_low(percent: Option<u8>) -> bool {
    matches!(percent, Some(p) if p <= 20)
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

fn corner_unit_arcs() -> &'static [[(f32, f32); 17]; 4] {
    static ARCS: OnceLock<[[(f32, f32); 17]; 4]> = OnceLock::new();
    ARCS.get_or_init(|| {
        let ranges = [
            (PI, 1.5 * PI),
            (1.5 * PI, 2.0 * PI),
            (0.0, 0.5 * PI),
            (0.5 * PI, PI),
        ];
        let mut out = [[(0.0_f32, 0.0_f32); 17]; 4];
        for (c_idx, &(a0, a1)) in ranges.iter().enumerate() {
            for k in 0..=16usize {
                let theta = a0 + (a1 - a0) * (k as f32 / 16.0);
                out[c_idx][k] = (theta.cos(), theta.sin());
            }
        }
        out
    })
}

fn push_rounded_rect_perimeter(rect: Rect, radius: f32, out: &mut Vec<(Pos2, Vec2)>) {
    let r = radius.clamp(0.0, 0.5 * rect.width().min(rect.height()));
    let centers = [
        Pos2::new(rect.left() + r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.bottom() - r),
        Pos2::new(rect.left() + r, rect.bottom() - r),
    ];
    let arcs = corner_unit_arcs();
    for (center, arc) in centers.into_iter().zip(arcs.iter()) {
        for &(nx, ny) in arc {
            let n = Vec2::new(nx, ny);
            let p = center + n * r;
            if let Some(&(prev_p, _)) = out.last() {
                if (p - prev_p).length_sq() < 1e-5 {
                    continue;
                }
            }
            out.push((p, n));
        }
    }
    if out.len() >= 2 && (out[0].0 - out[out.len() - 1].0).length_sq() < 1e-5 {
        out.pop();
    }
}

fn build_rounded_rect_polygon(rect: Rect, radius: f32) -> Vec<Pos2> {
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, radius, &mut ring);
    ring.into_iter().map(|(p, _)| p).collect()
}

#[inline]
fn snap_scalar_to_px(v: f32, ppp: f32) -> f32 {
    let p = ppp.max(0.5);
    (v * p).round() / p
}

#[inline]
fn snap_rect_to_pixels(rect: Rect, ppp: f32) -> Rect {
    let p = ppp.max(0.5);
    let inv = 1.0 / p;
    let x0 = (rect.min.x * p).round() * inv;
    let y0 = (rect.min.y * p).round() * inv;
    let x1 = ((rect.max.x * p).round() * inv).max(x0 + 2.0 * inv);
    let y1 = ((rect.max.y * p).round() * inv).max(y0 + 2.0 * inv);
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
}

/// Paints an unbroken 1px rounded outline ring strictly inside `rect` (DESIGN.md §6.1).
fn paint_crisp_ring_mesh(
    painter: &egui::Painter,
    rect: Rect,
    inset: f32,
    rounding: f32,
    border_color: Color32,
) {
    if rect.width() <= 2.0 || rect.height() <= 2.0 || border_color == Color32::TRANSPARENT {
        return;
    }
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 8 {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.vertices.reserve(n * 2);
    mesh.indices.reserve(n * 6);
    for &(p, norm) in &ring {
        mesh.vertices.push(Vertex {
            pos: p,
            uv: WHITE_UV,
            color: border_color,
        });
        mesh.vertices.push(Vertex {
            pos: p - norm * inset,
            uv: WHITE_UV,
            color: border_color,
        });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let o0 = (i * 2) as u32;
        let i0 = o0 + 1;
        let o1 = (j * 2) as u32;
        let i1 = o1 + 1;
        mesh.indices.extend_from_slice(&[o0, o1, i0, i0, o1, i1]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Paints an unbroken rounded rectangle border & optional fill (DESIGN.md §6.1).
fn paint_crisp_bordered_box(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    fill: Color32,
    border_color: Color32,
    border_px: f32,
    ppp: f32,
) -> Rect {
    let snapped = snap_rect_to_pixels(rect, ppp);
    let inset = (border_px / ppp.max(0.5)).max(1.0 / ppp.max(0.5));
    let inner = snapped.shrink(inset);
    if inner.width() > 0.0 && inner.height() > 0.0 && fill != Color32::TRANSPARENT {
        painter.rect_filled(inner, Rounding::same((rounding - inset).max(0.0)), fill);
    }
    paint_crisp_ring_mesh(painter, snapped, inset, rounding, border_color);
    snapped
}

/// Optically vertically centers galley ink inside `outer_rect` using pixel-snapped cap/x-height metrics.
fn optical_galley_pos(
    ui: &egui::Ui,
    outer_rect: Rect,
    galley: &std::sync::Arc<egui::epaint::Galley>,
    align_right_x: Option<f32>,
    align_center_x: bool,
) -> Pos2 {
    let ppp = ui.ctx().pixels_per_point();
    let x = if let Some(right_x) = align_right_x {
        (right_x - galley.size().x).max(outer_rect.min.x)
    } else if align_center_x {
        outer_rect.center().x - galley.size().x * 0.5
    } else {
        outer_rect.min.x
    };
    let ink_center_y = if let Some(font_id) = galley
        .job
        .sections
        .first()
        .map(|s| s.format.font_id.clone())
    {
        let text = galley.job.text.as_str();
        let has_lower = text.chars().any(|c| c.is_lowercase());
        let has_upper_or_digit = text.chars().any(|c| c.is_uppercase() || c.is_ascii_digit());
        let probe = if has_lower && !has_upper_or_digit { "x" } else { "Hx" };
        let ref_g = ui.fonts(|f| f.layout_no_wrap(probe.to_owned(), font_id, Color32::WHITE));
        if ref_g.mesh_bounds.is_positive() {
            ref_g.mesh_bounds.center().y
        } else if galley.mesh_bounds.is_positive() {
            galley.mesh_bounds.center().y
        } else {
            galley.size().y * 0.5
        }
    } else if galley.mesh_bounds.is_positive() {
        galley.mesh_bounds.center().y
    } else {
        galley.size().y * 0.5
    };
    let y = outer_rect.center().y - ink_center_y;
    Pos2::new(snap_scalar_to_px(x, ppp), snap_scalar_to_px(y, ppp))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    PrimaryOutline,
    SecondaryOutline,
    SubtleOutline,
    DangerStatus,
}

#[allow(clippy::too_many_arguments)]
fn draw_outline_button(
    ui: &mut egui::Ui,
    label: &str,
    min_size: Vec2,
    font_size: f32,
    rounding: f32,
    enabled: bool,
    active_highlight: bool,
    variant: ButtonVariant,
    pal: &ThemePalette,
) -> egui::Response {
    let h_pad = if min_size.y > 0.0 && min_size.y <= 22.0 {
        SPACE_SM
    } else {
        SPACE_MD
    };
    let font_id = FontId::new(font_size.max(10.5), FontFamily::Proportional);
    let measure_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let desired_w = (measure_galley.size().x + h_pad * 2.0).max(min_size.x).round();
    let desired_h = if min_size.y > 0.0 {
        min_size.y.round()
    } else {
        26.0
    };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, desired_h), sense);

    let (fill, border_col, text_color) = if !enabled {
        (Color32::TRANSPARENT, pal.border, pal.muted)
    } else if variant == ButtonVariant::DangerStatus {
        if resp.is_pointer_button_down_on() {
            (STATUS_RED, STATUS_RED, Color32::WHITE)
        } else if resp.hovered() {
            (lerp_color(pal.panel, STATUS_RED, 0.28), STATUS_RED, pal.text)
        } else {
            (pal.pill_red_fill, pal.status_red_text, pal.status_red_text)
        }
    } else if resp.is_pointer_button_down_on() {
        (pal.solid_fill, pal.accent_line, pal.on_solid)
    } else if resp.hovered() {
        (pal.soft_fill_hover, pal.accent_line, pal.text)
    } else if active_highlight {
        (pal.solid_fill, pal.accent_line, pal.on_solid)
    } else {
        match variant {
            ButtonVariant::PrimaryOutline => (pal.soft_fill, pal.accent_line, pal.pill_ink),
            ButtonVariant::SecondaryOutline => (
                Color32::TRANSPARENT,
                lerp_color(pal.border, pal.accent_line, 0.72),
                pal.pill_ink,
            ),
            ButtonVariant::SubtleOutline => (Color32::TRANSPARENT, pal.border, pal.muted),
            ButtonVariant::DangerStatus => (pal.pill_red_fill, pal.status_red_text, pal.status_red_text),
        }
    };

    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, rounding, fill, border_col, 1.0, ppp);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let text_pos = optical_galley_pos(ui, snapped, &galley, None, true);
    ui.painter().galley(text_pos, galley, text_color);
    resp
}

/// Renders a non-interactive full-pill semantic badge (`rounding = 0.5 * h`, >= 10.5pt Regular, DESIGN.md §7).
fn paint_pill_badge(
    ui: &mut egui::Ui,
    label: &str,
    fill: Color32,
    border: Color32,
    ink: Color32,
) {
    let font_id = FontId::new(10.5, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, ink));
    let width = (galley.size().x + SPACE_SM * 2.0).round();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, PILL_BADGE_H), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(
        ui.painter(),
        rect,
        RADIUS_BADGE_PILL,
        fill,
        border,
        1.0,
        ppp,
    );
    let pos = optical_galley_pos(ui, snapped, &galley, None, true);
    ui.painter().galley(pos, galley, ink);
}

/// Renders a 16x16 crisp rounded toggle checkbox matching WT_UI_LIBRARY.md §3 & §4 #6.
fn draw_crisp_checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: &str,
    enabled: bool,
    pal: &ThemePalette,
) -> bool {
    let mut toggled = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (box_rect, resp) = ui.allocate_exact_size(Vec2::splat(16.0), sense);
        let ppp = ui.ctx().pixels_per_point();
        let hovered = enabled && resp.hovered();
        let fill = if !enabled {
            pal.bg
        } else if *checked {
            pal.solid_fill
        } else if hovered {
            pal.soft_fill
        } else {
            pal.input_bg
        };
        let border_col = if !enabled {
            pal.border
        } else if *checked || hovered {
            pal.accent_line
        } else {
            pal.border
        };
        let snapped =
            paint_crisp_bordered_box(ui.painter(), box_rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
        if *checked {
            let c = snapped.center();
            let check_col = if enabled { pal.on_solid } else { pal.muted };
            let st = Stroke::new(1.75_f32, check_col);
            ui.painter()
                .line_segment([Pos2::new(c.x - 4.0, c.y + 0.5), Pos2::new(c.x - 1.0, c.y + 3.5)], st);
            ui.painter()
                .line_segment([Pos2::new(c.x - 1.0, c.y + 3.5), Pos2::new(c.x + 4.5, c.y - 3.0)], st);
        }
        let lbl_resp = ui.add(
            egui::Label::new(
                RichText::new(label)
                    .size(12.0)
                    .color(if enabled { pal.text } else { pal.muted }),
            )
            .sense(sense),
        );
        if enabled && (resp.clicked() || lbl_resp.clicked()) {
            *checked = !*checked;
            toggled = true;
        }
    });
    toggled
}

fn draw_synced_scroll_area<R>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    auto_shrink: [bool; 2],
    pal: &ThemePalette,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    ui.scope(|ui| {
        ui.spacing_mut().scroll.floating = false;
        ui.spacing_mut().scroll.foreground_color = false;
        ui.spacing_mut().scroll.bar_width = 6.0;
        ui.spacing_mut().scroll.bar_inner_margin = SPACE_SM;
        ui.spacing_mut().scroll.bar_outer_margin = 0.0;
        ui.spacing_mut().scroll.handle_min_length = 24.0;
        {
            let v = ui.visuals_mut();
            v.extreme_bg_color = pal.bg;
            v.widgets.inactive.bg_fill = pal.scrollbar_thumb;
            v.widgets.hovered.bg_fill = pal.scrollbar_thumb_hover;
            v.widgets.active.bg_fill = pal.accent_line;
        }
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .auto_shrink(auto_shrink)
            .show(ui, |ui| {
                let v = ui.visuals_mut();
                v.extreme_bg_color = pal.bg;
                v.widgets.inactive.bg_fill = pal.widget;
                v.widgets.hovered.bg_fill = pal.widget_hover;
                v.widgets.active.bg_fill = pal.widget_active;
                body(ui)
            })
    })
    .inner
}

/// Renders the WT masthead lockup (24px unmodified logo mark + title + short muted caption).
fn draw_masthead_lockup_left(ui: &mut egui::Ui, pal: &ThemePalette) {
    let ppp = ui.ctx().pixels_per_point();
    let (mark_rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    if let Some(tex) = masthead_logo_texture(ui.ctx(), 24.0) {
        let snapped_mark = snap_rect_to_pixels(mark_rect, ppp);
        ui.painter().image(
            tex.id(),
            snapped_mark,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    let title_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "z-controller".to_owned(),
            FontId::new(16.0, FontFamily::Proportional),
            pal.text,
        )
    });
    let (title_rect, _) = ui.allocate_exact_size(
        Vec2::new(title_galley.size().x.ceil(), HEADER_CONTROL_H),
        Sense::hover(),
    );
    let title_pos = optical_galley_pos(ui, title_rect, &title_galley, None, false);
    ui.painter().galley(title_pos, title_galley, pal.text);

    let cap_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            MASTHEAD_CAPTION.to_owned(),
            FontId::new(11.5, FontFamily::Proportional),
            pal.muted,
        )
    });
    let (cap_rect, _) = ui.allocate_exact_size(
        Vec2::new(cap_galley.size().x.ceil(), HEADER_CONTROL_H),
        Sense::hover(),
    );
    let cap_pos = optical_galley_pos(ui, cap_rect, &cap_galley, None, false);
    ui.painter().galley(cap_pos, cap_galley, pal.muted);
}

fn paint_active_card_gradient_border(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    pal: &ThemePalette,
) {
    if rect.width() <= 4.0 || rect.height() <= 4.0 {
        return;
    }
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 8 {
        return;
    }

    // Within-family monochrome sweep strictly bounded inside `rect` (WT_UI_LIBRARY.md §0.4).
    const BANDS: usize = 5;
    let offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.0_f32];
    let alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8];

    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);

    let mut mesh = Mesh::default();
    mesh.vertices.reserve(n * BANDS);
    mesh.indices.reserve(n * (BANDS - 1) * 6);

    for (p, norm) in &ring {
        let t = (((p.x - rect.left()) / w) * 0.65 + ((p.y - rect.top()) / h) * 0.35).clamp(0.0, 1.0);
        let c = if t <= 0.5 {
            lerp_color(pal.rim_stops[0], pal.rim_stops[1], t * 2.0)
        } else {
            lerp_color(pal.rim_stops[1], pal.rim_stops[2], (t - 0.5) * 2.0)
        };
        for (&off, &a) in offsets.iter().zip(alphas.iter()) {
            let color = if a == 0 {
                Color32::TRANSPARENT
            } else {
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
            };
            mesh.vertices.push(Vertex {
                pos: *p + *norm * off,
                uv: WHITE_UV,
                color,
            });
        }
    }

    let stride = BANDS as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        let b0 = (i as u32) * stride;
        let b1 = (j as u32) * stride;
        for band in 0..(stride - 1) {
            let i0 = b0 + band;
            let i1 = b0 + band + 1;
            let i2 = b1 + band;
            let i3 = b1 + band + 1;
            mesh.indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    painter.add(egui::Shape::mesh(mesh));
}

fn draw_aligned_divider(ui: &mut egui::Ui, left: f32, right: f32, pal: &ThemePalette) {
    let y = ui.cursor().top();
    ui.painter()
        .line_segment([Pos2::new(left, y), Pos2::new(right, y)], Stroke::new(1.0_f32, pal.border));
    ui.add_space(SPACE_SM);
}

fn draw_theme_toggle_button(ui: &mut egui::Ui, mode: ThemeMode, pal: &ThemePalette) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HEADER_CONTROL_H), Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let (fill, border) = if pressed {
        (pal.solid_fill, pal.accent_line)
    } else if hovered {
        (pal.soft_fill_hover, pal.accent_line)
    } else {
        (
            Color32::TRANSPARENT,
            lerp_color(pal.border, pal.accent_line, 0.72),
        )
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_CONTROL, fill, border, 1.0, ppp);
    let c = snapped.center();
    let p = ui.painter();
    let fg = if pressed {
        pal.on_solid
    } else if hovered {
        pal.text
    } else {
        pal.pill_ink
    };
    match mode {
        ThemeMode::Dark => {
            p.circle_stroke(c, 3.2, Stroke::new(1.3_f32, fg));
            for k in 0..8 {
                let a = (k as f32) * (PI * 0.25);
                let dir = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + dir * 5.0, c + dir * 7.2], Stroke::new(1.25_f32, fg));
            }
        }
        ThemeMode::Light => {
            let cutout = if pressed || hovered { fill } else { pal.bg };
            p.circle_filled(c + Vec2::new(-0.5, 0.2), 5.3, fg);
            p.circle_filled(c + Vec2::new(2.2, -1.9), 4.4, cutout);
        }
    }
    resp.clicked()
}

fn draw_theme_swatch_button(
    ui: &mut egui::Ui,
    accent: ThemeAccent,
    picker_open: bool,
    pal: &ThemePalette,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HEADER_CONTROL_H), Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let (fill, border) = if pressed {
        (pal.solid_fill, pal.accent_line)
    } else if hovered || picker_open {
        (pal.soft_fill_hover, pal.accent_line)
    } else {
        (
            Color32::TRANSPARENT,
            lerp_color(pal.border, pal.accent_line, 0.72),
        )
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_CONTROL, fill, border, 1.0, ppp);
    let swatch_rect = Rect::from_center_size(snapped.center(), Vec2::splat(14.0));
    paint_crisp_bordered_box(
        ui.painter(),
        swatch_rect,
        RADIUS_PILL,
        pal.accent_source,
        pal.accent_line,
        1.0,
        ppp,
    );
    resp
        .on_hover_text(format!("Theme colour: {} (click to change)", accent.display_name()))
        .clicked()
}

fn draw_collapse_chevron_button(ui: &mut egui::Ui, expanded: bool, pal: &ThemePalette) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let (fill, border, fg) = if pressed {
        (pal.solid_fill, pal.accent_line, pal.on_solid)
    } else if hovered {
        (pal.soft_fill_hover, pal.accent_line, pal.text)
    } else {
        (Color32::TRANSPARENT, pal.border, pal.muted)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_PILL, fill, border, 1.0, ppp);
    let c = snapped.center();
    let p = ui.painter();
    let st = Stroke::new(1.5_f32, fg);
    if expanded {
        p.line_segment([Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.5)], st);
        p.line_segment([Pos2::new(c.x, c.y + 2.5), Pos2::new(c.x + 4.0, c.y - 2.0)], st);
    } else {
        p.line_segment([Pos2::new(c.x + 2.0, c.y - 4.0), Pos2::new(c.x - 2.5, c.y)], st);
        p.line_segment([Pos2::new(c.x - 2.5, c.y), Pos2::new(c.x + 2.0, c.y + 4.0)], st);
    }
    resp.clicked()
}

/// Renders an unambiguous semantic status circle (`✓` OK / `✕` Attention) so status never relies on colour alone (WT_UI_LIBRARY.md §0.8 & §4B S10).
fn draw_status_icon(ui: &mut egui::Ui, ok: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let c = rect.center();
    let p = ui.painter();
    if ok {
        p.circle_filled(c, 5.5, STATUS_GREEN);
        let ink = Color32::from_rgb(10, 18, 12);
        let st = Stroke::new(1.5_f32, ink);
        p.line_segment([Pos2::new(c.x - 2.6, c.y + 0.1), Pos2::new(c.x - 0.6, c.y + 2.3)], st);
        p.line_segment([Pos2::new(c.x - 0.6, c.y + 2.3), Pos2::new(c.x + 2.8, c.y - 2.0)], st);
    } else {
        p.circle_filled(c, 5.5, STATUS_RED);
        let st = Stroke::new(1.4_f32, Color32::WHITE);
        p.line_segment([Pos2::new(c.x - 2.2, c.y - 2.2), Pos2::new(c.x + 2.2, c.y + 2.2)], st);
        p.line_segment([Pos2::new(c.x + 2.2, c.y - 2.2), Pos2::new(c.x - 2.2, c.y + 2.2)], st);
    }
}

fn draw_neutral_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

fn paint_monochrome_sweep_bar(
    painter: &egui::Painter,
    full_track: Rect,
    fill_rect: Rect,
    stops: [Color32; 3],
) {
    if fill_rect.width() <= 0.5 || fill_rect.height() <= 0.5 || full_track.width() <= 0.5 {
        return;
    }
    let inner = fill_rect;
    let r = (3.0_f32).min(inner.height() * 0.5).min(inner.width() * 0.5);
    let x0 = inner.min.x;
    let x1 = inner.max.x;
    let y0 = inner.min.y;
    let y1 = inner.max.y;

    let mut xs: Vec<f32> = Vec::with_capacity(18);
    xs.push(x0);
    if r > 0.25 {
        for k in 1..=4 {
            let a = (k as f32) / 5.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x0 + r * (1.0 - a.cos()));
        }
    }
    let mid_x = full_track.center().x;
    if mid_x > x0 + r + 0.5 && mid_x < x1 - r - 0.5 {
        xs.push(mid_x);
    }
    if r > 0.25 {
        for k in 1..=4 {
            let a = (k as f32) / 5.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x1 - r * (1.0 - a.sin()));
        }
    }
    xs.push(x1);
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);

    let sweep_at = |x: f32| -> Color32 {
        let t = ((x - full_track.min.x) / full_track.width()).clamp(0.0, 1.0);
        if t <= 0.5 {
            lerp_color(stops[0], stops[1], t * 2.0)
        } else {
            lerp_color(stops[1], stops[2], (t - 0.5) * 2.0)
        }
    };

    if xs.len() < 2 {
        painter.rect_filled(inner, Rounding::same(r), sweep_at(inner.center().x));
        return;
    }

    let mut mesh = Mesh::default();
    let mut prev_t = 0u32;
    let mut prev_b = 0u32;
    for (idx, &x) in xs.iter().enumerate() {
        let (ty, by) = if r > 0.0 && x < x0 + r {
            let dx = (x0 + r) - x;
            let dy = (r * r - dx * dx).max(0.0).sqrt();
            (y0 + r - dy, y1 - r + dy)
        } else if r > 0.0 && x > x1 - r {
            let dx = x - (x1 - r);
            let dy = (r * r - dx * dx).max(0.0).sqrt();
            (y0 + r - dy, y1 - r + dy)
        } else {
            (y0, y1)
        };
        let col = sweep_at(x);
        let vt = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: Pos2::new(x, ty),
            uv: WHITE_UV,
            color: col,
        });
        let vb = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: Pos2::new(x, by),
            uv: WHITE_UV,
            color: col,
        });
        if idx > 0 {
            mesh.indices.extend_from_slice(&[prev_t, vt, prev_b, prev_b, vt, vb]);
        }
        prev_t = vt;
        prev_b = vb;
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn draw_battery_bar(
    ui: &mut egui::Ui,
    battery: &z_core::battery::BatteryReading,
    pal: &ThemePalette,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        ui.label(
            RichText::new("Battery:")
                .size(11.0)
                .color(pal.muted),
        );
        let bar_size = Vec2::new(124.0, 8.0);
        let (rect, _) = ui.allocate_exact_size(bar_size, Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(4.0), pal.bg);
        p.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, pal.border));

        let fraction = match (battery.percent, battery.status) {
            (Some(pct), _) => (pct as f32 / 100.0).clamp(0.0, 1.0),
            (None, BatteryStatus::Wired) => 1.0,
            _ => 0.0,
        };
        let low = battery_is_low(battery.percent);
        if fraction > 0.0 {
            let inner = rect.shrink(1.0);
            let fill_w = (inner.width() * fraction).max(4.0).min(inner.width());
            let fill_rect = Rect::from_min_size(inner.min, Vec2::new(fill_w, inner.height()));
            if low {
                p.rect_filled(fill_rect, Rounding::same(3.0), STATUS_RED);
            } else {
                paint_monochrome_sweep_bar(p, inner, fill_rect, pal.rim_stops);
            }
        }

        if low {
            let (warn_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            let c = warn_rect.center();
            let tri = vec![
                Pos2::new(c.x, c.y - 5.0),
                Pos2::new(c.x + 5.0, c.y + 4.5),
                Pos2::new(c.x - 5.0, c.y + 4.5),
            ];
            ui.painter().add(egui::Shape::Path(PathShape::convex_polygon(
                tri,
                STATUS_RED,
                Stroke::NONE,
            )));
            ui.painter().line_segment(
                [Pos2::new(c.x, c.y - 2.2), Pos2::new(c.x, c.y + 1.0)],
                Stroke::new(1.3_f32, Color32::WHITE),
            );
            ui.painter().circle_filled(Pos2::new(c.x, c.y + 2.8), 0.9, Color32::WHITE);
            ui.label(
                RichText::new(format!("{} Low", battery.format_badge()))
                    .font(FontId::new(11.0, FontFamily::Monospace))
                    .color(pal.status_red_text),
            );
        } else {
            ui.label(
                RichText::new(battery.format_badge())
                    .font(FontId::new(11.0, FontFamily::Monospace))
                    .color(pal.text),
            );
        }
    });
}

fn draw_theme_picker_popover(
    ctx: &egui::Context,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    pal: &ThemePalette,
    send: &mut impl FnMut(EngineCommand),
) {
    if !state.theme_picker_open {
        return;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.theme_picker_open = false;
        return;
    }

    let screen = ctx.screen_rect();
    let popover_w = (screen.width() - SPACE_MD * 2.0).min(344.0).max(260.0);
    let active_accent = snap.config.theme_accent;

    egui::Area::new(egui::Id::new("theme_engine_picker_popover"))
        .order(egui::Order::Foreground)
        .anchor(
            Align2::RIGHT_TOP,
            Vec2::new(-SPACE_MD, SPACE_MD + HEADER_CONTROL_H + SPACE_SM),
        )
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(pal.panel)
                .rounding(CARD_ROUNDING)
                .stroke(Stroke::new(1.0_f32, pal.accent_line))
                .inner_margin(Margin::same(SPACE_MD))
                .show(ui, |ui| {
                    ui.set_width(popover_w);
                    ui.spacing_mut().item_spacing.y = SPACE_SM;

                    // Header row
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                        ui.label(
                            RichText::new("Theme Accent")
                                .font(strong_font(13.5))
                                .color(pal.text),
                        );
                        paint_pill_badge(
                            ui,
                            match snap.config.theme_mode {
                                ThemeMode::Dark => "Dark",
                                ThemeMode::Light => "Light",
                            },
                            pal.soft_fill,
                            pal.accent_line,
                            pal.pill_ink,
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if draw_outline_button(
                                ui,
                                "×",
                                Vec2::new(24.0, 22.0),
                                12.0,
                                RADIUS_PILL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.theme_picker_open = false;
                            }
                        });
                    });

                    // 7 curated preset chips in clean 4 + 3 rows
                    ui.label(RichText::new("Presets").size(11.0).color(pal.muted));
                    let chip_w = ((popover_w - SPACE_SM * 3.0) / 4.0).floor();
                    for chunk in ThemePreset::ALL.chunks(4) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            for &preset in chunk {
                                let cand = ThemeAccent::Preset { preset };
                                let selected = active_accent == cand;
                                let ladder = MonochromeLadder::from_accent(cand);
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(chip_w, 26.0),
                                    Sense::click(),
                                );
                                let ppp = ui.ctx().pixels_per_point();
                                let (fill, border, text_col) = if resp.is_pointer_button_down_on() {
                                    (pal.solid_fill, pal.accent_line, pal.on_solid)
                                } else if selected {
                                    (pal.soft_fill, pal.accent_line, pal.text)
                                } else if resp.hovered() {
                                    (pal.widget_hover, pal.accent_line, pal.text)
                                } else {
                                    (Color32::TRANSPARENT, pal.border, pal.muted)
                                };
                                let snapped = paint_crisp_bordered_box(
                                    ui.painter(),
                                    rect,
                                    RADIUS_CONTROL,
                                    fill,
                                    border,
                                    1.0,
                                    ppp,
                                );
                                let sw_rect = Rect::from_center_size(
                                    Pos2::new(snapped.left() + 12.0, snapped.center().y),
                                    Vec2::splat(11.0),
                                );
                                paint_crisp_bordered_box(
                                    ui.painter(),
                                    sw_rect,
                                    3.0,
                                    ladder.source,
                                    ladder.deep,
                                    1.0,
                                    ppp,
                                );
                                let lbl_galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        preset.label().to_owned(),
                                        FontId::new(11.0, FontFamily::Proportional),
                                        text_col,
                                    )
                                });
                                let lbl_rect = Rect::from_min_max(
                                    Pos2::new(sw_rect.right() + 5.0, snapped.top()),
                                    snapped.right_bottom(),
                                );
                                let lpos = optical_galley_pos(ui, lbl_rect, &lbl_galley, None, false);
                                ui.painter().galley(lpos, lbl_galley, text_col);
                                if resp
                                    .on_hover_text(format!("{} ({})", preset.label(), preset.hex_string()))
                                    .clicked()
                                {
                                    state.custom_rgb = preset.base_rgb();
                                    state.custom_hex_input = preset.hex_string();
                                    state.custom_hex_error = None;
                                    send(EngineCommand::SetThemeAccent(cand));
                                }
                            }
                        });
                    }

                    ui.add_space(SPACE_XS);
                    ui.label(
                        RichText::new("Custom Hex / RGB (contrast-normalised)")
                            .size(11.0)
                            .color(pal.muted),
                    );

                    // Hex input row
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                        let hex_id = ui.make_persistent_id("theme_custom_hex_input");
                        let focused = ui.ctx().memory(|m| m.has_focus(hex_id));
                        let hex_resp = egui::Frame::none()
                            .fill(pal.input_bg)
                            .rounding(Rounding::same(RADIUS_CONTROL))
                            .stroke(Stroke::new(
                                1.0_f32,
                                if state.custom_hex_error.is_some() {
                                    pal.status_red_text
                                } else if focused {
                                    pal.accent_line
                                } else {
                                    pal.border
                                },
                            ))
                            .inner_margin(Margin::symmetric(SPACE_SM, SPACE_XS))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.custom_hex_input)
                                        .id(hex_id)
                                        .frame(false)
                                        .desired_width(116.0)
                                        .font(FontId::new(11.5, FontFamily::Monospace))
                                        .text_color(pal.text)
                                        .hint_text(
                                            RichText::new("#7CCBF2")
                                                .font(FontId::new(11.5, FontFamily::Monospace))
                                                .color(pal.hint_text),
                                        ),
                                )
                            })
                            .inner;

                        if hex_resp.changed() {
                            if let Some(rgb) = ThemeAccent::parse_hex(&state.custom_hex_input) {
                                state.custom_rgb = rgb;
                                state.custom_hex_error = None;
                            }
                        }

                        if draw_outline_button(
                            ui,
                            "Apply Hex",
                            Vec2::new(76.0, 26.0),
                            11.5,
                            RADIUS_CONTROL,
                            true,
                            false,
                            ButtonVariant::PrimaryOutline,
                            pal,
                        )
                        .clicked()
                            || (hex_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            if let Some([r, g, b]) = ThemeAccent::parse_hex(&state.custom_hex_input) {
                                state.custom_rgb = [r, g, b];
                                state.custom_hex_input = format!("#{r:02X}{g:02X}{b:02X}");
                                state.custom_hex_error = None;
                                send(EngineCommand::SetThemeAccent(ThemeAccent::Custom { r, g, b }));
                            } else {
                                state.custom_hex_error = Some("Use #RRGGBB (6 hex digits)".to_string());
                            }
                        }
                    });

                    if let Some(err) = &state.custom_hex_error {
                        ui.label(RichText::new(err).size(10.5).color(pal.status_red_text));
                    }

                    // Compact RGB sliders updating Custom live
                    let mut rgb_changed = false;
                    for (idx, lbl) in [(0, "R"), (1, "G"), (2, "B")] {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            ui.label(
                                RichText::new(lbl)
                                    .font(FontId::new(10.5, FontFamily::Monospace))
                                    .color(pal.muted),
                            );
                            let mut val = state.custom_rgb[idx] as u32;
                            if ui
                                .add(egui::Slider::new(&mut val, 0..=255).show_value(true))
                                .changed()
                            {
                                state.custom_rgb[idx] = val as u8;
                                rgb_changed = true;
                            }
                        });
                    }
                    if rgb_changed {
                        let [r, g, b] = state.custom_rgb;
                        state.custom_hex_input = format!("#{r:02X}{g:02X}{b:02X}");
                        state.custom_hex_error = None;
                        send(EngineCommand::SetThemeAccent(ThemeAccent::Custom { r, g, b }));
                    }

                    // Contrast-normalised readout
                    let ladder = MonochromeLadder::from_accent(active_accent);
                    let dark_cr = contrast_ratio(ladder.accent_dark, Color32::from_rgb(25, 25, 25));
                    let light_cr = contrast_ratio(ladder.deep, Color32::WHITE);
                    ui.label(
                        RichText::new(format!(
                            "Dark {} ({dark_cr:.1}:1) · Light {} ({light_cr:.1}:1)",
                            MonochromeLadder::format_hex(ladder.accent_dark),
                            MonochromeLadder::format_hex(ladder.deep),
                        ))
                        .font(FontId::new(10.5, FontFamily::Monospace))
                        .color(pal.subtle_text),
                    );
                });
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
    let accent = snap.config.theme_accent;
    if state.applied_theme != Some(mode) || state.applied_accent != Some(accent) {
        apply_theme_visuals(ctx, mode, accent);
        state.applied_theme = Some(mode);
        state.applied_accent = Some(accent);
        state.custom_rgb = accent.base_rgb();
        state.custom_hex_input = accent.hex_string();
    }
    let pal = ThemePalette::for_theme(mode, accent);

    state.last_dynamic_rect_main = None;
    let all_drivers_working =
        snap.driver_status.vigem_working() && snap.driver_status.hidhide_working();
    if state.last_all_drivers_working != Some(all_drivers_working) {
        state.drivers_section_override = None;
        state.last_all_drivers_working = Some(all_drivers_working);
    }
    let show_drivers_section = state
        .drivers_section_override
        .unwrap_or(!all_drivers_working);
    let status_bar_h = 34.0_f32;

    // Bottom status bar (DESIGN.md §2.1: pinned footer uses panel_alt surface)
    egui::TopBottomPanel::bottom("compact_status_bar")
        .exact_height(status_bar_h)
        .frame(
            egui::Frame::none()
                .fill(pal.panel_alt)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            let sr = ctx.screen_rect();
            let top_y = ui.max_rect().top() - SPACE_SM;
            ui.painter().line_segment(
                [Pos2::new(sr.left(), top_y), Pos2::new(sr.right(), top_y)],
                Stroke::new(1.0_f32, pal.border),
            );
            ui.horizontal_centered(|ui| {
                if draw_status_bar_row(ui, snap, show_drivers_section, &pal) {
                    state.drivers_section_override = Some(!show_drivers_section);
                }
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(pal.bg)
                .inner_margin(Margin::same(SPACE_MD)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = SPACE_SM;
            let header_w = ui.available_width();

            // FIX 1: WT Header Lockup — 24px unmodified logo mark + title + functional caption + app-level controls on right.
            ui.allocate_ui_with_layout(
                Vec2::new(header_w, HEADER_CONTROL_H),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    draw_masthead_lockup_left(ui, &pal);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                        if draw_outline_button(
                            ui,
                            "Refresh",
                            Vec2::new(68.0, HEADER_CONTROL_H),
                            12.0,
                            RADIUS_CONTROL,
                            true,
                            false,
                            ButtonVariant::SecondaryOutline,
                            &pal,
                        )
                        .clicked()
                        {
                            send(EngineCommand::RefreshStatus);
                        }
                        if draw_theme_toggle_button(ui, mode, &pal) {
                            send(EngineCommand::SetThemeMode(mode.toggled()));
                        }
                        if draw_theme_swatch_button(ui, accent, state.theme_picker_open, &pal) {
                            state.theme_picker_open = !state.theme_picker_open;
                        }
                    });
                },
            );

            ui.add(egui::Label::new(
                RichText::new("Controllers").size(13.5).color(pal.text),
            ));

            let mut content_bottom_y: Option<f32> = None;
            draw_synced_scroll_area(ui, "main_controllers_scroll", [false, true], &pal, |ui| {
                let stack_resp = ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let avail_col = ui.available_width();
                    ui.set_max_width(avail_col);
                    let card_inner_w = (avail_col - SPACE_MD * 2.0 - 2.0).max(200.0);

                    if snap.controllers.is_empty() {
                        egui::Frame::none()
                            .fill(pal.panel)
                            .rounding(CARD_ROUNDING)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::same(SPACE_MD))
                            .show(ui, |ui| {
                                ui.set_width(card_inner_w);
                                ui.spacing_mut().item_spacing.y = SPACE_XS;
                                ui.label(
                                    RichText::new("No physical controllers detected.")
                                        .size(13.0)
                                        .color(pal.text),
                                );
                                ui.label(
                                    RichText::new("Connect a controller via Bluetooth or USB.")
                                        .size(11.0)
                                        .color(pal.muted),
                                );
                            });
                    } else {
                        let total_cards = snap.controllers.len();
                        for (idx, ctrl) in snap.controllers.iter().enumerate() {
                            let is_selected =
                                state.selected_controller.as_deref() == Some(&ctrl.canonical_id);
                            let card_fill = if is_selected {
                                pal.widget
                            } else {
                                pal.panel
                            };
                            let card_bg_slot = if ctrl.virtual_active {
                                Some(ui.painter().add(egui::Shape::Noop))
                            } else {
                                None
                            };
                            let card_stroke = if ctrl.virtual_active {
                                Stroke::NONE
                            } else {
                                Stroke::new(
                                    1.0_f32,
                                    if is_selected {
                                        pal.accent_line
                                    } else {
                                        pal.border
                                    },
                                )
                            };
                            let frame_fill = if ctrl.virtual_active {
                                Color32::TRANSPARENT
                            } else {
                                card_fill
                            };

                            let card_resp = egui::Frame::none()
                                .fill(frame_fill)
                                .rounding(CARD_ROUNDING)
                                .inner_margin(Margin::same(SPACE_MD))
                                .stroke(card_stroke)
                                .show(ui, |ui| {
                                    ui.set_width(card_inner_w);
                                    ui.spacing_mut().item_spacing = Vec2::new(SPACE_SM, SPACE_XS);

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        draw_status_icon(ui, ctrl.connected);
                                        let title_resp = ui.add(
                                            egui::Label::new(
                                                RichText::new(&ctrl.name)
                                                    .font(strong_font(13.5))
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

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        ui.label(
                                            RichText::new(format!(
                                                "{} | {} | ",
                                                ctrl.family.label(),
                                                ctrl.transport.label(),
                                            ))
                                            .size(11.0)
                                            .color(pal.muted),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{:04X}:{:04X}",
                                                ctrl.key.vid, ctrl.key.pid
                                            ))
                                            .font(FontId::new(11.0, FontFamily::Monospace))
                                            .color(pal.subtle_text),
                                        );
                                    });

                                    draw_battery_bar(ui, &ctrl.battery, &pal);

                                    let vigem_ok = snap.driver_status.vigem_working();
                                    let hidhide_ok = snap.driver_status.hidhide_working();

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        draw_status_icon(ui, ctrl.enabled && vigem_ok);
                                        let mut enabled = ctrl.enabled;
                                        if draw_crisp_checkbox(
                                            ui,
                                            &mut enabled,
                                            "Active",
                                            true,
                                            &pal,
                                        ) {
                                            if enabled && !vigem_ok {
                                                state.missing_vigem_notice_controller =
                                                    Some(ctrl.canonical_id.clone());
                                            } else {
                                                state.missing_vigem_notice_controller = None;
                                                send(EngineCommand::SetDeviceEnabled {
                                                    key: ctrl.key.clone(),
                                                    enabled,
                                                });
                                            }
                                        }
                                        if ctrl.virtual_active {
                                            paint_pill_badge(
                                                ui,
                                                &format!("-> {}", ctrl.target_kind.label()),
                                                pal.soft_fill,
                                                pal.accent_line,
                                                pal.pill_ink,
                                            );
                                        }
                                        if hidhide_ok {
                                            draw_status_icon(ui, ctrl.cloaked);
                                            ui.label(
                                                RichText::new(if ctrl.cloaked {
                                                    "Cloaked"
                                                } else {
                                                    "Visible"
                                                })
                                                .size(11.0)
                                                .color(if ctrl.cloaked {
                                                    pal.text
                                                } else {
                                                    pal.muted
                                                }),
                                            );
                                        } else {
                                            draw_status_icon(ui, false);
                                            ui.label(
                                                RichText::new("Cloak off (no HidHide)")
                                                    .size(11.0)
                                                    .color(pal.muted),
                                            );
                                        }
                                    });

                                    if state.missing_vigem_notice_controller.as_deref()
                                        == Some(&ctrl.canonical_id)
                                        && !vigem_ok
                                    {
                                        egui::Frame::none()
                                            .fill(pal.pill_red_fill)
                                            .rounding(Rounding::same(RADIUS_CONTROL))
                                            .stroke(Stroke::new(1.0_f32, pal.status_red_text))
                                            .inner_margin(Margin::same(SPACE_SM))
                                            .show(ui, |ui| {
                                                ui.spacing_mut().item_spacing.y = SPACE_XS;
                                                ui.label(
                                                    RichText::new(format!(
                                                        "Virtual pad needs ViGEmBus ({}).",
                                                        snap.driver_status
                                                            .vigem
                                                            .state
                                                            .status_label()
                                                    ))
                                                    .size(11.0)
                                                    .color(pal.text),
                                                );
                                                ui.horizontal(|ui| {
                                                    if let Some(rem) =
                                                        remedy_for_entry(&snap.driver_status.vigem)
                                                    {
                                                        if draw_outline_button(
                                                            ui,
                                                            rem.button_label(),
                                                            Vec2::ZERO,
                                                            11.5,
                                                            RADIUS_CONTROL,
                                                            true,
                                                            false,
                                                            ButtonVariant::PrimaryOutline,
                                                            &pal,
                                                        )
                                                        .clicked()
                                                        {
                                                            if rem.requires_reboot_confirm() {
                                                                state.pending_reboot_confirm =
                                                                    Some(DriverId::ViGEmBus);
                                                            } else {
                                                                send(
                                                                    EngineCommand::RunDriverRemedy(
                                                                        DriverId::ViGEmBus,
                                                                    ),
                                                                );
                                                            }
                                                        }
                                                    } else if draw_outline_button(
                                                        ui,
                                                        "Install ViGEmBus…",
                                                        Vec2::ZERO,
                                                        11.5,
                                                        RADIUS_CONTROL,
                                                        true,
                                                        false,
                                                        ButtonVariant::PrimaryOutline,
                                                        &pal,
                                                    )
                                                    .clicked()
                                                    {
                                                        state.pending_install_confirm =
                                                            Some(DriverId::ViGEmBus);
                                                    }
                                                    if draw_outline_button(
                                                        ui,
                                                        "Dismiss",
                                                        Vec2::ZERO,
                                                        11.5,
                                                        RADIUS_CONTROL,
                                                        true,
                                                        false,
                                                        ButtonVariant::SubtleOutline,
                                                        &pal,
                                                    )
                                                    .clicked()
                                                    {
                                                        state.missing_vigem_notice_controller =
                                                            None;
                                                    }
                                                });
                                            });
                                    }

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        ui.label(
                                            RichText::new("Profile:")
                                                .size(11.0)
                                                .color(pal.muted),
                                        );
                                        let edit_text_w = ui.fonts(|f| {
                                            f.layout_no_wrap(
                                                "Edit Profile".to_owned(),
                                                FontId::new(11.5, FontFamily::Proportional),
                                                pal.text,
                                            )
                                            .size()
                                            .x
                                        });
                                        let edit_btn_w =
                                            (edit_text_w + SPACE_MD * 2.0).ceil();
                                        let combo_w = (ui.available_width()
                                            - edit_btn_w
                                            - SPACE_SM)
                                            .max(120.0);
                                        egui::ComboBox::from_id_salt((
                                            "ctrl_prof",
                                            &ctrl.canonical_id,
                                        ))
                                        .width(combo_w)
                                        .truncate()
                                        .selected_text(
                                            RichText::new(&ctrl.profile_name)
                                                .size(11.5)
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

                                        if draw_outline_button(
                                            ui,
                                            "Edit Profile",
                                            Vec2::new(edit_btn_w, 26.0),
                                            11.5,
                                            RADIUS_CONTROL,
                                            true,
                                            false,
                                            ButtonVariant::PrimaryOutline,
                                            &pal,
                                        )
                                        .clicked()
                                        {
                                            state.selected_controller =
                                                Some(ctrl.canonical_id.clone());
                                            state.editing_profile_id =
                                                Some(ctrl.profile_id.clone());
                                            state.draft_profile = Some(
                                                snap.config
                                                    .profile_by_id(&ctrl.profile_id)
                                                    .clone(),
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

                            if let Some(slot) = card_bg_slot {
                                let inner_poly = build_rounded_rect_polygon(
                                    card_resp.response.rect.shrink(1.0),
                                    (CARD_ROUNDING - 1.0).max(0.0),
                                );
                                ui.painter().set(
                                    slot,
                                    egui::Shape::convex_polygon(
                                        inner_poly,
                                        card_fill,
                                        Stroke::NONE,
                                    ),
                                );
                                paint_active_card_gradient_border(
                                    ui.painter(),
                                    card_resp.response.rect,
                                    CARD_ROUNDING,
                                    &pal,
                                );
                            }

                            if idx + 1 < total_cards {
                                ui.add_space(SPACE_MD);
                            }
                        }
                    }

                    if show_drivers_section {
                        ui.add_space(SPACE_MD);
                        draw_drivers_section(
                            ui,
                            state,
                            snap,
                            card_inner_w,
                            &pal,
                            &mut actions,
                            &mut send,
                        );
                    }
                });
                content_bottom_y = Some(stack_resp.response.rect.bottom());
            });
            if let Some(cb) = content_bottom_y {
                state.desired_compact_logical_h = Some((cb + SPACE_MD + status_bar_h).ceil());
            }
        });

    draw_theme_picker_popover(ctx, state, snap, &pal, &mut send);
    draw_install_confirmation_modal(ctx, state, snap, &pal, &mut send);
    draw_reboot_confirmation_modal(ctx, state, &pal, &mut send);

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
    let accent = snap.config.theme_accent;
    if state.applied_theme != Some(mode) || state.applied_accent != Some(accent) {
        apply_theme_visuals(ctx, mode, accent);
        state.applied_theme = Some(mode);
        state.applied_accent = Some(accent);
        state.custom_rgb = accent.base_rgb();
        state.custom_hex_input = accent.hex_string();
    }
    let pal = ThemePalette::for_theme(mode, accent);

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
                .fill(pal.panel_alt)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                if let Some(tex) = masthead_logo_texture(ui.ctx(), 24.0) {
                    let ppp = ui.ctx().pixels_per_point();
                    let (mark_rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                    ui.painter().image(
                        tex.id(),
                        snap_rect_to_pixels(mark_rect, ppp),
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                ui.label(RichText::new("z-controller").size(15.0).color(pal.text));
                ui.label(RichText::new("•").size(11.5).color(pal.subtle_text));
                ui.label(RichText::new("Editing Profile:").size(12.0).color(pal.muted));
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
                    .selected_text(RichText::new(cur_name).size(12.0).color(pal.text))
                    .show_ui(ui, |ui| {
                        for p in &snap.config.profiles {
                            if ui.selectable_label(p.id == cur_id, &p.name).clicked() {
                                state.editing_profile_id = Some(p.id.clone());
                                state.draft_profile = Some(p.clone());
                                state.draft_dirty = false;
                            }
                        }
                    });

                if draw_outline_button(
                    ui,
                    "+ New",
                    Vec2::new(0.0, HEADER_CONTROL_H),
                    12.0,
                    RADIUS_CONTROL,
                    true,
                    false,
                    ButtonVariant::SecondaryOutline,
                    &pal,
                )
                .clicked()
                {
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
                        if !Profile::is_builtin_id(id)
                            && draw_outline_button(
                                ui,
                                "Delete",
                                Vec2::new(0.0, HEADER_CONTROL_H),
                                12.0,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                &pal,
                            )
                            .clicked()
                        {
                            send(EngineCommand::DeleteProfile(id.clone()));
                            let fallback = snap.config.default_profile_id.clone();
                            state.editing_profile_id = Some(fallback.clone());
                            state.draft_profile = Some(snap.config.profile_by_id(&fallback).clone());
                            state.draft_dirty = false;
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    if let Some(prof) = state.draft_profile.as_ref() {
                        let btn_text = if state.draft_dirty {
                            "Apply & Save *"
                        } else {
                            "Apply & Save"
                        };
                        if draw_outline_button(
                            ui,
                            btn_text,
                            Vec2::new(0.0, HEADER_CONTROL_H),
                            12.0,
                            RADIUS_CONTROL,
                            true,
                            state.draft_dirty,
                            ButtonVariant::PrimaryOutline,
                            &pal,
                        )
                        .clicked()
                        {
                            send(EngineCommand::UpsertProfile(prof.clone()));
                            state.draft_dirty = false;
                        }
                    }
                    if draw_theme_toggle_button(ui, mode, &pal) {
                        send(EngineCommand::SetThemeMode(mode.toggled()));
                    }
                    if draw_theme_swatch_button(ui, accent, state.theme_picker_open, &pal) {
                        state.theme_picker_open = !state.theme_picker_open;
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
                            .spacing([SPACE_MD, SPACE_SM])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Profile Name:").size(11.0).color(pal.muted));
                                ui.horizontal(|ui| {
                                    let name_id = ui.make_persistent_id("editor_profile_name_input");
                                    let focused = ui.ctx().memory(|m| m.has_focus(name_id));
                                    egui::Frame::none()
                                        .fill(pal.input_bg)
                                        .rounding(Rounding::same(RADIUS_CONTROL))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            if focused { pal.accent_line } else { pal.border },
                                        ))
                                        .inner_margin(Margin::symmetric(SPACE_SM, SPACE_XS))
                                        .show(ui, |ui| {
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut prof.name)
                                                        .id(name_id)
                                                        .frame(false)
                                                        .desired_width(196.0)
                                                        .font(FontId::new(12.0, FontFamily::Proportional))
                                                        .text_color(pal.text)
                                                        .hint_text(
                                                            RichText::new("Profile name")
                                                                .size(12.0)
                                                                .color(pal.hint_text),
                                                        ),
                                                )
                                                .changed()
                                            {
                                                state.draft_dirty = true;
                                            }
                                        });
                                    ui.add_space(SPACE_SM);
                                    ui.label(RichText::new("Layout:").size(11.0).color(pal.muted));
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

                                ui.label(RichText::new("Virtual Output:").size(11.0).color(pal.muted));
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
                                                Rounding::same(RADIUS_PILL),
                                                Stroke::new(1.0_f32, pal.accent_line),
                                            );
                                        }
                                        if r.changed() {
                                            state.draft_dirty = true;
                                        }
                                    }
                                    ui.separator();
                                    if snap.driver_status.hidhide_working() {
                                        if draw_crisp_checkbox(
                                            ui,
                                            &mut prof.cloak_physical,
                                            "Cloak",
                                            true,
                                            &pal,
                                        ) {
                                            state.draft_dirty = true;
                                        }
                                    } else {
                                        let mut dummy_cloak = prof.cloak_physical;
                                        let _ = draw_crisp_checkbox(
                                            ui,
                                            &mut dummy_cloak,
                                            "Cloak",
                                            false,
                                            &pal,
                                        );
                                        if draw_outline_button(
                                            ui,
                                            "HidHide needed…",
                                            Vec2::ZERO,
                                            11.5,
                                            RADIUS_CONTROL,
                                            true,
                                            false,
                                            ButtonVariant::SecondaryOutline,
                                            &pal,
                                        )
                                        .on_hover_text(
                                            snap.driver_status.hidhide.state.status_label(),
                                        )
                                        .clicked()
                                        {
                                            state.pending_install_confirm = Some(DriverId::HidHide);
                                        }
                                    }
                                    if draw_crisp_checkbox(
                                        ui,
                                        &mut prof.rumble_enabled,
                                        "Rumble",
                                        true,
                                        &pal,
                                    ) {
                                        state.draft_dirty = true;
                                    }
                                });
                                ui.end_row();

                                ui.label(RichText::new("Options:").size(11.0).color(pal.muted));
                                ui.horizontal(|ui| {
                                    if draw_crisp_checkbox(
                                        ui,
                                        &mut prof.swap_ab_xy,
                                        "Swap A/B & X/Y",
                                        true,
                                        &pal,
                                    ) {
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

                                ui.label(RichText::new("Stick Deadzone:").size(11.0).color(pal.muted));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("L").size(11.0).color(pal.muted));
                                    let mut l_pct = (prof.left_deadzone * 100.0).round();
                                    if ui
                                        .add(egui::Slider::new(&mut l_pct, 0.0..=50.0).suffix("%"))
                                        .changed()
                                    {
                                        prof.left_deadzone = l_pct / 100.0;
                                        state.draft_dirty = true;
                                    }
                                    ui.label(RichText::new("R").size(11.0).color(pal.muted));
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
                                    .size(11.0)
                                    .color(pal.muted),
                            );
                            ui.label(
                                RichText::new(sel_btn.family_label(prof.family))
                                    .font(strong_font(12.5))
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
                                RichText::new("Input Mappings")
                                    .font(strong_font(13.5))
                                    .color(pal.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if draw_outline_button(
                                        ui,
                                        "Reset All",
                                        Vec2::new(0.0, 22.0),
                                        11.0,
                                        RADIUS_CONTROL,
                                        true,
                                        false,
                                        ButtonVariant::SubtleOutline,
                                        &pal,
                                    )
                                    .clicked()
                                    {
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
                                let row_stroke = if is_phys || is_sel {
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
                                                pal.accent_line
                                            } else if prof.button_remap.contains_key(&src) {
                                                pal.accent_source
                                            } else {
                                                pal.border
                                            };
                                            draw_neutral_dot(ui, dot_col);
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

    draw_theme_picker_popover(ctx, state, snap, &pal, &mut send);
    draw_install_confirmation_modal(ctx, state, snap, &pal, &mut send);
    draw_reboot_confirmation_modal(ctx, state, &pal, &mut send);
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

fn short_driver_badge(entry: &DriverEntryStatus) -> &'static str {
    match entry.state {
        DriverState::Working => "OK",
        DriverState::InstalledNotRunning(_) => "Stopped",
        DriverState::NotInstalled => "Missing",
    }
}

fn draw_status_bar_row(
    ui: &mut egui::Ui,
    snap: &EngineSnapshot,
    drivers_expanded: bool,
    pal: &ThemePalette,
) -> bool {
    let mut toggled = false;
    ui.spacing_mut().item_spacing.x = SPACE_SM;
    let ds = &snap.driver_status;
    draw_status_icon(ui, ds.vigem_working());
    ui.label(
        RichText::new(format!("ViGEmBus: {}", short_driver_badge(&ds.vigem)))
            .size(11.0)
            .color(pal.text),
    );
    ui.label(RichText::new("•").size(11.0).color(pal.subtle_text));

    draw_status_icon(ui, ds.hidhide_working());
    ui.label(
        RichText::new(format!("HidHide: {}", short_driver_badge(&ds.hidhide)))
            .size(11.0)
            .color(pal.text),
    );

    if draw_collapse_chevron_button(ui, drivers_expanded, pal) {
        toggled = true;
    }

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let (ws, priv_b) = query_process_memory_bytes();
        let ws_mb = ws as f64 / (1024.0 * 1024.0);
        let priv_mb = priv_b as f64 / (1024.0 * 1024.0);
        ui.label(
            RichText::new(format!("RAM: {ws_mb:.1} / {priv_mb:.1} MB"))
                .font(FontId::new(11.0, FontFamily::Monospace))
                .color(pal.muted),
        );
    });
    toggled
}

fn draw_drivers_section(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    card_inner_w: f32,
    pal: &ThemePalette,
    actions: &mut Vec<UiAction>,
    send: &mut impl FnMut(EngineCommand),
) {
    ui.spacing_mut().item_spacing.y = SPACE_SM;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Drivers").size(13.5).color(pal.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if draw_collapse_chevron_button(ui, true, pal) {
                state.drivers_section_override = Some(false);
            }
        });
    });

    draw_single_driver_card(
        ui,
        state,
        snap,
        &snap.driver_status.vigem,
        card_inner_w,
        pal,
        actions,
        send,
    );
    draw_single_driver_card(
        ui,
        state,
        snap,
        &snap.driver_status.hidhide,
        card_inner_w,
        pal,
        actions,
        send,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_single_driver_card(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    entry: &DriverEntryStatus,
    card_inner_w: f32,
    pal: &ThemePalette,
    actions: &mut Vec<UiAction>,
    send: &mut impl FnMut(EngineCommand),
) {
    let driver = entry.id;
    let working = entry.state.is_working();
    let busy = snap.busy_driver == Some(driver);

    egui::Frame::none()
        .fill(pal.panel)
        .rounding(CARD_ROUNDING)
        .stroke(Stroke::new(1.0_f32, pal.border))
        .inner_margin(Margin::same(SPACE_MD))
        .show(ui, |ui| {
            ui.set_width(card_inner_w);
            ui.spacing_mut().item_spacing = Vec2::new(SPACE_SM, SPACE_XS);

            // Header: status icon + driver name + full-pill requirement chip + publisher + right-aligned Consolas version
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_XS;
                draw_status_icon(ui, working);
                ui.label(
                    RichText::new(driver.display_name())
                        .font(strong_font(13.5))
                        .color(pal.text),
                );
                if driver.is_required() {
                    paint_pill_badge(
                        ui,
                        driver.requirement_badge(),
                        pal.soft_fill,
                        pal.accent_line,
                        pal.pill_ink,
                    );
                } else {
                    paint_pill_badge(
                        ui,
                        driver.requirement_badge(),
                        Color32::TRANSPARENT,
                        pal.border,
                        pal.muted,
                    );
                }
                ui.label(
                    RichText::new(format!("by {}", driver.publisher_short()))
                        .size(11.0)
                        .color(pal.subtle_text),
                );
                if let Some(ver) = &entry.version {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("v{ver}"))
                                .font(FontId::new(11.5, FontFamily::Monospace))
                                .color(pal.muted),
                        );
                    });
                }
            });

            // FIX 3: When a driver reports Working, show ONLY header + Status: Working (no action buttons).
            if working {
                ui.label(
                    RichText::new(format!("Status: {}", entry.state.status_label()))
                        .size(11.5)
                        .color(pal.text),
                );
                return;
            }

            // Plain-language purpose line (shown when driver needs attention)
            ui.add(
                egui::Label::new(
                    RichText::new(driver.purpose_line())
                        .size(11.0)
                        .color(pal.subtle_text),
                )
                .wrap(),
            );

            // Detailed state label
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Status: {}", entry.state.status_label()))
                        .size(11.5)
                        .color(pal.status_red_text),
                )
                .wrap(),
            );

            if let Some(msg) = snap.driver_messages.get(&driver).or(entry.detail_note.as_ref()) {
                ui.add(
                    egui::Label::new(RichText::new(msg).size(11.0).color(pal.muted)).wrap(),
                );
            }

            if let Some(dir) = snap.config.located_dir(driver) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_XS;
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("Located: {dir}"))
                                .size(11.0)
                                .color(pal.muted),
                        )
                        .truncate(),
                    );
                    if draw_outline_button(
                        ui,
                        "Clear",
                        Vec2::new(0.0, 22.0),
                        11.0,
                        RADIUS_CONTROL,
                        true,
                        false,
                        ButtonVariant::SubtleOutline,
                        pal,
                    )
                    .clicked()
                    {
                        send(EngineCommand::ClearDriverLocation(driver));
                    }
                });
            }

            // Action buttons (non-Working states only; remedies come before any Reinstall option)
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(SPACE_SM, SPACE_XS);

                if busy {
                    let _ = draw_outline_button(
                        ui,
                        "In progress…",
                        Vec2::ZERO,
                        11.5,
                        RADIUS_CONTROL,
                        false,
                        false,
                        ButtonVariant::SubtleOutline,
                        pal,
                    );
                } else {
                    match entry.state {
                        DriverState::NotInstalled => {
                            if draw_outline_button(
                                ui,
                                &format!("Install {}…", driver.display_name()),
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::PrimaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_install_confirm = Some(driver);
                            }
                        }
                        DriverState::InstalledNotRunning(_) => {
                            if let Some(rem) = remedy_for_entry(entry) {
                                if draw_outline_button(
                                    ui,
                                    rem.button_label(),
                                    Vec2::ZERO,
                                    11.5,
                                    RADIUS_CONTROL,
                                    true,
                                    false,
                                    ButtonVariant::PrimaryOutline,
                                    pal,
                                )
                                .clicked()
                                {
                                    if rem.requires_reboot_confirm() {
                                        state.pending_reboot_confirm = Some(driver);
                                    } else {
                                        send(EngineCommand::RunDriverRemedy(driver));
                                    }
                                }
                            } else if draw_outline_button(
                                ui,
                                "Re-check Status",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::PrimaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                send(EngineCommand::RefreshStatus);
                            }

                            if draw_outline_button(
                                ui,
                                "Reinstall (Later Option)…",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_install_confirm = Some(driver);
                            }
                        }
                        DriverState::Working => {}
                    }

                    if draw_outline_button(
                        ui,
                        "I already have this — Locate…",
                        Vec2::ZERO,
                        11.5,
                        RADIUS_CONTROL,
                        true,
                        false,
                        ButtonVariant::SecondaryOutline,
                        pal,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::BrowseDriverFolder { driver });
                    }
                }
            });
        });
}

fn draw_install_confirmation_modal(
    ctx: &egui::Context,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    pal: &ThemePalette,
    send: &mut impl FnMut(EngineCommand),
) {
    let Some(driver) = state.pending_install_confirm else {
        return;
    };
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("driver_install_confirm_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            let tint = if snap.config.theme_mode == ThemeMode::Dark {
                Color32::from_rgba_unmultiplied(4, 4, 8, 170)
            } else {
                Color32::from_rgba_unmultiplied(18, 28, 42, 120)
            };
            ui.painter().rect_filled(rect, Rounding::ZERO, tint);
        });

    let modal_w = (screen.width() - SPACE_LG * 2.0).min(368.0).max(260.0);
    egui::Area::new(egui::Id::new("driver_install_confirm_dialog"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(pal.panel)
                .rounding(CARD_ROUNDING)
                .stroke(Stroke::new(1.0_f32, pal.accent_line))
                .inner_margin(Margin::same(SPACE_MD))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.spacing_mut().item_spacing.y = SPACE_SM;

                    let verb = if snap.driver_status.entry(driver).state.is_not_installed() {
                        "Install"
                    } else {
                        "Reinstall"
                    };
                    ui.label(
                        RichText::new(format!("{verb} {}?", driver.display_name()))
                            .font(strong_font(14.5))
                            .color(pal.text),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label(
                            RichText::new(format!("Publisher: {} | Bundled ", driver.publisher_full()))
                                .size(11.0)
                                .color(pal.accent_line),
                        );
                        ui.label(
                            RichText::new(format!("v{}", driver.bundled_version_hint()))
                                .font(FontId::new(11.0, FontFamily::Monospace))
                                .color(pal.accent_line),
                        );
                    });
                    ui.add(
                        egui::Label::new(
                            RichText::new(driver.install_impact())
                                .size(11.5)
                                .color(pal.text),
                        )
                        .wrap(),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                "The official signed installer wizard will now open with no silent flags so you can step through it yourself. Windows may show a UAC prompt.",
                            )
                            .size(11.0)
                            .color(pal.muted),
                        )
                        .wrap(),
                    );

                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            if draw_outline_button(
                                ui,
                                "Continue to Installer",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::PrimaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_install_confirm = None;
                                send(EngineCommand::ConfirmInstallDriver(driver));
                            }

                            if draw_outline_button(
                                ui,
                                "Cancel",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_install_confirm = None;
                                send(EngineCommand::CancelInstallDriver(driver));
                            }
                        });
                    });
                });
        });
}

fn draw_reboot_confirmation_modal(
    ctx: &egui::Context,
    state: &mut AppUiState,
    pal: &ThemePalette,
    send: &mut impl FnMut(EngineCommand),
) {
    let Some(driver) = state.pending_reboot_confirm else {
        return;
    };
    let screen = ctx.screen_rect();
    let modal_w = (screen.width() - SPACE_LG * 2.0).min(352.0).max(260.0);
    egui::Area::new(egui::Id::new("driver_reboot_confirm_dialog"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(pal.panel)
                .rounding(CARD_ROUNDING)
                .stroke(Stroke::new(1.0_f32, pal.status_red_text))
                .inner_margin(Margin::same(SPACE_MD))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.spacing_mut().item_spacing.y = SPACE_SM;
                    ui.label(
                        RichText::new("Restart Windows Now?")
                            .font(strong_font(14.5))
                            .color(pal.text),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "{} has a pending reboot before Windows will load the driver. Save any open work before restarting.",
                                driver.display_name()
                            ))
                            .size(11.0)
                            .color(pal.muted),
                        )
                        .wrap(),
                    );
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            if draw_outline_button(
                                ui,
                                "Restart Windows",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::DangerStatus,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_reboot_confirm = None;
                                send(EngineCommand::ConfirmSystemReboot(driver));
                            }
                            if draw_outline_button(
                                ui,
                                "Cancel",
                                Vec2::ZERO,
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                pal,
                            )
                            .clicked()
                            {
                                state.pending_reboot_confirm = None;
                            }
                        });
                    });
                });
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
        FontId::new(10.5, FontFamily::Proportional),
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
            Color32::from_rgba_unmultiplied(pal.accent_line.r(), pal.accent_line.g(), pal.accent_line.b(), 64),
        );
        p.circle_filled(dot_pos, 3.8, pal.accent_line);
    } else {
        p.circle_filled(dot_pos, 3.0, pal.accent_line);
    }
}

fn draw_mini_trigger_row(ui: &mut egui::Ui, label: &str, frac: f32, pal: &ThemePalette) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
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
            let fill_rect = Rect::from_min_size(inner.min, Vec2::new(fw, inner.height()));
            paint_monochrome_sweep_bar(p, inner, fill_rect, pal.rim_stops);
        }
        ui.label(
            RichText::new(format!("{:.0}%", u * 100.0))
                .font(FontId::new(10.5, FontFamily::Monospace))
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
                Color32::from_rgba_unmultiplied(pal.accent_line.r(), pal.accent_line.g(), pal.accent_line.b(), 80),
            ),
        );
        p.rect_filled(rect, Rounding::same(3.5), pal.solid_fill);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.5_f32, pal.accent_line));
    } else {
        p.rect_filled(rect, Rounding::same(3.5), pal.bg);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.0_f32, pal.border));
    }
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.5, FontFamily::Proportional),
        if pressed { pal.on_solid } else { pal.muted },
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
            driver_status: DriverStatus::all_working_for_tests(),
            driver_messages: BTreeMap::new(),
            busy_driver: None,
            last_status_message: None,
        }
    }

    #[test]
    fn battery_low_threshold_triggers_at_or_below_twenty() {
        assert!(battery_is_low(Some(0)));
        assert!(battery_is_low(Some(20)));
        assert!(!battery_is_low(Some(21)));
        assert!(!battery_is_low(Some(100)));
        assert!(!battery_is_low(None));
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
                        if (rs.rect.width() - 20.0).abs() < 2.5
                            && (rs.rect.height() - 20.0).abs() < 2.5
                            && (rs.rect.center().y - target_y).abs() < 10.0 =>
                    {
                        Some(rs.rect.center())
                    }
                    egui::Shape::Mesh(m) if !m.vertices.is_empty() => {
                        let b = m.calc_bounds();
                        if (b.width() - 20.0).abs() < 2.5
                            && (b.height() - 20.0).abs() < 2.5
                            && (b.center().y - target_y).abs() < 10.0
                        {
                            Some(b.center())
                        } else {
                            None
                        }
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
            collapsed_h < 310.0 && (expanded_h - collapsed_h) > 100.0,
            "collapsed height ({collapsed_h}) should shrink >100px vs expanded ({expanded_h})"
        );
        let edit_pt = find_text_center(&out_col.shapes, "Edit Profile").unwrap();
        assert!(find_text_center(&out_col.shapes, "Drivers").is_none());
        assert!(collapsed_h - (edit_pt.y + 12.0) < 76.0);
    }

    #[test]
    fn healthy_drivers_hide_section_by_default_and_manual_expand_shows_buttonless_cards() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.controllers.truncate(1);
        snap.controllers[0].show_live_input = false;
        let mut ui_state = AppUiState::default();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(412.0, 900.0)));
        let mut t = 0.05_f64;

        let out_hidden = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        assert!(find_text_center(&out_hidden.shapes, "Drivers").is_none());
        assert!(find_text_center(&out_hidden.shapes, "I already have this — Locate…").is_none());
        assert!(find_text_center(&out_hidden.shapes, "Reinstall ViGEmBus…").is_none());
        let hidden_h = ui_state.desired_compact_logical_h.unwrap();

        // Click the small Drivers chevron toggle in the footer bar to manually expand.
        let status_y = find_text_center(&out_hidden.shapes, "ViGEmBus: OK").unwrap().y;
        fn find_chevron_at_y(shapes: &[egui::epaint::ClippedShape], target_y: f32) -> Option<Pos2> {
            fn scan(s: &egui::Shape, target_y: f32) -> Option<Pos2> {
                match s {
                    egui::Shape::Vec(v) => v.iter().rev().find_map(|c| scan(c, target_y)),
                    egui::Shape::Rect(rs)
                        if (rs.rect.width() - 20.0).abs() < 2.5
                            && (rs.rect.height() - 20.0).abs() < 2.5
                            && (rs.rect.center().y - target_y).abs() < 10.0 =>
                    {
                        Some(rs.rect.center())
                    }
                    egui::Shape::Mesh(m) if !m.vertices.is_empty() => {
                        let b = m.calc_bounds();
                        if (b.width() - 20.0).abs() < 2.5
                            && (b.height() - 20.0).abs() < 2.5
                            && (b.center().y - target_y).abs() < 10.0
                        {
                            Some(b.center())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            shapes.iter().rev().find_map(|c| scan(&c.shape, target_y))
        }
        let footer_toggle = find_chevron_at_y(&out_hidden.shapes, status_y)
            .expect("expected 20x20 Drivers toggle chevron in status bar");
        for pressed in [true, false] {
            t += 0.05;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(footer_toggle),
                        egui::Event::PointerButton {
                            pos: footer_toggle,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
                },
            );
        }
        assert_eq!(ui_state.drivers_section_override, Some(true));

        t += 0.05;
        let out_exp = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        let expanded_h = ui_state.desired_compact_logical_h.unwrap();
        assert!(expanded_h > hidden_h + 120.0);
        assert!(find_text_center(&out_exp.shapes, "Drivers").is_some());
        assert!(find_text_center(&out_exp.shapes, "ViGEmBus").is_some());
        assert!(find_text_center(&out_exp.shapes, "HidHide").is_some());
        assert!(find_text_center(&out_exp.shapes, "Status: Working").is_some());
        // FIX 3: working cards must show NO Locate or Reinstall buttons.
        assert!(find_text_center(&out_exp.shapes, "I already have this — Locate…").is_none());
        assert!(find_text_center(&out_exp.shapes, "Reinstall ViGEmBus…").is_none());
        assert!(find_text_center(&out_exp.shapes, "Reinstall HidHide…").is_none());

        // Auto-appears expanded when any driver becomes non-Working even if previously hidden.
        ui_state.drivers_section_override = Some(false);
        snap.driver_status.vigem = DriverEntryStatus::not_installed(DriverId::ViGEmBus);
        t += 0.05;
        let out_unhealthy = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );
        assert!(find_text_center(&out_unhealthy.shapes, "Install ViGEmBus…").is_some());
        assert!(find_text_center(&out_unhealthy.shapes, "I already have this — Locate…").is_some());
    }

    #[test]
    fn header_and_card_buttons_change_visuals_on_hover_and_press_in_both_themes() {
        fn button_visual_signature(
            shapes: &[egui::epaint::ClippedShape],
            around: Pos2,
        ) -> (Color32, Color32) {
            fn scan(
                s: &egui::Shape,
                around: Pos2,
                fill_out: &mut Color32,
                stroke_out: &mut Color32,
            ) {
                match s {
                    egui::Shape::Vec(v) => {
                        for c in v {
                            scan(c, around, fill_out, stroke_out);
                        }
                    }
                    egui::Shape::Rect(rs)
                        if rs.rect.contains(around)
                            && rs.rect.width() >= 40.0
                            && rs.rect.height() >= 20.0
                            && rs.rect.height() <= 36.0 =>
                    {
                        *fill_out = rs.fill;
                    }
                    egui::Shape::Mesh(m) if !m.vertices.is_empty() && m.texture_id == egui::TextureId::Managed(0) => {
                        let b = m.calc_bounds();
                        if b.contains(around)
                            && b.width() >= 40.0
                            && b.height() >= 20.0
                            && b.height() <= 36.0
                        {
                            *stroke_out = m.vertices[0].color;
                        }
                    }
                    _ => {}
                }
            }
            let mut fill = Color32::TRANSPARENT;
            let mut stroke = Color32::TRANSPARENT;
            for c in shapes {
                scan(&c.shape, around, &mut fill, &mut stroke);
            }
            (fill, stroke)
        }

        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let ctx = egui::Context::default();
            configure_neon_theme_and_fonts(&ctx);
            let mut snap = sample_snapshot();
            snap.config.theme_mode = mode;
            let mut ui_state = AppUiState::default();
            let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(412.0, 601.0)));

            let out_idle = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05),
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
                },
            );
            let refresh_pt = find_text_center(&out_idle.shapes, "Refresh").unwrap();
            let (idle_fill, idle_stroke) = button_visual_signature(&out_idle.shapes, refresh_pt);
            assert_eq!(idle_fill, Color32::TRANSPARENT, "SecondaryOutline rests transparent");
            assert_ne!(idle_stroke, Color32::TRANSPARENT, "Refresh must draw crisp 1px ring at rest");

            let out_hover = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.10),
                    events: vec![egui::Event::PointerMoved(refresh_pt)],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
                },
            );
            let (hover_fill, hover_stroke) = button_visual_signature(&out_hover.shapes, refresh_pt);
            assert_ne!(hover_fill, idle_fill, "hover fill must differ in {mode:?}");
            assert_ne!(hover_stroke, idle_stroke, "hover stroke must differ in {mode:?}");

            let out_press = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.15),
                    events: vec![egui::Event::PointerButton {
                        pos: refresh_pt,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    focused: true,
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
                },
            );
            let (press_fill, press_stroke) = button_visual_signature(&out_press.shapes, refresh_pt);
            let pal = ThemePalette::for_theme(mode, snap.config.theme_accent);
            assert_ne!(press_fill, hover_fill, "press fill must differ in {mode:?}");
            assert_eq!(press_fill, pal.solid_fill, "pressed fill must be monochrome solid_fill in {mode:?}");
            assert_eq!(press_stroke, pal.accent_line, "pressed stroke must stay within theme hue in {mode:?}");
        }
    }

    #[test]
    fn theme_picker_preset_emits_set_theme_accent() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let snap = sample_snapshot();
        let mut ui_state = AppUiState {
            theme_picker_open: true,
            ..Default::default()
        };
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(412.0, 620.0)));
        let mut emitted = Vec::new();
        // Anchored egui::Area performs a hidden sizing pass on its first frame.
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.05),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| emitted.push(cmd));
            },
        );
        let out = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(0.10),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| emitted.push(cmd));
            },
        );
        let pink_pt = find_text_center(&out.shapes, "Pink").expect("expected Pink preset chip in open Theme Picker");
        for (i, pressed) in [(1, true), (2, false)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05 * (i + 1) as f64),
                    events: vec![
                        egui::Event::PointerMoved(pink_pt),
                        egui::Event::PointerButton {
                            pos: pink_pt,
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
                EngineCommand::SetThemeAccent(ThemeAccent::Preset { preset: ThemePreset::Pink })
            )),
            "expected SetThemeAccent(Pink) when clicking Pink chip, got {emitted:?}"
        );
    }

    #[test]
    fn masthead_lockup_and_design_md_type_floor_hold_in_both_themes() {
        fn collect_min_font_size(shapes: &[egui::epaint::ClippedShape]) -> f32 {
            fn scan(s: &egui::Shape, min_pt: &mut f32) {
                match s {
                    egui::Shape::Vec(v) => {
                        for c in v {
                            scan(c, min_pt);
                        }
                    }
                    egui::Shape::Text(ts) => {
                        for sec in &ts.galley.job.sections {
                            *min_pt = min_pt.min(sec.format.font_id.size);
                        }
                    }
                    _ => {}
                }
            }
            let mut min_pt = 999.0_f32;
            for c in shapes {
                scan(&c.shape, &mut min_pt);
            }
            min_pt
        }

        fn find_logo_quad(shapes: &[egui::epaint::ClippedShape]) -> Option<Rect> {
            fn scan(s: &egui::Shape) -> Option<Rect> {
                match s {
                    egui::Shape::Vec(v) => v.iter().find_map(scan),
                    egui::Shape::Mesh(m) if !matches!(m.texture_id, egui::TextureId::Managed(0)) => {
                        Some(m.calc_bounds())
                    }
                    _ => None,
                }
            }
            shapes.iter().find_map(|c| scan(&c.shape))
        }

        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let ctx = egui::Context::default();
            configure_neon_theme_and_fonts(&ctx);
            let mut snap = sample_snapshot();
            snap.config.theme_mode = mode;
            let mut ui_state = AppUiState {
                drivers_section_override: Some(true),
                last_all_drivers_working: Some(true),
                ..Default::default()
            };
            let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(412.0, 900.0)));
            let out = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(0.05),
                    ..Default::default()
                },
                |c| {
                    let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
                },
            );

            let logo_rect = find_logo_quad(&out.shapes).expect("expected 24px masthead WT logo quad");
            assert!((logo_rect.width() - 24.0).abs() <= 1.0);
            assert!((logo_rect.height() - 24.0).abs() <= 1.0);

            let title_pos = find_text_center(&out.shapes, "z-controller").unwrap();
            let caption_pos = find_text_center(&out.shapes, MASTHEAD_CAPTION).unwrap();
            let controllers_pos = find_text_center(&out.shapes, "Controllers").unwrap();
            assert!(logo_rect.right() < title_pos.x);
            assert!(title_pos.x < caption_pos.x);
            assert!((title_pos.y - caption_pos.y).abs() < 4.0);
            assert!(controllers_pos.y > title_pos.y + 12.0);

            let min_pt = collect_min_font_size(&out.shapes);
            assert!(
                min_pt >= 10.5,
                "DESIGN.md §3 / FC7: found rendered font smaller than 10.5pt: {min_pt}"
            );
        }
    }

    #[test]
    fn driver_cards_render_separately_with_no_combined_install_button() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.driver_status.vigem = DriverEntryStatus::not_installed(DriverId::ViGEmBus);
        snap.driver_status.hidhide = DriverEntryStatus::not_installed(DriverId::HidHide);
        let mut ui_state = AppUiState::default();

        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0))),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |_| {});
            },
        );

        assert!(find_text_center(&out.shapes, "Install ViGEmBus…").is_some());
        assert!(find_text_center(&out.shapes, "Install HidHide…").is_some());
        assert!(find_text_center(&out.shapes, "Install Missing Drivers").is_none());
        assert!(find_text_center(&out.shapes, "REQUIRED to create virtual Xbox 360 / DualShock 4 pads.").is_some());
        assert!(find_text_center(&out.shapes, "OPTIONAL, only needed to hide (cloak) the physical pad from games.").is_some());
    }

    #[test]
    fn install_confirmation_dialog_names_driver_publisher_uac_and_cancel_logs_without_installing() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.driver_status.vigem = DriverEntryStatus::not_installed(DriverId::ViGEmBus);
        let mut ui_state = AppUiState::default();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0)));
        let mut emitted = Vec::new();
        let mut t = 0.05_f64;

        let out_idle = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| emitted.push(cmd));
            },
        );
        let inst_btn = find_text_center(&out_idle.shapes, "Install ViGEmBus…").unwrap();
        for pressed in [true, false] {
            t += 0.05;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(inst_btn),
                        egui::Event::PointerButton {
                            pos: inst_btn,
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
                        emitted.push(cmd)
                    });
                },
            );
        }
        // Clicking Install opens confirmation modal ONLY — never emits ConfirmInstallDriver yet.
        assert_eq!(ui_state.pending_install_confirm, Some(DriverId::ViGEmBus));
        assert!(emitted.is_empty());

        t += 0.05;
        let out_modal = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| emitted.push(cmd));
            },
        );
        assert!(find_text_center(&out_modal.shapes, "Install ViGEmBus?").is_some());
        let cancel_pt = find_text_center(&out_modal.shapes, "Cancel").unwrap();
        for pressed in [true, false] {
            t += 0.05;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(cancel_pt),
                        egui::Event::PointerButton {
                            pos: cancel_pt,
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
                        emitted.push(cmd)
                    });
                },
            );
        }
        assert_eq!(ui_state.pending_install_confirm, None);
        assert!(emitted.contains(&EngineCommand::CancelInstallDriver(DriverId::ViGEmBus)));
        assert!(!emitted.contains(&EngineCommand::ConfirmInstallDriver(DriverId::ViGEmBus)));
    }

    #[test]
    fn missing_vigem_toggle_explains_and_offers_install_without_auto_installing() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let mut snap = sample_snapshot();
        snap.controllers.truncate(1);
        snap.controllers[0].enabled = false;
        snap.controllers[0].virtual_active = false;
        snap.driver_status.vigem = DriverEntryStatus::not_installed(DriverId::ViGEmBus);

        let mut ui_state = AppUiState::default();
        let screen = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0)));
        let mut emitted = Vec::new();
        let mut t = 0.05_f64;

        let out_idle = ctx.run(
            egui::RawInput {
                screen_rect: screen,
                time: Some(t),
                ..Default::default()
            },
            |c| {
                let _ = draw_compact_main_snapshot(c, &mut ui_state, &snap, |cmd| emitted.push(cmd));
            },
        );
        let active_pt = find_text_center(&out_idle.shapes, "Active").unwrap();
        for pressed in [true, false] {
            t += 0.05;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: screen,
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(active_pt),
                        egui::Event::PointerButton {
                            pos: active_pt,
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
                        emitted.push(cmd)
                    });
                },
            );
        }
        assert!(emitted.is_empty());
        assert_eq!(
            ui_state.missing_vigem_notice_controller.as_deref(),
            Some("057e:2009:NSP-1")
        );
    }

    #[test]
    fn remedy_buttons_match_stopped_disabled_device_error_and_reboot_pending() {
        use z_core::driver::NotRunningReason;
        let stopped = DriverEntryStatus {
            id: DriverId::ViGEmBus,
            state: DriverState::InstalledNotRunning(NotRunningReason::ServiceStopped),
            version: None,
            pnp_instance_id: None,
            detail_note: None,
        };
        assert_eq!(
            remedy_for_entry(&stopped),
            Some(z_platform_windows::drivers::RemedyKind::StartService)
        );

        let disabled = DriverEntryStatus {
            id: DriverId::HidHide,
            state: DriverState::InstalledNotRunning(NotRunningReason::ServiceDisabled),
            version: None,
            pnp_instance_id: None,
            detail_note: None,
        };
        assert_eq!(
            remedy_for_entry(&disabled),
            Some(z_platform_windows::drivers::RemedyKind::EnableAndStartService)
        );

        let dev_err = DriverEntryStatus {
            id: DriverId::ViGEmBus,
            state: DriverState::InstalledNotRunning(NotRunningReason::DeviceError {
                problem_code: 22,
            }),
            version: None,
            pnp_instance_id: Some("ROOT\\SYSTEM\\0003".to_string()),
            detail_note: None,
        };
        assert_eq!(
            remedy_for_entry(&dev_err),
            Some(z_platform_windows::drivers::RemedyKind::ReenableDevice {
                instance_id: "ROOT\\SYSTEM\\0003".to_string()
            })
        );

        let reboot = DriverEntryStatus {
            id: DriverId::HidHide,
            state: DriverState::InstalledNotRunning(NotRunningReason::RebootPending),
            version: None,
            pnp_instance_id: None,
            detail_note: None,
        };
        assert_eq!(
            remedy_for_entry(&reboot),
            Some(z_platform_windows::drivers::RemedyKind::RestartMachine)
        );
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
        let toggle_center = Pos2::new(refresh_center.x - 52.0, refresh_center.y);

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
