//! Testing helpers, all off by default. Turn them on with environment variables:
//!
//! - `SIEGE_AUTOPILOT=1`       castle drives itself in a big loop and fires barrages
//! - `SIEGE_SHOTS=5,60,120`    save screenshots at these run times (seconds)
//! - `SIEGE_SHOT_DIR=path`     where screenshots go (default: `screenshots`)
//! - `SIEGE_EXIT_AFTER=180`    quit after this many seconds of play
//! - `SIEGE_SPEED=3`           run the game clock faster
//! - `SIEGE_LOG=1`             print a status line every 5 seconds
//!
//! F12 always saves a screenshot.

use crate::castle::Castle;
use crate::enemy::Enemy;
use crate::waves::WaveDirector;
use crate::{GameState, RunStats};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let cfg = DebugConfig {
            shots: env("SIEGE_SHOTS")
                .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
                .unwrap_or_default(),
            shot_dir: env("SIEGE_SHOT_DIR").unwrap_or_else(|| "screenshots".into()),
            exit_after: env("SIEGE_EXIT_AFTER").and_then(|v| v.parse().ok()),
            speed: env("SIEGE_SPEED").and_then(|v| v.parse().ok()),
            log: env("SIEGE_LOG").is_some(),
            next_log: 0.0,
            manual_shots: 0,
        };
        if env("SIEGE_AUTOPILOT").is_some() {
            app.insert_resource(Autopilot);
        }
        app.insert_resource(cfg)
            .add_systems(Startup, apply_speed)
            .add_systems(Update, (debug_tick, exit_soon));
    }
}

/// Counts down frames, then quits (gives the final screenshot time to save).
#[derive(Resource)]
struct ExitSoon(u32);

/// When present, the castle drives itself.
#[derive(Resource)]
pub struct Autopilot;

impl Autopilot {
    /// Circle the map center at a fixed distance, kiting the swarm.
    pub fn direction(&self, pos: Vec2) -> Vec2 {
        let radius = 1100.0;
        let tangent = Vec2::new(-pos.y, pos.x).normalize_or(Vec2::X);
        let inward = -pos.normalize_or(Vec2::X) * ((pos.length() - radius) / 300.0).clamp(-1.0, 1.0);
        (tangent + inward).normalize_or(Vec2::X)
    }
}

#[derive(Resource)]
struct DebugConfig {
    shots: Vec<f32>,
    shot_dir: String,
    exit_after: Option<f32>,
    speed: Option<f32>,
    log: bool,
    next_log: f32,
    manual_shots: u32,
}

fn apply_speed(cfg: Res<DebugConfig>, mut time: ResMut<Time<Virtual>>) {
    if let Some(s) = cfg.speed {
        time.set_relative_speed(s);
        // Allow bigger steps so the fast clock is not capped by a slow frame rate.
        time.set_max_delta(std::time::Duration::from_millis(100));
    }
}

#[allow(clippy::too_many_arguments)]
fn debug_tick(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<RunStats>,
    state: Res<State<GameState>>,
    waves: Res<WaveDirector>,
    diagnostics: Res<DiagnosticsStore>,
    castle: Query<&Castle>,
    enemies: Query<(), With<Enemy>>,
    mut cfg: ResMut<DebugConfig>,
) {
    let t = stats.time;
    let mut take = |name: String, dir: &str| {
        let _ = std::fs::create_dir_all(dir);
        let path = format!("{dir}/{name}.png");
        info!("screenshot -> {path}");
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    };

    if keys.just_pressed(KeyCode::F12) {
        cfg.manual_shots += 1;
        take(format!("shot_{:03}", cfg.manual_shots), &cfg.shot_dir.clone());
    }
    if let Some(&next) = cfg.shots.first()
        && t >= next
    {
        cfg.shots.remove(0);
        let dir = cfg.shot_dir.clone();
        take(format!("t{:04}", next as u32), &dir);
    }
    let ended = matches!(state.get(), GameState::GameOver | GameState::Victory);

    if cfg.log && (t >= cfg.next_log || ended) {
        cfg.next_log = t + 5.0;
        if let Ok(c) = castle.single() {
            let fps = diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0);
            info!(
                "t={:>5.0}s state={:?} wave={} stage={} hp={:.0}/{:.0} xp={} enemies={} kills={} fps={:.0}",
                t,
                state.get(),
                waves.wave,
                c.stage + 1,
                c.hp,
                c.def().max_hp,
                c.xp,
                enemies.iter().count(),
                stats.kills,
                fps
            );
        }
    }

    if let Some(limit) = cfg.exit_after
        && (t >= limit || (ended && cfg.shots.is_empty()))
    {
        // Give the last screenshot a moment to save.
        cfg.exit_after = None;
        cfg.shots.clear();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(format!("{}/final.png", cfg.shot_dir)));
        commands.insert_resource(ExitSoon(30));
    }
}

fn exit_soon(mut commands: Commands, soon: Option<ResMut<ExitSoon>>, mut exit: MessageWriter<AppExit>) {
    let Some(mut s) = soon else {
        return;
    };
    if s.0 == 0 {
        exit.write(AppExit::Success);
        commands.remove_resource::<ExitSoon>();
    } else {
        s.0 -= 1;
    }
}
