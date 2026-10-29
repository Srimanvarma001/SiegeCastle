//! SiegeCastle: drive a castle on wheels, collect weapon modules and
//! artifacts, grow the castle through 3 stages, beat 5 bosses, then survive
//! an endless siege that gets exponentially harder.

// Bevy systems naturally take many parameters and long query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use rand::SeedableRng;
use rand::rngs::SmallRng;

mod anim;
mod art;
mod assets;
mod combat;
mod debug;
mod director;
mod enemy;
mod grid;
mod hud;
mod icons;
mod items;
mod models;
mod modules;
mod player;
mod previews;
mod progression;
mod texgen;
mod vfx;
mod world;

/// The battlefield is a square from -MAP_HALF to +MAP_HALF (world units).
pub const MAP_HALF: f32 = 220.0;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Waiting for models and materials to be built.
    #[default]
    Boot,
    /// Builds a fresh run, then switches to `Playing`.
    Starting,
    Playing,
    /// Level-up card screen. The world is frozen.
    Choosing,
    Paused,
    GameOver,
}

/// Order of the gameplay steps inside one frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Sim {
    Input,
    Spawn,
    Grid,
    Move,
    Combat,
    Resolve,
}

/// Everything that belongs to a single run. Removed on restart.
#[derive(Component)]
pub struct GameEntity;

#[derive(Resource)]
pub struct GameRng(pub SmallRng);

#[derive(Resource, Default)]
pub struct RunStats {
    pub kills: u32,
    pub time: f32,
    pub bosses_killed: u32,
    pub villages: u32,
    pub sheep: u32,
}

/// Big message in the middle of the screen.
#[derive(Resource, Default)]
pub struct Banner {
    pub text: String,
    pub sub: String,
    pub timer: f32,
}

impl Banner {
    pub fn show(&mut self, text: impl Into<String>, sub: impl Into<String>, secs: f32) {
        self.text = text.into();
        self.sub = sub.into();
        self.timer = secs;
    }
}

/// Camera shake strength. Decays every frame.
#[derive(Resource, Default)]
pub struct Shake(pub f32);

fn main() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(7);

    // Test option: a small window renders much faster without a GPU.
    let lowres = std::env::var("SIEGE_LOWRES").is_ok();
    let (w, h) = if lowres { (640, 360) } else { (1280, 720) };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SiegeCastle".into(),
                resolution: WindowResolution::new(w, h),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.20, 0.27, 0.18)))
        .insert_resource(GameRng(SmallRng::seed_from_u64(seed)))
        .init_resource::<RunStats>()
        .init_resource::<Banner>()
        .init_resource::<Shake>()
        .init_state::<GameState>()
        .configure_sets(
            Update,
            (Sim::Input, Sim::Spawn, Sim::Grid, Sim::Move, Sim::Combat, Sim::Resolve)
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_plugins((
            assets::AssetsPlugin,
            grid::GridPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            modules::ModulesPlugin,
            enemy::EnemyPlugin,
            combat::CombatPlugin,
            director::DirectorPlugin,
            progression::ProgressionPlugin,
            previews::PreviewsPlugin,
            hud::HudPlugin,
            debug::DebugPlugin,
        ))
        .add_plugins((anim::AnimPlugin, vfx::VfxPlugin))
        .add_systems(OnEnter(GameState::Starting), start_run)
        .add_systems(Update, (pause_and_restart, tick_run_time))
        .run();
}

fn start_run(
    mut commands: Commands,
    old: Query<Entity, With<GameEntity>>,
    mut stats: ResMut<RunStats>,
    mut banner: ResMut<Banner>,
    mut next: ResMut<NextState<GameState>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    *stats = RunStats::default();
    banner.show("Defend the realm!", "WASD drive   SHIFT nitro   Q E R SPACE abilities", 4.0);
    next.set(GameState::Playing);
}

fn pause_and_restart(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let pause = keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape);
    match state.get() {
        GameState::Playing if pause => next.set(GameState::Paused),
        GameState::Paused if pause => next.set(GameState::Playing),
        GameState::Paused | GameState::GameOver if keys.just_pressed(KeyCode::KeyR) => {
            next.set(GameState::Starting)
        }
        _ => {}
    }
}

fn tick_run_time(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut stats: ResMut<RunStats>,
    mut banner: ResMut<Banner>,
    mut shake: ResMut<Shake>,
) {
    let dt = time.delta_secs();
    if *state.get() == GameState::Playing {
        stats.time += dt;
        banner.timer = (banner.timer - dt).max(0.0);
    }
    shake.0 = (shake.0 - shake.0 * 7.0 * dt - 0.2 * dt).max(0.0);
}
