//! Visual effects: smoke, fire, sparks, flashes of light, flying debris and
//! marks left on the ground.
//!
//! Smoke, fire and sparks are flat sprites that always face the camera,
//! painted with soft textures generated at startup. Marks on the ground
//! (blood, scorch, wheel ruts) are flat decals that fade out.

use crate::art::w3;
use crate::assets::GameAssets;
use crate::texgen::{fbm, image_with_mips_sized, smooth};
use crate::{GameEntity, GameRng, Sim};
use bevy::image::{ImageAddressMode, ImageSampler};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use rand::Rng;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_fx.in_set(Sim::Resolve));
    }
}

/// Effects alive last frame. Above `max_fx()`, purely cosmetic extras are skipped.
static LIVE: AtomicUsize = AtomicUsize::new(0);
/// Flash lights alive (each one costs real lighting work).
static LIGHTS: AtomicUsize = AtomicUsize::new(0);
static SEED: AtomicU32 = AtomicU32::new(0x9e37_79b9);

/// Effect detail from the graphics preset: 0 = Low, 1 = Medium, 2 = High.
static DETAIL: AtomicUsize = AtomicUsize::new(2);

/// Sets the effect detail for a graphics preset (see `graphics::apply`).
pub fn set_detail(level: usize) {
    DETAIL.store(level.min(2), Ordering::Relaxed);
}

fn detail() -> usize {
    DETAIL.load(Ordering::Relaxed)
}

/// Most effects alive at once before cosmetic ones are skipped.
fn max_fx() -> usize {
    [1200, 2200, 3200][detail()]
}

/// Most flash lights at once; each one costs real lighting work.
fn max_lights() -> usize {
    [3, 6, 10][detail()]
}

/// Most fallen troops lying around at once.
pub fn max_corpses() -> usize {
    [60, 110, crate::enemy::MAX_CORPSES][detail()]
}

/// Cheap random number in 0..1, for variety in effects.
pub fn rnd() -> f32 {
    let mut x = SEED.fetch_add(0x6d2b_79f5, Ordering::Relaxed);
    x = (x ^ (x >> 15)).wrapping_mul(x | 1);
    x ^= x.wrapping_add((x ^ (x >> 7)).wrapping_mul(x | 61));
    ((x ^ (x >> 14)) >> 8) as f32 / 16_777_216.0
}

/// The game camera always looks down from the same angle (see
/// `world::follow_camera`), so sprites can share one rotation.
pub fn facing() -> Quat {
    Quat::from_rotation_arc(Vec3::Z, Vec3::new(0.0, 1.0, 0.42).normalize())
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

pub struct VfxAssets {
    /// Camera-facing card (1 x 1).
    pub quad: Handle<Mesh>,
    /// Flat card on the ground (1 x 1).
    pub decal: Handle<Mesh>,
    pub smoke: Vec<Handle<StandardMaterial>>,
    pub smoke_dark: Vec<Handle<StandardMaterial>>,
    pub dust: Vec<Handle<StandardMaterial>>,
    pub fire: Vec<Handle<StandardMaterial>>,
    pub spark: Handle<StandardMaterial>,
    pub ember: Handle<StandardMaterial>,
    pub muzzle: Handle<StandardMaterial>,
    pub shock: Vec<Handle<StandardMaterial>>,
    pub blood: Vec<Handle<StandardMaterial>>,
    pub scorch: Vec<Handle<StandardMaterial>>,
    pub rut: Vec<Handle<StandardMaterial>>,
    pub patch: Vec<Handle<StandardMaterial>>,
    pub chip: Handle<StandardMaterial>,
}

/// Paints an RGBA texture. `f` gets coordinates in -1..1 (y down) and the
/// raw 0..1 texture coordinates.
fn paint(w: usize, h: usize, f: impl Fn(Vec2, Vec2) -> [f32; 4]) -> Vec<[f32; 4]> {
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let uv = Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            px.push(f(uv * 2.0 - Vec2::ONE, uv));
        }
    }
    px
}

