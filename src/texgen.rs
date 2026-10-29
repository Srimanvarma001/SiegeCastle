//! Realistic surface textures, generated in code at startup.
//!
//! Each surface gets a color map, a normal map (fine relief so light catches
//! edges and grooves) and a roughness/metal map. All tile seamlessly and come
//! with mipmaps so distant surfaces stay crisp instead of shimmering.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const S: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Surface {
    Stone,
    Slate,
    Wood,
    Iron,
    Grass,
    Mud,
    Wheat,
    Plaster,
    Bark,
}

pub struct SurfaceTextures {
    pub color: Handle<Image>,
    pub normal: Handle<Image>,
    /// Green = roughness, blue = metalness (glTF layout).
    pub rough_metal: Handle<Image>,
}

/// Raw maps before upload.
struct Maps {
    color: Vec<Vec3>,
    height: Vec<f32>,
    rough: Vec<f32>,
    metal: Vec<f32>,
    /// How strong the relief looks.
    bump: f32,
}

impl Maps {
    fn new(bump: f32) -> Self {
        Self {
            color: vec![Vec3::ZERO; S * S],
            height: vec![0.0; S * S],
            rough: vec![0.9; S * S],
            metal: vec![0.0; S * S],
            bump,
        }
    }
}

// --- Tileable noise ---------------------------------------------------------

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_215.0
}

