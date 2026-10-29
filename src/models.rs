//! Every 3D model in the game, built in code from textured shapes.
//!
//! A `Model` collects geometry per material. Architecture uses the generated
//! stone, slate, wood and iron textures (UVs are in meters, so detail stays
//! the same size on every surface). Small or numerous things like troops use
//! one vertex-colored material so thousands of them stay cheap to draw.

use crate::art::{castle_mounts, rgb, Variant};
use crate::items::ModuleKind;
use crate::texgen::Surface;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mat {
    Stone,
    Slate,
    Wood,
    Iron,
    Plaster,
    Wheat,
    Glass,
    Gold,
    /// Vertex-colored, for troops and small details.
    Paint,
    /// Vertex-colored foliage.
    Leaf,
    /// Vertex-colored, two-sided (flags and banners).
    Cloth,
    /// Leaf, needle and grass cards from the foliage atlas (cut-out, two-sided).
    Foliage,
    Bark,
    /// Polished cast bronze (cannons, spyglass).
    Bronze,
}

impl Mat {
    pub const ALL: [Mat; 14] = [
        Mat::Foliage,
        Mat::Bark,
        Mat::Bronze,
        Mat::Stone,
        Mat::Slate,
        Mat::Wood,
        Mat::Iron,
        Mat::Plaster,
        Mat::Wheat,
        Mat::Glass,
        Mat::Gold,
        Mat::Paint,
        Mat::Leaf,
        Mat::Cloth,
    ];

    pub fn surface(self) -> Option<Surface> {
        match self {
            Mat::Stone => Some(Surface::Stone),
            Mat::Slate => Some(Surface::Slate),
            Mat::Wood => Some(Surface::Wood),
            Mat::Iron => Some(Surface::Iron),
            Mat::Plaster => Some(Surface::Plaster),
            Mat::Wheat => Some(Surface::Wheat),
            Mat::Bark => Some(Surface::Bark),
            _ => None,
        }
    }
}

#[derive(Default)]
struct Part {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Part {
    fn vert(&mut self, p: Vec3, n: Vec3, uv: Vec2, c: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.to_array());
        self.uv.push(uv.to_array());
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }
}

/// Darker or lighter version of a color.
pub fn sh(c: Color, k: f32) -> Color {
    let l = c.to_linear();
    Color::LinearRgba(LinearRgba::new(l.red * k, l.green * k, l.blue * k, l.alpha))
}

fn lin(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

#[derive(Default)]
pub struct Model {
    parts: HashMap<Mat, Part>,
}

const W: Color = Color::WHITE;

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    fn part(&mut self, m: Mat) -> &mut Part {
        self.parts.entry(m).or_default()
    }

    /// A flat polygon (convex, 3+ corners) facing `normal`. Winding is fixed automatically.
    pub fn poly(&mut self, mat: Mat, color: Color, pts: &[Vec3], uvs: &[Vec2], normal: Vec3) {
        let n = normal.normalize_or(Vec3::Y);
        let geo = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
        let flip = geo.dot(n) < 0.0;
        let c = lin(color);
        let part = self.part(mat);
        let base: Vec<u32> = pts.iter().zip(uvs).map(|(p, uv)| part.vert(*p, n, *uv, c)).collect();
        for i in 1..pts.len() - 1 {
            if flip {
                part.idx.extend_from_slice(&[base[0], base[i + 1], base[i]]);
            } else {
                part.idx.extend_from_slice(&[base[0], base[i], base[i + 1]]);
            }
        }
    }

    /// Box of `size` centered at `center`, rotated by `rot`. UVs in meters.
    pub fn cube(&mut self, mat: Mat, size: Vec3, center: Vec3, rot: Quat, color: Color) {
        let h = size / 2.0;
        for n in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            let t2 = if n.y.abs() > 0.5 { Vec3::Z } else { Vec3::Y };
            let t1 = t2.cross(n);
            let hn = (n * h).length();
            let h1 = (t1 * h).length();
            let h2 = (t2 * h).length();
            let corners = [(-h1, -h2), (h1, -h2), (h1, h2), (-h1, h2)];
            let pts: Vec<Vec3> = corners.iter().map(|(a, b)| center + rot * (n * hn + t1 * *a + t2 * *b)).collect();
            let uvs: Vec<Vec2> = corners.iter().map(|(a, b)| Vec2::new(a + h1, h2 - b)).collect();
            self.poly(mat, color, &pts, &uvs, rot * n);
        }
    }

    pub fn boxy(&mut self, mat: Mat, size: Vec3, center: Vec3, color: Color) {
        self.cube(mat, size, center, Quat::IDENTITY, color);
    }

    /// Cone/cylinder/frustum standing on `base`, along `rot * Y`.
    #[allow(clippy::too_many_arguments)]
    pub fn frustum_rot(
        &mut self,
        mat: Mat,
        base: Vec3,
        rot: Quat,
        rb: f32,
        rt: f32,
        h: f32,
        seg: u32,
        color: Color,
        smooth: bool,
        cap: bool,
    ) {
        let c = lin(color);
        let slant = Vec2::new(rb - rt, h).length();
        let avg = (rb + rt) / 2.0;
        let ring = |r: f32, a: f32, y: f32| Vec3::new(a.cos() * r, y, a.sin() * r);
        let nrm = |a: f32| Vec3::new(a.cos() * h, rb - rt, a.sin() * h).normalize();
        if smooth {
            let part = self.part(mat);
            let start = part.pos.len() as u32;
            for i in 0..=seg {
                let a = i as f32 / seg as f32 * TAU;
                let n = rot * nrm(a);
                let u = a * avg;
                part.vert(base + rot * ring(rb, a, 0.0), n, Vec2::new(u, slant), c);
                part.vert(base + rot * ring(rt, a, h), n, Vec2::new(u, 0.0), c);
            }
            for i in 0..seg {
                let (b0, t0, b1, t1) = (start + i * 2, start + i * 2 + 1, start + i * 2 + 2, start + i * 2 + 3);
                part.idx.extend_from_slice(&[b0, t0, t1, b0, t1, b1]);
            }
        } else {
            for i in 0..seg {
                let a0 = i as f32 / seg as f32 * TAU;
                let a1 = (i + 1) as f32 / seg as f32 * TAU;
                let n = rot * nrm((a0 + a1) / 2.0);
                let w = 2.0 * (a1 - a0).sin() / 2.0 * rb;
                let wt = 2.0 * (a1 - a0).sin() / 2.0 * rt;
                if rt > 0.001 {
                    let pts = [ring(rb, a0, 0.0), ring(rb, a1, 0.0), ring(rt, a1, h), ring(rt, a0, h)].map(|p| base + rot * p);
                    let uvs = [Vec2::new(0.0, slant), Vec2::new(w, slant), Vec2::new((w + wt) / 2.0, 0.0), Vec2::new((w - wt) / 2.0, 0.0)];
                    self.poly(mat, color, &pts, &uvs, n);
                } else {
                    let pts = [ring(rb, a0, 0.0), ring(rb, a1, 0.0), Vec3::new(0.0, h, 0.0)].map(|p| base + rot * p);
                    let uvs = [Vec2::new(0.0, slant), Vec2::new(w, slant), Vec2::new(w / 2.0, 0.0)];
                    self.poly(mat, color, &pts, &uvs, n);
                }
            }
        }
        if cap && rt > 0.001 {
            let pts: Vec<Vec3> = (0..seg).map(|i| base + rot * ring(rt, i as f32 / seg as f32 * TAU, h)).collect();
            let uvs: Vec<Vec2> = (0..seg).map(|i| {
                let a = i as f32 / seg as f32 * TAU;
                Vec2::new(a.cos() * rt, a.sin() * rt)
            }).collect();
            self.poly(mat, color, &pts, &uvs, rot * Vec3::Y);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frustum(&mut self, mat: Mat, base: Vec3, rb: f32, rt: f32, h: f32, seg: u32, color: Color, smooth: bool) {
        self.frustum_rot(mat, base, Quat::IDENTITY, rb, rt, h, seg, color, smooth, true);
    }

    /// Gable roof along local X with stone gable ends.
    #[allow(clippy::too_many_arguments)]
    pub fn roof(&mut self, base: Vec3, rot: Quat, len: f32, wid: f32, h: f32, overhang: f32, roof_c: Color, wall_mat: Mat, wall_c: Color, roof_mat: Mat) {
        let (hl, hw) = (len / 2.0, wid / 2.0);
        let (ol, ow) = (hl + overhang, hw + overhang);
        let drop = h * overhang / hw;
        let slope = Vec2::new(ow, h + drop).length();
        for s in [-1.0f32, 1.0] {
            let pts = [
                Vec3::new(-ol, -drop, s * ow),
                Vec3::new(ol, -drop, s * ow),
                Vec3::new(ol, h, 0.0),
                Vec3::new(-ol, h, 0.0),
            ]
            .map(|p| base + rot * p);
            let uvs = [Vec2::new(0.0, slope), Vec2::new(2.0 * ol, slope), Vec2::new(2.0 * ol, 0.0), Vec2::new(0.0, 0.0)];
            let n = rot * Vec3::new(0.0, ow, s * (h + drop)).normalize();
            self.poly(roof_mat, roof_c, &pts, &uvs, n);
            // Underside so the roof never looks hollow from below.
            self.poly(roof_mat, sh(roof_c, 0.4), &pts, &uvs, -n);
        }
        for s in [-1.0f32, 1.0] {
            let pts = [Vec3::new(s * hl, 0.0, -hw), Vec3::new(s * hl, 0.0, hw), Vec3::new(s * hl, h, 0.0)].map(|p| base + rot * p);
            let uvs = [Vec2::new(0.0, h), Vec2::new(wid, h), Vec2::new(hw, 0.0)];
            self.poly(wall_mat, wall_c, &pts, &uvs, rot * Vec3::X * s);
        }
    }

    /// Lumpy sphere for foliage, rocks and wool. Smooth shaded, darker underneath.
    #[allow(clippy::too_many_arguments)]
    pub fn blob(&mut self, mat: Mat, center: Vec3, r: f32, scale: Vec3, color: Color, amp: f32, seed: u32, subdiv: u32) {
        let mesh = Sphere::new(1.0).mesh().ico(subdiv).expect("ico");
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(src)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            return;
        };
        let Some(Indices::U32(src_idx)) = mesh.indices() else {
            return;
        };
        let pos: Vec<Vec3> = src
            .iter()
            .map(|p| {
                let d = Vec3::from_array(*p);
                let bump = 1.0 + amp * (noise3(d * 2.3, seed) - 0.5) * 2.0 + amp * 0.5 * (noise3(d * 5.1, seed + 7) - 0.5);
                d * r * bump * scale
            })
            .collect();
        let mut nrm = vec![Vec3::ZERO; pos.len()];
        for t in src_idx.chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let fnrm = (pos[b] - pos[a]).cross(pos[c] - pos[a]);
            for i in [a, b, c] {
                nrm[i] += fnrm;
            }
        }
        let part = self.part(mat);
        let start = part.pos.len() as u32;
        let base = color.to_linear();
        for (p, n) in pos.iter().zip(&nrm) {
            let shade = 0.55 + 0.45 * (p.y / (r * scale.y)).clamp(-1.0, 1.0) * 0.5 + 0.225;
            let c = LinearRgba::new(base.red * shade, base.green * shade, base.blue * shade, 1.0);
            part.vert(center + *p, n.normalize_or(Vec3::Y), Vec2::new(p.x, p.z), c.to_f32_array());
        }
        for t in src_idx.chunks(3) {
            part.idx.extend_from_slice(&[start + t[0], start + t[1], start + t[2]]);
        }
    }

    /// A flat card (quad) around `center`, spanning `right` and `up` (half extents),
    /// textured with an atlas region. Normals point away from `light_from`
    /// when given, which makes leafy crowns shade softly like a whole canopy.
    #[allow(clippy::too_many_arguments)]
    pub fn card(&mut self, mat: Mat, center: Vec3, right: Vec3, up: Vec3, uv_min: Vec2, uv_max: Vec2, color: Color, light_from: Option<Vec3>) {
        let c = lin(color);
        let flat = right.cross(up).normalize_or(Vec3::Y);
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let part = self.part(mat);
        let start = part.pos.len() as u32;
        for (a, b) in corners {
            let p = center + right * a + up * b;
            let n = match light_from {
                Some(o) => ((p - o).normalize_or(Vec3::Y) + Vec3::Y * 0.35).normalize(),
                None => flat,
            };
            let uv = Vec2::new(if a < 0.0 { uv_min.x } else { uv_max.x }, if b < 0.0 { uv_max.y } else { uv_min.y });
            part.vert(p, n, uv, c);
        }
        part.idx.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    /// Adds another model's geometry, moved by `tf`.
    pub fn append(&mut self, other: Model, tf: Transform) {
        for (mat, src) in other.parts {
            let part = self.part(mat);
            let off = part.pos.len() as u32;
            for i in 0..src.pos.len() {
                part.pos.push(tf.transform_point(Vec3::from_array(src.pos[i])).to_array());
                part.nrm.push((tf.rotation * Vec3::from_array(src.nrm[i])).to_array());
                part.uv.push(src.uv[i]);
                part.col.push(src.col[i]);
            }
            part.idx.extend(src.idx.iter().map(|i| i + off));
        }
    }

    /// One mesh per material, ready to upload.
    pub fn build(self) -> Vec<(Mat, Mesh)> {
        let mut out: Vec<(Mat, Mesh)> = Vec::new();
        for mat in Mat::ALL {
            let Some(p) = self.parts.get(&mat) else { continue };
            if p.idx.is_empty() {
                continue;
            }
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p.pos.clone())
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, p.nrm.clone())
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uv.clone())
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, p.col.clone())
                .with_inserted_indices(Indices::U32(p.idx.clone()));
            if mat.surface().is_some() {
                let _ = mesh.generate_tangents();
            }
            out.push((mat, mesh));
        }
        out
    }

    /// Everything merged into one mesh (for single-material models like troops).
    pub fn build_single(self) -> Mesh {
        let mut all = Part::default();
        for mat in Mat::ALL {
            if let Some(p) = self.parts.get(&mat) {
                let off = all.pos.len() as u32;
                all.pos.extend_from_slice(&p.pos);
                all.nrm.extend_from_slice(&p.nrm);
                all.uv.extend_from_slice(&p.uv);
                all.col.extend_from_slice(&p.col);
                all.idx.extend(p.idx.iter().map(|i| i + off));
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, all.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, all.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, all.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, all.col)
            .with_inserted_indices(Indices::U32(all.idx))
    }
}