fn texture(images: &mut Assets<Image>, px: Vec<[f32; 4]>, w: usize, h: usize, repeat_v: bool) -> Handle<Image> {
    let mut img = image_with_mips_sized(px, w, h, TextureFormat::Rgba8UnormSrgb, false);
    if let ImageSampler::Descriptor(s) = &mut img.sampler {
        s.address_mode_u = ImageAddressMode::ClampToEdge;
        s.address_mode_v = if repeat_v { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    }
    images.add(img)
}

/// A soft, billowing puff, lit from the left like the rest of the scene.
fn puff_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let n = fbm(uv.x, uv.y, 4, 4, 71);
        let d = p.length() + (n - 0.5) * 0.6;
        let body = 1.0 - smooth(0.3, 0.95, d);
        let wisps = 0.6 + 0.4 * fbm(uv.x, uv.y, 8, 3, 77);
        let lit = (0.55 - 0.38 * p.x + 0.12 * p.y + (n - 0.5) * 0.5).clamp(0.0, 1.0);
        let shade = 0.6 + 0.4 * lit;
        [shade, shade * 0.99, shade * 0.97, (body * wisps).clamp(0.0, 1.0)]
    })
}

/// A ball of flame: white-hot core, orange body, ragged red edges.
fn fire_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let n = fbm(uv.x, uv.y, 5, 4, 91);
        let d = p.length() + (n - 0.5) * 0.7;
        let mut heat = (1.0 - d / 0.92).clamp(0.0, 1.0).powf(1.15);
        heat *= 0.7 + 0.6 * fbm(uv.x, uv.y, 10, 3, 95);
        let red = Vec3::new(0.85, 0.22, 0.04);
        let orange = Vec3::new(1.0, 0.55, 0.12);
        let white = Vec3::new(1.0, 0.93, 0.72);
        let c = red.lerp(orange, smooth(0.12, 0.45, heat)).lerp(white, smooth(0.55, 0.95, heat));
        [c.x, c.y, c.z, smooth(0.02, 0.32, heat)]
    })
}

/// A tight glowing dot.
fn spark_px() -> Vec<[f32; 4]> {
    paint(64, 64, |p, _| {
        let d2 = p.length_squared();
        let core = (-d2 * 30.0).exp();
        let glow = (-d2 * 5.0).exp() * 0.45;
        let c = Vec3::new(1.0, 0.62, 0.25).lerp(Vec3::new(1.0, 0.96, 0.85), core);
        [c.x, c.y, c.z, (core + glow).min(1.0)]
    })
}

/// A star-shaped muzzle blast.
fn muzzle_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let d = p.length();
        let a = p.y.atan2(p.x);
        let n = fbm(uv.x, uv.y, 6, 2, 13);
        let rays = (a * 7.0).cos().abs().powf(6.0) * (1.0 - d).max(0.0) * (0.6 + 0.8 * n);
        let core = (-d * d * 9.0).exp();
        let v = (core + rays * 0.8).min(1.0);
        let c = Vec3::new(1.0, 0.6, 0.2).lerp(Vec3::new(1.0, 0.95, 0.8), core);
        [c.x, c.y, c.z, v]
    })
}

/// A thin ring for shockwaves.
fn ring_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let d = p.length();
        let n = fbm(uv.x, uv.y, 6, 3, 31);
        let ring = (-((d - 0.8) / 0.09).powi(2)).exp() * (0.5 + 0.7 * n);
        let fill = (-((d - 0.45) / 0.35).powi(2)).exp() * 0.12;
        [1.0, 0.92, 0.8, (ring + fill).min(1.0)]
    })
}

/// A splash of blood with droplets thrown around it.
fn splat_px() -> Vec<[f32; 4]> {
    let mut drops = Vec::new();
    let mut s = 0x1234_5678u32;
    let mut r = || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / 16_777_216.0
    };
    for _ in 0..16 {
        let a = r() * TAU;
        let dist = 0.45 + r() * 0.45;
        drops.push((Vec2::from_angle(a) * dist, 0.025 + r() * 0.06));
    }
    paint(128, 128, |p, uv| {
        let n = fbm(uv.x, uv.y, 5, 4, 51);
        let main = p.length() + (n - 0.5) * 0.75;
        let mut cover = 1.0 - smooth(0.3, 0.4, main);
        for (c, rad) in &drops {
            cover = cover.max(1.0 - smooth(rad * 0.6, *rad, p.distance(*c)));
        }
        let k = 0.65 + 0.5 * fbm(uv.x, uv.y, 12, 2, 57);
        let c = Vec3::new(0.34, 0.035, 0.025) * k * (0.8 + 0.3 * p.length());
        [c.x, c.y, c.z, cover * 0.92]
    })
}

