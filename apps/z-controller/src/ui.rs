use std::collections::BTreeMap;
use std::f32::consts::PI;

use egui::epaint::{Mesh, Shadow, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Rect, RichText,
    Rounding, Stroke, TextStyle, Vec2,
};
use z_core::battery::BatteryStatus;
use z_core::model::{ControllerFamily, DisconnectPolicy, LogicalButton, VirtualTargetKind};
use z_core::profile::Profile;
use z_platform_windows::drivers::query_process_memory_bytes;

use crate::diagram::draw_controller_diagram;
use crate::engine::{
    EngineCommand, EngineHandle, EngineSnapshot, PhysicalSnapshot,
};

// Modern neon-on-black palette (neon strictly for accents, edges, bars, and dots — never large fills or body text).
pub const BG_NEAR_BLACK: Color32 = Color32::from_rgb(10, 10, 16); // #0A0A10
pub const PANEL_SURFACE: Color32 = Color32::from_rgb(20, 20, 31); // #14141F
pub const WIDGET_SURFACE: Color32 = Color32::from_rgb(24, 24, 38);
pub const WIDGET_HOVER: Color32 = Color32::from_rgb(31, 31, 49);
pub const WIDGET_ACTIVE: Color32 = Color32::from_rgb(34, 26, 52);
pub const SUBTLE_BORDER: Color32 = Color32::from_rgb(38, 38, 58); // #26263A

pub const NEON_GREEN: Color32 = Color32::from_rgb(61, 255, 122); // #3DFF7A
pub const NEON_VIOLET: Color32 = Color32::from_rgb(169, 112, 255); // #A970FF
pub const NEON_PINK: Color32 = Color32::from_rgb(255, 92, 168); // #FF5CA8

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(237, 237, 242); // #EDEDF2
pub const TEXT_MUTED: Color32 = Color32::from_rgb(154, 154, 176); // #9A9AB0

// Consistent 4 / 8 / 12 / 16 spacing scale applied uniformly across all panels, cards, and grids.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
#[allow(dead_code)]
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
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            selected_controller: None,
            editing_profile_id: None,
            draft_profile: None,
            draft_dirty: false,
            selected_diagram_button: Some(LogicalButton::South),
        }
    }
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

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = BG_NEAR_BLACK;
    visuals.window_fill = PANEL_SURFACE;
    visuals.extreme_bg_color = BG_NEAR_BLACK;
    visuals.faint_bg_color = Color32::from_rgb(16, 16, 26);
    visuals.code_bg_color = BG_NEAR_BLACK;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_rounding = Rounding::same(8.0);
    visuals.menu_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0_f32, SUBTLE_BORDER);
    visuals.window_shadow = Shadow::NONE;
    visuals.popup_shadow = Shadow::NONE;

    visuals.widgets.noninteractive.bg_fill = PANEL_SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL_SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, SUBTLE_BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_MUTED);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = WIDGET_SURFACE;
    visuals.widgets.inactive.weak_bg_fill = WIDGET_SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, SUBTLE_BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = WIDGET_HOVER;
    visuals.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, NEON_VIOLET);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = WIDGET_ACTIVE;
    visuals.widgets.active.weak_bg_fill = WIDGET_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, NEON_PINK);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    visuals.widgets.open.bg_fill = WIDGET_HOVER;
    visuals.widgets.open.weak_bg_fill = WIDGET_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, NEON_VIOLET);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.open.rounding = Rounding::same(6.0);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(169, 112, 255, 42);
    visuals.selection.stroke = Stroke::new(1.0_f32, NEON_VIOLET);

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(SPACE_MD, SPACE_XS + 1.0);
    ctx.set_style(style);
}

#[inline]
pub fn battery_bar_color(percent: Option<u8>) -> Color32 {
    match percent {
        Some(p) if p <= 20 => NEON_PINK,
        _ => NEON_GREEN,
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
        let c = lerp_color(c_start, c_end, t);
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
    // Static layered translucent outer glow (strictly outside card edge) + 1.5px violet-to-pink edge.
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
            NEON_VIOLET,
            NEON_PINK,
            alpha,
        );
    }
    paint_gradient_rounded_rect_stroke(
        painter,
        rect,
        rounding,
        1.5_f32,
        NEON_VIOLET,
        NEON_PINK,
        245,
    );
}

fn draw_aligned_divider(ui: &mut egui::Ui, left: f32, right: f32) {
    let y = ui.cursor().top();
    ui.painter()
        .line_segment([Pos2::new(left, y), Pos2::new(right, y)], Stroke::new(1.0_f32, SUBTLE_BORDER));
    ui.add_space(SPACE_SM);
}

