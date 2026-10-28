//! Enemy types, spawning, swarm movement and boss attacks.

use crate::assets::{GameAssets, shape};
use crate::castle::Castle;
use crate::combat::EnemyShot;
use crate::grid::Grid;
use crate::{GameEntity, MAP_HALF, Sim};
use bevy::prelude::*;

pub const Z_ENEMY: f32 = 3.0;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(crate::GameState::Starting), spawn_boss_pointer)
            .add_systems(Update, boss_pointer.in_set(Sim::Resolve))
            .add_systems(Update, enemy_move.in_set(Sim::Move))
            .add_systems(Update, boss_shoot.in_set(Sim::Combat))
            .add_systems(Update, boss_bars.in_set(Sim::Resolve));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Fast and weak. The bulk of every swarm.
    Raider,
    /// Armored footman. Slower, takes more hits.
    Knight,
    /// Battering ram. Slow, tanky, hits the walls hard.
    Ram,
    /// A rival castle on wheels. Mini-boss that fires back.
    RivalCastle,
    /// The final boss: the Warlord's citadel.
    Warlord,
}

pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub radius: f32,
    /// Damage per second while touching the castle.
    pub dps: f32,
    pub xp: u32,
}

impl EnemyKind {
    pub fn stats(self) -> EnemyStats {
        let (hp, speed, radius, dps, xp) = match self {
            EnemyKind::Raider => (22.0, 150.0, 10.0, 4.0, 1),
            EnemyKind::Knight => (70.0, 105.0, 14.0, 8.0, 3),
            EnemyKind::Ram => (260.0, 75.0, 20.0, 20.0, 10),
            EnemyKind::RivalCastle => (3000.0, 62.0, 55.0, 40.0, 80),
            EnemyKind::Warlord => (8000.0, 52.0, 85.0, 70.0, 0),
        };
        EnemyStats { hp, speed, radius, dps, xp }
    }

    pub fn is_boss(self) -> bool {
        matches!(self, EnemyKind::RivalCastle | EnemyKind::Warlord)
    }
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub radius: f32,
    pub dps: f32,
    pub xp: u32,
    /// True while pressed against the castle walls.
    pub touching: bool,
    /// Seconds left of the white "got hit" flash.
    pub flash: f32,
    flashing: bool,
    base_mat: Handle<ColorMaterial>,
}

#[derive(Component)]
pub struct Boss {
    pub shoot_timer: f32,
}

/// Red health bar fill above a boss.
#[derive(Component)]
pub struct BossBarFill {
    width: f32,
}

