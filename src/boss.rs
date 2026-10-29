//! The five colossal bosses: spawning their rigs, animating their parts,
//! framing them with the camera, burning ground, warning markers, and the
//! way they collapse when destroyed. What each boss does in a fight is in
//! `boss_ai.rs`; their models are in `boss_models.rs`.

use crate::art::{p2, w3, yaw};
use crate::assets::{GameAssets, ModelHandle};
use crate::boss_models::{PartDef, PartKind, TREB_LONG};
use crate::combat::{Coin, FxKind, drop_coins, explosion, spawn_fx};
use crate::enemy::{Enemy, EnemyKind};
use crate::player::Castle;
use crate::progression::Progress;
use crate::{GameEntity, GameRng, GameState, Shake, Sim};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossCam>()
            .add_systems(OnEnter(GameState::Starting), |mut cam: ResMut<BossCam>| *cam = BossCam::default())
            .add_systems(Update, crate::boss_ai::boss_ai.in_set(Sim::Move))
            .add_systems(Update, (burn, wrecks).in_set(Sim::Combat))
            .add_systems(Update, (animate_rigs, frame_bosses).in_set(Sim::Resolve));
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// A boss model ready to spawn: uploaded meshes plus the part layout.
#[derive(Clone)]
pub struct Rig {
    pub models: Vec<ModelHandle>,
    pub parts: Vec<PartDef>,
}

/// Fixed facts about each boss.
pub struct Spec {
    /// Tallest point, so the camera stays above it.
    pub height: f32,
    /// Wheel tracks: distance of the outer wheels from the center line.
    pub track: f32,
    pub track_size: f32,
    /// Chimneys and fires that smoke (model space).
    pub smoke: &'static [Vec3],
    /// The camera banner when it arrives.
    pub title: &'static str,
    pub hint: &'static str,
}

const TOWER_SMOKE: [Vec3; 2] = [Vec3::new(4.8, 35.5, 4.6), Vec3::new(4.8, 35.5, -4.6)];
const WAGON_SMOKE: [Vec3; 1] = [Vec3::new(-8.8, 17.6, 0.0)];
const JUG_SMOKE: [Vec3; 2] = [Vec3::new(-11.0, 24.4, 3.4), Vec3::new(-11.0, 24.4, -3.4)];
const WARLORD_SMOKE: [Vec3; 2] = [Vec3::new(31.0, 29.0, 7.5), Vec3::new(31.0, 29.0, -7.5)];

pub fn spec(kind: EnemyKind) -> Spec {
    match kind {
        EnemyKind::SiegeTower => Spec {
            height: 44.0,
            track: 8.5,
            track_size: 1.4,
            smoke: &TOWER_SMOKE,
            title: "THE SIEGE TOWER",
            hint: "Archers rain arrows from the top. Hug it and it pours burning oil.",
        },
        EnemyKind::WarWagon => Spec {
            height: 18.0,
            track: 5.9,
            track_size: 1.3,
            smoke: &WAGON_SMOKE,
            title: "TWIN WAR WAGONS",
            hint: "They flank you from both sides. Stay out of their broadsides.",
        },
        EnemyKind::Juggernaut => Spec {
            height: 26.0,
            track: 8.3,
            track_size: 2.0,
            smoke: &JUG_SMOKE,
            title: "THE IRON JUGGERNAUT",
            hint: "Dodge sideways when it charges. Never stand before its ram.",
        },
        EnemyKind::Trebuchet => Spec {
            height: 48.0,
            track: 14.6,
            track_size: 1.8,
            smoke: &[],
            title: "THE TREBUCHET FORTRESS",
            hint: "Leave the red circles. Its ballistas guard it up close.",
        },
        _ => Spec {
            height: 76.0,
            track: 23.0,
            track_size: 2.6,
            smoke: &WARLORD_SMOKE,
            title: "THE WARLORD'S CITADEL",
            hint: "The final siege. It grows more desperate as it burns.",
        },
    }
}

