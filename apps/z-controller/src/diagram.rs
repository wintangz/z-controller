use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use z_core::model::{ControllerFamily, LogicalButton};
use z_core::profile::Profile;

use crate::engine::PhysicalSnapshot;
use crate::ui::{
    BG_NEAR_BLACK, NEON_GREEN, NEON_PINK, NEON_VIOLET, PANEL_SURFACE, SUBTLE_BORDER, TEXT_MUTED,
    TEXT_PRIMARY, normalize_stick_axis, normalize_trigger_axis,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceGlyph {
    Text(&'static str),
    Triangle,
    Circle,
    Square,
    Cross,
}

#[derive(Debug, Clone, Copy)]
struct DiagramLayoutSpec {
    left_stick_rel: Pos2,
    dpad_rel: Pos2,
    right_stick_rel: Pos2,
    face_center_rel: Pos2,
    face_glyphs: [FaceGlyph; 4], // [North, West, East, South]
    lt_label: &'static str,
    rt_label: &'static str,
    lb_label: &'static str,
    rb_label: &'static str,
}

fn family_spec(family: ControllerFamily) -> DiagramLayoutSpec {
    match family {
        ControllerFamily::Sony => DiagramLayoutSpec {
            // PlayStation symmetric lower sticks + upper-left D-pad
            dpad_rel: Pos2::new(0.30, 0.41),
            left_stick_rel: Pos2::new(0.40, 0.61),
            right_stick_rel: Pos2::new(0.60, 0.61),
            face_center_rel: Pos2::new(0.70, 0.41),
            face_glyphs: [
                FaceGlyph::Triangle,
                FaceGlyph::Square,
                FaceGlyph::Circle,
                FaceGlyph::Cross,
            ],
            lt_label: "L2",
            rt_label: "R2",
            lb_label: "L1",
            rb_label: "R1",
        },
        ControllerFamily::Nintendo => DiagramLayoutSpec {
            // Switch Pro asymmetric sticks + Nintendo A/B & X/Y physical positions
            left_stick_rel: Pos2::new(0.30, 0.41),
            dpad_rel: Pos2::new(0.39, 0.60),
            right_stick_rel: Pos2::new(0.61, 0.60),
            face_center_rel: Pos2::new(0.70, 0.41),
            face_glyphs: [
                FaceGlyph::Text("X"),
                FaceGlyph::Text("Y"),
                FaceGlyph::Text("A"),
                FaceGlyph::Text("B"),
            ],
            lt_label: "ZL",
            rt_label: "ZR",
            lb_label: "L",
            rb_label: "R",
        },
        ControllerFamily::Xbox | ControllerFamily::Generic => DiagramLayoutSpec {
            // Xbox asymmetric sticks + Xbox X/Y & A/B positions
            left_stick_rel: Pos2::new(0.30, 0.41),
            dpad_rel: Pos2::new(0.39, 0.60),
            right_stick_rel: Pos2::new(0.61, 0.60),
            face_center_rel: Pos2::new(0.70, 0.41),
            face_glyphs: [
                FaceGlyph::Text("Y"),
                FaceGlyph::Text("X"),
                FaceGlyph::Text("B"),
                FaceGlyph::Text("A"),
            ],
            lt_label: "LT",
            rt_label: "RT",
            lb_label: "LB",
            rb_label: "RB",
        },
    }
}

/// Renders an interactive `x360ce`-style vector controller diagram using the software `egui::Painter`.
/// Clicking any control selects it for remapping; active physical input highlights its control live.
pub fn draw_controller_diagram(
    ui: &mut egui::Ui,
    family: ControllerFamily,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let spec = family_spec(family);
    let avail_w = ui.available_width().max(460.0);
    let diagram_h = 304.0_f32;
    let (canvas, _) = ui.allocate_exact_size(Vec2::new(avail_w, diagram_h), Sense::hover());

    {
        let p = ui.painter();
        p.rect_filled(canvas, Rounding::same(8.0), BG_NEAR_BLACK);
        p.rect_stroke(canvas, Rounding::same(8.0), Stroke::new(1.0_f32, SUBTLE_BORDER));

        // Fit fixed aspect stage inside canvas with consistent 12px inner padding.
        let stage_w = (canvas.width() - 24.0).min(488.0);
        let stage_h = canvas.height() - 16.0;
        let stage = Rect::from_center_size(canvas.center(), Vec2::new(stage_w, stage_h));

        paint_body_silhouette(p, stage);

        let pt = |rel: Pos2| -> Pos2 {
            Pos2::new(stage.left() + rel.x * stage.width(), stage.top() + rel.y * stage.height())
        };

        // Top Triggers (analog fill bars inside trigger caps)
        let lt_rect = Rect::from_center_size(pt(Pos2::new(0.28, 0.11)), Vec2::new(56.0, 22.0));
        let rt_rect = Rect::from_center_size(pt(Pos2::new(0.72, 0.11)), Vec2::new(56.0, 22.0));
        paint_trigger_cap(
            p,
            lt_rect,
            spec.lt_label,
            phys.map_or(0.0, |s| normalize_trigger_axis(s.left_trigger)),
        );
        paint_trigger_cap(
            p,
            rt_rect,
            spec.rt_label,
            phys.map_or(0.0, |s| normalize_trigger_axis(s.right_trigger)),
        );

        // Shoulders (LB/RB, L1/R1, L/R)
        let lb_rect = Rect::from_center_size(pt(Pos2::new(0.29, 0.22)), Vec2::new(72.0, 18.0));
        let rb_rect = Rect::from_center_size(pt(Pos2::new(0.71, 0.22)), Vec2::new(72.0, 18.0));
        draw_rect_button(
            ui,
            lb_rect,
            6.0,
            spec.lb_label,
            LogicalButton::LeftShoulder,
            profile,
            phys,
            selected_button,
        );
        draw_rect_button(
            ui,
            rb_rect,
            6.0,
            spec.rb_label,
            LogicalButton::RightShoulder,
            profile,
            phys,
            selected_button,
        );

        // Left & Right Analog Sticks + Stick Clicks
        draw_stick_well(
            ui,
            pt(spec.left_stick_rel),
            24.0,
            LogicalButton::LeftStick,
            phys.map_or(0.0, |s| normalize_stick_axis(s.left_x)),
            phys.map_or(0.0, |s| normalize_stick_axis(s.left_y)),
            profile,
            phys,
            selected_button,
        );
        draw_stick_well(
            ui,
            pt(spec.right_stick_rel),
            24.0,
            LogicalButton::RightStick,
            phys.map_or(0.0, |s| normalize_stick_axis(s.right_x)),
            phys.map_or(0.0, |s| normalize_stick_axis(s.right_y)),
            profile,
            phys,
            selected_button,
        );

        // 4-Way D-Pad
        draw_dpad_cluster(ui, pt(spec.dpad_rel), profile, phys, selected_button);

        // Face Button Diamond ([North, West, East, South])
        let fc = pt(spec.face_center_rel);
        let d = 22.0_f32;
        let face_defs = [
            (LogicalButton::North, Pos2::new(fc.x, fc.y - d), spec.face_glyphs[0]),
            (LogicalButton::West, Pos2::new(fc.x - d, fc.y), spec.face_glyphs[1]),
            (LogicalButton::East, Pos2::new(fc.x + d, fc.y), spec.face_glyphs[2]),
            (LogicalButton::South, Pos2::new(fc.x, fc.y + d), spec.face_glyphs[3]),
        ];
        for (btn, pos, glyph) in face_defs {
            draw_face_button(ui, pos, 11.5, btn, glyph, profile, phys, selected_button);
        }

        // Center Cluster (family specific)
        match family {
            ControllerFamily::Sony => {
                let tp_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.36)), Vec2::new(88.0, 34.0));
                draw_rect_button(
                    ui,
                    tp_rect,
                    6.0,
                    "Touchpad",
                    LogicalButton::Touchpad,
                    profile,
                    phys,
                    selected_button,
                );
                let cr_rect =
                    Rect::from_center_size(pt(Pos2::new(0.38, 0.31)), Vec2::new(14.0, 22.0));
                let op_rect =
                    Rect::from_center_size(pt(Pos2::new(0.62, 0.31)), Vec2::new(14.0, 22.0));
                draw_rect_button(
                    ui,
                    cr_rect,
                    4.0,
                    "Cr",
                    LogicalButton::Back,
                    profile,
                    phys,
                    selected_button,
                );
                draw_rect_button(
                    ui,
                    op_rect,
                    4.0,
                    "Op",
                    LogicalButton::Start,
                    profile,
                    phys,
                    selected_button,
                );
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.50, 0.50)),
                    10.5,
                    LogicalButton::Guide,
                    FaceGlyph::Text("PS"),
                    profile,
                    phys,
                    selected_button,
                );
                let mic_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.61)), Vec2::new(28.0, 12.0));
                draw_rect_button(
                    ui,
                    mic_rect,
                    4.0,
                    "Mic",
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
            }
            ControllerFamily::Nintendo => {
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.44, 0.39)),
                    9.5,
                    LogicalButton::Back,
                    FaceGlyph::Text("-"),
                    profile,
                    phys,
                    selected_button,
                );
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.56, 0.39)),
                    9.5,
                    LogicalButton::Start,
                    FaceGlyph::Text("+"),
                    profile,
                    phys,
                    selected_button,
                );
                let cap_rect =
                    Rect::from_center_size(pt(Pos2::new(0.44, 0.50)), Vec2::new(20.0, 20.0));
                draw_rect_button(
                    ui,
                    cap_rect,
                    3.0,
                    "Cap",
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.56, 0.50)),
                    10.0,
                    LogicalButton::Guide,
                    FaceGlyph::Text("Hm"),
                    profile,
                    phys,
                    selected_button,
                );
            }
            ControllerFamily::Xbox | ControllerFamily::Generic => {
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.50, 0.34)),
                    11.5,
                    LogicalButton::Guide,
                    FaceGlyph::Text("Xb"),
                    profile,
                    phys,
                    selected_button,
                );
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.43, 0.43)),
                    9.5,
                    LogicalButton::Back,
                    FaceGlyph::Text("Vw"),
                    profile,
                    phys,
                    selected_button,
                );
                draw_face_button(
                    ui,
                    pt(Pos2::new(0.57, 0.43)),
                    9.5,
                    LogicalButton::Start,
                    FaceGlyph::Text("Mn"),
                    profile,
                    phys,
                    selected_button,
                );
                let shr_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.52)), Vec2::new(28.0, 14.0));
                draw_rect_button(
                    ui,
                    shr_rect,
                    4.0,
                    "Shr",
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
            }
        }

        // Rear Paddles Strip (bottom edge of diagram)
        let paddles = [
            (Pos2::new(0.24, 0.91), "L4", LogicalButton::LeftPaddle1),
            (Pos2::new(0.34, 0.91), "L5", LogicalButton::LeftPaddle2),
            (Pos2::new(0.66, 0.91), "R5", LogicalButton::RightPaddle2),
            (Pos2::new(0.76, 0.91), "R4", LogicalButton::RightPaddle1),
        ];
        for (rel, lbl, btn) in paddles {
            let r = Rect::from_center_size(pt(rel), Vec2::new(34.0, 17.0));
            draw_rect_button(ui, r, 4.0, lbl, btn, profile, phys, selected_button);
        }

        // Legend hint bottom-center
        let p = ui.painter();
        p.text(
            pt(Pos2::new(0.50, 0.91)),
            Align2::CENTER_CENTER,
            "Click part to select",
            FontId::new(10.0, FontFamily::Proportional),
            TEXT_MUTED,
        );
    }
}

