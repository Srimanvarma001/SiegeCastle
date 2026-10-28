//! Shared meshes and colors. Every entity reuses these handles, which lets
//! Bevy batch thousands of enemies into a few draw calls.

use crate::GameState;
use bevy::prelude::*;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_assets);
    }
}

#[derive(Resource)]
pub struct GameAssets {
    /// Circle with diameter 1. Scale the transform to set the size.
    pub circle: Handle<Mesh>,
    /// 1x1 square.
    pub square: Handle<Mesh>,
    /// 1x1 diamond.
    pub diamond: Handle<Mesh>,
    pub m: Mats,
}

pub struct Mats {
    pub grass: Handle<ColorMaterial>,
    pub grass_light: Handle<ColorMaterial>,
    pub tree: Handle<ColorMaterial>,
    pub tree_dark: Handle<ColorMaterial>,
    pub rock: Handle<ColorMaterial>,
    pub border: Handle<ColorMaterial>,
    pub wheel: Handle<ColorMaterial>,
    pub wood: Handle<ColorMaterial>,
    pub wood_light: Handle<ColorMaterial>,
    pub stone: Handle<ColorMaterial>,
    pub stone_light: Handle<ColorMaterial>,
    pub roof_red: Handle<ColorMaterial>,
    pub roof_blue: Handle<ColorMaterial>,
    pub gold: Handle<ColorMaterial>,
    pub archer: Handle<ColorMaterial>,
    pub cannon: Handle<ColorMaterial>,
    pub mage: Handle<ColorMaterial>,
    pub raider: Handle<ColorMaterial>,
    pub knight: Handle<ColorMaterial>,
    pub knight_trim: Handle<ColorMaterial>,
    pub ram: Handle<ColorMaterial>,
    pub boss: Handle<ColorMaterial>,
    pub boss_roof: Handle<ColorMaterial>,
    pub warlord: Handle<ColorMaterial>,
    pub flash: Handle<ColorMaterial>,
    pub arrow: Handle<ColorMaterial>,
    pub cannonball: Handle<ColorMaterial>,
    pub magic: Handle<ColorMaterial>,
    pub enemy_shot: Handle<ColorMaterial>,
    pub gem: Handle<ColorMaterial>,
    pub house: Handle<ColorMaterial>,
    pub house_roof: Handle<ColorMaterial>,
    pub spark: Handle<ColorMaterial>,
    pub smoke: Handle<ColorMaterial>,
    pub blood: Handle<ColorMaterial>,
    pub bar_bg: Handle<ColorMaterial>,
    pub bar_fill: Handle<ColorMaterial>,
}

fn create_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let mut c = |r: f32, g: f32, b: f32| mats.add(ColorMaterial::from_color(Color::srgb(r, g, b)));
    let m = Mats {
        grass: c(0.30, 0.46, 0.24),
        grass_light: c(0.36, 0.53, 0.28),
        tree: c(0.17, 0.33, 0.16),
        tree_dark: c(0.11, 0.24, 0.11),
        rock: c(0.48, 0.47, 0.44),
        border: c(0.20, 0.15, 0.11),
        wheel: c(0.16, 0.12, 0.09),
        wood: c(0.52, 0.34, 0.18),
        wood_light: c(0.68, 0.48, 0.27),
        stone: c(0.55, 0.56, 0.60),
        stone_light: c(0.72, 0.73, 0.76),
        roof_red: c(0.74, 0.22, 0.18),
        roof_blue: c(0.22, 0.38, 0.72),
        gold: c(0.95, 0.76, 0.22),
        archer: c(0.35, 0.62, 0.27),
        cannon: c(0.22, 0.22, 0.25),
        mage: c(0.58, 0.32, 0.85),
        raider: c(0.86, 0.24, 0.20),
        knight: c(0.42, 0.44, 0.50),
        knight_trim: c(0.75, 0.15, 0.15),
        ram: c(0.45, 0.28, 0.14),
        boss: c(0.30, 0.18, 0.32),
        boss_roof: c(0.60, 0.10, 0.20),
        warlord: c(0.14, 0.10, 0.12),
        flash: c(1.0, 1.0, 1.0),
        arrow: c(0.98, 0.90, 0.55),
        cannonball: c(0.08, 0.08, 0.08),
        magic: c(0.80, 0.55, 1.0),
        enemy_shot: c(1.0, 0.45, 0.10),
        gem: c(0.35, 0.90, 1.0),
        house: c(0.78, 0.66, 0.45),
        house_roof: c(0.62, 0.25, 0.16),
        spark: c(1.0, 0.80, 0.30),
        smoke: c(0.35, 0.33, 0.30),
        blood: c(0.60, 0.10, 0.08),
        bar_bg: c(0.10, 0.05, 0.05),
        bar_fill: c(0.90, 0.20, 0.20),
    };

    commands.insert_resource(GameAssets {
        circle: meshes.add(Circle::new(0.5)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        diamond: meshes.add(Rhombus::new(1.0, 1.0)),
        m,
    });
    next.set(GameState::Starting);
}

/// A flat colored shape at `pos` with the given size (diameter for circles).
pub fn shape(
    mesh: &Handle<Mesh>,
    mat: &Handle<ColorMaterial>,
    pos: Vec3,
    size: Vec2,
) -> (Mesh2d, MeshMaterial2d<ColorMaterial>, Transform) {
    (
        Mesh2d(mesh.clone()),
        MeshMaterial2d(mat.clone()),
        Transform::from_translation(pos).with_scale(size.extend(1.0)),
    )
}
