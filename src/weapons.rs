//! Weapon animation. Turrets swing round to track targets, the ballista
//! looses and is winched back, the bombard and cannons kick and run out,
//! archers turn and loose, the ram swings on its chains, and the mine chute
//! feeds one mine at a time.

use crate::art::p2;
use crate::assets::{GameAssets, ModelHandle};
use crate::combat::{spawn_fx, FxKind};
use crate::grid::Grid;
use crate::items::{ModuleKind, Stats};
use crate::player::Castle;
use crate::weapon_models::{self as wm, ballista as bow, Role};
use crate::Sim;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WeaponFired>().add_systems(Update, animate.in_set(Sim::Resolve));
    }
}

/// A weapon just fired.
#[derive(Message, Clone, Copy)]
pub struct WeaponFired {
    pub kind: ModuleKind,
    /// Where it aimed, on the ground plane.
    pub target: Option<Vec2>,
    /// Which wall fired (+1 right, -1 left), or 0 for every copy.
    pub side: f32,
    /// Seconds until it can fire again.
    pub reload: f32,
}

/// Uploaded rig: the fixed mount and its moving parts.
#[derive(Clone)]
pub struct RigHandle {
    pub base: ModelHandle,
    pub parts: Vec<PartHandle>,
}

#[derive(Clone)]
pub struct PartHandle {
    pub role: Role,
    pub parent: Option<usize>,
    pub rest: Transform,
    pub model: ModelHandle,
}

/// Animation state of one mounted weapon. The parts read their pose from here.
#[derive(Component)]
pub struct WeaponAnim {
    kind: ModuleKind,
    /// +1 for the weapon itself, -1 for the mirrored copy on the left wall.
    side: f32,
    /// Which way the mount faces on the castle (turn about Y).
    facing: f32,
    since: f32,
    reload: f32,
    shots: u32,
    aim: f32,
    elev: f32,
    focus: Option<Vec2>,
    focus_t: f32,
    swing: f32,
    swing_vel: f32,
    last_speed: f32,
    pose: Vec<Transform>,
}

impl WeaponAnim {
    pub fn new(kind: ModuleKind, side: f32, rig: &RigHandle) -> Self {
        Self {
            kind,
            side,
            facing: 0.0,
            since: 99.0,
            reload: 1.0,
            shots: 0,
            aim: 0.0,
            elev: wm::BOMBARD_ELEV,
            focus: None,
            focus_t: 0.0,
            swing: 0.0,
            swing_vel: 0.0,
            last_speed: 0.0,
            pose: rig.parts.iter().map(|p| p.rest).collect(),
        }
    }

    /// The mount is turned by `turn` on the castle.
    pub fn facing(mut self, turn: f32) -> Self {
        self.facing = turn;
        self
    }
}

/// A moving part of a weapon.
#[derive(Component)]
pub struct RigPiece {
    owner: Entity,
    index: usize,
}

/// Spawns the mount and its moving parts under the weapon entity `owner`.
pub fn spawn_rig(p: &mut ChildSpawnerCommands, rig: &RigHandle, owner: Entity) {
    rig.base.spawn_parts(p, ());
    spawn_children(p, rig, None, owner);
}