fn draw_status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

fn draw_battery_bar(ui: &mut egui::Ui, battery: &z_core::battery::BatteryReading) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        ui.label(RichText::new("Battery:").small().color(TEXT_MUTED));
        let bar_size = Vec2::new(124.0, 8.0);
        let (rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(4.0), BG_NEAR_BLACK);
        p.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, SUBTLE_BORDER));

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

        ui.label(RichText::new(battery.format_badge()).small().color(TEXT_PRIMARY));
    });
}

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

    // Bottom status bar (driver readiness, HidHide lock warnings, live RAM readout — NO popups)
    egui::TopBottomPanel::bottom("compact_status_bar")
        .exact_height(36.0)
        .frame(
            egui::Frame::none()
                .fill(PANEL_SURFACE)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            let sr = ctx.screen_rect();
            let top_y = ui.max_rect().top() - SPACE_SM;
            ui.painter().line_segment(
                [Pos2::new(sr.left(), top_y), Pos2::new(sr.right(), top_y)],
                Stroke::new(1.0_f32, SUBTLE_BORDER),
            );
            draw_status_bar(ui, snap, &mut send);
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(BG_NEAR_BLACK)
                .inner_margin(Margin::same(SPACE_MD)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Controllers").color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Refresh").clicked() {
                        send(EngineCommand::RefreshStatus);
                    }
                });
            });
            ui.add_space(SPACE_XS);
            let bounds = ui.available_rect_before_wrap();
            draw_aligned_divider(ui, bounds.left(), bounds.right());

            if snap.controllers.is_empty() {
                egui::Frame::none()
                    .fill(PANEL_SURFACE)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, SUBTLE_BORDER))
                    .inner_margin(Margin::same(SPACE_MD))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("No physical controllers detected.").color(TEXT_PRIMARY),
                        );
                        ui.add_space(SPACE_XS);
                        ui.label(
                            RichText::new("Connect a controller via Bluetooth or USB.")
                                .small()
                                .color(TEXT_MUTED),
                        );
                    });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Inner 4px/8px inset keeps active cards' 4px static outer glow unclipped while aligning card edges.
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(SPACE_XS + 2.0, SPACE_XS + 2.0))
                    .show(ui, |ui| {
                        let card_inner_w = (ui.available_width() - SPACE_MD * 2.0).max(280.0);
                        for ctrl in &snap.controllers {
                            let is_selected =
                                state.selected_controller.as_deref() == Some(&ctrl.canonical_id);
                            let card_fill = if is_selected {
                                WIDGET_SURFACE
                            } else {
                                PANEL_SURFACE
                            };
                            let card_stroke = if ctrl.virtual_active {
                                Stroke::NONE
                            } else {
                                Stroke::new(
                                    1.0_f32,
                                    if is_selected { NEON_VIOLET } else { SUBTLE_BORDER },
                                )
                            };

                            let card_resp = egui::Frame::none()
                                .fill(card_fill)
                                .rounding(CARD_ROUNDING)
                                .inner_margin(Margin::same(SPACE_MD))
                                .stroke(card_stroke)
                                .show(ui, |ui| {
                                    ui.set_min_width(card_inner_w);
                                    ui.spacing_mut().item_spacing.y = SPACE_XS + 2.0;

                                    let header_resp = ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        let dot =
                                            if ctrl.connected { NEON_GREEN } else { TEXT_MUTED };
                                        draw_status_dot(ui, dot);
                                        ui.label(
                                            RichText::new(&ctrl.name).strong().color(TEXT_PRIMARY),
                                        );
                                    });
                                    if ui
                                        .interact(
                                            header_resp.response.rect,
                                            ui.id().with(&ctrl.canonical_id),
                                            egui::Sense::click(),
                                        )
                                        .clicked()
                                    {
                                        state.selected_controller =
                                            Some(ctrl.canonical_id.clone());
                                    }

                                    ui.label(
                                        RichText::new(format!(
                                            "{} | {} | {:04X}:{:04X}",
                                            ctrl.family.label(),
                                            ctrl.transport.label(),
                                            ctrl.key.vid,
                                            ctrl.key.pid
                                        ))
                                        .small()
                                        .color(TEXT_MUTED),
                                    );

                                    draw_battery_bar(ui, &ctrl.battery);

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
                                                .fill(Color32::from_rgb(24, 20, 40))
                                                .rounding(Rounding::same(4.0))
                                                .stroke(Stroke::new(1.0_f32, NEON_VIOLET))
                                                .inner_margin(Margin::symmetric(SPACE_SM, 2.0))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "-> {}",
                                                            ctrl.target_kind.label()
                                                        ))
                                                        .small()
                                                        .color(TEXT_PRIMARY),
                                                    );
                                                });
                                        }
                                        let cloak_dot =
                                            if ctrl.cloaked { NEON_GREEN } else { TEXT_MUTED };
                                        draw_status_dot(ui, cloak_dot);
                                        ui.label(
                                            RichText::new(if ctrl.cloaked {
                                                "Cloaked"
                                            } else {
                                                "Visible"
                                            })
                                            .small()
                                            .color(if ctrl.cloaked {
                                                TEXT_PRIMARY
                                            } else {
                                                TEXT_MUTED
                                            }),
                                        );
                                    });

                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                                        ui.label(
                                            RichText::new("Profile:").small().color(TEXT_MUTED),
                                        );
                                        egui::ComboBox::from_id_salt((
                                            "ctrl_prof",
                                            &ctrl.canonical_id,
                                        ))
                                        .width(172.0)
                                        .selected_text(
                                            RichText::new(&ctrl.profile_name).color(TEXT_PRIMARY),
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
                                            RichText::new("Edit Profile").small().color(TEXT_PRIMARY),
                                        )
                                        .fill(Color32::from_rgb(27, 22, 42))
                                        .stroke(Stroke::new(1.0_f32, NEON_VIOLET))
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

                                    draw_card_live_status(
                                        ui,
                                        ctrl.family,
                                        ctrl.present_buttons_mask,
                                        &ctrl.physical_state,
                                        card_inner_w,
                                    );
                                });

                            if ctrl.virtual_active {
                                paint_active_card_neon_edge_and_glow(
                                    ui.painter(),
                                    card_resp.response.rect,
                                    CARD_ROUNDING,
                                );
                            }

                            ui.add_space(SPACE_MD);
                        }
                    });
            });
        });

    actions
}

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
                .fill(PANEL_SURFACE)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                ui.label(RichText::new("Editing Profile:").color(TEXT_MUTED));
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
                    .selected_text(RichText::new(cur_name).color(TEXT_PRIMARY))
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
                        let accent = if state.draft_dirty { NEON_PINK } else { NEON_VIOLET };
                        let save_btn =
                            egui::Button::new(RichText::new(btn_text).strong().color(TEXT_PRIMARY))
                                .fill(Color32::from_rgb(27, 22, 42))
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
                Stroke::new(1.0_f32, SUBTLE_BORDER),
            );
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(BG_NEAR_BLACK)
                .inner_margin(Margin::same(SPACE_MD)),
        )
        .show(ctx, |ui| {
            let Some(prof) = state.draft_profile.as_mut() else {
                ui.label(RichText::new("Select a profile to edit.").color(TEXT_MUTED));
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
                    .fill(PANEL_SURFACE)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, SUBTLE_BORDER))
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
                                ui.label(RichText::new("Profile Name:").small().color(TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(egui::TextEdit::singleline(&mut prof.name).desired_width(210.0))
                                        .changed()
                                    {
                                        state.draft_dirty = true;
                                    }
                                    ui.add_space(SPACE_SM);
                                    ui.label(RichText::new("Layout:").small().color(TEXT_MUTED));
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

                                ui.label(RichText::new("Virtual Output:").small().color(TEXT_MUTED));
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
                                                Stroke::new(1.0_f32, NEON_VIOLET),
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

                                ui.label(RichText::new("Options:").small().color(TEXT_MUTED));
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

                                ui.label(RichText::new("Stick Deadzone:").small().color(TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("L").small().color(TEXT_MUTED));
                                    let mut l_pct = (prof.left_deadzone * 100.0).round();
                                    if ui
                                        .add(egui::Slider::new(&mut l_pct, 0.0..=50.0).suffix("%"))
                                        .changed()
                                    {
                                        prof.left_deadzone = l_pct / 100.0;
                                        state.draft_dirty = true;
                                    }
                                    ui.label(RichText::new("R").small().color(TEXT_MUTED));
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
                        draw_controller_diagram(
                            ui,
                            prof.family,
                            prof,
                            phys_ref,
                            &mut state.selected_diagram_button,
                        );

                        // Selected Input quick-remap bar synced directly with diagram click
                        let sel_btn = state
                            .selected_diagram_button
                            .unwrap_or(LogicalButton::South);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            ui.label(
                                RichText::new("Selected Input:")
                                    .small()
                                    .color(TEXT_MUTED),
                            );
                            ui.label(
                                RichText::new(sel_btn.family_label(prof.family))
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(RichText::new("->").color(TEXT_MUTED));
                            draw_remap_combobox(
                                ui,
                                prof,
                                sel_btn,
                                "selected_diagram_remap",
                                180.0,
                                &mut state.draft_dirty,
                            );
                        });
                        });
                    });

                // Right Column: Synchronized Text Mapping List (exact same mapping data in text form)
                egui::Frame::none()
                    .fill(PANEL_SURFACE)
                    .rounding(CARD_ROUNDING)
                    .stroke(Stroke::new(1.0_f32, SUBTLE_BORDER))
                    .inner_margin(Margin::same(SPACE_MD))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                        ui.set_width(right_w - SPACE_MD * 2.0);
                        ui.set_min_height((total_h - SPACE_MD * 2.0).max(400.0));
                        ui.spacing_mut().item_spacing.y = SPACE_SM;

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Input Mappings").strong().color(TEXT_PRIMARY),
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
                        draw_aligned_divider(ui, div.left(), div.right());

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
                                    Color32::from_rgb(28, 23, 46)
                                } else {
                                    BG_NEAR_BLACK
                                };
                                let row_stroke = if is_phys {
                                    Stroke::new(1.0_f32, NEON_PINK)
                                } else if is_sel {
                                    Stroke::new(1.0_f32, NEON_VIOLET)
                                } else {
                                    Stroke::new(1.0_f32, SUBTLE_BORDER)
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
                                                NEON_GREEN
                                            } else if prof.button_remap.contains_key(&src) {
                                                NEON_VIOLET
                                            } else {
                                                SUBTLE_BORDER
                                            };
                                            draw_status_dot(ui, dot_col);
                                            let lbl_resp = ui.add(
                                                egui::Label::new(
                                                    RichText::new(src.family_label(prof.family))
                                                        .small()
                                                        .color(if is_sel || is_phys {
                                                            TEXT_PRIMARY
                                                        } else {
                                                            TEXT_MUTED
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
        .selected_text(RichText::new(label).small().color(TEXT_PRIMARY))
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

fn draw_status_bar(
    ui: &mut egui::Ui,
    snap: &EngineSnapshot,
    send: &mut impl FnMut(EngineCommand),
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        let ds = &snap.driver_status;
        draw_status_dot(ui, if ds.vigem_installed { NEON_GREEN } else { NEON_PINK });
        ui.label(
            RichText::new(if ds.vigem_installed {
                "ViGEmBus: OK"
            } else {
                "ViGEmBus: Missing"
            })
            .small()
            .color(TEXT_PRIMARY),
        );
        ui.separator();

        let (hh_text, hh_dot) = if !ds.hidhide_installed {
            ("HidHide: Missing", NEON_PINK)
        } else if ds.hidhide_accessible {
            ("HidHide: Ready", NEON_GREEN)
        } else {
            ("HidHide: Locked", NEON_PINK)
        };
        draw_status_dot(ui, hh_dot);
        ui.label(RichText::new(hh_text).small().color(TEXT_PRIMARY));

        if !ds.vigem_installed || !ds.hidhide_installed {
            if ui.button("Install Missing Drivers").clicked() {
                send(EngineCommand::InstallDrivers);
            }
        }
        if ds.reboot_recommended {
            ui.separator();
            ui.label(
                RichText::new("Reboot recommended")
                    .small()
                    .color(TEXT_MUTED),
            );
        } else if let Some(err) = &ds.hidhide_error {
            ui.separator();
            ui.label(RichText::new(err).small().color(TEXT_MUTED));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (ws, priv_b) = query_process_memory_bytes();
            let ws_mb = ws as f64 / (1024.0 * 1024.0);
            let priv_mb = priv_b as f64 / (1024.0 * 1024.0);
            ui.label(
                RichText::new(format!("RAM: {ws_mb:.1} / {priv_mb:.1} MB"))
                    .small()
                    .color(TEXT_MUTED),
            );
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
) {
    ui.add_space(SPACE_XS);
    let div_left = ui.cursor().left();
    draw_aligned_divider(ui, div_left, div_left + card_inner_w);

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
        );
        draw_mini_stick_pad(
            ui,
            "R",
            normalize_stick_axis(phys.right_x),
            normalize_stick_axis(phys.right_y),
        );
        ui.vertical(|ui| {
            ui.add_space(SPACE_XS);
            draw_mini_trigger_row(ui, lt_lbl, normalize_trigger_axis(phys.left_trigger));
            ui.add_space(SPACE_XS);
            draw_mini_trigger_row(ui, rt_lbl, normalize_trigger_axis(phys.right_trigger));
        });
    });

    ui.add_space(SPACE_XS);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS;
        for btn in CARD_STANDARD_ROW1 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, btn.family_chip(family), pressed, 32.0);
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS;
        for btn in CARD_STANDARD_ROW2 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, btn.family_chip(family), pressed, 32.0);
        }
    });

    let extras = extra_buttons_for_device(family, present_mask | phys.buttons_mask);
    if !extras.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(SPACE_XS);
            for btn in extras {
                let pressed = (phys.buttons_mask & btn.bit()) != 0;
                draw_mini_button_pill(ui, btn.family_chip(family), pressed, 36.0);
            }
        });
    }
}

