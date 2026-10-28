//! Projectiles, hits, explosions, enemy deaths, XP gems and particles.

use crate::assets::{GameAssets, shape};
use crate::castle::Castle;
use crate::enemy::{Boss, Enemy, EnemyKind};
use crate::grid::{Grid, MAX_ENEMY_RADIUS};
use crate::{Banner, GameEntity, GameRng, GameState, RunStats, Shake, Sim};
use bevy::prelude::*;
use rand::Rng;

/// Above this many gems on the ground, XP goes straight to the castle.
const MAX_GEMS: usize = 1200;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (move_projectiles, move_enemy_shots)
                .in_set(Sim::Combat)
                .after(crate::castle::towers_fire),
        )
        .add_systems(
            Update,
            (enemy_deaths, collect_gems, update_particles)
                .chain()
                .in_set(Sim::Resolve)
                .before(crate::castle::castle_growth),
        );
    }
}

#[derive(Component)]
pub struct Projectile {
    pub vel: Vec2,
    pub damage: f32,
    pub radius: f32,
    /// Explosion radius. 0 = single target.
    pub splash: f32,
    /// How many extra enemies it can pass through.
    pub pierce: u32,
    pub life: f32,
    hits: Vec<Entity>,
}

impl Projectile {
    pub fn new(vel: Vec2, damage: f32, radius: f32, splash: f32, pierce: u32, life: f32) -> Self {
        Self { vel, damage, radius, splash, pierce, life, hits: Vec::new() }
    }
}

#[derive(Component)]
pub struct EnemyShot {
    pub vel: Vec2,
    pub damage: f32,
    pub life: f32,
}

#[derive(Component)]
pub struct Gem {
    pub xp: u32,
}

#[derive(Component)]
pub struct Particle {
    vel: Vec2,
    life: f32,
    max_life: f32,
    size: f32,
}

/// Throws `count` particles out from `pos`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_burst(
    commands: &mut Commands,
    assets: &GameAssets,
    mat: &Handle<ColorMaterial>,
    rng: &mut GameRng,
    pos: Vec2,
    count: u32,
    speed: f32,
    size: f32,
    life: f32,
) {
    for _ in 0..count {
        let dir = Vec2::from_angle(rng.0.random::<f32>() * std::f32::consts::TAU);
        let v = dir * speed * (0.3 + rng.0.random::<f32>() * 0.7);
        let l = life * (0.6 + rng.0.random::<f32>() * 0.4);
        let s = size * (0.6 + rng.0.random::<f32>() * 0.6);
        commands.spawn((
            GameEntity,
            Particle { vel: v, life: l, max_life: l, size: s },
            shape(&assets.circle, mat, pos.extend(7.0), Vec2::splat(s)),
        ));
    }
}

fn damage_area(grid: &Grid, enemies: &mut Query<&mut Enemy>, pos: Vec2, radius: f32, dmg: f32) {
    grid.for_each_near(pos, radius + MAX_ENEMY_RADIUS, |g| {
        let reach = radius + g.radius;
        if g.pos.distance_squared(pos) < reach * reach
            && let Ok(mut en) = enemies.get_mut(g.entity)
        {
            en.hp -= dmg;
            en.flash = 0.08;
        }
    });
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    mut enemies: Query<&mut Enemy>,
) {
    let dt = time.delta_secs();
    for (pe, mut tf, mut p) in &mut projectiles {
        let start = tf.translation.truncate();
        let step = p.vel * dt;
        p.life -= dt;

        // Check along the whole path this frame so fast shots never skip
        // over an enemy, even when a frame takes long.
        let substeps = (step.length() / (p.radius + 8.0)).ceil().clamp(1.0, 16.0) as u32;
        let mut hit = None;
        let mut pos = start + step;
        for i in 1..=substeps {
            let at = start + step * (i as f32 / substeps as f32);
            grid.for_each_near(at, p.radius + MAX_ENEMY_RADIUS, |g| {
                if hit.is_some() || p.hits.contains(&g.entity) {
                    return;
                }
                let reach = p.radius + g.radius;
                if g.pos.distance_squared(at) < reach * reach
                    && enemies.get(g.entity).is_ok_and(|e| e.hp > 0.0)
                {
                    hit = Some(g.entity);
                }
            });
            if hit.is_some() {
                pos = at;
                break;
            }
        }
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;

        let expired = p.life <= 0.0;
        if hit.is_none() && !expired {
            continue;
        }

        if p.splash > 0.0 {
            damage_area(&grid, &mut enemies, pos, p.splash, p.damage);
            spawn_burst(&mut commands, &assets, &assets.m.spark, &mut rng, pos, 6, 220.0, 14.0, 0.3);
            commands.entity(pe).despawn();
            continue;
        }
        let Some(target) = hit else {
            commands.entity(pe).despawn();
            continue;
        };
        if let Ok(mut en) = enemies.get_mut(target) {
            en.hp -= p.damage;
            en.flash = 0.08;
        }
        if p.pierce > 0 {
            p.pierce -= 1;
            p.hits.push(target);
        } else {
            commands.entity(pe).despawn();
        }
    }
}

