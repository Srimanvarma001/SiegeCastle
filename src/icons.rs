//! Small procedural pictures for the UI: modules, artifacts and HUD symbols.

use crate::items::{ArtifactKind, ModuleKind};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

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

