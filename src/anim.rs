//! Movement and body language.
//!
//! Wheels roll, weapons swing round to aim and kick back when they fire, the
//! castle leans into turns and rocks when hit, troops march, hack at the
//! walls and topple over when they die, and every boss works its own
//! machinery: the Siege Tower drops its drawbridge, the War Wagon's turret
//! tracks you, the Juggernaut's drill spins up before it charges, the
//! Trebuchet's arm whips over the top, and the Warlord's guns slam back.
//!
//! Gameplay code never reads any of this: it only changes how things look.

use crate::art::{castle_mounts, p2, w3};
use crate::assets::{GameAssets, PartHandle};
use crate::combat::{ArcShot, Bullet};
use crate::enemy::{Boss, BossState, Enemy, EnemyKind};
use crate::grid::Grid;
use crate::items::{Loadout, ModuleKind, Stats};
use crate::models::Role;
use crate::modules::Cooldowns;
use crate::player::{Castle, CastleBody, MountOf};
use crate::vfx::{explosion, muzzle_blast, rnd, spawn_fx, FxKind};
use crate::{GameEntity, GameRng, GameState, Shake, Sim};
use bevy::light::NotShadowCaster;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use rand::Rng;
use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponFire>()
            .init_resource::<Corpses>()
            .init_resource::<CastleSway>()
            .add_systems(OnEnter(GameState::Starting), reset)
            .add_systems(Update, (animate_troops, animate_bosses).in_set(Sim::Combat))
            .add_systems(
                Update,
                (
                    spawn_corpses.before(crate::combat::enemy_deaths),
                    rig_bosses,
                    (track_weapon_fire, animate_castle, animate_pieces).chain(),
                    animate_corpses,
                    animate_wrecks,
                    sink,
                    trails,
                )
                    .in_set(Sim::Resolve),
            );
    }
}

