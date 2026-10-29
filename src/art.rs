//! All art is built in code: low-poly models made from colored primitives
//! merged into one mesh each, plus small pixel icons for the UI.
//!
//! One shared vertex-colored material is used for almost everything, so
//! thousands of units render in a handful of batched draws.

use crate::items::{ArtifactKind, ModuleKind};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

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

// Palette -------------------------------------------------------------------
pub mod pal {
    use super::rgb;
    use bevy::prelude::Color;
    pub fn wood() -> Color { rgb(0.56, 0.37, 0.22) }
    pub fn wood_dark() -> Color { rgb(0.36, 0.23, 0.15) }
    pub fn wood_light() -> Color { rgb(0.74, 0.56, 0.36) }
    pub fn stone() -> Color { rgb(0.66, 0.63, 0.58) }
    pub fn stone_dark() -> Color { rgb(0.46, 0.44, 0.43) }
    pub fn stone_light() -> Color { rgb(0.80, 0.77, 0.70) }
    pub fn iron() -> Color { rgb(0.27, 0.27, 0.30) }
    pub fn iron_dark() -> Color { rgb(0.16, 0.16, 0.18) }
    pub fn roof() -> Color { rgb(0.66, 0.24, 0.18) }
    pub fn blue() -> Color { rgb(0.24, 0.38, 0.72) }
    pub fn gold() -> Color { rgb(0.96, 0.74, 0.24) }
    pub fn red() -> Color { rgb(0.72, 0.17, 0.14) }
    pub fn red_dark() -> Color { rgb(0.45, 0.10, 0.10) }
    pub fn skin() -> Color { rgb(0.90, 0.72, 0.56) }
    pub fn cream() -> Color { rgb(0.90, 0.84, 0.70) }
    pub fn wheat() -> Color { rgb(0.90, 0.76, 0.38) }
    pub fn wheat_dark() -> Color { rgb(0.76, 0.60, 0.28) }
    pub fn leaf() -> Color { rgb(0.33, 0.50, 0.30) }
    pub fn leaf_dark() -> Color { rgb(0.23, 0.38, 0.25) }
    pub fn leaf_light() -> Color { rgb(0.44, 0.60, 0.34) }
    pub fn pine() -> Color { rgb(0.17, 0.30, 0.22) }
    pub fn green_cloth() -> Color { rgb(0.32, 0.42, 0.22) }
    pub fn white() -> Color { rgb(0.95, 0.95, 0.92) }
    pub fn black() -> Color { rgb(0.10, 0.09, 0.09) }
}

// ---------------------------------------------------------------------------
// Model builder
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Model {
    mesh: Option<Mesh>,
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a primitive mesh: flat-shaded, one solid color, placed by `tf`.
    pub fn push(&mut self, mut m: Mesh, color: Color, tf: Transform) -> &mut Self {
        m.remove_attribute(Mesh::ATTRIBUTE_UV_0);
        m.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
        let mut m = m.transformed_by(tf);
        m.duplicate_vertices();
        m.compute_flat_normals();
        let n = m.count_vertices();
        let c = color.to_linear().to_f32_array();
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![c; n]);
        match &mut self.mesh {
            None => self.mesh = Some(m),
            Some(base) => {
                let _ = base.merge(&m);
            }
        }
        self
    }

    pub fn cube(&mut self, size: Vec3, pos: Vec3, color: Color) -> &mut Self {
        self.push(Mesh::from(Cuboid::from_size(size)), color, Transform::from_translation(pos))
    }

    pub fn cube_rot(&mut self, size: Vec3, pos: Vec3, rot: Quat, color: Color) -> &mut Self {
        self.push(
            Mesh::from(Cuboid::from_size(size)),
            color,
            Transform::from_translation(pos).with_rotation(rot),
        )
    }

    /// Vertical cylinder centered at `pos`.
    pub fn cyl(&mut self, r: f32, h: f32, pos: Vec3, color: Color, res: u32) -> &mut Self {
        self.cyl_rot(r, h, pos, Quat::IDENTITY, color, res)
    }

    pub fn cyl_rot(&mut self, r: f32, h: f32, pos: Vec3, rot: Quat, color: Color, res: u32) -> &mut Self {
        let m = Cylinder::new(r, h).mesh().resolution(res).build();
        self.push(m, color, Transform::from_translation(pos).with_rotation(rot))
    }

    /// Wheel: a cylinder lying on its side, axle along Z.
    pub fn wheel(&mut self, r: f32, w: f32, pos: Vec3) -> &mut Self {
        let rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        self.cyl_rot(r, w, pos, rot, pal::wood_dark(), 10);
        self.cyl_rot(r * 0.35, w + 0.08, pos, rot, pal::iron(), 6)
    }

    pub fn cone(&mut self, r: f32, h: f32, pos: Vec3, color: Color, res: u32) -> &mut Self {
        self.cone_rot(r, h, pos, Quat::IDENTITY, color, res)
    }

    pub fn cone_rot(&mut self, r: f32, h: f32, pos: Vec3, rot: Quat, color: Color, res: u32) -> &mut Self {
        let m = Cone::new(r, h).mesh().resolution(res).build();
        self.push(m, color, Transform::from_translation(pos).with_rotation(rot))
    }

    pub fn frustum(&mut self, r_top: f32, r_bottom: f32, h: f32, pos: Vec3, color: Color, res: u32) -> &mut Self {
        let m = ConicalFrustum { radius_top: r_top, radius_bottom: r_bottom, height: h }
            .mesh()
            .resolution(res)
            .build();
        self.push(m, color, Transform::from_translation(pos))
    }

    pub fn ball(&mut self, r: f32, pos: Vec3, color: Color) -> &mut Self {
        self.ball_scaled(r, Vec3::ONE, pos, color)
    }

    pub fn ball_scaled(&mut self, r: f32, scale: Vec3, pos: Vec3, color: Color) -> &mut Self {
        let m = Sphere::new(r).mesh().ico(1).expect("ico sphere");
        self.push(m, color, Transform::from_translation(pos).with_scale(scale))
    }

    /// Gable roof (triangular prism) along X.
    pub fn roof(&mut self, len: f32, width: f32, pos: Vec3, color: Color) -> &mut Self {
        let s = width / std::f32::consts::SQRT_2;
        self.cube_rot(
            Vec3::new(len, s, s),
            pos,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_4),
            color,
        )
    }

    pub fn build(self) -> Mesh {
        self.mesh.unwrap_or_else(|| Mesh::from(Cuboid::from_size(Vec3::splat(0.01))))
    }
}

