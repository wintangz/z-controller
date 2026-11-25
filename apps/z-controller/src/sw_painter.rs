use std::collections::HashMap;
use std::ptr::null_mut;
use std::slice;

use egui::epaint::{ImageData, Primitive, TextureId, WHITE_UV};
use egui::{ClippedPrimitive, Color32, TexturesDelta};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SRCCOPY, SelectObject,
};

struct CpuTexture {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>, // Premultiplied sRGBA [R, G, B, A]
}

struct DibBackBuffer {
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits: *mut u32,
}

impl DibBackBuffer {
    fn new(hdc_ref: HDC, width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let mem_dc = unsafe { CreateCompatibleDC(hdc_ref) };
        if mem_dc.is_null() {
            return None;
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [unsafe { std::mem::zeroed() }],
        };
        let mut raw_bits: *mut std::ffi::c_void = null_mut();
        let bitmap = unsafe {
            CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut raw_bits, null_mut(), 0)
        };
        if bitmap.is_null() || raw_bits.is_null() {
            unsafe {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(mem_dc);
            }
            return None;
        }
        let old_bitmap = unsafe { SelectObject(mem_dc, bitmap) };
        Some(Self {
            mem_dc,
            bitmap,
            old_bitmap,
            bits: raw_bits.cast::<u32>(),
        })
    }
}

impl Drop for DibBackBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.mem_dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.mem_dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                DeleteDC(self.mem_dc);
            }
        }
    }
}

pub struct SoftwarePainter {
    width: usize,
    height: usize,
    dib: Option<DibBackBuffer>,
    fallback_fb: Vec<u32>, // Used only if headless/GDI allocation unavailable
    static_cache: Vec<u32>,
    static_key: u64,
    textures: HashMap<TextureId, CpuTexture>,
}

