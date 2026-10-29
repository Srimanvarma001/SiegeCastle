//! Enemy troops (marching in formations) and the five bosses.

use crate::art::{p2, w3, yaw};
use crate::assets::GameAssets;
use crate::combat::{Bullet, Payload, spawn_arc, spawn_fx, FxKind};
use crate::director::Director;
use crate::grid::Grid;
use crate::items::{Loadout, ModuleKind, Stats, level_mult};
use crate::player::Castle;
use crate::{GameEntity, GameRng, MAP_HALF, Shake, Sim};
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{PI, TAU};

/// Archers shoot at the castle from this far away.
const ARCHER_RANGE: f32 = 21.0;
/// At most this many fallen troops lie around at once.
pub const MAX_CORPSES: usize = 160;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(crate::GameState::Starting), spawn_pointer)
            .add_systems(Update, (boss_pointer, corpses).in_set(Sim::Resolve))
            .add_systems(Update, (enemy_move, boss_ai).in_set(Sim::Move))
            .add_systems(Update, archer_fire.in_set(Sim::Combat));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Footman,
    Archer,
    Knight,
    Cart,
    SiegeTower,
    WarWagon,
    Juggernaut,
    Trebuchet,
    Warlord,
}

pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub radius: f32,
    /// Damage per second while pressed against the castle.
    pub dps: f32,
    /// Coins dropped.
    pub coins: u32,
}

impl EnemyKind {

    pub fn stats(self) -> EnemyStats {
        let (hp, speed, radius, dps, coins) = match self {
            EnemyKind::Footman => (12.0, 5.0, 0.55, 4.0, 1),
            EnemyKind::Archer => (9.0, 5.2, 0.55, 3.0, 1),
            EnemyKind::Knight => (45.0, 9.5, 0.9, 8.0, 3),
            EnemyKind::Cart => (140.0, 6.5, 1.6, 14.0, 6),
            EnemyKind::SiegeTower => (700.0, 3.2, 3.2, 20.0, 40),
            EnemyKind::WarWagon => (420.0, 6.0, 2.6, 15.0, 30),
            EnemyKind::Juggernaut => (1300.0, 4.0, 4.6, 30.0, 60),
            EnemyKind::Trebuchet => (1500.0, 3.0, 4.6, 20.0, 70),
            EnemyKind::Warlord => (3600.0, 3.4, 8.5, 45.0, 150),
        };
        EnemyStats { hp, speed, radius, dps, coins }
    }

    pub fn is_boss(self) -> bool {
        matches!(
            self,
            EnemyKind::SiegeTower | EnemyKind::WarWagon | EnemyKind::Juggernaut | EnemyKind::Trebuchet | EnemyKind::Warlord
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Footman => "Footman",
            EnemyKind::Archer => "Archer",
            EnemyKind::Knight => "Knight",
            EnemyKind::Cart => "War Cart",
            EnemyKind::SiegeTower => "The Siege Tower",
            EnemyKind::WarWagon => "War Wagon",
            EnemyKind::Juggernaut => "Iron Juggernaut",
            EnemyKind::Trebuchet => "Trebuchet Fortress",
            EnemyKind::Warlord => "The Warlord's Citadel",
        }
    }
}

/// Multipliers that grow over the run.
#[derive(Clone, Copy, Debug)]
pub struct Strength {
    pub hp: f32,
    pub dmg: f32,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub radius: f32,
    pub dps: f32,
    pub coins: u32,
    pub dmg_mult: f32,
    pub touching: bool,
    pub flash: f32,
    flashing: bool,
    /// Current movement, used to aim shots ahead of it.
    pub vel: Vec2,
    /// Push from rams and explosions. Fades quickly.
    pub knock: Vec2,
    /// Place in its marching formation, relative to the target.
    pub slot: Vec2,
    pub attack: f32,
    /// Walk cycle phase, advanced by the distance covered.
    stride: f32,
    /// Heading the model shows; turns smoothly toward where it is going.
    facing: f32,
    /// Smoothed speed compared to top speed (0 = standing).
    pace: f32,
    /// Archers: showing the drawn-bow model.
    aiming: bool,
}