fn spawn_children(p: &mut ChildSpawnerCommands, rig: &RigHandle, parent: Option<usize>, owner: Entity) {
    for (i, part) in rig.parts.iter().enumerate() {
        if part.parent != parent {
            continue;
        }
        p.spawn((RigPiece { owner, index: i }, part.rest, Visibility::Inherited)).with_children(|q| {
            part.model.spawn_parts(q, ());
            spawn_children(q, rig, Some(i), owner);
        });
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Sharp jolt that dies away: rises over `up` seconds, then decays at `rate`.
fn kick(t: f32, up: f32, rate: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < up {
        t / up
    } else {
        (-(t - up) * rate).exp()
    }
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(2.0 * PI) - PI
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    stats: Res<Stats>,
    assets: Res<GameAssets>,
    mut fired: MessageReader<WeaponFired>,
    castle: Query<(&Transform, &Castle)>,
    mut mounts: Query<(&GlobalTransform, &mut WeaponAnim)>,
    mut pieces: Query<(&RigPiece, &mut Transform), Without<Castle>>,
) {
    let Ok((_, c)) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let shots: Vec<WeaponFired> = fired.read().copied().collect();
    for (gt, mut an) in &mut mounts {
        let Some(rig) = assets.rigs.get(&an.kind) else {
            continue;
        };
        let an = &mut *an;
        an.since += dt;
        let (kind, side) = (an.kind, an.side);
        for s in shots.iter().filter(|s| s.kind == kind && (s.side == 0.0 || s.side == side)) {
            an.since = 0.0;
            an.reload = s.reload.max(0.4);
            an.shots += 1;
            if let Some(t) = s.target {
                an.focus = Some(t);
                an.focus_t = 0.7;
            }
            if an.kind == ModuleKind::Ram {
                // Crew hauls the log back to start the Charge.
                an.swing_vel -= 3.0;
            }
            if an.kind == ModuleKind::Broadside {
                let muzzle = gt.transform_point(Vec3::Y * 0.1 + wm::CANNON_MUZZLE);
                let out = gt.rotation() * Vec3::Z;
                spawn_fx(&mut commands, &assets, FxKind::Smoke, muzzle, 1.0, out * 3.0 + Vec3::Y);
            }
        }

        // Aim at the last target for a moment, otherwise at the nearest enemy.
        let pos = p2(gt.translation());
        an.focus_t -= dt;
        let aims = matches!(an.kind, ModuleKind::Ballista | ModuleKind::Mortar | ModuleKind::ArrowLoft);
        let range = (an.kind.range() * stats.range).max(1.0);
        let target = match an.focus {
            _ if !aims => None,
            Some(f) if an.focus_t > 0.0 => Some(f),
            _ => grid.nearest(pos, range).map(|g| g.pos),
        };
        if let Some(t) = target {
            let want = wrap((t - pos).to_angle() - c.heading - an.facing);
            let rate = match an.kind {
                ModuleKind::ArrowLoft => 5.0,
                ModuleKind::Ballista => 3.2,
                _ => 2.0,
            };
            let d = wrap(want - an.aim);
            an.aim = wrap(an.aim + d.clamp(-rate * dt, rate * dt));
            let want_elev = (1.15 - 0.45 * (t.distance(pos) / range)).clamp(0.7, 1.15);
            an.elev += (want_elev - an.elev).clamp(-0.6 * dt, 0.6 * dt);
        }

        if an.kind == ModuleKind::Ram {
            swing_ram(an, c, dt, time.elapsed_secs());
        }
        pose(an, rig);
    }
    for (piece, mut tf) in &mut pieces {
        if let Ok((_, an)) = mounts.get(piece.owner)
            && let Some(p) = an.pose.get(piece.index)
            // Only write when it moved: a written transform makes Bevy
            // recompute the part and every mesh under it.
            && *tf != *p
        {
            *tf = *p;
        }
    }
}

/// The ram hangs like a pendulum: it lags when the castle speeds up or
/// brakes, and the crew drives it in big strokes during a Charge.
fn swing_ram(an: &mut WeaponAnim, c: &Castle, dt: f32, now: f32) {
    let acc = if dt > 0.0 { ((c.speed - an.last_speed) / dt).clamp(-40.0, 40.0) } else { 0.0 };
    an.last_speed = c.speed;
    let steps = (dt / 0.008).ceil().max(1.0) as i32;
    let h = dt / steps as f32;
    let g = 9.8 / wm::RAM_CHAIN;
    for _ in 0..steps {
        let (th, w) = (an.swing, an.swing_vel);
        let mut a = -g * th.sin() - 2.2 * w - acc * 0.9 * th.cos() + (now * 6.0).sin() * c.speed.abs() * 0.12;
        if c.charge > 0.0 && th.abs() < 0.5 {
            a += 32.0 * if w >= 0.0 { 1.0 } else { -1.0 };
        }
        an.swing_vel = w + a * h;
        an.swing = th + an.swing_vel * h;
        if an.swing > 0.62 {
            an.swing = 0.62;
            an.swing_vel = an.swing_vel.min(0.0) * 0.35 - 0.3;
        }
        if an.swing < -0.55 {
            an.swing = -0.55;
            an.swing_vel = an.swing_vel.max(0.0) * 0.3;
        }
    }
}

struct BowPose {
    arm: f32,
    claw: f32,
    nock: Vec3,
    bolt: Option<Vec3>,
    /// How much rope is wound on the windlass (1 = drawn back).
    wound: f32,
}

/// Loose, catch the string, winch back, drop in a new bolt.
fn bow_pose(t: f32, reload: f32) -> BowPose {
    use bow::*;
    let back = Vec3::new(CLAW_BACK, CLAW_Y, 0.0) + NOCK;
    let free = |a: f32| Vec3::new(tip(a, 1.0).x, ARM_PIVOT.y, 0.0);
    let snap = 0.07;
    let catch = (0.25 * reload).max(snap + 0.1);
    let wound = (0.82 * reload).max(catch + 0.2);
    let loaded = wound + 0.15;
    let front = claw_front();
    if t < snap {
        let e = (t / snap).powi(2);
        let arm = ARM_COCKED + (ARM_FIRED + 0.08 - ARM_COCKED) * e;
        BowPose { arm, claw: CLAW_BACK, nock: back.lerp(free(arm), e), bolt: None, wound: 1.0 }
    } else if t < catch {
        let u = smooth((t - snap) / (catch - snap));
        let arm = ARM_FIRED + 0.08 * (-(t - snap) * 9.0).exp() * ((t - snap) * 55.0).cos();
        BowPose { arm, claw: CLAW_BACK + (front - CLAW_BACK) * u, nock: free(arm), bolt: None, wound: 1.0 - u }
    } else if t < wound {
        let d = smooth((t - catch) / (wound - catch));
        let claw = front + (CLAW_BACK - front) * d;
        BowPose { arm: ARM_FIRED + (ARM_COCKED - ARM_FIRED) * d, claw, nock: Vec3::new(claw, CLAW_Y, 0.0) + NOCK, bolt: None, wound: d }
    } else {
        let l = smooth((t - wound) / (loaded - wound));
        BowPose { arm: ARM_COCKED, claw: CLAW_BACK, nock: back, bolt: Some(Vec3::new(-0.15, 0.3, 0.0) * (1.0 - l)), wound: 1.0 }
    }
}

/// Distance the cannon carriage has rolled back.
fn recoil(t: f32, reload: f32) -> f32 {
    let back = 0.07;
    let hold = (0.2 * reload).max(back + 0.12);
    let out = (0.8 * reload).max(hold + 0.3);
    let r = wm::CANNON_RECOIL;
    if t < back {
        r * (1.0 - (1.0 - t / back).powi(2))
    } else if t < hold {
        r
    } else {
        r * (1.0 - smooth((t - hold) / (out - hold)))
    }
}

fn pose(an: &mut WeaponAnim, rig: &RigHandle) {
    let t = an.since;
    let bp = (an.kind == ModuleKind::Ballista).then(|| bow_pose(t, an.reload));
    let rec = recoil(t, an.reload);
    let jolt = kick(t, 0.04, 9.0);
    for (i, part) in rig.parts.iter().enumerate() {
        let rest = part.rest;
        let tf = match part.role {
            Role::Turret => {
                let rot = Quat::from_rotation_y(an.aim);
                let size = if an.kind == ModuleKind::Ballista { 0.035 } else { 0.07 };
                rest.with_rotation(rot).with_translation(rest.translation + rot * Vec3::new(-size, -size * 0.4, 0.0) * jolt)
            }
            Role::Barrel => {
                let e = an.elev + 0.12 * jolt;
                let rot = Quat::from_rotation_z(-(FRAC_PI_2 - e));
                rest.with_rotation(rot).with_translation(rest.translation - rot * Vec3::Y * 0.08 * jolt)
            }
            Role::Carriage => rest.with_translation(rest.translation - Vec3::Z * rec),
            Role::Wheel(r) => rest.with_rotation(Quat::from_rotation_x(-rec / r)),
            Role::Arm(s) => rest.with_rotation(bow::arm_rot(bp.as_ref().map_or(bow::ARM_COCKED, |b| b.arm), s)),
            Role::Claw => rest.with_translation(Vec3::new(bp.as_ref().map_or(bow::CLAW_BACK, |b| b.claw), bow::CLAW_Y, 0.0)),
            Role::Nock => rest.with_translation(bp.as_ref().map_or(rest.translation, |b| b.nock)),
            Role::Bolt => match bp.as_ref().and_then(|b| b.bolt) {
                Some(off) => rest.with_translation(rest.translation + off),
                None => rest.with_scale(Vec3::ZERO),
            },
            Role::Windlass => rest.with_rotation(Quat::from_rotation_z(-bp.as_ref().map_or(1.0, |b| b.wound) * 5.0 * PI)),
            Role::Archer(off) => {
                let lean = kick(t - (off + 0.1) * 0.6, 0.05, 6.0);
                let turn = wrap(an.aim + off).clamp(-1.5, 1.5);
                rest.with_rotation(Quat::from_rotation_y(turn) * Quat::from_rotation_z(0.14 * lean))
                    .with_translation(rest.translation - Vec3::Y * 0.02 * lean)
            }
            Role::RamLog => {
                let l = wm::RAM_CHAIN;
                rest.with_translation(rest.translation + Vec3::new(l * an.swing.sin(), l * (1.0 - an.swing.cos()), 0.0))
            }
            Role::RamChain => rest.with_rotation(Quat::from_rotation_z(an.swing)),
            Role::Feeder => rest.with_rotation(Quat::from_rotation_z((an.shots as f32 + smooth(t / 0.3)) * FRAC_PI_2)),
            Role::SlidingMine => slide_mine(t, an.shots, rest),
            Role::Flap => {
                let open = smooth((t - 0.32) / 0.08) * (1.0 - smooth((t - 0.55) / 0.35));
                rest.with_rotation(Quat::from_rotation_z(-1.1 * open))
            }
            Role::Rope(..) => rest,
        };
        an.pose[i] = tf;
    }
    // Ropes last, once the parts they hang from are in place.
    let parents: Vec<Option<usize>> = rig.parts.iter().map(|p| p.parent).collect();
    for (i, part) in rig.parts.iter().enumerate() {
        if let Role::Rope(a, b) = part.role {
            an.pose[i] = wm::rope_pose(&parents, &an.pose, part.parent, a, b);
        }
    }
}

/// A mine slides down the chute, pushes the flap open and drops off the end.
fn slide_mine(t: f32, shots: u32, rest: Transform) -> Transform {
    let (start, dur) = (0.08, 0.36);
    if shots == 0 || t < start || t > start + dur + 0.2 {
        return rest.with_scale(Vec3::ZERO);
    }
    let u = ((t - start) / dur).clamp(0.0, 1.0);
    let s = u * u;
    let pos = wm::CHUTE_TOP.lerp(wm::CHUTE_END, s);
    let mut tf = rest.with_translation(pos).with_scale(Vec3::ONE);
    if u >= 1.0 {
        let f = t - start - dur;
        let dir = (wm::CHUTE_END - wm::CHUTE_TOP).normalize();
        let speed = 2.0 * wm::CHUTE_TOP.distance(wm::CHUTE_END) / dur;
        tf.translation = wm::CHUTE_END + dir * speed * f - Vec3::Y * 4.9 * f * f;
        tf.rotation = Quat::from_rotation_z(wm::CHUTE_SLOPE + f * 6.0);
    }
    tf
}
