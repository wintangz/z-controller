use std::collections::HashMap;
use std::ptr::null;

use egui::epaint::{ImageData, Primitive, TextureId, WHITE_UV};
use egui::{ClippedPrimitive, Color32, TexturesDelta};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HDC, SetDIBitsToDevice,
};

struct CpuTexture {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>, // Premultiplied sRGBA [R, G, B, A]
}

pub struct SoftwarePainter {
    width: usize,
    height: usize,
    framebuffer: Vec<u32>, // 0x00RRGGBB top-down
    textures: HashMap<TextureId, CpuTexture>,
}

impl SoftwarePainter {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            framebuffer: Vec::new(),
            textures: HashMap::new(),
        }
    }

    pub fn release_buffers(&mut self) {
        self.width = 0;
        self.height = 0;
        self.framebuffer = Vec::new();
        self.textures = HashMap::new();
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

    pub fn paint_and_blit(
        &mut self,
        hdc: HDC,
        width: usize,
        height: usize,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        clear_rgb: u32,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        if self.width != width || self.height != height || self.framebuffer.len() != width * height {
            self.width = width;
            self.height = height;
            self.framebuffer.resize(width * height, clear_rgb);
        } else {
            self.framebuffer.fill(clear_rgb);
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

            if clip_min_x >= clip_max_x || clip_min_y >= clip_max_y {
                continue;
            }

            if let Primitive::Mesh(mesh) = &cp.primitive {
                let tex = self.textures.get(&mesh.texture_id);
                for tri in mesh.indices.chunks_exact(3) {
                    let v0 = &mesh.vertices[tri[0] as usize];
                    let v1 = &mesh.vertices[tri[1] as usize];
                    let v2 = &mesh.vertices[tri[2] as usize];
                    rasterize_triangle(
                        &mut self.framebuffer,
                        width,
                        clip_min_x,
                        clip_min_y,
                        clip_max_x,
                        clip_max_y,
                        pixels_per_point,
                        v0,
                        v1,
                        v2,
                        tex,
                    );
                }
            }
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

        unsafe {
            SetDIBitsToDevice(
                hdc,
                0,
                0,
                width as u32,
                height as u32,
                0,
                0,
                0,
                height as u32,
                self.framebuffer.as_ptr().cast(),
                &bmi,
                DIB_RGB_COLORS,
            );
        }
        let _ = null::<u8>();
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
    let fx = u * tex.width as f32 - 0.5;
    let fy = v * tex.height as f32 - 0.5;
    let x0 = (fx.floor() as isize).clamp(0, tex.width as isize - 1) as usize;
    let y0 = (fy.floor() as isize).clamp(0, tex.height as isize - 1) as usize;
    let x1 = (x0 + 1).min(tex.width - 1);
    let y1 = (y0 + 1).min(tex.height - 1);
    let tx = fx.fract().clamp(0.0, 1.0);
    let ty = fy.fract().clamp(0.0, 1.0);

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