/// Value noise that wraps every `period` cells, so textures tile seamlessly.
fn vnoise(u: f32, v: f32, period: i32, seed: u32) -> f32 {
    let x = u * period as f32;
    let y = v * period as f32;
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let w = |a: i32| a.rem_euclid(period);
    let a = hash(w(xi), w(yi), seed);
    let b = hash(w(xi + 1), w(yi), seed);
    let c = hash(w(xi), w(yi + 1), seed);
    let d = hash(w(xi + 1), w(yi + 1), seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

fn fbm(u: f32, v: f32, period: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 0.5, 0.0, period);
    for o in 0..octaves {
        sum += vnoise(u, v, p, seed + o * 17) * amp;
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rgb(r: f32, g: f32, b: f32) -> Vec3 {
    Vec3::new(r, g, b)
}

fn each(m: &mut Maps, mut f: impl FnMut(f32, f32, &mut Vec3, &mut f32, &mut f32, &mut f32)) {
    for y in 0..S {
        for x in 0..S {
            let i = y * S + x;
            let (u, v) = (x as f32 / S as f32, y as f32 / S as f32);
            let (mut c, mut h, mut r, mut mt) = (m.color[i], m.height[i], m.rough[i], m.metal[i]);
            f(u, v, &mut c, &mut h, &mut r, &mut mt);
            m.color[i] = c;
            m.height[i] = h;
            m.rough[i] = r;
            m.metal[i] = mt;
        }
    }
}

// --- Surfaces ----------------------------------------------------------------

/// Cut limestone blocks in staggered courses with recessed mortar.
fn stone() -> Maps {
    let mut m = Maps::new(5.0);
    let rows = 8.0;
    let cols = 4.0;
    each(&mut m, |u, v, c, h, r, _| {
        let row = (v * rows).floor();
        let fv = v * rows - row;
        let off = if row as i32 % 2 == 0 { 0.0 } else { 0.5 } + hash(row as i32, 3, 7) * 0.2;
        let uu = (u + off / cols).rem_euclid(1.0);
        let col = (uu * cols).floor();
        let fu = uu * cols - col;
        let dx = fu.min(1.0 - fu) / cols;
        let dy = fv.min(1.0 - fv) / rows;
        let edge = dx.min(dy);
        let id = hash(col as i32, row as i32, 11);
        let mortar = 0.006;
        let bevel = smooth(mortar, mortar + 0.014, edge);
        let chips = fbm(u, v, 32, 3, 5);
        let grain = fbm(u, v, 96, 2, 9);
        let dirt = fbm(u, v, 4, 3, 21);
        let tone = 0.82 + id * 0.3;
        let base = rgb(0.58, 0.55, 0.50) * tone + rgb(0.05, 0.03, 0.0) * (id - 0.5);
        let stone_col = base * (0.85 + grain * 0.3) * (0.78 + dirt * 0.35) * (0.8 + 0.2 * bevel);
        let mortar_col = rgb(0.36, 0.34, 0.31) * (0.8 + grain * 0.3);
        let in_block = bevel > 0.0;
        *c = if in_block { stone_col.lerp(mortar_col, 1.0 - bevel) } else { mortar_col };
        *h = if in_block { 0.4 + 0.6 * bevel - chips * 0.25 * bevel } else { 0.05 + grain * 0.05 };
        *r = 0.82 + grain * 0.15;
    });
    m
}

/// Overlapping slate shingles with rounded ends, dark blue-grey with moss.
fn slate() -> Maps {
    let mut m = Maps::new(6.0);
    let rows = 12.0;
    let cols = 8.0;
    each(&mut m, |u, v, c, h, r, _| {
        let row = (v * rows).floor();
        let fv = v * rows - row;
        let off = if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
        let uu = (u + off / cols).rem_euclid(1.0);
        let col = (uu * cols).floor();
        let fu = uu * cols - col;
        let id = hash(col as i32, row as i32, 31);
        // Rounded bottom edge of each shingle.
        let round = ((fu - 0.5) * 2.0).powi(2) * 0.25;
        let body = fv < 1.0 - round;
        let gap = smooth(0.0, 0.05, fu.min(1.0 - fu));
        let moss = smooth(0.62, 0.8, fbm(u, v, 6, 4, 41));
        let grain = fbm(u, v, 128, 2, 43);
        let base = rgb(0.20, 0.22, 0.26) * (0.8 + id * 0.45) * (0.9 + grain * 0.2);
        let col_s = base.lerp(rgb(0.25, 0.30, 0.16), moss * 0.7);
        if body {
            *c = col_s * (0.65 + 0.35 * fv) * (0.6 + 0.4 * gap);
            *h = (0.2 + 0.8 * fv) * gap;
        } else {
            *c = col_s * 0.35;
            *h = 0.0;
        }
        *r = 0.55 + grain * 0.2 + moss * 0.3;
    });
    m
}

/// Weathered planks with grain and dark gaps.
fn wood() -> Maps {
    let mut m = Maps::new(3.0);
    let rows = 6.0;
    each(&mut m, |u, v, c, h, r, _| {
        let row = (v * rows).floor();
        let fv = v * rows - row;
        let id = hash(row as i32, 1, 51);
        let grain = fbm(u * 1.0 + id, v * 6.0, 8, 4, 53);
        let streak = vnoise(u, v * 0.5 + id, 64, 57);
        let gap = smooth(0.0, 0.06, fv.min(1.0 - fv));
        let knot = smooth(0.82, 0.9, vnoise(u, v, 12, 59 + row as u32));
        let base = rgb(0.38, 0.26, 0.16) * (0.75 + id * 0.45);
        *c = base * (0.75 + grain * 0.4 + streak * 0.1) * (0.4 + 0.6 * gap) * (1.0 - knot * 0.4);
        *h = gap * (0.6 + grain * 0.4) - knot * 0.2;
        *r = 0.75 + grain * 0.2;
    });
    m
}

/// Dark forged iron with hammer marks and rust.
fn iron() -> Maps {
    let mut m = Maps::new(2.0);
    each(&mut m, |u, v, c, h, r, mt| {
        let n = fbm(u, v, 16, 4, 61);
        let fine = fbm(u, v, 128, 2, 63);
        let rust = smooth(0.55, 0.75, fbm(u, v, 5, 4, 67));
        let metal_col = rgb(0.22, 0.22, 0.24) * (0.8 + n * 0.4);
        let rust_col = rgb(0.36, 0.17, 0.08) * (0.7 + fine * 0.5);
        *c = metal_col.lerp(rust_col, rust);
        *h = n * 0.6 + fine * 0.2 + rust * 0.3;
        *r = 0.35 + fine * 0.2 + rust * 0.5;
        *mt = 0.85 * (1.0 - rust);
    });
    m
}

/// Meadow grass seen from above: blades of several greens, dry stalks,
/// clover, bare soil and tiny wildflowers.
fn grass() -> Maps {
    let mut m = Maps::new(3.0);
    each(&mut m, |u, v, c, h, r, _| {
        let blades = fbm(u * 1.0, v * 1.0, 160, 2, 71);
        let streak = fbm(u * 3.0, v * 0.6, 64, 2, 72);
        let clumps = fbm(u, v, 24, 3, 73);
        let large = fbm(u, v, 4, 3, 75);
        let bare = smooth(0.70, 0.86, fbm(u, v, 10, 3, 77));
        let dry = smooth(0.55, 0.75, fbm(u, v, 14, 3, 79));
        let clover = smooth(0.74, 0.8, fbm(u, v, 40, 2, 80));
        let fresh = rgb(0.22, 0.32, 0.10).lerp(rgb(0.34, 0.43, 0.15), large);
        let mut g = fresh * (0.55 + blades * 0.7) * (0.85 + clumps * 0.3) * (0.9 + streak * 0.2);
        g = g.lerp(rgb(0.46, 0.43, 0.22) * (0.7 + blades * 0.5), dry * 0.45);
        g = g.lerp(rgb(0.18, 0.30, 0.10), clover * 0.5);
        g = g.lerp(rgb(0.34, 0.27, 0.18) * (0.8 + blades * 0.3), bare * 0.55);
        // Tiny flowers.
        let f = vnoise(u, v, 220, 82);
        if f > 0.93 {
            let kind = hash((u * 220.0) as i32, (v * 220.0) as i32, 83);
            let petal = if kind < 0.5 { rgb(0.92, 0.90, 0.82) } else if kind < 0.8 { rgb(0.90, 0.78, 0.25) } else { rgb(0.55, 0.42, 0.75) };
            g = g.lerp(petal, smooth(0.93, 0.97, f));
        }
        *c = g;
        *h = blades * 0.7 + clumps * 0.3 + clover * 0.2;
        *r = 0.9 - clover * 0.15;
    });
    m
}

/// Rough bark with vertical furrows.
fn bark() -> Maps {
    let mut m = Maps::new(6.0);
    each(&mut m, |u, v, c, h, r, _| {
        let furrow = fbm(u * 1.0, v * 0.15, 24, 4, 111);
        let ridge = (furrow * 9.0).sin() * 0.5 + 0.5;
        let lichen = smooth(0.66, 0.8, fbm(u, v, 6, 3, 113));
        let base = rgb(0.24, 0.18, 0.13) * (0.6 + ridge * 0.6);
        *c = base.lerp(rgb(0.42, 0.45, 0.32), lichen * 0.5);
        *h = ridge;
        *r = 0.95;
    });
    m
}

/// Churned mud with pebbles, for tracks and harvested fields.
fn mud() -> Maps {
    let mut m = Maps::new(4.0);
    each(&mut m, |u, v, c, h, r, _| {
        let n = fbm(u, v, 12, 4, 81);
        let fine = fbm(u, v, 96, 2, 83);
        let pebble = smooth(0.78, 0.84, vnoise(u, v, 48, 85));
        let wet = smooth(0.6, 0.75, fbm(u, v, 5, 3, 87));
        let base = rgb(0.33, 0.24, 0.16) * (0.75 + n * 0.4) * (0.9 + fine * 0.2);
        *c = base.lerp(rgb(0.52, 0.48, 0.42), pebble * 0.8) * (1.0 - wet * 0.25);
        *h = n * 0.6 + pebble * 0.5 + fine * 0.1;
        *r = 0.9 - wet * 0.45;
    });
    m
}

/// Rows of ripe wheat seen from above.
fn wheat() -> Maps {
    let mut m = Maps::new(3.0);
    each(&mut m, |u, v, c, h, r, _| {
        let rows = 10.0;
        let fv = (v * rows).fract();
        let row_shade = smooth(0.0, 0.35, fv.min(1.0 - fv));
        let stalks = fbm(u * 0.5, v, 96, 3, 91);
        let heads = smooth(0.55, 0.7, vnoise(u, v, 160, 93));
        let base = rgb(0.72, 0.56, 0.26).lerp(rgb(0.84, 0.70, 0.36), stalks);
        *c = base * (0.55 + 0.45 * row_shade) * (1.0 + heads * 0.15);
        *h = row_shade * 0.7 + stalks * 0.3 + heads * 0.2;
        *r = 0.85;
    });
    m
}

/// Lime plaster walls for farmhouses.
fn plaster() -> Maps {
    let mut m = Maps::new(1.5);
    each(&mut m, |u, v, c, h, r, _| {
        let n = fbm(u, v, 8, 4, 101);
        let fine = fbm(u, v, 96, 2, 103);
        let stain = smooth(0.55, 0.8, fbm(u, v, 3, 3, 105));
        *c = rgb(0.80, 0.75, 0.64) * (0.88 + fine * 0.15) * (1.0 - stain * 0.2);
        *h = n * 0.5 + fine * 0.3;
        *r = 0.92;
    });
    m
}

// --- Upload ------------------------------------------------------------------

fn sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    })
}