pub fn spawn_enemy(
    commands: &mut Commands,
    assets: &GameAssets,
    kind: EnemyKind,
    pos: Vec2,
    wave: u32,
) {
    let s = kind.stats();
    let w = wave.saturating_sub(1) as f32;
    let hp = s.hp * (1.0 + 0.15 * w);
    let speed = s.speed * (1.0 + 0.02 * w);
    let m = &assets.m;
    let base_mat = match kind {
        EnemyKind::Raider => m.raider.clone(),
        EnemyKind::Knight => m.knight.clone(),
        EnemyKind::Ram => m.ram.clone(),
        EnemyKind::RivalCastle => m.boss.clone(),
        EnemyKind::Warlord => m.warlord.clone(),
    };
    let enemy = Enemy {
        kind,
        hp,
        max_hp: hp,
        speed,
        radius: s.radius,
        dps: s.dps * (1.0 + 0.05 * w),
        xp: s.xp,
        touching: false,
        flash: 0.0,
        flashing: false,
        base_mat: base_mat.clone(),
    };
    let pos3 = pos.extend(Z_ENEMY);

    match kind {
        EnemyKind::Raider => {
            commands.spawn((
                GameEntity,
                enemy,
                shape(&assets.circle, &base_mat, pos3, Vec2::splat(s.radius * 2.0)),
            ));
        }
        EnemyKind::Knight => {
            commands
                .spawn((
                    GameEntity,
                    enemy,
                    shape(&assets.square, &base_mat, pos3, Vec2::splat(s.radius * 1.8)),
                ))
                .with_child(shape(
                    &assets.square,
                    &m.knight_trim,
                    Vec3::new(0.25, 0.0, 0.1),
                    Vec2::new(0.3, 0.7),
                ));
        }
        EnemyKind::Ram => {
            commands
                .spawn((
                    GameEntity,
                    enemy,
                    shape(&assets.square, &base_mat, pos3, Vec2::new(46.0, 26.0)),
                ))
                .with_child(shape(
                    &assets.square,
                    &m.cannon,
                    Vec3::new(0.55, 0.0, 0.1),
                    Vec2::new(0.25, 0.4),
                ));
        }
        EnemyKind::RivalCastle | EnemyKind::Warlord => {
            let r = s.radius;
            let (roof, tower) = if kind == EnemyKind::Warlord {
                (&m.roof_red, &m.boss)
            } else {
                (&m.boss_roof, &m.warlord)
            };
            let bar_w = r * 2.2;
            commands
                .spawn((
                    GameEntity,
                    enemy,
                    Boss { shoot_timer: 2.0 },
                    Transform::from_translation(pos3),
                    Visibility::Visible,
                ))
                .with_children(|p| {
                    p.spawn(shape(&assets.square, &base_mat, Vec3::ZERO, Vec2::splat(r * 1.7)));
                    p.spawn(shape(&assets.square, roof, Vec3::Z * 0.2, Vec2::splat(r * 0.8)));
                    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        p.spawn(shape(
                            &assets.circle,
                            tower,
                            Vec3::new(x * r * 0.85, y * r * 0.85, 0.3),
                            Vec2::splat(r * 0.6),
                        ));
                    }
                    p.spawn(shape(
                        &assets.square,
                        &m.bar_bg,
                        Vec3::new(0.0, r * 1.25, 0.5),
                        Vec2::new(bar_w + 4.0, 10.0),
                    ));
                    p.spawn((
                        BossBarFill { width: bar_w },
                        shape(
                            &assets.square,
                            &m.bar_fill,
                            Vec3::new(0.0, r * 1.25, 0.6),
                            Vec2::new(bar_w, 6.0),
                        ),
                    ));
                });
        }
    }
}

/// Every enemy walks toward the castle and pushes away from its neighbours.
/// Runs in parallel across all CPU cores.
fn enemy_move(
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    castle: Query<(&Transform, &Castle), Without<Enemy>>,
    mut enemies: Query<(
        Entity,
        &mut Transform,
        &mut Enemy,
        Option<&mut MeshMaterial2d<ColorMaterial>>,
    )>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let cpos = ctf.translation.truncate();
    let crad = c.radius;
    let dt = time.delta_secs();
    let flash_mat = &assets.m.flash;

    enemies
        .par_iter_mut()
        .for_each(|(entity, mut tf, mut en, mat)| {
            let pos = tf.translation.truncate();
            let to = cpos - pos;
            let dist = to.length().max(0.001);
            let dir = to / dist;

            // Push apart from overlapping neighbours. Heavy enemies barely move.
            let mut push = Vec2::ZERO;
            grid.for_each_near(pos, en.radius + 30.0, |o| {
                if o.entity == entity {
                    return;
                }
                let d = pos - o.pos;
                let min = en.radius + o.radius;
                let l2 = d.length_squared();
                if l2 < min * min && l2 > 0.0001 {
                    let l = l2.sqrt();
                    let weight = o.radius / (en.radius + o.radius);
                    push += d / l * (min - l) * weight;
                }
            });
            let push = push.clamp_length_max(en.speed * dt * 1.5 + 4.0);

            let mut next = pos + dir * en.speed * dt + push;
            let contact = crad + en.radius;
            en.touching = false;
            let from_castle = next - cpos;
            if from_castle.length_squared() < contact * contact {
                next = cpos + from_castle.normalize_or(-dir) * contact;
                en.touching = true;
            }
            next = next.clamp(Vec2::splat(-MAP_HALF), Vec2::splat(MAP_HALF));
            tf.translation.x = next.x;
            tf.translation.y = next.y;

            if !en.kind.is_boss() {
                tf.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
            }

            // White hit flash: only touch the material when it changes.
            en.flash = (en.flash - dt).max(0.0);
            let want = en.flash > 0.0;
            if want != en.flashing {
                en.flashing = want;
                if let Some(mut m) = mat {
                    m.0 = if want { flash_mat.clone() } else { en.base_mat.clone() };
                }
            }
        });
}

