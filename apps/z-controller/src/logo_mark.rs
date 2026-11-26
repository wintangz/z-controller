use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureOptions};

/// Original unmodified WT logo mark (1200x1200 RGBA PNG, transparent background).
pub const LOGO_MARK_PNG: &[u8] = include_bytes!("../../../assets/logo-mark.png");

struct DecodedRgba {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>, // Straight sRGBA bytes
}

#[repr(C)]
struct GdiplusStartupInput {
    gdiplus_version: u32,
    debug_event_callback: *const c_void,
    suppress_background_thread: i32,
    suppress_external_codecs: i32,
}

#[repr(C)]
struct GpRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
struct BitmapData {
    width: u32,
    height: u32,
    stride: i32,
    pixel_format: i32,
    scan0: *mut u8,
    reserved: usize,
}

#[repr(C)]
struct IStreamVtbl {
    query_interface: *const c_void,
    add_ref: *const c_void,
    release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
struct IStreamRaw {
    vtbl: *const IStreamVtbl,
}

const IMAGE_LOCK_MODE_READ: u32 = 1;
const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;

#[link(name = "shlwapi")]
unsafe extern "system" {
    fn SHCreateMemStream(p_init: *const u8, cb_init: u32) -> *mut IStreamRaw;
}

#[link(name = "gdiplus")]
unsafe extern "system" {
    fn GdiplusStartup(
        token: *mut usize,
        input: *const GdiplusStartupInput,
        output: *mut c_void,
    ) -> i32;
    fn GdipCreateBitmapFromStream(stream: *mut IStreamRaw, bitmap: *mut *mut c_void) -> i32;
    fn GdipGetImageWidth(image: *mut c_void, width: *mut u32) -> i32;
    fn GdipGetImageHeight(image: *mut c_void, height: *mut u32) -> i32;
    fn GdipBitmapLockBits(
        bitmap: *mut c_void,
        rect: *const GpRect,
        flags: u32,
        format: i32,
        locked_bitmap_data: *mut BitmapData,
    ) -> i32;
    fn GdipBitmapUnlockBits(bitmap: *mut c_void, locked_bitmap_data: *mut BitmapData) -> i32;
    fn GdipDisposeImage(image: *mut c_void) -> i32;
}

fn ensure_gdiplus_initialized() -> bool {
    static GDIP_TOKEN: OnceLock<Option<usize>> = OnceLock::new();
    GDIP_TOKEN
        .get_or_init(|| {
            let mut token: usize = 0;
            let input = GdiplusStartupInput {
                gdiplus_version: 1,
                debug_event_callback: null(),
                suppress_background_thread: 0,
                suppress_external_codecs: 0,
            };
            let status = unsafe { GdiplusStartup(&mut token, &input, null_mut()) };
            if status == 0 {
                Some(token)
            } else {
                None
            }
        })
        .is_some()
}

fn decode_logo_mark_once() -> Option<&'static DecodedRgba> {
    static DECODED: OnceLock<Option<DecodedRgba>> = OnceLock::new();
    DECODED
        .get_or_init(|| {
            if !ensure_gdiplus_initialized() || LOGO_MARK_PNG.is_empty() {
                return None;
            }
            let stream =
                unsafe { SHCreateMemStream(LOGO_MARK_PNG.as_ptr(), LOGO_MARK_PNG.len() as u32) };
            if stream.is_null() {
                return None;
            }

            let mut gp_bitmap: *mut c_void = null_mut();
            let create_status = unsafe { GdipCreateBitmapFromStream(stream, &mut gp_bitmap) };
            // GDI+ AddRefs the stream if needed; release our initial ref immediately after load.
            unsafe {
                let release_fn = (*(*stream).vtbl).release;
                release_fn(stream.cast());
            }
            if create_status != 0 || gp_bitmap.is_null() {
                return None;
            }

            let mut w: u32 = 0;
            let mut h: u32 = 0;
            let dim_ok = unsafe {
                GdipGetImageWidth(gp_bitmap, &mut w) == 0
                    && GdipGetImageHeight(gp_bitmap, &mut h) == 0
            };
            if !dim_ok || w == 0 || h == 0 {
                unsafe { GdipDisposeImage(gp_bitmap) };
                return None;
            }

            let lock_rect = GpRect {
                x: 0,
                y: 0,
                width: w as i32,
                height: h as i32,
            };
            let mut data = BitmapData {
                width: 0,
                height: 0,
                stride: 0,
                pixel_format: 0,
                scan0: null_mut(),
                reserved: 0,
            };
            let lock_status = unsafe {
                GdipBitmapLockBits(
                    gp_bitmap,
                    &lock_rect,
                    IMAGE_LOCK_MODE_READ,
                    PIXEL_FORMAT_32BPP_ARGB,
                    &mut data,
                )
            };
            if lock_status != 0 || data.scan0.is_null() {
                unsafe { GdipDisposeImage(gp_bitmap) };
                return None;
            }

            let width = data.width as usize;
            let height = data.height as usize;
            let mut pixels = vec![[0u8; 4]; width * height];
            for y in 0..height {
                let row_ptr = unsafe { data.scan0.offset((y as isize) * (data.stride as isize)) };
                for x in 0..width {
                    // GDI+ 32bppARGB stores [B, G, R, A] in little-endian memory.
                    let px = unsafe { row_ptr.add(x * 4) };
                    let b = unsafe { *px };
                    let g = unsafe { *px.add(1) };
                    let r = unsafe { *px.add(2) };
                    let a = unsafe { *px.add(3) };
                    pixels[y * width + x] = [r, g, b, a];
                }
            }

            unsafe {
                GdipBitmapUnlockBits(gp_bitmap, &mut data);
                GdipDisposeImage(gp_bitmap);
            }

            Some(DecodedRgba {
                width,
                height,
                pixels,
            })
        })
        .as_ref()
}

