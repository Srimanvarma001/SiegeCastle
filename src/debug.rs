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
//! - `SIEGE_BOSS=3`          spawn boss 1-5 near the castle right away
//! - `SIEGE_STILL=1`         with the autopilot: stand still (for screenshots)
//! - `SIEGE_LOWRES=1`        small window (fast tests without a GPU)
//! - `SIEGE_PROFILE=1`       with `SIEGE_LOG`, also print main-thread and
//!   render-thread time per frame and the slowest GPU passes
//! - `SIEGE_VSYNC=0`         turn vsync off (to measure the uncapped frame rate)
//! - `SIEGE_STAGE=3`         start at this castle stage (1-3)
//! - `SIEGE_VARIANT=movement` growth path for stages 2 and 3 (`defense` or `movement`)
//! - `SIEGE_LOADOUT=ballista,mortar,loft,ram,mines,cannons` start with these weapons
//! - `SIEGE_CAM=0.3,40,35`   close-up camera: zoom, turn and tilt (degrees); turn 0 looks
//!   from behind the castle, 90 from its right, 180 from the front
//! - `SIEGE_ARMORY=1`        every weapon on the grass in front of the castle, firing on a
//!   loop (the close-up camera then looks at them)
//!
//! Graphics presets have their own options, see `graphics.rs`.
//! F12 always saves a screenshot.

use crate::art::{p2, w3, Variant};
use crate::director::{Director, BOSSES};
use crate::enemy::{Boss, Enemy};
use crate::items::{Loadout, ModuleKind, OwnedModule, Stats};
use crate::modules::AutoAbilities;
use crate::player::{Castle, DriveInput};
use crate::progression::{xp_needed, Progress};
use crate::weapons::{spawn_rig, WeaponAnim, WeaponFired};
use crate::{GameEntity, GameRng, GameState, RunStats, Sim};
use crate::graphics::MainCamera;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::renderer::render_system;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::{Render, RenderApp, RenderSystems};
use rand::Rng;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

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
            boss: env("SIEGE_BOSS").and_then(|v| v.parse().ok()),
            stage: env("SIEGE_STAGE").and_then(|v| v.parse::<usize>().ok()).map(|s| s.clamp(1, 3) - 1),
            variant: match env("SIEGE_VARIANT").as_deref() {
                Some("movement") => Variant::Movement,
                _ => Variant::Defense,
            },
            loadout: env("SIEGE_LOADOUT").map(|v| v.split(',').filter_map(module_by_name).collect()),
            armory: env("SIEGE_ARMORY").is_some(),
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
        if env("SIEGE_PROFILE").is_some() {
            let render_cpu = RenderCpu::default();
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin)
                .init_resource::<FrameCpu>()
                .insert_resource(render_cpu.clone())
                .add_systems(First, |mut f: ResMut<FrameCpu>| f.start = Some(Instant::now()))
                .add_systems(Last, |mut f: ResMut<FrameCpu>| {
                    if let Some(start) = f.start.take() {
                        f.avg_ms = smooth(f.avg_ms, start.elapsed().as_secs_f32() * 1000.0);
                    }
                });
            // Render thread: preparing the frame, then encoding, submitting and
            // presenting it (which waits for the GPU and vsync when they lag).
            if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
                render_app
                    .insert_resource(render_cpu)
                    .init_resource::<RenderMarks>()
                    .add_systems(
                        Render,
                        (|mut m: ResMut<RenderMarks>| m.start = Some(Instant::now())).in_set(RenderSystems::ExtractCommands),
                    )
                    .add_systems(
                        Render,
                        (|mut m: ResMut<RenderMarks>| m.submit = Some(Instant::now()))
                            .in_set(RenderSystems::Render)
                            .before(render_system),
                    )
                    .add_systems(
                        Render,
                        (|mut m: ResMut<RenderMarks>, cpu: Res<RenderCpu>| {
                            if let (Some(start), Some(submit)) = (m.start.take(), m.submit.take()) {
                                let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
                                let now = Instant::now();
                                cpu.store(&cpu.prepare, ms(start, submit));
                                cpu.store(&cpu.submit, ms(submit, now));
                            }
                        })
                        .in_set(RenderSystems::PostCleanup),
                    );
            }
        }
        if let Some(cam) = env("SIEGE_CAM") {
            let v: Vec<f32> = cam.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            let get = |i: usize, d: f32| v.get(i).copied().unwrap_or(d);
            app.insert_resource(CloseUp {
                zoom: get(0, 0.3),
                turn: get(1, 0.0).to_radians(),
                tilt: get(2, 65.0).to_radians(),
                armory: env("SIEGE_ARMORY").is_some(),
            })
                .add_systems(
                    PostUpdate,
                    close_up.after(crate::world::follow_camera).before(bevy::transform::TransformSystems::Propagate),
                );
        }
        if env("SIEGE_ARMORY").is_some() {
            app.add_systems(Update, armory_fire.in_set(Sim::Combat).run_if(in_state(GameState::Playing)));
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
    boss: Option<usize>,
    stage: Option<usize>,
    variant: Variant,
    loadout: Option<Vec<ModuleKind>>,
    armory: bool,
    next_log: f32,
    manual_shots: u32,
    applied: bool,
}

