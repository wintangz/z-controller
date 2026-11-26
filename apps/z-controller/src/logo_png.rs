// Pure-Rust PNG RGBA decoder (DEFLATE + Adler-32 + all 5 PNG filters) and premultiplied-alpha area-box downsampler.

pub fn alpha_bbox(w: usize, h: usize, rgba: &[u8]) -> Option<(usize, usize, usize, usize)> {
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return None;
    }
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found = false;
    for y in 0..h {
        let row = y * w * 4;
        for x in 0..w {
            if rgba[row + x * 4 + 3] > 0 {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if found {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Downsamples the trimmed opaque artwork bounds from `src_rgba` into a sharp `out_size x out_size` RGBA sprite
/// using exact premultiplied-alpha area-box integration (no darkened edges on transparent/light backgrounds).
pub fn downsample_trimmed_logo_rgba(
    src_w: usize,
    src_h: usize,
    src_rgba: &[u8],
    out_size: usize,
) -> Vec<u8> {
    if out_size == 0 || src_w == 0 || src_h == 0 || src_rgba.len() < src_w * src_h * 4 {
        return Vec::new();
    }
    let (min_x, min_y, max_x, max_y) =
        alpha_bbox(src_w, src_h, src_rgba).unwrap_or((0, 0, src_w - 1, src_h - 1));
    let bw = (max_x - min_x + 1) as f64;
    let bh = (max_y - min_y + 1) as f64;
    let cx = 0.5 * (min_x as f64 + (max_x + 1) as f64);
    let cy = 0.5 * (min_y as f64 + (max_y + 1) as f64);
    let side = bw.max(bh) * 1.04;
    let crop_x0 = cx - side * 0.5;
    let crop_y0 = cy - side * 0.5;
    let step = side / (out_size as f64);

    let mut out = vec![0u8; out_size * out_size * 4];
    for oy in 0..out_size {
        let sy0 = crop_y0 + (oy as f64) * step;
        let sy1 = sy0 + step;
        let iy0 = (sy0.floor() as isize).max(0) as usize;
        let iy1 = (sy1.ceil() as isize).clamp(0, src_h as isize) as usize;

        for ox in 0..out_size {
            let sx0 = crop_x0 + (ox as f64) * step;
            let sx1 = sx0 + step;
            let ix0 = (sx0.floor() as isize).max(0) as usize;
            let ix1 = (sx1.ceil() as isize).clamp(0, src_w as isize) as usize;

            let total_area = step * step;
            let mut sum_ra = 0.0_f64;
            let mut sum_ga = 0.0_f64;
            let mut sum_ba = 0.0_f64;
            let mut sum_a = 0.0_f64;

            for sy in iy0..iy1 {
                let wy = (sy1.min((sy + 1) as f64) - sy0.max(sy as f64)).max(0.0);
                if wy <= 0.0 {
                    continue;
                }
                let row_off = sy * src_w * 4;
                for sx in ix0..ix1 {
                    let wx = (sx1.min((sx + 1) as f64) - sx0.max(sx as f64)).max(0.0);
                    let w = wx * wy;
                    if w <= 0.0 {
                        continue;
                    }
                    let off = row_off + sx * 4;
                    let a = (src_rgba[off + 3] as f64) / 255.0;
                    if a <= 0.0 {
                        continue;
                    }
                    let aw = a * w;
                    sum_ra += (src_rgba[off] as f64) * aw;
                    sum_ga += (src_rgba[off + 1] as f64) * aw;
                    sum_ba += (src_rgba[off + 2] as f64) * aw;
                    sum_a += aw;
                }
            }

            let dst = (oy * out_size + ox) * 4;
            let avg_a = (sum_a / total_area).clamp(0.0, 1.0);
            let out_a = (avg_a * 255.0).round() as u8;
            if out_a == 0 || sum_a <= 1e-9 {
                out[dst..dst + 4].copy_from_slice(&[0, 0, 0, 0]);
            } else {
                out[dst] = (sum_ra / sum_a).round().clamp(0.0, 255.0) as u8;
                out[dst + 1] = (sum_ga / sum_a).round().clamp(0.0, 255.0) as u8;
                out[dst + 2] = (sum_ba / sum_a).round().clamp(0.0, 255.0) as u8;
                out[dst + 3] = out_a;
            }
        }
    }
    out
}

pub fn decode_png_rgba(png_bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    const SIG: &[u8; 8] = b"\x89PNG\r\n\x1A\n";
    if png_bytes.len() < 8 || &png_bytes[..8] != SIG {
        return Err("invalid PNG signature".into());
    }

    let mut pos = 8usize;
    let mut width = 0usize;
    let mut height = 0usize;
    let mut saw_ihdr = false;
    let mut idat = Vec::new();

    while pos + 12 <= png_bytes.len() {
        let len = u32::from_be_bytes(png_bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let tag = &png_bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| "PNG chunk length overflow".to_string())?;
        if data_end + 4 > png_bytes.len() {
            return Err("truncated PNG chunk".into());
        }
        let chunk = &png_bytes[data_start..data_end];
        pos = data_end + 4;

        match tag {
            b"IHDR" => {
                if chunk.len() < 13 {
                    return Err("invalid IHDR chunk".into());
                }
                width = u32::from_be_bytes(chunk[0..4].try_into().unwrap()) as usize;
                height = u32::from_be_bytes(chunk[4..8].try_into().unwrap()) as usize;
                let bit_depth = chunk[8];
                let color_type = chunk[9];
                let compression = chunk[10];
                let filter = chunk[11];
                let interlace = chunk[12];
                if bit_depth != 8 || color_type != 6 || compression != 0 || filter != 0 || interlace != 0 {
                    return Err(format!(
                        "unsupported PNG format: depth={bit_depth}, color={color_type}, comp={compression}, filt={filter}, interlace={interlace}"
                    ));
                }
                saw_ihdr = true;
            }
            b"IDAT" => idat.extend_from_slice(chunk),
            b"IEND" => break,
            _ => {}
        }
    }

    if !saw_ihdr || width == 0 || height == 0 {
        return Err("missing valid PNG IHDR".into());
    }

    let raw = inflate_zlib(&idat)?;
    let bpp = 4usize;
    let stride = width * bpp;
    let expected = height * (stride + 1);
    if raw.len() != expected {
        return Err(format!(
            "unexpected inflated PNG byte count: got {}, expected {}",
            raw.len(),
            expected
        ));
    }

    let mut rgba = vec![0u8; height * stride];
    for y in 0..height {
        let src_row_off = y * (stride + 1);
        let filter_type = raw[src_row_off];
        let src_row = &raw[src_row_off + 1..src_row_off + 1 + stride];
        let (prev_slice, cur_slice) = if y == 0 {
            (&[][..], &mut rgba[0..stride])
        } else {
            let (before, after) = rgba.split_at_mut(y * stride);
            (&before[(y - 1) * stride..], &mut after[0..stride])
        };

        for x in 0..stride {
            let raw_b = src_row[x];
            let a = if x >= bpp { cur_slice[x - bpp] } else { 0 };
            let b = if !prev_slice.is_empty() { prev_slice[x] } else { 0 };
            let c = if !prev_slice.is_empty() && x >= bpp {
                prev_slice[x - bpp]
            } else {
                0
            };
            cur_slice[x] = match filter_type {
                0 => raw_b,
                1 => raw_b.wrapping_add(a),
                2 => raw_b.wrapping_add(b),
                3 => raw_b.wrapping_add((((a as u16) + (b as u16)) >> 1) as u8),
                4 => raw_b.wrapping_add(paeth_predictor(a, b, c)),
                other => return Err(format!("unsupported PNG scanline filter {other}")),
            };
        }
    }

    Ok((width, height, rgba))
}

#[inline]
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let c = c as i32;
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], byte_offset: usize) -> Self {
        Self {
            data,
            bit_pos: byte_offset * 8,
        }
    }

    fn read_bits(&mut self, n: usize) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if self.bit_pos + n > self.data.len() * 8 {
            return Err("unexpected EOF in DEFLATE stream".into());
        }
        let mut val = 0u32;
        for shift in 0..n {
            let b = self.data[self.bit_pos >> 3];
            let bit = ((b >> (self.bit_pos & 7)) & 1) as u32;
            val |= bit << shift;
            self.bit_pos += 1;
        }
        Ok(val)
    }

    fn align_byte(&mut self) {
        let rem = self.bit_pos & 7;
        if rem != 0 {
            self.bit_pos += 8 - rem;
        }
    }

    fn read_aligned_u16(&mut self) -> Result<u16, String> {
        self.align_byte();
        let byte_idx = self.bit_pos >> 3;
        if byte_idx + 2 > self.data.len() {
            return Err("unexpected EOF reading aligned u16".into());
        }
        let v = u16::from_le_bytes([self.data[byte_idx], self.data[byte_idx + 1]]);
        self.bit_pos += 16;
        Ok(v)
    }

    fn read_aligned_slice(&mut self, len: usize) -> Result<&'a [u8], String> {
        self.align_byte();
        let byte_idx = self.bit_pos >> 3;
        if byte_idx + len > self.data.len() {
            return Err("unexpected EOF in stored DEFLATE block".into());
        }
        self.bit_pos += len * 8;
        Ok(&self.data[byte_idx..byte_idx + len])
    }
}