/// Builds an RGBA8 image with a full mip chain from a level-0 pixel function.
fn image_with_mips(pixels: Vec<[f32; 4]>, format: TextureFormat, renormalize: bool) -> Image {
    image_with_mips_sized(pixels, S, S, format, renormalize)
}

fn image_with_mips_sized(pixels: Vec<[f32; 4]>, w0: usize, h0: usize, format: TextureFormat, renormalize: bool) -> Image {
    let mut levels: Vec<Vec<[f32; 4]>> = vec![pixels];
    let (mut w, mut h) = (w0, h0);
    while w > 1 || h > 1 {
        let prev = levels.last().expect("level");
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![[0.0; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(w - 1);
                    let sy = (y * 2 + dy).min(h - 1);
                    let p = prev[sy * w + sx];
                    for k in 0..4 {
                        acc[k] += p[k] * 0.25;
                    }
                }
                if renormalize {
                    let n = Vec3::new(acc[0] * 2.0 - 1.0, acc[1] * 2.0 - 1.0, acc[2] * 2.0 - 1.0).normalize_or(Vec3::Z);
                    acc = [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0];
                }
                next[y * nw + x] = acc;
            }
        }
        levels.push(next);
        w = nw;
        h = nh;
    }
    let count = levels.len() as u32;
    let data: Vec<u8> = levels
        .iter()
        .flatten()
        .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
        .collect();
    let mut img = Image::new(
        Extent3d { width: w0 as u32, height: h0 as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0; w0 * h0 * 4],
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(data);
    img.texture_descriptor.mip_level_count = count;
    img.sampler = sampler();
    img
}