impl Enemy {
    pub fn hit(&mut self, dmg: f32) {
        self.hp -= dmg;
        self.flash = 0.09;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossState {
    Approach,
    Windup,
    Charging,
}

#[derive(Component)]
pub struct Boss {
    pub state: BossState,
    pub timer: f32,
    pub timer2: f32,
    pub dir: Vec2,
    pub telegraph: Option<Entity>,
}

pub fn spawn_enemy(
    commands: &mut Commands,
    assets: &GameAssets,
    kind: EnemyKind,
    pos: Vec2,
    heading: f32,
    slot: Vec2,
    s: Strength,
) -> Entity {
    let st = kind.stats();
    let hp = st.hp * s.hp;
    // Cheap per-troop randomness from the spawn spot: look and walk phase.
    let pick = ((pos.x * 12.9898 + pos.y * 78.233).sin() * 43_758.547).fract().abs();
    let mut e = commands.spawn((
        GameEntity,
        Enemy {
            kind,
            hp,
            max_hp: hp,
            speed: st.speed,
            radius: st.radius,
            dps: st.dps,
            coins: st.coins,
            dmg_mult: s.dmg,
            touching: false,
            flash: 0.0,
            flashing: false,
            vel: Vec2::ZERO,
            knock: Vec2::ZERO,
            slot,
            attack: 1.0 + (pos.x.abs() % 1.5),
            stride: pick * TAU,
            facing: heading,
            pace: 0.0,
            aiming: false,
        },
        Transform::from_translation(w3(pos, 0.0)).with_rotation(yaw(heading)),
        Visibility::Visible,
    ));
    if kind.is_boss() {
        // Bosses are textured models made of several pieces.
        e.insert(Boss { state: BossState::Approach, timer: 3.0, timer2: 2.0, dir: Vec2::X, telegraph: None });
        let model = assets.bosses[&kind].clone();
        e.with_children(|p| model.spawn_parts(p, ()));
    } else {
        let looks = &assets.enemies[&kind];
        let mesh = looks[(pick * looks.len() as f32) as usize % looks.len()].clone();
        e.insert((Mesh3d(mesh), MeshMaterial3d(assets.m.vc.clone())));
    }
    e.id()
}

/// What enemies need to know about the castle this frame.
#[derive(Clone, Copy)]
struct CastleInfo {
    pos: Vec2,
    vel: Vec2,
    radius: f32,
    speed: f32,
    heading: Vec2,
    ram_hit: f32,
    knock: f32,
}

fn castle_info(tf: &Transform, c: &Castle, stats: &Stats, loadout: &Loadout) -> CastleInfo {
    let speed = c.speed.max(0.0);
    let ram = loadout.module(ModuleKind::Ram);
    let base = match ram {
        Some(m) => ModuleKind::Ram.damage() * level_mult(m.level),
        None => 5.0,
    };
    let charge = if c.charge > 0.0 { 3.0 } else { 1.0 };
    CastleInfo {
        pos: p2(tf.translation),
        vel: c.vel,
        radius: stats.radius,
        speed,
        heading: Vec2::from_angle(c.heading),
        ram_hit: base * stats.damage * (speed / 12.0) * charge,
        knock: (4.0 + speed * 0.9) * (0.7 + 0.3 * stats.mass.min(4.0)) * if ram.is_some() { 1.4 } else { 1.0 },
    }
}

/// Troops march toward the castle in formation, then swarm it.
/// Runs in parallel on all CPU cores.
fn enemy_move(
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    castle: Query<(&Transform, &Castle), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, &mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>), Without<Boss>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let ci = castle_info(ctf, c, &stats, &loadout);
    let dt = time.delta_secs();
    let (flash_mat, base_mat) = (&assets.m.flash, &assets.m.vc);
    let (archer_walk, archer_aim) = (&assets.enemies[&EnemyKind::Archer][0], &assets.archer_aim);
    let knock_decay = (-5.0 * dt).exp();
    let turn_rate = 1.0 - (-10.0 * dt).exp();
    let pace_rate = 1.0 - (-8.0 * dt).exp();

    enemies.par_iter_mut().for_each(|(entity, mut tf, mut en, mut mesh, mut mat)| {
        let pos = p2(tf.translation);
        let to = ci.pos - pos;
        let dist = to.length().max(0.001);

        // Keep formation while far away, swarm when close.
        let keep = ((dist - 14.0) / 26.0).clamp(0.0, 1.0);
        let target = ci.pos + en.slot * keep;
        let mut dir = (target - pos).normalize_or(to / dist);
        if en.kind == EnemyKind::Archer {
            if dist < 13.0 {
                dir = -to / dist;
            } else if dist < 19.0 {
                dir = Vec2::ZERO;
            }
        }
        en.vel = dir * en.speed;

        let mut push = Vec2::ZERO;
        grid.for_each_near(pos, en.radius + 2.0, |o| {
            if o.entity == entity {
                return;
            }
            let d = pos - o.pos;
            let min = en.radius + o.radius;
            let l2 = d.length_squared();
            if l2 < min * min && l2 > 0.0001 {
                let l = l2.sqrt();
                push += d / l * (min - l) * (o.radius / (en.radius + o.radius));
            }
        });
        let push = push.clamp_length_max(en.speed * dt * 1.5 + 0.08);

        let mut next = pos + en.vel * dt + en.knock * dt + push;
        en.knock *= knock_decay;

        let contact = ci.radius + en.radius;
        let from = next - ci.pos;
        let was_touching = en.touching;
        en.touching = false;
        if from.length_squared() < contact * contact {
            let away = from.normalize_or(-to / dist);
            next = ci.pos + away * contact;
            en.touching = true;
            // Ram hit when the castle drives into it.
            let front = away.dot(ci.heading);
            if !was_touching && ci.speed > 3.0 && front > 0.0 {
                en.hp -= ci.ram_hit * (0.5 + front);
                en.flash = 0.09;
                en.knock = (away * 0.7 + ci.heading * 0.6).normalize_or(away) * ci.knock + ci.vel * 0.3;
            }
        }
        let lim = MAP_HALF - 2.0;
        next = next.clamp(Vec2::splat(-lim), Vec2::splat(lim));
        let moved = next.distance(pos);

        // Archers in range stand and draw their bows, facing the castle.
        let aim = en.kind == EnemyKind::Archer && dist < ARCHER_RANGE;
        if aim != en.aiming {
            en.aiming = aim;
            mesh.0 = if aim { archer_aim.clone() } else { archer_walk.clone() };
        }
        let want = if aim || en.touching || en.vel.length_squared() < 0.01 { to } else { en.vel };
        let turn = (want.to_angle() - en.facing + PI).rem_euclid(TAU) - PI;
        en.facing += turn * turn_rate;
        let pace = (moved / dt.max(1e-4) / en.speed).min(1.0);
        en.pace += (pace - en.pace) * pace_rate;
        let (lift, tilt) = animate(&mut en, moved, dt);
        tf.translation = w3(next, lift);
        tf.rotation = yaw(en.facing) * tilt;

        en.flash = (en.flash - dt).max(0.0);
        let want = en.flash > 0.0;
        if want != en.flashing {
            en.flashing = want;
            mat.0 = if want { flash_mat.clone() } else { base_mat.clone() };
        }
        // Squash a little when hit.
        let k = (en.flash / 0.09).clamp(0.0, 1.0);
        tf.scale = Vec3::new(1.0 + 0.12 * k, 1.0 - 0.14 * k, 1.0 + 0.12 * k);
    });
}

/// Walk, gallop and rumble cycles, and the hacking motion against the walls.
/// Only moves the whole model (feet stay the pivot), so it costs next to nothing.
/// Returns the height to lift the model and the tilt to apply after its heading.
fn animate(en: &mut Enemy, moved: f32, dt: f32) -> (f32, Quat) {
    let p = en.pace;
    // (lift, lean forward, roll sideways)
    let (lift, lean, roll) = if en.touching {
        en.stride += dt * if en.kind == EnemyKind::Knight { 6.0 } else { 8.0 };
        let hit = en.stride.sin().max(0.0);
        match en.kind {
            // The horse rears up and stamps.
            EnemyKind::Knight => (0.1 * hit, -0.18 * hit, 0.0),
            EnemyKind::Cart => (0.0, 0.05 * hit, 0.0),
            // Lunging and hacking at the walls.
            _ => (0.0, 0.25 * hit, 0.0),
        }
    } else {
        match en.kind {
            EnemyKind::Knight => {
                en.stride += moved * 1.6;
                (0.14 * (0.5 + 0.5 * en.stride.sin()) * p, 0.09 * en.stride.cos() * p, 0.0)
            }
            EnemyKind::Cart => {
                en.stride += moved * 4.0;
                (0.025 * en.stride.sin().abs() * p, 0.0, 0.03 * (en.stride * 0.37).sin() * p)
            }
            _ => {
                en.stride += moved * 1.45;
                (0.08 * en.stride.sin().abs() * p, 0.09 * p, 0.07 * en.stride.sin() * p)
            }
        }
    };
    // Wrap the phase without a jump (0.37 * 100 is a whole number of turns).
    en.stride = en.stride.rem_euclid(TAU * 100.0);
    (lift, Quat::from_rotation_z(-lean) * Quat::from_rotation_x(roll))
}

/// A fallen troop: topples over, lies still for a moment, then sinks away.
#[derive(Component)]
pub struct Corpse {
    age: f32,
    /// Axis it falls around and how far it tips over.
    axis: Vec3,
    tip: f32,
    /// Raised a little as it lies down, so it rests on the ground instead of in it.
    lift: f32,
    /// How deep it sinks before it is gone.
    depth: f32,
    /// Slide from the blow that killed it.
    slide: Vec3,
    upright: Quat,
}

/// Leaves the body of a dead troop behind, in the same spot and pose.
pub fn spawn_corpse(commands: &mut Commands, assets: &GameAssets, en: &Enemy, tf: &Transform, mesh: &Mesh3d) {
    // Knocked flat by the blow, or falling backward and a bit to one side.
    let back = Vec2::from_angle(en.facing + PI + en.stride.sin() * 0.9);
    let fall = if en.knock.length_squared() > 4.0 { en.knock.normalize() } else { back };
    let (tip, lift, depth, mat) = match en.kind {
        EnemyKind::Cart => (0.22, 0.0, 2.6, assets.m.charred.clone()),
        EnemyKind::Knight => (1.35, 0.32, 1.4, assets.m.vc.clone()),
        _ => (1.45, 0.22, 1.1, assets.m.vc.clone()),
    };
    commands.spawn((
        GameEntity,
        Corpse {
            age: 0.0,
            axis: Vec3::Y.cross(w3(fall, 0.0)).normalize_or(Vec3::Z),
            tip,
            lift,
            depth,
            slide: w3(en.knock * 0.35 + fall * 1.2, 0.0),
            upright: yaw(en.facing),
        },
        mesh.clone(),
        MeshMaterial3d(mat),
        Transform::from_translation(tf.translation.with_y(0.0)).with_rotation(yaw(en.facing)),
    ));
}

fn corpses(mut commands: Commands, time: Res<Time>, mut bodies: Query<(Entity, &mut Transform, &mut Corpse)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut c) in &mut bodies {
        c.age += dt;
        // Falls like a plank, slow at first and then faster, with a small bounce.
        let f = (c.age / 0.42).min(1.0);
        let settle = (c.age - 0.42).max(0.0);
        let bounce = if settle > 0.0 { (settle * 22.0).sin().abs() * (-settle * 9.0).exp() * 0.12 } else { 0.0 };
        tf.rotation = Quat::from_axis_angle(c.axis, c.tip * f * f - bounce) * c.upright;
        let slide = c.slide;
        tf.translation += slide * dt;
        c.slide *= (-5.0 * dt).exp();
        let sink = ((c.age - 1.1) / 0.9).clamp(0.0, 1.0);
        tf.translation.y = c.lift * f - sink * sink * c.depth;
        if c.age > 2.0 {
            commands.entity(e).despawn();
        }
    }
}