// ---------------------------------------------------------------------------
// Player castle: 3 stages, each with a Defense or Movement look.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Variant {
    #[default]
    Base,
    Defense,
    Movement,
}

/// Where module models sit on each castle stage.
pub struct Mounts {
    pub top: [Vec3; 3],
    pub front: Vec3,
    pub back: Vec3,
    pub side: Vec3,
    pub chimney: Vec3,
}

pub fn castle_mounts(stage: usize) -> Mounts {
    match stage {
        0 => Mounts {
            top: [Vec3::new(0.0, 3.7, 0.0), Vec3::new(-1.6, 1.4, 1.2), Vec3::new(-1.6, 1.4, -1.2)],
            front: Vec3::new(3.0, 0.9, 0.0),
            back: Vec3::new(-3.0, 1.0, 0.0),
            side: Vec3::new(0.6, 1.6, 0.0),
            chimney: Vec3::new(-1.2, 4.6, 0.9),
        },
        1 => Mounts {
            top: [Vec3::new(0.3, 5.2, 0.0), Vec3::new(2.6, 3.2, 2.0), Vec3::new(2.6, 3.2, -2.0)],
            front: Vec3::new(4.6, 1.0, 0.0),
            back: Vec3::new(-4.6, 1.2, 0.0),
            side: Vec3::new(1.0, 2.4, 0.0),
            chimney: Vec3::new(-2.6, 6.2, 1.2),
        },
        _ => Mounts {
            top: [Vec3::new(0.0, 8.4, 0.0), Vec3::new(4.2, 4.6, 3.4), Vec3::new(4.2, 4.6, -3.4)],
            front: Vec3::new(6.8, 1.2, 0.0),
            back: Vec3::new(-6.8, 1.4, 0.0),
            side: Vec3::new(1.5, 3.4, 0.0),
            chimney: Vec3::new(-4.0, 9.0, 2.0),
        },
    }
}

pub fn castle_model(stage: usize, variant: Variant) -> Mesh {
    let mut m = Model::new();
    let (len, wid) = match stage {
        0 => (6.0, 4.0),
        1 => (9.0, 6.0),
        _ => (13.0, 9.0),
    };
    // Chassis and wheels.
    m.cube(Vec3::new(len, 0.7, wid - 0.6), Vec3::new(0.0, 1.0, 0.0), pal::wood_dark());
    let wheels = match stage {
        0 => vec![-1.9, 1.9],
        1 => vec![-3.0, 0.0, 3.0],
        _ => vec![-4.8, -1.6, 1.6, 4.8],
    };
    let wr = match (stage, variant) {
        (_, Variant::Movement) => 1.15 + stage as f32 * 0.15,
        _ => 0.9 + stage as f32 * 0.15,
    };
    for &x in &wheels {
        for s in [-1.0, 1.0] {
            m.wheel(wr, 0.55, Vec3::new(x, wr, s * (wid / 2.0)));
        }
    }

    match stage {
        0 => {
            // Small stone keep with one tower.
            m.cube(Vec3::new(3.4, 2.4, 3.0), Vec3::new(0.3, 2.5, 0.0), pal::stone());
            for x in [-1.1, 0.0, 1.1] {
                for z in [-1.3, 1.3] {
                    m.cube(Vec3::new(0.5, 0.45, 0.4), Vec3::new(0.3 + x, 3.9, z), pal::stone_light());
                }
            }
            m.cube(Vec3::new(0.1, 1.2, 0.9), Vec3::new(2.05, 2.0, 0.0), pal::wood_dark());
            m.cyl(0.75, 3.6, Vec3::new(-1.3, 3.0, -1.0), pal::stone_light(), 8);
            m.cone(1.0, 1.4, Vec3::new(-1.3, 5.5, -1.0), pal::roof(), 8);
            m.cyl(0.25, 1.2, Vec3::new(-1.2, 4.0, 0.9), pal::iron_dark(), 6);
            m.cube(Vec3::new(0.08, 2.0, 0.08), Vec3::new(-1.3, 7.0, -1.0), pal::wood_dark());
            m.cube(Vec3::new(0.9, 0.55, 0.05), Vec3::new(-0.85, 7.6, -1.0), pal::blue());
            m.cube(Vec3::new(1.2, 0.4, 0.6), Vec3::new(-2.6, 1.6, 0.0), pal::wood());
        }
        1 => {
            // Walled fort: walls, two towers, central keep.
            m.cube(Vec3::new(7.6, 2.6, 5.0), Vec3::new(0.0, 2.6, 0.0), pal::stone());
            m.cube(Vec3::new(7.2, 0.3, 4.6), Vec3::new(0.0, 3.95, 0.0), pal::wood());
            for i in 0..7 {
                let x = -3.4 + i as f32 * 1.13;
                for z in [-2.35, 2.35] {
                    m.cube(Vec3::new(0.55, 0.55, 0.35), Vec3::new(x, 4.2, z), pal::stone_light());
                }
            }
            m.cube(Vec3::new(3.2, 3.2, 3.0), Vec3::new(-0.8, 5.4, 0.0), pal::stone_light());
            m.roof(3.4, 3.2, Vec3::new(-0.8, 7.4, 0.0), pal::roof());
            for (x, z) in [(2.6, 2.0), (2.6, -2.0)] {
                m.cyl(1.05, 4.6, Vec3::new(x, 3.1, z), pal::stone_light(), 10);
            }
            m.cyl(0.9, 5.0, Vec3::new(-3.0, 3.3, -1.7), pal::stone_light(), 10);
            m.cone(1.25, 1.8, Vec3::new(-3.0, 6.7, -1.7), pal::roof(), 10);
            m.cyl(0.3, 1.6, Vec3::new(-2.6, 5.4, 1.2), pal::iron_dark(), 6);
            m.cube(Vec3::new(0.12, 1.8, 1.6), Vec3::new(3.85, 2.2, 0.0), pal::wood_dark());
            m.cube(Vec3::new(0.1, 2.6, 0.1), Vec3::new(-3.0, 8.9, -1.7), pal::wood_dark());
            m.cube(Vec3::new(1.3, 0.75, 0.05), Vec3::new(-2.35, 9.8, -1.7), pal::blue());
        }
        _ => {
            // Grand citadel: outer walls, 4 towers, great keep with a gold spire.
            m.cube(Vec3::new(11.4, 3.4, 7.8), Vec3::new(0.0, 3.0, 0.0), pal::stone());
            m.cube(Vec3::new(11.0, 0.3, 7.4), Vec3::new(0.0, 4.75, 0.0), pal::wood());
            for i in 0..10 {
                let x = -5.1 + i as f32 * 1.13;
                for z in [-3.75, 3.75] {
                    m.cube(Vec3::new(0.6, 0.6, 0.4), Vec3::new(x, 5.0, z), pal::stone_light());
                }
            }
            for (x, z) in [(4.2, 3.4), (4.2, -3.4), (-4.4, 3.4), (-4.4, -3.4)] {
                m.cyl(1.4, 6.2, Vec3::new(x, 4.0, z), pal::stone_light(), 10);
                if x < 0.0 {
                    m.cone(1.7, 2.2, Vec3::new(x, 8.2, z), pal::roof(), 10);
                }
            }
            m.cube(Vec3::new(4.6, 5.6, 4.2), Vec3::new(-0.6, 6.4, 0.0), pal::stone_light());
            m.cube(Vec3::new(4.8, 0.4, 4.4), Vec3::new(-0.6, 9.3, 0.0), pal::stone_dark());
            m.cyl(1.2, 2.4, Vec3::new(-0.6, 10.6, 0.0), pal::stone_light(), 10);
            m.cone(1.5, 3.0, Vec3::new(-0.6, 13.3, 0.0), pal::gold(), 10);
            m.cyl(0.4, 2.0, Vec3::new(-4.0, 8.0, 2.0), pal::iron_dark(), 6);
            m.cube(Vec3::new(0.15, 2.4, 2.2), Vec3::new(5.75, 2.6, 0.0), pal::wood_dark());
            m.cube(Vec3::new(0.12, 3.0, 0.12), Vec3::new(-0.6, 16.2, 0.0), pal::wood_dark());
            m.cube(Vec3::new(1.8, 1.0, 0.06), Vec3::new(0.3, 17.2, 0.0), pal::blue());
        }
    }

    match variant {
        Variant::Defense => {
            // Iron plating and spikes along the sides.
            let n = 2 + stage * 2;
            for i in 0..n {
                let x = -len / 2.0 + 0.8 + i as f32 * (len - 1.6) / (n as f32 - 1.0).max(1.0);
                for s in [-1.0, 1.0] {
                    m.cube(Vec3::new(1.0, 1.3, 0.15), Vec3::new(x, 2.0, s * (wid / 2.0 - 0.1)), pal::iron());
                    m.cone_rot(0.18, 0.7, Vec3::new(x, 2.0, s * (wid / 2.0 + 0.3)),
                        Quat::from_rotation_x(-s * std::f32::consts::FRAC_PI_2), pal::iron_dark(), 5);
                }
            }
        }
        Variant::Movement => {
            // Exhaust pipes at the back.
            for s in [-1.0, 1.0] {
                m.cyl_rot(0.32, 2.2, Vec3::new(-len / 2.0 - 0.3, 2.2, s * (wid / 4.0)),
                    Quat::from_rotation_z(1.1), pal::iron_dark(), 8);
            }
        }
        Variant::Base => {}
    }
    m.build()
}