fn upload(m: Maps, images: &mut Assets<Image>) -> SurfaceTextures {
    // Colors are authored directly in sRGB.
    let to_srgb = |c: f32| c.clamp(0.0, 1.0);
    let color: Vec<[f32; 4]> = m.color.iter().map(|c| [to_srgb(c.x), to_srgb(c.y), to_srgb(c.z), 1.0]).collect();

    let at = |x: i32, y: i32| m.height[(y.rem_euclid(S as i32) as usize) * S + x.rem_euclid(S as i32) as usize];
    let mut normal = Vec::with_capacity(S * S);
    for y in 0..S as i32 {
        for x in 0..S as i32 {
            let dx = (at(x + 1, y) - at(x - 1, y)) * m.bump;
            let dy = (at(x, y + 1) - at(x, y - 1)) * m.bump;
            let n = Vec3::new(-dx, dy, 1.0).normalize();
            normal.push([n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0]);
        }
    }
    let rm: Vec<[f32; 4]> = m.rough.iter().zip(&m.metal).map(|(r, mt)| [1.0, *r, *mt, 1.0]).collect();

    SurfaceTextures {
        color: images.add(image_with_mips(color, TextureFormat::Rgba8UnormSrgb, false)),
        normal: images.add(image_with_mips(normal, TextureFormat::Rgba8Unorm, true)),
        rough_metal: images.add(image_with_mips(rm, TextureFormat::Rgba8Unorm, false)),
    }
}

pub fn build(surface: Surface, images: &mut Assets<Image>) -> SurfaceTextures {
    let maps = match surface {
        Surface::Stone => stone(),
        Surface::Slate => slate(),
        Surface::Wood => wood(),
        Surface::Iron => iron(),
        Surface::Grass => grass(),
        Surface::Mud => mud(),
        Surface::Wheat => wheat(),
        Surface::Plaster => plaster(),
        Surface::Bark => bark(),
    };
    upload(maps, images)
}

// --- Foliage and roads (with transparency) -------------------------------------

