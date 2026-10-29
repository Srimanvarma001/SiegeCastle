//! The player's castle: car-like driving, nitro, health, looks and growth.

use crate::anim::spawn_pieces;
use crate::art::{castle_mounts, p2, w3, yaw, Variant};
use crate::assets::GameAssets;
use crate::models::Role;
use crate::combat::{damage_area, explosion, spawn_fx, spawn_fx_rot, FxKind};
use crate::enemy::Enemy;
use crate::grid::Grid;
use crate::items::{compute_stats, stage_info, Loadout, ModuleKind, Mount, Stats};
use crate::{Banner, GameEntity, GameRng, GameState, MAP_HALF, Shake, Sim};
use bevy::prelude::*;
use std::collections::VecDeque;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Loadout::starting())
            .insert_resource(compute_stats(&Loadout::starting()))
            .init_resource::<DriveInput>()
            .init_resource::<Tracks>()
            .add_systems(OnEnter(GameState::Starting), spawn_castle)
            .add_systems(Update, (read_input, drive).chain().in_set(Sim::Input))
            .add_systems(Update, (apply_loadout, rebuild_castle).chain())
            .add_systems(Update, (stage_shockwave, castle_health).chain().in_set(Sim::Resolve));
    }
}

#[derive(Component)]
pub struct Castle {
    pub hp: f32,
    pub nitro: f32,
    /// Facing angle on the ground plane.
    pub heading: f32,
    /// Forward speed.
    pub speed: f32,
    pub vel: Vec2,
    pub radius: f32,
    /// Seconds of Ram charge left.
    pub charge: f32,
    pub boosting: bool,
    /// Push from boss hits.
    pub knock: Vec2,
    /// Seconds of red "got hit" flash on the health bar.
    pub hurt_flash: f32,
    shockwave: bool,
    track_acc: f32,
    smoke_acc: f32,
}

impl Castle {
    pub fn hurt(&mut self, dmg: f32) {
        self.hp -= dmg;
        self.hurt_flash = 0.25;
    }
}

/// Model pieces on the castle. Rebuilt when it grows or gets a module.
#[derive(Component, Clone)]
pub struct CastlePart;

/// A mounted weapon model; its world position is where shots come from.
#[derive(Component)]
pub struct ModuleMount(pub ModuleKind);

/// Every mounted weapon model, including the mirrored side cannons
/// (used for animation; `ModuleMount` is only on the one that shoots).
#[derive(Component)]
pub struct MountOf(pub ModuleKind);

/// Holds everything above the wheels, so the body can lean and bounce.
#[derive(Component)]
pub struct CastleBody;

/// Steering for this frame, from the keyboard or the test autopilot.
#[derive(Resource, Default)]
pub struct DriveInput {
    pub dir: Vec2,
    pub nitro: bool,
}

/// Recent dirt tracks, oldest first, so the count stays bounded.
#[derive(Resource, Default)]
pub struct Tracks(VecDeque<Entity>);

const MAX_TRACKS: usize = 700;

fn spawn_castle(
    mut commands: Commands,
    mut loadout: ResMut<Loadout>,
    mut stats: ResMut<Stats>,
    mut tracks: ResMut<Tracks>,
) {
    *loadout = Loadout::starting();
    *stats = compute_stats(&loadout);
    tracks.0.clear();
    commands.spawn((
        GameEntity,
        Castle {
            hp: stats.max_hp,
            nitro: stats.nitro_max,
            heading: std::f32::consts::FRAC_PI_2,
            speed: 0.0,
            vel: Vec2::ZERO,
            radius: stats.radius,
            charge: 0.0,
            boosting: false,
            knock: Vec2::ZERO,
            hurt_flash: 0.0,
            shockwave: false,
            track_acc: 0.0,
            smoke_acc: 0.0,
        },
        Transform::from_rotation(yaw(std::f32::consts::FRAC_PI_2)),
        Visibility::Visible,
    ));
}

pub fn read_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<DriveInput>) {
    let mut d = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        d.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        d.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        d.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        d.x += 1.0;
    }
    input.dir = d.normalize_or_zero();
    input.nitro = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Car-like driving: the castle turns toward where you steer and rolls forward.