pub fn module_model(kind: ModuleKind) -> Mesh {
    let mut m = Model::new();
    match kind {
        ModuleKind::ArrowLoft => {
            m.cube(Vec3::new(1.8, 0.25, 1.8), Vec3::new(0.0, 0.1, 0.0), pal::wood());
            for (x, z) in [(-0.8, -0.8), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8)] {
                m.cube(Vec3::new(0.15, 0.7, 0.15), Vec3::new(x, 0.55, z), pal::wood_dark());
            }
            m.roof(2.0, 2.0, Vec3::new(0.0, 1.35, 0.0), pal::roof());
            for z in [-0.4, 0.4] {
                m.cyl(0.18, 0.5, Vec3::new(0.3, 0.5, z), pal::blue(), 6);
                m.ball(0.16, Vec3::new(0.3, 0.88, z), pal::skin());
            }
        }
        ModuleKind::Mortar => {
            m.cyl(0.9, 0.4, Vec3::new(0.0, 0.2, 0.0), pal::wood_dark(), 8);
            m.frustum(0.55, 0.75, 1.1, Vec3::new(0.0, 0.9, 0.0), pal::iron(), 10);
            m.cyl(0.45, 0.12, Vec3::new(0.0, 1.47, 0.0), pal::iron_dark(), 10);
        }
        ModuleKind::Ram => {
            m.cube(Vec3::new(1.6, 0.6, 1.0), Vec3::new(0.4, 0.6, 0.0), pal::wood());
            m.cone_rot(0.55, 1.4, Vec3::new(1.85, 0.6, 0.0),
                Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron(), 8);
            for z in [-0.6, 0.6] {
                m.cone_rot(0.2, 0.8, Vec3::new(1.2, 0.6, z),
                    Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron_dark(), 5);
            }
        }
        ModuleKind::MineLayer => {
            m.cyl_rot(0.6, 1.2, Vec3::new(-0.2, 0.7, 0.0),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), pal::wood(), 8);
            m.cyl_rot(0.63, 0.12, Vec3::new(-0.5, 0.7, 0.0),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), pal::iron_dark(), 8);
            m.cube(Vec3::new(1.0, 0.15, 0.6), Vec3::new(-0.9, 0.3, 0.0), pal::iron());
        }
        ModuleKind::Broadside => {
            for s in [-1.0, 1.0] {
                m.cyl_rot(0.32, 1.8, Vec3::new(0.0, 0.5, s * 1.2),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), pal::iron_dark(), 8);
                m.cube(Vec3::new(0.8, 0.4, 0.8), Vec3::new(0.0, 0.25, s * 0.5), pal::wood());
            }
        }
    }
    m.build()
}