struct Rgba {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Rgba {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0.0; 4]; w * h] }
    }

    fn blend(&mut self, x: i32, y: i32, c: Vec3, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || a <= 0.0 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        let keep = 1.0 - a;
        p[0] = p[0] * keep + c.x * a;
        p[1] = p[1] * keep + c.y * a;
        p[2] = p[2] * keep + c.z * a;
        p[3] = p[3].max(a);
    }

    /// A pointed leaf shape (or needle/blade) from `base` along `angle`.
    fn leaf(&mut self, base: Vec2, angle: f32, len: f32, wid: f32, c: Vec3, rib: bool) {
        let dir = Vec2::from_angle(angle);
        let side = Vec2::new(-dir.y, dir.x);
        let r = len.max(wid) + 2.0;
        for y in (base.y - r) as i32..=(base.y + r) as i32 {
            for x in (base.x - r) as i32..=(base.x + r) as i32 {
                let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - base;
                let t = d.dot(dir) / len;
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let half = wid * 0.5 * (std::f32::consts::PI * t).sin().powf(0.8);
                let across = d.dot(side);
                let cover = (half - across.abs() + 0.7).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                // Lit on one side, darker midrib.
                let mut col = c * (0.85 + 0.25 * (across / wid.max(0.1)).clamp(-0.5, 0.5) * 2.0 * 0.5 + 0.1 * (1.0 - t));
                if rib && across.abs() < 0.9 {
                    col *= 0.75;
                }
                self.blend(x, y, col, cover);
            }
        }
    }

    fn line(&mut self, a: Vec2, b: Vec2, w0: f32, w1: f32, c: Vec3) {
        let ab = b - a;
        let len2 = ab.length_squared().max(0.001);
        let r = w0.max(w1) + 1.0;
        let (minx, maxx) = (a.x.min(b.x) - r, a.x.max(b.x) + r);
        let (miny, maxy) = (a.y.min(b.y) - r, a.y.max(b.y) + r);
        for y in miny as i32..=maxy as i32 {
            for x in minx as i32..=maxx as i32 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
                let w = w0 + (w1 - w0) * t;
                let d = (p - a - ab * t).length();
                let cover = (w * 0.5 - d + 0.6).clamp(0.0, 1.0);
                self.blend(x, y, c, cover);
            }
        }
    }

    fn disc(&mut self, c0: Vec2, r: f32, c: Vec3) {
        for y in (c0.y - r - 1.0) as i32..=(c0.y + r + 1.0) as i32 {
            for x in (c0.x - r - 1.0) as i32..=(c0.x + r + 1.0) as i32 {
                let d = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - c0).length();
                self.blend(x, y, c, (r - d + 0.6).clamp(0.0, 1.0));
            }
        }
    }
}

/// Small deterministic random numbers for painting.
struct Dice(u32);

