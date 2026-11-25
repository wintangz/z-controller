use egui::epaint::{Mesh, PathShape};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Shape, Stroke, Vec2};
use z_core::model::{ControllerFamily, LogicalButton};
use z_core::profile::Profile;

use crate::engine::PhysicalSnapshot;
use crate::ui::{
    BG_NEAR_BLACK, LOGO_BLUE, LOGO_LAVENDER, LOGO_PINK, SUBTLE_BORDER, TEXT_MUTED, TEXT_PRIMARY,
    normalize_stick_axis, normalize_trigger_axis,
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
enum CenterVisual {
    XboxLogo,
    ViewBoxes,
    MenuLines,
    ShareTray,
    TouchpadBar,
    CreateRays,
    OptionsLines,
    PsLogo,
    MicMute,
    MinusBar,
    PlusMark,
    CaptureSquare,
    HomeHouse,
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
            // DualSense: upper D-Pad, symmetric lower sticks
            dpad_rel: Pos2::new(0.285, 0.425),
            left_stick_rel: Pos2::new(0.395, 0.615),
            right_stick_rel: Pos2::new(0.605, 0.615),
            face_center_rel: Pos2::new(0.715, 0.425),
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
            // Switch Pro: asymmetric sticks + Nintendo face positions (X top, Y left, A right, B bottom)
            left_stick_rel: Pos2::new(0.285, 0.43),
            dpad_rel: Pos2::new(0.385, 0.605),
            right_stick_rel: Pos2::new(0.615, 0.605),
            face_center_rel: Pos2::new(0.715, 0.43),
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
            // Xbox: asymmetric sticks + Y/X/B/A face diamond
            left_stick_rel: Pos2::new(0.285, 0.43),
            dpad_rel: Pos2::new(0.385, 0.61),
            right_stick_rel: Pos2::new(0.615, 0.61),
            face_center_rel: Pos2::new(0.715, 0.43),
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

/// Renders an interactive `x360ce`-style vector controller diagram using `egui::Painter`
/// and returns its bounding `Rect` for dirty-region invalidation.
pub fn draw_controller_diagram(
    ui: &mut egui::Ui,
    family: ControllerFamily,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) -> Rect {
    let spec = family_spec(family);
    let avail_w = ui.available_width().max(460.0);
    let diagram_h = 314.0_f32;
    let (canvas, _) = ui.allocate_exact_size(Vec2::new(avail_w, diagram_h), Sense::hover());

    {
        let p = ui.painter();
        p.rect_filled(canvas, Rounding::same(8.0), BG_NEAR_BLACK);
        p.rect_stroke(canvas, Rounding::same(8.0), Stroke::new(1.0_f32, SUBTLE_BORDER));

        // Fit clean fixed-aspect controller illustration inside canvas.
        let stage_w = (canvas.width() - 24.0).min(486.0);
        let stage_h = canvas.height() - 16.0;
        let stage = Rect::from_center_size(canvas.center(), Vec2::new(stage_w, stage_h));

        let pt = |rel: Pos2| -> Pos2 {
            Pos2::new(stage.left() + rel.x * stage.width(), stage.top() + rel.y * stage.height())
        };

        // Top Triggers (sleek rounded bars with live analog fill)
        let lt_rect = Rect::from_center_size(pt(Pos2::new(0.285, 0.115)), Vec2::new(62.0, 22.0));
        let rt_rect = Rect::from_center_size(pt(Pos2::new(0.715, 0.115)), Vec2::new(62.0, 22.0));
        paint_trigger_bar(
            p,
            lt_rect,
            spec.lt_label,
            phys.map_or(0.0, |s| normalize_trigger_axis(s.left_trigger)),
        );
        paint_trigger_bar(
            p,
            rt_rect,
            spec.rt_label,
            phys.map_or(0.0, |s| normalize_trigger_axis(s.right_trigger)),
        );

        // Smooth symmetric Bezier body silhouette + subtle contour seams
        paint_controller_silhouette(p, stage, family);

        // Shoulders (sleek rounded horizontal bars hugging the upper crown)
        let lb_rect = Rect::from_center_size(pt(Pos2::new(0.285, 0.225)), Vec2::new(84.0, 17.0));
        let rb_rect = Rect::from_center_size(pt(Pos2::new(0.715, 0.225)), Vec2::new(84.0, 17.0));
        draw_bar_button(
            ui,
            lb_rect,
            8.0,
            Some(spec.lb_label),
            None,
            LogicalButton::LeftShoulder,
            profile,
            phys,
            selected_button,
        );
        draw_bar_button(
            ui,
            rb_rect,
            8.0,
            Some(spec.rb_label),
            None,
            LogicalButton::RightShoulder,
            profile,
            phys,
            selected_button,
        );

        // Analog Sticks (ringed outer well + inner thumb cap + center/deflected dot)
        draw_stick_assembly(
            ui,
            pt(spec.left_stick_rel),
            24.5,
            LogicalButton::LeftStick,
            phys.map_or(0.0, |s| normalize_stick_axis(s.left_x)),
            phys.map_or(0.0, |s| normalize_stick_axis(s.left_y)),
            profile,
            phys,
            selected_button,
        );
        draw_stick_assembly(
            ui,
            pt(spec.right_stick_rel),
            24.5,
            LogicalButton::RightStick,
            phys.map_or(0.0, |s| normalize_stick_axis(s.right_x)),
            phys.map_or(0.0, |s| normalize_stick_axis(s.right_y)),
            profile,
            phys,
            selected_button,
        );

        // Unified Plus-Shaped D-Pad
        draw_plus_dpad(ui, pt(spec.dpad_rel), profile, phys, selected_button);

        // Uniform Circular Face Buttons ([North, West, East, South])
        let fc = pt(spec.face_center_rel);
        let d = 23.0_f32;
        let face_defs = [
            (LogicalButton::North, Pos2::new(fc.x, fc.y - d), spec.face_glyphs[0]),
            (LogicalButton::West, Pos2::new(fc.x - d, fc.y), spec.face_glyphs[1]),
            (LogicalButton::East, Pos2::new(fc.x + d, fc.y), spec.face_glyphs[2]),
            (LogicalButton::South, Pos2::new(fc.x, fc.y + d), spec.face_glyphs[3]),
        ];
        for (btn, pos, glyph) in face_defs {
            draw_face_circle(ui, pos, 11.8, btn, glyph, profile, phys, selected_button);
        }

        // Center Cluster (real vector symbols — zero cramped abbreviations)
        match family {
            ControllerFamily::Sony => {
                let tp_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.375)), Vec2::new(98.0, 38.0));
                draw_bar_button(
                    ui,
                    tp_rect,
                    6.0,
                    None,
                    Some(CenterVisual::TouchpadBar),
                    LogicalButton::Touchpad,
                    profile,
                    phys,
                    selected_button,
                );
                let cr_rect =
                    Rect::from_center_size(pt(Pos2::new(0.365, 0.315)), Vec2::new(13.0, 22.0));
                let op_rect =
                    Rect::from_center_size(pt(Pos2::new(0.635, 0.315)), Vec2::new(13.0, 22.0));
                draw_bar_button(
                    ui,
                    cr_rect,
                    5.0,
                    None,
                    Some(CenterVisual::CreateRays),
                    LogicalButton::Back,
                    profile,
                    phys,
                    selected_button,
                );
                draw_bar_button(
                    ui,
                    op_rect,
                    5.0,
                    None,
                    Some(CenterVisual::OptionsLines),
                    LogicalButton::Start,
                    profile,
                    phys,
                    selected_button,
                );
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.50, 0.515)),
                    11.5,
                    LogicalButton::Guide,
                    CenterVisual::PsLogo,
                    profile,
                    phys,
                    selected_button,
                );
                let mic_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.62)), Vec2::new(26.0, 12.0));
                draw_bar_button(
                    ui,
                    mic_rect,
                    4.0,
                    None,
                    Some(CenterVisual::MicMute),
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
            }
            ControllerFamily::Nintendo => {
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.43, 0.39)),
                    9.8,
                    LogicalButton::Back,
                    CenterVisual::MinusBar,
                    profile,
                    phys,
                    selected_button,
                );
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.57, 0.39)),
                    9.8,
                    LogicalButton::Start,
                    CenterVisual::PlusMark,
                    profile,
                    phys,
                    selected_button,
                );
                let cap_rect =
                    Rect::from_center_size(pt(Pos2::new(0.44, 0.505)), Vec2::new(19.0, 19.0));
                draw_bar_button(
                    ui,
                    cap_rect,
                    4.0,
                    None,
                    Some(CenterVisual::CaptureSquare),
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.56, 0.505)),
                    10.5,
                    LogicalButton::Guide,
                    CenterVisual::HomeHouse,
                    profile,
                    phys,
                    selected_button,
                );
            }
            ControllerFamily::Xbox | ControllerFamily::Generic => {
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.50, 0.35)),
                    12.5,
                    LogicalButton::Guide,
                    CenterVisual::XboxLogo,
                    profile,
                    phys,
                    selected_button,
                );
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.43, 0.445)),
                    9.8,
                    LogicalButton::Back,
                    CenterVisual::ViewBoxes,
                    profile,
                    phys,
                    selected_button,
                );
                draw_center_circle(
                    ui,
                    pt(Pos2::new(0.57, 0.445)),
                    9.8,
                    LogicalButton::Start,
                    CenterVisual::MenuLines,
                    profile,
                    phys,
                    selected_button,
                );
                let shr_rect =
                    Rect::from_center_size(pt(Pos2::new(0.50, 0.535)), Vec2::new(26.0, 14.0));
                draw_bar_button(
                    ui,
                    shr_rect,
                    5.0,
                    None,
                    Some(CenterVisual::ShareTray),
                    LogicalButton::Misc1,
                    profile,
                    phys,
                    selected_button,
                );
            }
        }

        // Rear Paddles Strip (neatly aligned along bottom bar)
        let paddles = [
            (Pos2::new(0.22, 0.92), "L4", LogicalButton::LeftPaddle1),
            (Pos2::new(0.32, 0.92), "L5", LogicalButton::LeftPaddle2),
            (Pos2::new(0.68, 0.92), "R5", LogicalButton::RightPaddle2),
            (Pos2::new(0.78, 0.92), "R4", LogicalButton::RightPaddle1),
        ];
        for (rel, lbl, btn) in paddles {
            let r = Rect::from_center_size(pt(rel), Vec2::new(34.0, 17.0));
            draw_bar_button(ui, r, 5.0, Some(lbl), None, btn, profile, phys, selected_button);
        }

        let p = ui.painter();
        p.text(
            pt(Pos2::new(0.50, 0.92)),
            Align2::CENTER_CENTER,
            "Click part to select",
            FontId::new(10.0, FontFamily::Proportional),
            TEXT_MUTED,
        );
    }

    canvas
}