// ---------------------------------------------------------------------------
// Enemies and bosses
// ---------------------------------------------------------------------------

fn soldier(m: &mut Model, tunic: Color, hat: Color) {
    m.cyl(0.32, 0.8, Vec3::new(0.0, 0.6, 0.0), tunic, 6);
    m.ball(0.24, Vec3::new(0.0, 1.2, 0.0), pal::skin());
    m.cone(0.27, 0.35, Vec3::new(0.0, 1.45, 0.0), hat, 6);
    m.cube(Vec3::new(0.12, 0.4, 0.16), Vec3::new(0.0, 0.2, 0.12), pal::black());
    m.cube(Vec3::new(0.12, 0.4, 0.16), Vec3::new(0.0, 0.2, -0.12), pal::black());
}

pub fn footman_model() -> Mesh {
    let mut m = Model::new();
    soldier(&mut m, pal::red(), pal::iron());
    m.cube(Vec3::new(0.1, 0.6, 0.5), Vec3::new(0.35, 0.7, 0.18), pal::wood_light());
    m.cube_rot(Vec3::new(0.06, 1.9, 0.06), Vec3::new(0.25, 1.0, -0.3), Quat::from_rotation_z(-0.35), pal::wood_dark());
    m.cone_rot(0.08, 0.25, Vec3::new(0.6, 1.95, -0.3), Quat::from_rotation_z(-0.35), pal::iron(), 4);
    m.build()
}

pub fn archer_model() -> Mesh {
    let mut m = Model::new();
    soldier(&mut m, pal::green_cloth(), pal::red_dark());
    m.cube_rot(Vec3::new(0.06, 1.1, 0.06), Vec3::new(0.35, 0.95, 0.0), Quat::from_rotation_x(0.0), pal::wood_dark());
    m.cube(Vec3::new(0.2, 0.5, 0.2), Vec3::new(-0.3, 0.9, 0.0), pal::wood());
    m.build()
}

pub fn knight_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(1.5, 0.65, 0.55), Vec3::new(0.0, 1.0, 0.0), pal::wood());
    m.cube_rot(Vec3::new(0.6, 0.35, 0.35), Vec3::new(0.85, 1.4, 0.0), Quat::from_rotation_z(0.6), pal::wood());
    for (x, z) in [(-0.55, -0.18), (-0.55, 0.18), (0.5, -0.18), (0.5, 0.18)] {
        m.cube(Vec3::new(0.14, 0.7, 0.14), Vec3::new(x, 0.35, z), pal::wood_dark());
    }
    m.cube(Vec3::new(1.0, 0.25, 0.65), Vec3::new(-0.1, 1.2, 0.0), pal::red());
    m.cyl(0.26, 0.6, Vec3::new(-0.1, 1.65, 0.0), pal::iron(), 6);
    m.ball(0.22, Vec3::new(-0.1, 2.1, 0.0), pal::iron());
    m.cube_rot(Vec3::new(2.2, 0.07, 0.07), Vec3::new(0.7, 1.7, 0.25), Quat::from_rotation_z(0.1), pal::wood_light());
    m.build()
}

pub fn cart_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(2.6, 0.9, 1.6), Vec3::new(0.0, 1.0, 0.0), pal::wood());
    m.roof(2.4, 1.7, Vec3::new(-0.1, 1.85, 0.0), pal::red());
    for x in [-0.8, 0.8] {
        for s in [-1.0, 1.0] {
            m.wheel(0.5, 0.25, Vec3::new(x, 0.5, s * 0.9));
        }
    }
    for z in [-0.5, 0.0, 0.5] {
        m.cone_rot(0.15, 0.7, Vec3::new(1.6, 0.9, z), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron(), 5);
    }
    m.build()
}

pub fn siege_tower_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(5.0, 0.6, 4.4), Vec3::new(0.0, 0.9, 0.0), pal::wood_dark());
    for x in [-1.8, 1.8] {
        for s in [-1.0, 1.0] {
            m.wheel(0.8, 0.5, Vec3::new(x, 0.8, s * 2.3));
        }
    }
    m.frustum(1.7, 2.2, 8.0, Vec3::new(0.0, 5.2, 0.0), pal::wood(), 4);
    for i in 0..5 {
        m.cube(Vec3::new(4.0 - i as f32 * 0.25, 0.18, 4.0 - i as f32 * 0.25), Vec3::new(0.0, 2.0 + i as f32 * 1.6, 0.0), pal::wood_dark());
    }
    m.cube(Vec3::new(3.6, 0.3, 3.6), Vec3::new(0.0, 9.3, 0.0), pal::wood_dark());
    for (x, z) in [(1.5, 1.5), (-1.5, 1.5), (1.5, -1.5), (-1.5, -1.5)] {
        m.cube(Vec3::new(0.25, 1.0, 0.25), Vec3::new(x, 9.9, z), pal::wood_dark());
    }
    for (x, z) in [(0.6, 0.6), (-0.6, -0.6), (0.6, -0.7)] {
        m.cyl(0.3, 0.8, Vec3::new(x, 9.85, z), pal::red(), 6);
        m.ball(0.24, Vec3::new(x, 10.45, z), pal::skin());
    }
    m.cube(Vec3::new(0.08, 2.6, 0.08), Vec3::new(-1.5, 11.0, 1.5), pal::wood_dark());
    m.cube(Vec3::new(1.1, 0.7, 0.05), Vec3::new(-0.9, 11.9, 1.5), pal::red());
    m.cube_rot(Vec3::new(0.3, 3.0, 2.6), Vec3::new(2.6, 7.5, 0.0), Quat::from_rotation_z(-0.25), pal::wood_light());
    m.build()
}

