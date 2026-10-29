//! Movement and body language of the castle and its weapons.
//!
//! Wheels roll, weapons swing round to aim and kick back when they fire, the
//! castle leans into turns and rocks when hit, and shots leave trails.
//! Troops are animated in `enemy.rs` and bosses in `boss.rs`.
//!
//! Gameplay code never reads any of this: it only changes how things look.

use crate::art::{castle_mounts, p2, w3};
use crate::assets::{GameAssets, PartHandle};
use crate::combat::{ArcShot, Bullet};
use crate::enemy::{Boss, Enemy};
use crate::grid::Grid;
use crate::items::{Loadout, ModuleKind, Stats};
use crate::models::Role;
use crate::modules::Cooldowns;
use crate::player::{Castle, CastleBody, MountOf};
use crate::vfx::{muzzle_blast, rnd, spawn_fx, FxKind};
use crate::{GameRng, GameState, Sim};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{PI, TAU};

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponFire>()
            .init_resource::<CastleSway>()
            .add_systems(OnEnter(GameState::Starting), reset)
            .add_systems(
                Update,
                ((track_weapon_fire, animate_castle, animate_pieces).chain(), sink, trails).in_set(Sim::Resolve),
            );
    }
}

fn reset(mut wf: ResMut<WeaponFire>, mut sway: ResMut<CastleSway>) {
    *wf = WeaponFire::default();
    *sway = CastleSway::default();
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

// ---------------------------------------------------------------------------
// Moving pieces
// ---------------------------------------------------------------------------

/// A moving piece of a model (see `models::Role`).
#[derive(Component)]
pub struct Piece {
    role: Role,
    pivot: Vec3,
    rest: Quat,
    /// The castle or weapon mount whose state drives this piece.
    owner: Entity,
    angle: f32,
    /// Spin or swing speed; for barrels, whether the last shot came from here.
    speed: f32,
}

/// Spawns the moving pieces for which `keep` is true under the entity being built.
pub fn spawn_pieces(p: &mut ChildSpawnerCommands, parts: &[PartHandle], owner: Entity, keep: impl Fn(Role) -> bool) {
    for part in parts {
        if keep(part.role) {
            spawn_piece(p, part, owner);
        }
    }
}

fn spawn_piece(p: &mut ChildSpawnerCommands, part: &PartHandle, owner: Entity) {
    p.spawn((
        Piece { role: part.role, pivot: part.pivot, rest: part.rest, owner, angle: 0.0, speed: 1.0 },
        Transform::from_translation(part.pivot).with_rotation(part.rest),
        Visibility::Inherited,
    ))
    .with_children(|q| {
        part.model.spawn_parts(q, ());
        for sub in &part.parts {
            spawn_piece(q, sub, owner);
        }
    });
}

#[derive(Clone, Copy, PartialEq, Default)]
enum Owner {
    #[default]
    None,
    Castle,
    Module(ModuleKind),
}

/// What an owner is doing this frame, as far as its pieces care.
#[derive(Clone, Copy)]
struct Drive {
    kind: Owner,
    /// Ground speed (for wheels).
    speed: f32,
    accel: f32,
    /// Seconds since the main weapon fired, and whether that was this frame.
    fired: f32,
    just: bool,
    /// For the side cannons: whether the last shot came out of this side.
    side_ok: bool,
    /// Yaw to aim at, relative to the owner.
    aim: Option<f32>,
    charging: bool,
}

impl Default for Drive {
    fn default() -> Self {
        Self {
            kind: Owner::None,
            speed: 0.0,
            accel: 0.0,
            fired: 99.0,
            just: false,
            side_ok: true,
            aim: None,
            charging: false,
        }
    }
}

/// When each weapon last fired, worked out from its cooldown timers.
#[derive(Resource, Default)]
struct WeaponFire {
    last: HashMap<ModuleKind, (f32, f32)>,
    /// Seconds since it fired, and whether that was its ability.
    since: HashMap<ModuleKind, (f32, bool)>,
}

fn track_weapon_fire(time: Res<Time>, cds: Res<Cooldowns>, mut wf: ResMut<WeaponFire>) {
    let dt = time.delta_secs();
    let WeaponFire { last, since } = &mut *wf;
    for (k, t) in &cds.0 {
        let s = since.entry(*k).or_insert((99.0, false));
        s.0 += dt;
        if let Some(&(auto, ability)) = last.get(k) {
            // A timer jumping back up means it just fired. (Retries without
            // a target only add a fraction of a second.)
            if t.ability > ability + 0.5 {
                *s = (0.0, true);
            } else if t.auto > auto + 0.3 {
                *s = (0.0, false);
            }
        }
        last.insert(*k, (t.auto, t.ability));
    }
}

/// 0 → 1 the instant a gun fires, then eases back into place.
fn recoil(s: f32) -> f32 {
    if s < 0.035 { s / 0.035 } else { (-(s - 0.035) * 7.0).exp() }
}

fn animate_pieces(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    stats: Res<Stats>,
    assets: Res<GameAssets>,
    wf: Res<WeaponFire>,
    sway: Res<CastleSway>,
    castle: Query<(Entity, &Transform, &Castle)>,
    mounts: Query<(Entity, &MountOf, &GlobalTransform)>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    mut pieces: Query<(&mut Piece, &mut Transform, &mut Visibility, &GlobalTransform), (Without<Castle>, Without<Boss>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let Ok((ce, ctf, c)) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let mut owners: HashMap<Entity, Drive> = HashMap::default();
    owners.insert(ce, Drive { kind: Owner::Castle, speed: c.speed, ..default() });

    let boss_spots: Vec<(Vec2, f32)> = bosses.iter().map(|(tf, en)| (p2(tf.translation), en.radius)).collect();
    for (e, mount, gt) in &mounts {
        let kind = mount.0;
        let (since, ability) = wf.since.get(&kind).copied().unwrap_or((99.0, false));
        let from = p2(gt.translation());
        let mut d = Drive {
            kind: Owner::Module(kind),
            speed: c.speed,
            accel: sway.accel,
            fired: since,
            just: since < 1e-6,
            charging: c.charge > 0.0,
            ..default()
        };
        if matches!(kind, ModuleKind::ArrowLoft | ModuleKind::Mortar | ModuleKind::Ballista) {
            // Same priorities as the guns themselves: a boss in range, else the nearest.
            let range = kind.range() * stats.range;
            let boss = boss_spots
                .iter()
                .filter(|(p, r)| p.distance(from) < range + r)
                .min_by(|a, b| a.0.distance(from).total_cmp(&b.0.distance(from)))
                .map(|(p, _)| *p);
            let target = boss.or_else(|| grid.nearest(from, range).map(|g| g.pos));
            d.aim = target.map(|p| wrap((p - from).to_angle() - c.heading));
        }
        if kind == ModuleKind::Broadside && d.just && !ability {
            // On its own, the broadside only fires toward the side with enemies.
            let out = p2(gt.rotation() * Vec3::Z).normalize_or_zero();
            let range = kind.range() * stats.range;
            let mut any = false;
            grid.for_each_near(cpos, range, |g| {
                if !any && g.pos.distance_squared(cpos) < range * range && (g.pos - cpos).normalize_or_zero().dot(out) > 0.5 {
                    any = true;
                }
            });
            d.side_ok = any;
        }
        owners.insert(e, d);
    }
    let turn = |rate: f32| 1.0 - (-rate * dt).exp();
    for (mut pc, mut tf, mut vis, gt) in &mut pieces {
        let d = owners.get(&pc.owner).copied().unwrap_or_default();
        match pc.role {
            Role::Wheel { r } => {
                pc.angle -= d.speed * dt / r.max(0.1);
                tf.rotation = pc.rest * Quat::from_rotation_z(pc.angle);
            }
            Role::Turret => {
                let want = d.aim.unwrap_or(0.0);
                pc.angle += wrap(want - pc.angle) * turn(7.0);
                let yaw_q = Quat::from_rotation_y(pc.angle);
                let kick = if matches!(d.kind, Owner::Module(_)) { recoil(d.fired) * 0.16 } else { 0.0 };
                tf.rotation = pc.rest * yaw_q;
                tf.translation = pc.pivot + yaw_q * Vec3::NEG_X * kick;
            }
            Role::Recoil { axis, dist, muzzle } => {
                if d.just {
                    pc.speed = if d.side_ok { 1.0 } else { 0.0 };
                    if d.side_ok {
                        let at = gt.transform_point(muzzle);
                        let dir = gt.rotation() * muzzle.normalize_or(Vec3::X);
                        match d.kind {
                            Owner::Module(ModuleKind::Mortar) => muzzle_blast(&mut commands, &assets, at, dir, 1.3),
                            Owner::Module(ModuleKind::Broadside) => muzzle_blast(&mut commands, &assets, at, dir, 1.0),
                            Owner::Module(ModuleKind::MineLayer) => {
                                spawn_fx(&mut commands, &assets, FxKind::Dust, at, 1.2, Vec3::Y * 0.5);
                            }
                            _ => {}
                        }
                    }
                }
                let k = recoil(d.fired) * pc.speed;
                tf.translation = pc.pivot + pc.rest * axis * dist * k;
            }
            Role::Archer => {
                let rest = pc.rest.to_euler(EulerRot::YXZ).0;
                let want = d.aim.unwrap_or(rest);
                pc.angle += wrap(want - pc.angle) * turn(9.0);
                // Lean back as the arrow leaves, breathe while waiting.
                let lean = recoil(d.fired) * 0.22;
                let breathe = (t * 2.1 + pc.pivot.z * 4.0).sin() * 0.015;
                tf.rotation = Quat::from_rotation_y(pc.angle) * Quat::from_rotation_z(lean + breathe);
            }
            Role::Ammo => {
                let back_at = 0.45;
                let want = if d.fired < back_at { Visibility::Hidden } else { Visibility::Inherited };
                if *vis != want {
                    *vis = want;
                }
                if matches!(d.kind, Owner::Module(_)) {
                    // The next bolt is winched forward into place.
                    let s = ((d.fired - back_at) / 0.35).clamp(0.0, 1.0);
                    tf.translation = pc.pivot + Vec3::NEG_X * (1.0 - ease_out(s)) * 0.9;
                }
            }
            Role::Swing => {
                // The ram log lags as you speed up and swings forward when you
                // brake; while charging it pounds back and forth.
                let target = if d.charging { (t * 11.0).sin() * 0.5 } else { 0.0 };
                let push = -d.accel * 0.6;
                pc.speed += ((target - pc.angle) * 55.0 - pc.speed * 3.5 + push) * dt;
                pc.angle = (pc.angle + pc.speed * dt).clamp(-0.7, 0.7);
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