struct CanonicalHuffman {
    first_code: [u32; 16],
    first_sym_idx: [u16; 16],
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl CanonicalHuffman {
    fn from_lengths(lengths: &[u8]) -> Result<Self, String> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            if l as usize >= 16 {
                return Err(format!("Huffman code length {l} exceeds 15 bits"));
            }
            if l > 0 {
                counts[l as usize] += 1;
            }
        }

        let mut next_code = [0u32; 16];
        let mut first_code = [0u32; 16];
        let mut first_sym_idx = [0u16; 16];
        let mut code = 0u32;
        let mut sym_idx = 0u16;
        for bits in 1..=15 {
            code = (code + counts[bits - 1] as u32) << 1;
            first_code[bits] = code;
            next_code[bits] = code;
            first_sym_idx[bits] = sym_idx;
            sym_idx = sym_idx.saturating_add(counts[bits]);
        }

        let total_syms = sym_idx as usize;
        let mut symbols = vec![0u16; total_syms];
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                let b = l as usize;
                let offset = (next_code[b] - first_code[b]) as usize;
                let idx = first_sym_idx[b] as usize + offset;
                if idx >= total_syms {
                    return Err("corrupt Huffman table".into());
                }
                symbols[idx] = sym as u16;
                next_code[b] += 1;
            }
        }

        Ok(Self {
            first_code,
            first_sym_idx,
            counts,
            symbols,
        })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Result<u16, String> {
        let mut code = 0u32;
        for len in 1..=15 {
            code = (code << 1) | br.read_bits(1)?;
            let count = self.counts[len] as u32;
            if count > 0 {
                let first = self.first_code[len];
                if code >= first && (code - first) < count {
                    let idx = self.first_sym_idx[len] as usize + (code - first) as usize;
                    return Ok(self.symbols[idx]);
                }
            }
        }
        Err("invalid Huffman symbol in DEFLATE stream".into())
    }
}

