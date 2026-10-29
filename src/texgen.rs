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

/// Meadow grass seen from above: blades, clover and bare specks.
fn grass() -> Maps {
    let mut m = Maps::new(2.5);
    each(&mut m, |u, v, c, h, r, _| {
        let blades = fbm(u, v, 128, 2, 71);
        let clumps = fbm(u, v, 24, 3, 73);
        let large = fbm(u, v, 4, 3, 75);
        let bare = smooth(0.7, 0.85, fbm(u, v, 10, 3, 77));
        let green = rgb(0.24, 0.33, 0.12).lerp(rgb(0.36, 0.42, 0.17), large);
        let g = green * (0.65 + blades * 0.55) * (0.85 + clumps * 0.3);
        *c = g.lerp(rgb(0.36, 0.30, 0.20) * (0.8 + blades * 0.3), bare * 0.6);
        *h = blades * 0.7 + clumps * 0.3;
        *r = 0.88 + blades * 0.1;
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
    let mut levels: Vec<Vec<[f32; 4]>> = vec![pixels];
    let mut size = S;
    while size > 1 {
        let prev = levels.last().expect("level");
        let half = size / 2;
        let mut next = vec![[0.0; 4]; half * half];
        for y in 0..half {
            for x in 0..half {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(y * 2 + dy) * size + x * 2 + dx];
                    for k in 0..4 {
                        acc[k] += p[k] * 0.25;
                    }
                }
                if renormalize {
                    let n = Vec3::new(acc[0] * 2.0 - 1.0, acc[1] * 2.0 - 1.0, acc[2] * 2.0 - 1.0).normalize_or(Vec3::Z);
                    acc = [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0];
                }
                next[y * half + x] = acc;
            }
        }
        levels.push(next);
        size = half;
    }
    let count = levels.len() as u32;
    let data: Vec<u8> = levels
        .iter()
        .flatten()
        .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
        .collect();
    let mut img = Image::new(
        Extent3d { width: S as u32, height: S as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0; S * S * 4],
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
    };
    upload(maps, images)
}