pub fn war_wagon_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(5.4, 1.8, 3.2), Vec3::new(0.0, 1.8, 0.0), pal::iron());
    m.cube(Vec3::new(5.0, 0.4, 2.8), Vec3::new(0.0, 2.9, 0.0), pal::wood_dark());
    for x in [-1.8, 0.0, 1.8] {
        for s in [-1.0, 1.0] {
            m.wheel(0.9, 0.45, Vec3::new(x, 0.9, s * 1.75));
        }
    }
    m.cyl(0.8, 0.6, Vec3::new(0.2, 3.4, 0.0), pal::iron_dark(), 8);
    m.cyl_rot(0.35, 2.6, Vec3::new(1.4, 3.6, 0.0), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron_dark(), 8);
    for i in 0..4 {
        m.cube(Vec3::new(0.5, 0.5, 3.3), Vec3::new(-2.0 + i as f32 * 1.3, 2.4, 0.0), pal::red_dark());
    }
    m.build()
}

pub fn juggernaut_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(9.0, 2.6, 5.4), Vec3::new(0.0, 2.2, 0.0), pal::iron());
    m.cube(Vec3::new(7.0, 1.6, 4.4), Vec3::new(-0.6, 4.2, 0.0), pal::iron_dark());
    m.roof(6.0, 3.6, Vec3::new(-0.8, 5.4, 0.0), pal::iron());
    for x in [-3.2, -1.0, 1.2, 3.4] {
        for s in [-1.0, 1.0] {
            m.wheel(1.1, 0.6, Vec3::new(x, 1.1, s * 2.95));
        }
    }
    m.cone_rot(1.5, 3.2, Vec3::new(6.0, 2.2, 0.0), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron_dark(), 8);
    for z in [-1.8, 1.8] {
        m.cone_rot(0.5, 1.8, Vec3::new(5.0, 1.6, z), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron_dark(), 6);
        m.cube(Vec3::new(0.3, 0.3, 0.6), Vec3::new(4.55, 3.0, z * 0.6), rgb(1.0, 0.25, 0.1));
    }
    for i in 0..6 {
        m.cone(0.35, 1.0, Vec3::new(-3.0 + i as f32 * 1.1, 5.2, 0.0), pal::iron_dark(), 5);
    }
    m.cyl(0.45, 2.0, Vec3::new(-3.4, 6.2, 1.4), pal::iron_dark(), 6);
    m.build()
}

pub fn trebuchet_model() -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(8.0, 1.0, 6.0), Vec3::new(0.0, 1.3, 0.0), pal::wood_dark());
    for x in [-2.8, 0.0, 2.8] {
        for s in [-1.0, 1.0] {
            m.wheel(1.0, 0.55, Vec3::new(x, 1.0, s * 3.1));
        }
    }
    for z in [-1.2, 1.2] {
        m.cube_rot(Vec3::new(0.4, 5.5, 0.4), Vec3::new(-0.6, 4.2, z), Quat::from_rotation_z(0.2), pal::wood());
        m.cube_rot(Vec3::new(0.4, 5.5, 0.4), Vec3::new(0.6, 4.2, z), Quat::from_rotation_z(-0.2), pal::wood());
    }
    m.cube_rot(Vec3::new(9.0, 0.45, 0.45), Vec3::new(-0.5, 7.4, 0.0), Quat::from_rotation_z(0.45), pal::wood_light());
    m.cube(Vec3::new(1.8, 1.8, 1.8), Vec3::new(3.6, 6.2, 0.0), pal::stone_dark());
    m.ball(0.8, Vec3::new(-4.4, 5.6, 0.0), pal::stone());
    m.cube(Vec3::new(2.4, 3.0, 2.4), Vec3::new(-2.6, 3.2, -1.9), pal::stone());
    m.cone(1.8, 1.6, Vec3::new(-2.6, 5.5, -1.9), pal::red(), 4);
    m.build()
}