/// A burnt patch: soot in the middle, grey ash toward the edge.
fn scorch_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let n = fbm(uv.x, uv.y, 4, 4, 61);
        let d = p.length() + (n - 0.5) * 0.55;
        let a = (1.0 - smooth(0.25, 0.95, d)) * 0.92;
        let ash = Vec3::new(0.24, 0.22, 0.2);
        let soot = Vec3::new(0.035, 0.03, 0.028);
        let k = 0.8 + 0.4 * fbm(uv.x, uv.y, 16, 2, 67);
        let c = ash.lerp(soot, 1.0 - smooth(0.35, 0.8, d)) * k;
        [c.x, c.y, c.z, a]
    })
}

/// A wheel rut: U runs across, V along (tiles), with faint tread marks.
fn rut_px() -> Vec<[f32; 4]> {
    paint(64, 128, |p, uv| {
        let across = p.x.abs();
        let n = fbm(uv.x, uv.y, 4, 3, 41);
        let a = (1.0 - smooth(0.45, 1.0, across + (n - 0.5) * 0.3)) * 0.5;
        let tread = 0.82 + 0.18 * (uv.y * TAU * 4.0).sin();
        let c = Vec3::new(0.2, 0.15, 0.1) * (0.8 + 0.35 * n);
        [c.x, c.y, c.z, a * tread]
    })
}

/// Trampled mud left where a farm was flattened.
fn patch_px() -> Vec<[f32; 4]> {
    paint(128, 128, |p, uv| {
        let n = fbm(uv.x, uv.y, 4, 4, 81);
        let q = p.abs() - Vec2::splat(0.55);
        let d = q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0);
        let a = 1.0 - smooth(0.05, 0.4, d + (n - 0.5) * 0.35);
        let fine = fbm(uv.x, uv.y, 24, 2, 85);
        let mud = Vec3::new(0.32, 0.24, 0.16) * (0.75 + 0.45 * n);
        let straw = Vec3::new(0.62, 0.52, 0.3);
        let c = mud.lerp(straw, smooth(0.62, 0.72, fine) * 0.7);
        [c.x, c.y, c.z, a * 0.95]
    })
}