fn draw_mini_stick_pad(ui: &mut egui::Ui, tag: &str, x: f32, y: f32) {
    let size = 44.0_f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter();
    let center = rect.center();
    let radius = 19.0_f32;

    p.circle_filled(center, radius, BG_NEAR_BLACK);
    p.circle_stroke(center, radius, Stroke::new(1.0_f32, SUBTLE_BORDER));
    p.line_segment(
        [
            Pos2::new(center.x - radius + 2.0, center.y),
            Pos2::new(center.x + radius - 2.0, center.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 44)),
    );
    p.line_segment(
        [
            Pos2::new(center.x, center.y - radius + 2.0),
            Pos2::new(center.x, center.y + radius - 2.0),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 44)),
    );
    p.text(
        rect.min + Vec2::new(2.0, 0.0),
        Align2::LEFT_TOP,
        tag,
        FontId::new(9.5, FontFamily::Proportional),
        TEXT_MUTED,
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
            Color32::from_rgba_unmultiplied(NEON_PINK.r(), NEON_PINK.g(), NEON_PINK.b(), 64),
        );
        p.circle_filled(dot_pos, 3.8, NEON_PINK);
    } else {
        p.circle_filled(dot_pos, 3.0, NEON_VIOLET);
    }
}

fn draw_mini_trigger_row(ui: &mut egui::Ui, label: &str, frac: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_XS + 2.0;
        ui.label(
            RichText::new(label)
                .font(FontId::new(10.5, FontFamily::Proportional))
                .color(TEXT_MUTED),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::new(112.0, 7.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(3.5), BG_NEAR_BLACK);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.0_f32, SUBTLE_BORDER));
        let u = frac.clamp(0.0, 1.0);
        if u > 0.01 {
            let inner = rect.shrink(1.0);
            let fw = (inner.width() * u).max(3.0).min(inner.width());
            p.rect_filled(
                Rect::from_min_size(inner.min, Vec2::new(fw, inner.height())),
                Rounding::same(2.5),
                NEON_VIOLET,
            );
        }
        ui.label(
            RichText::new(format!("{:.0}%", u * 100.0))
                .font(FontId::new(10.5, FontFamily::Proportional))
                .color(if u > 0.01 { TEXT_PRIMARY } else { TEXT_MUTED }),
        );
    });
}