#[derive(Resource)]
struct ExitSoon(u32);

/// Main-thread time per frame (game systems, without rendering), smoothed.
#[derive(Resource, Default)]
struct FrameCpu {
    start: Option<Instant>,
    avg_ms: f32,
}

/// Render-thread times, written in the render world and read in the main
/// world. Milliseconds stored as f32 bits.
#[derive(Resource, Clone, Default)]
struct RenderCpu {
    prepare: Arc<AtomicU32>,
    submit: Arc<AtomicU32>,
}

impl RenderCpu {
    fn store(&self, slot: &AtomicU32, ms: f32) {
        let old = f32::from_bits(slot.load(Ordering::Relaxed));
        slot.store(smooth(old, ms).to_bits(), Ordering::Relaxed);
    }

    fn load(slot: &AtomicU32) -> f32 {
        f32::from_bits(slot.load(Ordering::Relaxed))
    }
}

#[derive(Resource, Default)]
struct RenderMarks {
    start: Option<Instant>,
    submit: Option<Instant>,
}

/// Exponential moving average over roughly the last 20 frames.
fn smooth(avg: f32, sample: f32) -> f32 {
    if avg == 0.0 { sample } else { avg * 0.95 + sample * 0.05 }
}

/// Sorts mesh entities by what they belong to, for the profiler.
#[derive(bevy::ecs::system::SystemParam)]
struct Census<'w, 's> {
    meshes: Query<'w, 's, (Entity, &'static ViewVisibility), With<Mesh3d>>,
    fx_kinds: Query<'w, 's, &'static crate::vfx::Fx>,
    parents: Query<'w, 's, &'static ChildOf>,
    fx: Query<'w, 's, (), With<crate::vfx::Fx>>,
    troops: Query<'w, 's, (), (With<Enemy>, Without<Boss>)>,
    bosses: Query<'w, 's, (), Or<(With<Boss>, With<crate::boss::Wreck>)>>,
    coins: Query<'w, 's, (), With<crate::combat::Coin>>,
    corpses: Query<'w, 's, (), With<crate::enemy::Corpse>>,
    villages: Query<'w, 's, (), With<crate::world::Village>>,
    sheep: Query<'w, 's, (), With<crate::world::Sheep>>,
    castle: Query<'w, 's, (), With<Castle>>,
}

impl Census<'_, '_> {
    fn owner(&self, mut e: Entity) -> &'static str {
        loop {
            if self.fx.contains(e) {
                return "effects";
            } else if self.troops.contains(e) {
                return "troops";
            } else if self.bosses.contains(e) {
                return "bosses";
            } else if self.coins.contains(e) {
                return "coins";
            } else if self.corpses.contains(e) {
                return "corpses";
            } else if self.villages.contains(e) {
                return "farms";
            } else if self.sheep.contains(e) {
                return "sheep";
            } else if self.castle.contains(e) {
                return "castle";
            }
            match self.parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => return "world/other",
            }
        }
    }

    fn count(&self, meshes: impl Iterator<Item = Entity>) -> String {
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for e in meshes {
            let o = self.owner(e);
            match counts.iter_mut().find(|(n, _)| *n == o) {
                Some((_, c)) => *c += 1,
                None => counts.push((o, 1)),
            }
        }
        counts.sort_by_key(|c| std::cmp::Reverse(c.1));
        counts.iter().map(|(n, c)| format!("{n}={c}")).collect::<Vec<_>>().join(" ")
    }
}