#[derive(Clone, Copy)]
struct CubicSeg {
    c1: [f32; 2],
    c2: [f32; 2],
    end: [f32; 2],
}

fn eval_cubic(p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let uu = u * u;
    let tt = t * t;
    let uuu = uu * u;
    let ttt = tt * t;
    [
        uuu * p0[0] + 3.0 * uu * t * c1[0] + 3.0 * u * tt * c2[0] + ttt * p3[0],
        uuu * p0[1] + 3.0 * uu * t * c1[1] + 3.0 * u * tt * c2[1] + ttt * p3[1],
    ]
}

fn build_symmetric_bezier_outline(
    stage: Rect,
    top_center_y: f32,
    right_segs: &[CubicSeg],
    steps_per_seg: usize,
) -> Vec<Pos2> {
    let mut right_half: Vec<[f32; 2]> = Vec::with_capacity(right_segs.len() * steps_per_seg + 2);
    let mut cur = [0.50, top_center_y];
    right_half.push(cur);
    for seg in right_segs {
        for s in 1..=steps_per_seg {
            let t = s as f32 / steps_per_seg as f32;
            right_half.push(eval_cubic(cur, seg.c1, seg.c2, seg.end, t));
        }
        cur = seg.end;
    }

    let mut poly: Vec<Pos2> = Vec::with_capacity(right_half.len() * 2);
    for &[nx, ny] in &right_half {
        poly.push(Pos2::new(
            stage.left() + nx * stage.width(),
            stage.top() + ny * stage.height(),
        ));
    }
    // Mirror left half across x = 0.50 excluding duplicate bottom & top center endpoints
    for &[nx, ny] in right_half.iter().rev().skip(1) {
        let lx = 1.0 - nx;
        if (lx - 0.50).abs() > 1e-4 || (ny - top_center_y).abs() > 1e-4 {
            poly.push(Pos2::new(
                stage.left() + lx * stage.width(),
                stage.top() + ny * stage.height(),
            ));
        }
    }
    poly
}