pub fn warlord_model() -> Mesh {
    let mut m = Model::new();
    let dark = rgb(0.30, 0.27, 0.27);
    let darker = rgb(0.20, 0.18, 0.18);
    m.cube(Vec3::new(17.0, 1.2, 12.0), Vec3::new(0.0, 1.4, 0.0), pal::wood_dark());
    for x in [-6.6, -3.3, 0.0, 3.3, 6.6] {
        for s in [-1.0, 1.0] {
            m.wheel(1.5, 0.8, Vec3::new(x, 1.5, s * 6.3));
        }
    }
    m.cube(Vec3::new(15.0, 4.2, 10.4), Vec3::new(0.0, 4.0, 0.0), dark);
    for (x, z) in [(6.6, 4.4), (6.6, -4.4), (-6.6, 4.4), (-6.6, -4.4)] {
        m.cyl(2.0, 8.6, Vec3::new(x, 5.6, z), darker, 10);
        m.cone(2.5, 3.6, Vec3::new(x, 11.7, z), pal::red_dark(), 10);
    }
    m.cube(Vec3::new(6.4, 8.0, 6.0), Vec3::new(-1.0, 8.0, 0.0), darker);
    m.roof(6.8, 6.4, Vec3::new(-1.0, 13.4, 0.0), pal::red_dark());
    m.cyl(1.5, 4.0, Vec3::new(2.6, 13.0, 0.0), dark, 8);
    m.cone(2.0, 4.4, Vec3::new(2.6, 17.2, 0.0), pal::red_dark(), 8);
    m.cube(Vec3::new(0.2, 3.4, 3.4), Vec3::new(7.6, 3.6, 0.0), pal::iron_dark());
    for z in [-3.6, -1.2, 1.2, 3.6] {
        m.cone_rot(0.6, 2.6, Vec3::new(8.6, 2.0, z), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron(), 6);
    }
    for z in [-3.0, 3.0] {
        m.cyl_rot(0.55, 3.0, Vec3::new(4.6, 6.6, z), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron_dark(), 8);
    }
    m.cube(Vec3::new(0.15, 4.0, 0.15), Vec3::new(2.6, 21.0, 0.0), pal::wood_dark());
    m.cube(Vec3::new(2.6, 1.4, 0.08), Vec3::new(3.9, 22.2, 0.0), pal::red());
    m.build()
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

/// A round leafy tree, added to a bigger merged model.
pub fn add_tree(m: &mut Model, pos: Vec3, size: f32, tint: f32) {
    let shade = |c: Color| c.mix(&pal::leaf_light(), tint);
    m.cyl(0.25 * size, 1.4 * size, pos + Vec3::Y * 0.7 * size, pal::wood_dark(), 5);
    m.ball(1.25 * size, pos + Vec3::new(0.0, 2.1 * size, 0.0), shade(pal::leaf()));
    m.ball(0.95 * size, pos + Vec3::new(0.7 * size, 1.7 * size, 0.5 * size), shade(pal::leaf_dark()));
    m.ball(0.85 * size, pos + Vec3::new(-0.6 * size, 1.8 * size, -0.5 * size), shade(pal::leaf()));
}

pub fn add_pine(m: &mut Model, pos: Vec3, size: f32) {
    m.cyl(0.25 * size, 1.0 * size, pos + Vec3::Y * 0.5 * size, pal::wood_dark(), 5);
    m.cone(1.4 * size, 2.4 * size, pos + Vec3::Y * 2.0 * size, pal::pine(), 7);
    m.cone(1.0 * size, 2.0 * size, pos + Vec3::Y * 3.2 * size, pal::pine(), 7);
}

pub fn add_rock(m: &mut Model, pos: Vec3, size: f32) {
    m.ball_scaled(size, Vec3::new(1.2, 0.6, 1.0), pos, pal::stone());
}

pub fn add_tuft(m: &mut Model, pos: Vec3, size: f32) {
    let c = rgb(0.42, 0.52, 0.28);
    m.cone(0.12 * size, 0.6 * size, pos + Vec3::new(0.0, 0.3 * size, 0.0), c, 3);
    m.cone(0.1 * size, 0.45 * size, pos + Vec3::new(0.15 * size, 0.22 * size, 0.1 * size), c, 3);
}

/// Farm: a house and a wheat field. Driving through it absorbs it.
pub fn village_model(seed: u32) -> Mesh {
    let mut m = Model::new();
    let flip = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    // Wheat field with rows.
    m.cube(Vec3::new(9.0, 0.35, 7.0), Vec3::new(1.5 * flip, 0.18, 0.0), pal::wheat());
    for i in 0..7 {
        m.cube(Vec3::new(8.6, 0.12, 0.25), Vec3::new(1.5 * flip, 0.4, -3.0 + i as f32 * 1.0), pal::wheat_dark());
    }
    for i in 0..10 {
        let x = 1.5 * flip - 4.5 + i as f32 * 1.0;
        m.cube(Vec3::new(0.12, 0.6, 0.12), Vec3::new(x, 0.3, 3.6), pal::wood_dark());
    }
    m.cube(Vec3::new(9.0, 0.08, 0.08), Vec3::new(1.5 * flip, 0.5, 3.6), pal::wood());
    // House.
    let hx = -4.6 * flip;
    m.cube(Vec3::new(3.0, 2.0, 2.6), Vec3::new(hx, 1.0, -1.0), pal::cream());
    m.roof(3.4, 3.0, Vec3::new(hx, 2.4, -1.0), pal::roof());
    m.cube(Vec3::new(0.1, 1.0, 0.7), Vec3::new(hx + 1.52 * flip, 0.5, -1.0), pal::wood_dark());
    m.ball_scaled(0.9, Vec3::new(1.0, 1.1, 1.0), Vec3::new(hx, 0.8, 2.0), pal::wheat());
    m.build()
}

pub fn sheep_model() -> Mesh {
    let mut m = Model::new();
    m.ball_scaled(0.55, Vec3::new(1.25, 0.9, 1.0), Vec3::new(0.0, 0.75, 0.0), pal::white());
    m.ball(0.32, Vec3::new(0.25, 1.05, 0.0), pal::white());
    m.ball_scaled(0.24, Vec3::new(1.3, 1.0, 0.9), Vec3::new(0.75, 0.85, 0.0), pal::black());
    for (x, z) in [(-0.35, -0.22), (-0.35, 0.22), (0.35, -0.22), (0.35, 0.22)] {
        m.cube(Vec3::new(0.12, 0.4, 0.12), Vec3::new(x, 0.2, z), pal::black());
    }
    m.build()
}

pub fn coin_model() -> Mesh {
    let mut m = Model::new();
    m.cyl_rot(0.42, 0.14, Vec3::ZERO, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), pal::gold(), 10);
    m.cyl_rot(0.26, 0.17, Vec3::ZERO, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), rgb(1.0, 0.86, 0.45), 10);
    m.build()
}

pub fn arrow_model(color: Color) -> Mesh {
    let mut m = Model::new();
    m.cube(Vec3::new(1.1, 0.07, 0.07), Vec3::ZERO, color);
    m.cone_rot(0.09, 0.25, Vec3::new(0.65, 0.0, 0.0), Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), pal::iron(), 4);
    m.cube(Vec3::new(0.25, 0.02, 0.2), Vec3::new(-0.5, 0.0, 0.0), pal::white());
    m.build()
}

pub fn mine_model() -> Mesh {
    let mut m = Model::new();
    m.frustum(0.35, 0.55, 0.3, Vec3::new(0.0, 0.15, 0.0), pal::iron_dark(), 8);
    m.cyl(0.12, 0.12, Vec3::new(0.0, 0.36, 0.0), rgb(1.0, 0.2, 0.1), 6);
    m.build()
}

pub fn pointer_model() -> Mesh {
    let mut m = Model::new();
    m.cone_rot(0.8, 1.8, Vec3::ZERO, Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), rgb(1.0, 0.25, 0.15), 3);
    m.build()
}

/// The ground: a big grid with soft color patches.
pub fn ground_mesh(half: f32, cells: u32, color_at: impl Fn(Vec2) -> Color) -> Mesh {
    let n = cells + 1;
    let step = 2.0 * half / cells as f32;
    let mut pos = Vec::with_capacity((n * n) as usize);
    let mut col = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let p = Vec2::new(-half + i as f32 * step, -half + j as f32 * step);
            pos.push(w3(p, 0.0).to_array());
            col.push(color_at(p).to_linear().to_f32_array());
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
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
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
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![c; count])
        .with_inserted_indices(Indices::U32(idx));
    mesh.duplicate_vertices();
    mesh
}