/// Tiny deterministic random numbers for placing leaves and props.
pub struct Rand(pub u32);

impl Rand {
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn dir(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, TAU);
        let r = (1.0 - z * z).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }
}

/// Atlas cells in the foliage texture.
const LEAF_A: (Vec2, Vec2) = (Vec2::new(0.0, 0.0), Vec2::new(0.5, 0.5));
const LEAF_B: (Vec2, Vec2) = (Vec2::new(0.5, 0.0), Vec2::new(1.0, 0.5));
const FIR: (Vec2, Vec2) = (Vec2::new(0.0, 0.5), Vec2::new(0.5, 1.0));
const GRASS: (Vec2, Vec2) = (Vec2::new(0.5, 0.5), Vec2::new(1.0, 1.0));

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f) ^ seed.wrapping_mul(0x1656_67b1);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn noise3(p: Vec3, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = (if dx == 1 { s.x } else { 1.0 - s.x }) * (if dy == 1 { s.y } else { 1.0 - s.y }) * (if dz == 1 { s.z } else { 1.0 - s.z });
                acc += w * hash3(x + dx, y + dy, z + dz, seed);
            }
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

fn stone_tint() -> Color {
    rgb(1.0, 0.97, 0.92)
}
fn slate_tint() -> Color {
    rgb(0.9, 0.95, 1.0)
}
fn skin() -> Color {
    rgb(0.72, 0.53, 0.42)
}
fn mail() -> Color {
    rgb(0.40, 0.41, 0.43)
}
fn steel() -> Color {
    rgb(0.55, 0.56, 0.58)
}
fn leather() -> Color {
    rgb(0.30, 0.20, 0.13)
}
fn timber() -> Color {
    rgb(0.33, 0.23, 0.15)
}
fn crimson() -> Color {
    rgb(0.45, 0.07, 0.07)
}
fn royal() -> Color {
    rgb(0.12, 0.20, 0.42)
}

// ---------------------------------------------------------------------------
// Gothic building blocks
// ---------------------------------------------------------------------------

/// Pointed (lancet) window set into a wall facing `out`.
fn window(m: &mut Model, center: Vec3, out: Vec3, w: f32, h: f32) {
    let rot = Quat::from_rotation_arc(Vec3::Z, out.normalize());
    let at = |x: f32, y: f32| center + rot * Vec3::new(x, y, 0.03);
    let hw = w / 2.0;
    let pts = [at(-hw, -h / 2.0), at(hw, -h / 2.0), at(hw, h / 2.0), at(-hw, h / 2.0)];
    m.poly(Mat::Glass, W, &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], out);
    let arch = [at(-hw, h / 2.0), at(hw, h / 2.0), at(0.0, h / 2.0 + w * 0.75)];
    m.poly(Mat::Glass, W, &arch, &[Vec2::ZERO, Vec2::X, Vec2::Y], out);
    // Stone sill and mullion.
    m.cube(Mat::Stone, Vec3::new(w + 0.16, 0.1, 0.14), center + rot * Vec3::new(0.0, -h / 2.0 - 0.05, 0.06), rot, stone_tint());
    m.cube(Mat::Stone, Vec3::new(0.05, h, 0.06), center + rot * Vec3::new(0.0, 0.0, 0.05), rot, stone_tint());
}

/// Merlons along a wall top from `a` to `b`.
fn battlements(m: &mut Model, a: Vec3, b: Vec3, thick: f32) {
    let d = b - a;
    let len = d.length();
    let dir = d / len;
    let rot = Quat::from_rotation_arc(Vec3::X, dir);
    let n = (len / 0.95).floor().max(1.0) as i32;
    let step = len / n as f32;
    for i in 0..n {
        let c = a + dir * (step * (i as f32 + 0.5)) + Vec3::Y * 0.27;
        m.cube(Mat::Stone, Vec3::new(step * 0.55, 0.55, thick), c, rot, stone_tint());
    }
}

/// Curtain wall segment with battlements, standing on `y0`.
fn wall(m: &mut Model, a: Vec2, b: Vec2, y0: f32, h: f32, thick: f32) {
    let mid = (a + b) / 2.0;
    let d = b - a;
    let len = d.length();
    let rot = Quat::from_rotation_y(-d.y.atan2(d.x));
    m.cube(Mat::Stone, Vec3::new(len, h, thick), Vec3::new(mid.x, y0 + h / 2.0, mid.y), rot, stone_tint());
    let top = y0 + h;
    let edge = Vec3::new(0.0, 0.0, 0.0);
    let pa = Vec3::new(a.x, top, a.y) + edge;
    let pb = Vec3::new(b.x, top, b.y) + edge;
    let side = rot * Vec3::Z * (thick / 2.0 - 0.1);
    battlements(m, pa + side, pb + side, 0.22);
    battlements(m, pa - side, pb - side, 0.22);
}

/// Round tower with a corbelled parapet and a steep slate spire.
fn round_tower(m: &mut Model, base: Vec3, r: f32, h: f32, spire: f32, flag: Option<Color>) {
    m.frustum(Mat::Stone, base, r * 1.12, r * 1.06, 0.4, 16, stone_tint(), true);
    m.frustum(Mat::Stone, base + Vec3::Y * 0.4, r * 1.04, r, h - 0.4, 18, stone_tint(), true);
    // Corbel ring and parapet.
    m.frustum(Mat::Stone, base + Vec3::Y * (h - 0.5), r, r * 1.16, 0.3, 18, sh(stone_tint(), 0.9), true);
    m.frustum(Mat::Stone, base + Vec3::Y * (h - 0.2), r * 1.16, r * 1.16, 0.45, 18, stone_tint(), true);
    // Arrow slits.
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.4;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        for y in [h * 0.35, h * 0.68] {
            window(m, base + Vec3::new(0.0, y, 0.0) + out * (r * 1.02), out, 0.16, 0.5);
        }
    }
    let top = base + Vec3::Y * (h + 0.25);
    if spire > 0.0 {
        m.frustum(Mat::Slate, top, r * 1.28, 0.0, spire, 10, slate_tint(), false);
        let tip = top + Vec3::Y * spire;
        m.frustum(Mat::Gold, tip - Vec3::Y * 0.1, 0.07, 0.0, 0.7, 6, W, true);
        if let Some(c) = flag {
            flag_at(m, tip + Vec3::Y * 0.2, c);
        }
    } else {
        for k in 0..8 {
            let a = k as f32 / 8.0 * TAU;
            let p = top + Vec3::new(a.cos(), 0.0, a.sin()) * (r * 1.06) + Vec3::Y * 0.4;
            m.cube(Mat::Stone, Vec3::new(0.35, 0.5, 0.35), p, Quat::from_rotation_y(-a), stone_tint());
        }
    }
}

/// Square tower with string courses, corner pinnacles and an octagonal spire.
fn square_tower(m: &mut Model, base: Vec3, w: f32, h: f32, spire: f32, clock: bool, flag: Option<Color>) {
    m.boxy(Mat::Stone, Vec3::new(w + 0.2, 0.5, w + 0.2), base + Vec3::Y * 0.25, stone_tint());
    m.boxy(Mat::Stone, Vec3::new(w, h, w), base + Vec3::Y * (h / 2.0), stone_tint());
    let mut y = 2.6;
    while y < h - 0.5 {
        m.boxy(Mat::Stone, Vec3::new(w + 0.14, 0.14, w + 0.14), base + Vec3::Y * y, sh(stone_tint(), 0.88));
        for out in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
            window(m, base + Vec3::Y * (y + 1.0) + out * (w / 2.0), out, 0.32, 0.9);
        }
        y += 2.6;
    }
    if clock {
        let c = base + Vec3::new(0.0, h - 1.1, w / 2.0 + 0.04);
        m.frustum_rot(Mat::Gold, c, Quat::from_rotation_x(FRAC_PI_2), 0.6, 0.6, 0.06, 24, W, true, true);
        m.frustum_rot(Mat::Iron, c + Vec3::Z * 0.06, Quat::from_rotation_x(FRAC_PI_2), 0.48, 0.48, 0.03, 24, W, true, true);
    }
    let top = base + Vec3::Y * h;
    battlements(m, top + Vec3::new(-w / 2.0, 0.0, w / 2.0 - 0.1), top + Vec3::new(w / 2.0, 0.0, w / 2.0 - 0.1), 0.2);
    battlements(m, top + Vec3::new(-w / 2.0, 0.0, -w / 2.0 + 0.1), top + Vec3::new(w / 2.0, 0.0, -w / 2.0 + 0.1), 0.2);
    for (x, z) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let p = top + Vec3::new(x * (w / 2.0 - 0.18), 0.0, z * (w / 2.0 - 0.18));
        m.boxy(Mat::Stone, Vec3::new(0.32, 0.6, 0.32), p + Vec3::Y * 0.3, stone_tint());
        m.frustum(Mat::Slate, p + Vec3::Y * 0.6, 0.22, 0.0, spire * 0.3, 4, slate_tint(), false);
    }
    if spire > 0.0 {
        m.frustum(Mat::Slate, top, w * 0.48, 0.0, spire, 8, slate_tint(), false);
        let tip = top + Vec3::Y * spire;
        m.frustum(Mat::Gold, tip - Vec3::Y * 0.1, 0.08, 0.0, 0.9, 6, W, true);
        if let Some(c) = flag {
            flag_at(m, tip + Vec3::Y * 0.3, c);
        }
    }
}

