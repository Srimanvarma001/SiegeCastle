//! Builds every texture, material and model once at startup.

use crate::GameState;
use crate::art::{self, Variant};
use crate::enemy::EnemyKind;
use crate::items::ModuleKind;
use crate::models::{self, Mat, Model, RigPart, Role};
use crate::texgen::{self, Surface};
use bevy::math::Affine2;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_assets);
    }
}

/// A model made of several meshes, one per material.
#[derive(Clone, Default)]
pub struct ModelHandle(pub Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);

impl ModelHandle {
    /// Spawns the pieces as children of the entity being built.
    pub fn spawn_parts(&self, p: &mut ChildSpawnerCommands, extra: impl Bundle + Clone) {
        for (mesh, mat) in &self.0 {
            p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::IDENTITY, extra.clone()));
        }
    }
}

/// A moving piece of a model, ready to spawn (see `anim::spawn_pieces`).
#[derive(Clone)]
pub struct PartHandle {
    pub role: Role,
    pub pivot: Vec3,
    pub rest: Quat,
    pub model: ModelHandle,
    pub parts: Vec<PartHandle>,
}

/// A model split into its static body and its moving pieces.
#[derive(Clone, Default)]
pub struct RigHandle {
    pub body: ModelHandle,
    pub parts: Vec<PartHandle>,
}

#[derive(Resource)]
pub struct GameAssets {
    /// Castle per (stage, variant), all in one piece (for portraits).
    pub castle: HashMap<(usize, Variant), ModelHandle>,
    /// The same castles with moving wheels and flags.
    pub castle_rigs: HashMap<(usize, Variant), RigHandle>,
    pub modules: HashMap<ModuleKind, ModelHandle>,
    pub module_rigs: HashMap<ModuleKind, RigHandle>,
    /// Troops: one vertex-colored mesh each.
    pub enemies: HashMap<EnemyKind, Handle<Mesh>>,
    /// Boss bodies. Their moving pieces are in `boss_parts`.
    pub bosses: HashMap<EnemyKind, ModelHandle>,
    pub boss_parts: HashMap<EnemyKind, Vec<PartHandle>>,
    pub vfx: crate::vfx::VfxAssets,
    pub villages: Vec<ModelHandle>,
    pub sheep: Handle<Mesh>,
    pub coin: ModelHandle,
    pub arrow: Handle<Mesh>,
    pub enemy_arrow: Handle<Mesh>,
    pub mine: Handle<Mesh>,
    pub pointer: Handle<Mesh>,
    /// Plain unit shapes for effects.
    pub ball: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub disc: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    /// Flat quad for wheel ruts (soft-edged road texture across it).
    pub rut: Handle<Mesh>,
    pub mats: HashMap<Mat, Handle<StandardMaterial>>,
    pub m: Mats,
}

impl GameAssets {
    pub fn mat(&self, m: Mat) -> Handle<StandardMaterial> {
        self.mats[&m].clone()
    }
}

static FONT_TITLE: std::sync::OnceLock<Handle<Font>> = std::sync::OnceLock::new();
static FONT_BODY: std::sync::OnceLock<Handle<Font>> = std::sync::OnceLock::new();

/// Text in the book serif (body) font.
pub fn body(s: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    let f = FONT_BODY.get().cloned().unwrap_or_default();
    (Text::new(s), TextFont::from_font_size(size).with_font(f), TextColor(color))
}

/// Text in the carved capitals (title) font.
pub fn title(s: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    let f = FONT_TITLE.get().cloned().unwrap_or_default();
    (Text::new(s), TextFont::from_font_size(size).with_font(f), TextColor(color))
}

#[derive(Clone)]
pub struct Fonts {
    /// Carved capitals for titles.
    pub title: Handle<Font>,
    /// Book serif for everything else.
    pub body: Handle<Font>,
}

pub struct Mats {
    /// Vertex-colored, for troops and small details.
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
    pub grass: Handle<StandardMaterial>,
    pub road: Handle<StandardMaterial>,
    /// Smoke at decreasing opacity, swapped in as a puff ages.
    pub smoke_fade: Vec<Handle<StandardMaterial>>,
    pub smoke_dark_fade: Vec<Handle<StandardMaterial>>,
}

/// Meters covered by one repeat of each texture.
fn tile_size(s: Surface) -> f32 {
    match s {
        Surface::Stone => 4.0,
        Surface::Slate => 2.4,
        Surface::Wood => 2.0,
        Surface::Iron => 2.0,
        Surface::Grass => 7.0,
        Surface::Mud => 3.0,
        Surface::Wheat => 3.0,
        Surface::Plaster => 3.0,
        Surface::Bark => 1.5,
    }
}

