//! Weapon modules: automatic attacks plus an ability on Q / E / R / Space.

use crate::art::{p2, w3};
use crate::assets::GameAssets;
use crate::combat::{spawn_arc, spawn_fx, Bullet, FxKind, Mine, Payload};
use crate::enemy::{Boss, Enemy};
use crate::grid::{Grid, GridEntry};
use crate::items::{level_mult, Loadout, ModuleKind, Stats};
use crate::player::{Castle, ModuleMount};
use crate::{GameEntity, GameRng, GameState, Shake, Sim};
use bevy::light::NotShadowCaster;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use rand::Rng;

pub const ABILITY_KEYS: [KeyCode; 4] = [KeyCode::KeyQ, KeyCode::KeyE, KeyCode::KeyR, KeyCode::Space];
pub const ABILITY_LABELS: [&str; 4] = ["Q", "E", "R", "Space"];

pub struct ModulesPlugin;

impl Plugin for ModulesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cooldowns>()
            .init_resource::<AutoAbilities>()
            .add_systems(OnEnter(GameState::Starting), |mut cd: ResMut<Cooldowns>| cd.0.clear())
            .add_systems(Update, use_abilities.in_set(Sim::Input).after(crate::player::drive))
            .add_systems(Update, auto_attacks.in_set(Sim::Combat));
    }
}

#[derive(Default, Clone, Copy)]
pub struct Timers {
    pub auto: f32,
    /// Seconds until the ability is ready again.
    pub ability: f32,
}

#[derive(Resource, Default)]
pub struct Cooldowns(pub HashMap<ModuleKind, Timers>);

/// When on, abilities fire as soon as they are ready (test autopilot).
#[derive(Resource, Default)]
pub struct AutoAbilities(pub bool);

/// Up to `n` living enemies nearest to `p` within `range`.
fn nearest_n(grid: &Grid, p: Vec2, range: f32, n: usize) -> Vec<GridEntry> {
    let mut found: Vec<(f32, GridEntry)> = Vec::new();
    grid.for_each_near(p, range, |g| {
        let d = g.pos.distance_squared(p);
        if d < range * range {
            found.push((d, *g));
        }
    });
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().take(n).map(|(_, g)| g).collect()
}

fn mount_pos(mounts: &Query<(&GlobalTransform, &ModuleMount)>, kind: ModuleKind, fallback: Vec3) -> Vec3 {
    mounts
        .iter()
        .find(|(_, m)| m.0 == kind)
        .map(|(t, _)| t.translation())
        .unwrap_or(fallback)
}