/// Great hall: tall walls, buttresses, lancet windows and a steep slate roof.
fn hall(m: &mut Model, center: Vec3, len: f32, wid: f32, wall_h: f32, roof_h: f32) {
    m.boxy(Mat::Stone, Vec3::new(len, wall_h, wid), center + Vec3::Y * (wall_h / 2.0), stone_tint());
    m.roof(center + Vec3::Y * wall_h, Quat::IDENTITY, len, wid, roof_h, 0.3, slate_tint(), Mat::Stone, stone_tint(), Mat::Slate);
    let n = (len / 1.3).floor().max(2.0) as i32;
    let step = len / n as f32;
    for i in 0..=n {
        let x = center.x - len / 2.0 + i as f32 * step;
        for s in [-1.0f32, 1.0] {
            let z = center.z + s * (wid / 2.0 + 0.22);
            m.boxy(Mat::Stone, Vec3::new(0.38, wall_h * 0.85, 0.44), Vec3::new(x, center.y + wall_h * 0.425, z), sh(stone_tint(), 0.95));
            m.cube(
                Mat::Stone,
                Vec3::new(0.34, 0.9, 0.3),
                Vec3::new(x, center.y + wall_h * 0.85 + 0.3, z - s * 0.08),
                Quat::from_rotation_x(s * 0.5),
                sh(stone_tint(), 0.95),
            );
            if i < n {
                let wx = x + step / 2.0;
                window(m, Vec3::new(wx, center.y + wall_h * 0.5, center.z + s * wid / 2.0), Vec3::Z * s, 0.42, wall_h * 0.45);
            }
        }
    }
    // Rose window on the front gable.
    let gable = Vec3::new(center.x + len / 2.0 + 0.02, center.y + wall_h + roof_h * 0.35, center.z);
    m.frustum_rot(Mat::Glass, gable, Quat::from_rotation_z(-FRAC_PI_2), 0.55, 0.55, 0.04, 12, W, true, true);
    m.frustum_rot(Mat::Stone, gable - Vec3::X * 0.02, Quat::from_rotation_z(-FRAC_PI_2), 0.68, 0.68, 0.03, 12, stone_tint(), true, true);
}

fn flag_at(m: &mut Model, top: Vec3, c: Color) {
    m.frustum(Mat::Iron, top - Vec3::Y * 0.4, 0.03, 0.03, 1.6, 6, W, true);
    let p = top + Vec3::Y * 0.8;
    let pts = [p, p + Vec3::new(1.1, -0.1, 0.0), p + Vec3::new(1.0, -0.55, 0.05), p + Vec3::new(0.0, -0.6, 0.0)];
    m.poly(Mat::Cloth, c, &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], Vec3::Z);
}

/// Solid wooden wheel with an iron tire, axle along Z.
fn wheel(m: &mut Model, center: Vec3, r: f32, w: f32) {
    let rot = Quat::from_rotation_x(FRAC_PI_2);
    let base = center - Vec3::Z * (w / 2.0);
    m.frustum_rot(Mat::Wood, base, rot, r * 0.93, r * 0.93, w, 16, W, true, true);
    m.frustum_rot(Mat::Iron, base - Vec3::Z * 0.02, rot, r, r, w + 0.04, 20, W, true, false);
    m.frustum_rot(Mat::Iron, base - Vec3::Z * 0.08, rot, r * 0.22, r * 0.22, w + 0.16, 8, W, true, true);
    // Back face of the disc.
    m.frustum_rot(Mat::Wood, base, Quat::from_rotation_x(-FRAC_PI_2), r * 0.93, r * 0.93, 0.01, 16, W, true, true);
}

/// Heavy timber platform with iron bands and wheels.
fn chassis(m: &mut Model, len: f32, wid: f32, top: f32, thick: f32, wheel_x: &[f32], wheel_r: f32) {
    m.boxy(Mat::Wood, Vec3::new(len, thick, wid), Vec3::new(0.0, top - thick / 2.0, 0.0), W);
    let mut x = -len / 2.0 + 0.6;
    while x < len / 2.0 - 0.3 {
        m.boxy(Mat::Iron, Vec3::new(0.16, thick + 0.04, wid + 0.04), Vec3::new(x, top - thick / 2.0, 0.0), W);
        x += 1.8;
    }
    for &wx in wheel_x {
        for s in [-1.0f32, 1.0] {
            wheel(m, Vec3::new(wx, wheel_r, s * (wid / 2.0 + 0.3)), wheel_r, 0.42);
        }
    }
    m.frustum_rot(Mat::Iron, Vec3::new(0.0, wheel_r, -wid / 2.0 - 0.3), Quat::from_rotation_x(FRAC_PI_2), 0.08, 0.08, wid + 0.6, 6, W, true, false);
}

// ---------------------------------------------------------------------------
// Player castle
// ---------------------------------------------------------------------------

pub fn castle(stage: usize, variant: Variant) -> Model {
    let mut m = Model::new();
    let mo = castle_mounts(stage);
    let (len, wid) = (mo.len, mo.wid);
    let big_wheels = variant == Variant::Movement;
    match stage {
        0 => {
            let top = 1.35;
            chassis(&mut m, len, wid - 0.2, top, 0.7, &[-2.0, 2.0], if big_wheels { 1.1 } else { 0.95 });
            // Keep.
            m.boxy(Mat::Stone, Vec3::new(3.2, 0.4, 2.8), Vec3::new(0.3, top + 0.2, 0.0), stone_tint());
            m.boxy(Mat::Stone, Vec3::new(3.0, 3.4, 2.6), Vec3::new(0.3, top + 1.7, 0.0), stone_tint());
            m.boxy(Mat::Stone, Vec3::new(3.12, 0.14, 2.72), Vec3::new(0.3, top + 2.3, 0.0), sh(stone_tint(), 0.88));
            let kt = top + 3.4;
            battlements(&mut m, Vec3::new(-1.15, kt, 1.2), Vec3::new(1.75, kt, 1.2), 0.2);
            battlements(&mut m, Vec3::new(-1.15, kt, -1.2), Vec3::new(1.75, kt, -1.2), 0.2);
            battlements(&mut m, Vec3::new(1.7, kt, -1.1), Vec3::new(1.7, kt, 1.1), 0.2);
            for s in [-1.0f32, 1.0] {
                for x in [-0.4, 1.0] {
                    window(&mut m, Vec3::new(x, top + 1.3, s * 1.3), Vec3::Z * s, 0.34, 0.8);
                    window(&mut m, Vec3::new(x, top + 2.9, s * 1.3), Vec3::Z * s, 0.3, 0.6);
                }
            }
            // Gate with a wooden door.
            m.boxy(Mat::Wood, Vec3::new(0.06, 1.3, 0.9), Vec3::new(1.82, top + 0.65, 0.0), timber());
            m.boxy(Mat::Stone, Vec3::new(0.12, 0.2, 1.2), Vec3::new(1.84, top + 1.4, 0.0), sh(stone_tint(), 0.9));
            round_tower(&mut m, Vec3::new(-1.95, top, 1.15), 0.75, 5.4, 3.4, Some(royal()));
            round_tower(&mut m, Vec3::new(-1.15, top, -1.3), 0.42, 4.5, 2.1, None);
            m.boxy(Mat::Stone, Vec3::new(0.38, 1.0, 0.38), Vec3::new(1.2, kt + 0.5, -0.9), sh(stone_tint(), 0.9));
            m.boxy(Mat::Stone, Vec3::new(0.48, 0.1, 0.48), Vec3::new(1.2, kt + 1.02, -0.9), sh(stone_tint(), 0.8));
        }
        1 => {
            let top = 1.45;
            chassis(&mut m, len, wid - 0.2, top, 0.75, &[-3.1, 0.0, 3.1], if big_wheels { 1.3 } else { 1.1 });
            let (hx, hz) = (4.45, 2.85);
            let (y0, h) = (top, 2.3);
            wall(&mut m, Vec2::new(-hx, -hz), Vec2::new(hx, -hz), y0, h, 0.45);
            wall(&mut m, Vec2::new(-hx, hz), Vec2::new(hx, hz), y0, h, 0.45);
            wall(&mut m, Vec2::new(-hx, -hz), Vec2::new(-hx, hz), y0, h, 0.45);
            wall(&mut m, Vec2::new(hx, -hz), Vec2::new(hx, -1.0), y0, h, 0.45);
            wall(&mut m, Vec2::new(hx, 1.0), Vec2::new(hx, hz), y0, h, 0.45);
            // Gatehouse with a portcullis.
            m.boxy(Mat::Stone, Vec3::new(0.6, 0.7, 2.2), Vec3::new(hx, y0 + h - 0.35, 0.0), stone_tint());
            for z in [-0.6, -0.2, 0.2, 0.6] {
                m.boxy(Mat::Iron, Vec3::new(0.06, 1.6, 0.06), Vec3::new(hx + 0.1, y0 + 0.8, z), W);
            }
            for y in [0.4, 0.9, 1.4] {
                m.boxy(Mat::Iron, Vec3::new(0.06, 0.06, 1.6), Vec3::new(hx + 0.1, y0 + y, 0.0), W);
            }
            // Fighting platform behind the gate.
            m.boxy(Mat::Wood, Vec3::new(1.4, 0.2, 2.4), Vec3::new(3.65, top + h - 0.1, 0.0), W);
            for (x, z) in [(3.1, -1.0), (3.1, 1.0)] {
                m.boxy(Mat::Wood, Vec3::new(0.2, h, 0.2), Vec3::new(x, top + h / 2.0, z), W);
            }
            hall(&mut m, Vec3::new(-0.3, top, 0.0), 4.6, 2.6, 3.4, 2.4);
            round_tower(&mut m, Vec3::new(4.2, top, 2.6), 0.85, 5.0, 3.0, None);
            round_tower(&mut m, Vec3::new(4.2, top, -2.6), 0.85, 5.0, 3.0, None);
            round_tower(&mut m, Vec3::new(-3.6, top, 2.05), 1.1, 8.0, 4.4, Some(royal()));
            square_tower(&mut m, Vec3::new(-3.4, top, -1.9), 1.6, 6.0, 2.6, false, None);
            m.boxy(Mat::Stone, Vec3::new(0.45, 2.6, 0.45), Vec3::new(-1.3, top + 3.4 + 1.3, 0.75), sh(stone_tint(), 0.9));
        }
        _ => {
            let top = 1.6;
            chassis(&mut m, len, wid - 0.2, top, 0.8, &[-4.8, -1.6, 1.6, 4.8], if big_wheels { 1.55 } else { 1.3 });
            let (hx, hz) = (6.55, 4.25);
            let (y0, h) = (top, 3.25);
            wall(&mut m, Vec2::new(-hx, -hz), Vec2::new(hx, -hz), y0, h, 0.55);
            wall(&mut m, Vec2::new(-hx, hz), Vec2::new(hx, hz), y0, h, 0.55);
            wall(&mut m, Vec2::new(-hx, -hz), Vec2::new(-hx, hz), y0, h, 0.55);
            wall(&mut m, Vec2::new(hx, -hz), Vec2::new(hx, -1.2), y0, h, 0.55);
            wall(&mut m, Vec2::new(hx, 1.2), Vec2::new(hx, hz), y0, h, 0.55);
            m.boxy(Mat::Stone, Vec3::new(0.7, 0.9, 2.6), Vec3::new(hx, y0 + h - 0.45, 0.0), stone_tint());
            for z in [-0.8, -0.4, 0.0, 0.4, 0.8] {
                m.boxy(Mat::Iron, Vec3::new(0.07, 2.2, 0.07), Vec3::new(hx + 0.12, y0 + 1.1, z), W);
            }
            for y in [0.5, 1.1, 1.7] {
                m.boxy(Mat::Iron, Vec3::new(0.07, 0.07, 2.1), Vec3::new(hx + 0.12, y0 + y, 0.0), W);
            }
            m.boxy(Mat::Wood, Vec3::new(1.6, 0.22, 3.0), Vec3::new(5.6, top + h - 0.11, 0.0), W);
            round_tower(&mut m, Vec3::new(hx, top, 2.0), 0.8, 5.0, 0.0, None);
            round_tower(&mut m, Vec3::new(hx, top, -2.0), 0.8, 5.0, 0.0, None);
            round_tower(&mut m, Vec3::new(6.0, top, 3.7), 1.2, 6.4, 3.8, None);
            round_tower(&mut m, Vec3::new(6.0, top, -3.7), 1.2, 6.4, 3.8, None);
            round_tower(&mut m, Vec3::new(-6.0, top, 3.7), 1.25, 7.6, 4.6, Some(crimson()));
            round_tower(&mut m, Vec3::new(-6.0, top, -3.7), 1.25, 7.6, 4.6, None);
            hall(&mut m, Vec3::new(0.2, top, 0.0), 6.0, 3.2, 4.4, 3.0);
            square_tower(&mut m, Vec3::new(-4.6, top, 0.0), 2.6, 11.0, 6.5, true, Some(royal()));
            round_tower(&mut m, Vec3::new(4.1, top, 2.3), 0.9, 10.0, 4.8, Some(royal()));
            // Covered bridge from the great tower to the hall.
            m.boxy(Mat::Stone, Vec3::new(1.2, 1.2, 1.0), Vec3::new(-2.9, top + 5.4, 0.0), sh(stone_tint(), 0.95));
            m.roof(Vec3::new(-2.9, top + 6.0, 0.0), Quat::IDENTITY, 1.3, 1.0, 0.6, 0.1, slate_tint(), Mat::Stone, stone_tint(), Mat::Slate);
            m.boxy(Mat::Stone, Vec3::new(0.5, 3.2, 0.5), Vec3::new(-1.9, top + 4.4 + 1.6, 1.1), sh(stone_tint(), 0.9));
        }
    }
    match (stage, variant) {
        (s, Variant::Defense) if s > 0 => {
            // Iron plating and spikes along the chassis.
            let n = 3 + s * 2;
            for i in 0..n {
                let x = -len / 2.0 + 0.8 + i as f32 * (len - 1.6) / (n as f32 - 1.0);
                for side in [-1.0f32, 1.0] {
                    let z = side * (wid / 2.0 - 0.05);
                    m.boxy(Mat::Iron, Vec3::new(1.0, 0.8, 0.08), Vec3::new(x, 1.2, z), W);
                    m.frustum_rot(Mat::Iron, Vec3::new(x, 1.2, z), Quat::from_rotation_x(side * FRAC_PI_2), 0.12, 0.0, 0.6, 6, W, false, false);
                }
            }
        }
        (s, Variant::Movement) if s > 0 => {
            for side in [-1.0f32, 1.0] {
                m.frustum_rot(Mat::Iron, Vec3::new(-len / 2.0 + 0.3, 1.4, side * wid * 0.25), Quat::from_rotation_z(1.0), 0.28, 0.34, 2.4, 10, W, true, false);
            }
        }
        _ => {}
    }
    m
}

