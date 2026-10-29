//! Troop photo studio, for checking the look of the regular enemies.
//!
//! `SIEGE_TROOPS=1` lines up every troop type at several angles on a patch of
//! grass outside the map, saves a close-up, a high view and a view from the
//! real game distance into `SIEGE_SHOT_DIR` (default `screenshots`), then quits.

use crate::art::{w3, yaw};
use crate::assets::GameAssets;
use crate::enemy::EnemyKind;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("SIEGE_TROOPS").is_ok() {
            app.add_systems(Update, run_studio);
        }
    }
}

const ORIGIN: Vec2 = Vec2::new(700.0, 0.0);
const KINDS: [EnemyKind; 4] = [EnemyKind::Footman, EnemyKind::Archer, EnemyKind::Knight, EnemyKind::Cart];

#[derive(Component)]
struct StudioCam;

fn run_studio(
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Option<Res<GameAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cams: Query<(&mut Camera, &mut Transform), Without<StudioCam>>,
    mut studio_cam: Query<&mut Transform, With<StudioCam>>,
    mut ui: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>,
    mut step: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(assets) = assets else { return };
    let t = time.elapsed_secs();
    for mut v in &mut ui {
        *v = Visibility::Hidden;
    }
    for (mut cam, _) in &mut cams {
        if cam.order == 0 {
            cam.is_active = false;
        }
    }
    let center = w3(ORIGIN, 0.8);
    if *step == 0 {
        *step = 1;
        let mut ground = Plane3d::new(Vec3::Y, Vec2::splat(40.0)).mesh().build();
        if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = ground.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
            for c in uv.iter_mut() {
                *c = [c[0] * 80.0, c[1] * 80.0];
            }
        }
        commands.spawn((Mesh3d(meshes.add(ground)), MeshMaterial3d(assets.m.grass.clone()), Transform::from_translation(w3(ORIGIN, 0.0))));
        // Rows: every troop look. Columns: side, three-quarter front, front, three-quarter back.
        let mut looks: Vec<(String, Handle<Mesh>)> = Vec::new();
        for kind in KINDS {
            for (i, h) in assets.enemies[&kind].iter().enumerate() {
                looks.push((format!("{} {}", kind.name(), i + 1), h.clone()));
            }
        }
        looks.push(("Archer aiming".into(), assets.archer_aim.clone()));
        for (name, h) in &looks {
            if let Some(mesh) = meshes.get(h) {
                info!("{name}: {} triangles", mesh.indices().map_or(0, |i| i.len() / 3));
            }
        }
        let angles = [0.0, -0.8, -1.57, 2.4];
        let rows = looks.len() as f32;
        for (r, (_, h)) in looks.iter().enumerate() {
            for (c, a) in angles.iter().enumerate() {
                let p = ORIGIN + Vec2::new((c as f32 - 1.5) * 4.6, (r as f32 - (rows - 1.0) / 2.0) * 3.4);
                commands.spawn((
                    Mesh3d(h.clone()),
                    MeshMaterial3d(assets.m.vc.clone()),
                    Transform::from_translation(w3(p, 0.0)).with_rotation(yaw(*a)),
                ));
            }
        }
        commands.spawn((StudioCam, Camera3d::default(), Camera { order: 5, ..default() }, bevy::camera::Hdr,
            bevy::core_pipeline::tonemapping::Tonemapping::AgX,
            Projection::Perspective(PerspectiveProjection { fov: 32f32.to_radians(), ..default() }),
            Transform::default()));
    }
    let dir = std::env::var("SIEGE_SHOT_DIR").unwrap_or_else(|_| "screenshots".into());
    let views: [(&str, Vec3); 3] = [
        ("troops_close", Vec3::new(0.0, 10.0, 26.0)),
        ("troops_top", Vec3::new(0.0, 22.0, 9.2)),
        ("troops_game", Vec3::new(0.0, 40.0, 16.8)),
    ];
    // Shaders compile on the first frames: wait a while, then one view per second.
    let i = ((t - 12.0) / 1.5).floor();
    if i >= 0.0 && (i as usize) < views.len() {
        let (name, off) = views[i as usize];
        if let Ok(mut tf) = studio_cam.single_mut() {
            *tf = Transform::from_translation(center + off).looking_at(center, Vec3::Y);
        }
        let shoot_at = 12.0 + i * 1.5 + 1.0;
        if t >= shoot_at && *step == i as u32 + 1 {
            *step += 1;
            let _ = std::fs::create_dir_all(&dir);
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(format!("{dir}/{name}.png")));
        }
    }
    if t > 12.0 + views.len() as f32 * 1.5 + 1.0 {
        exit.write(AppExit::Success);
    }
}