fn family_body_polygon(stage: Rect, family: ControllerFamily) -> Vec<Pos2> {
    match family {
        ControllerFamily::Sony => {
            // DualSense: broad flat bridge, flared tapered handles, pronounced concave grip arch
            build_symmetric_bezier_outline(
                stage,
                0.245,
                &[
                    CubicSeg { c1: [0.60, 0.245], c2: [0.69, 0.240], end: [0.76, 0.258] }, // crown -> shoulder
                    CubicSeg { c1: [0.83, 0.278], c2: [0.875, 0.375], end: [0.892, 0.525] }, // outer palm
                    CubicSeg { c1: [0.910, 0.675], c2: [0.900, 0.795], end: [0.855, 0.825] }, // outer horn down
                    CubicSeg { c1: [0.810, 0.852], c2: [0.752, 0.828], end: [0.728, 0.742] }, // grip tip curve
                    CubicSeg { c1: [0.706, 0.668], c2: [0.680, 0.650], end: [0.640, 0.650] }, // inner arch
                    CubicSeg { c1: [0.595, 0.650], c2: [0.550, 0.654], end: [0.500, 0.654] }, // bottom center
                ],
                8,
            )
        }
        ControllerFamily::Nintendo => {
            // Switch Pro: smooth compact upper crown + gentle swept rounded handles
            build_symmetric_bezier_outline(
                stage,
                0.248,
                &[
                    CubicSeg { c1: [0.59, 0.248], c2: [0.68, 0.246], end: [0.755, 0.272] },
                    CubicSeg { c1: [0.825, 0.298], c2: [0.870, 0.400], end: [0.882, 0.545] },
                    CubicSeg { c1: [0.895, 0.675], c2: [0.875, 0.775], end: [0.828, 0.802] },
                    CubicSeg { c1: [0.782, 0.826], c2: [0.730, 0.785], end: [0.698, 0.715] },
                    CubicSeg { c1: [0.668, 0.652], c2: [0.630, 0.645], end: [0.585, 0.645] },
                    CubicSeg { c1: [0.548, 0.645], c2: [0.520, 0.646], end: [0.500, 0.646] },
                ],
                8,
            )
        }
        ControllerFamily::Xbox | ControllerFamily::Generic => {
            // Xbox: sculpted center dome + wide muscular palm sweep + ergonomic handles
            build_symmetric_bezier_outline(
                stage,
                0.242,
                &[
                    CubicSeg { c1: [0.585, 0.242], c2: [0.675, 0.252], end: [0.752, 0.270] },
                    CubicSeg { c1: [0.826, 0.288], c2: [0.878, 0.390], end: [0.892, 0.535] },
                    CubicSeg { c1: [0.905, 0.670], c2: [0.888, 0.778], end: [0.838, 0.808] },
                    CubicSeg { c1: [0.788, 0.838], c2: [0.735, 0.798], end: [0.702, 0.725] },
                    CubicSeg { c1: [0.668, 0.652], c2: [0.625, 0.642], end: [0.578, 0.642] },
                    CubicSeg { c1: [0.545, 0.642], c2: [0.520, 0.644], end: [0.500, 0.644] },
                ],
                8,
            )
        }
    }
}