/// Spoked wooden wheel with an iron tire; axle along Z.
fn spoked_wheel(m: &mut Model, center: Vec3, r: f32, w: f32) {
    let n = 14;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let p = center + Vec3::new(a.cos(), a.sin(), 0.0) * (r * 0.9);
        let rot = Quat::from_rotation_z(a + FRAC_PI_2);
        let seg = TAU * r * 0.9 / n as f32 * 1.08;
        m.cube(Mat::Wood, Vec3::new(seg, r * 0.16, w), p, rot, W);
        m.cube(Mat::Iron, Vec3::new(seg, r * 0.05, w * 1.05), center + Vec3::new(a.cos(), a.sin(), 0.0) * (r * 1.0), rot, W);
    }
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        let rot = Quat::from_rotation_z(a - FRAC_PI_2);
        m.frustum_rot(Mat::Wood, center, rot, r * 0.07, r * 0.05, r * 0.85, 5, W, true, false);
    }
    m.frustum_rot(Mat::Iron, center - Vec3::Z * (w * 0.9), Quat::from_rotation_x(FRAC_PI_2), r * 0.18, r * 0.16, w * 1.8, 10, W, true, true);
}

/// An archer drawing a longbow, facing +X.
fn archer_figure(m: &mut Model, tunic: Color, hood: Color) {
    let p = Mat::Paint;
    for z in [-0.11f32, 0.11] {
        m.frustum(p, Vec3::new(0.0, 0.0, z), 0.09, 0.08, 0.6, 7, rgb(0.20, 0.17, 0.14), true);
        m.blob(p, Vec3::new(0.05, 0.05, z), 0.09, Vec3::new(1.5, 0.6, 1.0), leather(), 0.0, 1, 1);
    }
    m.frustum(p, Vec3::new(0.0, 0.5, 0.0), 0.28, 0.23, 0.68, 10, tunic, true);
    m.frustum(p, Vec3::new(0.0, 0.8, 0.0), 0.255, 0.255, 0.07, 10, leather(), true);
    m.frustum(p, Vec3::new(0.0, 1.18, 0.0), 0.07, 0.07, 0.1, 6, skin(), true);
    m.blob(p, Vec3::new(0.0, 1.38, 0.0), 0.15, Vec3::new(1.0, 1.1, 0.95), skin(), 0.02, 3, 2);
    m.blob(p, Vec3::new(-0.03, 1.42, 0.0), 0.18, Vec3::new(1.05, 1.0, 1.0), hood, 0.03, 6, 2);
    m.frustum(p, Vec3::new(0.0, 1.05, 0.0), 0.26, 0.12, 0.2, 10, hood, true);
    // Bow arm straight out, draw arm pulled back to the cheek.
    let shoulder_l = Vec3::new(0.0, 1.12, -0.24);
    let shoulder_r = Vec3::new(0.0, 1.12, 0.24);
    let bow_hand = Vec3::new(0.6, 1.2, -0.12);
    m.frustum_rot(p, shoulder_l, Quat::from_rotation_arc(Vec3::Y, (bow_hand - shoulder_l).normalize()), 0.065, 0.055, (bow_hand - shoulder_l).length(), 6, tunic, true, false);
    let draw_hand = Vec3::new(0.12, 1.32, 0.08);
    m.frustum_rot(p, shoulder_r, Quat::from_rotation_arc(Vec3::Y, (draw_hand - shoulder_r).normalize()), 0.065, 0.055, (draw_hand - shoulder_r).length(), 6, tunic, true, false);
    for dir in [Vec3::new(-0.3, 1.0, 0.05), Vec3::new(-0.3, -1.0, 0.05)] {
        m.frustum_rot(p, bow_hand, Quat::from_rotation_arc(Vec3::Y, dir.normalize()), 0.03, 0.015, 0.72, 5, timber(), true, false);
    }
    let tip_up = bow_hand + Vec3::new(-0.3, 1.0, 0.05).normalize() * 0.72;
    let tip_dn = bow_hand + Vec3::new(-0.3, -1.0, 0.05).normalize() * 0.72;
    for tip in [tip_up, tip_dn] {
        let d = draw_hand - tip;
        m.frustum_rot(p, tip, Quat::from_rotation_arc(Vec3::Y, d.normalize()), 0.006, 0.006, d.length(), 3, rgb(0.85, 0.82, 0.7), true, false);
    }
    m.frustum_rot(p, draw_hand, Quat::from_rotation_arc(Vec3::Y, (bow_hand - draw_hand).normalize()), 0.012, 0.012, 0.9, 4, rgb(0.6, 0.5, 0.35), true, false);
    m.frustum_rot(p, Vec3::new(-0.22, 0.75, 0.1), Quat::from_rotation_x(0.25), 0.08, 0.09, 0.6, 7, leather(), true, true);
}

