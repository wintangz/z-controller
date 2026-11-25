use std::collections::BTreeMap;
use std::f32::consts::PI;

use egui::epaint::{Mesh, Shadow, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Rect, RichText,
    Rounding, Stroke, TextStyle, Vec2,
};
use z_core::battery::BatteryStatus;
use z_core::model::{DisconnectPolicy, LogicalButton, VirtualTargetKind};
use z_core::profile::Profile;
use z_platform_windows::drivers::query_process_memory_bytes;

use crate::engine::{
    ControllerSnapshot, EngineCommand, EngineHandle, EngineSnapshot, PhysicalSnapshot,
    VirtualOutputSnapshot,
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

const SEGOE_UI_FONT_PATH: &str = r"C:\Windows\Fonts\segoeui.ttf";
const CARD_ROUNDING: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    ProfileEditor,
    LiveTest,
}

pub struct AppUiState {
    pub selected_controller: Option<String>,
    pub active_tab: MainTab,
    pub editing_profile_id: Option<String>,
    pub draft_profile: Option<Profile>,
    pub draft_dirty: bool,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            selected_controller: None,
            active_tab: MainTab::ProfileEditor,
            editing_profile_id: None,
            draft_profile: None,
            draft_dirty: false,
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
        (TextStyle::Heading, FontId::new(17.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
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
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
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

fn draw_status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

fn draw_battery_bar(ui: &mut egui::Ui, battery: &z_core::battery::BatteryReading) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Battery:").small().color(TEXT_MUTED));
        let bar_size = Vec2::new(112.0, 8.0);
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

pub fn draw_app_ui(ctx: &egui::Context, state: &mut AppUiState, engine: &EngineHandle) {
    let snap = engine.snapshot();
    draw_app_ui_snapshot(ctx, state, &snap, |cmd| engine.send(cmd));
}

pub fn draw_app_ui_snapshot(
    ctx: &egui::Context,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    mut send: impl FnMut(EngineCommand),
) {
    if state.selected_controller.is_none()
        || !snap
            .controllers
            .iter()
            .any(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
    {
        state.selected_controller = snap.controllers.first().map(|c| c.canonical_id.clone());
    }

    let selected_ctrl = snap
        .controllers
        .iter()
        .find(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
        .cloned();

    let desired_profile_id = selected_ctrl
        .as_ref()
        .map(|c| c.profile_id.clone())
        .or_else(|| Some(snap.config.default_profile_id.clone()));

    if let Some(pid) = desired_profile_id {
        if state.editing_profile_id.as_deref() != Some(&pid) && !state.draft_dirty {
            state.editing_profile_id = Some(pid.clone());
            state.draft_profile = Some(snap.config.profile_by_id(&pid).clone());
        }
    }

    // Bottom status bar (driver readiness, HidHide lock warnings, live RAM readout — NO popups)
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(36.0)
        .frame(
            egui::Frame::none()
                .fill(PANEL_SURFACE)
                .inner_margin(Margin::symmetric(12.0, 6.0)),
        )
        .show(ctx, |ui| {
            let r = ui.max_rect();
            ui.painter().line_segment(
                [Pos2::new(r.left() - 12.0, r.top() - 6.0), Pos2::new(r.right() + 12.0, r.top() - 6.0)],
                Stroke::new(1.0_f32, SUBTLE_BORDER),
            );
            draw_status_bar(ui, snap, &mut send);
        });

    // Left sidebar: Controller list with view-only battery & manual profile selector
    egui::SidePanel::left("controllers_panel")
        .resizable(false)
        .exact_width(322.0)
        .frame(
            egui::Frame::none()
                .fill(BG_NEAR_BLACK)
                .inner_margin(Margin::symmetric(8.0, 8.0)),
        )
        .show(ctx, |ui| {
            draw_controller_list(ui, state, snap, &mut send);
        });

    // Main area: Tabs for Profile Editor and Live Input Test
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(BG_NEAR_BLACK)
                .inner_margin(Margin::same(8.0)),
        )
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(PANEL_SURFACE)
                .rounding(CARD_ROUNDING)
                .stroke(Stroke::new(1.0_f32, SUBTLE_BORDER))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    ui.horizontal(|ui| {
                        draw_tab_button(ui, &mut state.active_tab, MainTab::ProfileEditor, "Profile Editor");
                        ui.add_space(6.0);
                        draw_tab_button(ui, &mut state.active_tab, MainTab::LiveTest, "Live Input Test");
                    });
                    ui.add_space(4.0);
                    let sep_y = ui.cursor().top();
                    let avail = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [Pos2::new(avail.left(), sep_y), Pos2::new(avail.right(), sep_y)],
                        Stroke::new(1.0_f32, SUBTLE_BORDER),
                    );
                    ui.add_space(8.0);

                    match state.active_tab {
                        MainTab::ProfileEditor => {
                            draw_profile_editor(ui, state, snap, &mut send);
                        }
                        MainTab::LiveTest => draw_live_test(ui, selected_ctrl.as_ref()),
                    }
                });
        });
}