fn fixed_huffman_tables() -> Result<(CanonicalHuffman, CanonicalHuffman), String> {
    let mut lit_lens = vec![0u8; 288];
    lit_lens[0..=143].fill(8);
    lit_lens[144..=255].fill(9);
    lit_lens[256..=279].fill(7);
    lit_lens[280..=287].fill(8);
    let dist_lens = vec![5u8; 32];
    Ok((
        CanonicalHuffman::from_lengths(&lit_lens)?,
        CanonicalHuffman::from_lengths(&dist_lens)?,
    ))
}

fn dynamic_huffman_tables(br: &mut BitReader<'_>) -> Result<(CanonicalHuffman, CanonicalHuffman), String> {
    const CL_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;

    let mut cl_lens = [0u8; 19];
    for &idx in CL_ORDER.iter().take(hclen) {
        cl_lens[idx] = br.read_bits(3)? as u8;
    }
    let cl_tree = CanonicalHuffman::from_lengths(&cl_lens)?;

    let total = hlit + hdist;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let sym = cl_tree.decode(br)?;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                let prev = *lengths
                    .last()
                    .ok_or_else(|| "DEFLATE repeat length with empty history".to_string())?;
                let rep = br.read_bits(2)? as usize + 3;
                lengths.resize((lengths.len() + rep).min(total), prev);
            }
            17 => {
                let rep = br.read_bits(3)? as usize + 3;
                lengths.resize((lengths.len() + rep).min(total), 0);
            }
            18 => {
                let rep = br.read_bits(7)? as usize + 11;
                lengths.resize((lengths.len() + rep).min(total), 0);
            }
            _ => return Err(format!("invalid code length symbol {sym}")),
        }
    }

    Ok((
        CanonicalHuffman::from_lengths(&lengths[..hlit])?,
        CanonicalHuffman::from_lengths(&lengths[hlit..hlit + hdist])?,
    ))
}