pub fn module(kind: ModuleKind) -> Model {
    let mut m = Model::new();
    match kind {
        ModuleKind::ArrowLoft => {
            // Covered wooden hoarding with archers drawing longbows.
            m.boxy(Mat::Wood, Vec3::new(2.2, 0.2, 2.2), Vec3::new(0.0, 0.1, 0.0), W);
            for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                m.boxy(Mat::Wood, Vec3::new(0.16, 1.9, 0.16), Vec3::new(x, 1.05, z), sh(timber(), 1.0));
            }
            for (side, axis) in [(1.0f32, true), (-1.0, false), (1.0, false)] {
                for k in 0..7 {
                    let t = -0.9 + k as f32 * 0.3;
                    let h = if k % 2 == 0 { 0.75 } else { 0.5 };
                    let (pos, size) = if axis {
                        (Vec3::new(1.05 * side, 0.2 + h / 2.0, t), Vec3::new(0.06, h, 0.26))
                    } else {
                        (Vec3::new(t, 0.2 + h / 2.0, 1.05 * side), Vec3::new(0.26, h, 0.06))
                    };
                    m.boxy(Mat::Wood, size, pos, W);
                }
            }
            m.roof(Vec3::new(0.0, 1.95, 0.0), Quat::IDENTITY, 2.4, 2.4, 1.0, 0.15, slate_tint(), Mat::Wood, W, Mat::Slate);
            for (x, z, a) in [(0.35, -0.55, 0.15), (0.45, 0.5, -0.2), (-0.45, 0.0, 0.0)] {
                let mut fig = Model::new();
                archer_figure(&mut fig, rgb(0.14, 0.24, 0.44), rgb(0.16, 0.18, 0.26));
                m.append(fig, Transform::from_xyz(x, 0.2, z).with_rotation(Quat::from_rotation_y(a)).with_scale(Vec3::splat(0.9)));
            }
        }
        ModuleKind::Mortar => {
            // Squat bronze bombard on a heavy timber bed, with spare shot.
            m.boxy(Mat::Wood, Vec3::new(1.7, 0.35, 1.3), Vec3::new(0.0, 0.18, 0.0), W);
            for x in [-0.6, 0.0, 0.6] {
                m.boxy(Mat::Iron, Vec3::new(0.1, 0.37, 1.32), Vec3::new(x, 0.18, 0.0), W);
            }
            // Timber cradle holding a short, thick bronze barrel tilted skyward.
            for z in [-0.42f32, 0.42] {
                m.boxy(Mat::Wood, Vec3::new(0.9, 0.55, 0.14), Vec3::new(-0.1, 0.62, z), W);
            }
            let rot = Quat::from_rotation_z(-0.5);
            let base = Vec3::new(-0.25, 0.55, 0.0);
            m.blob(Mat::Bronze, base, 0.36, Vec3::new(1.0, 0.9, 1.0), W, 0.0, 1, 2);
            m.frustum_rot(Mat::Bronze, base, rot, 0.36, 0.31, 0.95, 20, W, true, false);
            for (t, r) in [(0.1, 0.39), (0.5, 0.36), (0.88, 0.37)] {
                m.frustum_rot(Mat::Bronze, base + rot * Vec3::Y * t, rot, r, r, 0.08, 20, sh(W, 0.85), true, false);
            }
            m.frustum_rot(Mat::Bronze, base + rot * Vec3::Y * 0.95, rot, 0.36, 0.36, 0.05, 20, W, true, false);
            m.frustum_rot(Mat::Paint, base + rot * Vec3::Y * 0.97, rot, 0.25, 0.25, 0.01, 16, rgb(0.02, 0.02, 0.02), true, true);
            m.frustum_rot(Mat::Bronze, base + Vec3::new(0.0, 0.0, -0.5), Quat::from_rotation_x(FRAC_PI_2), 0.07, 0.07, 1.0, 8, W, true, true);
            for (x, z) in [(0.55, 0.4), (0.7, 0.15), (0.6, -0.15)] {
                m.blob(Mat::Iron, Vec3::new(x, 0.47, z), 0.13, Vec3::ONE, W, 0.0, 1, 2);
            }
        }
        ModuleKind::Ram => {
            // Covered battering ram: iron-headed log hung on chains.
            for (x, z) in [(-0.6, -0.55), (-0.6, 0.55), (1.0, -0.55), (1.0, 0.55)] {
                m.boxy(Mat::Wood, Vec3::new(0.15, 1.3, 0.15), Vec3::new(x, 0.65, z), W);
            }
            m.boxy(Mat::Wood, Vec3::new(1.8, 0.12, 0.15), Vec3::new(0.2, 1.3, 0.0), W);
            m.roof(Vec3::new(0.2, 1.32, 0.0), Quat::IDENTITY, 1.9, 1.3, 0.55, 0.1, rgb(0.55, 0.42, 0.3), Mat::Wood, W, Mat::Wood);
            let axis = Quat::from_rotation_z(-FRAC_PI_2);
            m.frustum_rot(Mat::Bark, Vec3::new(-0.7, 0.62, 0.0), axis, 0.22, 0.2, 2.2, 10, W, true, true);
            for x in [-0.4, 0.4, 1.1] {
                m.frustum_rot(Mat::Iron, Vec3::new(x, 0.62, 0.0), axis, 0.24, 0.24, 0.08, 10, W, true, false);
            }
            m.frustum_rot(Mat::Iron, Vec3::new(1.5, 0.62, 0.0), axis, 0.3, 0.26, 0.35, 10, W, true, true);
            m.frustum_rot(Mat::Iron, Vec3::new(1.85, 0.62, 0.0), axis, 0.24, 0.0, 0.35, 8, W, false, false);
            for x in [-0.2, 0.8] {
                m.boxy(Mat::Iron, Vec3::new(0.04, 0.65, 0.04), Vec3::new(x, 0.98, 0.0), W);
            }
        }
        ModuleKind::MineLayer => {
            m.frustum_rot(Mat::Wood, Vec3::new(0.5, 0.75, 0.0), Quat::from_rotation_z(FRAC_PI_2), 0.5, 0.56, 1.1, 16, W, true, true);
            for x in [0.35, -0.05, -0.45] {
                m.frustum_rot(Mat::Iron, Vec3::new(x + 0.03, 0.75, 0.0), Quat::from_rotation_z(FRAC_PI_2), 0.58, 0.58, 0.06, 16, W, true, false);
            }
            m.cube(Mat::Wood, Vec3::new(1.1, 0.08, 0.7), Vec3::new(-1.0, 0.4, 0.0), Quat::from_rotation_z(0.35), W);
            for z in [-0.36f32, 0.36] {
                m.cube(Mat::Wood, Vec3::new(1.1, 0.2, 0.05), Vec3::new(-1.0, 0.48, z), Quat::from_rotation_z(0.35), W);
            }
            for (x, z) in [(-0.9, 0.0), (-1.35, 0.15)] {
                m.blob(Mat::Iron, Vec3::new(x, 0.6 - (x + 0.9).abs() * 0.3, z), 0.17, Vec3::ONE, W, 0.0, 1, 2);
            }
        }
        ModuleKind::Broadside => {
            // Bronze cannon on a wooden carriage, facing +Z (mounted on both sides).
            for x in [-0.27f32, 0.27] {
                m.cube(Mat::Wood, Vec3::new(0.11, 0.5, 1.5), Vec3::new(x, 0.42, -0.25), Quat::from_rotation_x(0.12), W);
            }
            m.boxy(Mat::Wood, Vec3::new(0.6, 0.18, 0.3), Vec3::new(0.0, 0.3, -0.75), W);
            m.frustum_rot(Mat::Iron, Vec3::new(-0.62, 0.36, 0.25), Quat::from_rotation_z(-FRAC_PI_2), 0.05, 0.05, 1.24, 6, W, true, false);
            for x in [-0.5f32, 0.5] {
                let mut wheel = Model::new();
                spoked_wheel(&mut wheel, Vec3::ZERO, 0.36, 0.09);
                m.append(wheel, Transform::from_xyz(x, 0.36, 0.25).with_rotation(Quat::from_rotation_y(FRAC_PI_2)));
            }
            let axis = Quat::from_rotation_x(FRAC_PI_2);
            let breech = Vec3::new(0.0, 0.72, -0.6);
            m.frustum_rot(Mat::Bronze, breech, axis, 0.25, 0.17, 1.9, 20, W, true, false);
            for (t, r) in [(0.0, 0.27), (0.45, 0.24), (1.0, 0.22), (1.78, 0.2)] {
                m.frustum_rot(Mat::Bronze, breech + Vec3::Z * t, axis, r, r, 0.09, 20, sh(W, 0.9), true, false);
            }
            m.frustum_rot(Mat::Bronze, breech + Vec3::Z * 1.9, axis, 0.2, 0.2, 0.06, 20, W, true, true);
            m.frustum_rot(Mat::Paint, breech + Vec3::Z * 1.965, axis, 0.1, 0.1, 0.005, 12, rgb(0.02, 0.02, 0.02), true, true);
            m.blob(Mat::Bronze, breech - Vec3::Z * 0.12, 0.09, Vec3::ONE, W, 0.0, 1, 2);
            m.frustum_rot(Mat::Bronze, breech + Vec3::new(-0.33, 0.0, 0.55), Quat::from_rotation_z(-FRAC_PI_2), 0.06, 0.06, 0.66, 8, W, true, true);
        }
        ModuleKind::Ballista => {
            // Giant torsion crossbow on a turntable, facing +X.
            m.frustum(Mat::Wood, Vec3::ZERO, 0.65, 0.6, 0.22, 16, W, true);
            m.frustum(Mat::Iron, Vec3::Y * 0.22, 0.62, 0.62, 0.04, 16, W, true);
            m.frustum(Mat::Wood, Vec3::Y * 0.26, 0.16, 0.13, 0.45, 8, W, true);
            let tilt = Quat::from_rotation_z(0.08);
            m.cube(Mat::Wood, Vec3::new(2.5, 0.16, 0.3), Vec3::new(0.0, 0.8, 0.0), tilt, W);
            m.cube(Mat::Wood, Vec3::new(0.35, 0.65, 0.6), Vec3::new(0.75, 0.86, 0.0), tilt, W);
            for z in [-0.22f32, 0.22] {
                m.frustum(Mat::Paint, Vec3::new(0.75, 0.55, z), 0.09, 0.09, 0.65, 8, rgb(0.45, 0.35, 0.22), true);
                for y in [0.58, 1.17] {
                    m.frustum(Mat::Iron, Vec3::new(0.75, y, z), 0.11, 0.11, 0.05, 8, W, true);
                }
            }
            let mut tips = Vec::new();
            for s in [-1.0f32, 1.0] {
                let root = Vec3::new(0.8, 0.88, s * 0.24);
                let dir = Vec3::new(-0.38, 0.03, s).normalize();
                m.frustum_rot(Mat::Wood, root, Quat::from_rotation_arc(Vec3::Y, dir), 0.07, 0.04, 1.05, 7, W, true, false);
                let tip = root + dir * 1.05;
                m.blob(Mat::Iron, tip, 0.05, Vec3::ONE, W, 0.0, 1, 1);
                tips.push(tip);
            }
            let nock = Vec3::new(-0.45, 0.93, 0.0);
            for tip in tips {
                let d = nock - tip;
                m.frustum_rot(Mat::Paint, tip, Quat::from_rotation_arc(Vec3::Y, d.normalize()), 0.012, 0.012, d.length(), 3, rgb(0.82, 0.78, 0.66), true, false);
            }
            let bolt = Quat::from_rotation_z(-FRAC_PI_2 + 0.08);
            m.frustum_rot(Mat::Wood, Vec3::new(-0.45, 0.95, 0.0), bolt, 0.035, 0.035, 1.9, 6, rgb(0.8, 0.7, 0.55), true, false);
            m.frustum_rot(Mat::Iron, Vec3::new(1.44, 1.1, 0.0), bolt, 0.07, 0.0, 0.25, 4, W, false, false);
            for s in [-1.0f32, 1.0] {
                m.card(Mat::Cloth, Vec3::new(-0.3, 0.97, 0.0), Vec3::X * 0.14, Vec3::new(0.0, 0.05, s * 0.08), Vec2::ZERO, Vec2::ONE, rgb(0.85, 0.83, 0.78), None);
            }
            m.frustum_rot(Mat::Wood, Vec3::new(-1.1, 0.78, -0.32), Quat::from_rotation_x(FRAC_PI_2), 0.09, 0.09, 0.64, 10, W, true, true);
            for z in [-0.36f32, 0.36] {
                m.cube(Mat::Iron, Vec3::new(0.04, 0.3, 0.04), Vec3::new(-1.1, 0.88, z), Quat::from_rotation_x(0.6), W);
            }
        }
    }
    m
}