fn move_enemy_shots(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut shots: Query<(Entity, &mut Transform, &mut EnemyShot), Without<Castle>>,
    mut castle: Query<(&Transform, &mut Castle)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = ctf.translation.truncate();
    let dt = time.delta_secs();
    for (e, mut tf, mut s) in &mut shots {
        tf.translation += (s.vel * dt).extend(0.0);
        s.life -= dt;
        let hit = tf.translation.truncate().distance(cpos) < c.radius + 8.0;
        if hit {
            c.hp -= s.damage;
            shake.0 += 4.0;
        }
        if hit || s.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn enemy_deaths(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<RunStats>,
    mut banner: ResMut<Banner>,
    mut shake: ResMut<Shake>,
    mut next: ResMut<NextState<GameState>>,
    enemies: Query<(Entity, &Transform, &Enemy), With<Enemy>>,
    bosses: Query<(), With<Boss>>,
    gems: Query<(), With<Gem>>,
    mut castle: Query<&mut Castle>,
) {
    let Ok(mut c) = castle.single_mut() else {
        return;
    };
    let mut gem_count = gems.iter().count();
    for (e, tf, en) in &enemies {
        if en.hp > 0.0 {
            continue;
        }
        commands.entity(e).despawn();
        stats.kills += 1;
        let pos = tf.translation.truncate();

        if bosses.contains(e) {
            spawn_burst(&mut commands, &assets, &assets.m.smoke, &mut rng, pos, 50, 350.0, 34.0, 1.2);
            spawn_burst(&mut commands, &assets, &assets.m.spark, &mut rng, pos, 40, 500.0, 18.0, 0.8);
            shake.0 += 30.0;
            if en.kind == EnemyKind::Warlord {
                banner.show("The Warlord has fallen!", 6.0);
                next.set(GameState::Victory);
            } else {
                banner.show("Rival castle destroyed!", 3.0);
            }
        } else {
            spawn_burst(&mut commands, &assets, &assets.m.blood, &mut rng, pos, 3, 120.0, 8.0, 0.35);
        }

        // Drop XP. Bosses drop a pile of gems.
        let pieces = if en.kind.is_boss() { 16 } else { 1 };
        let per = (en.xp / pieces).max(1);
        for i in 0..pieces {
            if en.xp == 0 {
                break;
            }
            if gem_count >= MAX_GEMS {
                c.xp += per;
                continue;
            }
            gem_count += 1;
            let offset = if pieces > 1 {
                Vec2::from_angle(i as f32 / pieces as f32 * std::f32::consts::TAU) * 50.0
            } else {
                Vec2::ZERO
            };
            let size = 8.0 + (per as f32).sqrt() * 3.0;
            commands.spawn((
                GameEntity,
                Gem { xp: per },
                shape(&assets.diamond, &assets.m.gem, (pos + offset).extend(2.0), Vec2::new(size * 0.7, size)),
            ));
        }
    }
}

fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    mut gems: Query<(Entity, &mut Transform, &Gem), Without<Castle>>,
    mut castle: Query<(&Transform, &mut Castle)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = ctf.translation.truncate();
    let magnet = c.def().magnet + c.radius;
    let dt = time.delta_secs();
    for (e, mut tf, gem) in &mut gems {
        let pos = tf.translation.truncate();
        let d = pos.distance(cpos);
        if d < c.radius {
            c.xp += gem.xp;
            commands.entity(e).despawn();
        } else if d < magnet {
            let step = (cpos - pos) / d * (500.0 + (magnet - d) * 4.0) * dt;
            tf.translation += step.extend(0.0);
        }
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Particle)>,
) {
    let dt = time.delta_secs();
    let drag = (-5.0 * dt).exp();
    for (e, mut tf, mut p) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation += (p.vel * dt).extend(0.0);
        p.vel *= drag;
        let s = p.size * (p.life / p.max_life);
        tf.scale = Vec3::new(s, s, 1.0);
    }
}