fn paint_body_silhouette(p: &egui::Painter, stage: Rect) {
    let cx = stage.center().x;
    let deck = Rect::from_center_size(
        Pos2::new(cx, stage.top() + stage.height() * 0.48),
        Vec2::new(stage.width() * 0.56, stage.height() * 0.46),
    );
    let grip_l = Pos2::new(stage.left() + stage.width() * 0.22, stage.top() + stage.height() * 0.66);
    let grip_r = Pos2::new(stage.left() + stage.width() * 0.78, stage.top() + stage.height() * 0.66);
    let grip_r_rad = stage.height() * 0.21;
    let bridge = Rect::from_center_size(
        Pos2::new(cx, stage.top() + stage.height() * 0.34),
        Vec2::new(stage.width() * 0.52, stage.height() * 0.24),
    );

    // Outer border pass then interior pass creates one unified seamless body outline.
    let border_col = SUBTLE_BORDER;
    p.rect_filled(deck.expand(1.5), Rounding::same(28.0), border_col);
    p.rect_filled(bridge.expand(1.5), Rounding::same(20.0), border_col);
    p.circle_filled(grip_l, grip_r_rad + 1.5, border_col);
    p.circle_filled(grip_r, grip_r_rad + 1.5, border_col);

    p.rect_filled(deck, Rounding::same(27.0), PANEL_SURFACE);
    p.rect_filled(bridge, Rounding::same(19.0), PANEL_SURFACE);
    p.circle_filled(grip_l, grip_r_rad, PANEL_SURFACE);
    p.circle_filled(grip_r, grip_r_rad, PANEL_SURFACE);
}

