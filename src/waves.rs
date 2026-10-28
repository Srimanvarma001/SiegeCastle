//! The wave director. Enemies arrive in 10 waves that grow bigger each time.
//! Enemies left alive are never removed, so the map fills up over a run.

use crate::assets::GameAssets;
use crate::castle::Castle;
use crate::enemy::{Enemy, EnemyKind, spawn_enemy};
use crate::{Banner, GameRng, GameState, MAP_HALF, Sim};
use bevy::prelude::*;
use rand::Rng;

pub const TOTAL_WAVES: u32 = 10;
const WAVE_TIME: f32 = 40.0;
const BREAK_TIME: f32 = 5.0;
/// Hard cap so the game stays fast even if the player is overwhelmed.
pub const MAX_ENEMIES: usize = 3500;

pub struct WavePlugin;

impl Plugin for WavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaveDirector::default())
            .add_systems(OnEnter(GameState::Starting), reset_waves)
            .add_systems(Update, run_waves.in_set(Sim::Spawn));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Break,
    Active,
}

#[derive(Resource)]
pub struct WaveDirector {
    /// Current wave, 1-based. 0 before the first wave.
    pub wave: u32,
    pub phase: Phase,
    pub timer: f32,
    spawn_acc: f32,
    /// Direction the current group of enemies comes from.
    group_angle: f32,
    group_timer: f32,
    horde_timer: f32,
}

impl Default for WaveDirector {
    fn default() -> Self {
        Self {
            wave: 0,
            phase: Phase::Break,
            timer: BREAK_TIME - 2.0,
            spawn_acc: 0.0,
            group_angle: 0.0,
            group_timer: 0.0,
            horde_timer: 0.0,
        }
    }
}

impl WaveDirector {
    /// Seconds until the next wave starts, during a break.
    pub fn break_left(&self) -> f32 {
        (BREAK_TIME - self.timer).max(0.0)
    }

    /// Enemies spawned per second for a wave.
    fn rate(wave: u32) -> f32 {
        let w = wave as f32;
        1.0 + w.powf(1.6) * 0.9
    }
}

fn reset_waves(mut d: ResMut<WaveDirector>) {
    *d = WaveDirector::default();
}

/// Picks an enemy type. Tougher types join as the waves go on.
fn pick_kind(wave: u32, roll: f32) -> EnemyKind {
    let (knight, ram) = match wave {
        0..=1 => (0.0, 0.0),
        2 => (0.15, 0.0),
        3 => (0.30, 0.0),
        4 => (0.30, 0.08),
        5..=6 => (0.35, 0.12),
        7..=8 => (0.38, 0.16),
        _ => (0.40, 0.20),
    };
    if roll < ram {
        EnemyKind::Ram
    } else if roll < ram + knight {
        EnemyKind::Knight
    } else {
        EnemyKind::Raider
    }
}

#[allow(clippy::too_many_arguments)]
fn run_waves(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut d: ResMut<WaveDirector>,
    mut rng: ResMut<GameRng>,
    mut banner: ResMut<Banner>,
    castle: Query<(&Transform, &Castle)>,
    enemies: Query<(), With<Enemy>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let cpos = ctf.translation.truncate();
    d.timer += dt;

    match d.phase {
        Phase::Break => {
            if d.timer < BREAK_TIME {
                return;
            }
            d.wave += 1;
            d.phase = Phase::Active;
            d.timer = 0.0;
            d.horde_timer = 6.0;
            let wave = d.wave;
            let msg = match wave {
                5 => "Wave 5: A rival castle approaches!".to_string(),
                8 => "Wave 8: Two rival castles!".to_string(),
                TOTAL_WAVES => "FINAL WAVE: The Warlord's citadel!".to_string(),
                w => format!("Wave {w}"),
            };
            banner.show(msg, 3.0);

            let bosses: &[EnemyKind] = match wave {
                5 => &[EnemyKind::RivalCastle],
                8 => &[EnemyKind::RivalCastle, EnemyKind::RivalCastle],
                TOTAL_WAVES => &[EnemyKind::Warlord],
                _ => &[],
            };
            let view = 800.0 * c.def().cam_scale;
            for &kind in bosses {
                let a = rng.0.random::<f32>() * std::f32::consts::TAU;
                let pos = spawn_point(cpos, a, view + 150.0);
                spawn_enemy(&mut commands, &assets, kind, pos, wave);
            }
        }
        Phase::Active => {
            let wave = d.wave;
            // The last wave never ends on its own: beat the Warlord to win.
            if wave < TOTAL_WAVES && d.timer >= WAVE_TIME {
                d.phase = Phase::Break;
                d.timer = 0.0;
                banner.show(format!("Wave {wave} survived!"), 2.0);
                return;
            }

            let alive = enemies.iter().count();
            let view = 800.0 * c.def().cam_scale;

            // Enemies come in groups from one direction, which shifts every few seconds.
            d.group_timer -= dt;
            if d.group_timer <= 0.0 {
                d.group_timer = 3.0;
                d.group_angle = rng.0.random::<f32>() * std::f32::consts::TAU;
            }

            d.spawn_acc += WaveDirector::rate(wave) * dt;
            let mut budget = MAX_ENEMIES.saturating_sub(alive);
            while d.spawn_acc >= 1.0 && budget > 0 {
                d.spawn_acc -= 1.0;
                budget -= 1;
                let a = d.group_angle + (rng.0.random::<f32>() - 0.5) * 1.4;
                let r = view + rng.0.random_range(40.0..260.0);
                let kind = pick_kind(wave, rng.0.random());
                spawn_enemy(&mut commands, &assets, kind, spawn_point(cpos, a, r), wave);
            }
            d.spawn_acc = d.spawn_acc.min(5.0);

            // Every so often a big horde rushes in from a single side.
            d.horde_timer -= dt;
            if d.horde_timer <= 0.0 && wave >= 3 {
                d.horde_timer = 14.0;
                let count = (10 + wave * 10).min(budget as u32);
                let a = rng.0.random::<f32>() * std::f32::consts::TAU;
                for _ in 0..count {
                    let spread = (rng.0.random::<f32>() - 0.5) * 0.6;
                    let r = view + rng.0.random_range(60.0..360.0);
                    let kind = pick_kind(wave, rng.0.random());
                    spawn_enemy(&mut commands, &assets, kind, spawn_point(cpos, a + spread, r), wave);
                }
            }
        }
    }
}

fn spawn_point(center: Vec2, angle: f32, dist: f32) -> Vec2 {
    let lim = MAP_HALF - 20.0;
    (center + Vec2::from_angle(angle) * dist).clamp(Vec2::splat(-lim), Vec2::splat(lim))
}