impl Dice {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// Foliage atlas, 1024x1024 in four 512 cells:
/// top-left broadleaf cluster, top-right lighter broadleaf,
/// bottom-left fir branch, bottom-right grass blades with wildflowers.
pub fn foliage(images: &mut Assets<Image>) -> Handle<Image> {
    let mut c = Rgba::new(1024, 1024);
    let mut d = Dice(12345);
    for (cell, (dark, light)) in [(0usize, (rgb(0.14, 0.24, 0.08), rgb(0.30, 0.42, 0.14))), (1, (rgb(0.22, 0.32, 0.10), rgb(0.42, 0.50, 0.18)))] {
        let origin = Vec2::new(cell as f32 * 512.0, 0.0);
        let center = origin + Vec2::splat(256.0);
        for _ in 0..9 {
            let a = d.range(0.0, std::f32::consts::TAU);
            c.line(center, center + Vec2::from_angle(a) * d.range(120.0, 200.0), 7.0, 2.0, rgb(0.20, 0.14, 0.09));
        }
        for i in 0..230 {
            let a = d.range(0.0, std::f32::consts::TAU);
            let r = d.next().sqrt() * 190.0;
            let p = center + Vec2::from_angle(a) * r;
            let len = d.range(34.0, 56.0);
            let wid = d.range(15.0, 23.0);
            // Brighter toward the top-left, as if lit by the sun.
            let lit = ((-(p - center).x - (p - center).y) / 380.0 + 0.5).clamp(0.0, 1.0);
            let k = d.next() * 0.5 + lit * 0.5 + (i as f32 / 230.0) * 0.2;
            let col = dark.lerp(light, k.clamp(0.0, 1.0));
            c.leaf(p, a + d.range(-0.8, 0.8), len, wid, col, true);
        }
    }
    // Fir branch.
    {
        let origin = Vec2::new(0.0, 512.0);
        let base = origin + Vec2::new(256.0, 495.0);
        let tip = origin + Vec2::new(250.0, 25.0);
        c.line(base, tip, 9.0, 2.0, rgb(0.22, 0.16, 0.10));
        for i in 0..170 {
            let t = i as f32 / 170.0;
            let p = base.lerp(tip, t);
            let len = (1.0 - t * 0.6) * d.range(70.0, 110.0);
            for s in [-1.0f32, 1.0] {
                let a = -std::f32::consts::FRAC_PI_2 + s * d.range(0.9, 1.25);
                let col = rgb(0.07, 0.16, 0.09).lerp(rgb(0.16, 0.28, 0.14), d.next());
                c.leaf(p, a, len, 5.0, col, false);
            }
        }
    }
    // Grass blades with a few wildflowers.
    {
        let origin = Vec2::new(512.0, 512.0);
        for _ in 0..260 {
            let x = d.range(40.0, 472.0);
            let base = origin + Vec2::new(x, 506.0);
            let h = d.range(160.0, 440.0);
            let bend = d.range(-0.35, 0.35);
            let a = -std::f32::consts::FRAC_PI_2 + bend;
            let col = rgb(0.16, 0.28, 0.08).lerp(rgb(0.45, 0.48, 0.18), d.next());
            c.leaf(base, a, h, d.range(6.0, 11.0), col, false);
        }
        for _ in 0..12 {
            let x = d.range(60.0, 452.0);
            let top = origin + Vec2::new(x, d.range(120.0, 300.0));
            c.line(top, Vec2::new(top.x, origin.y + 506.0), 3.0, 3.0, rgb(0.18, 0.30, 0.10));
            let petal = match (d.next() * 3.0) as i32 {
                0 => rgb(0.95, 0.94, 0.88),
                1 => rgb(0.96, 0.80, 0.22),
                _ => rgb(0.58, 0.44, 0.80),
            };
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                c.disc(top + Vec2::from_angle(a) * 9.0, 8.0, petal);
            }
            c.disc(top, 6.0, rgb(0.85, 0.65, 0.15));
        }
    }
    let mut img = image_with_mips_sized(c.px, 1024, 1024, TextureFormat::Rgba8UnormSrgb, false);
    // Keep atlas cells from bleeding into each other.
    if let ImageSampler::Descriptor(s) = &mut img.sampler {
        s.address_mode_u = ImageAddressMode::ClampToEdge;
        s.address_mode_v = ImageAddressMode::ClampToEdge;
    }
    images.add(img)
}

/// Dirt road: soft grassy edges, two wheel ruts and a grass strip in the middle.
/// U runs across the road (0..1), V along it (tiles).
pub fn road(images: &mut Assets<Image>) -> Handle<Image> {
    let (w, h) = (256usize, 512usize);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let n = fbm(u * 0.25, v, 8, 4, 121);
            let fine = fbm(u * 0.5, v, 64, 2, 123);
            let edge = 0.12 + n * 0.12;
            let alpha = smooth(0.0, edge, u) * smooth(0.0, edge, 1.0 - u);
            let rut = (1.0 - ((u - 0.3).abs() / 0.07).min(1.0)).max(1.0 - ((u - 0.7).abs() / 0.07).min(1.0));
            let middle = 1.0 - ((u - 0.5).abs() / 0.09).min(1.0);
            let mut c = rgb(0.42, 0.33, 0.22) * (0.75 + fine * 0.45);
            c = c.lerp(rgb(0.26, 0.19, 0.13), rut * 0.7);
            c = c.lerp(rgb(0.30, 0.36, 0.16) * (0.7 + fine * 0.5), middle * 0.55 * smooth(0.4, 0.6, n));
            let pebble = smooth(0.8, 0.86, vnoise(u * 0.5, v, 96, 125));
            c = c.lerp(rgb(0.62, 0.58, 0.52), pebble * 0.6);
            px.push([c.x, c.y, c.z, alpha]);
        }
    }
    let mut img = image_with_mips_sized(px, w, h, TextureFormat::Rgba8UnormSrgb, false);
    if let ImageSampler::Descriptor(s) = &mut img.sampler {
        s.address_mode_u = ImageAddressMode::ClampToEdge;
    }
    images.add(img)
}
