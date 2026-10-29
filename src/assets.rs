//! Builds every model and material once at startup.

use crate::GameState;
use crate::art::{self, Variant};
use crate::enemy::EnemyKind;
use crate::items::ModuleKind;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::Face;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_assets);
    }
}

#[derive(Resource)]
pub struct GameAssets {
    /// Castle body per (stage, variant).
    pub castle: HashMap<(usize, Variant), Handle<Mesh>>,
    pub modules: HashMap<ModuleKind, Handle<Mesh>>,
    pub enemies: HashMap<EnemyKind, Handle<Mesh>>,
    pub villages: Vec<Handle<Mesh>>,
    pub sheep: Handle<Mesh>,
    pub coin: Handle<Mesh>,
    pub arrow: Handle<Mesh>,
    pub enemy_arrow: Handle<Mesh>,
    pub mine: Handle<Mesh>,
    pub pointer: Handle<Mesh>,
    /// Plain unit shapes for effects.
    pub ball: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub disc: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub m: Mats,
}

pub struct Mats {
    /// White base color: models use their vertex colors.
    pub vc: Handle<StandardMaterial>,
    pub flash: Handle<StandardMaterial>,
    pub fire: Handle<StandardMaterial>,
    pub spark: Handle<StandardMaterial>,
    pub smoke: Handle<StandardMaterial>,
    pub smoke_dark: Handle<StandardMaterial>,
    pub track: Handle<StandardMaterial>,
    pub scorch: Handle<StandardMaterial>,
    pub dirt: Handle<StandardMaterial>,
    pub debris: Handle<StandardMaterial>,
    pub blood: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub telegraph: Handle<StandardMaterial>,
    pub cannonball: Handle<StandardMaterial>,
    pub stone: Handle<StandardMaterial>,
    pub coin_glow: Handle<StandardMaterial>,
}

fn create_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let mut castle = HashMap::default();
    castle.insert((0, Variant::Base), meshes.add(art::castle_model(0, Variant::Base)));
    for stage in 1..3 {
        for v in [Variant::Defense, Variant::Movement] {
            castle.insert((stage, v), meshes.add(art::castle_model(stage, v)));
        }
    }
    let modules = ModuleKind::ALL.iter().map(|&k| (k, meshes.add(art::module_model(k)))).collect();
    let mut enemies = HashMap::default();
    for kind in EnemyKind::ALL {
        let mesh = match kind {
            EnemyKind::Footman => art::footman_model(),
            EnemyKind::Archer => art::archer_model(),
            EnemyKind::Knight => art::knight_model(),
            EnemyKind::Cart => art::cart_model(),
            EnemyKind::SiegeTower => art::siege_tower_model(),
            EnemyKind::WarWagon => art::war_wagon_model(),
            EnemyKind::Juggernaut => art::juggernaut_model(),
            EnemyKind::Trebuchet => art::trebuchet_model(),
            EnemyKind::Warlord => art::warlord_model(),
        };
        enemies.insert(kind, meshes.add(mesh));
    }

    let solid = |c: Color| StandardMaterial { base_color: c, perceptual_roughness: 0.95, ..default() };
    let glow = |c: Color, e: LinearRgba| StandardMaterial {
        base_color: c,
        emissive: e,
        unlit: true,
        ..default()
    };
    let m = Mats {
        vc: mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, ..default() }),
        flash: mats.add(glow(Color::WHITE, LinearRgba::rgb(4.0, 4.0, 4.0))),
        fire: mats.add(glow(Color::srgb(1.0, 0.75, 0.3), LinearRgba::rgb(12.0, 5.0, 1.2))),
        spark: mats.add(glow(Color::srgb(1.0, 0.9, 0.5), LinearRgba::rgb(6.0, 4.0, 1.5))),
        smoke: mats.add(solid(Color::srgb(0.93, 0.92, 0.90))),
        smoke_dark: mats.add(solid(Color::srgb(0.32, 0.29, 0.27))),
        track: mats.add(solid(Color::srgb(0.70, 0.45, 0.30))),
        scorch: mats.add(solid(Color::srgb(0.36, 0.25, 0.18))),
        dirt: mats.add(solid(Color::srgb(0.64, 0.50, 0.33))),
        debris: mats.add(solid(Color::srgb(0.55, 0.38, 0.24))),
        blood: mats.add(solid(Color::srgb(0.45, 0.20, 0.14))),
        water: mats.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.27, 0.27),
            perceptual_roughness: 0.25,
            cull_mode: None::<Face>,
            ..default()
        }),
        telegraph: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.22, 0.12, 0.45),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        cannonball: mats.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.12, 0.13),
            perceptual_roughness: 0.4,
            ..default()
        }),
        stone: mats.add(solid(Color::srgb(0.55, 0.53, 0.50))),
        coin_glow: mats.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::rgb(0.6, 0.35, 0.05),
            perceptual_roughness: 0.4,
            ..default()
        }),
    };

    commands.insert_resource(art::build_icons(&mut images));
    commands.insert_resource(GameAssets {
        castle,
        modules,
        enemies,
        villages: (0..4).map(|i| meshes.add(art::village_model(i))).collect(),
        sheep: meshes.add(art::sheep_model()),
        coin: meshes.add(art::coin_model()),
        arrow: meshes.add(art::arrow_model(art::pal::wood_light())),
        enemy_arrow: meshes.add(art::arrow_model(art::pal::wood_dark())),
        mine: meshes.add(art::mine_model()),
        pointer: meshes.add(art::pointer_model()),
        ball: meshes.add(Sphere::new(0.5).mesh().ico(1).expect("ico")),
        cube: meshes.add(Cuboid::from_size(Vec3::ONE)),
        disc: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(14).build()),
        ring: meshes.add(Torus::new(0.85, 1.0)),
        m,
    });
    next.set(GameState::Starting);
}