fn draw_mini_button_pill(ui: &mut egui::Ui, label: &str, pressed: bool, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), egui::Sense::hover());
    let p = ui.painter();
    if pressed {
        p.rect_stroke(
            rect.expand(1.2),
            Rounding::same(4.5),
            Stroke::new(
                1.5_f32,
                Color32::from_rgba_unmultiplied(NEON_PINK.r(), NEON_PINK.g(), NEON_PINK.b(), 80),
            ),
        );
        p.rect_filled(rect, Rounding::same(3.5), Color32::from_rgb(38, 24, 58));
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.5_f32, NEON_VIOLET));
    } else {
        p.rect_filled(rect, Rounding::same(3.5), BG_NEAR_BLACK);
        p.rect_stroke(rect, Rounding::same(3.5), Stroke::new(1.0_f32, SUBTLE_BORDER));
    }
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.0, FontFamily::Proportional),
        if pressed { TEXT_PRIMARY } else { TEXT_MUTED },
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
    fn battery_bar_uses_pink_at_or_below_twenty_and_green_above() {
        assert_eq!(battery_bar_color(Some(0)), NEON_PINK);
        assert_eq!(battery_bar_color(Some(20)), NEON_PINK);
        assert_eq!(battery_bar_color(Some(21)), NEON_GREEN);
        assert_eq!(battery_bar_color(Some(100)), NEON_GREEN);
        assert_eq!(battery_bar_color(None), NEON_GREEN);
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
}
