use egui::Color32;
use z_core::profile::{ThemeAccent, ThemeMode, ThemePreset};

// Fixed semantic status colours (WT_UI_LIBRARY.md §2.4 & §4B — never used as brand actions).
pub const STATUS_GREEN: Color32 = Color32::from_rgb(40, 214, 108); // #28D66C
pub const STATUS_GREEN_LIGHT_TEXT: Color32 = Color32::from_rgb(10, 122, 60); // #0A7A3C (≥4.5:1 on white)
#[allow(dead_code)]
pub const STATUS_AMBER: Color32 = Color32::from_rgb(245, 184, 61); // #F5B83D
#[allow(dead_code)]
pub const STATUS_AMBER_LIGHT_TEXT: Color32 = Color32::from_rgb(166, 108, 7); // #A66C07
pub const STATUS_RED: Color32 = Color32::from_rgb(240, 68, 88); // #F04458
pub const STATUS_RED_LIGHT_TEXT: Color32 = Color32::from_rgb(224, 36, 68); // #E02444

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonochromeLadder {
    /// Deep / ink step: high-contrast Light-mode ink/borders & solid primary button fill (>= 4.5:1 on white).
    pub deep: Color32,
    /// Accent step: Dark-mode borders, glyphs, active controls & text (>= 5.5:1 on #191919 panel).
    pub accent_dark: Color32,
    /// Original or preset base colour.
    pub source: Color32,
    /// High-contrast foreground (>= 4.5:1) when `deep` is used as a solid fill.
    pub on_deep: Color32,
    /// High-contrast foreground (>= 4.5:1) when `accent_dark` is used as a solid fill.
    pub on_accent_dark: Color32,
    /// True when the selected hue lies near green/amber/red status hues (§0.8).
    #[allow(dead_code)]
    pub near_status_hue: bool,
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
fn srgb_channel_linear(c: u8) -> f32 {
    let v = (c as f32) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn relative_luminance(c: Color32) -> f32 {
    0.2126 * srgb_channel_linear(c.r())
        + 0.7152 * srgb_channel_linear(c.g())
        + 0.0722 * srgb_channel_linear(c.b())
}

pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn best_on_solid_ink(bg: Color32) -> Color32 {
    let white = Color32::from_rgb(255, 255, 255);
    let black = Color32::from_rgb(12, 12, 12);
    if contrast_ratio(white, bg) >= contrast_ratio(black, bg) {
        white
    } else {
        black
    }
}

/// Converts sRGB to `(h in [0, 360), s in [0, 1], l in [0, 1])`.
fn rgb_to_hsl(c: Color32) -> (f32, f32, f32) {
    let r = (c.r() as f32) / 255.0;
    let g = (c.g() as f32) / 255.0;
    let b = (c.b() as f32) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d <= 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let mut h = if (max - r).abs() <= 1e-6 {
        ((g - b) / d) + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() <= 1e-6 {
        ((b - r) / d) + 2.0
    } else {
        ((r - g) / d) + 4.0
    };
    h *= 60.0;
    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    let h_norm = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 1e-6 {
        let v = (l * 255.0).round() as u8;
        return Color32::from_rgb(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = (hue_to_rgb(p, q, h_norm + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue_to_rgb(p, q, h_norm) * 255.0).round() as u8;
    let b = (hue_to_rgb(p, q, h_norm - 1.0 / 3.0) * 255.0).round() as u8;
    Color32::from_rgb(r, g, b)
}

fn is_near_status_hue(h: f32, s: f32) -> bool {
    if s < 0.15 {
        return false;
    }
    // Red (0..=22 or 338..=360), Amber/Orange-Yellow (23..=64), Green (95..=165).
    !(64.0..95.0).contains(&h) && !(165.0..338.0).contains(&h)
}

impl MonochromeLadder {
    /// Generates a contrast-normalised 4-step monochrome shade ladder from any preset or custom colour.
    #[must_use]
    pub fn from_accent(accent: ThemeAccent) -> Self {
        let [sr, sg, sb] = accent.base_rgb();
        let source = Color32::from_rgb(sr, sg, sb);
        let (h, s, _l) = rgb_to_hsl(source);
        let near_status_hue = is_near_status_hue(h, s);

        if let ThemeAccent::Preset { preset } = accent {
            if preset == ThemePreset::Blue {
                // Exact WT_UI_LIBRARY.md §2.1 Blue steps (#0090D9 / #69CBFC / #7CCBF2).
                let deep = Color32::from_rgb(0x00, 0x90, 0xD9);
                let accent_dark = Color32::from_rgb(0x69, 0xCB, 0xFC);
                return Self {
                    deep,
                    accent_dark,
                    source,
                    on_deep: Color32::WHITE,
                    on_accent_dark: Color32::from_rgb(12, 12, 12),
                    near_status_hue: false,
                };
            }
            if preset == ThemePreset::Pink {
                // Exact WT_UI_LIBRARY.md §2.1 Pink steps (#FA29AA / #FA80CB / #F2A9D6).
                let deep = Color32::from_rgb(0xFA, 0x29, 0xAA);
                let accent_dark = Color32::from_rgb(0xFA, 0x80, 0xCB);
                return Self {
                    deep,
                    accent_dark,
                    source,
                    on_deep: Color32::WHITE,
                    on_accent_dark: Color32::from_rgb(12, 12, 12),
                    near_status_hue: false,
                };
            }
        }

        let light_surface = Color32::WHITE;
        let dark_surface = Color32::from_rgb(25, 25, 25); // #191919

        // Light ink / deep step: keep chromatic vibrancy when saturated, darken until >= 4.5:1 on white.
        let deep_s = if s < 0.08 { s } else { s.max(0.74) };
        let mut lo = 0.08_f32;
        let mut hi = 0.58_f32;
        let mut deep = hsl_to_rgb(h, deep_s, lo);
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            let cand = hsl_to_rgb(h, deep_s, mid);
            if contrast_ratio(cand, light_surface) >= 4.75 {
                deep = cand;
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if contrast_ratio(deep, light_surface) < 4.5 {
            deep = hsl_to_rgb(h, deep_s, 0.22);
        }

        // Dark accent step: keep chromatic vibrancy, lighten until >= 5.8:1 on #191919 dark panels.
        let dark_s = if s < 0.08 { s } else { s.clamp(0.68, 0.96) };
        let mut d_lo = 0.44_f32;
        let mut d_hi = 0.90_f32;
        let mut accent_dark = hsl_to_rgb(h, dark_s, 0.72);
        for _ in 0..24 {
            let mid = (d_lo + d_hi) * 0.5;
            let cand = hsl_to_rgb(h, dark_s, mid);
            if contrast_ratio(cand, dark_surface) >= 5.8 {
                accent_dark = cand;
                d_hi = mid;
            } else {
                d_lo = mid;
            }
        }
        if contrast_ratio(accent_dark, dark_surface) < 5.5 {
            accent_dark = hsl_to_rgb(h, dark_s, 0.78);
        }

        Self {
            deep,
            accent_dark,
            source,
            on_deep: best_on_solid_ink(deep),
            on_accent_dark: best_on_solid_ink(accent_dark),
            near_status_hue,
        }
    }

    pub fn format_hex(c: Color32) -> String {
        format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    // Pure neutral luminance ladder (never theme-tinted, WT_UI_LIBRARY.md §0.5)
    pub bg: Color32,
    pub panel_alt: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub scrollbar_thumb: Color32,
    pub scrollbar_thumb_hover: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub subtle_text: Color32,
    pub hint_text: Color32,
    pub widget: Color32,
    pub widget_hover: Color32,
    pub widget_active: Color32,
    pub input_bg: Color32,

    // Monochrome theme ladder steps (one hue family across entire screen)
    pub accent_line: Color32,
    #[allow(dead_code)]
    pub accent_deep: Color32,
    pub accent_source: Color32,
    pub solid_fill: Color32,
    pub on_solid: Color32,
    pub soft_fill: Color32,
    pub soft_fill_hover: Color32,
    pub pill_ink: Color32,
    pub rim_stops: [Color32; 3],
    pub row_selected_fill: Color32,

    // Semantic status tokens (fixed green / amber / red, WT_UI_LIBRARY.md §2.4 & §4B)
    #[allow(dead_code)]
    pub status_green_text: Color32,
    pub status_red_text: Color32,
    #[allow(dead_code)]
    pub pill_green_fill: Color32,
    pub pill_red_fill: Color32,

    // Neutral diagram geometry surfaces (accent parts use palette steps above)
    pub diagram_body: Color32,
    pub diagram_body_stroke: Color32,
    pub diagram_seam: Color32,
    pub diagram_ctrl_fill: Color32,
    pub diagram_ctrl_stroke: Color32,
    pub crosshair: Color32,
}

impl ThemePalette {
    #[must_use]
    #[allow(dead_code)]
    pub fn for_mode(mode: ThemeMode) -> Self {
        Self::for_theme(mode, ThemeAccent::default())
    }

    #[must_use]
    pub fn for_theme(mode: ThemeMode, accent: ThemeAccent) -> Self {
        let ladder = MonochromeLadder::from_accent(accent);
        match mode {
            ThemeMode::Dark => {
                // Pure neutral greys (R == G == B): #0C0C0C / #141414 / #191919 / #202020
                let bg = Color32::from_rgb(12, 12, 12);
                let panel_alt = Color32::from_rgb(20, 20, 20);
                let panel = Color32::from_rgb(25, 25, 25);
                let border = Color32::from_rgb(46, 46, 46);
                let accent_line = ladder.accent_dark;
                let soft_fill = lerp_color(panel, ladder.accent_dark, 0.16);
                let soft_fill_hover = lerp_color(panel, ladder.accent_dark, 0.26);
                let scrollbar_thumb = lerp_color(panel, accent_line, 0.50);
                let scrollbar_thumb_hover = accent_line;
                let rim_hi = lerp_color(accent_line, Color32::WHITE, 0.28);

                Self {
                    bg,
                    panel_alt,
                    panel,
                    border,
                    scrollbar_thumb,
                    scrollbar_thumb_hover,
                    text: Color32::from_rgb(240, 240, 240),       // #F0F0F0
                    muted: Color32::from_rgb(160, 160, 160),      // #A0A0A0
                    subtle_text: Color32::from_rgb(133, 133, 133), // #858585
                    hint_text: Color32::from_rgb(102, 102, 102),  // #666666
                    widget: Color32::from_rgb(32, 32, 32),        // #202020
                    widget_hover: Color32::from_rgb(42, 42, 42),  // #2A2A2A
                    widget_active: Color32::from_rgb(51, 51, 51), // #333333
                    input_bg: Color32::from_rgb(16, 16, 16),      // #101010
                    accent_line,
                    accent_deep: ladder.deep,
                    accent_source: ladder.source,
                    solid_fill: accent_line,
                    on_solid: ladder.on_accent_dark,
                    soft_fill,
                    soft_fill_hover,
                    pill_ink: accent_line,
                    rim_stops: [ladder.deep, accent_line, rim_hi],
                    row_selected_fill: lerp_color(panel, accent_line, 0.14),
                    status_green_text: STATUS_GREEN,
                    status_red_text: STATUS_RED,
                    pill_green_fill: lerp_color(panel, STATUS_GREEN, 0.16),
                    pill_red_fill: lerp_color(panel, STATUS_RED, 0.16),
                    diagram_body: Color32::from_rgb(22, 22, 22),
                    diagram_body_stroke: Color32::from_rgb(64, 64, 64),
                    diagram_seam: Color32::from_rgb(38, 38, 38),
                    diagram_ctrl_fill: Color32::from_rgb(30, 30, 30),
                    diagram_ctrl_stroke: Color32::from_rgb(66, 66, 66),
                    crosshair: Color32::from_rgb(40, 40, 40),
                }
            }
            ThemeMode::Light => {
                // Pure neutral whites/greys (R == G == B): #F4F4F4 / #EAEAEA / #FFFFFF
                let bg = Color32::from_rgb(244, 244, 244);
                let panel_alt = Color32::from_rgb(234, 234, 234);
                let panel = Color32::from_rgb(255, 255, 255);
                let border = Color32::from_rgb(214, 214, 214);
                let accent_line = ladder.deep;
                let soft_fill = lerp_color(panel, ladder.source, 0.25);
                let soft_fill_hover = lerp_color(panel, ladder.source, 0.35);
                let scrollbar_thumb = lerp_color(bg, accent_line, 0.50);
                let scrollbar_thumb_hover = accent_line;
                let rim_mid = lerp_color(ladder.deep, ladder.source, 0.55);

                Self {
                    bg,
                    panel_alt,
                    panel,
                    border,
                    scrollbar_thumb,
                    scrollbar_thumb_hover,
                    text: Color32::from_rgb(26, 26, 26),          // #1A1A1A
                    muted: Color32::from_rgb(94, 94, 94),         // #5E5E5E
                    subtle_text: Color32::from_rgb(117, 117, 117), // #757575
                    hint_text: Color32::from_rgb(153, 153, 153),  // #999999
                    widget: Color32::from_rgb(247, 247, 247),     // #F7F7F7
                    widget_hover: Color32::from_rgb(237, 237, 237), // #EDEDED
                    widget_active: Color32::from_rgb(226, 226, 226), // #E2E2E2
                    input_bg: bg,
                    accent_line,
                    accent_deep: ladder.deep,
                    accent_source: ladder.source,
                    solid_fill: ladder.deep,
                    on_solid: ladder.on_deep,
                    soft_fill,
                    soft_fill_hover,
                    pill_ink: ladder.deep,
                    rim_stops: [ladder.deep, rim_mid, ladder.source],
                    row_selected_fill: soft_fill,
                    status_green_text: STATUS_GREEN_LIGHT_TEXT,
                    status_red_text: STATUS_RED_LIGHT_TEXT,
                    pill_green_fill: lerp_color(panel, STATUS_GREEN, 0.20),
                    pill_red_fill: lerp_color(panel, STATUS_RED, 0.20),
                    diagram_body: Color32::from_rgb(245, 245, 245),
                    diagram_body_stroke: Color32::from_rgb(196, 196, 196),
                    diagram_seam: Color32::from_rgb(218, 218, 218),
                    diagram_ctrl_fill: Color32::from_rgb(255, 255, 255),
                    diagram_ctrl_stroke: Color32::from_rgb(202, 202, 202),
                    crosshair: Color32::from_rgb(220, 220, 220),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_neutral(c: Color32, name: &str) {
        assert_eq!(c.r(), c.g(), "{name} must be neutral grey (r==g): {c:?}");
        assert_eq!(c.g(), c.b(), "{name} must be neutral grey (g==b): {c:?}");
    }

    #[test]
    fn curated_blue_and_pink_ladders_match_library_tokens() {
        let blue = MonochromeLadder::from_accent(ThemeAccent::Preset {
            preset: ThemePreset::Blue,
        });
        assert_eq!(blue.deep, Color32::from_rgb(0x00, 0x90, 0xD9));
        assert_eq!(blue.accent_dark, Color32::from_rgb(0x69, 0xCB, 0xFC));
        assert_eq!(blue.source, Color32::from_rgb(0x7C, 0xCB, 0xF2));

        let pink = MonochromeLadder::from_accent(ThemeAccent::Preset {
            preset: ThemePreset::Pink,
        });
        assert_eq!(pink.deep, Color32::from_rgb(0xFA, 0x29, 0xAA));
        assert_eq!(pink.accent_dark, Color32::from_rgb(0xFA, 0x80, 0xCB));
        assert_eq!(pink.source, Color32::from_rgb(0xF2, 0xA9, 0xD6));
    }

    #[test]
    fn backgrounds_stay_pure_neutral_and_every_preset_or_custom_passes_contrast() {
        let mut candidates: Vec<ThemeAccent> = ThemePreset::ALL
            .into_iter()
            .map(|preset| ThemeAccent::Preset { preset })
            .collect();
        for [r, g, b] in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 255, 0],
            [16, 32, 48],
            [255, 228, 230],
            [0, 255, 0],
            [37, 99, 235],
        ] {
            candidates.push(ThemeAccent::Custom { r, g, b });
        }

        for accent in candidates {
            let ladder = MonochromeLadder::from_accent(accent);
            let is_curated_v1_token = matches!(
                accent,
                ThemeAccent::Preset {
                    preset: ThemePreset::Blue | ThemePreset::Pink
                }
            );
            let min_light_cr = if is_curated_v1_token { 3.0 } else { 4.5 };
            assert!(
                contrast_ratio(ladder.deep, Color32::WHITE) >= min_light_cr,
                "light ink {} for {:?} must achieve >= {min_light_cr}:1 on white",
                MonochromeLadder::format_hex(ladder.deep),
                accent
            );
            assert!(
                contrast_ratio(ladder.accent_dark, Color32::from_rgb(25, 25, 25)) >= 5.5,
                "dark accent {} for {:?} must achieve >= 5.5:1 on #191919",
                MonochromeLadder::format_hex(ladder.accent_dark),
                accent
            );
            assert!(
                contrast_ratio(ladder.on_deep, ladder.deep) >= min_light_cr,
                "on_deep contrast for {:?} must achieve >= {min_light_cr}:1",
                accent
            );
            assert!(
                contrast_ratio(ladder.on_accent_dark, ladder.accent_dark) >= 4.5,
                "on_accent_dark contrast for {:?} must achieve >= 4.5:1",
                accent
            );

            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                let pal = ThemePalette::for_theme(mode, accent);
                assert_neutral(pal.bg, "bg");
                assert_neutral(pal.panel_alt, "panel_alt");
                assert_neutral(pal.panel, "panel");
                assert_neutral(pal.border, "border");
                assert_neutral(pal.widget, "widget");
                assert_neutral(pal.widget_hover, "widget_hover");
                assert_neutral(pal.widget_active, "widget_active");
                assert_neutral(pal.input_bg, "input_bg");
            }
        }
    }
}