/// Camera override for looking at the castle up close.
#[derive(Resource)]
struct CloseUp {
    zoom: f32,
    turn: f32,
    tilt: f32,
    armory: bool,
}

/// Where the armory weapons stand (ground plane), left to right.
fn armory_spot(i: usize) -> Vec2 {
    Vec2::new(-7.5 + i as f32 * 3.0, 16.0)
}

/// Fires every armory weapon in turn, so their animations can be watched.
fn armory_fire(time: Res<Time>, mut fired: MessageWriter<WeaponFired>, mut clock: Local<f32>) {
    let before = *clock;
    *clock += time.delta_secs();
    for (i, kind) in ModuleKind::ALL.iter().enumerate() {
        let phase = |t: f32| ((t + i as f32 * 0.27) / 1.9).floor();
        if phase(*clock) > phase(before) {
            let target = Some(armory_spot(i) + Vec2::new(-6.0 + i as f32 * 2.5, 12.0));
            fired.write(WeaponFired { kind: *kind, target, side: 0.0, reload: 1.6 });
        }
    }
}

fn module_by_name(name: &str) -> Option<ModuleKind> {
    Some(match name.trim() {
        "ballista" => ModuleKind::Ballista,
        "mortar" | "bombard" => ModuleKind::Mortar,
        "loft" | "archers" => ModuleKind::ArrowLoft,
        "ram" => ModuleKind::Ram,
        "mines" | "chute" => ModuleKind::MineLayer,
        "cannons" | "broadside" => ModuleKind::Broadside,
        _ => return None,
    })
}