/// Small 3D still-life for each artifact card.
pub fn artifact(kind: crate::items::ArtifactKind) -> Model {
    use crate::items::ArtifactKind as A;
    let mut m = Model::new();
    match kind {
        A::Whetstone => {
            m.blob(Mat::Stone, Vec3::new(0.0, 0.12, 0.0), 0.3, Vec3::new(2.2, 0.5, 0.9), rgb(0.6, 0.6, 0.62), 0.05, 3, 2);
            m.cube(Mat::Iron, Vec3::new(1.6, 0.03, 0.18), Vec3::new(0.1, 0.45, 0.1), Quat::from_rotation_z(0.35), rgb(1.3, 1.3, 1.35));
            m.cube(Mat::Wood, Vec3::new(0.45, 0.08, 0.08), Vec3::new(-0.75, 0.15, 0.1), Quat::from_rotation_z(0.35), W);
            m.cube(Mat::Bronze, Vec3::new(0.06, 0.1, 0.4), Vec3::new(-0.52, 0.24, 0.1), Quat::from_rotation_z(0.35), W);
        }
        A::GreasedAxles => {
            let mut w = Model::new();
            spoked_wheel(&mut w, Vec3::ZERO, 0.55, 0.1);
            m.append(w, Transform::from_xyz(0.0, 0.6, 0.0));
            m.frustum_rot(Mat::Iron, Vec3::new(0.0, 0.6, -0.6), Quat::from_rotation_x(FRAC_PI_2), 0.06, 0.06, 1.2, 8, W, true, true);
            m.frustum(Mat::Bronze, Vec3::new(0.7, 0.0, 0.3), 0.18, 0.14, 0.35, 12, W, true);
            m.frustum_rot(Mat::Bronze, Vec3::new(0.7, 0.3, 0.3), Quat::from_rotation_z(0.9), 0.04, 0.02, 0.4, 6, W, true, false);
        }
        A::Lodestone => {
            for z in [-0.3f32, 0.3] {
                m.boxy(Mat::Iron, Vec3::new(0.9, 0.22, 0.22), Vec3::new(0.0, 0.3, z), W);
                m.boxy(Mat::Paint, Vec3::new(0.25, 0.23, 0.23), Vec3::new(0.55, 0.3, z), if z < 0.0 { rgb(0.75, 0.1, 0.08) } else { rgb(0.12, 0.2, 0.6) });
            }
            m.frustum_rot(Mat::Iron, Vec3::new(-0.45, 0.3, -0.3), Quat::from_rotation_x(FRAC_PI_2), 0.11, 0.11, 0.6, 12, W, true, false);
            for k in 0..5 {
                m.blob(Mat::Gold, Vec3::new(0.85 + k as f32 * 0.06, 0.1, -0.3 + k as f32 * 0.15), 0.07, Vec3::new(1.0, 0.25, 1.0), W, 0.0, 1, 1);
            }
        }
        A::ReinforcedHull => {
            m.boxy(Mat::Wood, Vec3::new(1.3, 1.0, 0.12), Vec3::new(0.0, 0.5, 0.0), W);
            m.boxy(Mat::Iron, Vec3::new(1.1, 0.8, 0.08), Vec3::new(0.0, 0.5, 0.09), W);
            for (x, y) in [(-0.45, 0.2), (0.45, 0.2), (-0.45, 0.8), (0.45, 0.8), (0.0, 0.2), (0.0, 0.8)] {
                m.blob(Mat::Iron, Vec3::new(x, y, 0.14), 0.05, Vec3::ONE, sh(W, 1.4), 0.0, 1, 1);
            }
        }
        A::QuickHands => {
            for y in [0.0, 1.0] {
                m.frustum(Mat::Wood, Vec3::new(0.0, y, 0.0), 0.42, 0.42, 0.1, 16, W, true);
            }
            for (x, z) in [(0.32, 0.32), (-0.32, 0.32), (0.32, -0.32), (-0.32, -0.32)] {
                m.frustum(Mat::Wood, Vec3::new(x, 0.1, z), 0.035, 0.035, 0.9, 6, W, true);
            }
            m.frustum(Mat::Glass, Vec3::new(0.0, 0.1, 0.0), 0.3, 0.04, 0.45, 16, W, true);
            m.frustum(Mat::Glass, Vec3::new(0.0, 0.55, 0.0), 0.04, 0.3, 0.45, 16, W, true);
            m.frustum(Mat::Paint, Vec3::new(0.0, 0.1, 0.0), 0.24, 0.0, 0.25, 12, rgb(0.85, 0.7, 0.4), true);
        }
        A::BlackPowder => {
            m.frustum(Mat::Wood, Vec3::ZERO, 0.42, 0.48, 0.45, 16, W, true);
            m.frustum(Mat::Wood, Vec3::Y * 0.45, 0.48, 0.42, 0.45, 16, W, true);
            for y in [0.1, 0.45, 0.8] {
                m.frustum(Mat::Iron, Vec3::Y * y, 0.47, 0.47, 0.05, 16, W, true);
            }
            m.blob(Mat::Iron, Vec3::new(0.65, 0.22, 0.25), 0.22, Vec3::ONE, sh(W, 0.6), 0.0, 1, 2);
            m.frustum_rot(Mat::Paint, Vec3::new(0.65, 0.42, 0.25), Quat::from_rotation_z(0.5), 0.02, 0.02, 0.25, 4, rgb(0.6, 0.5, 0.3), true, false);
            m.blob(Mat::Paint, Vec3::new(0.54, 0.65, 0.25), 0.05, Vec3::ONE, rgb(4.0, 2.0, 0.4), 0.0, 1, 1);
        }
        A::HawkEye => {
            let rot = Quat::from_rotation_z(-1.25);
            let mut base = Vec3::new(-0.7, 0.15, 0.0);
            for (len, r) in [(0.6, 0.13), (0.5, 0.11), (0.45, 0.09)] {
                m.frustum_rot(Mat::Bronze, base, rot, r, r, len, 16, W, true, true);
                m.frustum_rot(Mat::Bronze, base + rot * Vec3::Y * (len - 0.05), rot, r + 0.02, r + 0.02, 0.06, 16, sh(W, 0.8), true, false);
                base += rot * Vec3::Y * len;
            }
            m.frustum_rot(Mat::Glass, base, rot, 0.08, 0.08, 0.01, 12, W, true, true);
        }
        A::TinkersKit => {
            m.frustum_rot(Mat::Wood, Vec3::new(-0.6, 0.08, 0.1), Quat::from_rotation_z(-FRAC_PI_2), 0.045, 0.04, 1.0, 6, W, true, true);
            m.boxy(Mat::Iron, Vec3::new(0.16, 0.16, 0.45), Vec3::new(0.42, 0.1, 0.1), W);
            for (x, z, r) in [(-0.3, -0.45, 0.3), (0.25, -0.5, 0.2)] {
                m.frustum(Mat::Bronze, Vec3::new(x, 0.0, z), r, r, 0.08, 18, W, true);
                for k in 0..10 {
                    let a = k as f32 / 10.0 * TAU;
                    m.boxy(Mat::Bronze, Vec3::new(0.07, 0.08, 0.07), Vec3::new(x + a.cos() * (r + 0.03), 0.04, z + a.sin() * (r + 0.03)), W);
                }
            }
        }
        A::Poultice => {
            m.blob(Mat::Paint, Vec3::new(0.0, 0.35, 0.0), 0.33, Vec3::new(1.0, 1.05, 1.0), rgb(0.15, 0.55, 0.25), 0.0, 1, 3);
            m.blob(Mat::Glass, Vec3::new(0.0, 0.36, 0.0), 0.35, Vec3::new(1.0, 1.05, 1.0), W, 0.0, 1, 3);
            m.frustum(Mat::Glass, Vec3::new(0.0, 0.66, 0.0), 0.1, 0.08, 0.3, 12, W, true);
            m.frustum(Mat::Wood, Vec3::new(0.0, 0.92, 0.0), 0.085, 0.1, 0.14, 10, rgb(0.8, 0.65, 0.45), true);
            m.boxy(Mat::Paint, Vec3::new(0.3, 0.16, 0.02), Vec3::new(0.0, 0.32, 0.33), rgb(0.88, 0.82, 0.66));
        }
        A::GoldenFleece => {
            for (x, y, z, r) in [(0.0, 0.25, 0.0, 0.32), (0.3, 0.2, 0.2, 0.24), (-0.3, 0.22, -0.15, 0.25), (0.15, 0.4, -0.2, 0.2), (-0.2, 0.35, 0.25, 0.2)] {
                m.blob(Mat::Gold, Vec3::new(x, y, z), r, Vec3::new(1.0, 0.7, 1.0), W, 0.3, (x * 100.0) as u32 + 7, 2);
            }
        }
        A::NitroTank => {
            m.frustum(Mat::Iron, Vec3::ZERO, 0.32, 0.32, 0.9, 18, W, true);
            m.blob(Mat::Iron, Vec3::Y * 0.9, 0.32, Vec3::new(1.0, 0.6, 1.0), W, 0.0, 1, 2);
            m.frustum(Mat::Paint, Vec3::Y * 0.4, 0.325, 0.325, 0.16, 18, rgb(0.15, 0.35, 0.85), true);
            m.frustum(Mat::Bronze, Vec3::Y * 1.05, 0.06, 0.06, 0.18, 8, W, true);
            m.frustum_rot(Mat::Bronze, Vec3::Y * 1.18, Quat::from_rotation_x(FRAC_PI_2), 0.12, 0.12, 0.03, 12, W, true, true);
        }
        A::IronSpikes => {
            m.boxy(Mat::Iron, Vec3::new(1.2, 0.12, 0.8), Vec3::new(0.0, 0.06, 0.0), W);
            for x in [-0.4, 0.0, 0.4] {
                for z in [-0.2, 0.2] {
                    m.frustum(Mat::Iron, Vec3::new(x, 0.12, z), 0.1, 0.0, 0.5, 6, sh(W, 1.3), false);
                }
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Troops (single vertex-colored mesh each, so thousands stay cheap)
// ---------------------------------------------------------------------------

fn soldier(m: &mut Model, tabard: Color, helmet: Color) {
    let p = Mat::Paint;
    for z in [-0.11f32, 0.11] {
        m.frustum(p, Vec3::new(0.0, 0.0, z), 0.09, 0.08, 0.6, 7, rgb(0.20, 0.17, 0.14), true);
        m.blob(p, Vec3::new(0.05, 0.05, z), 0.09, Vec3::new(1.5, 0.6, 1.0), leather(), 0.0, 1, 1);
    }
    m.frustum(p, Vec3::new(0.0, 0.55, 0.0), 0.27, 0.23, 0.65, 10, mail(), true);
    m.frustum(p, Vec3::new(0.0, 0.5, 0.0), 0.29, 0.25, 0.55, 10, tabard, true);
    m.frustum(p, Vec3::new(0.0, 0.78, 0.0), 0.255, 0.255, 0.07, 10, leather(), true);
    for z in [-0.29f32, 0.29] {
        m.frustum_rot(p, Vec3::new(0.0, 1.15, z), Quat::from_rotation_x(std::f32::consts::PI - z.signum() * 0.15), 0.07, 0.06, 0.5, 6, mail(), true, false);
    }
    m.frustum(p, Vec3::new(0.0, 1.18, 0.0), 0.07, 0.07, 0.1, 6, skin(), true);
    m.blob(p, Vec3::new(0.0, 1.38, 0.0), 0.15, Vec3::new(1.0, 1.1, 0.95), skin(), 0.02, 3, 2);
    m.blob(p, Vec3::new(-0.01, 1.43, 0.0), 0.165, Vec3::new(1.05, 0.85, 1.0), helmet, 0.0, 4, 2);
    m.boxy(p, Vec3::new(0.03, 0.12, 0.03), Vec3::new(0.15, 1.38, 0.0), helmet);
}

pub fn footman() -> Mesh {
    let mut m = Model::new();
    soldier(&mut m, crimson(), steel());
    // Kite shield and spear.
    m.cube(Mat::Paint, Vec3::new(0.05, 0.75, 0.48), Vec3::new(0.3, 0.85, 0.24), Quat::from_rotation_y(0.3), rgb(0.55, 0.50, 0.42));
    m.cube(Mat::Paint, Vec3::new(0.055, 0.55, 0.12), Vec3::new(0.31, 0.9, 0.24), Quat::from_rotation_y(0.3), crimson());
    m.frustum_rot(Mat::Paint, Vec3::new(0.1, 0.2, -0.32), Quat::from_rotation_z(-0.25), 0.025, 0.02, 2.3, 5, timber(), true, false);
    m.frustum_rot(Mat::Paint, Vec3::new(0.66, 2.43, -0.32), Quat::from_rotation_z(-0.25), 0.05, 0.0, 0.25, 4, steel(), false, false);
    m.build_single()
}

pub fn archer() -> Mesh {
    let mut m = Model::new();
    archer_figure(&mut m, rgb(0.24, 0.30, 0.16), rgb(0.26, 0.20, 0.13));
    m.build_single()
}

pub fn knight() -> Mesh {
    let mut m = Model::new();
    let p = Mat::Paint;
    let horse = rgb(0.30, 0.20, 0.13);
    m.blob(p, Vec3::new(0.0, 1.05, 0.0), 0.42, Vec3::new(1.9, 1.0, 0.8), horse, 0.04, 9, 2);
    m.frustum_rot(p, Vec3::new(0.6, 1.2, 0.0), Quat::from_rotation_z(-0.7), 0.2, 0.14, 0.7, 8, horse, true, false);
    m.blob(p, Vec3::new(1.12, 1.62, 0.0), 0.16, Vec3::new(1.8, 0.9, 0.8), horse, 0.0, 9, 2);
    for (x, z) in [(-0.55, -0.16), (-0.55, 0.16), (0.55, -0.16), (0.55, 0.16)] {
        m.frustum(p, Vec3::new(x, 0.0, z), 0.06, 0.08, 0.85, 6, sh(horse, 0.8), true);
    }
    m.frustum_rot(p, Vec3::new(-0.75, 1.15, 0.0), Quat::from_rotation_z(2.4), 0.06, 0.03, 0.6, 5, rgb(0.12, 0.09, 0.07), true, false);
    // Caparison.
    m.frustum(p, Vec3::new(0.0, 0.62, 0.0), 0.58, 0.48, 0.5, 12, crimson(), true);
    // Rider in plate.
    m.frustum(p, Vec3::new(-0.1, 1.35, 0.0), 0.24, 0.21, 0.6, 10, steel(), true);
    m.blob(p, Vec3::new(-0.1, 2.08, 0.0), 0.17, Vec3::new(1.0, 1.2, 1.0), steel(), 0.0, 5, 2);
    m.cube(p, Vec3::new(0.06, 0.8, 0.5), Vec3::new(-0.36, 1.6, 0.0), Quat::from_rotation_z(0.12), crimson());
    m.frustum_rot(p, Vec3::new(-0.2, 1.65, 0.28), Quat::from_rotation_z(-1.45), 0.04, 0.02, 2.6, 6, rgb(0.62, 0.55, 0.42), true, false);
    m.build_single()
}

pub fn cart() -> Mesh {
    let mut m = Model::new();
    let p = Mat::Paint;
    m.boxy(p, Vec3::new(2.6, 0.9, 1.6), Vec3::new(0.0, 1.0, 0.0), timber());
    m.roof(Vec3::new(-0.1, 1.45, 0.0), Quat::IDENTITY, 2.4, 1.7, 0.7, 0.08, crimson(), p, timber(), p);
    for x in [-0.8f32, 0.8] {
        for s in [-1.0f32, 1.0] {
            m.frustum_rot(p, Vec3::new(x, 0.5, s * 0.8), Quat::from_rotation_x(s * FRAC_PI_2), 0.5, 0.5, 0.2, 12, sh(timber(), 0.7), true, true);
        }
    }
    for z in [-0.5, 0.0, 0.5] {
        m.frustum_rot(p, Vec3::new(1.3, 0.9, z), Quat::from_rotation_z(-FRAC_PI_2), 0.12, 0.0, 0.7, 5, steel(), false, false);
    }
    m.build_single()
}

pub fn sheep() -> Mesh {
    let mut m = Model::new();
    let p = Mat::Paint;
    m.blob(p, Vec3::new(0.0, 0.75, 0.0), 0.5, Vec3::new(1.3, 0.95, 1.0), rgb(0.86, 0.84, 0.78), 0.12, 21, 2);
    m.blob(p, Vec3::new(0.68, 0.85, 0.0), 0.2, Vec3::new(1.4, 1.0, 0.9), rgb(0.12, 0.10, 0.09), 0.0, 22, 2);
    for (x, z) in [(-0.35, -0.2), (-0.35, 0.2), (0.35, -0.2), (0.35, 0.2)] {
        m.frustum(p, Vec3::new(x, 0.0, z), 0.05, 0.05, 0.45, 5, rgb(0.12, 0.10, 0.09), true);
    }
    m.build_single()
}

pub fn arrow(shaft: Color) -> Mesh {
    let mut m = Model::new();
    let p = Mat::Paint;
    m.frustum_rot(p, Vec3::new(-0.55, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.025, 0.025, 1.1, 5, shaft, true, false);
    m.frustum_rot(p, Vec3::new(0.55, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.05, 0.0, 0.2, 4, steel(), false, false);
    m.cube(p, Vec3::new(0.22, 0.02, 0.14), Vec3::new(-0.48, 0.0, 0.0), Quat::IDENTITY, rgb(0.85, 0.85, 0.82));
    m.build_single()
}

pub fn mine() -> Mesh {
    let mut m = Model::new();
    let p = Mat::Paint;
    m.frustum(p, Vec3::ZERO, 0.55, 0.38, 0.28, 12, rgb(0.14, 0.14, 0.15), true);
    m.frustum(p, Vec3::Y * 0.28, 0.12, 0.1, 0.12, 8, rgb(0.8, 0.15, 0.08), true);
    m.build_single()
}

pub fn pointer() -> Mesh {
    let mut m = Model::new();
    m.frustum_rot(Mat::Paint, Vec3::ZERO, Quat::from_rotation_z(-FRAC_PI_2), 0.8, 0.0, 1.8, 3, rgb(1.0, 0.25, 0.15), false, false);
    m.build_single()
}

pub fn coin() -> Model {
    let mut m = Model::new();
    m.frustum_rot(Mat::Gold, Vec3::new(0.0, 0.0, -0.06), Quat::from_rotation_x(FRAC_PI_2), 0.38, 0.38, 0.12, 16, W, true, true);
    m.frustum_rot(Mat::Gold, Vec3::new(0.0, 0.0, 0.06), Quat::from_rotation_x(-FRAC_PI_2), 0.38, 0.38, 0.01, 16, W, true, true);
    m
}

// ---------------------------------------------------------------------------
// Bosses (textured, several materials each)
// ---------------------------------------------------------------------------

pub fn siege_tower() -> Model {
    let mut m = Model::new();
    chassis(&mut m, 5.4, 4.4, 1.35, 0.6, &[-1.9, 1.9], 0.85);
    m.frustum(Mat::Wood, Vec3::new(0.0, 1.35, 0.0), 2.4, 1.8, 8.6, 4, W, false);
    for i in 0..5 {
        let y = 2.6 + i as f32 * 1.7;
        let r = 2.35 - i as f32 * 0.13;
        m.frustum(Mat::Iron, Vec3::new(0.0, y, 0.0), r, r - 0.02, 0.14, 4, W, false);
    }
    m.cube(Mat::Paint, Vec3::new(0.2, 6.0, 3.0), Vec3::new(1.95, 6.0, 0.0), Quat::from_rotation_z(-0.12), rgb(0.42, 0.30, 0.20));
    m.boxy(Mat::Wood, Vec3::new(3.4, 0.3, 3.4), Vec3::new(0.0, 10.05, 0.0), W);
    for (x, z) in [(1.5, 1.5), (-1.5, 1.5), (1.5, -1.5), (-1.5, -1.5)] {
        m.boxy(Mat::Wood, Vec3::new(0.24, 1.1, 0.24), Vec3::new(x, 10.7, z), timber());
    }
    for (x, z) in [(0.6, 0.6), (-0.6, -0.5), (0.5, -0.8), (-0.4, 0.7)] {
        let mut s = Model::new();
        soldier(&mut s, crimson(), steel());
        let mesh_parts = s.parts.remove(&Mat::Paint).unwrap_or_default();
        let part = m.part(Mat::Paint);
        let off = part.pos.len() as u32;
        for i in 0..mesh_parts.pos.len() {
            let p = Vec3::from_array(mesh_parts.pos[i]) + Vec3::new(x, 10.2, z);
            part.pos.push(p.to_array());
            part.nrm.push(mesh_parts.nrm[i]);
            part.uv.push(mesh_parts.uv[i]);
            part.col.push(mesh_parts.col[i]);
        }
        part.idx.extend(mesh_parts.idx.iter().map(|i| i + off));
    }
    flag_at(&mut m, Vec3::new(-1.5, 11.2, 1.5), crimson());
    m
}

pub fn war_wagon() -> Model {
    let mut m = Model::new();
    chassis(&mut m, 5.4, 3.2, 1.3, 0.5, &[-1.8, 0.0, 1.8], 0.9);
    m.boxy(Mat::Iron, Vec3::new(5.0, 1.6, 3.2), Vec3::new(0.0, 2.1, 0.0), W);
    for i in 0..6 {
        let x = -2.2 + i as f32 * 0.88;
        for s in [-1.0f32, 1.0] {
            m.blob(Mat::Iron, Vec3::new(x, 2.4, s * 1.62), 0.07, Vec3::ONE, W, 0.0, 1, 1);
        }
    }
    m.roof(Vec3::new(0.0, 2.9, 0.0), Quat::IDENTITY, 5.0, 3.2, 0.6, 0.05, W, Mat::Iron, W, Mat::Iron);
    m.frustum(Mat::Iron, Vec3::new(0.3, 3.3, 0.0), 0.9, 0.8, 0.6, 12, W, true);
    m.frustum_rot(Mat::Iron, Vec3::new(0.8, 3.65, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.36, 0.3, 2.6, 12, rgb(0.6, 0.6, 0.6), true, true);
    flag_at(&mut m, Vec3::new(-2.2, 3.0, 1.2), crimson());
    m
}

pub fn juggernaut() -> Model {
    let mut m = Model::new();
    chassis(&mut m, 9.0, 5.4, 1.6, 0.8, &[-3.2, -1.0, 1.2, 3.4], 1.15);
    m.boxy(Mat::Iron, Vec3::new(8.6, 2.4, 5.2), Vec3::new(0.0, 2.8, 0.0), W);
    m.roof(Vec3::new(-0.6, 4.0, 0.0), Quat::IDENTITY, 7.0, 4.8, 1.6, 0.1, W, Mat::Iron, W, Mat::Iron);
    m.frustum_rot(Mat::Iron, Vec3::new(4.3, 2.4, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.5, 0.0, 3.4, 10, rgb(0.5, 0.5, 0.5), true, false);
    for z in [-1.8f32, 1.8] {
        m.frustum_rot(Mat::Iron, Vec3::new(4.3, 1.8, z), Quat::from_rotation_z(-FRAC_PI_2), 0.5, 0.0, 1.8, 8, W, false, false);
        m.blob(Mat::Paint, Vec3::new(4.32, 3.4, z * 0.5), 0.22, Vec3::new(0.4, 1.0, 1.0), rgb(4.0, 1.2, 0.2), 0.0, 1, 1);
    }
    for i in 0..7 {
        m.frustum(Mat::Iron, Vec3::new(-3.0 + i as f32 * 1.0, 5.4, 0.0), 0.35, 0.0, 1.1, 6, W, false);
    }
    m.frustum(Mat::Iron, Vec3::new(-3.6, 4.0, 1.4), 0.45, 0.5, 2.4, 10, rgb(0.4, 0.4, 0.4), true);
    m
}

pub fn trebuchet() -> Model {
    let mut m = Model::new();
    chassis(&mut m, 8.0, 6.0, 1.6, 0.8, &[-2.8, 0.0, 2.8], 1.0);
    for z in [-1.2f32, 1.2] {
        m.cube(Mat::Wood, Vec3::new(0.4, 6.0, 0.4), Vec3::new(-0.7, 4.5, z), Quat::from_rotation_z(0.22), W);
        m.cube(Mat::Wood, Vec3::new(0.4, 6.0, 0.4), Vec3::new(0.7, 4.5, z), Quat::from_rotation_z(-0.22), W);
    }
    m.frustum_rot(Mat::Iron, Vec3::new(0.0, 7.3, -1.5), Quat::from_rotation_x(FRAC_PI_2), 0.18, 0.18, 3.0, 8, W, true, false);
    m.cube(Mat::Wood, Vec3::new(9.5, 0.45, 0.45), Vec3::new(-0.6, 7.6, 0.0), Quat::from_rotation_z(0.45), W);
    m.boxy(Mat::Stone, Vec3::new(1.9, 1.9, 1.9), Vec3::new(3.6, 6.4, 0.0), rgb(0.6, 0.58, 0.55));
    m.blob(Mat::Stone, Vec3::new(-4.6, 5.6, 0.0), 0.8, Vec3::ONE, rgb(0.7, 0.68, 0.64), 0.15, 31, 2);
    m.boxy(Mat::Stone, Vec3::new(2.4, 3.2, 2.4), Vec3::new(-2.6, 3.2, -1.9), sh(stone_tint(), 0.8));
    m.frustum(Mat::Slate, Vec3::new(-2.6, 4.8, -1.9), 1.9, 0.0, 2.0, 4, slate_tint(), false);
    for i in 0..5 {
        m.blob(Mat::Stone, Vec3::new(-3.0 + i as f32 * 0.7, 1.95, 2.2), 0.35, Vec3::ONE, rgb(0.7, 0.68, 0.64), 0.2, 40 + i, 1);
    }
    m
}

pub fn warlord() -> Model {
    let mut m = Model::new();
    let dark = rgb(0.42, 0.40, 0.42);
    chassis(&mut m, 17.0, 12.0, 1.9, 1.0, &[-6.6, -3.3, 0.0, 3.3, 6.6], 1.5);
    // Dark walls.
    m.boxy(Mat::Stone, Vec3::new(15.0, 4.4, 10.4), Vec3::new(0.0, 4.1, 0.0), dark);
    battlements(&mut m, Vec3::new(-7.4, 6.3, 5.1), Vec3::new(7.4, 6.3, 5.1), 0.3);
    battlements(&mut m, Vec3::new(-7.4, 6.3, -5.1), Vec3::new(7.4, 6.3, -5.1), 0.3);
    for (x, z) in [(6.6, 4.4), (6.6, -4.4), (-6.6, 4.4), (-6.6, -4.4)] {
        m.frustum(Mat::Stone, Vec3::new(x, 1.9, z), 2.1, 1.9, 8.8, 16, dark, true);
        m.frustum(Mat::Stone, Vec3::new(x, 10.5, z), 1.9, 2.2, 0.4, 16, sh(dark, 0.9), true);
        m.frustum(Mat::Slate, Vec3::new(x, 10.9, z), 2.5, 0.0, 5.5, 8, rgb(0.55, 0.4, 0.42), false);
    }
    // Black keep and spire.
    m.boxy(Mat::Stone, Vec3::new(6.4, 9.0, 6.0), Vec3::new(-1.0, 6.4, 0.0), sh(dark, 0.85));
    for s in [-1.0f32, 1.0] {
        for x in [-2.6, -1.0, 0.6] {
            window(&mut m, Vec3::new(x, 8.0, s * 3.0), Vec3::Z * s, 0.5, 1.6);
        }
    }
    m.roof(Vec3::new(-1.0, 10.9, 0.0), Quat::IDENTITY, 6.4, 6.0, 4.2, 0.3, rgb(0.5, 0.36, 0.38), Mat::Stone, dark, Mat::Slate);
    m.frustum(Mat::Stone, Vec3::new(2.6, 10.9, 0.0), 1.5, 1.4, 4.0, 14, dark, true);
    m.frustum(Mat::Slate, Vec3::new(2.6, 14.9, 0.0), 2.0, 0.0, 6.5, 8, rgb(0.5, 0.36, 0.38), false);
    flag_at(&mut m, Vec3::new(2.6, 21.4, 0.0), crimson());
    // Iron gate, rams and cannons.
    m.boxy(Mat::Iron, Vec3::new(0.2, 3.4, 3.4), Vec3::new(7.6, 3.6, 0.0), W);
    for z in [-3.6f32, -1.2, 1.2, 3.6] {
        m.frustum_rot(Mat::Iron, Vec3::new(7.6, 2.4, z), Quat::from_rotation_z(-FRAC_PI_2), 0.6, 0.0, 2.6, 8, W, false, false);
    }
    for z in [-3.0f32, 3.0] {
        m.frustum_rot(Mat::Iron, Vec3::new(4.0, 6.8, z), Quat::from_rotation_z(-FRAC_PI_2), 0.55, 0.45, 3.0, 12, W, true, true);
    }
    for s in [-1.0f32, 1.0] {
        let pts = [Vec3::new(-6.0, 6.0, s * 5.25), Vec3::new(-3.0, 6.0, s * 5.25), Vec3::new(-3.0, 2.6, s * 5.25), Vec3::new(-6.0, 2.6, s * 5.25)];
        m.poly(Mat::Cloth, crimson(), &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], Vec3::Z * s);
    }
    m
}

// ---------------------------------------------------------------------------
// Countryside
// ---------------------------------------------------------------------------

/// A cluster of leaf cards around `c`, filled with a dark core so the crown
/// never looks see-through from above.
fn leaf_cluster(m: &mut Model, rnd: &mut Rand, c: Vec3, r: f32, cards: u32, tint: Color, card_size: f32) {
    m.blob(Mat::Leaf, c, r * 0.72, Vec3::new(1.0, 0.8, 1.0), sh(rgb(0.07, 0.12, 0.05), 1.0), 0.15, rnd.0, 1);
    for _ in 0..cards {
        let p = c + rnd.dir() * r * rnd.f().sqrt() * Vec3::new(1.0, 0.8, 1.0);
        let n = (rnd.dir() + Vec3::Y * 0.7).normalize();
        let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Z };
        let t = n.cross(seed).normalize();
        let b = n.cross(t);
        let a = rnd.range(0.0, TAU);
        let (right, up) = (t * a.cos() + b * a.sin(), -t * a.sin() + b * a.cos());
        let s = card_size * rnd.range(0.8, 1.2);
        let cell = if rnd.f() < 0.6 { LEAF_A } else { LEAF_B };
        let k = rnd.range(0.82, 1.15);
        m.card(Mat::Foliage, p, right * s, up * s, cell.0, cell.1, sh(tint, k), Some(c - Vec3::Y * r * 0.3));
    }
}

/// Leafy broadleaf tree: bark trunk, branches and a crown of leaf cards.
pub fn add_tree(m: &mut Model, pos: Vec3, size: f32, hue: f32, seed: u32) {
    let mut rnd = Rand(seed | 1);
    let tint = rgb(0.95, 1.0, 0.9).mix(&rgb(1.1, 1.05, 0.8), hue);
    m.frustum(Mat::Bark, pos, 0.32 * size, 0.2 * size, 2.4 * size, 9, W, true);
    let crowns = [
        (Vec3::new(0.0, 3.0, 0.0), 1.6),
        (Vec3::new(1.0, 2.5, 0.6), 1.15),
        (Vec3::new(-0.9, 2.6, -0.6), 1.1),
        (Vec3::new(0.2, 3.8, -0.4), 1.05),
        (Vec3::new(-0.3, 2.4, 0.9), 0.95),
    ];
    for (off, r) in crowns {
        let c = pos + off * size;
        // Branch from the trunk into each crown.
        let from = pos + Vec3::Y * 1.9 * size;
        let d = c - from;
        if d.length() > 0.3 {
            m.frustum_rot(Mat::Bark, from, Quat::from_rotation_arc(Vec3::Y, d.normalize()), 0.13 * size, 0.06 * size, d.length() * 0.8, 6, W, true, false);
        }
        leaf_cluster(m, &mut rnd, c, r * size, (14.0 * r) as u32 + 6, tint, 0.85 * size);
    }
}

/// Fir: drooping layers of needle branches around a bark trunk.
pub fn add_pine(m: &mut Model, pos: Vec3, size: f32, seed: u32) {
    let mut rnd = Rand(seed | 1);
    m.frustum(Mat::Bark, pos, 0.28 * size, 0.08 * size, 5.6 * size, 7, W, true);
    m.frustum(Mat::Leaf, pos + Vec3::Y * 1.0 * size, 1.3 * size, 0.0, 4.6 * size, 10, rgb(0.04, 0.09, 0.05), true);
    let tiers = 6;
    for t in 0..tiers {
        let y = (1.1 + t as f32 * 0.75) * size;
        let len = (1.9 - t as f32 * 0.25) * size;
        let n = 9 - t / 2;
        for k in 0..n {
            let a = k as f32 / n as f32 * TAU + rnd.range(-0.25, 0.25) + t as f32 * 0.4;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            let tangent = Vec3::new(-a.sin(), 0.0, a.cos());
            let droop = rnd.range(0.25, 0.5);
            let along = (out * droop.cos() - Vec3::Y * droop.sin()) * (len / 2.0);
            let base = pos + Vec3::Y * y;
            let shade = rgb(0.85, 0.95, 0.9);
            // Card "bottom" (the twig base) sits at the trunk.
            m.card(Mat::Foliage, base + along, tangent * (len * 0.33), along, FIR.0, FIR.1, sh(shade, rnd.range(0.8, 1.1)), Some(pos + Vec3::Y * (y - 1.0)));
        }
    }
    let tip = pos + Vec3::Y * 5.6 * size;
    m.card(Mat::Foliage, tip, Vec3::X * 0.35 * size, Vec3::Y * 0.6 * size, FIR.0, FIR.1, W, None);
    m.card(Mat::Foliage, tip, Vec3::Z * 0.35 * size, Vec3::Y * 0.6 * size, FIR.0, FIR.1, W, None);
}

/// Round shrub.
pub fn add_bush(m: &mut Model, pos: Vec3, size: f32, seed: u32) {
    let mut rnd = Rand(seed | 1);
    let tint = rgb(0.9, 1.0, 0.85);
    leaf_cluster(m, &mut rnd, pos + Vec3::Y * 0.55 * size, 0.85 * size, 14, tint, 0.55 * size);
    if rnd.f() < 0.6 {
        let off = Vec3::new(rnd.range(-0.7, 0.7), 0.4, rnd.range(-0.7, 0.7)) * size;
        leaf_cluster(m, &mut rnd, pos + off, 0.6 * size, 9, tint, 0.45 * size);
    }
}

pub fn add_rock(m: &mut Model, pos: Vec3, size: f32, seed: u32) {
    m.blob(Mat::Stone, pos, size, Vec3::new(1.3, 0.6, 1.0), rgb(0.78, 0.76, 0.72), 0.25, seed, 2);
}

/// A clump of grass blades (with the odd wildflower) as three crossed cards.
pub fn add_grass(m: &mut Model, pos: Vec3, size: f32, seed: u32) {
    let mut rnd = Rand(seed | 1);
    let h = rnd.range(0.35, 0.55) * size;
    let w = rnd.range(0.45, 0.7) * size;
    let base_a = rnd.range(0.0, TAU);
    // Blades fan outward around the clump so they read as a tuft from above.
    for k in 0..5 {
        let a = base_a + k as f32 * TAU / 5.0 + rnd.range(-0.3, 0.3);
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let right = Vec3::new(-a.sin(), 0.0, a.cos()) * w * 0.6;
        let lean = (out * rnd.range(0.7, 1.1) + Vec3::Y).normalize() * h;
        m.card(Mat::Foliage, pos + lean, right, lean, GRASS.0, GRASS.1, sh(rgb(0.95, 1.0, 0.9), rnd.range(0.85, 1.1)), Some(pos - Vec3::Y * 3.0));
    }
}

/// Farmstead: timber-framed house with a thatched roof, a wheat field, fences and a haystack.
pub fn village(seed: u32) -> Model {
    let mut m = Model::new();
    let flip = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    m.boxy(Mat::Wheat, Vec3::new(9.0, 0.45, 7.0), Vec3::new(1.5 * flip, 0.22, 0.0), W);
    for i in 0..10 {
        let x = 1.5 * flip - 4.5 + i as f32 * 1.0;
        m.boxy(Mat::Wood, Vec3::new(0.1, 0.8, 0.1), Vec3::new(x, 0.4, 3.7), timber());
        m.boxy(Mat::Wood, Vec3::new(0.1, 0.8, 0.1), Vec3::new(x, 0.4, -3.7), timber());
    }
    for z in [3.7f32, -3.7] {
        for y in [0.35, 0.65] {
            m.boxy(Mat::Wood, Vec3::new(9.0, 0.07, 0.06), Vec3::new(1.5 * flip, y, z), timber());
        }
    }
    let hx = -4.9 * flip;
    m.boxy(Mat::Stone, Vec3::new(3.4, 0.35, 3.0), Vec3::new(hx, 0.17, -1.0), sh(stone_tint(), 0.85));
    m.boxy(Mat::Plaster, Vec3::new(3.2, 2.2, 2.8), Vec3::new(hx, 1.45, -1.0), W);
    // Timber framing.
    for x in [-1.55, -0.5, 0.5, 1.55] {
        for s in [-1.0f32, 1.0] {
            m.boxy(Mat::Wood, Vec3::new(0.14, 2.2, 0.05), Vec3::new(hx + x, 1.45, -1.0 + s * 1.42), timber());
        }
    }
    for s in [-1.0f32, 1.0] {
        m.boxy(Mat::Wood, Vec3::new(3.2, 0.14, 0.05), Vec3::new(hx, 2.5, -1.0 + s * 1.42), timber());
        m.cube(Mat::Wood, Vec3::new(1.2, 0.12, 0.05), Vec3::new(hx - 1.0, 1.5, -1.0 + s * 1.42), Quat::from_rotation_z(0.7), timber());
        m.cube(Mat::Wood, Vec3::new(1.2, 0.12, 0.05), Vec3::new(hx + 1.0, 1.5, -1.0 + s * 1.42), Quat::from_rotation_z(-0.7), timber());
    }
    m.roof(Vec3::new(hx, 2.55, -1.0), Quat::IDENTITY, 3.2, 2.8, 1.9, 0.35, rgb(0.62, 0.52, 0.36), Mat::Plaster, W, Mat::Wheat);
    m.boxy(Mat::Stone, Vec3::new(0.45, 1.6, 0.45), Vec3::new(hx + 0.9, 3.8, -1.5), sh(stone_tint(), 0.8));
    m.boxy(Mat::Wood, Vec3::new(0.06, 1.1, 0.7), Vec3::new(hx + 1.62 * flip, 0.9, -1.0), sh(timber(), 0.8));
    m.blob(Mat::Wheat, Vec3::new(hx, 0.6, 2.2), 1.0, Vec3::new(1.0, 1.0, 1.0), rgb(0.85, 0.75, 0.55), 0.1, seed + 5, 2);
    m
}