fn ear_clip_polygon(pts: &[Pos2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let signed_area = (0..n).fold(0.0_f32, |acc, i| {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        acc + (a.x * b.y - b.x * a.y)
    });
    let ccw_sign = if signed_area >= 0.0 { 1.0_f32 } else { -1.0_f32 };

    let mut active: Vec<usize> = (0..n).collect();
    let mut tris = Vec::with_capacity(n.saturating_sub(2));

    let cross = |o: Pos2, a: Pos2, b: Pos2| -> f32 {
        ((a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)) * ccw_sign
    };
    let in_tri = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| -> bool {
        let c1 = cross(a, b, p);
        let c2 = cross(b, c, p);
        let c3 = cross(c, a, p);
        c1 >= -1e-4 && c2 >= -1e-4 && c3 >= -1e-4
    };

    let mut guard = 0usize;
    let max_iters = n * n + 8;
    while active.len() > 3 && guard < max_iters {
        guard += 1;
        let m = active.len();
        let mut clipped = false;
        for i in 0..m {
            let prev = active[(i + m - 1) % m];
            let curr = active[i];
            let next = active[(i + 1) % m];
            if cross(pts[prev], pts[curr], pts[next]) <= 1e-4 {
                continue;
            }
            let contains_other = active.iter().any(|&k| {
                k != prev && k != curr && k != next && in_tri(pts[k], pts[prev], pts[curr], pts[next])
            });
            if !contains_other {
                tris.push([prev, curr, next]);
                active.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if active.len() == 3 {
        tris.push([active[0], active[1], active[2]]);
    }
    tris
}

fn paint_controller_silhouette(p: &egui::Painter, stage: Rect, family: ControllerFamily) {
    let outline = family_body_polygon(stage, family);
    let tris = ear_clip_polygon(&outline);

    let body_fill = Color32::from_rgb(19, 19, 31);
    let mut mesh = Mesh::default();
    for &pos in &outline {
        mesh.colored_vertex(pos, body_fill);
    }
    for [a, b, c] in tris {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    p.add(Shape::mesh(mesh));

    // Soft outer pastel-blue hairline rim + crisp shell stroke
    p.add(Shape::Path(PathShape::closed_line(
        outline.clone(),
        Stroke::new(2.8_f32, Color32::from_rgba_unmultiplied(LOGO_BLUE.r(), LOGO_BLUE.g(), LOGO_BLUE.b(), 26)),
    )));
    p.add(Shape::Path(PathShape::closed_line(
        outline,
        Stroke::new(1.3_f32, Color32::from_rgb(56, 56, 86)),
    )));

    // Subtle interior grip seam curves matching real controller tooling
    let seam_col = Color32::from_rgb(32, 32, 50);
    let sx = |nx: f32| stage.left() + nx * stage.width();
    let sy = |ny: f32| stage.top() + ny * stage.height();
    for side in [1.0_f32, -1.0_f32] {
        let map_x = |dx: f32| if side > 0.0 { sx(0.5 + dx) } else { sx(0.5 - dx) };
        let pts: Vec<Pos2> = (0..=8)
            .map(|i| {
                let t = i as f32 / 8.0;
                let c = eval_cubic(
                    [0.245, 0.275],
                    [0.232, 0.43],
                    [0.245, 0.60],
                    [0.295, 0.74],
                    t,
                );
                Pos2::new(map_x(c[0]), sy(c[1]))
            })
            .collect();
        p.add(Shape::line(pts, Stroke::new(1.0_f32, seam_col)));
    }
}

fn paint_trigger_bar(p: &egui::Painter, rect: Rect, label: &str, value: f32) {
    let pressed = value > 0.05;
    let border = if pressed { LOGO_PINK } else { Color32::from_rgb(48, 48, 74) };
    if pressed {
        p.rect_stroke(
            rect.expand(1.5),
            Rounding::same(8.5),
            Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(LOGO_PINK.r(), LOGO_PINK.g(), LOGO_PINK.b(), 64)),
        );
    }
    p.rect_filled(rect, Rounding::same(7.0), Color32::from_rgb(20, 20, 34));
    p.rect_stroke(
        rect,
        Rounding::same(7.0),
        Stroke::new(if pressed { 1.6_f32 } else { 1.1_f32 }, border),
    );

    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + 5.0, rect.bottom() - 6.0),
        Pos2::new(rect.right() - 5.0, rect.bottom() - 2.5),
    );
    p.rect_filled(bar, Rounding::same(1.5), BG_NEAR_BLACK);
    if value > 0.01 {
        let fw = (bar.width() * value.clamp(0.0, 1.0)).max(2.0);
        p.rect_filled(
            Rect::from_min_size(bar.min, Vec2::new(fw, bar.height())),
            Rounding::same(1.5),
            LOGO_PINK,
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
        Color32::from_rgb(42, 24, 38)
    } else if selected {
        Color32::from_rgb(20, 32, 46)
    } else {
        Color32::from_rgb(24, 24, 40)
    };
    let stroke = if pressed {
        Stroke::new(1.8_f32, LOGO_PINK)
    } else if selected {
        Stroke::new(1.8_f32, LOGO_BLUE)
    } else if remapped {
        Stroke::new(1.2_f32, LOGO_LAVENDER)
    } else {
        Stroke::new(1.1_f32, Color32::from_rgb(54, 54, 82))
    };
    (pressed, selected, remapped, fill, stroke)
}

fn paint_selection_glow_rect(p: &egui::Painter, rect: Rect, rounding: f32, color: Color32) {
    for (exp, w, alpha) in [(3.2_f32, 2.0_f32, 26_u8), (1.8_f32, 2.0_f32, 58_u8)] {
        p.rect_stroke(
            rect.expand(exp),
            Rounding::same(rounding + exp),
            Stroke::new(w, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)),
        );
    }
}

fn paint_selection_glow_circle(p: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    for (exp, w, alpha) in [(3.4_f32, 2.2_f32, 28_u8), (1.8_f32, 2.0_f32, 62_u8)] {
        p.circle_stroke(
            center,
            radius + exp,
            Stroke::new(w, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)),
        );
    }
}

fn paint_center_icon(p: &egui::Painter, center: Pos2, rect: Rect, icon: CenterVisual, color: Color32) {
    let st = Stroke::new(1.4_f32, color);
    match icon {
        CenterVisual::XboxLogo => {
            // Two diagonal swooshes forming the Xbox emblem
            let r = rect.height() * 0.27;
            p.line_segment(
                [Pos2::new(center.x - r, center.y - r * 0.85), Pos2::new(center.x + r, center.y + r * 0.85)],
                Stroke::new(1.8_f32, color),
            );
            p.line_segment(
                [Pos2::new(center.x + r, center.y - r * 0.85), Pos2::new(center.x - r, center.y + r * 0.85)],
                Stroke::new(1.8_f32, color),
            );
        }
        CenterVisual::ViewBoxes => {
            // Overlapping View squares
            let back = Rect::from_min_size(Pos2::new(center.x - 4.5, center.y - 4.5), Vec2::splat(6.0));
            let front = Rect::from_min_size(Pos2::new(center.x - 1.5, center.y - 1.5), Vec2::splat(6.0));
            p.rect_stroke(back, Rounding::same(0.5), Stroke::new(1.1_f32, color));
            p.rect_filled(front, Rounding::same(0.5), Color32::from_rgb(24, 24, 40));
            p.rect_stroke(front, Rounding::same(0.5), Stroke::new(1.1_f32, color));
        }
        CenterVisual::MenuLines => {
            for dy in [-3.2_f32, 0.0, 3.2] {
                p.line_segment(
                    [Pos2::new(center.x - 4.5, center.y + dy), Pos2::new(center.x + 4.5, center.y + dy)],
                    Stroke::new(1.3_f32, color),
                );
            }
        }
        CenterVisual::ShareTray => {
            p.line_segment(
                [Pos2::new(center.x - 4.5, center.y + 3.0), Pos2::new(center.x + 4.5, center.y + 3.0)],
                st,
            );
            p.line_segment(
                [Pos2::new(center.x, center.y + 1.5), Pos2::new(center.x, center.y - 4.0)],
                st,
            );
            p.line_segment(
                [Pos2::new(center.x - 3.0, center.y - 1.2), Pos2::new(center.x, center.y - 4.2)],
                st,
            );
            p.line_segment(
                [Pos2::new(center.x + 3.0, center.y - 1.2), Pos2::new(center.x, center.y - 4.2)],
                st,
            );
        }
        CenterVisual::TouchpadBar => {
            let light = Rect::from_min_max(
                Pos2::new(rect.left() + 8.0, rect.top() + 3.5),
                Pos2::new(rect.right() - 8.0, rect.top() + 5.5),
            );
            p.rect_filled(light, Rounding::same(1.0), LOGO_BLUE);
            p.text(
                Pos2::new(center.x, center.y + 3.0),
                Align2::CENTER_CENTER,
                "Touchpad",
                FontId::new(10.0, FontFamily::Proportional),
                color,
            );
        }
        CenterVisual::CreateRays => {
            for dx in [-2.6_f32, 0.0, 2.6] {
                p.line_segment(
                    [Pos2::new(center.x + dx, center.y + 5.0), Pos2::new(center.x + dx * 1.4, center.y - 5.0)],
                    Stroke::new(1.1_f32, color),
                );
            }
        }
        CenterVisual::OptionsLines => {
            for dy in [-3.5_f32, 0.0, 3.5] {
                p.line_segment(
                    [Pos2::new(center.x - 3.5, center.y + dy), Pos2::new(center.x + 3.5, center.y + dy)],
                    Stroke::new(1.2_f32, color),
                );
            }
        }
        CenterVisual::PsLogo => {
            p.text(
                center,
                Align2::CENTER_CENTER,
                "PS",
                FontId::new(10.0, FontFamily::Proportional),
                color,
            );
        }
        CenterVisual::MicMute => {
            p.line_segment(
                [Pos2::new(center.x, center.y - 3.2), Pos2::new(center.x, center.y + 1.2)],
                Stroke::new(1.6_f32, color),
            );
            p.line_segment(
                [Pos2::new(center.x - 5.5, center.y + 3.0), Pos2::new(center.x + 5.5, center.y - 3.0)],
                Stroke::new(1.1_f32, LOGO_PINK),
            );
        }
        CenterVisual::MinusBar => {
            p.line_segment(
                [Pos2::new(center.x - 4.2, center.y), Pos2::new(center.x + 4.2, center.y)],
                Stroke::new(1.8_f32, color),
            );
        }
        CenterVisual::PlusMark => {
            p.line_segment(
                [Pos2::new(center.x - 4.2, center.y), Pos2::new(center.x + 4.2, center.y)],
                Stroke::new(1.8_f32, color),
            );
            p.line_segment(
                [Pos2::new(center.x, center.y - 4.2), Pos2::new(center.x, center.y + 4.2)],
                Stroke::new(1.8_f32, color),
            );
        }
        CenterVisual::CaptureSquare => {
            p.circle_filled(center, 4.2, color);
        }
        CenterVisual::HomeHouse => {
            let pts = vec![
                Pos2::new(center.x, center.y - 4.5),
                Pos2::new(center.x + 4.8, center.y - 0.5),
                Pos2::new(center.x + 3.5, center.y + 4.2),
                Pos2::new(center.x - 3.5, center.y + 4.2),
                Pos2::new(center.x - 4.8, center.y - 0.5),
            ];
            p.add(Shape::Path(PathShape::closed_line(pts, Stroke::new(1.3_f32, color))));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bar_button(
    ui: &mut egui::Ui,
    rect: Rect,
    rounding: f32,
    label: Option<&str>,
    icon: Option<CenterVisual>,
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
        paint_selection_glow_rect(p, rect, rounding, if pressed { LOGO_PINK } else { LOGO_BLUE });
    }
    p.rect_filled(rect, Rounding::same(rounding), fill);
    p.rect_stroke(rect, Rounding::same(rounding), stroke);

    let fg = if pressed || selected { TEXT_PRIMARY } else { TEXT_MUTED };
    if let Some(txt) = label {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            txt,
            FontId::new(10.5, FontFamily::Proportional),
            fg,
        );
    }
    if let Some(vis) = icon {
        paint_center_icon(p, rect.center(), rect, vis, fg);
    }
    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(rect.right() - 4.0, rect.top() + 4.0), 2.0, LOGO_LAVENDER);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_center_circle(
    ui: &mut egui::Ui,
    center: Pos2,
    radius: f32,
    btn: LogicalButton,
    icon: CenterVisual,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    let resp = ui.interact(rect, ui.id().with(("diag_center", btn)), Sense::click());
    if resp.clicked() {
        *selected_button = Some(btn);
    }
    let (pressed, selected, remapped, fill, stroke) =
        button_visuals(btn, profile, phys, *selected_button);
    let p = ui.painter();
    if pressed || selected {
        paint_selection_glow_circle(p, center, radius, if pressed { LOGO_PINK } else { LOGO_BLUE });
    }
    p.circle_filled(center, radius, fill);
    p.circle_stroke(center, radius, stroke);
    let fg = if pressed || selected { TEXT_PRIMARY } else { TEXT_MUTED };
    paint_center_icon(p, center, rect, icon, fg);
    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(center.x + radius * 0.65, center.y - radius * 0.65), 2.0, LOGO_LAVENDER);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_face_circle(
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
        paint_selection_glow_circle(p, center, radius, if pressed { LOGO_PINK } else { LOGO_BLUE });
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
                FontId::new(11.0, FontFamily::Proportional),
                fg,
            );
        }
        FaceGlyph::Triangle => {
            let r = radius * 0.48;
            let pts = vec![
                Pos2::new(center.x, center.y - r),
                Pos2::new(center.x - r * 0.9, center.y + r * 0.72),
                Pos2::new(center.x + r * 0.9, center.y + r * 0.72),
            ];
            p.add(Shape::Path(PathShape::closed_line(
                pts,
                Stroke::new(1.5_f32, if pressed { LOGO_PINK } else { fg }),
            )));
        }
        FaceGlyph::Square => {
            let sq = Rect::from_center_size(center, Vec2::splat(radius * 0.88));
            p.rect_stroke(
                sq,
                Rounding::same(1.0),
                Stroke::new(1.5_f32, if pressed { LOGO_PINK } else { fg }),
            );
        }
        FaceGlyph::Circle => {
            p.circle_stroke(
                center,
                radius * 0.46,
                Stroke::new(1.5_f32, if pressed { LOGO_PINK } else { fg }),
            );
        }
        FaceGlyph::Cross => {
            let r = radius * 0.42;
            let st = Stroke::new(1.6_f32, if pressed { LOGO_PINK } else { fg });
            p.line_segment([Pos2::new(center.x - r, center.y - r), Pos2::new(center.x + r, center.y + r)], st);
            p.line_segment([Pos2::new(center.x + r, center.y - r), Pos2::new(center.x - r, center.y + r)], st);
        }
    }

    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(center.x + radius * 0.65, center.y - radius * 0.65), 2.2, LOGO_LAVENDER);
    }
}