/// Box-area downsamples the embedded 1200x1200 logo mark into premultiplied sRGBA `ColorImage`
/// at exact physical pixel resolution `target_px x target_px` so transparent borders never fringe.
pub fn downsample_logo_premultiplied(target_px: usize) -> Option<ColorImage> {
    let src = decode_logo_mark_once()?;
    let dst = target_px.clamp(16, 128);
    let sw = src.width as f64;
    let sh = src.height as f64;
    let scale_x = sw / (dst as f64);
    let scale_y = sh / (dst as f64);

    let mut out = Vec::with_capacity(dst * dst);
    for dy in 0..dst {
        let sy0 = (dy as f64) * scale_y;
        let sy1 = ((dy + 1) as f64) * scale_y;
        let iy0 = (sy0.floor() as usize).min(src.height - 1);
        let iy1 = ((sy1.ceil() as usize).saturating_sub(1)).min(src.height - 1);

        for dx in 0..dst {
            let sx0 = (dx as f64) * scale_x;
            let sx1 = ((dx + 1) as f64) * scale_x;
            let ix0 = (sx0.floor() as usize).min(src.width - 1);
            let ix1 = ((sx1.ceil() as usize).saturating_sub(1)).min(src.width - 1);

            let mut acc_pr = 0.0_f64;
            let mut acc_pg = 0.0_f64;
            let mut acc_pb = 0.0_f64;
            let mut acc_a = 0.0_f64;
            let mut acc_w = 0.0_f64;

            for sy in iy0..=iy1 {
                let wy = (sy1.min((sy + 1) as f64) - sy0.max(sy as f64)).max(0.0);
                if wy <= 0.0 {
                    continue;
                }
                let row_off = sy * src.width;
                for sx in ix0..=ix1 {
                    let wx = (sx1.min((sx + 1) as f64) - sx0.max(sx as f64)).max(0.0);
                    let w = wx * wy;
                    if w <= 0.0 {
                        continue;
                    }
                    let [r, g, b, a] = src.pixels[row_off + sx];
                    let af = (a as f64) / 255.0;
                    acc_pr += (r as f64) * af * w;
                    acc_pg += (g as f64) * af * w;
                    acc_pb += (b as f64) * af * w;
                    acc_a += (a as f64) * w;
                    acc_w += w;
                }
            }

            if acc_w > 0.0 {
                let pa = (acc_a / acc_w).round().clamp(0.0, 255.0) as u8;
                let pr = (acc_pr / acc_w).round().clamp(0.0, pa as f64) as u8;
                let pg = (acc_pg / acc_w).round().clamp(0.0, pa as f64) as u8;
                let pb = (acc_pb / acc_w).round().clamp(0.0, pa as f64) as u8;
                out.push(Color32::from_rgba_premultiplied(pr, pg, pb, pa));
            } else {
                out.push(Color32::TRANSPARENT);
            }
        }
    }

    Some(ColorImage {
        size: [dst, dst],
        pixels: out,
    })
}

#[derive(Clone)]
struct CachedLogoTexture {
    target_px: usize,
    handle: TextureHandle,
}

/// Returns the premultiplied WT masthead logo texture sized cleanly for `24.0 * pixels_per_point`.
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

    #[test]
    fn embedded_logo_mark_decodes_at_native_resolution_and_downsamples_crisply() {
        let decoded = decode_logo_mark_once().expect("embedded PNG should decode via GDI+");
        assert_eq!(decoded.width, 1200);
        assert_eq!(decoded.height, 1200);
        assert_eq!(decoded.pixels[0][3], 0, "corners must remain transparent");
        assert!(
            decoded.pixels.iter().any(|px| px[3] == 255),
            "interior artwork must have opaque pixels"
        );

        let img24 = downsample_logo_premultiplied(24).expect("24px downsample must succeed");
        assert_eq!(img24.size, [24, 24]);
        assert_eq!(img24.pixels[0], Color32::TRANSPARENT);
        assert!(img24.pixels.iter().any(|c| c.a() > 200));

        let img36 = downsample_logo_premultiplied(36).expect("36px downsample must succeed");
        assert_eq!(img36.size, [36, 36]);
        assert!(img36.pixels.iter().any(|c| c.a() > 200));
    }
}