fn auto_attacks(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    mut cds: ResMut<Cooldowns>,
    mut rng: ResMut<GameRng>,
    castle: Query<(&Transform, &Castle)>,
    mounts: Query<(&GlobalTransform, &ModuleMount)>,
    enemies: Query<&Enemy>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let cpos = p2(ctf.translation);
    let fwd = Vec2::from_angle(c.heading);

    for m in &loadout.modules {
        let t = cds.0.entry(m.kind).or_default();
        t.ability = (t.ability - dt).max(0.0);
        t.auto -= dt * stats.rate;
        if t.auto > 0.0 || m.kind == ModuleKind::Ram {
            continue;
        }
        let dmg = m.kind.damage() * level_mult(m.level) * stats.damage;
        let range = m.kind.range() * stats.range;
        let from = mount_pos(&mounts, m.kind, ctf.translation + Vec3::Y * 3.0);
        let from2 = p2(from);
        // A boss in range gets priority from arrows and shells. Bosses are
        // huge, so aim at the near side of their body, not their center.
        let boss = bosses
            .iter()
            .map(|(t, e)| {
                let p = p2(t.translation);
                let axis = e.axis(t);
                let q = crate::grid::closest_on_segment(p - axis, p + axis, from2);
                let d = q.distance(from2);
                let aim = q + (from2 - q).normalize_or(Vec2::X) * (e.radius * 0.6).min(d);
                (aim, d - e.radius, e)
            })
            .filter(|(_, gap, _)| *gap < range)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _, e)| (p, e.vel));
        match m.kind {
            ModuleKind::ArrowLoft => {
                let count = 2 + m.level as usize;
                let targets = nearest_n(&grid, from2, range, count);
                if targets.is_empty() {
                    t.auto = 0.1;
                    continue;
                }
                t.auto = m.kind.auto_cooldown();
                for i in 0..count {
                    let g = targets[i % targets.len()];
                    let (gpos, vel) = match boss {
                        Some(b) if i % 2 == 0 => b,
                        _ => (g.pos, enemies.get(g.entity).map(|e| e.vel).unwrap_or_default()),
                    };
                    let dist = gpos.distance(from2);
                    let dur = 0.35 + dist / 34.0;
                    let jitter = Vec2::new(rng.0.random_range(-0.3..0.3), rng.0.random_range(-0.3..0.3));
                    spawn_arc(&mut commands, &assets, from, gpos + vel * dur + jitter, dur, 1.5 + dist * 0.12, Payload::Arrow { dmg });
                }
            }
            ModuleKind::Mortar => {
                // Aim at the busiest spot among a few candidates.
                let near = nearest_n(&grid, from2, range, 40);
                if near.is_empty() {
                    t.auto = 0.2;
                    continue;
                }
                t.auto = m.kind.auto_cooldown();
                let shells = 1 + (m.level >= 3) as usize + (m.level >= 5) as usize;
                for s in 0..shells {
                    let mut best = near[rng.0.random_range(0..near.len())];
                    let mut best_n = 0;
                    for _ in 0..5 {
                        let cand = near[rng.0.random_range(0..near.len())];
                        let mut n = 0;
                        grid.for_each_near(cand.pos, 4.0, |_| n += 1);
                        if n > best_n {
                            best_n = n;
                            best = cand;
                        }
                    }
                    let dur = 1.0 + s as f32 * 0.15;
                    let radius = (3.2 + 0.25 * m.level as f32) * stats.area;
                    let at = match boss {
                        Some((bp, bv)) if s == 0 => bp + bv * dur,
                        _ => best.pos,
                    };
                    spawn_arc(&mut commands, &assets, from + Vec3::Y, at, dur, 12.0, Payload::Shell { dmg, radius });
                }
                spawn_fx(&mut commands, &assets, FxKind::Smoke, from + Vec3::Y * 1.5, 1.6, Vec3::Y * 3.0);
            }
            ModuleKind::MineLayer => {
                if c.speed < 2.0 {
                    t.auto = 0.2;
                    continue;
                }
                t.auto = m.kind.auto_cooldown() * 0.88f32.powi(m.level as i32 - 1);
                drop_mine(&mut commands, &assets, from2 - fwd * 1.0, dmg, (3.0 + 0.2 * m.level as f32) * stats.area);
            }
            ModuleKind::Broadside => {
                let side = Vec2::new(-fwd.y, fwd.x);
                let near = nearest_n(&grid, cpos, range, 60);
                let mut fired = false;
                for s in [-1.0f32, 1.0] {
                    let dir = side * s;
                    if !near.iter().any(|g| (g.pos - cpos).normalize_or_zero().dot(dir) > 0.5) {
                        continue;
                    }
                    fired = true;
                    let balls = 1 + m.level / 2;
                    for b in 0..balls {
                        let spread = (b as f32 - (balls - 1) as f32 / 2.0) * 0.12;
                        let d = Vec2::from_angle(dir.to_angle() + spread);
                        fire_cannonball(&mut commands, &assets, cpos + d * (c.radius + 0.5), d * 34.0 + c.vel * 0.5, dmg, 1 + m.level, range / 34.0);
                    }
                }
                t.auto = if fired { m.kind.auto_cooldown() } else { 0.15 };
            }
            ModuleKind::Ballista => {
                // A boss in range first, otherwise the nearest enemy.
                let target = boss.or_else(|| {
                    nearest_n(&grid, from2, range, 1).first().map(|g| (g.pos, enemies.get(g.entity).map(|e| e.vel).unwrap_or_default()))
                });
                let Some((tpos, tvel)) = target else {
                    t.auto = 0.15;
                    continue;
                };
                t.auto = m.kind.auto_cooldown() * 0.9f32.powi(m.level as i32 - 1);
                let lead = tpos + tvel * (tpos.distance(from2) / 60.0);
                let dir = (lead - from2).normalize_or(fwd);
                fire_bolt(&mut commands, &assets, from2, dir, dmg, 4 + m.level * 2, range / 60.0);
            }
            ModuleKind::Ram => {}
        }
    }
}

fn fire_bolt(commands: &mut Commands, assets: &GameAssets, from: Vec2, dir: Vec2, dmg: f32, pierce: u32, life: f32) {
    commands.spawn((
        GameEntity,
        Bullet::friendly(dir * 60.0, dmg, 0.5, pierce, life),
        Mesh3d(assets.arrow.clone()),
        MeshMaterial3d(assets.m.vc.clone()),
        Transform::from_translation(w3(from, 2.0)).with_rotation(crate::art::yaw(dir.to_angle())).with_scale(Vec3::splat(2.4)),
        NotShadowCaster,
    ));
}