fn paint_trigger_cap(p: &egui::Painter, rect: Rect, label: &str, value: f32) {
    let pressed = value > 0.05;
    let border = if pressed { NEON_PINK } else { SUBTLE_BORDER };
    p.rect_filled(rect, Rounding::same(6.0), PANEL_SURFACE);
    p.rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(if pressed { 1.5_f32 } else { 1.0_f32 }, border),
    );

    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + 4.0, rect.bottom() - 6.0),
        Pos2::new(rect.right() - 4.0, rect.bottom() - 2.5),
    );
    p.rect_filled(bar, Rounding::same(1.5), BG_NEAR_BLACK);
    if value > 0.01 {
        let fw = (bar.width() * value.clamp(0.0, 1.0)).max(2.0);
        p.rect_filled(
            Rect::from_min_size(bar.min, Vec2::new(fw, bar.height())),
            Rounding::same(1.5),
            NEON_VIOLET,
        );
    }
    p.text(
        Pos2::new(rect.center().x, rect.top() + 8.0),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.5, FontFamily::Proportional),
        if pressed { TEXT_PRIMARY } else { TEXT_MUTED },
    );
}

fn button_visuals(
    btn: LogicalButton,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: Option<LogicalButton>,
) -> (bool, bool, bool, Color32, Stroke) {
    let pressed = phys.is_some_and(|s| (s.buttons_mask & btn.bit()) != 0);
    let selected = selected_button == Some(btn);
    let remapped = profile.button_remap.contains_key(&btn);

    let fill = if pressed {
        Color32::from_rgb(44, 24, 62)
    } else if selected {
        Color32::from_rgb(28, 23, 46)
    } else {
        Color32::from_rgb(20, 20, 34)
    };
    let stroke = if pressed {
        Stroke::new(1.8_f32, NEON_PINK)
    } else if selected {
        Stroke::new(1.8_f32, NEON_VIOLET)
    } else if remapped {
        Stroke::new(1.2_f32, Color32::from_rgb(122, 86, 185))
    } else {
        Stroke::new(1.0_f32, SUBTLE_BORDER)
    };
    (pressed, selected, remapped, fill, stroke)
}