fn close_up(
    cfg: Res<CloseUp>,
    castle: Query<&Transform, (With<Castle>, Without<MainCamera>)>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let (Ok(c), Ok(mut tf)) = (castle.single(), cam.single_mut()) else {
        return;
    };
    let d = 43.0 * cfg.zoom;
    let (look, rot) = if cfg.armory {
        (w3(armory_spot(0).lerp(armory_spot(5), 0.5), 0.6), crate::art::yaw(std::f32::consts::FRAC_PI_2))
    } else {
        (c.translation + Vec3::Y * 2.5, c.rotation)
    };
    let dir = Vec3::new(-cfg.turn.cos(), 0.0, cfg.turn.sin()) * cfg.tilt.cos() + Vec3::Y * cfg.tilt.sin();
    *tf = Transform::from_translation(look + rot * dir * d).looking_at(look, Vec3::Y);
}

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
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Boss>)>,
) {
    let Ok((tf, c)) = castle.single() else {
        return;
    };
    if std::env::var("SIEGE_STILL").is_ok() {
        input.dir = Vec2::ZERO;
        input.nitro = false;
        return;
    }
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
        .map(|(t, e)| (p2(t.translation), e.radius + e.half_len))
        .min_by(|a, b| a.0.distance_squared(pos).total_cmp(&b.0.distance_squared(pos)));
    let (center, radius) = match boss {
        // Circle bosses just outside their reach.
        Some((b, size)) => (b, size + 20.0),
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
    mut loadout: ResMut<Loadout>,
    stats: Res<Stats>,
    mut castle: Query<&mut Castle>,
    positions: Query<&Transform, With<Castle>>,
) {
    if !cfg.applied {
        cfg.applied = true;
        let c = positions.single().map(|t| p2(t.translation)).unwrap_or_default();
        if cfg.gallery {
            for (i, kind) in BOSSES.iter().enumerate() {
                let p = c + Vec2::from_angle(i as f32 / 5.0 * std::f32::consts::TAU + 0.3) * 95.0;
                crate::enemy::spawn_boss(&mut commands, &assets, *kind, p, (c - p).to_angle(), director.strength, i);
            }
            director.boss_index = BOSSES.len();
            director.boss_timer = f32::INFINITY;
        }
        if let Some(n) = cfg.boss
            && let Some(&kind) = BOSSES.get(n.saturating_sub(1))
        {
            let ext = kind.stats().radius + kind.half_len();
            let p = c + Vec2::from_angle(0.6) * (ext + 26.0);
            crate::enemy::spawn_boss(&mut commands, &assets, kind, p, (c - p).to_angle(), director.strength, 0);
            director.boss_index = n.min(BOSSES.len());
            director.boss_timer = f32::INFINITY;
        }
        if let Some(lvl) = cfg.start_level {
            let total: f32 = (1..lvl).map(xp_needed).sum();
            progress.add_xp(total);
        }
        if let Some(stage) = cfg.stage {
            loadout.stage = stage;
            loadout.variant = if stage == 0 { Variant::Base } else { cfg.variant };
        }
        if let Some(kinds) = &cfg.loadout {
            loadout.modules = kinds.iter().map(|&kind| OwnedModule { kind, level: 1 }).collect();
        }
        if cfg.armory {
            for (i, kind) in ModuleKind::ALL.iter().enumerate() {
                let rig = &assets.rigs[kind];
                let mut gun = commands.spawn((
                    GameEntity,
                    WeaponAnim::new(*kind, 1.0, rig),
                    Transform::from_translation(w3(armory_spot(i), 0.0)).with_rotation(crate::art::yaw(std::f32::consts::FRAC_PI_2)),
                    Visibility::Visible,
                ));
                let owner = gun.id();
                gun.with_children(|q| spawn_rig(q, rig, owner));
            }
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
    profile: Option<Res<FrameCpu>>,
    render_cpu: Option<Res<RenderCpu>>,
    census: Census,
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
            if let Some(profile) = &profile {
                let mut passes: Vec<(String, f64)> = diagnostics
                    .iter()
                    .filter(|d| d.path().as_str().ends_with("/elapsed_gpu"))
                    .filter_map(|d| Some((d.path().as_str().trim_end_matches("/elapsed_gpu").to_string(), d.smoothed()?)))
                    .collect();
                passes.sort_by(|a, b| b.1.total_cmp(&a.1));
                let gpu: Vec<String> = passes.iter().take(6).map(|(name, ms)| format!("{name}={ms:.1}")).collect();
                let (prepare, submit) = render_cpu
                    .as_ref()
                    .map_or((0.0, 0.0), |r| (RenderCpu::load(&r.prepare), RenderCpu::load(&r.submit)));
                info!(
                    "profile: main_thread={:.1}ms render_prepare={prepare:.1}ms render_submit={submit:.1}ms gpu_ms: {}",
                    profile.avg_ms,
                    gpu.join(" ")
                );
                // What the render thread has to prepare every frame.
                let visible = census.meshes.iter().filter(|(_, v)| v.get()).count();
                info!("profile: visible meshes by owner: {}", census.count(census.meshes.iter().filter(|(_, v)| v.get()).map(|(e, _)| e)));
                let mut kinds: Vec<(String, usize)> = Vec::new();
                for f in &census.fx_kinds {
                    let name = format!("{:?}", f.kind());
                    let name = name.split(' ').next().unwrap_or("").to_string();
                    match kinds.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, c)) => *c += 1,
                        None => kinds.push((name, 1)),
                    }
                }
                kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
                let top: Vec<String> = kinds.iter().take(6).map(|(n, c)| format!("{n}={c}")).collect();
                info!("profile: meshes={} visible={visible} effects={} ({})", census.meshes.iter().count(), census.fx_kinds.iter().count(), top.join(" "));
            }
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
