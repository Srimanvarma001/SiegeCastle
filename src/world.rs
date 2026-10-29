//! The battlefield: terrain, forests, ponds and a river, farms to absorb,
//! sheep to scoop up, plus the follow camera and sunlight.

use crate::art::{self, p2, w3, yaw};
use crate::models::{self, Model};
use crate::assets::GameAssets;
use crate::combat::{spawn_fx, FxKind};
use crate::items::{Loadout, Stats};
use crate::player::Castle;
use crate::progression::Progress;
use crate::{GameEntity, GameRng, GameState, MAP_HALF, RunStats, Shake, Sim};
use crate::graphics::{MainCamera, SceneImage};
use bevy::camera::RenderTarget;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use rand::Rng;

const VILLAGES: usize = 42;
const SHEEP: usize = 110;

/// Camera height above the castle at stage 1, before the player's own zoom.
pub const CAMERA_HEIGHT: f32 = 58.0;
/// Extra height per castle stage, so the bigger castles still fit.
const STAGE_ZOOM: [f32; 3] = [1.0, 1.2, 1.4];
/// How far the mouse wheel (or - and =) can zoom in and out.
const PLAYER_ZOOM: (f32, f32) = (0.6, 1.5);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, spawn_light))
            .add_systems(OnEnter(GameState::Starting), spawn_world)
            .add_systems(Update, (respawn_villages, respawn_sheep).in_set(Sim::Spawn))
            .add_systems(Update, (move_sheep, absorb).chain().in_set(Sim::Resolve))
            .add_systems(PostUpdate, follow_camera.before(TransformSystems::Propagate));
    }
}

#[derive(Component)]
pub struct Village;

#[derive(Component)]
pub struct Sheep {
    target: Vec2,
    wait: f32,
}

/// Where water is, so farms and trees stay off it.
#[derive(Resource, Default)]
struct Water {
    ponds: Vec<(Vec2, f32)>,
    river: Vec<Vec2>,
    /// Dirt road center lines (kept clear of trees and farms).
    roads: Vec<Vec2>,
}

impl Water {
    fn near(&self, p: Vec2, margin: f32) -> bool {
        self.ponds.iter().any(|(c, r)| c.distance(p) < r + margin)
            || self.river.iter().any(|q| q.distance(p) < 2.5 + margin)
            || self.roads.iter().any(|q| q.distance(p) < 2.0 + margin)
    }
}

/// A winding road from one map edge to the other through a few waypoints.
fn make_road(rng: &mut GameRng, horizontal: bool) -> Vec<Vec2> {
    let edge = MAP_HALF + 20.0;
    let mut way = Vec::new();
    let n = 5;
    for i in 0..=n {
        let t = -edge + 2.0 * edge * i as f32 / n as f32;
        let off = rng.0.random_range(-140.0..140.0);
        way.push(if horizontal { Vec2::new(t, off) } else { Vec2::new(off, t) });
    }
    // Catmull-Rom through the waypoints, one point every ~2 m.
    let mut out = Vec::new();
    for i in 0..way.len() - 1 {
        let p0 = way[i.saturating_sub(1)];
        let (p1, p2) = (way[i], way[i + 1]);
        let p3 = way[(i + 2).min(way.len() - 1)];
        let steps = (p1.distance(p2) / 2.0) as i32;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push(0.5 * ((2.0 * p1) + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3));
        }
    }
    out
}

// --- Noise for soft color patches ------------------------------------------