impl SoftwarePainter {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            dib: None,
            fallback_fb: Vec::new(),
            static_cache: Vec::new(),
            static_key: 0,
            textures: HashMap::new(),
        }
    }

    pub fn release_buffers(&mut self) {
        self.width = 0;
        self.height = 0;
        self.dib = None;
        self.fallback_fb = Vec::new();
        self.static_cache = Vec::new();
        self.static_key = 0;
        self.textures = HashMap::new();
    }

    pub fn has_ready_backbuffer(&self, width: usize, height: usize) -> bool {
        self.width == width
            && self.height == height
            && (self.dib.is_some() || self.fallback_fb.len() == width * height)
    }

    pub fn has_static_cache(&self, width: usize, height: usize, key: u64) -> bool {
        self.has_ready_backbuffer(width, height)
            && self.static_key == key
            && self.static_cache.len() == width * height
    }

    pub fn apply_textures(&mut self, delta: &TexturesDelta) {
        for (id, image_delta) in &delta.set {
            let (w, h, pixels) = decode_image(&image_delta.image);
            if let Some(pos) = image_delta.pos {
                if let Some(tex) = self.textures.get_mut(id) {
                    for row in 0..h {
                        let dy = pos[1] + row;
                        if dy >= tex.height {
                            break;
                        }
                        for col in 0..w {
                            let dx = pos[0] + col;
                            if dx >= tex.width {
                                break;
                            }
                            tex.pixels[dy * tex.width + dx] = pixels[row * w + col];
                        }
                    }
                    continue;
                }
            }
            self.textures.insert(
                *id,
                CpuTexture {
                    width: w,
                    height: h,
                    pixels,
                },
            );
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    fn ensure_backbuffer(&mut self, hdc_ref: HDC, width: usize, height: usize) {
        if self.width == width && self.height == height && (self.dib.is_some() || !self.fallback_fb.is_empty()) {
            return;
        }
        self.width = width;
        self.height = height;
        self.static_cache.clear();
        self.static_key = 0;
        self.dib = DibBackBuffer::new(hdc_ref, width, height);
        if self.dib.is_none() {
            self.fallback_fb.resize(width * height, 0);
        } else {
            self.fallback_fb = Vec::new();
        }
    }

    fn backbuffer_slice_mut(&mut self) -> &mut [u32] {
        let len = self.width * self.height;
        if let Some(d) = &self.dib {
            unsafe { slice::from_raw_parts_mut(d.bits, len) }
        } else {
            &mut self.fallback_fb[..len]
        }
    }

    pub fn save_static_cache(&mut self, key: u64) {
        let len = self.width * self.height;
        if len == 0 {
            return;
        }
        if self.static_cache.len() != len {
            self.static_cache.resize(len, 0);
        }
        let src_ptr = if let Some(d) = &self.dib {
            d.bits as *const u32
        } else if self.fallback_fb.len() == len {
            self.fallback_fb.as_ptr()
        } else {
            return;
        };
        unsafe {
            std::ptr::copy_nonoverlapping(src_ptr, self.static_cache.as_mut_ptr(), len);
        }
        self.static_key = key;
    }

    /// Blits ONLY the dirty region `dirty` from the persistent DIB back buffer to `hdc`.
    pub fn present_rect(&self, hdc: HDC, dirty: Option<RECT>) {
        if hdc.is_null() || self.width == 0 || self.height == 0 {
            return;
        }
        let (rx, ry, rw, rh) = match dirty {
            Some(r) => {
                let x0 = r.left.clamp(0, self.width as i32);
                let y0 = r.top.clamp(0, self.height as i32);
                let x1 = r.right.clamp(0, self.width as i32);
                let y1 = r.bottom.clamp(0, self.height as i32);
                if x1 <= x0 || y1 <= y0 {
                    (0, 0, self.width as i32, self.height as i32)
                } else {
                    (x0, y0, x1 - x0, y1 - y0)
                }
            }
            None => (0, 0, self.width as i32, self.height as i32),
        };
        if let Some(d) = &self.dib {
            unsafe {
                BitBlt(hdc, rx, ry, rw, rh, d.mem_dc, rx, ry, SRCCOPY);
            }
        }
    }

    /// Renders `primitives` into the double-buffered DIB (optionally clipped to `dirty_px`
    /// while restoring unchanged pixels from `static_cache`) and presents once via `BitBlt`.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_and_blit(
        &mut self,
        hdc: HDC,
        width: usize,
        height: usize,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        clear_rgb: u32,
        dirty_px: Option<[usize; 4]>,
        present_clip: Option<RECT>,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        self.ensure_backbuffer(hdc, width, height);

        let (bx0, by0, bx1, by1) = match dirty_px {
            Some([x0, y0, x1, y1]) if x0 < x1 && y0 < y1 => (
                x0.min(width),
                y0.min(height),
                x1.min(width),
                y1.min(height),
            ),
            _ => (0, 0, width, height),
        };
        if bx0 >= bx1 || by0 >= by1 {
            return;
        }

        // Clear dirty region in backbuffer (row-slice fill)
        {
            let fb = self.backbuffer_slice_mut();
            if bx0 == 0 && bx1 == width && by0 == 0 && by1 == height {
                fb.fill(clear_rgb);
            } else {
                for py in by0..by1 {
                    let off = py * width;
                    fb[off + bx0..off + bx1].fill(clear_rgb);
                }
            }
        }

        for cp in primitives {
            let clip_min_x = (cp.clip_rect.min.x * pixels_per_point)
                .floor()
                .clamp(0.0, width as f32) as usize;
            let clip_min_y = (cp.clip_rect.min.y * pixels_per_point)
                .floor()
                .clamp(0.0, height as f32) as usize;
            let clip_max_x = (cp.clip_rect.max.x * pixels_per_point)
                .ceil()
                .clamp(0.0, width as f32) as usize;
            let clip_max_y = (cp.clip_rect.max.y * pixels_per_point)
                .ceil()
                .clamp(0.0, height as f32) as usize;

            let cmin_x = clip_min_x.max(bx0);
            let cmin_y = clip_min_y.max(by0);
            let cmax_x = clip_max_x.min(bx1);
            let cmax_y = clip_max_y.min(by1);
            if cmin_x >= cmax_x || cmin_y >= cmax_y {
                continue;
            }

            if let Primitive::Mesh(mesh) = &cp.primitive {
                let has_tex = self.textures.contains_key(&mesh.texture_id);
                let len = width * height;
                let fb = if let Some(d) = &self.dib {
                    unsafe { slice::from_raw_parts_mut(d.bits, len) }
                } else {
                    &mut self.fallback_fb[..len]
                };

                // Axis-aligned flat-solid quad fast path (panels, rects, dividers, fills)
                let mut i = 0usize;
                while i + 6 <= mesh.indices.len() {
                    let t0 = &mesh.indices[i..i + 6];
                    if let Some((qx0, qy0, qx1, qy1, col)) =
                        detect_solid_quad(&mesh.vertices, t0, pixels_per_point)
                    {
                        fill_solid_rect(fb, width, cmin_x, cmin_y, cmax_x, cmax_y, qx0, qy0, qx1, qy1, col);
                        i += 6;
                    } else {
                        let tex = if has_tex {
                            self.textures.get(&mesh.texture_id)
                        } else {
                            None
                        };
                        for k in 0..2 {
                            let tri = &mesh.indices[i + k * 3..i + k * 3 + 3];
                            rasterize_triangle(
                                fb,
                                width,
                                cmin_x,
                                cmin_y,
                                cmax_x,
                                cmax_y,
                                pixels_per_point,
                                &mesh.vertices[tri[0] as usize],
                                &mesh.vertices[tri[1] as usize],
                                &mesh.vertices[tri[2] as usize],
                                tex,
                            );
                        }
                        i += 6;
                    }
                }
                while i + 3 <= mesh.indices.len() {
                    let tex = if has_tex {
                        self.textures.get(&mesh.texture_id)
                    } else {
                        None
                    };
                    let tri = &mesh.indices[i..i + 3];
                    rasterize_triangle(
                        fb,
                        width,
                        cmin_x,
                        cmin_y,
                        cmax_x,
                        cmax_y,
                        pixels_per_point,
                        &mesh.vertices[tri[0] as usize],
                        &mesh.vertices[tri[1] as usize],
                        &mesh.vertices[tri[2] as usize],
                        tex,
                    );
                    i += 3;
                }
            }
        }

        let blit_rect = present_clip.or_else(|| {
            if bx0 < bx1 && by0 < by1 {
                Some(RECT {
                    left: bx0 as i32,
                    top: by0 as i32,
                    right: bx1 as i32,
                    bottom: by1 as i32,
                })
            } else {
                None
            }
        });
        self.present_rect(hdc, blit_rect);
    }
}

