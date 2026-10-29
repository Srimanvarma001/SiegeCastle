//! Testing helpers, all off by default. Turn them on with environment variables:
//!
//! - `SIEGE_AUTOPILOT=1`     castle drives itself, uses abilities, picks cards
//! - `SIEGE_SHOTS=5,60,120`  save screenshots at these run times (seconds)
//! - `SIEGE_SHOT_CHOICE=1`   also save one screenshot of the first card screen
//! - `SIEGE_SHOT_DIR=path`   where screenshots go (default: `screenshots`)
//! - `SIEGE_EXIT_AFTER=180`  quit after this many seconds of play
//! - `SIEGE_SPEED=2`         run the game clock faster
//! - `SIEGE_LOG=1`           print a status line every 5 seconds
//! - `SIEGE_START_LEVEL=8`   start with this many levels' worth of XP
//! - `SIEGE_BOSS_IN=10`      first boss arrives after this many seconds
//! - `SIEGE_ENDLESS=1`       start directly in endless mode
//! - `SIEGE_GOD=1`           castle cannot be destroyed
//! - `SIEGE_GALLERY=1`       spawn all five bosses around the castle at once
//! - `SIEGE_LOWRES=1`        small window, no shadows (fast tests without a GPU)
//!
//! F12 always saves a screenshot.

use crate::art::p2;
use crate::director::{Director, BOSSES};
use crate::enemy::{Boss, Enemy};
use crate::items::{Loadout, Stats};
use crate::modules::AutoAbilities;
use crate::player::{Castle, DriveInput};
use crate::progression::{xp_needed, Progress};
use crate::{GameRng, GameState, RunStats, Sim};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use rand::Rng;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let cfg = DebugConfig {
            shots: env("SIEGE_SHOTS")
                .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
                .unwrap_or_default(),
            shot_choice: env("SIEGE_SHOT_CHOICE").is_some(),
            shot_dir: env("SIEGE_SHOT_DIR").unwrap_or_else(|| "screenshots".into()),
            exit_after: env("SIEGE_EXIT_AFTER").and_then(|v| v.parse().ok()),
            speed: env("SIEGE_SPEED").and_then(|v| v.parse().ok()),
            log: env("SIEGE_LOG").is_some(),
            start_level: env("SIEGE_START_LEVEL").and_then(|v| v.parse().ok()),
            boss_in: env("SIEGE_BOSS_IN").and_then(|v| v.parse().ok()),
            endless: env("SIEGE_ENDLESS").is_some(),
            god: env("SIEGE_GOD").is_some(),
            gallery: env("SIEGE_GALLERY").is_some(),
            next_log: 0.0,
            manual_shots: 0,
            applied: false,
        };
        if env("SIEGE_AUTOPILOT").is_some() {
            app.insert_resource(Autopilot)
                .insert_resource(AutoPick)
                .insert_resource(AutoAbilities(true))
                .add_systems(
                    Update,
                    autopilot
                        .in_set(Sim::Input)
                        .after(crate::player::read_input)
                        .before(crate::player::drive),
                );
        }
        app.insert_resource(cfg)
            .add_systems(Startup, apply_speed)
            .add_systems(OnEnter(GameState::Starting), |mut cfg: ResMut<DebugConfig>| cfg.applied = false)
            .add_systems(Update, (debug_tick, exit_soon, choice_shot.run_if(in_state(GameState::Choosing))))
            .add_systems(Update, cheats.run_if(in_state(GameState::Playing)));
    }
}

/// The castle drives itself.
#[derive(Resource)]
pub struct Autopilot;

/// Level-up cards are picked automatically.
#[derive(Resource)]
pub struct AutoPick;

#[derive(Resource)]
struct DebugConfig {
    shots: Vec<f32>,
    shot_choice: bool,
    shot_dir: String,
    exit_after: Option<f32>,
    speed: Option<f32>,
    log: bool,
    start_level: Option<u32>,
    boss_in: Option<f32>,
    endless: bool,
    god: bool,
    gallery: bool,
    next_log: f32,
    manual_shots: u32,
    applied: bool,
}

#[derive(Resource)]
struct ExitSoon(u32);

fn apply_speed(cfg: Res<DebugConfig>, mut time: ResMut<Time<Virtual>>) {
    if let Some(s) = cfg.speed {
        time.set_relative_speed(s);
        time.set_max_delta(std::time::Duration::from_millis(100));
    }
}

/// Circle the map (or the nearest boss), kiting the swarm.
fn autopilot(
    mut input: ResMut<DriveInput>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    mut rng: ResMut<GameRng>,
    castle: Query<(&Transform, &Castle)>,
    bosses: Query<&Transform, With<Boss>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Boss>)>,
) {
    let Ok((tf, c)) = castle.single() else {
        return;
    };
    let pos = p2(tf.translation);
    let (mut sum, mut n) = (Vec2::ZERO, 0.0);
    for t in &enemies {
        let p = p2(t.translation);
        if p.distance(pos) < 35.0 {
            sum += p;
            n += 1.0;
        }
    }
    let boss = bosses
        .iter()
        .map(|t| p2(t.translation))
        .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
    let (center, radius) = match boss {
        Some(b) => (b, 24.0),
        // Ram through crowds when we have a ram, otherwise circle them.
        None if n > 6.0 => (sum / n, if loadout.module(crate::items::ModuleKind::Ram).is_some() { 9.0 } else { 20.0 }),
        None => (Vec2::ZERO, 50.0),
    };
    let rel = pos - center;
    let tangent = Vec2::new(-rel.y, rel.x).normalize_or(Vec2::X);
    let inward = -rel.normalize_or(Vec2::X) * ((rel.length() - radius) / 12.0).clamp(-1.0, 1.0);
    input.dir = (tangent + inward).normalize_or(Vec2::X);
    input.nitro = c.hp < stats.max_hp * 0.4 || rng.0.random::<f32>() < 0.02;
}