fn archer_fire(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    castle: Query<(&Transform, &Castle), Without<Enemy>>,
    mut archers: Query<(&Transform, &mut Enemy), Without<Boss>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    for (tf, mut en) in &mut archers {
        if en.kind != EnemyKind::Archer {
            continue;
        }
        en.attack -= dt;
        let pos = p2(tf.translation);
        if en.attack > 0.0 || pos.distance(cpos) > ARCHER_RANGE {
            continue;
        }
        en.attack = 2.8;
        let dur = 1.1;
        let to = cpos + c.vel * dur;
        spawn_arc(&mut commands, &assets, w3(pos, 1.3), to, dur, 4.0, Payload::EnemyArrow { dmg: 3.0 * en.dmg_mult });
    }
}

/// Each boss has its own way of fighting.
fn boss_ai(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    director: Res<Director>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    mut castle: Query<(&Transform, &mut Castle), Without<Enemy>>,
    mut bosses: Query<(&mut Transform, &mut Enemy, &mut Boss)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    let crad = c.radius;
    let ci = castle_info(ctf, &c, &stats, &loadout);

    for (mut tf, mut en, mut boss) in &mut bosses {
        let pos = p2(tf.translation);
        let to = cpos - pos;
        let dist = to.length().max(0.01);
        let dir = to / dist;
        boss.timer -= dt;
        boss.timer2 -= dt;
        en.attack -= dt;
        let mut vel = dir * en.speed;
        let mut face = dir;

        match en.kind {
            EnemyKind::SiegeTower => {
                // Unloads a squad of footmen every few seconds.
                if boss.timer <= 0.0 && dist < 60.0 {
                    boss.timer = 8.0;
                    for i in 0..6 {
                        let off = Vec2::new((i % 4) as f32 * 1.3 - 2.0, (i / 4) as f32 * 1.3 - 0.6);
                        let p = pos + dir * (en.radius + 2.0) + off;
                        spawn_enemy(&mut commands, &assets, EnemyKind::Footman, p, dir.to_angle(), Vec2::ZERO, director.strength);
                    }
                    spawn_fx(&mut commands, &assets, FxKind::Smoke, w3(pos + dir * en.radius, 1.5), 2.0, Vec3::Y);
                }
            }
            EnemyKind::WarWagon => {
                // Keeps its distance and fires its cannon.
                let tangent = Vec2::new(-dir.y, dir.x);
                vel = if dist > 24.0 {
                    dir * en.speed
                } else if dist < 15.0 {
                    -dir * en.speed
                } else {
                    tangent * en.speed * 0.8
                };
                if boss.timer <= 0.0 && dist < 34.0 {
                    boss.timer = 2.2;
                    let lead = cpos + c.vel * (dist / 26.0);
                    let aim = (lead - pos).normalize_or(dir);
                    let muzzle = pos + aim * (en.radius + 1.0);
                    fire_ball(&mut commands, &assets, muzzle, aim * 26.0, 10.0 * en.dmg_mult);
                }
            }
            EnemyKind::Juggernaut => charge_ai(&mut commands, &assets, &mut boss, &mut en, pos, dir, dist, 34.0, 30.0, &mut vel, &mut face),
            EnemyKind::Trebuchet => {
                // Stays far back and lobs boulders where the castle is going.
                vel = if dist > 46.0 {
                    dir * en.speed
                } else if dist < 30.0 {
                    -dir * en.speed
                } else {
                    Vec2::ZERO
                };
                if boss.timer <= 0.0 && dist < 60.0 {
                    boss.timer = 4.0;
                    for _ in 0..3 {
                        let dur = 2.0;
                        let off = Vec2::new(rng.0.random_range(-6.0..6.0), rng.0.random_range(-6.0..6.0));
                        let to = cpos + c.vel * dur + off;
                        spawn_arc(&mut commands, &assets, w3(pos, 8.0), to, dur, 16.0,
                            Payload::Boulder { dmg: 22.0 * en.dmg_mult, radius: 4.0 });
                        spawn_fx(&mut commands, &assets, FxKind::Telegraph { dur }, w3(to, 0.12), 8.0, Vec3::ZERO);
                    }
                }
            }
            EnemyKind::Warlord => {
                if boss.timer2 <= 0.0 && dist < 45.0 {
                    boss.timer2 = 3.0;
                    for i in -2..=2 {
                        let aim = Vec2::from_angle(dir.to_angle() + i as f32 * 0.16);
                        fire_ball(&mut commands, &assets, pos + aim * (en.radius + 1.0), aim * 24.0, 14.0 * en.dmg_mult);
                    }
                }
                if boss.state == BossState::Approach && boss.timer <= 0.0 && dist < 60.0 && rng.0.random::<f32>() < 0.4 {
                    boss.timer = 12.0;
                    for i in 0..5 {
                        let p = pos - dir * (en.radius + 2.0) + Vec2::new(0.0, i as f32 * 2.0 - 4.0);
                        spawn_enemy(&mut commands, &assets, EnemyKind::Knight, p, dir.to_angle(), Vec2::ZERO, director.strength);
                    }
                } else {
                    charge_ai(&mut commands, &assets, &mut boss, &mut en, pos, dir, dist, 40.0, 16.0, &mut vel, &mut face);
                }
            }
            _ => {}
        }

        // Move, but never drive into the castle.
        let mut next = pos + vel * dt + en.knock * dt;
        en.knock *= (-3.0 * dt).exp();
        let contact = crad + en.radius;
        let from = next - cpos;
        en.touching = false;
        if from.length_squared() < contact * contact {
            let away = from.normalize_or(-dir);
            next = cpos + away * contact;
            en.touching = true;
            // Ramming hurts bosses too, at most twice a second.
            if ci.speed > 3.0 && away.dot(ci.heading) > 0.0 && en.attack <= 0.0 {
                en.hit(ci.ram_hit * 2.0);
                en.attack = 0.5;
                shake.0 += 0.3;
            }
            if boss.state == BossState::Charging {
                c.hurt(30.0 * en.dmg_mult);
                c.knock += boss.dir * 18.0;
                shake.0 += 1.2;
                boss.state = BossState::Approach;
                boss.timer = 5.0;
                if let Some(t) = boss.telegraph.take() {
                    commands.entity(t).despawn();
                }
            }
        }
        let lim = MAP_HALF - en.radius;
        next = next.clamp(Vec2::splat(-lim), Vec2::splat(lim));
        tf.translation = w3(next, 0.0);
        if vel.length_squared() > 0.01 || en.kind == EnemyKind::WarWagon {
            let want = if en.kind == EnemyKind::WarWagon { dir.to_angle() } else { face.to_angle() };
            let cur = tf.rotation.to_euler(EulerRot::YXZ).0;
            let diff = (want - cur + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            tf.rotation = yaw(cur + diff * (1.0 - (-4.0 * dt).exp()));
        }
        en.flash = (en.flash - dt).max(0.0);
        let want = en.flash > 0.0;
        if want && !en.flashing {
            // Bosses throw sparks when hit instead of flashing.
            spawn_fx(&mut commands, &assets, FxKind::Spark, w3(pos, 2.0 + en.radius * 0.4), 0.8 + en.radius * 0.15, Vec3::ZERO);
        }
        en.flashing = want;
    }
}

/// Wind up (with a red warning line), then charge in a straight line.
fn charge_ai(
    commands: &mut Commands,
    assets: &GameAssets,
    boss: &mut Boss,
    en: &mut Enemy,
    pos: Vec2,
    dir: Vec2,
    dist: f32,
    trigger: f32,
    charge_speed: f32,
    vel: &mut Vec2,
    face: &mut Vec2,
) {
    match boss.state {
        BossState::Approach => {
            if boss.timer <= 0.0 && dist < trigger {
                boss.state = BossState::Windup;
                boss.timer = 1.3;
                boss.dir = dir;
                let len = trigger + 12.0;
                let mid = pos + dir * (len / 2.0);
                let t = commands
                    .spawn((
                        GameEntity,
                        Mesh3d(assets.cube.clone()),
                        MeshMaterial3d(assets.m.telegraph.clone()),
                        Transform::from_translation(w3(mid, 0.1))
                            .with_rotation(yaw(dir.to_angle()))
                            .with_scale(Vec3::new(len, 0.05, en.radius * 1.2)),
                        bevy::light::NotShadowCaster,
                    ))
                    .id();
                boss.telegraph = Some(t);
            }
        }
        BossState::Windup => {
            *vel = Vec2::ZERO;
            *face = boss.dir;
            if boss.timer <= 0.0 {
                boss.state = BossState::Charging;
                boss.timer = 1.6;
            }
        }
        BossState::Charging => {
            *vel = boss.dir * charge_speed;
            *face = boss.dir;
            if boss.timer <= 0.0 {
                boss.state = BossState::Approach;
                boss.timer = 5.0;
                if let Some(t) = boss.telegraph.take() {
                    commands.entity(t).despawn();
                }
            }
        }
    }
}

fn fire_ball(commands: &mut Commands, assets: &GameAssets, from: Vec2, vel: Vec2, dmg: f32) {
    commands.spawn((
        GameEntity,
        Bullet::hostile(vel, dmg, 0.5, 3.0),
        Mesh3d(assets.ball.clone()),
        MeshMaterial3d(assets.m.cannonball.clone()),
        Transform::from_translation(w3(from, 1.8)).with_scale(Vec3::splat(1.0)),
    ));
    spawn_fx(commands, assets, FxKind::Smoke, w3(from, 2.0), 1.4, Vec3::new(0.0, 1.0, 0.0));
}

/// Red arrow next to the castle pointing at the nearest boss.
#[derive(Component)]
struct BossPointer;

fn spawn_pointer(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        GameEntity,
        BossPointer,
        Mesh3d(assets.pointer.clone()),
        MeshMaterial3d(assets.m.telegraph.clone()),
        Transform::default(),
        Visibility::Hidden,
        bevy::light::NotShadowCaster,
    ));
}

fn boss_pointer(
    time: Res<Time>,
    castle: Query<(&Transform, &Castle), (Without<BossPointer>, Without<Boss>)>,
    bosses: Query<&Transform, (With<Boss>, Without<BossPointer>)>,
    mut pointer: Query<(&mut Transform, &mut Visibility), With<BossPointer>>,
) {
    let (Ok((ctf, c)), Ok((mut tf, mut vis))) = (castle.single(), pointer.single_mut()) else {
        return;
    };
    let cpos = p2(ctf.translation);
    let nearest = bosses
        .iter()
        .map(|t| p2(t.translation))
        .min_by(|a, b| a.distance_squared(cpos).total_cmp(&b.distance_squared(cpos)));
    match nearest {
        Some(b) if b.distance(cpos) > c.radius + 30.0 => {
            let dir = (b - cpos).normalize_or(Vec2::X);
            let bob = (time.elapsed_secs() * 6.0).sin() * 0.4;
            *tf = Transform::from_translation(w3(cpos + dir * (c.radius + 4.0 + bob), 1.0)).with_rotation(yaw(dir.to_angle()));
            *vis = Visibility::Visible;
        }
        _ => *vis = Visibility::Hidden,
    }
}