fn hash(i: i32, j: i32, seed: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(374_761_393) ^ (j as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

fn noise(p: Vec2, seed: u32) -> f32 {
    let (i, j) = (p.x.floor() as i32, p.y.floor() as i32);
    let f = p - p.floor();
    let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let a = hash(i, j, seed);
    let b = hash(i + 1, j, seed);
    let c = hash(i, j + 1, seed);
    let d = hash(i + 1, j + 1, seed);
    let x1 = a + (b - a) * s.x;
    let x2 = c + (d - c) * s.x;
    x1 + (x2 - x1) * s.y
}

fn fbm(p: Vec2, seed: u32) -> f32 {
    0.5 * noise(p / 55.0, seed) + 0.3 * noise(p / 22.0, seed + 1) + 0.2 * noise(p / 8.0, seed + 2)
}

// --- Setup ------------------------------------------------------------------

/// Bloom and anti-aliasing are added by `graphics::apply` from the chosen
/// quality preset. It draws into the scene image (see `graphics.rs`), and
/// `follow_camera` moves it and keeps the haze in place.
fn spawn_camera(mut commands: Commands, scene: Res<SceneImage>) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        RenderTarget::from(scene.0.clone()),
        bevy::camera::Hdr,
        Tonemapping::AgX,
        Projection::Perspective(PerspectiveProjection { fov: 32f32.to_radians(), ..default() }),
        Transform::from_xyz(0.0, 46.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Light haze in the distance, tinted by the sun.
        DistanceFog {
            color: Color::srgba(0.62, 0.68, 0.74, 1.0),
            directional_light_color: Color::srgba(1.0, 0.85, 0.65, 0.4),
            directional_light_exponent: 20.0,
            falloff: FogFalloff::Linear { start: 70.0, end: 260.0 },
        },
        Msaa::Off,
    ));
}