fn drop_mine(commands: &mut Commands, assets: &GameAssets, at: Vec2, dmg: f32, radius: f32) {
    commands.spawn((
        GameEntity,
        Mine { arm: 0.6, life: 30.0, dmg, radius },
        Mesh3d(assets.mine.clone()),
        MeshMaterial3d(assets.m.vc.clone()),
        Transform::from_translation(w3(at, 0.0)),
        NotShadowCaster,
    ));
}

fn fire_cannonball(commands: &mut Commands, assets: &GameAssets, from: Vec2, vel: Vec2, dmg: f32, pierce: u32, life: f32) {
    commands.spawn((
        GameEntity,
        Bullet::friendly(vel, dmg, 0.45, pierce, life),
        Mesh3d(assets.ball.clone()),
        MeshMaterial3d(assets.m.cannonball.clone()),
        Transform::from_translation(w3(from, 1.6)).with_scale(Vec3::splat(0.75)),
        NotShadowCaster,
    ));
    spawn_fx(commands, assets, FxKind::Smoke, w3(from, 1.8), 1.3, w3(vel * 0.05, 1.5));
    spawn_fx(commands, assets, FxKind::Spark, w3(from, 1.6), 0.9, Vec3::ZERO);
}

fn use_abilities(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    auto: Res<AutoAbilities>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    mut cds: ResMut<Cooldowns>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    mut castle: Query<(&Transform, &mut Castle)>,
    mounts: Query<(&GlobalTransform, &ModuleMount)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let fwd = Vec2::from_angle(c.heading);
    for (slot, m) in loadout.modules.iter().enumerate() {
        let t = cds.0.entry(m.kind).or_default();
        let pressed = keys.just_pressed(ABILITY_KEYS[slot]) || (auto.0 && (m.kind != ModuleKind::Ram || grid.nearest(cpos + fwd * 12.0, 8.0).is_some()));
        if !pressed || t.ability > 0.0 {
            continue;
        }
        t.ability = m.kind.ability_cooldown() * stats.ability_cd;
        let dmg = m.kind.damage() * level_mult(m.level) * stats.damage;
        let from = mount_pos(&mounts, m.kind, ctf.translation + Vec3::Y * 3.0);
        match m.kind {
            ModuleKind::ArrowLoft => {
                for _ in 0..(30 + 6 * m.level) {
                    let a = rng.0.random::<f32>() * std::f32::consts::TAU;
                    let r = c.radius + rng.0.random_range(2.0..20.0) * stats.range;
                    let to = cpos + Vec2::from_angle(a) * r;
                    let dur = rng.0.random_range(0.6..1.4);
                    spawn_arc(&mut commands, &assets, from, to, dur, 9.0, Payload::Arrow { dmg: dmg * 1.5 });
                }
            }
            ModuleKind::Mortar => {
                let n = 8 + 2 * m.level;
                for i in 0..n {
                    let to = cpos - fwd * (c.radius + 3.0 + i as f32 * 3.0);
                    let radius = 3.6 * stats.area;
                    spawn_arc(&mut commands, &assets, from + Vec3::Y, to, 0.8 + i as f32 * 0.08, 10.0, Payload::Shell { dmg: dmg * 1.3, radius });
                }
                shake.0 += 0.5;
            }
            ModuleKind::Ram => {
                c.charge = 2.5;
                shake.0 += 0.6;
            }
            ModuleKind::MineLayer => {
                for _ in 0..(10 + 2 * m.level) {
                    let a = (-fwd).to_angle() + rng.0.random_range(-1.2..1.2);
                    let r = c.radius + rng.0.random_range(1.0..12.0);
                    drop_mine(&mut commands, &assets, cpos + Vec2::from_angle(a) * r, dmg, 3.4 * stats.area);
                }
            }
            ModuleKind::Ballista => {
                let n = 9 + m.level;
                for b in 0..n {
                    let a = fwd.to_angle() + (b as f32 / (n - 1) as f32 - 0.5) * 1.1;
                    fire_bolt(&mut commands, &assets, p2(from), Vec2::from_angle(a), dmg * 1.3, 8 + m.level, 1.0);
                }
                shake.0 += 0.4;
            }
            ModuleKind::Broadside => {
                let side = Vec2::new(-fwd.y, fwd.x);
                let n = 7 + m.level;
                for s in [-1.0f32, 1.0] {
                    for b in 0..n {
                        let spread = (b as f32 / (n - 1) as f32 - 0.5) * 0.9;
                        let d = Vec2::from_angle((side * s).to_angle() + spread);
                        fire_cannonball(&mut commands, &assets, cpos + d * (c.radius + 0.5), d * 36.0, dmg * 1.4, 3 + m.level, 1.2);
                    }
                }
                shake.0 += 0.8;
            }
        }
    }
}