fn textured(tex: &texgen::SurfaceTextures, tile: f32, extra: StandardMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(tex.color.clone()),
        normal_map_texture: Some(tex.normal.clone()),
        metallic_roughness_texture: Some(tex.rough_metal.clone()),
        perceptual_roughness: 1.0,
        metallic: 1.0,
        uv_transform: Affine2::from_scale(Vec2::splat(1.0 / tile)),
        ..extra
    }
}

pub fn upload(meshes: &mut Assets<Mesh>, mats: &HashMap<Mat, Handle<StandardMaterial>>, model: Model) -> ModelHandle {
    ModelHandle(model.build().into_iter().map(|(m, mesh)| (meshes.add(mesh), mats[&m].clone())).collect())
}

/// Uploads a model keeping its moving pieces separate.
pub fn upload_rig(meshes: &mut Assets<Mesh>, mats: &HashMap<Mat, Handle<StandardMaterial>>, model: Model) -> RigHandle {
    fn part(meshes: &mut Assets<Mesh>, mats: &HashMap<Mat, Handle<StandardMaterial>>, p: RigPart) -> PartHandle {
        let (body, rig) = p.model.split();
        PartHandle {
            role: p.role,
            pivot: p.pivot,
            rest: p.rest,
            model: upload(meshes, mats, body),
            parts: rig.into_iter().map(|r| part(meshes, mats, r)).collect(),
        }
    }
    let (body, rig) = model.split();
    RigHandle { body: upload(meshes, mats, body), parts: rig.into_iter().map(|r| part(meshes, mats, r)).collect() }
}