pub fn drive(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<DriveInput>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    assets: Res<GameAssets>,
    mut tracks: ResMut<Tracks>,
    mut castle: Query<(&mut Transform, &mut Castle)>,
) {
    let Ok((mut tf, mut c)) = castle.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let mut throttle = 0.0;
    if input.dir.length_squared() > 0.01 {
        let diff = wrap(input.dir.to_angle() - c.heading);
        let turn = stats.turn * dt * if c.charge > 0.0 { 0.4 } else { 1.0 };
        c.heading = wrap(c.heading + diff.clamp(-turn, turn));
        throttle = 0.35 + 0.65 * diff.cos().max(0.0);
    }

    c.boosting = input.nitro && c.nitro > 1.0 && throttle > 0.0;
    if c.boosting {
        c.nitro -= 38.0 * dt;
    } else {
        c.nitro = (c.nitro + stats.nitro_regen * dt).min(stats.nitro_max);
    }
    c.charge = (c.charge - dt).max(0.0);
    let boost = if c.boosting { 1.7 } else { 1.0 } * if c.charge > 0.0 { 1.8 } else { 1.0 };
    let target = stats.speed * boost * throttle;
    let rate = if target > c.speed { 16.0 } else { 22.0 };
    c.speed += (target - c.speed).clamp(-rate * dt, rate * dt);
    c.vel = Vec2::from_angle(c.heading) * c.speed;

    let pos = p2(tf.translation);
    let lim = MAP_HALF - stats.radius - 6.0;
    let knock = c.knock;
    let next = (pos + (c.vel + knock) * dt).clamp(Vec2::splat(-lim), Vec2::splat(lim));
    c.knock *= (-4.0 * dt).exp();
    c.radius = stats.radius;
    tf.translation = w3(next, 0.0);
    // A little body roll while turning hard.
    tf.rotation = yaw(c.heading);

    // Dirt tracks behind the rear wheels.
    let fwd = Vec2::from_angle(c.heading);
    let side = Vec2::new(-fwd.y, fwd.x);
    let (len, wid) = match loadout.stage {
        0 => (6.0, 4.0),
        1 => (9.0, 6.0),
        _ => (13.0, 9.0),
    };
    c.track_acc += c.speed.abs() * dt;
    while c.track_acc > 0.8 {
        c.track_acc -= 0.8;
        let back = c.track_acc;
        for s in [-1.0, 1.0] {
            let p = next - fwd * (len * 0.32 + back) + side * s * (wid / 2.0 - 0.2);
            let e = spawn_fx_rot(&mut commands, &assets, FxKind::Track, w3(p, 0.025), 0.55 + loadout.stage as f32 * 0.15, c.heading);
            tracks.0.push_back(e);
            if tracks.0.len() > MAX_TRACKS
                && let Some(old) = tracks.0.pop_front()
            {
                commands.entity(old).try_despawn();
            }
        }
    }

    // Chimney smoke, and exhaust while boosting.
    let mounts = castle_mounts(loadout.stage);
    c.smoke_acc += dt;
    let every = if c.boosting { 0.05 } else { 0.2 };
    while c.smoke_acc > every {
        c.smoke_acc -= every;
        let chimney = tf.transform_point(mounts.chimney);
        let drift = w3(-fwd * c.speed * 0.25, 2.5);
        spawn_fx(&mut commands, &assets, FxKind::Smoke, chimney, 1.2 + loadout.stage as f32 * 0.4, drift);
        if c.boosting {
            let back = tf.transform_point(mounts.back + Vec3::new(-0.5, 0.6, 0.0));
            spawn_fx(&mut commands, &assets, FxKind::Spark, back, 0.9, Vec3::ZERO);
            spawn_fx(&mut commands, &assets, FxKind::Smoke, back, 1.0, w3(-fwd * 4.0, 1.0));
        }
    }
}

/// Recomputes stats when the loadout changes and heals on growth.
fn apply_loadout(loadout: Res<Loadout>, mut stats: ResMut<Stats>, mut castle: Query<&mut Castle>, mut last_stage: Local<usize>) {
    if !loadout.is_changed() {
        return;
    }
    let new = compute_stats(&loadout);
    if let Ok(mut c) = castle.single_mut() {
        if new.max_hp > stats.max_hp {
            c.hp += new.max_hp - stats.max_hp;
        }
        if loadout.stage > *last_stage {
            c.hp = new.max_hp;
            c.nitro = new.nitro_max;
            c.shockwave = true;
        }
        c.hp = c.hp.min(new.max_hp);
        c.radius = new.radius;
    }
    *last_stage = loadout.stage;
    *stats = new;
}

