//! Shots, explosions, mines, enemy deaths and coins. Effects live in `vfx.rs`.

use crate::art::{p2, w3};
use crate::assets::GameAssets;
use crate::enemy::{Boss, Enemy, EnemyKind};
use crate::grid::{Grid, MAX_ENEMY_RADIUS};
use crate::items::Stats;
use crate::player::Castle;
use crate::progression::Progress;
use crate::{GameEntity, GameRng, RunStats, Shake, Sim};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;

pub use crate::vfx::{explosion, spawn_fx, spawn_fx_rot, FxKind};

/// Above this many coins on the ground, XP goes straight to the castle.
const MAX_COINS: usize = 900;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BossDefeated>()
            .add_systems(Update, (update_arcs, update_bullets, update_mines).in_set(Sim::Combat))
            .add_systems(Update, (enemy_deaths, collect_coins).chain().in_set(Sim::Resolve));
    }
}

#[derive(Message)]
pub struct BossDefeated(pub EnemyKind);

// ---------------------------------------------------------------------------
// Arcing shots: arrows, shells, boulders.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum Payload {
    Arrow { dmg: f32 },
    Shell { dmg: f32, radius: f32 },
    EnemyArrow { dmg: f32 },
    Boulder { dmg: f32, radius: f32 },
}

#[derive(Component)]
pub struct ArcShot {
    from: Vec3,
    to: Vec2,
    t: f32,
    dur: f32,
    apex: f32,
    payload: Payload,
}

pub fn spawn_arc(commands: &mut Commands, assets: &GameAssets, from: Vec3, to: Vec2, dur: f32, apex: f32, payload: Payload) {
    let (mesh, mat, scale) = match payload {
        Payload::Arrow { .. } => (assets.arrow.clone(), assets.m.vc.clone(), Vec3::ONE),
        Payload::EnemyArrow { .. } => (assets.enemy_arrow.clone(), assets.m.vc.clone(), Vec3::ONE),
        Payload::Shell { .. } => (assets.ball.clone(), assets.m.cannonball.clone(), Vec3::splat(0.8)),
        Payload::Boulder { .. } => (assets.ball.clone(), assets.m.stone.clone(), Vec3::splat(1.8)),
    };
    commands.spawn((
        GameEntity,
        ArcShot { from, to, t: 0.0, dur: dur.max(0.05), apex, payload },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(from).with_scale(scale),
        NotShadowCaster,
    ));
}

fn arc_point(a: &ArcShot, f: f32) -> Vec3 {
    let ground = p2(a.from).lerp(a.to, f);
    let h = a.from.y * (1.0 - f) + 4.0 * a.apex * f * (1.0 - f);
    w3(ground, h)
}

fn update_arcs(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    mut arcs: Query<(Entity, &mut Transform, &mut ArcShot)>,
    mut enemies: Query<&mut Enemy>,
    mut castle: Query<(&Transform, &mut Castle), Without<ArcShot>>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    for (e, mut tf, mut a) in &mut arcs {
        a.t += dt;
        let f = (a.t / a.dur).min(1.0);
        let p = arc_point(&a, f);
        let ahead = arc_point(&a, (f + 0.02).min(1.0));
        let dir = (ahead - p).normalize_or(Vec3::X);
        if f < 1.0 {
            tf.translation = p;
            tf.rotation = Quat::from_rotation_arc(Vec3::X, dir);
            continue;
        }
        commands.entity(e).despawn();
        let at = a.to;
        match a.payload {
            Payload::Arrow { dmg } => {
                if let Some(g) = grid.nearest(at, 1.3)
                    && let Ok(mut en) = enemies.get_mut(g.entity)
                {
                    en.hit(dmg);
                }
            }
            Payload::Shell { dmg, radius } => {
                damage_area(&grid, &mut enemies, at, radius, dmg, 8.0);
                explosion(&mut commands, &assets, &mut rng, at, radius);
                shake.0 += 0.15;
            }
            Payload::EnemyArrow { dmg } => {
                if at.distance(cpos) < c.radius + 0.6 {
                    c.hurt(dmg);
                }
            }
            Payload::Boulder { dmg, radius } => {
                if at.distance(cpos) < c.radius * 0.6 + radius {
                    c.hurt(dmg);
                    shake.0 += 0.8;
                }
                explosion(&mut commands, &assets, &mut rng, at, radius * 0.8);
            }
        }
    }
    let _ = stats;
}