fn reset(mut wf: ResMut<WeaponFire>, mut corpses: ResMut<Corpses>, mut sway: ResMut<CastleSway>) {
    *wf = WeaponFire::default();
    corpses.0.clear();
    *sway = CastleSway::default();
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Stable 0..1 number per entity, so every soldier keeps his own stride.
fn hash01(e: Entity) -> f32 {
    let b = e.to_bits();
    let mut x = (b ^ (b >> 32)) as u32;
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    (x & 0xffff) as f32 / 65535.0
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
    /// The castle, weapon mount or boss whose state drives this piece.
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
    Boss(EnemyKind),
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
    /// Same for a second weapon (the Warlord's guns).
    fired2: f32,
    just2: bool,
    /// For the side cannons: whether the last shot came out of this side.
    side_ok: bool,
    /// Yaw to aim at, relative to the owner.
    aim: Option<f32>,
    spin: f32,
    open: f32,
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
            fired2: 99.0,
            just2: false,
            side_ok: true,
            aim: None,
            spin: 0.0,
            open: 0.0,
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

/// Trebuchet arm: whips over, settles, then is winched slowly back down.
fn arm_angle(s: f32) -> f32 {
    const SWING: f32 = -2.2;
    const THROW: f32 = 0.36;
    if s < THROW {
        let k = s / THROW;
        SWING * k * k
    } else if s < 0.95 {
        let k = (s - THROW) / (0.95 - THROW);
        SWING + 0.22 * (1.0 - k) * (k * PI * 3.0).sin()
    } else if s < 3.3 {
        SWING * (1.0 - smoothstep((s - 0.95) / 2.35))
    } else {
        0.0
    }
}

/// Siege Tower drawbridge: crashes down, stays while the squad runs out, then
/// is hauled back up.
fn ramp_open(s: f32) -> f32 {
    if s < 0.45 {
        let k = s / 0.45;
        k * k
    } else if s < 0.7 {
        1.0 - 0.06 * ((s - 0.45) / 0.25 * PI).sin()
    } else if s < 2.6 {
        1.0
    } else if s < 3.8 {
        1.0 - smoothstep((s - 2.6) / 1.2)
    } else {
        0.0
    }
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
    bosses: Query<(Entity, &Transform, &Enemy, &BossAnim)>,
    mut pieces: Query<(&mut Piece, &mut Transform, &mut Visibility, &GlobalTransform), (Without<Castle>, Without<BossAnim>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let Ok((ce, ctf, c)) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let mut owners: HashMap<Entity, Drive> = HashMap::default();
    owners.insert(ce, Drive { kind: Owner::Castle, speed: c.speed, ..default() });

    let boss_spots: Vec<(Vec2, f32)> = bosses.iter().map(|(_, tf, en, _)| (p2(tf.translation), en.radius)).collect();
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
    for (e, tf, en, ba) in &bosses {
        let yaw_now = tf.rotation.to_euler(EulerRot::YXZ).0;
        let aim = wrap((cpos - p2(tf.translation)).to_angle() - yaw_now);
        owners.insert(
            e,
            Drive {
                kind: Owner::Boss(en.kind),
                speed: ba.speed,
                fired: ba.fired,
                just: ba.fired < 1e-6,
                fired2: ba.fired2,
                just2: ba.fired2 < 1e-6,
                aim: Some(aim),
                spin: ba.spin,
                open: ramp_open(ba.fired),
                ..default()
            },
        );
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
                let (fired, just) = match d.kind {
                    Owner::Boss(EnemyKind::Warlord) => (d.fired2, d.just2),
                    _ => (d.fired, d.just),
                };
                if just {
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
                            Owner::Boss(EnemyKind::WarWagon) => muzzle_blast(&mut commands, &assets, at, dir, 1.7),
                            Owner::Boss(EnemyKind::Warlord) => muzzle_blast(&mut commands, &assets, at, dir, 2.3),
                            _ => {}
                        }
                    }
                }
                let k = recoil(fired) * pc.speed;
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
                let back_at = match d.kind {
                    Owner::Boss(EnemyKind::Trebuchet) => 3.1,
                    _ => 0.45,
                };
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
            Role::Spin => {
                pc.angle = (pc.angle + d.spin * dt) % TAU;
                tf.rotation = pc.rest * Quat::from_rotation_x(pc.angle);
            }
            Role::Ramp => tf.rotation = pc.rest * Quat::from_rotation_z(-d.open * 2.05),
            Role::Arm => tf.rotation = pc.rest * Quat::from_rotation_z(arm_angle(d.fired)),
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
// Troops: march, hack at the walls, flinch, get bowled over.
// ---------------------------------------------------------------------------

fn animate_troops(time: Res<Time>, mut q: Query<(Entity, &mut Transform, &Enemy), Without<Boss>>) {
    let t = time.elapsed_secs();
    q.par_iter_mut().for_each(|(e, mut tf, en)| {
        // `enemy_move` has just set a plain heading; add the body language on top.
        let seed = hash01(e);
        let phase = seed * TAU;
        let moving = (en.vel.length() / en.speed.max(0.1)).min(1.0);
        let (freq, hop, sway, lean) = match en.kind {
            EnemyKind::Knight => (10.0, 0.16, 0.025, 0.0),
            EnemyKind::Cart => (7.0, 0.04, 0.035, 0.0),
            _ => (9.5, 0.09, 0.07, 0.07),
        };
        let ph = t * freq * (0.85 + 0.3 * seed) + phase;
        let mut y = hop * ph.sin().abs() * moving;
        let mut roll = sway * ph.sin() * moving;
        let mut pitch = -lean * moving;
        if en.kind == EnemyKind::Knight {
            // Gallop: the horse rocks nose to tail.
            pitch += ph.cos() * 0.08 * moving;
        }
        let mut thrust = 0.0;
        if en.touching {
            // Hack and stab at the walls.
            let swing = (t * 6.5 + phase).sin().max(0.0).powi(3);
            match en.kind {
                EnemyKind::Knight => pitch += swing * 0.35,
                EnemyKind::Cart => thrust = swing * 0.4,
                _ => {
                    thrust = swing * 0.3;
                    pitch -= swing * 0.3;
                }
            }
        }
        if en.kind == EnemyKind::Archer && en.attack > 2.45 {
            // Recoil of the bowstring.
            pitch += 0.16 * ((en.attack - 2.45) / 0.35).clamp(0.0, 1.0);
        }
        // Bowled over by rams and blasts.
        let kn = en.knock.length();
        if kn > 1.0 {
            let local = tf.rotation.inverse() * w3(en.knock / kn, 0.0);
            let tip = ((kn - 1.0) * 0.035).min(0.9);
            roll += local.z * tip;
            pitch -= local.x * tip;
            y += (kn * 0.03).min(0.6);
        }
        let fwd = tf.rotation * Vec3::X;
        tf.translation += fwd * thrust + Vec3::Y * y;
        tf.rotation = tf.rotation * Quat::from_rotation_x(roll) * Quat::from_rotation_z(pitch);
        tf.scale = if en.flash > 0.0 { Vec3::new(1.12, 0.86, 1.12) } else { Vec3::ONE };
    });
}

// ---------------------------------------------------------------------------
// Bosses: body motion, events and their effects.
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct BossAnim {
    prev_pos: Vec2,
    /// Forward speed (for wheels).
    speed: f32,
    prev_t1: f32,
    prev_t2: f32,
    /// Seconds since its main attack (shot, throw, unloading troops).
    fired: f32,
    /// Seconds since its second attack (the Warlord's volley).
    fired2: f32,
    /// (pitch, roll) and how fast they change.
    tilt: Vec2,
    tilt_v: Vec2,
    prev_flash: bool,
    /// Drill speed.
    spin: f32,
    fx: f32,
}

/// Gives every new boss its moving pieces.
fn rig_bosses(mut commands: Commands, assets: Res<GameAssets>, new: Query<(Entity, &Transform, &Enemy, &Boss), Added<Boss>>) {
    for (e, tf, en, b) in &new {
        commands.entity(e).insert(BossAnim {
            prev_pos: p2(tf.translation),
            speed: 0.0,
            prev_t1: b.timer,
            prev_t2: b.timer2,
            fired: 99.0,
            fired2: 99.0,
            tilt: Vec2::ZERO,
            tilt_v: Vec2::ZERO,
            prev_flash: false,
            spin: 0.0,
            fx: 0.0,
        });
        if let Some(parts) = assets.boss_parts.get(&en.kind) {
            let parts = parts.clone();
            commands.entity(e).with_children(|p| spawn_pieces(p, &parts, e, |_| true));
        }
    }
}

fn animate_bosses(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q: Query<(&mut Transform, &Enemy, &Boss, &mut BossAnim)>,
) {
    let dt = time.delta_secs().max(1e-4);
    let t = time.elapsed_secs();
    for (mut tf, en, boss, mut ba) in &mut q {
        let pos = p2(tf.translation);
        let yaw_now = tf.rotation.to_euler(EulerRot::YXZ).0;
        let fwd = Vec2::from_angle(yaw_now);
        let fwd_speed = ((pos - ba.prev_pos) / dt).dot(fwd);
        ba.prev_pos = pos;
        let prev_speed = ba.speed;
        ba.speed += (fwd_speed - ba.speed) * (1.0 - (-10.0 * dt).exp());
        let accel = (ba.speed - prev_speed) / dt;

        // A timer jumping back up means the boss just did its thing.
        ba.fired += dt;
        ba.fired2 += dt;
        let kind = en.kind;
        if boss.timer > ba.prev_t1 + 0.5 && matches!(kind, EnemyKind::SiegeTower | EnemyKind::WarWagon | EnemyKind::Trebuchet) {
            ba.fired = 0.0;
            match kind {
                EnemyKind::WarWagon => ba.tilt_v.x += 0.5,
                EnemyKind::Trebuchet => ba.tilt_v.x -= 0.45,
                _ => {
                    // Drawbridge slams into the dirt.
                    for i in 0..4 {
                        let at = tf.transform_point(Vec3::new(5.4, 0.3, i as f32 * 1.0 - 1.5));
                        spawn_fx(&mut commands, &assets, FxKind::Dust, at, 2.4, Vec3::new(0.0, 0.8, 0.0));
                    }
                }
            }
        }
        if boss.timer2 > ba.prev_t2 + 0.5 && kind == EnemyKind::Warlord {
            ba.fired2 = 0.0;
            ba.tilt_v.x += 0.3;
        }
        ba.prev_t1 = boss.timer;
        ba.prev_t2 = boss.timer2;

        let spin_want = match boss.state {
            BossState::Windup => 26.0,
            BossState::Charging => 34.0,
            BossState::Approach => 1.2,
        };
        ba.spin += (spin_want - ba.spin) * (1.0 - (-2.5 * dt).exp());

        // Lean: rears back winding up, digs in while charging, sways if tall.
        let mut target = Vec2::new((-accel * 0.002).clamp(-0.05, 0.05), 0.0);
        let mut shake = 0.0;
        match boss.state {
            BossState::Windup => {
                target.x += 0.07;
                shake = 0.02;
            }
            BossState::Charging => target.x -= 0.05,
            BossState::Approach => {}
        }
        if kind == EnemyKind::SiegeTower {
            target.y += (t * 1.4).sin() * 0.03 * (ba.speed.abs() / 3.0).min(1.0);
        }
        let flashing = en.flash > 0.0;
        if flashing && !ba.prev_flash {
            ba.tilt_v += Vec2::new(rnd() - 0.5, rnd() - 0.5) * 0.25;
        }
        ba.prev_flash = flashing;
        let (tilt, tilt_v) = (ba.tilt, ba.tilt_v);
        ba.tilt_v += ((target - tilt) * 50.0 - tilt_v * 7.0) * dt;
        let v = ba.tilt_v;
        ba.tilt += v * dt;
        let jitter = Vec2::new(rnd() - 0.5, rnd() - 0.5) * shake;
        let tl = ba.tilt + jitter;
        // Yaw first (as the boss AI expects), then roll and pitch.
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw_now, tl.y, tl.x);
        tf.translation.y = ((t * 17.0 + en.radius).sin() * 0.03 * (ba.speed.abs() / 4.0).min(1.5)).abs();

        // Dust from the wheels, sparks from the drill.
        ba.fx += dt;
        let every = match boss.state {
            BossState::Charging => 0.05,
            BossState::Windup => 0.09,
            BossState::Approach if ba.speed.abs() > 2.0 => 0.3,
            _ => f32::INFINITY,
        };
        if ba.fx > every {
            ba.fx = 0.0;
            let side = Vec2::new(-fwd.y, fwd.x) * en.radius * 0.6 * if rnd() < 0.5 { -1.0 } else { 1.0 };
            let back = pos - fwd * en.radius * 0.7 + side;
            spawn_fx(&mut commands, &assets, FxKind::Dust, w3(back, 0.5), en.radius * 0.7, w3(-fwd * 2.0, 0.7));
            if kind == EnemyKind::Juggernaut && boss.state != BossState::Approach {
                let tip = tf.transform_point(Vec3::new(7.3, 2.4, 0.0));
                for _ in 0..3 {
                    let v = Vec3::new(rnd() - 0.5, rnd() * 0.8, rnd() - 0.5) * 12.0;
                    spawn_fx(&mut commands, &assets, FxKind::Spark, tip, 0.6, v);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The fallen: soldiers topple and sink away, bosses burn and collapse.
// ---------------------------------------------------------------------------

const MAX_CORPSES: usize = 260;

#[derive(Resource, Default)]
struct Corpses(VecDeque<Entity>);

#[derive(Component)]
struct Corpse {
    t: f32,
    axis: Vec3,
    rot0: Quat,
    /// Keeps the body above the grass once it lies down.
    lift: f32,
}

#[derive(Component)]
struct Wreck {
    t: f32,
    radius: f32,
    axis: Vec3,
    rot0: Quat,
    fx: f32,
}

/// Leaves a body behind for every soldier about to be removed this frame.
fn spawn_corpses(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut corpses: ResMut<Corpses>,
    mut rng: ResMut<GameRng>,
    troops: Query<(&Transform, &Enemy, &Mesh3d), Without<Boss>>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
) {
    for (tf, en, mesh) in &troops {
        if en.hp > 0.0 || en.kind == EnemyKind::Cart {
            continue;
        }
        let rot0 = Quat::from_rotation_y(tf.rotation.to_euler(EulerRot::YXZ).0);
        let fwd = rot0 * Vec3::X;
        let side = rot0 * Vec3::Z;
        let knock = w3(en.knock, 0.0);
        let fall = if en.kind == EnemyKind::Knight {
            side * if rng.0.random::<bool>() { 1.0 } else { -1.0 }
        } else if knock.length() > 2.0 {
            knock.normalize()
        } else {
            (-fwd + side * rng.0.random_range(-0.7..0.7)).normalize()
        };
        let axis = Vec3::Y.cross(fall).normalize_or(Vec3::Z);
        let lift = if en.kind == EnemyKind::Knight { 0.35 } else { 0.14 };
        let e = commands
            .spawn((
                GameEntity,
                Corpse { t: 0.0, axis, rot0, lift },
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(assets.m.vc.clone()),
                Transform::from_translation(Vec3::new(tf.translation.x, 0.0, tf.translation.z)).with_rotation(rot0),
                NotShadowCaster,
            ))
            .id();
        corpses.0.push_back(e);
        while corpses.0.len() > MAX_CORPSES {
            if let Some(old) = corpses.0.pop_front() {
                commands.entity(old).try_despawn();
            }
        }
    }
    for (tf, en) in &bosses {
        if en.hp > 0.0 {
            continue;
        }
        let a = rng.0.random_range(0.0..TAU);
        let id = commands
            .spawn((
                GameEntity,
                Wreck { t: 0.0, radius: en.radius, axis: Vec3::new(a.cos(), 0.0, a.sin()), rot0: tf.rotation, fx: 0.0 },
                Transform::from_translation(tf.translation).with_rotation(tf.rotation),
                Visibility::Visible,
            ))
            .id();
        let body = assets.bosses[&en.kind].clone();
        let parts = assets.boss_parts.get(&en.kind).cloned().unwrap_or_default();
        commands.entity(id).with_children(|p| {
            body.spawn_parts(p, ());
            spawn_pieces(p, &parts, id, |_| true);
        });
    }
}

fn animate_corpses(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Transform, &mut Corpse)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut c) in &mut q {
        c.t += dt;
        if c.t > 3.4 {
            commands.entity(e).despawn();
            continue;
        }
        let fall = (c.t / 0.34).min(1.0);
        let angle = fall * fall * FRAC_PI_2 * 0.96;
        let hop = (fall * PI).sin() * 0.12;
        let sink = ((c.t - 2.3) / 1.1).clamp(0.0, 1.0);
        tf.rotation = Quat::from_axis_angle(c.axis, angle) * c.rot0;
        tf.translation.y = hop + c.lift * fall - sink * (0.9 + c.lift);
    }
}

fn animate_wrecks(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    mut q: Query<(Entity, &mut Transform, &mut Wreck)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut w) in &mut q {
        w.t += dt;
        if w.t > 3.4 {
            commands.entity(e).despawn();
            continue;
        }
        // Lists over and sinks into a pall of smoke.
        let f = (w.t / 3.4).min(1.0);
        tf.rotation = Quat::from_axis_angle(w.axis, f * 0.3) * w.rot0;
        tf.translation.y = -f * f * w.radius * 1.2;
        w.fx += dt;
        let pos = p2(tf.translation);
        if w.fx > 0.1 {
            w.fx = 0.0;
            let off = Vec2::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(-1.0..1.0)) * w.radius * 0.7;
            let h = rng.0.random_range(1.0..w.radius.max(1.5));
            spawn_fx(&mut commands, &assets, FxKind::Fireball, w3(pos + off, h), w.radius * 0.55, Vec3::ZERO);
            spawn_fx(&mut commands, &assets, FxKind::DarkSmoke, w3(pos + off, h + 1.0), w.radius * 0.8, Vec3::Y * 3.0);
            spawn_fx(&mut commands, &assets, FxKind::Ember, w3(pos + off, h), 0.8, Vec3::new(rnd() - 0.5, 2.0, rnd() - 0.5) * 2.0);
        }
        if rng.0.random::<f32>() < dt / 0.45 {
            let off = Vec2::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(-1.0..1.0)) * w.radius * 0.6;
            explosion(&mut commands, &assets, &mut rng, pos + off, w.radius * 0.35);
            shake.0 += 0.25;
        }
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