#[allow(clippy::too_many_arguments)]
fn draw_rect_button(
    ui: &mut egui::Ui,
    rect: Rect,
    rounding: f32,
    label: &str,
    btn: LogicalButton,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let resp = ui.interact(rect, ui.id().with(("diag_btn", btn)), Sense::click());
    if resp.clicked() {
        *selected_button = Some(btn);
    }
    let (pressed, selected, remapped, fill, stroke) =
        button_visuals(btn, profile, phys, *selected_button);
    let p = ui.painter();
    if pressed || selected {
        let glow_col = if pressed { NEON_PINK } else { NEON_VIOLET };
        p.rect_stroke(
            rect.expand(1.6),
            Rounding::same(rounding + 1.6),
            Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(glow_col.r(), glow_col.g(), glow_col.b(), 72)),
        );
    }
    p.rect_filled(rect, Rounding::same(rounding), fill);
    p.rect_stroke(rect, Rounding::same(rounding), stroke);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.5, FontFamily::Proportional),
        if pressed || selected { TEXT_PRIMARY } else { TEXT_MUTED },
    );
    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(rect.right() - 4.0, rect.top() + 4.0), 2.0, NEON_VIOLET);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_face_button(
    ui: &mut egui::Ui,
    center: Pos2,
    radius: f32,
    btn: LogicalButton,
    glyph: FaceGlyph,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    let resp = ui.interact(rect, ui.id().with(("diag_face", btn)), Sense::click());
    if resp.clicked() {
        *selected_button = Some(btn);
    }
    let (pressed, selected, remapped, fill, stroke) =
        button_visuals(btn, profile, phys, *selected_button);
    let p = ui.painter();
    if pressed || selected {
        let glow_col = if pressed { NEON_PINK } else { NEON_VIOLET };
        p.circle_stroke(
            center,
            radius + 1.8,
            Stroke::new(2.2_f32, Color32::from_rgba_unmultiplied(glow_col.r(), glow_col.g(), glow_col.b(), 72)),
        );
    }
    p.circle_filled(center, radius, fill);
    p.circle_stroke(center, radius, stroke);

    let fg = if pressed || selected { TEXT_PRIMARY } else { TEXT_MUTED };
    match glyph {
        FaceGlyph::Text(t) => {
            p.text(
                center,
                Align2::CENTER_CENTER,
                t,
                FontId::new(10.5, FontFamily::Proportional),
                fg,
            );
        }
        FaceGlyph::Triangle => {
            let r = radius * 0.48;
            let a = Pos2::new(center.x, center.y - r);
            let b = Pos2::new(center.x - r * 0.9, center.y + r * 0.72);
            let c = Pos2::new(center.x + r * 0.9, center.y + r * 0.72);
            let st = Stroke::new(1.4_f32, if pressed { NEON_GREEN } else { fg });
            p.line_segment([a, b], st);
            p.line_segment([b, c], st);
            p.line_segment([c, a], st);
        }
        FaceGlyph::Square => {
            let r = radius * 0.44;
            let sq = Rect::from_center_size(center, Vec2::splat(r * 2.0));
            p.rect_stroke(
                sq,
                Rounding::same(1.0),
                Stroke::new(1.4_f32, if pressed { NEON_PINK } else { fg }),
            );
        }
        FaceGlyph::Circle => {
            p.circle_stroke(
                center,
                radius * 0.46,
                Stroke::new(1.4_f32, if pressed { NEON_PINK } else { fg }),
            );
        }
        FaceGlyph::Cross => {
            let r = radius * 0.42;
            let st = Stroke::new(1.5_f32, if pressed { NEON_VIOLET } else { fg });
            p.line_segment([Pos2::new(center.x - r, center.y - r), Pos2::new(center.x + r, center.y + r)], st);
            p.line_segment([Pos2::new(center.x + r, center.y - r), Pos2::new(center.x - r, center.y + r)], st);
        }
    }

    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(center.x + radius * 0.65, center.y - radius * 0.65), 2.2, NEON_VIOLET);
    }
}

