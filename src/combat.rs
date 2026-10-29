//! Shots, explosions, mines, enemy deaths, coins and visual effects.

use crate::art::{p2, w3, yaw};
use crate::assets::GameAssets;
use crate::enemy::{spawn_corpse, Boss, Corpse, Enemy, EnemyKind, MAX_CORPSES};
use crate::grid::{Grid, MAX_ENEMY_RADIUS};
use crate::items::Stats;
use crate::player::Castle;
use crate::progression::Progress;
use crate::{GameEntity, GameRng, RunStats, Shake, Sim};
use bevy::prelude::*;
use rand::Rng;

/// Above this many coins on the ground, XP goes straight to the castle.
const MAX_COINS: usize = 900;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BossDefeated>()
            .add_systems(Update, (update_arcs, update_bullets, update_mines).in_set(Sim::Combat))
            .add_systems(Update, (enemy_deaths, collect_coins, update_fx).chain().in_set(Sim::Resolve));
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
    /// Bursts into burning ground.
    FirePot { dmg: f32, radius: f32 },
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
        Payload::Boulder { radius, .. } => (assets.ball.clone(), assets.m.stone.clone(), Vec3::splat((radius * 0.55).max(1.8))),
        Payload::FirePot { .. } => (assets.ball.clone(), assets.m.fire.clone(), Vec3::splat(1.6)),
    };
    commands.spawn((
        GameEntity,
        ArcShot { from, to, t: 0.0, dur: dur.max(0.05), apex, payload },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(from).with_scale(scale),
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
                // Boss boulders crush their own troops too.
                damage_troops(&grid, &mut enemies, at, radius, dmg * 2.0);
                explosion(&mut commands, &assets, &mut rng, at, radius * 0.8);
                spawn_fx(&mut commands, &assets, FxKind::Shockwave, w3(at, 0.6), radius * 1.4, Vec3::ZERO);
                shake.0 += 0.3;
            }
            Payload::FirePot { dmg, radius } => {
                explosion(&mut commands, &assets, &mut rng, at, radius * 0.5);
                crate::boss::spawn_fire(&mut commands, &assets, at, radius, dmg, 7.0);
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
                    if g.dist2(at) < reach * reach {
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
        let d = g.closest(pos) - pos;
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

/// Like `damage_area`, but bosses are not hurt.
fn damage_troops(grid: &Grid, enemies: &mut Query<&mut Enemy>, pos: Vec2, radius: f32, dmg: f32) {
    grid.for_each_near(pos, radius + MAX_ENEMY_RADIUS, |g| {
        let reach = radius + g.radius;
        if g.axis == Vec2::ZERO
            && g.radius <= MAX_ENEMY_RADIUS
            && g.pos.distance_squared(pos) < reach * reach
            && let Ok(mut en) = enemies.get_mut(g.entity)
        {
            en.hit(dmg);
            en.knock += (g.pos - pos).normalize_or(Vec2::X) * 12.0;
        }
    });
}

/// Fireball, smoke, flying debris and a scorch mark.
pub fn explosion(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, pos: Vec2, radius: f32) {
    spawn_fx(commands, assets, FxKind::Fireball, w3(pos, 0.8), radius * 1.6, Vec3::ZERO);
    for _ in 0..4 {
        let off = Vec3::new(rng.0.random_range(-1.0..1.0), 0.0, rng.0.random_range(-1.0..1.0)) * radius * 0.5;
        spawn_fx(commands, assets, FxKind::Smoke, w3(pos, 1.0) + off, radius * 0.7, Vec3::Y * 2.0 + off * 0.8);
    }
    for _ in 0..5 {
        let v = Vec3::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(1.0..2.0), rng.0.random_range(-1.0..1.0)) * 7.0;
        spawn_fx(commands, assets, FxKind::Debris, w3(pos, 0.6), 0.35, v);
    }
    spawn_fx(commands, assets, FxKind::Scorch, w3(pos, 0.03), radius * 1.4, Vec3::ZERO);
}

// ---------------------------------------------------------------------------
// Deaths and coins
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct Coin {
    xp: f32,
}

fn enemy_deaths(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<RunStats>,
    mut shake: ResMut<Shake>,
    mut progress: ResMut<Progress>,
    mut defeated: MessageWriter<BossDefeated>,
    enemies: Query<(Entity, &Transform, &Enemy, Option<&Boss>, Option<&Mesh3d>)>,
    castle: Query<&Transform, With<Castle>>,
    coins: Query<(), With<Coin>>,
    corpses: Query<(), With<Corpse>>,
) {
    let mut coin_count = coins.iter().count();
    let mut corpse_count = corpses.iter().count();
    let cpos = castle.single().map(|t| p2(t.translation)).unwrap_or_default();
    for (e, tf, en, boss, mesh) in &enemies {
        if en.hp > 0.0 {
            continue;
        }
        stats.kills += 1;
        let pos = p2(tf.translation);
        if boss.is_some() {
            // Bosses play their collapse, then drop their gold.
            crate::boss::begin_wreck(&mut commands, e, en, tf, cpos);
            shake.0 += 2.0;
            stats.bosses_killed += 1;
            defeated.write(BossDefeated(en.kind));
            continue;
        }
        commands.entity(e).despawn();
        spawn_fx(&mut commands, &assets, FxKind::Blood, w3(pos, 0.02), 0.5 + en.radius * 0.6, Vec3::ZERO);
        if en.kind == EnemyKind::Cart {
            explosion(&mut commands, &assets, &mut rng, pos, 2.0);
        }
        if let Some(mesh) = mesh
            && corpse_count < MAX_CORPSES
        {
            corpse_count += 1;
            spawn_corpse(&mut commands, &assets, en, tf, mesh);
        }
        drop_coins(&mut commands, &assets, &mut progress, &mut coin_count, pos, en.coins, en.radius + 2.0);
    }
}

/// Scatters `coins` worth of gold around `pos`, in at most 30 pieces.
/// Past the coin limit the XP goes straight to the castle.
pub fn drop_coins(
    commands: &mut Commands,
    assets: &GameAssets,
    progress: &mut Progress,
    coin_count: &mut usize,
    pos: Vec2,
    coins: u32,
    spread: f32,
) {
    let pieces = coins.min(30);
    let value = coins as f32 / pieces.max(1) as f32;
    for i in 0..pieces {
        if *coin_count >= MAX_COINS {
            progress.add_xp(value);
            continue;
        }
        *coin_count += 1;
        let off = if pieces > 1 {
            Vec2::from_angle(i as f32 * 2.4) * (0.5 + i as f32 * 0.25).min(spread)
        } else {
            Vec2::ZERO
        };
        commands.spawn((
            GameEntity,
            Coin { xp: value },
            Mesh3d(assets.coin.0[0].0.clone()),
            MeshMaterial3d(assets.coin.0[0].1.clone()),
            Transform::from_translation(w3(pos + off, 0.6)),
        ));
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

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FxKind {
    Fireball,
    Smoke,
    DarkSmoke,
    Spark,
    Debris,
    Scorch,
    Blood,
    Track,
    Dirt,
    Telegraph { dur: f32 },
    /// A faint red disc under a telegraph ring.
    Zone { dur: f32 },
    /// A red strip on the ground (charges, broadsides). Sized by its spawner.
    Lane { dur: f32 },
    /// A ring of dust racing outward.
    Shockwave,
}

#[derive(Component)]
pub struct Fx {
    kind: FxKind,
    life: f32,
    max: f32,
    vel: Vec3,
    size: f32,
}

pub fn spawn_fx(commands: &mut Commands, assets: &GameAssets, kind: FxKind, pos: Vec3, size: f32, vel: Vec3) -> Entity {
    let m = &assets.m;
    let (mesh, mat, life, scale) = match kind {
        FxKind::Fireball => (&assets.ball, &m.fire, 0.35, Vec3::splat(size * 0.4)),
        FxKind::Smoke => (&assets.ball, &m.smoke, 1.4, Vec3::splat(size * 0.5)),
        FxKind::DarkSmoke => (&assets.ball, &m.smoke_dark, 1.6, Vec3::splat(size * 0.5)),
        FxKind::Spark => (&assets.ball, &m.spark, 0.25, Vec3::splat(size)),
        FxKind::Debris => (&assets.cube, &m.debris, 1.0, Vec3::splat(size)),
        FxKind::Scorch => (&assets.disc, &m.scorch, 9.0, Vec3::new(size, 0.02, size)),
        FxKind::Blood => (&assets.disc, &m.blood, 6.0, Vec3::new(size, 0.02, size)),
        FxKind::Track => (&assets.rut, &m.track, 7.0, Vec3::new(size * 2.4, 1.0, size * 1.4)),
        FxKind::Dirt => (&assets.cube, &m.dirt, 60.0, Vec3::new(size, 0.03, size * 0.8)),
        FxKind::Telegraph { dur } => (&assets.ring, &m.telegraph, dur, Vec3::splat(size * 0.2)),
        FxKind::Zone { dur } => (&assets.disc, &m.warn, dur, Vec3::new(size * 2.0, 0.03, size * 2.0)),
        FxKind::Lane { dur } => (&assets.cube, &m.warn, dur, Vec3::ONE),
        FxKind::Shockwave => (&assets.ring, &m.smoke, 0.55, Vec3::splat(size * 0.2)),
    };
    commands
        .spawn((
            GameEntity,
            Fx { kind, life, max: life, vel, size },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos).with_scale(scale),
        ))
        .id()
}

/// Same as `spawn_fx` but facing a direction (used for tracks).
pub fn spawn_fx_rot(commands: &mut Commands, assets: &GameAssets, kind: FxKind, pos: Vec3, size: f32, angle: f32) -> Entity {
    let e = spawn_fx(commands, assets, kind, pos, size, Vec3::ZERO);
    commands.entity(e).entry::<Transform>().and_modify(move |mut t| t.rotation = yaw(angle));
    e
}

fn update_fx(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q: Query<(Entity, &mut Transform, &mut Fx, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut fx, mut mat) in &mut q {
        fx.life -= dt;
        if fx.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let age = 1.0 - fx.life / fx.max;
        let s = fx.size;
        match fx.kind {
            FxKind::Fireball => {
                let k = if age < 0.35 { 0.4 + age / 0.35 * 0.6 } else { 1.0 - (age - 0.35) / 0.65 };
                tf.scale = Vec3::splat(s * k.max(0.01));
            }
            FxKind::Smoke | FxKind::DarkSmoke => {
                tf.translation += fx.vel * dt;
                fx.vel *= (-1.5 * dt).exp();
                // Puffs swell as they rise and fade out.
                tf.scale = Vec3::splat(s * (0.5 + age * 1.1));
                let fades = if fx.kind == FxKind::Smoke { &assets.m.smoke_fade } else { &assets.m.smoke_dark_fade };
                let i = ((age * fades.len() as f32) as usize).min(fades.len() - 1);
                if mat.0 != fades[i] {
                    mat.0 = fades[i].clone();
                }
            }
            FxKind::Spark => tf.scale = Vec3::splat(s * (1.0 - age).max(0.01)),
            FxKind::Debris => {
                fx.vel.y -= 22.0 * dt;
                tf.translation += fx.vel * dt;
                if tf.translation.y < 0.1 {
                    tf.translation.y = 0.1;
                    fx.vel = Vec3::ZERO;
                }
                tf.rotate_x(dt * 8.0);
                tf.scale = Vec3::splat(s * (1.0 - age * age).max(0.01));
            }
            FxKind::Scorch | FxKind::Blood | FxKind::Track | FxKind::Dirt => {
                // Shrink away during the last quarter of life.
                let k = ((1.0 - age) / 0.25).min(1.0);
                let base = if fx.kind == FxKind::Track { Vec3::new(s * 2.0, 0.02, s) } else { Vec3::new(s, 0.02, s) };
                tf.scale = Vec3::new(base.x * k.max(0.01), base.y, base.z * k.max(0.01));
            }
            FxKind::Telegraph { .. } => {
                let k = 0.2 + age * 0.8;
                tf.scale = Vec3::new(s * k, 0.6, s * k);
            }
            FxKind::Zone { .. } | FxKind::Lane { .. } => {}
            FxKind::Shockwave => {
                let k = 0.25 + age.sqrt() * 0.85;
                tf.scale = Vec3::new(s * k, 2.5 * (1.0 - age) + 0.2, s * k);
                let fades = &assets.m.smoke_fade;
                let i = ((age * fades.len() as f32) as usize).min(fades.len() - 1);
                if mat.0 != fades[i] {
                    mat.0 = fades[i].clone();
                }
            }
        }
    }
}