fn spawn_light(mut commands: Commands) {
    // Cool sky light from above, warm late-afternoon sun. Nothing casts shadows.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.70, 0.78, 0.92),
        brightness: 380.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.90, 0.76),
            illuminance: 6_000.0,
            ..default()
        },
        Transform::from_xyz(-70.0, 62.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
) {
    let seed: u32 = rng.0.random();

    // Water and roads first, so everything else can avoid them.
    let mut water = Water::default();
    let mut roads = Vec::new();
    for horizontal in [true, false, true] {
        let r = make_road(&mut rng, horizontal);
        water.roads.extend(r.iter().copied());
        roads.push(r);
    }
    for _ in 0..7 {
        let c = Vec2::new(rng.0.random_range(-190.0..190.0), rng.0.random_range(-190.0..190.0));
        if c.length() > 40.0 {
            water.ponds.push((c, rng.0.random_range(7.0..13.0)));
        }
    }
    let y0 = rng.0.random_range(-150.0..150.0);
    let phase: f32 = rng.0.random_range(0.0..6.0);
    for i in 0..=240 {
        let x = -MAP_HALF - 10.0 + i as f32 * (2.0 * MAP_HALF + 20.0) / 240.0;
        let y = y0 + (x / 70.0 + phase).sin() * 40.0 + (x / 23.0).sin() * 8.0;
        let p = Vec2::new(x, y);
        if p.length() > 35.0 {
            water.river.push(p);
        } else if !water.river.is_empty() {
            break;
        }
    }

    // Ground with soft patches; darker grass near water.
    let w = &water;
    // The grass texture is tinted per vertex: lush, dry and shaded patches.
    // Reaches well past the pine wall so a zoomed-out camera never sees its edge.
    let ground = art::ground_mesh(MAP_HALF + 90.0, 200, |p| {
        let n = fbm(p, seed);
        let dry = fbm(p * 0.7 + Vec2::splat(91.0), seed + 3);
        let base = art::rgb(1.0, 1.0, 1.0);
        let lush = art::rgb(0.82, 0.95, 0.78);
        let pale = art::rgb(1.18, 1.1, 0.86);
        let mut c = if n > 0.5 { base.mix(&pale, (n - 0.5) * 1.6) } else { lush.mix(&base, n * 2.0) };
        if dry > 0.62 {
            c = c.mix(&art::rgb(1.3, 1.1, 0.75), ((dry - 0.62) * 3.0).min(0.6));
        }
        if w.near(p, 4.0) {
            c = c.mix(&art::rgb(0.65, 0.82, 0.62), 0.6);
        }
        if p.x.abs() > MAP_HALF || p.y.abs() > MAP_HALF {
            c = c.mix(&art::rgb(0.45, 0.55, 0.42), 0.7);
        }
        c
    });
    commands.spawn((GameEntity, Mesh3d(meshes.add(ground)), MeshMaterial3d(assets.m.grass.clone()), Transform::IDENTITY));

    // Dirt roads (soft edges blend into the grass).
    for (i, r) in roads.iter().enumerate() {
        commands.spawn((
            GameEntity,
            Mesh3d(meshes.add(art::road_mesh(r, 3.6, 0.02 + i as f32 * 0.004))),
            MeshMaterial3d(assets.m.road.clone()),
            Transform::IDENTITY,
        ));
    }

    // Water meshes.
    for &(c, r) in &water.ponds {
        let stretch = rng.0.random_range(0.7..1.3);
        commands.spawn((
            GameEntity,
            Mesh3d(assets.disc.clone()),
            MeshMaterial3d(assets.m.water.clone()),
            Transform::from_translation(w3(c, 0.03)).with_scale(Vec3::new(r * 2.0 * stretch, 0.04, r * 2.0 / stretch)),
        ));
    }
    if water.river.len() > 2 {
        let river = art::ribbon_mesh(&water.river, 2.6, 0.04, art::rgb(0.16, 0.27, 0.27));
        commands.spawn((GameEntity, Mesh3d(meshes.add(river)), MeshMaterial3d(assets.m.water.clone()), Transform::IDENTITY));
    }

    // Trees, rocks and grass, merged into a few big meshes per map chunk.
    const CHUNKS: usize = 6;
    let chunk_of = |p: Vec2| {
        let f = |v: f32| (((v + MAP_HALF + 40.0) / (2.0 * MAP_HALF + 80.0)) * CHUNKS as f32).clamp(0.0, CHUNKS as f32 - 1.0) as usize;
        f(p.y) * CHUNKS + f(p.x)
    };
    let mut solid: Vec<Model> = (0..CHUNKS * CHUNKS).map(|_| Model::new()).collect();
    let mut grass: Vec<Model> = (0..CHUNKS * CHUNKS).map(|_| Model::new()).collect();
    let mut has_solid = [false; CHUNKS * CHUNKS];
    let mut has_grass = [false; CHUNKS * CHUNKS];
    let free = |p: Vec2| p.length() > 28.0 && !w.near(p, 3.0) && p.x.abs() < MAP_HALF - 4.0 && p.y.abs() < MAP_HALF - 4.0;

    for _ in 0..120 {
        let c = Vec2::new(rng.0.random_range(-210.0..210.0), rng.0.random_range(-210.0..210.0));
        let n = rng.0.random_range(5..14);
        for _ in 0..n {
            let p = c + Vec2::new(rng.0.random_range(-12.0..12.0), rng.0.random_range(-9.0..9.0));
            if free(p) {
                let k = chunk_of(p);
                models::add_tree(&mut solid[k], w3(p, 0.0), rng.0.random_range(0.8..1.3), rng.0.random_range(0.0..1.0), rng.0.random());
                has_solid[k] = true;
            }
        }
    }
    for _ in 0..260 {
        let p = Vec2::new(rng.0.random_range(-215.0..215.0), rng.0.random_range(-215.0..215.0));
        if free(p) {
            let k = chunk_of(p);
            models::add_tree(&mut solid[k], w3(p, 0.0), rng.0.random_range(0.7..1.1), rng.0.random_range(0.0..1.0), rng.0.random());
            has_solid[k] = true;
        }
    }
    for _ in 0..160 {
        let p = Vec2::new(rng.0.random_range(-215.0..215.0), rng.0.random_range(-215.0..215.0));
        if free(p) {
            let k = chunk_of(p);
            models::add_rock(&mut solid[k], w3(p, 0.1), rng.0.random_range(0.3..0.8), rng.0.random());
            has_solid[k] = true;
        }
    }
    // A wall of pines around the edge of the map.
    let edge = MAP_HALF + 6.0;
    let mut t = -edge;
    while t < edge {
        for (p, jitter) in [
            (Vec2::new(t, edge), Vec2::new(0.0, 4.0)),
            (Vec2::new(t, -edge), Vec2::new(0.0, -4.0)),
            (Vec2::new(edge, t), Vec2::new(4.0, 0.0)),
            (Vec2::new(-edge, t), Vec2::new(-4.0, 0.0)),
        ] {
            for row in 0..2 {
                let q = p + jitter * row as f32 + Vec2::new(rng.0.random_range(-1.5..1.5), rng.0.random_range(-1.5..1.5));
                let k = chunk_of(q);
                models::add_pine(&mut solid[k], w3(q, 0.0), rng.0.random_range(1.2..1.8), rng.0.random());
                has_solid[k] = true;
            }
        }
        t += 4.5;
    }
    for _ in 0..1600 {
        let p = Vec2::new(rng.0.random_range(-215.0..215.0), rng.0.random_range(-215.0..215.0));
        if free(p) {
            let k = chunk_of(p);
            models::add_bush(&mut solid[k], w3(p, 0.0), rng.0.random_range(0.8..1.5), rng.0.random());
            has_solid[k] = true;
        }
    }
    for _ in 0..9000 {
        let p = Vec2::new(rng.0.random_range(-218.0..218.0), rng.0.random_range(-218.0..218.0));
        if !w.near(p, 1.0) {
            let k = chunk_of(p);
            models::add_grass(&mut grass[k], w3(p, 0.0), rng.0.random_range(0.8..1.6), rng.0.random());
            has_grass[k] = true;
        }
    }
    for (k, model) in solid.into_iter().enumerate() {
        if has_solid[k] {
            for (mat, mesh) in model.build() {
                commands.spawn((GameEntity, Mesh3d(meshes.add(mesh)), MeshMaterial3d(assets.mat(mat)), Transform::IDENTITY));
            }
        }
    }
    for (k, model) in grass.into_iter().enumerate() {
        if has_grass[k] {
            for (mat, mesh) in model.build() {
                commands.spawn((GameEntity, Mesh3d(meshes.add(mesh)), MeshMaterial3d(assets.mat(mat)), Transform::IDENTITY));
            }
        }
    }

    // Farms and sheep.
    for _ in 0..VILLAGES {
        if let Some(p) = free_spot(&mut rng, &water, Vec2::ZERO, 30.0) {
            spawn_village(&mut commands, &assets, &mut rng, p);
        }
    }
    for _ in 0..(SHEEP / 4) {
        if let Some(p) = free_spot(&mut rng, &water, Vec2::ZERO, 20.0) {
            spawn_flock(&mut commands, &assets, &mut rng, p, 4);
        }
    }
    commands.insert_resource(water);
}