/// What the boss is doing right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Idle,
    /// Siege Tower: lowers its ramp and sends out a squad.
    Unload,
    /// Siege Tower: archers loose arrows from the top.
    Volley,
    /// Siege Tower: burning oil all around its base.
    Oil,
    /// War Wagon: opens its gun ports and fires to one side.
    Broadside,
    /// Gathering speed before a charge (red lane on the ground).
    Windup,
    Charge,
    /// Crashed into something; briefly helpless.
    Stunned,
    /// Juggernaut: the ram swings forward.
    Swing,
    /// Juggernaut: blasts steam all around.
    Vent,
    /// Trebuchet: the arm swings and throws.
    Throw,
    /// Trebuchet: treadwheels wind the arm back down.
    Reload,
    /// Trebuchet and Warlord: burning fire pots.
    Pots,
    /// Warlord: the portcullis rises and knights ride out.
    Sally,
    /// Warlord: the great bombard fires a cross of shells.
    Bombard,
}

/// Where the parts are. The AI writes it; `animate_rigs` shows it.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// Distance rolled (turns the wheels).
    pub wheel: f32,
    /// Ramp, gun ports, portcullis: 0 closed, 1 open.
    pub hatch: f32,
    pub hatch_to: f32,
    /// Turret yaw relative to the body, per turret slot.
    pub turret: [f32; 5],
    /// Barrel recoil per slot, 1 right after a shot.
    pub recoil: [f32; 5],
    /// Juggernaut ram angle (radians).
    pub swing: f32,
    /// Trebuchet arm angle (radians, about the axle).
    pub arm: f32,
    /// Trebuchet sling angle in the model's side plane.
    pub sling: f32,
    /// The trebuchet has a boulder in its sling.
    pub loaded: bool,
    /// Treadwheel angle.
    pub reel: f32,
    /// Warlord's bombard pitch.
    pub pitch: f32,
    /// Fires burn brighter.
    pub glow: f32,
    /// Body lean (tall Siege Tower).
    pub lean: f32,
}

#[derive(Component)]
pub struct Boss {
    pub act: Act,
    /// Seconds into the current act.
    pub t: f32,
    /// Ability cooldowns.
    pub cd: [f32; 4],
    /// Per-turret reload timers.
    pub guns: [f32; 5],
    /// 0, 1, 2: grows as health drops.
    pub phase: u8,
    /// Locked direction for the current act (charges, broadsides).
    pub dir: Vec2,
    /// Progress counter inside an act (shots fired, waves sent).
    pub step: u32,
    /// Twin War Wagons: +1 leads, -1 takes the opposite side.
    pub side: f32,
    /// Charges left in a chain.
    pub chain: u32,
    pub heading: f32,
    pub speed: f32,
    pub enraged: bool,
    pub pose: Pose,
    /// Extra targets locked at the start of an act.
    pub marks: Vec<Vec2>,
    pub smoke_acc: f32,
    pub track_acc: f32,
}

impl Boss {
    fn new(kind: EnemyKind, heading: f32, index: usize) -> Self {
        let cd = match kind {
            EnemyKind::SiegeTower => [6.0, 3.0, 0.0, 0.0],
            EnemyKind::WarWagon => [1.5, 4.0, 5.0, 0.0],
            EnemyKind::Juggernaut => [4.0, 0.0, 0.0, 0.0],
            EnemyKind::Trebuchet => [2.0, 7.0, 0.0, 0.0],
            _ => [3.0, 0.0, 0.0, 0.0],
        };
        let pose = Pose { loaded: true, sling: 0.25, arm: TREB_REST, ..default() };
        Self {
            act: Act::Idle,
            t: 0.0,
            cd,
            guns: [0.0, 0.8, 1.6, 2.4, 3.2],
            phase: 0,
            dir: Vec2::from_angle(heading),
            step: 0,
            side: if index.is_multiple_of(2) { 1.0 } else { -1.0 },
            chain: 0,
            heading,
            speed: 0.0,
            enraged: false,
            pose,
            marks: Vec::new(),
            smoke_acc: 0.0,
            track_acc: 0.0,
        }
    }

    pub fn start(&mut self, act: Act) {
        self.act = act;
        self.t = 0.0;
        self.step = 0;
    }
}

/// Trebuchet arm angle when loaded (long end down at the back).
pub const TREB_REST: f32 = 0.96;
/// Trebuchet arm angle at the end of a throw (long end up in front).
pub const TREB_THROWN: f32 = -2.2;

