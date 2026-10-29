//! Shared art helpers: plane/world coordinates, palette, castle mount points,
//! and the big terrain and river meshes. Models live in `models.rs`.

pub use crate::icons::{build_icons, Icon, Icons};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Coordinates: gameplay runs on a 2D plane. Vec2 (x, y) maps to world
// (x, height, -y), so +y on the plane is "up" on screen.
// ---------------------------------------------------------------------------

pub fn w3(p: Vec2, h: f32) -> Vec3 {
    Vec3::new(p.x, h, -p.y)
}

pub fn p2(v: Vec3) -> Vec2 {
    Vec2::new(v.x, -v.z)
}

/// Rotation that makes a model (built facing +X) face plane angle `a`.
pub fn yaw(a: f32) -> Quat {
    Quat::from_rotation_y(a)
}

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Variant {
    #[default]
    Base,
    Defense,
    Movement,
}

/// Where module models sit on each castle stage (model space, +X = front).
pub struct Mounts {
    pub top: [Vec3; 3],
    /// Which way each top weapon faces (turn about Y; side-wall ones face out).
    pub top_yaw: [f32; 3],
    pub front: Vec3,
    pub back: Vec3,
    /// Right side wall; the left side is the mirror (-Z).
    pub side: Vec3,
    pub chimney: Vec3,
    /// Overall footprint (length along X, width along Z).
    pub len: f32,
    pub wid: f32,
}

pub fn castle_mounts(stage: usize) -> Mounts {
    use std::f32::consts::FRAC_PI_2;
    match stage {
        // Keep roof, the deck behind the keep, the deck in front of it.
        0 => Mounts {
            top: [Vec3::new(0.1, 4.75, 0.0), Vec3::new(-2.35, 1.35, -0.75), Vec3::new(2.25, 1.35, -1.2)],
            top_yaw: [0.0; 3],
            front: Vec3::new(3.1, 1.0, 0.0),
            back: Vec3::new(-3.1, 1.1, 0.0),
            side: Vec3::new(0.3, 2.4, 2.05),
            chimney: Vec3::new(1.2, 5.9, -0.9),
            len: 6.2,
            wid: 4.0,
        },
        // Gate platform, and stone platforms on the side walls.
        1 => Mounts {
            top: [Vec3::new(3.65, 3.75, 0.0), Vec3::new(-0.6, 3.75, -2.85), Vec3::new(-0.6, 3.75, 2.85)],
            top_yaw: [0.0, FRAC_PI_2, -FRAC_PI_2],
            front: Vec3::new(4.75, 1.1, 0.0),
            back: Vec3::new(-4.75, 1.2, 0.0),
            side: Vec3::new(1.8, 2.6, 4.0),
            chimney: Vec3::new(-1.3, 7.6, 0.75),
            len: 9.4,
            wid: 6.2,
        },
        _ => Mounts {
            top: [Vec3::new(5.5, 4.85, 0.0), Vec3::new(0.0, 4.85, -3.5), Vec3::new(0.0, 4.85, 3.5)],
            top_yaw: [0.0, FRAC_PI_2, -FRAC_PI_2],
            front: Vec3::new(6.95, 1.2, 0.0),
            back: Vec3::new(-6.95, 1.4, 0.0),
            side: Vec3::new(2.8, 3.2, 5.65),
            chimney: Vec3::new(-1.9, 9.4, 1.1),
            len: 13.6,
            wid: 9.2,
        },
    }
}

/// The ground: a big grid, textured, with soft color patches as a tint.
/// UVs are in meters; the material scales them to the texture size.
pub fn ground_mesh(half: f32, cells: u32, color_at: impl Fn(Vec2) -> Color) -> Mesh {
    let n = cells + 1;
    let step = 2.0 * half / cells as f32;
    let mut pos = Vec::with_capacity((n * n) as usize);
    let mut col = Vec::with_capacity((n * n) as usize);
    let mut uv = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let p = Vec2::new(-half + i as f32 * step, -half + j as f32 * step);
            pos.push(w3(p, 0.0).to_array());
            col.push(color_at(p).to_linear().to_f32_array());
            uv.push([p.x, p.y]);
        }
    }
    let mut idx = Vec::with_capacity((cells * cells * 6) as usize);
    for j in 0..cells {
        for i in 0..cells {
            let a = j * n + i;
            let b = a + 1;
            let c = a + n;
            let d = c + 1;
            idx.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    let count = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx));
    let _ = mesh.generate_tangents();
    mesh
}

/// A flat ribbon (river) following `points`.
pub fn ribbon_mesh(points: &[Vec2], width: f32, height: f32, color: Color) -> Mesh {
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let prev = points[i.saturating_sub(1)];
        let next = points[(i + 1).min(points.len() - 1)];
        let dir = (next - prev).normalize_or(Vec2::X);
        let side = Vec2::new(-dir.y, dir.x) * width / 2.0;
        pos.push(w3(*p + side, height).to_array());
        pos.push(w3(*p - side, height).to_array());
        if i > 0 {
            let b = (i as u32 - 1) * 2;
            idx.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 1, b + 3]);
        }
    }
    let count = pos.len();
    let c = color.to_linear().to_f32_array();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![c; count])
        .with_inserted_indices(Indices::U32(idx))
}

/// Road ribbon: U runs across (0..1), V along the road in 6 m repeats.
pub fn road_mesh(points: &[Vec2], width: f32, height: f32) -> Mesh {
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut idx = Vec::new();
    let mut dist = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            dist += points[i - 1].distance(*p);
        }
        let prev = points[i.saturating_sub(1)];
        let next = points[(i + 1).min(points.len() - 1)];
        let dir = (next - prev).normalize_or(Vec2::X);
        let side = Vec2::new(-dir.y, dir.x) * width / 2.0;
        pos.push(w3(*p + side, height).to_array());
        pos.push(w3(*p - side, height).to_array());
        uv.push([0.0, dist / 6.0]);
        uv.push([1.0, dist / 6.0]);
        if i > 0 {
            let b = (i as u32 - 1) * 2;
            idx.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
        }
    }
    let count = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}