fn draw_tab_button(ui: &mut egui::Ui, current: &mut MainTab, tab: MainTab, label: &str) {
    let active = *current == tab;
    let text = if active {
        RichText::new(label).color(TEXT_PRIMARY).strong()
    } else {
        RichText::new(label).color(TEXT_MUTED)
    };
    let fill = if active { WIDGET_SURFACE } else { Color32::TRANSPARENT };
    let btn = egui::Button::new(text)
        .fill(fill)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(6.0));
    let resp = ui.add(btn);
    if active {
        let r = resp.rect;
        ui.painter().line_segment(
            [Pos2::new(r.left() + 6.0, r.bottom()), Pos2::new(r.right() - 6.0, r.bottom())],
            Stroke::new(2.0_f32, NEON_VIOLET),
        );
    }
    if resp.clicked() {
        *current = tab;
    }
}

fn draw_status_bar(
    ui: &mut egui::Ui,
    snap: &EngineSnapshot,
    send: &mut impl FnMut(EngineCommand),
) {
    ui.horizontal_centered(|ui| {
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
                RichText::new("Reboot recommended to finish HidHide setup")
                    .small()
                    .color(TEXT_MUTED),
            );
        } else if let Some(err) = &ds.hidhide_error {
            ui.separator();
            ui.label(RichText::new(err).small().color(TEXT_MUTED));
        } else if let Some(msg) = &snap.last_status_message {
            ui.separator();
            ui.label(RichText::new(msg).small().color(TEXT_MUTED));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (ws, priv_b) = query_process_memory_bytes();
            let ws_mb = ws as f64 / (1024.0 * 1024.0);
            let priv_mb = priv_b as f64 / (1024.0 * 1024.0);
            ui.label(
                RichText::new(format!("RAM: {ws_mb:.1} MB (Priv {priv_mb:.1} MB)"))
                    .small()
                    .color(TEXT_MUTED),
            );
        });
    });
}