fn draw_plus_dpad(
    ui: &mut egui::Ui,
    center: Pos2,
    profile: &Profile,
    phys: Option<&PhysicalSnapshot>,
    selected_button: &mut Option<LogicalButton>,
) {
    let hw = 7.2_f32;
    let arm_len = 21.5_f32;
    let c = center;

    // 12-vertex unified plus (+) outline
    let plus_pts = vec![
        Pos2::new(c.x - hw, c.y - arm_len),
        Pos2::new(c.x + hw, c.y - arm_len),
        Pos2::new(c.x + hw, c.y - hw),
        Pos2::new(c.x + arm_len, c.y - hw),
        Pos2::new(c.x + arm_len, c.y + hw),
        Pos2::new(c.x + hw, c.y + hw),
        Pos2::new(c.x + hw, c.y + arm_len),
        Pos2::new(c.x - hw, c.y + arm_len),
        Pos2::new(c.x - hw, c.y + hw),
        Pos2::new(c.x - arm_len, c.y + hw),
        Pos2::new(c.x - arm_len, c.y - hw),
        Pos2::new(c.x - hw, c.y - hw),
    ];

    {
        let p = ui.painter();
        let tris = ear_clip_polygon(&plus_pts);
        let base_fill = Color32::from_rgb(24, 24, 40);
        let mut mesh = Mesh::default();
        for &pos in &plus_pts {
            mesh.colored_vertex(pos, base_fill);
        }
        for [a, b, d] in tris {
            mesh.add_triangle(a as u32, b as u32, d as u32);
        }
        p.add(Shape::mesh(mesh));
        p.add(Shape::Path(PathShape::closed_line(
            plus_pts,
            Stroke::new(1.2_f32, Color32::from_rgb(56, 56, 84)),
        )));
    }

    // 4 interactive directional arm caps + directional chevron triangles
    let arm_defs = [
        (
            LogicalButton::DpadUp,
            Rect::from_min_max(Pos2::new(c.x - hw + 1.0, c.y - arm_len + 1.0), Pos2::new(c.x + hw - 1.0, c.y - hw + 1.0)),
            [
                Pos2::new(c.x, c.y - arm_len + 4.5),
                Pos2::new(c.x - 3.6, c.y - arm_len + 10.0),
                Pos2::new(c.x + 3.6, c.y - arm_len + 10.0),
            ],
        ),
        (
            LogicalButton::DpadDown,
            Rect::from_min_max(Pos2::new(c.x - hw + 1.0, c.y + hw - 1.0), Pos2::new(c.x + hw - 1.0, c.y + arm_len - 1.0)),
            [
                Pos2::new(c.x, c.y + arm_len - 4.5),
                Pos2::new(c.x - 3.6, c.y + arm_len - 10.0),
                Pos2::new(c.x + 3.6, c.y + arm_len - 10.0),
            ],
        ),
        (
            LogicalButton::DpadLeft,
            Rect::from_min_max(Pos2::new(c.x - arm_len + 1.0, c.y - hw + 1.0), Pos2::new(c.x - hw + 1.0, c.y + hw - 1.0)),
            [
                Pos2::new(c.x - arm_len + 4.5, c.y),
                Pos2::new(c.x - arm_len + 10.0, c.y - 3.6),
                Pos2::new(c.x - arm_len + 10.0, c.y + 3.6),
            ],
        ),
        (
            LogicalButton::DpadRight,
            Rect::from_min_max(Pos2::new(c.x + hw - 1.0, c.y - hw + 1.0), Pos2::new(c.x + arm_len - 1.0, c.y + hw - 1.0)),
            [
                Pos2::new(c.x + arm_len - 4.5, c.y),
                Pos2::new(c.x + arm_len - 10.0, c.y - 3.6),
                Pos2::new(c.x + arm_len - 10.0, c.y + 3.6),
            ],
        ),
    ];

    for (btn, rect, chevron) in arm_defs {
        let resp = ui.interact(rect, ui.id().with(("diag_dpad", btn)), Sense::click());
        if resp.clicked() {
            *selected_button = Some(btn);
        }
        let (pressed, selected, remapped, fill, stroke) =
            button_visuals(btn, profile, phys, *selected_button);
        let p = ui.painter();
        if pressed || selected {
            paint_selection_glow_rect(p, rect, 3.0, if pressed { LOGO_PINK } else { LOGO_BLUE });
            p.rect_filled(rect, Rounding::same(2.5), fill);
            p.rect_stroke(rect, Rounding::same(2.5), stroke);
        } else if remapped {
            p.rect_stroke(rect, Rounding::same(2.5), stroke);
        }
        let tri_col = if pressed || selected { TEXT_PRIMARY } else { TEXT_MUTED };
        p.add(Shape::convex_polygon(chevron.to_vec(), tri_col, Stroke::NONE));
    }

    // Center concave pivot ring
    let p = ui.painter();
    p.circle_filled(c, 4.8, BG_NEAR_BLACK);
    p.circle_stroke(c, 4.8, Stroke::new(1.0_f32, Color32::from_rgb(44, 44, 68)));
}

