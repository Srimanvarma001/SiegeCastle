//! SiegeCastle — drive a growing castle on wheels and survive the siege.
//!
//! One map, one castle that grows through 3 stages, and enemy waves that keep
//! getting bigger until the Warlord's citadel arrives.

// Bevy systems naturally take many parameters and long query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use rand::SeedableRng;
use rand::rngs::SmallRng;

mod assets;
mod castle;
mod combat;
mod debug;
mod enemy;
mod grid;
mod hud;
mod waves;
mod world;

/// The playable area is a square from -MAP_HALF to +MAP_HALF on both axes.
pub const MAP_HALF: f32 = 3000.0;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Waiting for assets to be created.
    #[default]
    Boot,
    /// Builds a fresh run, then switches to `Playing`.
    Starting,
    Playing,
    Paused,
    GameOver,
    Victory,
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
    pub villages: u32,
}

/// Big message in the middle of the screen.
#[derive(Resource, Default)]
pub struct Banner {
    pub text: String,
    pub timer: f32,
}

impl Banner {
    pub fn show(&mut self, text: impl Into<String>, secs: f32) {
        self.text = text.into();
        self.timer = secs;
    }
}

/// Camera shake strength in world units. Decays every frame.
#[derive(Resource, Default)]
pub struct Shake(pub f32);

fn main() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(7);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SiegeCastle".into(),
                resolution: WindowResolution::new(1280, 720),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.10, 0.13, 0.09)))
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
            castle::CastlePlugin,
            enemy::EnemyPlugin,
            combat::CombatPlugin,
            waves::WavePlugin,
            hud::HudPlugin,
            debug::DebugPlugin,
        ))
        .add_systems(OnEnter(GameState::Starting), start_run)
        .add_systems(Update, (pause_and_restart, tick_run_time))
        .run();
}

/// Clears the previous run and hands over to `Playing`.
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
    banner.show("Defend the castle!", 2.5);
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
        GameState::Paused | GameState::GameOver | GameState::Victory
            if keys.just_pressed(KeyCode::KeyR) =>
        {
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
    }
    banner.timer = (banner.timer - dt).max(0.0);
    shake.0 = (shake.0 - shake.0 * 8.0 * dt - 2.0 * dt).max(0.0);
}