fn draw_controller_list(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    send: &mut impl FnMut(EngineCommand),
) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Controllers").color(TEXT_PRIMARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Refresh").clicked() {
                send(EngineCommand::RefreshStatus);
            }
        });
    });
    ui.add_space(4.0);
    let sep_y = ui.cursor().top();
    let avail = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [Pos2::new(avail.left(), sep_y), Pos2::new(avail.right(), sep_y)],
        Stroke::new(1.0_f32, SUBTLE_BORDER),
    );
    ui.add_space(6.0);

    if snap.controllers.is_empty() {
        egui::Frame::none()
            .fill(PANEL_SURFACE)
            .rounding(CARD_ROUNDING)
            .stroke(Stroke::new(1.0_f32, SUBTLE_BORDER))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.label(RichText::new("No physical controllers detected.").color(TEXT_PRIMARY));
                ui.label(
                    RichText::new("Connect a controller via Bluetooth or USB.")
                        .small()
                        .color(TEXT_MUTED),
                );
            });
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Horizontal/vertical padding inside ScrollArea ensures active cards' 5px outer glow never clips.
        egui::Frame::none()
            .inner_margin(Margin::symmetric(6.0, 5.0))
            .show(ui, |ui| {
                for ctrl in &snap.controllers {
                    let is_selected = state.selected_controller.as_deref() == Some(&ctrl.canonical_id);
                    let card_fill = if is_selected {
                        Color32::from_rgb(24, 24, 38)
                    } else {
                        PANEL_SURFACE
                    };
                    let card_stroke = if ctrl.virtual_active {
                        Stroke::NONE
                    } else {
                        Stroke::new(1.0_f32, if is_selected { NEON_VIOLET } else { SUBTLE_BORDER })
                    };

                    let card_resp = egui::Frame::none()
                        .fill(card_fill)
                        .rounding(CARD_ROUNDING)
                        .inner_margin(Margin::same(10.0))
                        .stroke(card_stroke)
                        .show(ui, |ui| {
                            let header_resp = ui.horizontal(|ui| {
                                let status_dot = if ctrl.connected { NEON_GREEN } else { TEXT_MUTED };
                                draw_status_dot(ui, status_dot);
                                ui.label(RichText::new(&ctrl.name).strong().color(TEXT_PRIMARY));
                            });
                            if ui
                                .interact(
                                    header_resp.response.rect,
                                    ui.id().with(&ctrl.canonical_id),
                                    egui::Sense::click(),
                                )
                                .clicked()
                            {
                                state.selected_controller = Some(ctrl.canonical_id.clone());
                                state.draft_dirty = false;
                                state.editing_profile_id = Some(ctrl.profile_id.clone());
                                state.draft_profile =
                                    Some(snap.config.profile_by_id(&ctrl.profile_id).clone());
                            }

                            ui.horizontal(|ui| {
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
                            });

                            draw_battery_bar(ui, &ctrl.battery);

                            ui.horizontal(|ui| {
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
                                        .inner_margin(Margin::symmetric(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(format!("-> {}", ctrl.target_kind.label()))
                                                    .small()
                                                    .color(TEXT_PRIMARY),
                                            );
                                        });
                                }
                                let cloak_dot = if ctrl.cloaked { NEON_GREEN } else { TEXT_MUTED };
                                draw_status_dot(ui, cloak_dot);
                                ui.label(
                                    RichText::new(if ctrl.cloaked { "Cloaked" } else { "Visible" })
                                        .small()
                                        .color(if ctrl.cloaked { TEXT_PRIMARY } else { TEXT_MUTED }),
                                );
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Profile:").small().color(TEXT_MUTED));
                                let mut chosen = ctrl.profile_id.clone();
                                egui::ComboBox::from_id_salt(("ctrl_prof", &ctrl.canonical_id))
                                    .width(190.0)
                                    .selected_text(RichText::new(&ctrl.profile_name).color(TEXT_PRIMARY))
                                    .show_ui(ui, |ui| {
                                        for p in &snap.config.profiles {
                                            if ui
                                                .selectable_value(&mut chosen, p.id.clone(), &p.name)
                                                .changed()
                                            {
                                                send(EngineCommand::SelectDeviceProfile {
                                                    key: ctrl.key.clone(),
                                                    profile_id: chosen.clone(),
                                                });
                                                state.draft_dirty = false;
                                                state.editing_profile_id = Some(chosen.clone());
                                                state.draft_profile = Some(p.clone());
                                            }
                                        }
                                    });
                            });

                            draw_card_live_status(ui, &ctrl.physical_state);
                        });

                    if ctrl.virtual_active {
                        paint_active_card_neon_edge_and_glow(
                            ui.painter(),
                            card_resp.response.rect,
                            CARD_ROUNDING,
                        );
                    }

                    ui.add_space(10.0);
                }
            });
    });
}

