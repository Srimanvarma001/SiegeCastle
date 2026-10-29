//! Movement and body language of the castle.
//!
//! Wheels roll, flags wave, the castle leans into turns, squats when it
//! speeds up and rocks when hit, and shots leave trails. Weapons are animated
//! in `weapons.rs`, troops in `enemy.rs` and bosses in `boss.rs`.
//!
//! Gameplay code never reads any of this: it only changes how things look.

use crate::art::{castle_mounts, p2, w3};
use crate::assets::{GameAssets, PartHandle};
use crate::combat::{ArcShot, Bullet};
use crate::items::Loadout;
use crate::models::Role;
use crate::player::{Castle, CastleBody};
use crate::vfx::{rnd, spawn_fx, FxKind};
use crate::{GameRng, GameState, Sim};
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{PI, TAU};

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastleSway>()
            .add_systems(OnEnter(GameState::Starting), reset)
            .add_systems(Update, ((animate_castle, animate_pieces).chain(), sink, trails).in_set(Sim::Resolve));
    }
}

fn reset(mut sway: ResMut<CastleSway>) {
    *sway = CastleSway::default();
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

// ---------------------------------------------------------------------------
// Moving pieces of the castle
// ---------------------------------------------------------------------------

/// A moving piece of the castle model (see `models::Role`).
#[derive(Component)]
pub struct Piece {
    role: Role,
    pivot: Vec3,
    rest: Quat,
    angle: f32,
}

/// Spawns the moving pieces for which `keep` is true under the entity being built.
pub fn spawn_pieces(p: &mut ChildSpawnerCommands, parts: &[PartHandle], keep: impl Fn(Role) -> bool) {
    for part in parts {
        if keep(part.role) {
            spawn_piece(p, part);
        }
    }
}

fn spawn_piece(p: &mut ChildSpawnerCommands, part: &PartHandle) {
    p.spawn((
        Piece { role: part.role, pivot: part.pivot, rest: part.rest, angle: 0.0 },
        Transform::from_translation(part.pivot).with_rotation(part.rest),
        Visibility::Inherited,
    ))
    .with_children(|q| {
        part.model.spawn_parts(q, ());
        for sub in &part.parts {
            spawn_piece(q, sub);
        }
    });
}

/// Wheels roll with the castle's speed; flags wave in the wind.
fn animate_pieces(time: Res<Time>, castle: Query<&Castle>, mut pieces: Query<(&mut Piece, &mut Transform), Without<Castle>>) {
    let Ok(c) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut pc, mut tf) in &mut pieces {
        match pc.role {
            Role::Wheel { r } => {
                pc.angle -= c.speed * dt / r.max(0.1);
                tf.rotation = pc.rest * Quat::from_rotation_z(pc.angle);
            }
            Role::Flag => {
                let ph = pc.pivot.x * 1.3 + pc.pivot.z * 0.7 + pc.pivot.y * 0.5;
                let gust = (t * 0.7 + ph).sin() * 0.5 + 0.5;
                let wave = (t * (3.0 + gust) + ph).sin() * (0.16 + 0.12 * gust);
                let flutter = (t * 7.3 + ph * 2.0).sin() * 0.08;
                tf.rotation = pc.rest * Quat::from_rotation_y(wave + 0.12) * Quat::from_rotation_x(flutter);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The castle: leans into turns, squats when it speeds up, rocks when hit.
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct CastleSway {
    prev_speed: f32,
    prev_heading: f32,
    /// Smoothed acceleration (also swings the ram).
    accel: f32,
    pitch: f32,
    pitch_v: f32,
    roll: f32,
    roll_v: f32,
    prev_hurt: f32,
    dust: f32,
}

fn animate_castle(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    loadout: Res<Loadout>,
    mut rng: ResMut<GameRng>,
    mut st: ResMut<CastleSway>,
    castle: Query<(&Transform, &Castle)>,
    mut body: Query<&mut Transform, (With<CastleBody>, Without<Castle>)>,
) {
    let (Ok((tf, c)), Ok(mut bt)) = (castle.single(), body.single_mut()) else {
        return;
    };
    let dt = time.delta_secs().max(1e-4);
    let accel = (c.speed - st.prev_speed) / dt;
    st.accel += (accel - st.accel) * (1.0 - (-8.0 * dt).exp());
    let turn = wrap(c.heading - st.prev_heading) / dt;
    st.prev_speed = c.speed;
    st.prev_heading = c.heading;

    let target_pitch = (st.accel * 0.0035).clamp(-0.06, 0.06) + if c.boosting { 0.02 } else { 0.0 };
    let target_roll = (turn * c.speed * 0.0045).clamp(-0.07, 0.07);
    if c.hurt_flash > st.prev_hurt + 0.1 {
        // A heavy hit: the whole castle rocks and stone chips fly off the wall.
        st.pitch_v += rng.0.random_range(-1.0..1.0) * 0.9;
        st.roll_v += rng.0.random_range(-1.0..1.0) * 1.2;
        let a = rng.0.random_range(0.0..TAU);
        let at = p2(tf.translation) + Vec2::from_angle(a) * c.radius * 0.9;
        for _ in 0..4 {
            let v = Vec3::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(1.0..2.0), rng.0.random_range(-1.0..1.0)) * 6.0;
            spawn_fx(&mut commands, &assets, FxKind::Chip, w3(at, 2.0), 0.22, v);
        }
        spawn_fx(&mut commands, &assets, FxKind::Dust, w3(at, 1.5), 2.5, Vec3::Y);
    }
    st.prev_hurt = c.hurt_flash;
    let (tp, tr) = (target_pitch, target_roll);
    st.pitch_v += ((tp - st.pitch) * 80.0 - st.pitch_v * 10.0) * dt;
    st.roll_v += ((tr - st.roll) * 80.0 - st.roll_v * 10.0) * dt;
    st.pitch += st.pitch_v * dt;
    st.roll += st.roll_v * dt;
    st.pitch = st.pitch.clamp(-0.15, 0.15);
    st.roll = st.roll.clamp(-0.15, 0.15);

    // Rumble over rough ground.
    let t = time.elapsed_secs();
    let k = (c.speed.abs() / 12.0).min(1.5);
    let mut rumble = ((t * 21.0).sin() * 0.6 + (t * 33.0).sin() * 0.4).abs() * 0.025 * k;
    if c.boosting {
        rumble += (t * 57.0).sin().abs() * 0.015;
    }
    bt.translation = Vec3::Y * rumble;
    bt.rotation = Quat::from_rotation_z(st.pitch) * Quat::from_rotation_x(st.roll);

    // Dust thrown up by the wheels.
    if c.speed > 5.0 {
        st.dust += dt * c.speed * 0.3;
        let mounts = castle_mounts(loadout.stage);
        while st.dust > 1.0 {
            st.dust -= 1.0;
            let side = if rnd() < 0.5 { -1.0 } else { 1.0 };
            let at = tf.transform_point(Vec3::new(-mounts.len * 0.36, 0.4, side * mounts.wid * 0.55));
            let back = tf.rotation * Vec3::NEG_X;
            spawn_fx(&mut commands, &assets, FxKind::Dust, at, 1.6 + loadout.stage as f32 * 0.6, back * 1.5 + Vec3::Y * 0.6);
        }
    } else {
        st.dust = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Shots: drop from the muzzle to hitting height, and leave trails.
// ---------------------------------------------------------------------------

/// Eases a straight shot's height toward this value after it leaves the barrel.
#[derive(Component)]
pub struct Sink(pub f32);

fn sink(time: Res<Time>, mut q: Query<(&mut Transform, &Sink)>) {
    let k = 1.0 - (-3.5 * time.delta_secs()).exp();
    for (mut tf, s) in &mut q {
        tf.translation.y += (s.0 - tf.translation.y) * k;
    }
}

fn trails(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    bullets: Query<(&Transform, &Mesh3d, &Bullet)>,
    arcs: Query<(&Transform, &Mesh3d), With<ArcShot>>,
) {
    let dt = time.delta_secs();
    for (tf, mesh, b) in &bullets {
        if mesh.0 == assets.ball && rnd() < dt / 0.03 {
            let back = w3(-b.vel.normalize_or_zero(), 0.0);
            let size = if b.hostile { 1.2 } else { 0.85 };
            spawn_fx(&mut commands, &assets, FxKind::Wisp, tf.translation + back * 0.3, size, back * 1.5 + Vec3::Y * 0.3);
        }
    }
    for (tf, mesh) in &arcs {
        if mesh.0 != assets.ball {
            continue;
        }
        if tf.scale.x > 1.2 {
            // Boulders shed grit.
            if rnd() < dt / 0.07 {
                spawn_fx(&mut commands, &assets, FxKind::Dust, tf.translation, 1.6, Vec3::Y * 0.2);
            }
        } else if rnd() < dt / 0.035 {
            // A lit fuse trailing smoke.
            spawn_fx(&mut commands, &assets, FxKind::Wisp, tf.translation, 0.8, Vec3::Y * 0.3);
            if rnd() < 0.35 {
                spawn_fx(&mut commands, &assets, FxKind::Ember, tf.translation, 0.7, Vec3::Y * 0.5);
            }
        }
    }
}