pub fn build_assets(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) -> VfxAssets {
    let puff = texture(images, puff_px(), 128, 128, false);
    let fire = texture(images, fire_px(), 128, 128, false);
    let spark = texture(images, spark_px(), 64, 64, false);
    let muzzle = texture(images, muzzle_px(), 128, 128, false);
    let ring = texture(images, ring_px(), 128, 128, false);
    let splat = texture(images, splat_px(), 128, 128, false);
    let scorch = texture(images, scorch_px(), 128, 128, false);
    let rut = texture(images, rut_px(), 64, 128, true);
    let patch = texture(images, patch_px(), 128, 128, false);

    // Soft sprites: unlit, see-through, several copies at falling opacity
    // that are swapped in as each puff ages.
    let mut faded = |tex: &Handle<Image>, c: Color, alphas: &[f32], unlit: bool, rough: f32| -> Vec<Handle<StandardMaterial>> {
        alphas
            .iter()
            .map(|a| {
                materials.add(StandardMaterial {
                    base_color: c.with_alpha(*a),
                    base_color_texture: Some(tex.clone()),
                    alpha_mode: AlphaMode::Blend,
                    unlit,
                    perceptual_roughness: rough,
                    reflectance: 0.2,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                })
            })
            .collect()
    };
    let smoke_a = [0.62, 0.56, 0.48, 0.4, 0.31, 0.22, 0.13, 0.05];
    let smoke = faded(&puff, Color::srgb(0.80, 0.79, 0.77), &smoke_a, true, 1.0);
    let smoke_dark = faded(&puff, Color::srgb(0.17, 0.155, 0.145), &[0.82, 0.74, 0.64, 0.53, 0.41, 0.3, 0.18, 0.07], true, 1.0);
    let dust = faded(&puff, Color::srgb(0.60, 0.51, 0.39), &[0.55, 0.48, 0.4, 0.32, 0.24, 0.16, 0.09, 0.03], true, 1.0);
    let decal_fade = [1.0, 0.8, 0.6, 0.42, 0.25, 0.1];
    let blood = faded(&splat, Color::WHITE, &decal_fade, false, 0.35);
    let scorch = faded(&scorch, Color::WHITE, &decal_fade, false, 0.95);
    let rut = faded(&rut, Color::WHITE, &decal_fade, false, 0.95);
    let patch = faded(&patch, Color::WHITE, &decal_fade, false, 0.95);

    // Glowing sprites add light on top of the scene (bright enough to bloom).
    let mut glow = |tex: &Handle<Image>, c: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: Color::LinearRgba(c),
            base_color_texture: Some(tex.clone()),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        })
    };
    let fire_steps: Vec<Handle<StandardMaterial>> = (0..8)
        .map(|i| {
            let t = i as f32 / 7.0;
            let hot = Vec3::new(1.0, 0.8, 0.55).lerp(Vec3::new(1.0, 0.32, 0.08), t);
            let k = 7.0 * (1.0 - t).powf(1.6) + 0.25;
            glow(&fire, LinearRgba::rgb(hot.x * k, hot.y * k, hot.z * k))
        })
        .collect();
    let spark_m = glow(&spark, LinearRgba::rgb(9.0, 5.0, 1.6));
    let ember_m = glow(&spark, LinearRgba::rgb(6.0, 1.8, 0.35));
    let muzzle_m = glow(&muzzle, LinearRgba::rgb(12.0, 8.0, 3.5));
    let shock: Vec<Handle<StandardMaterial>> =
        [1.4, 1.1, 0.85, 0.6, 0.4, 0.22].iter().map(|k| glow(&ring, LinearRgba::rgb(1.0 * k, 0.78 * k, 0.5 * k))).collect();

    VfxAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        decal: meshes.add(Plane3d::default().mesh().size(1.0, 1.0)),
        smoke,
        smoke_dark,
        dust,
        fire: fire_steps,
        spark: spark_m,
        ember: ember_m,
        muzzle: muzzle_m,
        shock,
        blood,
        scorch,
        rut,
        patch,
        chip: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.53, 0.5), perceptual_roughness: 0.9, ..default() }),
    }
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FxKind {
    Fireball,
    Smoke,
    DarkSmoke,
    /// Pale dust kicked up from the ground.
    Dust,
    /// A small, short puff (trails behind shots).
    Wisp,
    Spark,
    /// A slow glowing cinder that drifts up.
    Ember,
    /// Muzzle blast (lasts a few frames).
    Muzzle,
    /// A brief point light. `size` is the blast radius.
    Flash,
    /// A ring of light racing out along the ground.
    Shockwave,
    /// Wood splinters.
    Debris,
    /// Stone chips.
    Chip,
    Scorch,
    Blood,
    Track,
    Dirt,
    Telegraph { dur: f32 },
    /// A faint red disc under a telegraph ring.
    Zone { dur: f32 },
    /// A red strip on the ground (charges, broadsides). Sized by its spawner.
    Lane { dur: f32 },
}

impl FxKind {
    /// Purely decorative: skipped when there are already lots of effects.
    fn cosmetic(self) -> bool {
        matches!(
            self,
            FxKind::Smoke
                | FxKind::DarkSmoke
                | FxKind::Dust
                | FxKind::Wisp
                | FxKind::Spark
                | FxKind::Ember
                | FxKind::Muzzle
                | FxKind::Shockwave
                | FxKind::Debris
                | FxKind::Chip
        )
    }
}