// ---------------------------------------------------------------------------
// UI icons: tiny procedural pictures, 64x64.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Module(ModuleKind),
    Artifact(ArtifactKind),
    Defense,
    Movement,
    Nitro,
    Skull,
}

#[derive(Resource, Default)]
pub struct Icons(pub HashMap<Icon, Handle<Image>>);

impl Icons {
    pub fn get(&self, icon: Icon) -> Handle<Image> {
        self.0.get(&icon).cloned().unwrap_or_default()
    }
}

struct Canvas {
    px: Vec<[u8; 4]>,
}

const N: i32 = 64;

impl Canvas {
    fn new() -> Self {
        Self { px: vec![[0, 0, 0, 0]; (N * N) as usize] }
    }
    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if (0..N).contains(&x) && (0..N).contains(&y) {
            self.px[(y * N + x) as usize] = c;
        }
    }
    fn circle(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 4]) {
        for y in 0..N {
            for x in 0..N {
                let d = Vec2::new(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy).length();
                if d <= r {
                    self.set(x, y, c);
                }
            }
        }
    }
    fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: [u8; 4]) {
        for y in 0..N {
            for x in 0..N {
                let d = Vec2::new(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy).length();
                if (d - r).abs() <= t / 2.0 {
                    self.set(x, y, c);
                }
            }
        }
    }
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 4]) {
        for y in y0 as i32..y1 as i32 {
            for x in x0 as i32..x1 as i32 {
                self.set(x, y, c);
            }
        }
    }
    fn line(&mut self, a: Vec2, b: Vec2, t: f32, c: [u8; 4]) {
        let ab = b - a;
        let len2 = ab.length_squared().max(0.0001);
        for y in 0..N {
            for x in 0..N {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let h = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
                if (p - a - ab * h).length() <= t / 2.0 {
                    self.set(x, y, c);
                }
            }
        }
    }
    fn tri(&mut self, a: Vec2, b: Vec2, c3: Vec2, c: [u8; 4]) {
        let s = |p: Vec2, q: Vec2, r: Vec2| (p.x - r.x) * (q.y - r.y) - (q.x - r.x) * (p.y - r.y);
        for y in 0..N {
            for x in 0..N {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let (d1, d2, d3) = (s(p, a, b), s(p, b, c3), s(p, c3, a));
                let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                if !(neg && pos) {
                    self.set(x, y, c);
                }
            }
        }
    }
    fn image(self) -> Image {
        let data: Vec<u8> = self.px.into_iter().flatten().collect();
        Image::new(
            Extent3d { width: N as u32, height: N as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }
}

const IRON: [u8; 4] = [70, 70, 78, 255];
const DARK: [u8; 4] = [30, 28, 30, 255];
const WOOD: [u8; 4] = [150, 96, 56, 255];
const GOLD: [u8; 4] = [245, 190, 60, 255];
const RED: [u8; 4] = [210, 60, 45, 255];
const ORANGE: [u8; 4] = [255, 150, 40, 255];
const WHITE: [u8; 4] = [240, 236, 228, 255];
const BLUE: [u8; 4] = [70, 140, 240, 255];
const GREEN: [u8; 4] = [110, 190, 80, 255];
const STONE: [u8; 4] = [170, 165, 155, 255];

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn draw(icon: Icon) -> Canvas {
    let mut c = Canvas::new();
    match icon {
        Icon::Module(ModuleKind::ArrowLoft) => {
            for (dx, col) in [(-10.0, WOOD), (0.0, WOOD), (10.0, WOOD)] {
                c.line(v(18.0 + dx, 50.0), v(42.0 + dx, 14.0), 4.0, col);
                c.tri(v(44.0 + dx, 10.0), v(36.0 + dx, 14.0), v(43.0 + dx, 20.0), IRON);
                c.line(v(16.0 + dx, 53.0), v(22.0 + dx, 46.0), 5.0, WHITE);
            }
        }
        Icon::Module(ModuleKind::Mortar) => {
            c.rect(14.0, 46.0, 50.0, 54.0, WOOD);
            c.tri(v(18.0, 46.0), v(46.0, 46.0), v(38.0, 22.0), IRON);
            c.tri(v(18.0, 46.0), v(38.0, 22.0), v(26.0, 22.0), IRON);
            c.rect(24.0, 18.0, 40.0, 24.0, DARK);
            c.circle(46.0, 12.0, 6.0, DARK);
            c.circle(52.0, 6.0, 3.0, ORANGE);
        }
        Icon::Module(ModuleKind::Ram) => {
            c.rect(8.0, 26.0, 36.0, 40.0, WOOD);
            c.tri(v(36.0, 20.0), v(36.0, 46.0), v(58.0, 33.0), IRON);
            c.tri(v(30.0, 16.0), v(30.0, 24.0), v(44.0, 20.0), DARK);
            c.tri(v(30.0, 42.0), v(30.0, 50.0), v(44.0, 46.0), DARK);
        }
        Icon::Module(ModuleKind::MineLayer) => {
            c.circle(32.0, 36.0, 16.0, DARK);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let d = Vec2::from_angle(a);
                c.line(v(32.0, 36.0) + d * 14.0, v(32.0, 36.0) + d * 22.0, 4.0, IRON);
            }
            c.circle(32.0, 36.0, 5.0, RED);
        }
        Icon::Module(ModuleKind::Broadside) => {
            c.rect(22.0, 22.0, 42.0, 44.0, WOOD);
            c.rect(4.0, 28.0, 24.0, 38.0, DARK);
            c.rect(40.0, 28.0, 60.0, 38.0, DARK);
            c.circle(4.0, 33.0, 4.0, ORANGE);
            c.circle(60.0, 33.0, 4.0, ORANGE);
        }
        Icon::Artifact(a) => draw_artifact(&mut c, a),
        Icon::Defense => {
            c.tri(v(10.0, 12.0), v(54.0, 12.0), v(32.0, 58.0), STONE);
            c.rect(10.0, 8.0, 54.0, 20.0, STONE);
            c.tri(v(18.0, 16.0), v(46.0, 16.0), v(32.0, 48.0), IRON);
        }
        Icon::Movement => {
            c.ring(32.0, 32.0, 18.0, 8.0, IRON);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                c.line(v(32.0, 32.0), v(32.0, 32.0) + Vec2::from_angle(a) * 24.0, 6.0, IRON);
            }
            c.circle(32.0, 32.0, 7.0, STONE);
        }
        Icon::Nitro => {
            c.circle(32.0, 32.0, 26.0, BLUE);
            c.tri(v(36.0, 8.0), v(18.0, 36.0), v(34.0, 34.0), WHITE);
            c.tri(v(30.0, 56.0), v(46.0, 28.0), v(30.0, 30.0), WHITE);
        }
        Icon::Skull => {
            c.circle(32.0, 28.0, 18.0, WHITE);
            c.rect(22.0, 38.0, 42.0, 52.0, WHITE);
            c.circle(25.0, 28.0, 5.0, DARK);
            c.circle(39.0, 28.0, 5.0, DARK);
            for x in [26.0, 32.0, 38.0] {
                c.rect(x - 1.0, 44.0, x + 1.0, 52.0, DARK);
            }
        }
    }
    c
}