fn draw_profile_editor(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    send: &mut impl FnMut(EngineCommand),
) {
    ui.horizontal(|ui| {
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
        egui::ComboBox::from_id_salt("profile_selector")
            .width(240.0)
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
            let mut new_prof = Profile::default_x360();
            new_prof.id = format!("custom-{n}");
            new_prof.name = format!("Custom Profile {n}");
            state.editing_profile_id = Some(new_prof.id.clone());
            state.draft_profile = Some(new_prof.clone());
            state.draft_dirty = false;
            send(EngineCommand::UpsertProfile(new_prof));
        }

        if snap.config.profiles.len() > 1 {
            if let Some(id) = &state.editing_profile_id {
                if id != "x360-fifa" && id != "ds4-default" && ui.button("Delete").clicked() {
                    send(EngineCommand::DeleteProfile(id.clone()));
                    state.editing_profile_id = None;
                    state.draft_profile = None;
                    state.draft_dirty = false;
                    return;
                }
            }
        }
    });

    ui.add_space(6.0);
    let sep_y = ui.cursor().top();
    let avail = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [Pos2::new(avail.left(), sep_y), Pos2::new(avail.right(), sep_y)],
        Stroke::new(1.0_f32, SUBTLE_BORDER),
    );
    ui.add_space(8.0);

    let Some(prof) = state.draft_profile.as_mut() else {
        ui.label(RichText::new("Select a profile to edit.").color(TEXT_MUTED));
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("profile_settings_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Profile Name:").color(TEXT_MUTED));
                if ui.text_edit_singleline(&mut prof.name).changed() {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label(RichText::new("Virtual Output Target:").color(TEXT_MUTED));
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
                });
                ui.end_row();

                ui.label(RichText::new("HidHide Cloaking:").color(TEXT_MUTED));
                if ui
                    .checkbox(&mut prof.cloak_physical, "Hide physical controller from games")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label(RichText::new("Disconnect Behaviour:").color(TEXT_MUTED));
                ui.horizontal(|ui| {
                    if ui
                        .radio_value(
                            &mut prof.disconnect_policy,
                            DisconnectPolicy::KeepNeutral,
                            "Keep virtual pad at neutral (default)",
                        )
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                    if ui
                        .radio_value(
                            &mut prof.disconnect_policy,
                            DisconnectPolicy::DropVirtual,
                            "Drop virtual pad",
                        )
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                });
                ui.end_row();

                ui.label(RichText::new("Force Feedback:").color(TEXT_MUTED));
                if ui
                    .checkbox(&mut prof.rumble_enabled, "Forward game rumble to physical pad")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label(RichText::new("Face Layout:").color(TEXT_MUTED));
                if ui
                    .checkbox(&mut prof.swap_ab_xy, "Swap A/B & X/Y (Nintendo <-> Xbox)")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label(RichText::new("Left Stick Deadzone:").color(TEXT_MUTED));
                let mut l_pct = (prof.left_deadzone * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut l_pct, 0.0..=50.0).suffix("%"))
                    .changed()
                {
                    prof.left_deadzone = l_pct / 100.0;
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label(RichText::new("Right Stick Deadzone:").color(TEXT_MUTED));
                let mut r_pct = (prof.right_deadzone * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut r_pct, 0.0..=50.0).suffix("%"))
                    .changed()
                {
                    prof.right_deadzone = r_pct / 100.0;
                    state.draft_dirty = true;
                }
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Per-Button Remapping (Level 2)").strong().color(TEXT_PRIMARY));
            if ui.small_button("Reset All Remaps").clicked() {
                prof.button_remap.clear();
                state.draft_dirty = true;
            }
        });
        ui.label(
            RichText::new("Override any logical SDL3 button to another virtual button or Unbound.")
                .small()
                .color(TEXT_MUTED),
        );
        ui.add_space(4.0);

        egui::Grid::new("remap_grid")
            .num_columns(3)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for src in LogicalButton::ALL {
                    ui.label(RichText::new(src.short_label()).color(TEXT_PRIMARY));
                    let current = prof.button_remap.get(&src).copied();
                    let label = match current {
                        None => "Default (Pass)".to_string(),
                        Some(None) => "Unbound".to_string(),
                        Some(Some(dst)) => dst.short_label().to_string(),
                    };
                    egui::ComboBox::from_id_salt(("remap", src))
                        .width(155.0)
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current.is_none(), "Default (Pass)").clicked() {
                                prof.button_remap.remove(&src);
                                state.draft_dirty = true;
                            }
                            if ui.selectable_label(current == Some(None), "Unbound").clicked() {
                                prof.button_remap.insert(src, None);
                                state.draft_dirty = true;
                            }
                            for dst in LogicalButton::ALL {
                                if ui
                                    .selectable_label(current == Some(Some(dst)), dst.short_label())
                                    .clicked()
                                {
                                    prof.button_remap.insert(src, Some(dst));
                                    state.draft_dirty = true;
                                }
                            }
                        });
                    ui.label("");
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let btn_text = if state.draft_dirty {
                "Apply & Save Profile *"
            } else {
                "Apply & Save Profile"
            };
            let accent = if state.draft_dirty { NEON_PINK } else { NEON_VIOLET };
            let save_btn = egui::Button::new(RichText::new(btn_text).strong().color(TEXT_PRIMARY))
                .fill(Color32::from_rgb(27, 22, 42))
                .stroke(Stroke::new(1.0_f32, accent))
                .rounding(Rounding::same(6.0));
            if ui.add(save_btn).clicked() {
                send(EngineCommand::UpsertProfile(prof.clone()));
                state.draft_dirty = false;
            }
        });
    });
}