#[derive(Component)]
pub struct Fx {
    kind: FxKind,
    life: f32,
    max: f32,
    vel: Vec3,
    size: f32,
    spin: f32,
    /// Light strength for flashes; full size for decals.
    power: f32,
    base: Vec3,
}

/// An effect that was skipped: removed again on the next update.
fn skipped(commands: &mut Commands) -> Entity {
    commands
        .spawn((GameEntity, Fx { kind: FxKind::Smoke, life: 0.0, max: 1.0, vel: Vec3::ZERO, size: 0.0, spin: 0.0, power: 0.0, base: Vec3::ONE }, Transform::default()))
        .id()
}

fn sprite(pos: Vec3, size: f32, spin: f32) -> Transform {
    Transform::from_translation(pos).with_rotation(facing() * Quat::from_rotation_z(spin)).with_scale(Vec3::splat(size))
}

fn flat(pos: Vec3, scale: Vec3, angle: f32) -> Transform {
    Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(angle)).with_scale(scale)
}

pub fn spawn_fx(commands: &mut Commands, assets: &GameAssets, kind: FxKind, pos: Vec3, size: f32, vel: Vec3) -> Entity {
    // On Low, also leave out some of the purely cosmetic puffs and sparks.
    if kind.cosmetic() && (LIVE.load(Ordering::Relaxed) > max_fx() || (detail() == 0 && rnd() < 0.4)) {
        return skipped(commands);
    }
    let v = &assets.vfx;
    let r = rnd();
    // Only a little spin on puffs, so their baked lighting stays believable.
    let tilt = (r - 0.5) * 0.7;
    let mut fx = Fx { kind, life: 1.0, max: 1.0, vel, size, spin: tilt, power: 0.0, base: Vec3::ONE };
    let (mesh, mat, tf) = match kind {
        FxKind::Fireball => {
            fx.life = 0.42 + 0.18 * r;
            fx.spin = r * TAU;
            (&v.quad, &v.fire[0], sprite(pos, size * 0.25, fx.spin))
        }
        FxKind::Smoke => {
            fx.life = 1.5 + 0.9 * r;
            (&v.quad, &v.smoke[0], sprite(pos, size * 0.5, tilt))
        }
        FxKind::DarkSmoke => {
            fx.life = 2.0 + 0.9 * r;
            (&v.quad, &v.smoke_dark[0], sprite(pos, size * 0.5, tilt))
        }
        FxKind::Dust => {
            fx.life = 0.9 + 0.6 * r;
            (&v.quad, &v.dust[0], sprite(pos, size * 0.45, tilt))
        }
        FxKind::Wisp => {
            fx.life = 0.55 + 0.3 * r;
            (&v.quad, &v.smoke[2], sprite(pos, size * 0.4, tilt))
        }
        FxKind::Spark => {
            fx.life = 0.3 + 0.3 * r;
            (&v.quad, &v.spark, sprite(pos, size * 0.45, 0.0))
        }
        FxKind::Ember => {
            fx.life = 0.9 + 0.9 * r;
            (&v.quad, &v.ember, sprite(pos, size * 0.3, 0.0))
        }
        FxKind::Muzzle => {
            fx.life = 0.1;
            fx.spin = r * TAU;
            (&v.quad, &v.muzzle, sprite(pos, size, fx.spin))
        }
        FxKind::Flash => {
            if LIGHTS.load(Ordering::Relaxed) >= max_lights() {
                return skipped(commands);
            }
            LIGHTS.fetch_add(1, Ordering::Relaxed);
            fx.life = 0.3;
            fx.max = 0.3;
            fx.power = 80_000.0 * size * size;
            return commands
                .spawn((
                    GameEntity,
                    fx,
                    PointLight {
                        color: Color::srgb(1.0, 0.64, 0.32),
                        intensity: 80_000.0 * size * size,
                        range: size * 6.0 + 6.0,
                        radius: 0.3,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(pos),
                ))
                .id();
        }
        FxKind::Shockwave => {
            fx.life = 0.38;
            (&v.decal, &v.shock[0], flat(pos.with_y(pos.y.min(0.1)), Vec3::new(size * 0.3, 1.0, size * 0.3), r * TAU))
        }
        FxKind::Debris | FxKind::Chip => {
            fx.life = 1.3 + 0.4 * r;
            let mat = if kind == FxKind::Debris { &assets.m.debris } else { &v.chip };
            let rot = Quat::from_euler(EulerRot::XYZ, r * 6.0, r * 11.0, r * 3.0);
            let shape = if kind == FxKind::Debris { Vec3::new(size * 1.6, size * 0.35, size * 0.5) } else { Vec3::splat(size) };
            fx.base = shape;
            (&assets.cube, mat, Transform::from_translation(pos).with_rotation(rot).with_scale(shape))
        }
        FxKind::Scorch => {
            fx.life = 9.0;
            (&v.decal, &v.scorch[0], flat(pos, Vec3::new(size * 1.3, 1.0, size * 1.3), r * TAU))
        }
        FxKind::Blood => {
            fx.life = 7.0;
            (&v.decal, &v.blood[0], flat(pos, Vec3::new(size * 1.6, 1.0, size * 1.6), r * TAU))
        }
        FxKind::Track => {
            fx.life = 7.0;
            (&assets.rut, &v.rut[0], Transform::from_translation(pos).with_scale(Vec3::new(0.82, 1.0, size)))
        }
        FxKind::Dirt => {
            fx.life = 60.0;
            (&v.decal, &v.patch[0], Transform::from_translation(pos).with_scale(Vec3::new(size * 1.15, 1.0, size * 0.95)))
        }
        FxKind::Telegraph { dur } => {
            fx.life = dur;
            (&assets.ring, &assets.m.telegraph, Transform::from_translation(pos).with_scale(Vec3::splat(size * 0.2)))
        }
        FxKind::Zone { dur } => {
            fx.life = dur;
            (&assets.disc, &assets.m.warn, Transform::from_translation(pos).with_scale(Vec3::new(size * 2.0, 0.03, size * 2.0)))
        }
        FxKind::Lane { dur } => {
            fx.life = dur;
            (&assets.cube, &assets.m.warn, Transform::from_translation(pos))
        }
    };
    if matches!(kind, FxKind::Scorch | FxKind::Blood | FxKind::Track | FxKind::Dirt | FxKind::Shockwave) {
        fx.base = tf.scale;
    }
    fx.max = fx.life;
    commands.spawn((GameEntity, fx, Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), tf, NotShadowCaster)).id()
}