fn cheats(
    mut commands: Commands,
    assets: Res<crate::assets::GameAssets>,
    mut cfg: ResMut<DebugConfig>,
    mut progress: ResMut<Progress>,
    mut director: ResMut<Director>,
    stats: Res<Stats>,
    mut castle: Query<&mut Castle>,
    positions: Query<&Transform, With<Castle>>,
) {
    if !cfg.applied {
        cfg.applied = true;
        if cfg.gallery {
            let c = positions.single().map(|t| p2(t.translation)).unwrap_or_default();
            for (i, kind) in BOSSES.iter().enumerate() {
                let p = c + Vec2::from_angle(i as f32 / 5.0 * std::f32::consts::TAU + 0.3) * 30.0;
                crate::enemy::spawn_enemy(&mut commands, &assets, *kind, p, (c - p).to_angle(), Vec2::ZERO, director.strength);
            }
            director.boss_index = BOSSES.len();
            director.boss_timer = f32::INFINITY;
        }
        if let Some(lvl) = cfg.start_level {
            let total: f32 = (1..lvl).map(xp_needed).sum();
            progress.add_xp(total);
        }
        if let Some(s) = cfg.boss_in {
            director.boss_timer = s;
        }
        if cfg.endless {
            director.endless = true;
            director.boss_index = BOSSES.len();
            director.boss_timer = 60.0;
        }
    }
    if cfg.god
        && let Ok(mut c) = castle.single_mut()
    {
        c.hp = stats.max_hp;
    }
}

fn take_shot(commands: &mut Commands, dir: &str, name: &str) {
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{dir}/{name}.png");
    info!("screenshot -> {path}");
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

fn choice_shot(mut commands: Commands, mut cfg: ResMut<DebugConfig>, mut frames: Local<u32>, time: Res<Time<Real>>) {
    if !cfg.shot_choice {
        return;
    }
    *frames += 1;
    if *frames > 2 && time.elapsed_secs() > 14.0 {
        cfg.shot_choice = false;
        let dir = cfg.shot_dir.clone();
        take_shot(&mut commands, &dir, "choice");
    }
}

fn debug_tick(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<RunStats>,
    state: Res<State<GameState>>,
    progress: Res<Progress>,
    loadout: Res<Loadout>,
    director: Res<Director>,
    diagnostics: Res<DiagnosticsStore>,
    castle: Query<(&Transform, &Castle)>,
    enemies: Query<(), With<Enemy>>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    mut cfg: ResMut<DebugConfig>,
) {
    let t = stats.time;
    if keys.just_pressed(KeyCode::F12) {
        cfg.manual_shots += 1;
        let (dir, n) = (cfg.shot_dir.clone(), cfg.manual_shots);
        take_shot(&mut commands, &dir, &format!("shot_{n:03}"));
    }
    if let Some(&next) = cfg.shots.first()
        && t >= next
        && *state.get() == GameState::Playing
    {
        cfg.shots.remove(0);
        let dir = cfg.shot_dir.clone();
        take_shot(&mut commands, &dir, &format!("t{:04}", next as u32));
    }
    let ended = *state.get() == GameState::GameOver;
    if cfg.log && (t >= cfg.next_log || (ended && cfg.next_log < f32::MAX)) {
        cfg.next_log = if ended { f32::MAX } else { t + 5.0 };
        if let Ok((ctf, c)) = castle.single() {
            let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
            let boss: Vec<String> = bosses
                .iter()
                .map(|(bt, e)| format!("{:?} {:.0}hp d{:.0}", e.kind, e.hp, bt.translation.distance(ctf.translation)))
                .collect();
            let mods: Vec<String> = loadout.modules.iter().map(|m| format!("{:?}{}", m.kind, m.level)).collect();
            info!(
                "t={:>4.0} {:?} lvl={} stage={} hp={:.0} enemies={} kills={} bosses={} endless={} fps={:.0} mods={:?} arts={} boss={:?}",
                t,
                state.get(),
                progress.level,
                loadout.stage + 1,
                c.hp,
                enemies.iter().count(),
                stats.kills,
                director.defeated.len(),
                if director.endless { format!("{:.0}s", director.endless_time) } else { "no".into() },
                fps,
                mods,
                loadout.artifacts.values().sum::<u32>(),
                boss
            );
        }
    }
    if let Some(limit) = cfg.exit_after
        && (t >= limit || ended)
    {
        cfg.exit_after = None;
        cfg.shots.clear();
        let dir = cfg.shot_dir.clone();
        take_shot(&mut commands, &dir, "final");
        commands.insert_resource(ExitSoon(40));
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