fn free_spot(rng: &mut GameRng, water: &Water, avoid: Vec2, min_dist: f32) -> Option<Vec2> {
    let lim = MAP_HALF - 20.0;
    for _ in 0..30 {
        let p = Vec2::new(rng.0.random_range(-lim..lim), rng.0.random_range(-lim..lim));
        if p.distance(avoid) > min_dist && !water.near(p, 8.0) {
            return Some(p);
        }
    }
    None
}

fn spawn_village(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, p: Vec2) {
    let i = rng.0.random_range(0..assets.villages.len());
    let angle = rng.0.random_range(0..4) as f32 * std::f32::consts::FRAC_PI_2;
    let model = assets.villages[i].clone();
    commands
        .spawn((GameEntity, Village, Transform::from_translation(w3(p, 0.0)).with_rotation(yaw(angle)), Visibility::Visible))
        .with_children(|c| model.spawn_parts(c, ()));
}

fn spawn_flock(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, p: Vec2, n: usize) {
    for _ in 0..n {
        let q = p + Vec2::new(rng.0.random_range(-3.0..3.0), rng.0.random_range(-3.0..3.0));
        commands.spawn((
            GameEntity,
            Sheep { target: q, wait: rng.0.random_range(0.0..3.0) },
            Mesh3d(assets.sheep.clone()),
            MeshMaterial3d(assets.m.vc.clone()),
            Transform::from_translation(w3(q, 0.0)).with_rotation(yaw(rng.0.random_range(0.0..std::f32::consts::TAU))),
        ));
    }
}