fn draw_live_test(ui: &mut egui::Ui, ctrl: Option<&ControllerSnapshot>) {
    let Some(c) = ctrl else {
        ui.label(
            RichText::new("Select a connected controller from the sidebar to inspect live inputs.")
                .color(TEXT_MUTED),
        );
        return;
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(&c.name).strong().color(TEXT_PRIMARY));
        ui.separator();
        ui.label(RichText::new(format!("Profile: {}", c.profile_name)).color(TEXT_MUTED));
        ui.separator();
        ui.label(RichText::new(format!("Output: {}", c.target_kind.label())).color(TEXT_PRIMARY));
    });
    ui.add_space(8.0);

    let (vx_l, vy_l, vx_r, vy_r, vlt, vrt) = match c.virtual_output {
        VirtualOutputSnapshot::Xbox360 {
            thumb_lx,
            thumb_ly,
            thumb_rx,
            thumb_ry,
            left_trigger,
            right_trigger,
            ..
        } => (
            thumb_lx as f32 / 32767.0,
            -(thumb_ly as f32 / 32767.0),
            thumb_rx as f32 / 32767.0,
            -(thumb_ry as f32 / 32767.0),
            left_trigger as f32 / 255.0,
            right_trigger as f32 / 255.0,
        ),
        VirtualOutputSnapshot::DualShock4 {
            thumb_lx,
            thumb_ly,
            thumb_rx,
            thumb_ry,
            trigger_left,
            trigger_right,
            ..
        } => (
            (thumb_lx as f32 - 128.0) / 127.0,
            (thumb_ly as f32 - 128.0) / 127.0,
            (thumb_rx as f32 - 128.0) / 127.0,
            (thumb_ry as f32 - 128.0) / 127.0,
            trigger_left as f32 / 255.0,
            trigger_right as f32 / 255.0,
        ),
        VirtualOutputSnapshot::None => (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    };

    ui.horizontal(|ui| {
        draw_stick_radar(
            ui,
            "Left Stick",
            c.physical_state.left_x as f32 / 32767.0,
            c.physical_state.left_y as f32 / 32767.0,
            vx_l,
            vy_l,
        );
        ui.add_space(18.0);
        draw_stick_radar(
            ui,
            "Right Stick",
            c.physical_state.right_x as f32 / 32767.0,
            c.physical_state.right_y as f32 / 32767.0,
            vx_r,
            vy_r,
        );
        ui.add_space(18.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("Triggers").strong().color(TEXT_PRIMARY));
            ui.add_space(6.0);
            draw_trigger_bar(ui, &format!("L2 / LT ({:.0}%)", vlt * 100.0), vlt);
            ui.add_space(8.0);
            draw_trigger_bar(ui, &format!("R2 / RT ({:.0}%)", vrt * 100.0), vrt);
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "Physical SDL3 L: ({}, {}) R: ({}, {})",
                    c.physical_state.left_x,
                    c.physical_state.left_y,
                    c.physical_state.right_x,
                    c.physical_state.right_y
                ))
                .small()
                .color(TEXT_MUTED),
            );
            ui.label(
                RichText::new(format!("Virtual Output: {:?}", c.virtual_output))
                    .small()
                    .color(TEXT_MUTED),
            );
        });
    });

    ui.add_space(12.0);
    ui.label(RichText::new("Buttons (Physical Pressed)").strong().color(TEXT_PRIMARY));
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for btn in LogicalButton::ALL {
            let down = (c.physical_state.buttons_mask & btn.bit()) != 0;
            let (fill, stroke) = if down {
                (Color32::from_rgb(30, 23, 48), Stroke::new(1.5_f32, NEON_VIOLET))
            } else {
                (BG_NEAR_BLACK, Stroke::new(1.0_f32, SUBTLE_BORDER))
            };
            let txt_col = if down { TEXT_PRIMARY } else { TEXT_MUTED };
            egui::Frame::none()
                .fill(fill)
                .stroke(stroke)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.label(RichText::new(btn.short_label()).color(txt_col));
                });
        }
    });
}

