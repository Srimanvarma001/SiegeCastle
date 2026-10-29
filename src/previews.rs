//! Renders small 3D portraits of every weapon, artifact and castle upgrade
//! into images, used on the level-up cards and the ability slots.

use crate::art::{yaw, Icon, Icons, Variant};
use crate::assets::{upload, GameAssets, ModelHandle};
use crate::items::{ArtifactKind, ModuleKind};
use crate::models;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Render layer used only by the portrait studio.
const STUDIO: usize = 3;
const SIZE: u32 = 256;

pub struct PreviewsPlugin;

impl Plugin for PreviewsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup.after(crate::assets::create_assets))
            .add_systems(Update, finish);
    }
}

#[derive(Component)]
struct StudioCam(u32);

#[derive(Component)]
struct StudioProp;

struct Subject {
    icon: Icon,
    model: ModelHandle,
    radius: f32,
    center: Vec3,
    turn: f32,
}

fn setup(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut icons: ResMut<Icons>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let layer = RenderLayers::layer(STUDIO);
    let mut subjects = Vec::new();
    for k in ModuleKind::ALL {
        let (radius, center, turn) = match k {
            ModuleKind::ArrowLoft => (1.75, Vec3::new(0.0, 0.85, 0.0), -0.5),
            ModuleKind::Broadside => (1.25, Vec3::new(0.0, 0.4, 0.2), -1.9),
            ModuleKind::Ballista => (1.45, Vec3::new(0.0, 0.65, 0.0), -0.5),
            ModuleKind::Ram => (1.5, Vec3::new(0.5, 0.85, 0.0), -0.5),
            ModuleKind::MineLayer => (1.35, Vec3::new(-0.05, 0.55, 0.0), 0.7),
            _ => (1.35, Vec3::new(0.0, 0.62, 0.0), -0.5),
        };
        subjects.push(Subject { icon: Icon::Module(k), model: assets.modules[&k].clone(), radius, center, turn });
    }
    for a in ArtifactKind::ALL {
        let model = upload(&mut meshes, &assets.mats, models::artifact(a));
        subjects.push(Subject { icon: Icon::Artifact(a), model, radius: 0.85, center: Vec3::new(0.0, 0.35, 0.0), turn: -0.4 });
    }
    for (icon, v) in [(Icon::Defense, Variant::Defense), (Icon::Movement, Variant::Movement)] {
        subjects.push(Subject { icon, model: assets.castle[&(1, v)].clone(), radius: 7.0, center: Vec3::new(0.0, 3.6, 0.0), turn: -0.5 });
    }

    // Soft studio lighting.
    commands.spawn((
        StudioProp,
        DirectionalLight { illuminance: 9_000.0, color: Color::srgb(1.0, 0.94, 0.85), ..default() },
        Transform::from_xyz(-1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
    ));

    for (i, s) in subjects.into_iter().enumerate() {
        let origin = Vec3::new(i as f32 * 60.0, -600.0, 0.0);
        let model = s.model.clone();
        let l = layer.clone();
        commands
            .spawn((StudioProp, Transform::from_translation(origin).with_rotation(yaw(s.turn)), Visibility::Visible))
            .with_children(|p| model.spawn_parts(p, l));
        let image = images.add(Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None));
        let look = origin + s.center;
        let dir = Vec3::new(1.0, 0.8, 1.25).normalize();
        let dist = s.radius / 15f32.to_radians().tan();
        commands.spawn((
            StudioCam(0),
            Camera3d::default(),
            Camera { order: -1 - i as isize, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            RenderTarget::from(image.clone()),
            Projection::Perspective(PerspectiveProjection { fov: 30f32.to_radians(), ..default() }),
            Tonemapping::TonyMcMapface,
            Transform::from_translation(look + dir * dist).looking_at(look, Vec3::Y),
            layer.clone(),
        ));
        icons.0.insert(s.icon, image);
    }
}

/// The portraits only need to be drawn once: switch the studio off afterwards.
/// Shaders compile in the background on the first frames, so keep the studio
/// running for a while (by real time and frame count) before switching it off.
/// Only one portrait camera draws per frame, in turn: drawing all of them every
/// frame made the first seconds of play stutter. A camera that is switched off
/// keeps its last picture.
fn finish(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut cams: Query<(Entity, &mut Camera, &mut StudioCam)>,
    props: Query<Entity, With<StudioProp>>,
    mut frame: Local<u32>,
) {
    let mut done = false;
    let late = time.elapsed_secs() > 10.0;
    let count = cams.iter().count().max(1) as u32;
    *frame += 1;
    for (e, mut cam, mut sc) in &mut cams {
        sc.0 += 1;
        let slot = (-1 - cam.order).max(0) as u32;
        let turn = (*frame + slot).is_multiple_of(count);
        if cam.is_active != turn {
            cam.is_active = turn;
        }
        if late && sc.0 > 60 {
            cam.is_active = false;
            if sc.0 > 64 {
                commands.entity(e).despawn();
                done = true;
            }
        }
    }
    if done {
        for e in &props {
            commands.entity(e).despawn();
        }
    }
}