fn draw_artifact(c: &mut Canvas, a: ArtifactKind) {
    match a {
        ArtifactKind::Whetstone => {
            c.rect(10.0, 40.0, 54.0, 52.0, STONE);
            c.line(v(14.0, 34.0), v(50.0, 10.0), 6.0, IRON);
            c.line(v(14.0, 34.0), v(22.0, 28.0), 8.0, WOOD);
        }
        ArtifactKind::GreasedAxles => {
            c.ring(32.0, 32.0, 16.0, 7.0, WOOD);
            c.line(v(8.0, 32.0), v(56.0, 32.0), 5.0, IRON);
            c.circle(48.0, 14.0, 6.0, GOLD);
        }
        ArtifactKind::Lodestone => {
            c.ring(32.0, 28.0, 16.0, 10.0, RED);
            c.rect(10.0, 28.0, 27.0, 50.0, RED);
            c.rect(37.0, 28.0, 54.0, 50.0, BLUE);
            c.rect(10.0, 44.0, 27.0, 52.0, WHITE);
            c.rect(37.0, 44.0, 54.0, 52.0, WHITE);
        }
        ArtifactKind::ReinforcedHull => {
            c.rect(10.0, 12.0, 54.0, 52.0, IRON);
            for (x, y) in [(16.0, 18.0), (48.0, 18.0), (16.0, 46.0), (48.0, 46.0)] {
                c.circle(x, y, 3.5, STONE);
            }
            c.rect(20.0, 28.0, 44.0, 36.0, STONE);
        }
        ArtifactKind::QuickHands => {
            c.circle(32.0, 34.0, 22.0, STONE);
            c.circle(32.0, 34.0, 18.0, WHITE);
            c.line(v(32.0, 34.0), v(32.0, 20.0), 4.0, DARK);
            c.line(v(32.0, 34.0), v(44.0, 38.0), 4.0, DARK);
            c.rect(28.0, 6.0, 36.0, 12.0, STONE);
        }
        ArtifactKind::BlackPowder => {
            c.circle(32.0, 38.0, 18.0, DARK);
            c.rect(26.0, 12.0, 38.0, 22.0, WOOD);
            c.line(v(32.0, 12.0), v(40.0, 4.0), 3.0, WOOD);
            c.circle(42.0, 4.0, 4.0, ORANGE);
        }
        ArtifactKind::HawkEye => {
            c.tri(v(4.0, 32.0), v(32.0, 14.0), v(60.0, 32.0), WHITE);
            c.tri(v(4.0, 32.0), v(32.0, 50.0), v(60.0, 32.0), WHITE);
            c.circle(32.0, 32.0, 11.0, GOLD);
            c.circle(32.0, 32.0, 5.0, DARK);
        }
        ArtifactKind::TinkersKit => {
            c.line(v(14.0, 50.0), v(42.0, 22.0), 7.0, IRON);
            c.ring(46.0, 18.0, 9.0, 6.0, IRON);
            c.rect(40.0, 6.0, 48.0, 18.0, Default::default());
            c.line(v(14.0, 14.0), v(50.0, 50.0), 6.0, WOOD);
        }
        ArtifactKind::Poultice => {
            c.circle(32.0, 32.0, 24.0, GREEN);
            c.rect(27.0, 14.0, 37.0, 50.0, WHITE);
            c.rect(14.0, 27.0, 50.0, 37.0, WHITE);
        }
        ArtifactKind::GoldenFleece => {
            for (x, y) in [(22.0, 30.0), (34.0, 24.0), (44.0, 32.0), (30.0, 38.0), (40.0, 42.0)] {
                c.circle(x, y, 10.0, GOLD);
            }
            c.circle(52.0, 22.0, 6.0, DARK);
        }
        ArtifactKind::NitroTank => {
            c.rect(18.0, 14.0, 46.0, 54.0, BLUE);
            c.circle(32.0, 14.0, 14.0, BLUE);
            c.rect(28.0, 2.0, 36.0, 10.0, IRON);
            c.tri(v(34.0, 22.0), v(24.0, 38.0), v(33.0, 37.0), WHITE);
            c.tri(v(30.0, 50.0), v(40.0, 34.0), v(31.0, 35.0), WHITE);
        }
        ArtifactKind::IronSpikes => {
            c.rect(6.0, 44.0, 58.0, 54.0, IRON);
            for x in [12.0, 24.0, 36.0, 48.0] {
                c.tri(v(x - 5.0, 44.0), v(x + 5.0, 44.0), v(x, 14.0), STONE);
            }
        }
    }
}

pub fn build_icons(images: &mut Assets<Image>) -> Icons {
    let mut map = HashMap::default();
    let mut all = vec![Icon::Defense, Icon::Movement, Icon::Nitro, Icon::Skull];
    all.extend(ModuleKind::ALL.iter().map(|&m| Icon::Module(m)));
    all.extend(ArtifactKind::ALL.iter().map(|&a| Icon::Artifact(a)));
    for icon in all {
        map.insert(icon, images.add(draw(icon).image()));
    }
    Icons(map)
}