fn inflate_zlib(zlib: &[u8]) -> Result<Vec<u8>, String> {
    if zlib.len() < 6 {
        return Err("zlib stream too short".into());
    }
    let cmf = zlib[0];
    let flg = zlib[1];
    if (cmf & 0x0F) != 8 || (((cmf as u16) << 8 | (flg as u16)) % 31) != 0 {
        return Err("invalid zlib header".into());
    }
    if (flg & 0x20) != 0 {
        return Err("unsupported preset zlib dictionary".into());
    }

    let mut br = BitReader::new(zlib, 2);
    let mut out = Vec::new();

    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [usize; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [usize; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                let len = br.read_aligned_u16()?;
                let nlen = br.read_aligned_u16()?;
                if len != !nlen {
                    return Err("corrupt stored DEFLATE block length".into());
                }
                let slice = br.read_aligned_slice(len as usize)?;
                out.extend_from_slice(slice);
            }
            1 | 2 => {
                let (lit_tree, dist_tree) = if btype == 1 {
                    fixed_huffman_tables()?
                } else {
                    dynamic_huffman_tables(&mut br)?
                };
                loop {
                    let sym = lit_tree.decode(&mut br)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let lidx = sym - 257;
                        if lidx >= LEN_BASE.len() {
                            return Err(format!("invalid DEFLATE length symbol {sym}"));
                        }
                        let length =
                            LEN_BASE[lidx] as usize + br.read_bits(LEN_EXTRA[lidx])? as usize;
                        let dsym = dist_tree.decode(&mut br)? as usize;
                        if dsym >= DIST_BASE.len() {
                            return Err(format!("invalid DEFLATE distance symbol {dsym}"));
                        }
                        let dist =
                            DIST_BASE[dsym] as usize + br.read_bits(DIST_EXTRA[dsym])? as usize;
                        if dist == 0 || dist > out.len() {
                            return Err("invalid DEFLATE back-reference distance".into());
                        }
                        for _ in 0..length {
                            let b = out[out.len() - dist];
                            out.push(b);
                        }
                    }
                }
            }
            other => return Err(format!("invalid DEFLATE block type {other}")),
        }
        if bfinal != 0 {
            break;
        }
    }

    br.align_byte();
    let tail_off = br.bit_pos >> 3;
    if tail_off + 4 <= zlib.len() {
        let expected_adler =
            u32::from_be_bytes(zlib[tail_off..tail_off + 4].try_into().unwrap());
        let actual_adler = adler32(&out);
        if expected_adler != actual_adler {
            return Err(format!(
                "zlib Adler-32 mismatch: expected {expected_adler:#010x}, got {actual_adler:#010x}"
            ));
        }
    }

    Ok(out)
}