fn respawn_villages(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    water: Option<Res<Water>>,
    mut rng: ResMut<GameRng>,
    mut timer: Local<f32>,
    villages: Query<(), With<Village>>,
    castle: Query<&Transform, With<Castle>>,
) {
    *timer += time.delta_secs();
    if *timer < 8.0 {
        return;
    }
    *timer = 0.0;
    let (Some(water), Ok(ctf)) = (water, castle.single()) else {
        return;
    };
    if villages.iter().count() < VILLAGES
        && let Some(p) = free_spot(&mut rng, &water, p2(ctf.translation), 80.0)
    {
        spawn_village(&mut commands, &assets, &mut rng, p);
    }
}

fn respawn_sheep(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    water: Option<Res<Water>>,
    mut rng: ResMut<GameRng>,
    mut timer: Local<f32>,
    sheep: Query<(), With<Sheep>>,
    castle: Query<&Transform, With<Castle>>,
) {
    *timer += time.delta_secs();
    if *timer < 5.0 {
        return;
    }
    *timer = 0.0;
    let (Some(water), Ok(ctf)) = (water, castle.single()) else {
        return;
    };
    if sheep.iter().count() < SHEEP
        && let Some(p) = free_spot(&mut rng, &water, p2(ctf.translation), 60.0)
    {
        spawn_flock(&mut commands, &assets, &mut rng, p, 4);
    }
}

/// Sheep graze, wander, and run from the castle.
fn move_sheep(
    time: Res<Time>,
    mut rng: ResMut<GameRng>,
    castle: Query<&Transform, (With<Castle>, Without<Sheep>)>,
    mut sheep: Query<(&mut Transform, &mut Sheep)>,
) {
    let Ok(ctf) = castle.single() else {
        return;
    };
    let cpos = p2(ctf.translation);
    let dt = time.delta_secs();
    for (mut tf, mut s) in &mut sheep {
        let pos = p2(tf.translation);
        let away = pos - cpos;
        let (dir, speed) = if away.length() < 11.0 {
            (away.normalize_or(Vec2::X), 4.5)
        } else {
            s.wait -= dt;
            if s.wait <= 0.0 {
                s.wait = rng.0.random_range(2.0..6.0);
                s.target = pos + Vec2::new(rng.0.random_range(-5.0..5.0), rng.0.random_range(-5.0..5.0));
            }
            let to = s.target - pos;
            if to.length() < 0.3 { (Vec2::ZERO, 0.0) } else { (to.normalize(), 1.2) }
        };
        if speed > 0.0 {
            let next = (pos + dir * speed * dt).clamp(Vec2::splat(-MAP_HALF + 5.0), Vec2::splat(MAP_HALF - 5.0));
            tf.translation = w3(next, 0.0);
            tf.rotation = yaw(dir.to_angle());
        }
    }
}

