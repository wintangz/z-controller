use std::sync::OnceLock;

use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureOptions};

use crate::logo_png::{decode_png_rgba, downsample_trimmed_logo_rgba};

/// Original unmodified WT logo mark (1200x1200 RGBA PNG, transparent background).
pub const LOGO_MARK_PNG: &[u8] = include_bytes!("../../../assets/logo-mark.png");

fn decoded_logo_mark() -> &'static (usize, usize, Vec<u8>) {
    static DECODED: OnceLock<(usize, usize, Vec<u8>)> = OnceLock::new();
    DECODED.get_or_init(|| {
        decode_png_rgba(LOGO_MARK_PNG)
            .expect("embedded assets/logo-mark.png must be a valid 8-bit RGBA PNG")
    })
}

/// Downsamples the trimmed opaque artwork bounds from `assets/logo-mark.png` into a premultiplied
/// `ColorImage` sized at `target_px x target_px` matching y-storage's exact 1.04x framing.
pub fn downsample_logo_premultiplied(target_px: usize) -> Option<ColorImage> {
    let dst = target_px.clamp(16, 128);
    let (w, h, ref straight_rgba) = *decoded_logo_mark();
    let down = downsample_trimmed_logo_rgba(w, h, straight_rgba, dst);
    if down.len() != dst * dst * 4 {
        return None;
    }
    let pixels = down
        .chunks_exact(4)
        .map(|px| Color32::from_rgba_unmultiplied(px[0], px[1], px[2], px[3]))
        .collect();
    Some(ColorImage {
        size: [dst, dst],
        pixels,
    })
}

#[derive(Clone)]
struct CachedLogoTexture {
    target_px: usize,
    handle: TextureHandle,
}

/// Returns the premultiplied WT masthead logo texture sized cleanly for `logical_size * pixels_per_point`.
pub fn masthead_logo_texture(ctx: &Context, logical_size: f32) -> Option<TextureHandle> {
    let target_px = (logical_size * ctx.pixels_per_point()).round().clamp(16.0, 128.0) as usize;
    let cache_id = Id::new("wt_masthead_logo_tex");

    if let Some(cached) = ctx.data(|d| d.get_temp::<CachedLogoTexture>(cache_id)) {
        if cached.target_px == target_px {
            return Some(cached.handle);
        }
    }

    let img = downsample_logo_premultiplied(target_px)?;
    let handle = ctx.load_texture(
        format!("wt_logo_mark_{target_px}"),
        img,
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| {
        d.insert_temp(
            cache_id,
            CachedLogoTexture {
                target_px,
                handle: handle.clone(),
            },
        );
    });
    Some(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logo_png::alpha_bbox;

    #[test]
    fn embedded_logo_mark_decodes_at_native_resolution_and_downsamples_crisply() {
        assert_eq!(LOGO_MARK_PNG.len(), 84_018);
        let (w, h, ref rgba) = *decoded_logo_mark();
        assert_eq!((w, h), (1200, 1200));
        assert_eq!(alpha_bbox(w, h, rgba), Some((283, 276, 952, 935)));

        for sz in [16usize, 24, 30, 32, 36] {
            let img = downsample_logo_premultiplied(sz).expect("downsample must succeed");
            assert_eq!(img.size, [sz, sz]);
            assert_eq!(img.pixels[0], Color32::TRANSPARENT, "corners must remain transparent");
            assert!(
                img.pixels.iter().any(|c| c.a() > 200),
                "downsampled mark at {sz}px must contain crisp opaque foreground pixels"
            );
        }
    }
}