/// Same as `spawn_fx` but facing a direction (used for tracks).
pub fn spawn_fx_rot(commands: &mut Commands, assets: &GameAssets, kind: FxKind, pos: Vec3, size: f32, angle: f32) -> Entity {
    let e = spawn_fx(commands, assets, kind, pos, size, Vec3::ZERO);
    commands.entity(e).entry::<Transform>().and_modify(move |mut t| t.rotation = Quat::from_rotation_y(angle));
    e
}

/// Fireball, flash of light, shockwave, smoke, sparks, debris and a scorch mark.
pub fn explosion(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, pos: Vec2, radius: f32) {
    let r = radius;
    spawn_fx(commands, assets, FxKind::Flash, w3(pos, 1.2 + r * 0.4), r, Vec3::ZERO);
    for i in 0..3 {
        let off = Vec3::new(rng.0.random_range(-0.3..0.3), 0.0, rng.0.random_range(-0.3..0.3)) * r;
        let h = 0.5 + i as f32 * 0.3 * r.min(3.0);
        spawn_fx(commands, assets, FxKind::Fireball, w3(pos, h) + off, r * (1.7 - i as f32 * 0.3), Vec3::ZERO);
    }
    spawn_fx(commands, assets, FxKind::Shockwave, w3(pos, 0.07), r * 2.6, Vec3::ZERO);
    for i in 0..5 {
        let off = Vec3::new(rng.0.random_range(-1.0..1.0), 0.0, rng.0.random_range(-1.0..1.0)) * r * 0.45;
        let kind = if i % 2 == 0 { FxKind::DarkSmoke } else { FxKind::Smoke };
        spawn_fx(commands, assets, kind, w3(pos, 0.7 + i as f32 * 0.2) + off, r * 0.85, Vec3::Y * (1.8 + i as f32 * 0.4) + off * 0.9);
    }
    for _ in 0..4 {
        let v = Vec3::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(1.0..2.0), rng.0.random_range(-1.0..1.0)) * 7.0;
        spawn_fx(commands, assets, FxKind::Debris, w3(pos, 0.6), 0.3, v);
    }
    for _ in 0..7 {
        let v = Vec3::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(0.6..1.6), rng.0.random_range(-1.0..1.0)) * rng.0.random_range(6.0..13.0);
        spawn_fx(commands, assets, FxKind::Spark, w3(pos, 0.8), 0.5, v);
    }
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU + rng.0.random_range(0.0..0.8);
        let dir = Vec2::from_angle(a);
        spawn_fx(commands, assets, FxKind::Dust, w3(pos + dir * r * 0.4, 0.35), r * 0.75, w3(dir * r * 2.2, 0.25));
    }
    spawn_fx(commands, assets, FxKind::Scorch, w3(pos, 0.03), r * 1.4, Vec3::ZERO);
}