/// The part entities of a boss, in the rig's order.
#[derive(Component)]
pub struct BossRig(pub Vec<Entity>);

#[derive(Component)]
pub struct RigPart {
    pub kind: PartKind,
    pub rest: Transform,
    pub k: f32,
    pub slot: u8,
}

/// A destroyed boss playing its collapse.
#[derive(Component)]
pub struct Wreck {
    pub kind: EnemyKind,
    pub t: f32,
    pub dur: f32,
    pub radius: f32,
    pub half_len: f32,
    pub coins: u32,
    pub base: Transform,
    /// Direction the Siege Tower falls (away from the castle).
    pub fall: Vec2,
    pub boom: f32,
    pub crashed: bool,
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

/// Builds the rig under `root` and makes it a boss.
pub fn attach_boss(commands: &mut Commands, assets: &GameAssets, kind: EnemyKind, root: Entity, heading: f32, index: usize) {
    let rig = &assets.bosses[&kind];
    let mut ents: Vec<Entity> = Vec::with_capacity(rig.parts.len());
    for p in &rig.parts {
        let parent = p.parent.map(|j| ents[j]).unwrap_or(root);
        let rest = Transform::from_translation(p.at).with_rotation(p.rot);
        let model = rig.models[p.model].clone();
        let hidden = p.kind == PartKind::Glow && p.slot == 1;
        let e = commands
            .spawn((
                RigPart { kind: p.kind, rest, k: p.k, slot: p.slot },
                rest,
                if hidden { Visibility::Hidden } else { Visibility::Inherited },
                ChildOf(parent),
            ))
            .with_children(|c| model.spawn_parts(c, ()))
            .id();
        ents.push(e);
    }
    commands.entity(root).insert((Boss::new(kind, heading, index), BossRig(ents)));
}

/// Model space (built facing +X) to the world, for a boss at `pos` facing `heading`.
pub fn to_world(pos: Vec2, heading: f32, p: Vec3) -> Vec3 {
    w3(pos, 0.0) + yaw(heading) * p
}

// ---------------------------------------------------------------------------
// Warnings and burning ground
// ---------------------------------------------------------------------------

/// A red circle on the ground: a ring closes in on its edge as the hit nears.
pub fn warn_circle(commands: &mut Commands, assets: &GameAssets, at: Vec2, radius: f32, dur: f32) {
    spawn_fx(commands, assets, FxKind::Telegraph { dur }, w3(at, 0.14), radius, Vec3::ZERO);
    spawn_fx(commands, assets, FxKind::Zone { dur }, w3(at, 0.1), radius, Vec3::ZERO);
}

/// A red lane on the ground from `from` along `dir`.
pub fn warn_lane(commands: &mut Commands, assets: &GameAssets, from: Vec2, dir: Vec2, len: f32, width: f32, dur: f32) {
    let mid = from + dir * (len / 2.0);
    let e = spawn_fx(commands, assets, FxKind::Lane { dur }, w3(mid, 0.12), 1.0, Vec3::ZERO);
    commands
        .entity(e)
        .insert(Transform::from_translation(w3(mid, 0.12)).with_rotation(yaw(dir.to_angle())).with_scale(Vec3::new(len, 0.05, width)));
}

/// Burning ground that hurts the castle while it stands in it.
#[derive(Component)]
pub struct Fire {
    radius: f32,
    dps: f32,
    life: f32,
    max: f32,
    acc: f32,
}

pub fn spawn_fire(commands: &mut Commands, assets: &GameAssets, at: Vec2, radius: f32, dps: f32, life: f32) {
    commands.spawn((
        GameEntity,
        Fire { radius, dps, life, max: life, acc: 0.0 },
        Mesh3d(assets.disc.clone()),
        MeshMaterial3d(assets.m.burning.clone()),
        Transform::from_translation(w3(at, 0.07)).with_scale(Vec3::new(radius * 2.0, 0.04, radius * 2.0)),
        NotShadowCaster,
    ));
    spawn_fx(commands, assets, FxKind::Scorch, w3(at, 0.04), radius * 2.0, Vec3::ZERO);
}

fn burn(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut fires: Query<(Entity, &mut Transform, &mut Fire)>,
    mut castle: Query<(&Transform, &mut Castle), Without<Fire>>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    for (e, mut tf, mut f) in &mut fires {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let pos = p2(tf.translation);
        let k = (f.life / 0.8).min(1.0) * (1.0 - (f.life - f.max + 0.25).max(0.0) / 0.25).max(0.3);
        tf.scale = Vec3::new(f.radius * 2.0 * k, 0.04, f.radius * 2.0 * k);
        f.acc += dt;
        while f.acc > 0.16 {
            f.acc -= 0.16;
            let off = Vec2::from_angle(rng.0.random::<f32>() * TAU) * f.radius * rng.0.random::<f32>().sqrt() * 0.85;
            spawn_fx(&mut commands, &assets, FxKind::Fireball, w3(pos + off, 0.4), 0.9 + rng.0.random::<f32>() * 1.2, Vec3::ZERO);
            if rng.0.random::<f32>() < 0.3 {
                spawn_fx(&mut commands, &assets, FxKind::DarkSmoke, w3(pos + off, 1.0), 1.6, Vec3::Y * 3.0);
            }
        }
        if pos.distance(cpos) < f.radius + c.radius * 0.5 {
            c.hurt(f.dps * dt);
        }
    }
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

fn animate_rigs(
    time: Res<Time>,
    bosses: Query<(&Boss, &BossRig)>,
    mut parts: Query<(&mut Transform, &RigPart, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (boss, rig) in &bosses {
        let p = &boss.pose;
        for &e in &rig.0 {
            let Ok((mut tf, part, mut vis)) = parts.get_mut(e) else {
                continue;
            };
            let rest = part.rest;
            let (mut off, mut rot, mut scale) = (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
            let slot = part.slot as usize;
            match part.kind {
                PartKind::Body => continue,
                PartKind::Wheel => {
                    let dir = if part.slot == 1 { 1.0 } else { -1.0 };
                    rot = Quat::from_rotation_z(dir * p.wheel / part.k);
                }
                PartKind::Sway => rot = Quat::from_rotation_z(p.lean),
                PartKind::Turret => rot = Quat::from_rotation_y(p.turret[slot.min(4)]),
                PartKind::Barrel => off = rest.rotation * Vec3::new(-p.recoil[slot.min(4)] * part.k, 0.0, 0.0),
                PartKind::Bridge => rot = Quat::from_rotation_z(p.hatch * part.k),
                PartKind::Port => rot = Quat::from_rotation_x(p.hatch * part.k),
                PartKind::Portcullis => off = Vec3::Y * (p.hatch * part.k),
                PartKind::RamBeam => off = Vec3::new(p.swing.sin(), 1.0 - p.swing.cos(), 0.0) * part.k,
                PartKind::RamChain => rot = Quat::from_rotation_z(p.swing),
                PartKind::Arm => rot = Quat::from_rotation_z(p.arm),
                PartKind::Hanging => {
                    let world = if part.slot == 1 { p.sling } else { -FRAC_PI_2 + (t * 1.7).sin() * 0.03 };
                    rot = Quat::from_rotation_z(world + FRAC_PI_2 - p.arm);
                }
                PartKind::Load => {
                    let want = if p.loaded { Visibility::Inherited } else { Visibility::Hidden };
                    if *vis != want {
                        *vis = want;
                    }
                }
                PartKind::Glow => {
                    let lit = part.slot == 0 || p.glow > 0.5;
                    let want = if lit { Visibility::Inherited } else { Visibility::Hidden };
                    if *vis != want {
                        *vis = want;
                    }
                    let flick = 1.0 + 0.12 * (t * 9.0 + (e.to_bits() % 97) as f32).sin() + 0.07 * (t * 14.3).sin();
                    let boost = if part.slot == 0 { 1.0 + p.glow * 0.7 } else { 1.0 };
                    scale = Vec3::new(flick, flick * (1.0 + 0.1 * (t * 6.1).sin()), flick) * boost;
                }
                PartKind::Flag => rot = Quat::from_rotation_y((t * 2.1 + part.slot as f32 * 1.7).sin() * 0.3 + 0.15 * (t * 3.7).sin()),
                PartKind::Treadwheel => rot = Quat::from_rotation_z(p.reel),
                PartKind::Bombard => {
                    rot = Quat::from_rotation_z(p.pitch);
                    off = rot * Vec3::new(-p.recoil[0] * part.k, 0.0, 0.0);
                }
            }
            tf.translation = rest.translation + off;
            tf.rotation = rest.rotation * rot;
            tf.scale = scale;
        }
    }
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// How the camera should frame the nearest boss. Read by `follow_camera`.
#[derive(Resource, Default)]
pub struct BossCam {
    /// Wanted zoom (camera height multiplier); 0 when no boss is near.
    pub zoom: f32,
    /// Shift of the camera's focus from the castle toward the boss.
    pub offset: Vec2,
}

/// Height of the game camera above its focus before any zoom (see `world::follow_camera`).
const CAMERA_BASE: f32 = 40.0;
/// The camera never rises above this many times `CAMERA_BASE` for a boss.
const MAX_ZOOM: f32 = 3.4;

/// Whether `p` shows on screen, clear of the HUD at the top, when the game
/// camera is `h` above `focus`. Matches `world::follow_camera`: the camera sits
/// 0.42h behind its focus, looks at it, and has a 32° vertical field of view.
fn on_screen(h: f32, focus: Vec2, p: Vec3) -> bool {
    let look = w3(focus, 0.0);
    let cam = Transform::from_translation(look + Vec3::new(0.0, h, h * 0.42)).looking_at(look, Vec3::Y);
    let v = cam.compute_affine().inverse().transform_point3(p);
    if v.z > -1.0 {
        return false;
    }
    let t = (16f32).to_radians().tan();
    let (x, y) = (v.x / -v.z / (t * 16.0 / 9.0), v.y / -v.z / t);
    // The bars across the top cover about a fifth of the screen.
    x.abs() < 0.92 && (-0.9..0.62).contains(&y)
}

fn frame_bosses(
    castle: Query<(&Transform, &Castle)>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    mut cam: ResMut<BossCam>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let mut best: Option<(f32, f32, Vec2)> = None;
    for (tf, en) in &bosses {
        let pos = p2(tf.translation);
        let axis = en.axis(tf);
        let q = crate::grid::closest_on_segment(pos - axis, pos + axis, cpos);
        let gap = q.distance(cpos) - en.radius - c.radius;
        // A point well inside the boss, so a good part of it is on screen.
        let into = (pos - q).normalize_or_zero() * en.radius.min(18.0) + (q - cpos).normalize_or_zero() * en.radius * 0.6;
        let target = q + into;
        let weight = ((130.0 - gap) / 60.0).clamp(0.0, 1.0);
        let weight = weight * weight * (3.0 - 2.0 * weight);
        let offset = (target - cpos) * 0.42;
        let focus = cpos + offset;
        // Raise the camera until the castle, the near part of the boss and the
        // top of the boss are all on screen. Tall bosses need this: seen from
        // above, their top lands much further out than their base.
        let points = [w3(cpos, 4.0), w3(target, 0.0), w3(pos, spec(en.kind).height)];
        let mut h = CAMERA_BASE;
        while h < CAMERA_BASE * MAX_ZOOM && !points.iter().all(|&p| on_screen(h, focus, p)) {
            h += 2.0;
        }
        let need = h / CAMERA_BASE;
        let zoom = 1.0 + (need.min(MAX_ZOOM) - 1.0).max(0.0) * weight;
        if best.is_none_or(|b| zoom > b.0) {
            best = Some((zoom, weight, offset * weight));
        }
    }
    match best {
        Some((zoom, w, offset)) if w > 0.0 => {
            cam.zoom = zoom;
            cam.offset = offset;
        }
        _ => {
            cam.zoom = 0.0;
            cam.offset = Vec2::ZERO;
        }
    }
}

// ---------------------------------------------------------------------------
// Collapse
// ---------------------------------------------------------------------------

/// Turns a destroyed boss into a wreck that plays its collapse.
pub fn begin_wreck(commands: &mut Commands, e: Entity, en: &Enemy, tf: &Transform, castle: Vec2) {
    let pos = p2(tf.translation);
    let dur = match en.kind {
        EnemyKind::SiegeTower => 4.6,
        EnemyKind::WarWagon => 3.6,
        EnemyKind::Juggernaut => 4.0,
        EnemyKind::Trebuchet => 4.4,
        _ => 6.5,
    };
    commands.entity(e).remove::<(Enemy, Boss)>().insert(Wreck {
        kind: en.kind,
        t: 0.0,
        dur,
        radius: en.radius,
        half_len: en.half_len,
        coins: en.coins,
        base: *tf,
        fall: (pos - castle).normalize_or(Vec2::X),
        boom: 0.0,
        crashed: false,
    });
}

fn wrecks(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    mut progress: ResMut<Progress>,
    coins: Query<(), With<Coin>>,
    mut wrecks: Query<(Entity, &mut Transform, &mut Wreck, &BossRig)>,
    mut parts: Query<(&mut Transform, &RigPart), Without<Wreck>>,
) {
    let dt = time.delta_secs();
    let mut coin_count = coins.iter().count();
    for (e, mut tf, mut w, rig) in &mut wrecks {
        w.t += dt;
        let t = w.t;
        let base = w.base;
        let pos = p2(base.translation);
        let heading = base.rotation.to_euler(EulerRot::YXZ).0;
        let fwd = Vec2::from_angle(heading);
        let side = Vec2::new(-fwd.y, fwd.x);
        let ext = w.half_len + w.radius;

        // Explosions all over the hull, faster near the end.
        w.boom -= dt;
        if w.boom <= 0.0 && t < w.dur - 0.3 {
            w.boom = rng.0.random_range(0.12..0.28) * if t > w.dur * 0.6 { 0.6 } else { 1.0 };
            let along = match w.kind {
                // The Juggernaut blows up from the front to the back.
                EnemyKind::Juggernaut => ext * (1.0 - 2.0 * (t / w.dur)) + rng.0.random_range(-3.0..3.0),
                _ => rng.0.random_range(-1.0..1.0) * (w.half_len + w.radius * 0.7),
            };
            let across = rng.0.random_range(-1.0..1.0) * w.radius * 0.8;
            let mut at = pos + fwd * along + side * across;
            if w.kind == EnemyKind::SiegeTower && w.crashed {
                at = pos + w.fall * rng.0.random_range(0.0..40.0) + side * rng.0.random_range(-6.0..6.0);
            }
            let size = rng.0.random_range(2.5..4.5) * (w.radius / 10.0).clamp(0.8, 1.8);
            explosion(&mut commands, &assets, &mut rng, at, size);
            spawn_fx(&mut commands, &assets, FxKind::DarkSmoke, w3(at, 6.0), w.radius * 0.5, Vec3::Y * 6.0);
            shake.0 += 0.25;
        }

        // Each boss falls apart in its own way.
        let sink = |start: f32, depth: f32| -> f32 {
            let k = ((t - start) / (w.dur - start)).clamp(0.0, 1.0);
            -depth * k * k
        };
        match w.kind {
            EnemyKind::SiegeTower => {
                // Topples away from the castle, then lies smoking.
                let fall_t = 2.2;
                let k = (t / fall_t).min(1.0);
                let angle = 1.42 * k * k;
                let fall3 = w3(w.fall, 0.0);
                let axis = Vec3::Y.cross(fall3).normalize_or(Vec3::X);
                let pivot = base.translation + fall3 * w.radius;
                let r = Quat::from_axis_angle(axis, angle);
                tf.translation = pivot + r * (base.translation - pivot) + Vec3::Y * sink(3.2, 6.0);
                tf.rotation = r * base.rotation;
                if k >= 1.0 && !w.crashed {
                    w.crashed = true;
                    shake.0 += 3.5;
                    for i in 0..10 {
                        let at = pos + w.fall * (8.0 + i as f32 * 4.0);
                        spawn_fx(&mut commands, &assets, FxKind::Smoke, w3(at, 1.5), 7.0, Vec3::new(0.0, 2.0, 0.0));
                        spawn_fx(&mut commands, &assets, FxKind::Scorch, w3(at, 0.05), 9.0, Vec3::ZERO);
                    }
                }
            }
            EnemyKind::WarWagon => {
                // The turret is blown off; the hull lists and sinks.
                tf.translation = base.translation + Vec3::Y * sink(1.2, 9.0);
                tf.rotation = base.rotation * Quat::from_rotation_x(0.14 * (t / 1.0).min(1.0));
            }
            EnemyKind::Juggernaut => {
                tf.translation = base.translation + Vec3::Y * sink(1.6, 11.0);
                tf.rotation = base.rotation * Quat::from_rotation_z(-0.12 * (t / 2.0).min(1.0));
            }
            EnemyKind::Trebuchet => {
                tf.translation = base.translation + Vec3::Y * sink(1.8, 14.0);
                tf.rotation = base.rotation * Quat::from_rotation_x(0.08 * (t / 2.5).min(1.0));
            }
            _ => {
                // The citadel shudders and sinks into the earth.
                let shudder = (t * 31.0).sin() * 0.25 * (1.0 - t / w.dur);
                tf.translation = base.translation + Vec3::Y * sink(1.0, 30.0) + w3(side * shudder, 0.0);
                tf.rotation = base.rotation * Quat::from_rotation_z(0.05 * (t / w.dur));
                shake.0 = shake.0.max(0.6);
            }
        }

        // Loose parts: turrets fly off, the trebuchet arm snaps.
        let mut turret_n = 0;
        for &pe in &rig.0 {
            let Ok((mut ptf, part)) = parts.get_mut(pe) else {
                continue;
            };
            match part.kind {
                PartKind::Turret => {
                    let delay = turret_n as f32 * 0.7 + if w.kind == EnemyKind::WarWagon { 0.3 } else { 0.8 };
                    turret_n += 1;
                    let s = (t - delay).max(0.0);
                    if s > 0.0 {
                        let out = Vec3::new(0.3, 0.0, if part.rest.translation.z >= 0.0 { 0.6 } else { -0.6 });
                        let up = 16.0 * s - 14.0 * s * s;
                        ptf.translation = part.rest.translation + out * (s * 9.0) + Vec3::Y * up.max(-part.rest.translation.y);
                        ptf.rotation = part.rest.rotation * Quat::from_rotation_x(s * 3.0) * Quat::from_rotation_z(s * 2.0);
                    }
                }
                PartKind::Arm => {
                    // Snaps and crashes down onto the walls.
                    let k = (t / 1.4).min(1.0);
                    ptf.rotation = Quat::from_rotation_z(TREB_THROWN + (PI * 0.62) * k * k);
                    ptf.translation = part.rest.translation - Vec3::Y * (8.0 * k * k);
                }
                PartKind::RamBeam => {
                    let k = (t / 0.9).min(1.0);
                    ptf.translation = part.rest.translation - Vec3::Y * (6.0 * k * k);
                    ptf.rotation = Quat::from_rotation_z(-0.3 * k);
                }
                _ => {}
            }
        }

        if t >= w.dur {
            for i in 0..8 {
                let at = pos + Vec2::from_angle(i as f32 / 8.0 * TAU) * (w.radius * 0.6) + fwd * (w.half_len * (i % 3) as f32 / 2.0 - w.half_len / 2.0);
                explosion(&mut commands, &assets, &mut rng, at, w.radius * 0.45);
            }
            for _ in 0..12 {
                let off = Vec3::new(rng.0.random_range(-1.0..1.0), 0.0, rng.0.random_range(-1.0..1.0)) * ext;
                spawn_fx(&mut commands, &assets, FxKind::DarkSmoke, w3(pos, 3.0) + off, w.radius * 0.7, Vec3::Y * 4.0);
            }
            shake.0 += 3.0;
            let center = if w.kind == EnemyKind::SiegeTower { pos + w.fall * 18.0 } else { pos };
            drop_coins(&mut commands, &assets, &mut progress, &mut coin_count, center, w.coins, ext);
            commands.entity(e).despawn();
        }
    }
}

/// Where the trebuchet's sling pouch is (model space), for its current pose.
pub fn sling_pouch(pose: &Pose) -> Vec3 {
    use crate::boss_models::{TREB_AXLE, TREB_SLING};
    let tip = TREB_AXLE + Quat::from_rotation_z(pose.arm) * Vec3::new(-TREB_LONG, 0.0, 0.0);
    tip + Vec3::new(pose.sling.cos(), pose.sling.sin(), 0.0) * TREB_SLING
}