#[inline]
fn detect_solid_quad(
    verts: &[egui::epaint::Vertex],
    idx6: &[u32],
    ppp: f32,
) -> Option<(usize, usize, usize, usize, [u8; 4])> {
    let v0 = &verts[idx6[0] as usize];
    if v0.uv != WHITE_UV {
        return None;
    }
    let c = v0.color.to_array();
    let mut xs = [f32::INFINITY, f32::NEG_INFINITY];
    let mut ys = [f32::INFINITY, f32::NEG_INFINITY];
    for &idx in idx6 {
        let v = &verts[idx as usize];
        if v.uv != WHITE_UV || v.color.to_array() != c {
            return None;
        }
        xs[0] = xs[0].min(v.pos.x);
        xs[1] = xs[1].max(v.pos.x);
        ys[0] = ys[0].min(v.pos.y);
        ys[1] = ys[1].max(v.pos.y);
    }
    for &idx in idx6 {
        let v = &verts[idx as usize];
        let on_x = (v.pos.x - xs[0]).abs() < 1e-3 || (v.pos.x - xs[1]).abs() < 1e-3;
        let on_y = (v.pos.y - ys[0]).abs() < 1e-3 || (v.pos.y - ys[1]).abs() < 1e-3;
        if !(on_x && on_y) {
            return None;
        }
    }
    let px0 = (xs[0] * ppp).round().max(0.0) as usize;
    let py0 = (ys[0] * ppp).round().max(0.0) as usize;
    let px1 = (xs[1] * ppp).round().max(0.0) as usize;
    let py1 = (ys[1] * ppp).round().max(0.0) as usize;
    if px1 <= px0 || py1 <= py0 {
        return None;
    }
    Some((px0, py0, px1, py1, c))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn fill_solid_rect(
    fb: &mut [u32],
    fb_w: usize,
    clip_min_x: usize,
    clip_min_y: usize,
    clip_max_x: usize,
    clip_max_y: usize,
    rx0: usize,
    ry0: usize,
    rx1: usize,
    ry1: usize,
    src: [u8; 4],
) {
    if src[3] == 0 {
        return;
    }
    let x0 = rx0.max(clip_min_x);
    let y0 = ry0.max(clip_min_y);
    let x1 = rx1.min(clip_max_x);
    let y1 = ry1.min(clip_max_y);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    if src[3] == 255 {
        let packed = ((src[0] as u32) << 16) | ((src[1] as u32) << 8) | (src[2] as u32);
        for py in y0..y1 {
            let row = py * fb_w;
            fb[row + x0..row + x1].fill(packed);
        }
    } else {
        let sr = src[0] as u32;
        let sg = src[1] as u32;
        let sb = src[2] as u32;
        let inv_a = 255 - src[3] as u32;
        for py in y0..y1 {
            let row = py * fb_w;
            for dst in &mut fb[row + x0..row + x1] {
                let d = *dst;
                let dr = (d >> 16) & 0xFF;
                let dg = (d >> 8) & 0xFF;
                let db = d & 0xFF;
                let r = (sr + ((dr * inv_a + 127) / 255)).min(255);
                let g = (sg + ((dg * inv_a + 127) / 255)).min(255);
                let b = (sb + ((db * inv_a + 127) / 255)).min(255);
                *dst = (r << 16) | (g << 8) | b;
            }
        }
    }
}

fn decode_image(img: &ImageData) -> (usize, usize, Vec<[u8; 4]>) {
    match img {
        ImageData::Color(c) => {
            let pixels = c.pixels.iter().map(Color32::to_array).collect();
            (c.size[0], c.size[1], pixels)
        }
        ImageData::Font(f) => {
            let pixels = f.srgba_pixels(None).map(|p| p.to_array()).collect();
            (f.size[0], f.size[1], pixels)
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn rasterize_triangle(
    fb: &mut [u32],
    fb_w: usize,
    clip_min_x: usize,
    clip_min_y: usize,
    clip_max_x: usize,
    clip_max_y: usize,
    ppp: f32,
    v0: &egui::epaint::Vertex,
    v1: &egui::epaint::Vertex,
    v2: &egui::epaint::Vertex,
    tex: Option<&CpuTexture>,
) {
    let x0 = v0.pos.x * ppp;
    let y0 = v0.pos.y * ppp;
    let x1 = v1.pos.x * ppp;
    let y1 = v1.pos.y * ppp;
    let x2 = v2.pos.x * ppp;
    let y2 = v2.pos.y * ppp;

    let mut area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
    if area.abs() < 1e-4 {
        return;
    }

    let min_x = x0.min(x1).min(x2).floor().max(clip_min_x as f32) as usize;
    let max_x = x0.max(x1).max(x2).ceil().min(clip_max_x as f32) as usize;
    let min_y = y0.min(y1).min(y2).floor().max(clip_min_y as f32) as usize;
    let max_y = y0.max(y1).max(y2).ceil().min(clip_max_y as f32) as usize;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let sign = if area < 0.0 {
        area = -area;
        -1.0f32
    } else {
        1.0f32
    };
    let inv_area = 1.0 / area;

    let c0 = v0.color.to_array();
    let c1 = v1.color.to_array();
    let c2 = v2.color.to_array();

    let flat_solid = c0 == c1 && c1 == c2 && v0.uv == WHITE_UV && v1.uv == WHITE_UV && v2.uv == WHITE_UV;
    let untextured = v0.uv == WHITE_UV && v1.uv == WHITE_UV && v2.uv == WHITE_UV;

    for py in min_y..max_y {
        let fy = py as f32 + 0.5;
        let row_off = py * fb_w;
        for px in min_x..max_x {
            let fx = px as f32 + 0.5;
            let w0 = ((x2 - x1) * (fy - y1) - (y2 - y1) * (fx - x1)) * sign;
            if w0 < -0.001 {
                continue;
            }
            let w1 = ((x0 - x2) * (fy - y2) - (y0 - y2) * (fx - x2)) * sign;
            if w1 < -0.001 {
                continue;
            }
            let w2 = ((x1 - x0) * (fy - y0) - (y1 - y0) * (fx - x0)) * sign;
            if w2 < -0.001 {
                continue;
            }

            let src = if flat_solid {
                c0
            } else {
                let b0 = (w0 * inv_area).clamp(0.0, 1.0);
                let b1 = (w1 * inv_area).clamp(0.0, 1.0);
                let b2 = (1.0 - b0 - b1).max(0.0);

                let vr = (c0[0] as f32 * b0 + c1[0] as f32 * b1 + c2[0] as f32 * b2 + 0.5) as u8;
                let vg = (c0[1] as f32 * b0 + c1[1] as f32 * b1 + c2[1] as f32 * b2 + 0.5) as u8;
                let vb = (c0[2] as f32 * b0 + c1[2] as f32 * b1 + c2[2] as f32 * b2 + 0.5) as u8;
                let va = (c0[3] as f32 * b0 + c1[3] as f32 * b1 + c2[3] as f32 * b2 + 0.5) as u8;

                if untextured {
                    [vr, vg, vb, va]
                } else if let Some(t) = tex {
                    let u = (v0.uv.x * b0 + v1.uv.x * b1 + v2.uv.x * b2).clamp(0.0, 1.0);
                    let v = (v0.uv.y * b0 + v1.uv.y * b1 + v2.uv.y * b2).clamp(0.0, 1.0);
                    let tc = sample_bilinear(t, u, v);
                    [
                        ((vr as u16 * tc[0] as u16 + 127) / 255) as u8,
                        ((vg as u16 * tc[1] as u16 + 127) / 255) as u8,
                        ((vb as u16 * tc[2] as u16 + 127) / 255) as u8,
                        ((va as u16 * tc[3] as u16 + 127) / 255) as u8,
                    ]
                } else {
                    [vr, vg, vb, va]
                }
            };

            if src[3] == 0 {
                continue;
            }
            if src[3] == 255 {
                fb[row_off + px] = ((src[0] as u32) << 16) | ((src[1] as u32) << 8) | (src[2] as u32);
            } else {
                let dst = fb[row_off + px];
                let dr = (dst >> 16) & 0xFF;
                let dg = (dst >> 8) & 0xFF;
                let db = dst & 0xFF;
                let inv_a = 255 - src[3] as u32;
                let r = (src[0] as u32 + ((dr * inv_a + 127) / 255)).min(255);
                let g = (src[1] as u32 + ((dg * inv_a + 127) / 255)).min(255);
                let b = (src[2] as u32 + ((db * inv_a + 127) / 255)).min(255);
                fb[row_off + px] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

#[inline]
fn sample_bilinear(tex: &CpuTexture, u: f32, v: f32) -> [u8; 4] {
    if tex.width == 0 || tex.height == 0 {
        return [255, 255, 255, 255];
    }
    let fx = (u * tex.width as f32 - 0.5).clamp(0.0, (tex.width - 1) as f32);
    let fy = (v * tex.height as f32 - 0.5).clamp(0.0, (tex.height - 1) as f32);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(tex.width - 1);
    let y1 = (y0 + 1).min(tex.height - 1);
    let tx = (fx - x0 as f32).clamp(0.0, 1.0);
    let ty = (fy - y0 as f32).clamp(0.0, 1.0);

    let p00 = tex.pixels[y0 * tex.width + x0];
    let p10 = tex.pixels[y0 * tex.width + x1];
    let p01 = tex.pixels[y1 * tex.width + x0];
    let p11 = tex.pixels[y1 * tex.width + x1];

    let w00 = (1.0 - tx) * (1.0 - ty);
    let w10 = tx * (1.0 - ty);
    let w01 = (1.0 - tx) * ty;
    let w11 = tx * ty;

    [
        (p00[0] as f32 * w00 + p10[0] as f32 * w10 + p01[0] as f32 * w01 + p11[0] as f32 * w11 + 0.5)
            as u8,
        (p00[1] as f32 * w00 + p10[1] as f32 * w10 + p01[1] as f32 * w01 + p11[1] as f32 * w11 + 0.5)
            as u8,
        (p00[2] as f32 * w00 + p10[2] as f32 * w10 + p01[2] as f32 * w01 + p11[2] as f32 * w11 + 0.5)
            as u8,
        (p00[3] as f32 * w00 + p10[3] as f32 * w10 + p01[3] as f32 * w01 + p11[3] as f32 * w11 + 0.5)
            as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput, Rect, Vec2, ViewportId};

    fn render_sample_text(ppp: f32, width: usize, height: usize) -> (f32, Vec<u32>) {
        let ctx = egui::Context::default();
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(width as f32 / ppp, height as f32 / ppp),
            )),
            ..Default::default()
        };
        raw.viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);

        let full_out = ctx.run(raw, |c| {
            egui::CentralPanel::default().show(c, |ui| {
                ui.heading("Controllers 150% DPI");
                ui.label("Crisp native glyph rasterization");
            });
        });

        let mut painter = SoftwarePainter::new();
        painter.apply_textures(&full_out.textures_delta);
        let clipped = ctx.tessellate(full_out.shapes, full_out.pixels_per_point);
        painter.paint_and_blit(
            null_mut(),
            width,
            height,
            full_out.pixels_per_point,
            &clipped,
            0x000A_0A10,
            None,
            None,
        );
        let pixels = painter.backbuffer_slice_mut().to_vec();
        (full_out.pixels_per_point, pixels)
    }

    #[test]
    fn high_dpi_scaling_renders_larger_font_atlas_and_crisp_150pct_buffer() {
        let (ppp100, fb100) = render_sample_text(1.0, 396, 601);
        let (ppp150, fb150) = render_sample_text(1.5, 594, 902);

        assert!((ppp100 - 1.0).abs() < 1e-4);
        assert!((ppp150 - 1.5).abs() < 1e-4);
        assert_eq!(fb100.len(), 396 * 601);
        assert_eq!(fb150.len(), 594 * 902);

        let lit_100 = fb100.iter().filter(|&&px| px != 0x000A_0A10 && px != 0x001B_1B1B).count();
        let lit_150 = fb150.iter().filter(|&&px| px != 0x000A_0A10 && px != 0x001B_1B1B).count();
        assert!(
            lit_150 * 10 > lit_100 * 17,
            "expected 150% DPI rendered pixel area ({lit_150}) to scale cleanly over 100% ({lit_100})"
        );
    }

    #[test]
    fn static_cache_and_dirty_rect_clipping_preserve_pixels() {
        let ctx = egui::Context::default();
        let mut painter = SoftwarePainter::new();
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 200.0))),
            ..Default::default()
        };
        let out = ctx.run(raw, |c| {
            egui::CentralPanel::default().show(c, |ui| {
                ui.label("Cached Chrome");
            });
        });
        painter.apply_textures(&out.textures_delta);
        let clipped = ctx.tessellate(out.shapes, 1.0);
        painter.paint_and_blit(null_mut(), 300, 200, 1.0, &clipped, 0x000A_0A10, None, None);
        painter.save_static_cache(42);
        assert!(painter.has_static_cache(300, 200, 42));
        assert!(!painter.has_static_cache(300, 200, 99));
    }
}