#[allow(clippy::too_many_arguments)]
fn draw_stick_assembly(
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
        paint_selection_glow_circle(p, center, radius, if pressed { LOGO_PINK } else { LOGO_BLUE });
    }
    // Outer gate well + inner ringed thumbstick cap
    p.circle_filled(center, radius, BG_NEAR_BLACK);
    p.circle_stroke(center, radius, stroke);
    p.circle_filled(center, radius * 0.68, Color32::from_rgb(20, 20, 34));
    p.circle_stroke(
        center,
        radius * 0.68,
        Stroke::new(1.0_f32, Color32::from_rgb(46, 46, 72)),
    );

    let mag = (x * x + y * y).sqrt();
    let travel = radius - 6.0_f32;
    let clamped = if mag > 1.0 { 1.0 / mag } else { 1.0 };
    let dot = Pos2::new(center.x + x * clamped * travel, center.y + y * clamped * travel);
    if mag > 0.08 {
        p.circle_filled(
            dot,
            7.5,
            Color32::from_rgba_unmultiplied(LOGO_PINK.r(), LOGO_PINK.g(), LOGO_PINK.b(), 68),
        );
        p.circle_filled(dot, 4.2, LOGO_PINK);
    } else {
        p.circle_filled(dot, 3.6, if pressed { LOGO_PINK } else { LOGO_BLUE });
    }
    if remapped && !pressed && !selected {
        p.circle_filled(Pos2::new(center.x + radius - 3.0, center.y - radius + 3.0), 2.2, LOGO_LAVENDER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_body_silhouettes_triangulate_cleanly_for_all_families() {
        let stage = Rect::from_min_size(Pos2::ZERO, Vec2::new(480.0, 300.0));
        for fam in [
            ControllerFamily::Xbox,
            ControllerFamily::Sony,
            ControllerFamily::Nintendo,
        ] {
            let poly = family_body_polygon(stage, fam);
            assert!(poly.len() > 60, "expected smooth Bezier contour points");
            let tris = ear_clip_polygon(&poly);
            assert_eq!(
                tris.len(),
                poly.len() - 2,
                "expected complete ear-clipped triangulation for {fam:?}"
            );
        }
    }

    #[test]
    fn family_layouts_distinguish_xbox_sony_and_nintendo() {
        let xbox = family_spec(ControllerFamily::Xbox);
        let sony = family_spec(ControllerFamily::Sony);
        let nintendo = family_spec(ControllerFamily::Nintendo);

        assert!(xbox.left_stick_rel.y < xbox.dpad_rel.y);
        assert!(nintendo.left_stick_rel.y < nintendo.dpad_rel.y);
        assert!(sony.dpad_rel.y < sony.left_stick_rel.y);

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
                    let rect = draw_controller_diagram(ui, fam, &prof, Some(&phys), &mut sel);
                    assert!(rect.width() > 300.0 && rect.height() > 250.0);
                });
            });
            assert!(!ctx.tessellate(out.shapes, 1.0).is_empty());
        }
    }
}