// ---------------------------------------------------------------------------
// Straight shots: cannonballs (ours and theirs).
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct Bullet {
    pub vel: Vec2,
    pub dmg: f32,
    pub radius: f32,
    pub pierce: u32,
    pub life: f32,
    pub hostile: bool,
    hits: Vec<Entity>,
}

impl Bullet {
    pub fn friendly(vel: Vec2, dmg: f32, radius: f32, pierce: u32, life: f32) -> Self {
        Self { vel, dmg, radius, pierce, life, hostile: false, hits: Vec::new() }
    }
    pub fn hostile(vel: Vec2, dmg: f32, radius: f32, life: f32) -> Self {
        Self { vel, dmg, radius, pierce: 0, life, hostile: true, hits: Vec::new() }
    }
}

fn update_bullets(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut shake: ResMut<Shake>,
    mut bullets: Query<(Entity, &mut Transform, &mut Bullet)>,
    mut enemies: Query<&mut Enemy>,
    mut castle: Query<(&Transform, &mut Castle), Without<Bullet>>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    for (e, mut tf, mut b) in &mut bullets {
        let start = p2(tf.translation);
        let step = b.vel * dt;
        b.life -= dt;
        let end = start + step;
        tf.translation = w3(end, tf.translation.y);
        if b.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if b.hostile {
            if end.distance(cpos) < c.radius + b.radius {
                c.hurt(b.dmg);
                shake.0 += 0.3;
                commands.entity(e).despawn();
            }
            continue;
        }
        // Check along the whole path so fast shots never skip enemies.
        let n = (step.length() / (b.radius + 0.4)).ceil().clamp(1.0, 12.0) as u32;
        let mut done = false;
        for i in 1..=n {
            let at = start + step * (i as f32 / n as f32);
            let mut hit = None;
            grid.for_each_near(at, b.radius + MAX_ENEMY_RADIUS, |g| {
                if hit.is_none() && !b.hits.contains(&g.entity) {
                    let reach = b.radius + g.radius;
                    if g.pos.distance_squared(at) < reach * reach {
                        hit = Some(g.entity);
                    }
                }
            });
            if let Some(target) = hit
                && let Ok(mut en) = enemies.get_mut(target)
            {
                en.hit(b.dmg);
                en.knock += b.vel.normalize_or(Vec2::X) * 6.0;
                if b.pierce == 0 {
                    done = true;
                    break;
                }
                b.pierce -= 1;
                b.hits.push(target);
            }
        }
        if done {
            commands.entity(e).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Mines
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct Mine {
    pub arm: f32,
    pub life: f32,
    pub dmg: f32,
    pub radius: f32,
}

fn update_mines(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut mines: Query<(Entity, &Transform, &mut Mine)>,
    mut enemies: Query<&mut Enemy>,
) {
    let dt = time.delta_secs();
    for (e, tf, mut m) in &mut mines {
        m.arm -= dt;
        m.life -= dt;
        if m.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if m.arm > 0.0 {
            continue;
        }
        let pos = p2(tf.translation);
        if grid.nearest(pos, 1.0).is_some() {
            damage_area(&grid, &mut enemies, pos, m.radius, m.dmg, 10.0);
            explosion(&mut commands, &assets, &mut rng, pos, m.radius);
            commands.entity(e).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Damage helpers
// ---------------------------------------------------------------------------

pub fn damage_area(grid: &Grid, enemies: &mut Query<&mut Enemy>, pos: Vec2, radius: f32, dmg: f32, knock: f32) {
    grid.for_each_near(pos, radius + MAX_ENEMY_RADIUS, |g| {
        let reach = radius + g.radius;
        let d = g.pos - pos;
        if d.length_squared() < reach * reach
            && let Ok(mut en) = enemies.get_mut(g.entity)
        {
            en.hit(dmg);
            if !en.kind.is_boss() {
                en.knock += d.normalize_or(Vec2::X) * knock;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Deaths and coins
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct Coin {
    xp: f32,
}

pub fn enemy_deaths(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<RunStats>,
    mut shake: ResMut<Shake>,
    mut progress: ResMut<Progress>,
    mut defeated: MessageWriter<BossDefeated>,
    enemies: Query<(Entity, &Transform, &Enemy, Option<&Boss>)>,
    coins: Query<(), With<Coin>>,
) {
    let mut coin_count = coins.iter().count();
    for (e, tf, en, boss) in &enemies {
        if en.hp > 0.0 {
            continue;
        }
        commands.entity(e).despawn();
        stats.kills += 1;
        let pos = p2(tf.translation);
        if let Some(b) = boss {
            if let Some(t) = b.telegraph {
                commands.entity(t).despawn();
            }
            for i in 0..6 {
                let off = Vec2::from_angle(i as f32) * en.radius * 0.6;
                explosion(&mut commands, &assets, &mut rng, pos + off, en.radius * 0.7);
            }
            for _ in 0..10 {
                let off = Vec3::new(rng.0.random_range(-1.0..1.0), 0.0, rng.0.random_range(-1.0..1.0)) * en.radius;
                spawn_fx(&mut commands, &assets, FxKind::DarkSmoke, w3(pos, 2.0) + off, en.radius * 0.8, Vec3::Y * 3.0);
            }
            shake.0 += 3.0;
            stats.bosses_killed += 1;
            defeated.write(BossDefeated(en.kind));
        } else {
            spawn_fx(&mut commands, &assets, FxKind::Blood, w3(pos, 0.02), 0.5 + en.radius * 0.6, Vec3::ZERO);
            if en.kind == EnemyKind::Cart {
                explosion(&mut commands, &assets, &mut rng, pos, 2.0);
            }
        }

        let pieces = en.coins.min(30);
        let value = en.coins as f32 / pieces.max(1) as f32;
        for i in 0..pieces {
            if coin_count >= MAX_COINS {
                progress.add_xp(value);
                continue;
            }
            coin_count += 1;
            let off = if pieces > 1 {
                Vec2::from_angle(i as f32 * 2.4) * (0.5 + i as f32 * 0.25).min(en.radius + 2.0)
            } else {
                Vec2::ZERO
            };
            commands.spawn((
                GameEntity,
                Coin { xp: value },
                Mesh3d(assets.coin.0[0].0.clone()),
                MeshMaterial3d(assets.coin.0[0].1.clone()),
                Transform::from_translation(w3(pos + off, 0.6)),
                NotShadowCaster,
            ));
        }
    }
}

fn collect_coins(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<Stats>,
    mut progress: ResMut<Progress>,
    mut coins: Query<(Entity, &mut Transform, &Coin), Without<Castle>>,
    castle: Query<&Transform, With<Castle>>,
) {
    let Ok(ctf) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let magnet = stats.magnet + stats.radius;
    for (e, mut tf, coin) in &mut coins {
        let pos = p2(tf.translation);
        let d = pos.distance(cpos);
        if d < stats.radius {
            progress.add_xp(coin.xp * stats.xp_mult);
            commands.entity(e).despawn();
            continue;
        }
        let mut next = pos;
        if d < magnet {
            next += (cpos - pos) / d * (14.0 + (magnet - d) * 3.0) * dt;
        }
        tf.translation = w3(next, 0.6 + (t * 3.0 + pos.x).sin() * 0.15);
        tf.rotation = Quat::from_rotation_y(t * 3.0 + pos.y);
    }
}
