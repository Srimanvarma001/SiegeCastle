//! Enemy troops (marching in formations). The five bosses share the
//! `Enemy` component but move and fight in `boss.rs` and `boss_ai.rs`.

use crate::art::{p2, w3, yaw};
use crate::assets::GameAssets;
use crate::combat::{Payload, spawn_arc};
use crate::grid::Grid;
use crate::items::{Loadout, ModuleKind, Stats, level_mult};
use crate::player::Castle;
use crate::{GameEntity, MAP_HALF, Sim};
use bevy::prelude::*;

pub use crate::boss::Boss;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(crate::GameState::Starting), spawn_pointer)
            .add_systems(Update, boss_pointer.in_set(Sim::Resolve))
            .add_systems(Update, enemy_move.in_set(Sim::Move))
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
            // Bosses: `radius` is the thickness of their capsule-shaped body.
            EnemyKind::SiegeTower => (1400.0, 3.4, 10.0, 14.0, 50),
            EnemyKind::WarWagon => (800.0, 8.5, 6.0, 12.0, 35),
            EnemyKind::Juggernaut => (2600.0, 5.0, 9.0, 20.0, 70),
            EnemyKind::Trebuchet => (3000.0, 3.0, 14.0, 14.0, 80),
            EnemyKind::Warlord => (7500.0, 3.0, 22.0, 24.0, 200),
        };
        EnemyStats { hp, speed, radius, dps, coins }
    }

    /// Half the length of a boss's body beyond its round ends (zero for troops).
    pub fn half_len(self) -> f32 {
        match self {
            EnemyKind::WarWagon => 7.0,
            EnemyKind::Juggernaut => 11.0,
            EnemyKind::Trebuchet => 3.0,
            EnemyKind::Warlord => 10.0,
            _ => 0.0,
        }
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
            EnemyKind::WarWagon => "The War Wagon",
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
    /// Bosses are capsules: a segment this long each way along their facing.
    pub half_len: f32,
    pub dps: f32,
    pub coins: u32,
    pub dmg_mult: f32,
    pub touching: bool,
    pub flash: f32,
    pub(crate) flashing: bool,
    /// Current movement, used to aim shots ahead of it.
    pub vel: Vec2,
    /// Push from rams and explosions. Fades quickly.
    pub knock: Vec2,
    /// Place in its marching formation, relative to the target.
    pub slot: Vec2,
    pub attack: f32,
}

impl Enemy {
    pub fn hit(&mut self, dmg: f32) {
        self.hp -= dmg;
        self.flash = 0.09;
    }

    /// Half of the body's center line on the ground plane (zero for troops).
    pub fn axis(&self, tf: &Transform) -> Vec2 {
        if self.half_len == 0.0 {
            return Vec2::ZERO;
        }
        let f = tf.rotation * Vec3::X;
        Vec2::new(f.x, -f.z).normalize_or(Vec2::X) * self.half_len
    }
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
    spawn_numbered(commands, assets, kind, pos, heading, slot, s, 0)
}

/// Spawns a boss. `index` tells twin bosses apart.
pub fn spawn_boss(commands: &mut Commands, assets: &GameAssets, kind: EnemyKind, pos: Vec2, heading: f32, s: Strength, index: usize) -> Entity {
    spawn_numbered(commands, assets, kind, pos, heading, Vec2::ZERO, s, index)
}

fn spawn_numbered(
    commands: &mut Commands,
    assets: &GameAssets,
    kind: EnemyKind,
    pos: Vec2,
    heading: f32,
    slot: Vec2,
    s: Strength,
    index: usize,
) -> Entity {
    let st = kind.stats();
    let hp = st.hp * s.hp;
    let mut e = commands.spawn((
        GameEntity,
        Enemy {
            kind,
            hp,
            max_hp: hp,
            speed: st.speed,
            radius: st.radius,
            half_len: kind.half_len(),
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
        },
        Transform::from_translation(w3(pos, 0.0)).with_rotation(yaw(heading)),
        Visibility::Visible,
    ));
    if kind.is_boss() {
        // Bosses are rigs of many moving, textured parts.
        let id = e.id();
        crate::boss::attach_boss(commands, assets, kind, id, heading, index);
        return id;
    }
    e.insert((Mesh3d(assets.enemies[&kind].clone()), MeshMaterial3d(assets.m.vc.clone())));
    e.id()
}

/// What enemies need to know about the castle this frame.
#[derive(Clone, Copy)]
pub(crate) struct CastleInfo {
    pos: Vec2,
    vel: Vec2,
    radius: f32,
    pub speed: f32,
    pub heading: Vec2,
    pub ram_hit: f32,
    knock: f32,
}

pub(crate) fn castle_info(tf: &Transform, c: &Castle, stats: &Stats, loadout: &Loadout) -> CastleInfo {
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
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, &mut MeshMaterial3d<StandardMaterial>), Without<Boss>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let ci = castle_info(ctf, c, &stats, &loadout);
    let dt = time.delta_secs();
    let (flash_mat, base_mat) = (&assets.m.flash, &assets.m.vc);
    let knock_decay = (-5.0 * dt).exp();

    enemies.par_iter_mut().for_each(|(entity, mut tf, mut en, mut mat)| {
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
            // Bosses are long: push away from the nearest point of their body.
            let d = pos - o.closest(pos);
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
        tf.translation = w3(next, 0.0);
        let face = if en.vel.length_squared() > 0.01 { en.vel } else { to };
        tf.rotation = yaw(face.to_angle());

        en.flash = (en.flash - dt).max(0.0);
        let want = en.flash > 0.0;
        if want != en.flashing {
            en.flashing = want;
            mat.0 = if want { flash_mat.clone() } else { base_mat.clone() };
        }
    });
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
        if en.attack > 0.0 || pos.distance(cpos) > 21.0 {
            continue;
        }
        en.attack = 2.8;
        let dur = 1.1;
        let to = cpos + c.vel * dur;
        spawn_arc(&mut commands, &assets, w3(pos, 1.3), to, dur, 4.0, Payload::EnemyArrow { dmg: 3.0 * en.dmg_mult });
    }
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
    bosses: Query<(&Transform, &Enemy), (With<Boss>, Without<BossPointer>)>,
    mut pointer: Query<(&mut Transform, &mut Visibility), With<BossPointer>>,
) {
    let (Ok((ctf, c)), Ok((mut tf, mut vis))) = (castle.single(), pointer.single_mut()) else {
        return;
    };
    let cpos = p2(ctf.translation);
    // Distance to each boss's outline, not its center: they are huge.
    let nearest = bosses
        .iter()
        .map(|(t, e)| {
            let p = p2(t.translation);
            let axis = e.axis(t);
            let q = crate::grid::closest_on_segment(p - axis, p + axis, cpos);
            (p, q.distance(cpos) - e.radius)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match nearest {
        Some((b, gap)) if gap > c.radius + 40.0 => {
            let dir = (b - cpos).normalize_or(Vec2::X);
            let bob = (time.elapsed_secs() * 6.0).sin() * 0.4;
            *tf = Transform::from_translation(w3(cpos + dir * (c.radius + 4.0 + bob), 1.0)).with_rotation(yaw(dir.to_angle()));
            *vis = Visibility::Visible;
        }
        _ => *vis = Visibility::Hidden,
    }
}