pub fn create_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut next: ResMut<NextState<GameState>>,
) {
    // Surfaces.
    let mut tex: HashMap<Surface, texgen::SurfaceTextures> = HashMap::default();
    for s in [Surface::Stone, Surface::Slate, Surface::Wood, Surface::Iron, Surface::Grass, Surface::Mud, Surface::Wheat, Surface::Plaster, Surface::Bark] {
        tex.insert(s, texgen::build(s, &mut images));
    }
    let foliage = texgen::foliage(&mut images);
    let road_tex = texgen::road(&mut images);
    let mut mats: HashMap<Mat, Handle<StandardMaterial>> = HashMap::default();
    for m in Mat::ALL {
        let mat = match m.surface() {
            Some(s) => textured(&tex[&s], tile_size(s), StandardMaterial::default()),
            None => match m {
                Mat::Glass => StandardMaterial {
                    base_color: Color::srgb(0.05, 0.07, 0.09),
                    perceptual_roughness: 0.08,
                    reflectance: 0.6,
                    emissive: LinearRgba::rgb(0.06, 0.04, 0.015),
                    ..default()
                },
                Mat::Gold => StandardMaterial {
                    base_color: Color::srgb(0.95, 0.72, 0.30),
                    metallic: 1.0,
                    perceptual_roughness: 0.28,
                    ..default()
                },
                Mat::Leaf => StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, reflectance: 0.3, ..default() },
                Mat::Foliage => StandardMaterial {
                    base_color_texture: Some(foliage.clone()),
                    alpha_mode: AlphaMode::Mask(0.45),
                    double_sided: true,
                    cull_mode: None,
                    perceptual_roughness: 0.8,
                    reflectance: 0.25,
                    ..default()
                },
                Mat::Bronze => StandardMaterial {
                    base_color: Color::srgb(0.62, 0.45, 0.25),
                    metallic: 1.0,
                    perceptual_roughness: 0.32,
                    ..default()
                },
                Mat::Cloth => StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.95,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                },
                _ => StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.62, metallic: 0.12, ..default() },
            },
        };
        mats.insert(m, materials.add(mat));
    }

    let mut castle = HashMap::default();
    let mut castle_rigs = HashMap::default();
    let mut kinds = vec![(0, Variant::Base)];
    for stage in 1..3 {
        for v in [Variant::Defense, Variant::Movement] {
            kinds.push((stage, v));
        }
    }
    for (stage, v) in kinds {
        castle.insert((stage, v), upload(&mut meshes, &mats, models::castle(stage, v)));
        castle_rigs.insert((stage, v), upload_rig(&mut meshes, &mats, models::castle(stage, v)));
    }
    let modules = ModuleKind::ALL.iter().map(|&k| (k, upload(&mut meshes, &mats, models::module(k)))).collect();
    let module_rigs = ModuleKind::ALL.iter().map(|&k| (k, upload_rig(&mut meshes, &mats, models::module(k)))).collect();
    let mut enemies = HashMap::default();
    enemies.insert(EnemyKind::Footman, meshes.add(models::footman()));
    enemies.insert(EnemyKind::Archer, meshes.add(models::archer()));
    enemies.insert(EnemyKind::Knight, meshes.add(models::knight()));
    enemies.insert(EnemyKind::Cart, meshes.add(models::cart()));
    let mut bosses = HashMap::default();
    let mut boss_parts = HashMap::default();
    for (kind, model) in [
        (EnemyKind::SiegeTower, models::siege_tower()),
        (EnemyKind::WarWagon, models::war_wagon()),
        (EnemyKind::Juggernaut, models::juggernaut()),
        (EnemyKind::Trebuchet, models::trebuchet()),
        (EnemyKind::Warlord, models::warlord()),
    ] {
        let rig = upload_rig(&mut meshes, &mats, model);
        bosses.insert(kind, rig.body);
        boss_parts.insert(kind, rig.parts);
    }
    let vfx = crate::vfx::build_assets(&mut meshes, &mut materials, &mut images);

    let glow = |c: Color, e: LinearRgba| StandardMaterial { base_color: c, emissive: e, unlit: true, ..default() };
    let soft = |c: Color, a: f32| StandardMaterial {
        base_color: c.with_alpha(a),
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let mud = textured(&tex[&Surface::Mud], tile_size(Surface::Mud), StandardMaterial::default());
    let m = Mats {
        vc: mats[&Mat::Paint].clone(),
        flash: materials.add(glow(Color::WHITE, LinearRgba::rgb(3.0, 2.6, 2.2))),
        fire: materials.add(glow(Color::srgb(1.0, 0.7, 0.3), LinearRgba::rgb(14.0, 5.5, 1.2))),
        spark: materials.add(glow(Color::srgb(1.0, 0.85, 0.5), LinearRgba::rgb(7.0, 4.0, 1.2))),
        smoke: materials.add(soft(Color::srgb(0.82, 0.81, 0.79), 0.38)),
        smoke_dark: materials.add(soft(Color::srgb(0.16, 0.14, 0.13), 0.6)),
        track: materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.8, 0.75, 0.85),
            base_color_texture: Some(road_tex.clone()),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            cull_mode: None,
            ..default()
        }),
        scorch: materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.2, 0.18), ..mud.clone() }),
        dirt: materials.add(mud),
        debris: mats[&Mat::Wood].clone(),
        blood: materials.add(StandardMaterial { base_color: Color::srgb(0.30, 0.10, 0.07), perceptual_roughness: 0.5, ..default() }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgb(0.17, 0.27, 0.29),
            perceptual_roughness: 0.12,
            reflectance: 0.8,
            cull_mode: None,
            ..default()
        }),
        telegraph: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.22, 0.12, 0.45),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        cannonball: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.10, 0.11),
            metallic: 0.8,
            perceptual_roughness: 0.35,
            ..default()
        }),
        stone: mats[&Mat::Stone].clone(),
        grass: materials.add(textured(&tex[&Surface::Grass], tile_size(Surface::Grass), StandardMaterial::default())),
        smoke_fade: [0.42, 0.34, 0.26, 0.18, 0.1].iter().map(|a| materials.add(soft(Color::srgb(0.84, 0.83, 0.81), *a))).collect(),
        smoke_dark_fade: [0.6, 0.48, 0.36, 0.24, 0.12].iter().map(|a| materials.add(soft(Color::srgb(0.16, 0.14, 0.13), *a))).collect(),
        road: materials.add(StandardMaterial {
            base_color_texture: Some(road_tex.clone()),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            cull_mode: None,
            ..default()
        }),
    };

    let fonts = Fonts {
        title: fonts.add(Font::from_bytes(include_bytes!("../assets/fonts/Cinzel.ttf").to_vec())),
        body: fonts.add(Font::from_bytes(include_bytes!("../assets/fonts/EBGaramond.ttf").to_vec())),
    };

    let _ = FONT_TITLE.set(fonts.title.clone());
    let _ = FONT_BODY.set(fonts.body.clone());
    commands.insert_resource(art::build_icons(&mut images));
    commands.insert_resource(GameAssets {
        castle,
        castle_rigs,
        modules,
        module_rigs,
        enemies,
        bosses,
        boss_parts,
        vfx,
        villages: (0..4).map(|i| upload(&mut meshes, &mats, models::village(i))).collect(),
        sheep: meshes.add(models::sheep()),
        coin: upload(&mut meshes, &mats, models::coin()),
        arrow: meshes.add(models::arrow(art::rgb(0.62, 0.50, 0.34))),
        enemy_arrow: meshes.add(models::arrow(art::rgb(0.30, 0.22, 0.15))),
        mine: meshes.add(models::mine()),
        pointer: meshes.add(models::pointer()),
        ball: meshes.add(Sphere::new(0.5).mesh().ico(2).expect("ico")),
        cube: meshes.add(Cuboid::from_size(Vec3::ONE)),
        disc: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(20).build()),
        ring: meshes.add(Torus::new(0.85, 1.0)),
        rut: meshes.add(art::road_mesh(&[Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0)], 1.0, 0.0)),
        mats,
        m,
    });
    next.set(GameState::Starting);
}