fn draw_trigger_bar(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.label(RichText::new(label).color(TEXT_MUTED));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(180.0, 10.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(5.0), BG_NEAR_BLACK);
    p.rect_stroke(rect, Rounding::same(5.0), Stroke::new(1.0_f32, SUBTLE_BORDER));
    let frac = value.clamp(0.0, 1.0);
    if frac > 0.0 {
        let inner = rect.shrink(1.0);
        let fill_w = (inner.width() * frac).max(4.0).min(inner.width());
        p.rect_filled(
            Rect::from_min_size(inner.min, Vec2::new(fill_w, inner.height())),
            Rounding::same(4.0),
            NEON_VIOLET,
        );
    }
}

fn draw_stick_radar(
    ui: &mut egui::Ui,
    title: &str,
    phys_x: f32,
    phys_y: f32,
    virt_x: f32,
    virt_y: f32,
) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).strong().color(TEXT_PRIMARY));
        let size = 140.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let p = ui.painter();
        let center = rect.center();
        let radius = size * 0.44;
        p.rect_filled(rect, CARD_ROUNDING, BG_NEAR_BLACK);
        p.rect_stroke(rect, CARD_ROUNDING, Stroke::new(1.0_f32, SUBTLE_BORDER));
        p.circle_stroke(center, radius, Stroke::new(1.2_f32, SUBTLE_BORDER));
        p.line_segment(
            [Pos2::new(center.x - radius, center.y), Pos2::new(center.x + radius, center.y)],
            Stroke::new(1.0_f32, SUBTLE_BORDER),
        );
        p.line_segment(
            [Pos2::new(center.x, center.y - radius), Pos2::new(center.x, center.y + radius)],
            Stroke::new(1.0_f32, SUBTLE_BORDER),
        );

        let phys_pt = Pos2::new(
            center.x + phys_x.clamp(-1.0, 1.0) * radius,
            center.y + phys_y.clamp(-1.0, 1.0) * radius,
        );
        p.circle_stroke(phys_pt, 5.0, Stroke::new(1.5_f32, NEON_PINK));

        let virt_pt = Pos2::new(
            center.x + virt_x.clamp(-1.0, 1.0) * radius,
            center.y + virt_y.clamp(-1.0, 1.0) * radius,
        );
        p.circle_filled(virt_pt, 5.0, NEON_GREEN);
    });
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