fn boss_shoot(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    castle: Query<&Transform, With<Castle>>,
    mut bosses: Query<(&Transform, &Enemy, &mut Boss)>,
) {
    let Ok(ctf) = castle.single() else {
        return;
    };
    let cpos = ctf.translation.truncate();
    for (tf, enemy, mut boss) in &mut bosses {
        boss.shoot_timer -= time.delta_secs();
        let pos = tf.translation.truncate();
        if boss.shoot_timer > 0.0 || pos.distance(cpos) > 900.0 {
            continue;
        }
        let final_boss = enemy.kind == EnemyKind::Warlord;
        boss.shoot_timer = if final_boss { 1.2 } else { 1.8 };
        let shots = if final_boss { 5 } else { 3 };
        let dmg = if final_boss { 30.0 } else { 22.0 };
        let base = (cpos - pos).to_angle();
        for i in 0..shots {
            let a = base + (i as f32 - (shots - 1) as f32 / 2.0) * 0.18;
            let vel = Vec2::from_angle(a) * 420.0;
            commands.spawn((
                GameEntity,
                EnemyShot { vel, damage: dmg, life: 3.0 },
                shape(
                    &assets.circle,
                    &assets.m.enemy_shot,
                    (pos + vel.normalize() * enemy.radius).extend(6.0),
                    Vec2::splat(16.0),
                ),
            ));
        }
    }
}

fn boss_bars(
    bosses: Query<(&Enemy, &Children), With<Boss>>,
    mut bars: Query<(&mut Transform, &BossBarFill)>,
) {
    for (enemy, children) in &bosses {
        let frac = (enemy.hp / enemy.max_hp).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok((mut tf, bar)) = bars.get_mut(child) {
                tf.scale.x = bar.width * frac;
                tf.translation.x = -bar.width * (1.0 - frac) / 2.0;
            }
        }
    }
}

/// Red arrow at the edge of the castle pointing to the nearest boss.
#[derive(Component)]
struct BossPointer;

fn spawn_boss_pointer(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        GameEntity,
        BossPointer,
        Visibility::Hidden,
        shape(&assets.diamond, &assets.m.bar_fill, Vec3::new(0.0, 0.0, 8.0), Vec2::new(34.0, 16.0)),
    ));
}

fn boss_pointer(
    castle: Query<(&Transform, &Castle), Without<BossPointer>>,
    bosses: Query<&Transform, (With<Boss>, Without<BossPointer>)>,
    mut pointer: Query<(&mut Transform, &mut Visibility), With<BossPointer>>,
) {
    let (Ok((ctf, c)), Ok((mut tf, mut vis))) = (castle.single(), pointer.single_mut()) else {
        return;
    };
    let cpos = ctf.translation.truncate();
    let nearest = bosses
        .iter()
        .map(|t| t.translation.truncate())
        .min_by(|a, b| a.distance_squared(cpos).total_cmp(&b.distance_squared(cpos)));
    match nearest {
        Some(b) if b.distance(cpos) > c.radius + 500.0 => {
            let dir = (b - cpos).normalize_or(Vec2::X);
            let p = cpos + dir * (c.radius + 45.0);
            tf.translation = p.extend(8.0);
            tf.rotation = Quat::from_rotation_z(dir.to_angle());
            *vis = Visibility::Visible;
        }
        _ => *vis = Visibility::Hidden,
    }
}