fn rebuild_castle(
    mut commands: Commands,
    loadout: Res<Loadout>,
    assets: Res<GameAssets>,
    castle: Query<Entity, With<Castle>>,
    added: Query<(), Added<Castle>>,
) {
    let Ok(e) = castle.single() else {
        return;
    };
    if !loadout.is_changed() && added.is_empty() {
        return;
    }
    let variant = if loadout.stage == 0 { Variant::Base } else { loadout.variant };
    let mounts = castle_mounts(loadout.stage);
    let scale = 1.0 + loadout.stage as f32 * 0.25;
    let rig = assets.castle_rigs[&(loadout.stage, variant)].clone();
    let modules = assets.module_rigs.clone();
    let owned = loadout.modules.clone();
    let is_wheel = |r: Role| matches!(r, Role::Wheel { .. });
    commands.entity(e).despawn_related::<Children>().with_children(|p| {
        // Wheels stay on the ground; the body above them sways.
        spawn_pieces(p, &rig.parts, e, is_wheel);
        p.spawn((CastlePart, CastleBody, Transform::IDENTITY, Visibility::Inherited)).with_children(|b| {
            rig.body.spawn_parts(b, CastlePart);
            spawn_pieces(b, &rig.parts, e, |r| !is_wheel(r));
            let mut tops = mounts.top.iter();
            for m in &owned {
                let at = match m.kind.mount() {
                    Mount::Top => *tops.next().unwrap_or(&mounts.top[0]),
                    Mount::Front => mounts.front,
                    Mount::Back => mounts.back,
                    Mount::Side => mounts.side,
                };
                let model = modules[&m.kind].clone();
                let place = Transform::from_translation(at).with_scale(Vec3::splat(scale));
                b.spawn((CastlePart, ModuleMount(m.kind), MountOf(m.kind), place, Visibility::Inherited)).with_children(|q| {
                    model.body.spawn_parts(q, ());
                    let owner = q.target_entity();
                    spawn_pieces(q, &model.parts, owner, |_| true);
                });
                if m.kind.mount() == Mount::Side {
                    // Guns on the other side too, facing out.
                    let mirror = Vec3::new(at.x, at.y, -at.z);
                    let place = Transform::from_translation(mirror)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                        .with_scale(Vec3::splat(scale));
                    b.spawn((CastlePart, MountOf(m.kind), place, Visibility::Inherited)).with_children(|q| {
                        model.body.spawn_parts(q, ());
                        let owner = q.target_entity();
                        spawn_pieces(q, &model.parts, owner, |_| true);
                    });
                }
            }
        });
    });
}

/// Growing to a new stage blasts nearby enemies away.
fn stage_shockwave(
    mut commands: Commands,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    loadout: Res<Loadout>,
    mut rng: ResMut<GameRng>,
    mut banner: ResMut<Banner>,
    mut shake: ResMut<Shake>,
    mut castle: Query<(&Transform, &mut Castle)>,
    mut enemies: Query<&mut Enemy>,
) {
    let Ok((tf, mut c)) = castle.single_mut() else {
        return;
    };
    if !c.shockwave {
        return;
    }
    c.shockwave = false;
    let pos = p2(tf.translation);
    let r = c.radius + 14.0;
    damage_area(&grid, &mut enemies, pos, r, 120.0, 30.0);
    for i in 0..8 {
        let off = Vec2::from_angle(i as f32 / 8.0 * std::f32::consts::TAU) * (c.radius + 4.0);
        explosion(&mut commands, &assets, &mut rng, pos + off, 3.0);
    }
    shake.0 += 2.5;
    let info = stage_info(loadout.stage, loadout.variant);
    banner.show(format!("Your castle grew: {}!", info.name), "A new module slot is open", 3.5);
}

fn castle_health(
    time: Res<Time>,
    stats: Res<Stats>,
    mut castle: Query<&mut Castle>,
    mut enemies: Query<&mut Enemy>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Ok(mut c) = castle.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let mut dps = 0.0;
    for mut en in &mut enemies {
        if en.touching {
            dps += en.dps * en.dmg_mult;
            if stats.spikes_dps > 0.0 {
                en.hit(stats.spikes_dps * dt);
            }
        }
    }
    c.hp -= dps * dt;
    if dps > 0.0 {
        c.hurt_flash = c.hurt_flash.max(0.1);
    }
    c.hp = (c.hp + stats.regen * stats.max_hp * dt).min(stats.max_hp);
    c.hurt_flash = (c.hurt_flash - dt).max(0.0);
    if c.hp <= 0.0 {
        next.set(GameState::GameOver);
    }
}