/// Driving over farms and sheep turns them into XP (farms also repair walls).
fn absorb(
    mut commands: Commands,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut progress: ResMut<Progress>,
    mut run: ResMut<RunStats>,
    mut shake: ResMut<Shake>,
    mut rng: ResMut<GameRng>,
    villages: Query<(Entity, &Transform), (With<Village>, Without<Castle>)>,
    sheep: Query<(Entity, &Transform), (With<Sheep>, Without<Castle>)>,
    mut castle: Query<(&Transform, &mut Castle)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = p2(ctf.translation);
    for (e, tf) in &villages {
        let p = p2(tf.translation);
        if p.distance(cpos) < stats.radius + 4.5 {
            commands.entity(e).despawn();
            progress.add_xp(14.0 * stats.xp_mult);
            c.hp = (c.hp + stats.max_hp * 0.06).min(stats.max_hp);
            run.villages += 1;
            shake.0 += 0.4;
            let angle = tf.rotation.to_euler(EulerRot::YXZ).0;
            let e = spawn_fx(&mut commands, &assets, FxKind::Dirt, w3(p, 0.02), 9.0, Vec3::ZERO);
            commands.entity(e).entry::<Transform>().and_modify(move |mut t| t.rotation = yaw(angle));
            for _ in 0..8 {
                let v = Vec3::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(1.0..2.0), rng.0.random_range(-1.0..1.0)) * 8.0;
                spawn_fx(&mut commands, &assets, FxKind::Debris, w3(p, 1.0), 0.5, v);
            }
            spawn_fx(&mut commands, &assets, FxKind::Smoke, w3(p, 1.0), 3.0, Vec3::Y * 2.0);
        }
    }
    for (e, tf) in &sheep {
        let p = p2(tf.translation);
        if p.distance(cpos) < stats.radius + 0.8 {
            commands.entity(e).despawn();
            progress.add_xp(3.0 * stats.xp_mult);
            run.sheep += 1;
            spawn_fx(&mut commands, &assets, FxKind::Smoke, w3(p, 0.8), 1.4, Vec3::Y * 2.0);
        }
    }
}

/// Player zoom: the target set by the mouse wheel and the smoothed value.
#[derive(Default)]
pub struct PlayerZoom {
    target: f32,
    current: f32,
}

pub fn follow_camera(
    time: Res<Time<Real>>,
    shake: Res<Shake>,
    loadout: Res<Loadout>,
    scroll: Res<AccumulatedMouseScroll>,
    keys: Res<ButtonInput<KeyCode>>,
    boss: Res<crate::boss::BossCam>,
    mut rng: ResMut<GameRng>,
    castle: Query<(&Transform, &Castle), Without<MainCamera>>,
    mut cam: Query<(&mut Transform, &mut DistanceFog), With<MainCamera>>,
    mut zoom: Local<f32>,
    mut player: Local<PlayerZoom>,
    mut focus: Local<Vec2>,
) {
    let (Ok((ctf, c)), Ok((mut tf, mut fog))) = (castle.single(), cam.single_mut()) else {
        return;
    };
    // Real time, so zooming still works while paused or picking cards.
    let dt = time.delta_secs().min(0.1);
    if *zoom == 0.0 {
        *zoom = 1.0;
        *player = PlayerZoom { target: 1.0, current: 1.0 };
        *focus = p2(ctf.translation);
    }

    // Wheel up zooms in. Touchpads scroll in pixels, mice in lines.
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    let mut steps = -lines;
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        steps -= 4.0 * dt;
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        steps += 4.0 * dt;
    }
    if steps != 0.0 {
        player.target = (player.target * 1.1f32.powf(steps)).clamp(PLAYER_ZOOM.0, PLAYER_ZOOM.1);
    }
    player.current += (player.target - player.current) * (1.0 - (-10.0 * dt).exp());

    // Pull back when a colossal boss is near so both it and the castle fit.
    let want_zoom = STAGE_ZOOM[loadout.stage.min(2)].max(boss.zoom);
    *zoom += (want_zoom - *zoom) * (1.0 - (-1.2 * dt).exp());
    let target = p2(ctf.translation) + c.vel * 0.3 + boss.offset;
    *focus = focus.lerp(target, 1.0 - (-5.0 * dt).exp());
    let h = CAMERA_HEIGHT * *zoom * player.current;
    // The haze was tuned for a camera 40 m up (starting at 70 m, full at 260 m).
    // Scale it with the height so it stays at the same place on screen.
    fog.falloff = FogFalloff::Linear { start: h * 1.75, end: h * 6.5 };
    let mut look = w3(*focus, 0.0);
    if shake.0 > 0.0 {
        let s = shake.0.min(3.0) * 0.5;
        look += Vec3::new(rng.0.random_range(-s..s), 0.0, rng.0.random_range(-s..s));
    }
    *tf = Transform::from_translation(look + Vec3::new(0.0, h, h * 0.42)).looking_at(look, Vec3::Y);
}
