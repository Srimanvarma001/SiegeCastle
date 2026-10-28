//! Testing helpers, all off by default. Turn them on with environment variables:
//!
//! - `SIEGE_AUTOPILOT=1`       castle drives itself in a big loop and fires barrages
//! - `SIEGE_SHOTS=5,60,120`    save screenshots at these run times (seconds)
//! - `SIEGE_SHOT_DIR=path`     where screenshots go (default: `screenshots`)
//! - `SIEGE_EXIT_AFTER=180`    quit after this many seconds of play
//! - `SIEGE_SPEED=3`           run the game clock faster
//! - `SIEGE_LOG=1`             print a status line every 5 seconds
//! - `SIEGE_START_WAVE=10`    jump straight to a wave (castle starts fully grown)
//! - `SIEGE_GOD=1`             castle cannot be destroyed
//!
//! F12 always saves a screenshot.

use crate::castle::Castle;
use crate::enemy::{Boss, Enemy};
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
            start_wave: env("SIEGE_START_WAVE").and_then(|v| v.parse().ok()),
            god: env("SIEGE_GOD").is_some(),
        };
        if env("SIEGE_AUTOPILOT").is_some() {
            app.insert_resource(Autopilot);
        }
        app.insert_resource(cfg)
            .add_systems(Startup, apply_speed)
            .add_systems(Update, (debug_tick, exit_soon, cheats.run_if(in_state(GameState::Playing))));
    }
}

/// Counts down frames, then quits (gives the final screenshot time to save).
#[derive(Resource)]
struct ExitSoon(u32);

/// When present, the castle drives itself.
#[derive(Resource)]
pub struct Autopilot;

impl Autopilot {
    /// Circle the map center (or a boss, if one is out) at a fixed distance, kiting the swarm.
    pub fn direction(&self, pos: Vec2, boss: Option<Vec2>) -> Vec2 {
        let (center, radius) = match boss {
            Some(b) => (b, 420.0),
            None => (Vec2::ZERO, 1100.0),
        };
        let rel = pos - center;
        let tangent = Vec2::new(-rel.y, rel.x).normalize_or(Vec2::X);
        let inward = -rel.normalize_or(Vec2::X) * ((rel.length() - radius) / 300.0).clamp(-1.0, 1.0);
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
    start_wave: Option<u32>,
    god: bool,
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
    castle: Query<(&Transform, &Castle)>,
    enemies: Query<(), With<Enemy>>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
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

    if cfg.log && (t >= cfg.next_log || (ended && cfg.next_log < f32::MAX)) {
        cfg.next_log = if ended { f32::MAX } else { t + 5.0 };
        if let Ok((ctf, c)) = castle.single() {
            let boss_info: Vec<String> = bosses
                .iter()
                .map(|(t, e)| format!("{:?} hp={:.0} dist={:.0}", e.kind, e.hp, t.translation.distance(ctf.translation)))
                .collect();
            let fps = diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0);
            info!(
                "t={:>5.0}s state={:?} wave={} stage={} hp={:.0}/{:.0} xp={} enemies={} kills={} fps={:.0} bosses={:?}",
                t,
                state.get(),
                waves.wave,
                c.stage + 1,
                c.hp,
                c.def().max_hp,
                c.xp,
                enemies.iter().count(),
                stats.kills,
                fps,
                boss_info
            );
        }
    }

    if let Some(limit) = cfg.exit_after
        && (t >= limit || ended)
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

/// Test-only shortcuts: jump to a wave and/or make the castle unbreakable.
fn cheats(
    cfg: Res<DebugConfig>,
    stats: Res<RunStats>,
    mut waves: ResMut<WaveDirector>,
    mut castle: Query<&mut Castle>,
) {
    let Ok(mut c) = castle.single_mut() else {
        return;
    };
    if let Some(w) = cfg.start_wave
        && waves.wave == 0
        && stats.time < 1.0
    {
        waves.wave = w.saturating_sub(1);
        c.xp = c.xp.max(2000);
    }
    if cfg.god {
        c.hp = c.def().max_hp;
    }
}