fn draw_dpad_cluster(
    ui: &mut egui::Ui,
    center: Pos2,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let arm = 16.0_f32;
    let p = ui.painter();
    p.circle_filled(center, 7.0, Color32::from_rgb(20, 20, 34));
    let arms = [
        (LogicalButton::DpadUp, Rect::from_center_size(Pos2::new(center.x, center.y - arm), Vec2::new(14.0, 15.0)), "^"),
        (LogicalButton::DpadDown, Rect::from_center_size(Pos2::new(center.x, center.y + arm), Vec2::new(14.0, 15.0)), "v"),
        (LogicalButton::DpadLeft, Rect::from_center_size(Pos2::new(center.x - arm, center.y), Vec2::new(15.0, 14.0)), "<"),
        (LogicalButton::DpadRight, Rect::from_center_size(Pos2::new(center.x + arm, center.y), Vec2::new(15.0, 14.0)), ">"),
    ];
    for (btn, rect, glyph) in arms {
        draw_rect_button(ui, rect, 3.0, glyph, btn, profile, phys, selected_button);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_stick_well(
    ui: &mut egui::Ui,
    center: Pos2,
    radius: f32,
    click_btn: LogicalButton,
    x: f32,
    y: f32,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    let resp = ui.interact(rect, ui.id().with(("diag_stick", click_btn)), Sense::click());
    if resp.clicked() {
        *selected_button = Some(click_btn);
    }
    let (pressed, selected, remapped, _fill, stroke) =
        button_visuals(click_btn, profile, phys, *selected_button);
    let p = ui.painter();
    if pressed || selected {
        let glow_col = if pressed { NEON_PINK } else { NEON_VIOLET };
        p.circle_stroke(
            center,
            radius + 1.8,
            Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(glow_col.r(), glow_col.g(), glow_col.b(), 72)),
        );
    }
    p.circle_filled(center, radius, BG_NEAR_BLACK);
    p.circle_stroke(center, radius, stroke);
    p.line_segment(
        [Pos2::new(center.x - radius + 4.0, center.y), Pos2::new(center.x + radius - 4.0, center.y)],
        Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 44)),
    );
    p.line_segment(
        [Pos2::new(center.x, center.y - radius + 4.0), Pos2::new(center.x, center.y + radius - 4.0)],
        Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 44)),
    );

    let mag = (x * x + y * y).sqrt();
    let travel = radius - 6.0_f32;
    let clamped = if mag > 1.0 { 1.0 / mag } else { 1.0 };
    let dot = Pos2::new(center.x + x * clamped * travel, center.y + y * clamped * travel);
    if mag > 0.08 {
        p.circle_filled(
            dot,
            7.5,
            Color32::from_rgba_unmultiplied(NEON_PINK.r(), NEON_PINK.g(), NEON_PINK.b(), 64),
        );
        p.circle_filled(dot, 4.2, NEON_PINK);
    } else {
        p.circle_filled(dot, 3.6, if pressed { NEON_GREEN } else { NEON_VIOLET });
    }
    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(center.x + radius - 3.0, center.y - radius + 3.0), 2.2, NEON_VIOLET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_layouts_distinguish_xbox_sony_and_nintendo() {
        let xbox = family_spec(ControllerFamily::Xbox);
        let sony = family_spec(ControllerFamily::Sony);
        let nintendo = family_spec(ControllerFamily::Nintendo);

        // Xbox and Nintendo have asymmetric upper-left stick; PlayStation is symmetric lower-left.
        assert!(xbox.left_stick_rel.y < xbox.dpad_rel.y);
        assert!(nintendo.left_stick_rel.y < nintendo.dpad_rel.y);
        assert!(sony.dpad_rel.y < sony.left_stick_rel.y);

        // Face glyphs match family conventions.
        assert!(matches!(xbox.face_glyphs[2], FaceGlyph::Text("B")));
        assert!(matches!(xbox.face_glyphs[3], FaceGlyph::Text("A")));
        assert!(matches!(nintendo.face_glyphs[2], FaceGlyph::Text("A")));
        assert!(matches!(nintendo.face_glyphs[3], FaceGlyph::Text("B")));
        assert!(matches!(sony.face_glyphs[0], FaceGlyph::Triangle));
        assert!(matches!(sony.face_glyphs[3], FaceGlyph::Cross));
    }

    #[test]
    fn diagram_renders_all_three_families_without_panic() {
        let ctx = egui::Context::default();
        let prof = Profile::builtin_xbox360();
        let phys = PhysicalSnapshot {
            buttons_mask: LogicalButton::South.bit() | LogicalButton::Misc1.bit(),
            left_x: 20000,
            left_y: -20000,
            right_x: 0,
            right_y: 0,
            left_trigger: 0,
            right_trigger: 18000,
        };
        for fam in [
            ControllerFamily::Xbox,
            ControllerFamily::Sony,
            ControllerFamily::Nintendo,
        ] {
            let mut sel = Some(LogicalButton::Misc1);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
                ..Default::default()
            };
            let out = ctx.run(raw, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    draw_controller_diagram(ui, fam, &prof, Some(&phys), &mut sel);
                });
            });
            assert!(!ctx.tessellate(out.shapes, 1.0).is_empty());
        }
    }
}