/// Muzzle blast from a gun at `pos` firing along `dir`.
pub fn muzzle_blast(commands: &mut Commands, assets: &GameAssets, pos: Vec3, dir: Vec3, size: f32) {
    let dir = dir.normalize_or(Vec3::X);
    spawn_fx(commands, assets, FxKind::Muzzle, pos + dir * 0.35 * size, size * 1.5, Vec3::ZERO);
    spawn_fx(commands, assets, FxKind::Flash, pos + dir * 0.5, size * 0.55, Vec3::ZERO);
    for i in 0..3 {
        let k = i as f32;
        spawn_fx(commands, assets, FxKind::Smoke, pos + dir * (0.3 + k * 0.45) * size, size * (0.7 + k * 0.2), dir * (5.5 - k * 1.5) + Vec3::Y * 0.8);
    }
    for _ in 0..3 {
        let spread = Vec3::new(rnd() - 0.5, rnd() * 0.5, rnd() - 0.5);
        spawn_fx(commands, assets, FxKind::Spark, pos + dir * 0.3, 0.35 * size, (dir + spread) * 9.0);
    }
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// Swaps in the material for how far along `f` (0..1) the effect is.
fn step(mat: &mut Option<Mut<MeshMaterial3d<StandardMaterial>>>, list: &[Handle<StandardMaterial>], f: f32) {
    if let Some(m) = mat {
        let i = ((f * list.len() as f32) as usize).min(list.len() - 1);
        if m.0 != list[i] {
            m.0 = list[i].clone();
        }
    }
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn update_fx(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q: Query<(Entity, &mut Transform, &mut Fx, Option<&mut MeshMaterial3d<StandardMaterial>>, Option<&mut PointLight>)>,
) {
    let dt = time.delta_secs();
    let v = &assets.vfx;
    let face = facing();
    let (mut live, mut lights) = (0usize, 0usize);
    for (e, mut tf, mut fx, mut mat, light) in &mut q {
        fx.life -= dt;
        if fx.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        live += 1;
        let age = 1.0 - fx.life / fx.max;
        let s = fx.size;
        match fx.kind {
            FxKind::Fireball => {
                tf.scale = Vec3::splat(s * 0.45 * (0.55 + 0.8 * ease_out(age * 1.6)));
                tf.translation.y += dt * (0.6 + s * 0.35);
                tf.rotation = face * Quat::from_rotation_z(fx.spin + age * 0.9);
                step(&mut mat, &v.fire, age);
            }
            FxKind::Smoke | FxKind::DarkSmoke | FxKind::Dust | FxKind::Wisp => {
                let vel = fx.vel;
                tf.translation += vel * dt;
                fx.vel *= (-1.7 * dt).exp();
                if fx.kind != FxKind::Dust {
                    fx.vel.y += 0.7 * dt;
                }
                let (grow, list) = match fx.kind {
                    FxKind::Smoke => (1.1, &v.smoke),
                    FxKind::DarkSmoke => (1.2, &v.smoke_dark),
                    FxKind::Dust => (1.3, &v.dust),
                    _ => (0.9, &v.smoke),
                };
                let base = if fx.kind == FxKind::Wisp { 0.4 } else { 0.5 };
                tf.scale = Vec3::splat(s * base * (0.55 + grow * ease_out(age * 1.3)));
                tf.rotation = face * Quat::from_rotation_z(fx.spin + age * 0.4 * fx.spin.signum());
                let f = if fx.kind == FxKind::Wisp { 0.25 + age * 0.75 } else { age };
                step(&mut mat, list, f);
            }
            FxKind::Spark => {
                fx.vel.y -= 20.0 * dt;
                let vel = fx.vel;
                tf.translation += vel * dt;
                if tf.translation.y < 0.08 {
                    tf.translation.y = 0.08;
                    fx.vel = Vec3::new(vel.x * 0.4, vel.y.abs() * 0.25, vel.z * 0.4);
                }
                tf.scale = Vec3::splat(s * 0.45 * (1.0 - age).sqrt());
                tf.rotation = face;
            }
            FxKind::Ember => {
                fx.vel *= (-0.9 * dt).exp();
                fx.vel.y += 1.4 * dt;
                let vel = fx.vel;
                tf.translation += vel * dt;
                let flicker = 0.7 + 0.3 * (age * 45.0 + fx.spin * 9.0).sin();
                tf.scale = Vec3::splat(s * 0.3 * (1.0 - age) * flicker);
                tf.rotation = face;
            }
            FxKind::Muzzle => {
                tf.scale = Vec3::splat(s * (0.75 + age * 0.7) * (1.0 - age * 0.5));
            }
            FxKind::Flash => {
                lights += 1;
                if let Some(mut l) = light {
                    l.intensity = fx.power * (1.0 - age).powi(2);
                }
            }
            FxKind::Shockwave => {
                let k = 0.25 + 0.85 * ease_out(age);
                tf.scale = Vec3::new(fx.base.x * k / 0.3, 1.0, fx.base.z * k / 0.3);
                step(&mut mat, &v.shock, age);
            }
            FxKind::Debris | FxKind::Chip => {
                fx.vel.y -= 22.0 * dt;
                let vel = fx.vel;
                tf.translation += vel * dt;
                if tf.translation.y < 0.08 {
                    tf.translation.y = 0.08;
                    fx.vel = if vel.y < -3.0 { Vec3::new(vel.x * 0.4, -vel.y * 0.3, vel.z * 0.4) } else { Vec3::ZERO };
                } else {
                    tf.rotate_local_x(dt * 9.0);
                    tf.rotate_local_z(dt * 5.0);
                }
                let k = (1.0 - ((age - 0.75) / 0.25).max(0.0)).max(0.01);
                tf.scale = fx.base * k;
            }
            FxKind::Blood => {
                // Splashes out quickly, then soaks in and fades.
                let grow = ease_out((fx.max - fx.life) / 0.14);
                tf.scale = Vec3::new(fx.base.x * (0.3 + 0.7 * grow), 1.0, fx.base.z * (0.3 + 0.7 * grow));
                step(&mut mat, &v.blood, ((age - 0.6) / 0.4).max(0.0));
            }
            FxKind::Scorch => step(&mut mat, &v.scorch, ((age - 0.6) / 0.4).max(0.0)),
            FxKind::Track => step(&mut mat, &v.rut, ((age - 0.55) / 0.45).max(0.0)),
            FxKind::Dirt => step(&mut mat, &v.patch, ((age - 0.8) / 0.2).max(0.0)),
            FxKind::Telegraph { .. } => {
                let k = 0.2 + age * 0.8;
                let pulse = 1.0 + 0.04 * (age * 40.0).sin();
                tf.scale = Vec3::new(s * k * pulse, 0.6, s * k * pulse);
            }
            FxKind::Zone { .. } | FxKind::Lane { .. } => {}
        }
    }
    LIVE.store(live, Ordering::Relaxed);
    LIGHTS.store(lights, Ordering::Relaxed);
}