const CARD_MINI_BUTTONS_ROW1: [(LogicalButton, &str); 8] = [
    (LogicalButton::South, "A"),
    (LogicalButton::East, "B"),
    (LogicalButton::West, "X"),
    (LogicalButton::North, "Y"),
    (LogicalButton::LeftShoulder, "LB"),
    (LogicalButton::RightShoulder, "RB"),
    (LogicalButton::LeftStick, "L3"),
    (LogicalButton::RightStick, "R3"),
];

const CARD_MINI_BUTTONS_ROW2: [(LogicalButton, &str); 7] = [
    (LogicalButton::DpadUp, "Up"),
    (LogicalButton::DpadDown, "Dn"),
    (LogicalButton::DpadLeft, "Lt"),
    (LogicalButton::DpadRight, "Rt"),
    (LogicalButton::Back, "Bk"),
    (LogicalButton::Start, "St"),
    (LogicalButton::Guide, "Gd"),
];

fn draw_card_live_status(ui: &mut egui::Ui, phys: &PhysicalSnapshot) {
    ui.add_space(4.0);
    let sep_y = ui.cursor().top();
    let bounds = ui.min_rect();
    ui.painter().line_segment(
        [Pos2::new(bounds.left(), sep_y), Pos2::new(bounds.right(), sep_y)],
        Stroke::new(1.0_f32, SUBTLE_BORDER),
    );
    ui.add_space(6.0);

    ui.horizontal(|ui| {
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
            ui.add_space(3.0);
            draw_mini_trigger_row(ui, "LT", normalize_trigger_axis(phys.left_trigger));
            ui.add_space(4.0);
            draw_mini_trigger_row(ui, "RT", normalize_trigger_axis(phys.right_trigger));
        });
    });

    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.5;
        for (btn, label) in CARD_MINI_BUTTONS_ROW1 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, label, pressed);
        }
    });
    ui.add_space(3.5);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.5;
        for (btn, label) in CARD_MINI_BUTTONS_ROW2 {
            let pressed = (phys.buttons_mask & btn.bit()) != 0;
            draw_mini_button_pill(ui, label, pressed);
        }
    });
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
        [Pos2::new(center.x - radius + 2.0, center.y), Pos2::new(center.x + radius - 2.0, center.y)],
        Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 44)),
    );
    p.line_segment(
        [Pos2::new(center.x, center.y - radius + 2.0), Pos2::new(center.x, center.y + radius - 2.0)],
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
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(
            RichText::new(label)
                .font(FontId::new(10.5, FontFamily::Proportional))
                .color(TEXT_MUTED),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::new(82.0, 7.0), egui::Sense::hover());
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

fn draw_mini_button_pill(ui: &mut egui::Ui, label: &str, pressed: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(26.5, 17.0), egui::Sense::hover());
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
    use z_core::backend::DriverStatus;
    use z_core::battery::BatteryReading;
    use z_core::model::{ControllerFamily, HardwareKey, Transport, VirtualTargetKind};
    use z_core::profile::AppConfig;

    #[test]
    fn battery_bar_uses_pink_at_or_below_twenty_and_green_above() {
        assert_eq!(battery_bar_color(Some(0)), NEON_PINK);
        assert_eq!(battery_bar_color(Some(20)), NEON_PINK);
        assert_eq!(battery_bar_color(Some(21)), NEON_GREEN);
        assert_eq!(battery_bar_color(Some(100)), NEON_GREEN);
        assert_eq!(battery_bar_color(None), NEON_GREEN);
    }

    #[test]
    fn gradient_lerp_endpoints_match_violet_to_pink() {
        assert_eq!(lerp_color(NEON_VIOLET, NEON_PINK, 0.0), NEON_VIOLET);
        assert_eq!(lerp_color(NEON_VIOLET, NEON_PINK, 1.0), NEON_PINK);
        let mid = lerp_color(NEON_VIOLET, NEON_PINK, 0.5);
        assert_eq!(mid, Color32::from_rgb(212, 102, 212));
    }

    #[test]
    fn card_live_status_covers_all_buttons_and_axis_normalization() {
        let mut seen = Vec::new();
        for (btn, _) in CARD_MINI_BUTTONS_ROW1
            .iter()
            .chain(CARD_MINI_BUTTONS_ROW2.iter())
        {
            if !seen.contains(btn) {
                seen.push(*btn);
            }
        }
        assert_eq!(seen.len(), LogicalButton::ALL.len());
        for b in LogicalButton::ALL {
            assert!(seen.contains(&b), "missing button {b:?} from card live status view");
        }

        assert_eq!(normalize_stick_axis(0), 0.0);
        assert_eq!(normalize_stick_axis(i16::MAX), 1.0);
        assert_eq!(normalize_stick_axis(i16::MIN), -1.0);
        assert_eq!(normalize_trigger_axis(0), 0.0);
        assert_eq!(normalize_trigger_axis(-500), 0.0);
        assert_eq!(normalize_trigger_axis(i16::MAX), 1.0);
    }

    #[test]
    fn restyled_ui_renders_both_tabs_with_active_glow_and_subtle_cards() {
        let ctx = egui::Context::default();
        configure_neon_theme_and_fonts(&ctx);
        let snap = EngineSnapshot {
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
                    profile_id: "x360-fifa".to_string(),
                    profile_name: "Xbox 360 Default (FIFA)".to_string(),
                    target_kind: VirtualTargetKind::Xbox360,
                    cloaked: true,
                    virtual_active: true,
                    virtual_serial: Some(1),
                    battery: BatteryReading {
                        percent: Some(86),
                        status: BatteryStatus::OnBattery,
                        source: "sdl3",
                    },
                    physical_state: PhysicalSnapshot {
                        buttons_mask: LogicalButton::South.bit() | LogicalButton::RightShoulder.bit(),
                        left_x: 24000,
                        left_y: -21000,
                        right_x: 0,
                        right_y: 0,
                        left_trigger: 0,
                        right_trigger: 22384,
                    },
                    virtual_output: VirtualOutputSnapshot::None,
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
                    profile_id: "ds4-default".to_string(),
                    profile_name: "DualShock 4 Default".to_string(),
                    target_kind: VirtualTargetKind::DualShock4,
                    cloaked: false,
                    virtual_active: false,
                    virtual_serial: None,
                    battery: BatteryReading {
                        percent: Some(18),
                        status: BatteryStatus::OnBattery,
                        source: "sony-hid",
                    },
                    physical_state: PhysicalSnapshot::default(),
                    virtual_output: VirtualOutputSnapshot::None,
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
        };

        let mut ui_state = AppUiState::default();
        for tab in [MainTab::ProfileEditor, MainTab::LiveTest] {
            ui_state.active_tab = tab;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 640.0))),
                ..Default::default()
            };
            let out = ctx.run(raw, |c| {
                draw_app_ui_snapshot(c, &mut ui_state, &snap, |_| {});
            });
            let primitives = ctx.tessellate(out.shapes, 1.0);
            assert!(!primitives.is_empty(), "expected tessellated shapes for {tab:?}");
        }
    }
}
