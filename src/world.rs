//! The map, decorations, villages to absorb, and the follow camera.

use crate::assets::{GameAssets, shape};
use crate::castle::Castle;
use crate::combat::spawn_burst;
use crate::{GameEntity, GameRng, GameState, MAP_HALF, RunStats, Shake, Sim};
use bevy::prelude::*;
use rand::Rng;

const VILLAGE_COUNT: usize = 35;
const VILLAGE_RADIUS: f32 = 45.0;
const VILLAGE_XP: u32 = 12;
const VILLAGE_RESPAWN: f32 = 6.0;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VillageTimer(0.0))
            .add_systems(Startup, spawn_camera)
            .add_systems(OnEnter(GameState::Starting), spawn_world)
            .add_systems(Update, respawn_villages.in_set(Sim::Spawn))
            .add_systems(Update, absorb_villages.in_set(Sim::Resolve))
            .add_systems(PostUpdate, follow_camera.before(TransformSystems::Propagate));
    }
}

#[derive(Component)]
pub struct Village;

#[derive(Resource)]
struct VillageTimer(f32);

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_world(mut commands: Commands, assets: Res<GameAssets>, mut rng: ResMut<GameRng>) {
    let m = &assets.m;
    let size = MAP_HALF * 2.0;
    commands.spawn((GameEntity, shape(&assets.square, &m.grass, Vec3::ZERO, Vec2::splat(size))));

    // Border walls.
    let t = 60.0;
    for (pos, sz) in [
        (Vec2::new(0.0, MAP_HALF + t / 2.0), Vec2::new(size + 2.0 * t, t)),
        (Vec2::new(0.0, -MAP_HALF - t / 2.0), Vec2::new(size + 2.0 * t, t)),
        (Vec2::new(MAP_HALF + t / 2.0, 0.0), Vec2::new(t, size)),
        (Vec2::new(-MAP_HALF - t / 2.0, 0.0), Vec2::new(t, size)),
    ] {
        commands.spawn((GameEntity, shape(&assets.square, &m.border, pos.extend(0.1), sz)));
    }

    let rand_pos = |rng: &mut GameRng| {
        Vec2::new(
            rng.0.random_range(-MAP_HALF..MAP_HALF),
            rng.0.random_range(-MAP_HALF..MAP_HALF),
        )
    };

    // Light grass patches, trees and rocks give a sense of speed while driving.
    for _ in 0..220 {
        let p = rand_pos(&mut rng);
        let s = rng.0.random_range(120.0..360.0);
        commands.spawn((GameEntity, shape(&assets.circle, &m.grass_light, p.extend(0.2), Vec2::splat(s))));
    }
    for _ in 0..450 {
        let p = rand_pos(&mut rng);
        if p.length() < 250.0 {
            continue;
        }
        let s = rng.0.random_range(26.0..58.0);
        commands
            .spawn((GameEntity, shape(&assets.circle, &m.tree_dark, p.extend(0.4), Vec2::splat(s))))
            .with_child(shape(&assets.circle, &m.tree, Vec3::new(-0.08, 0.08, 0.1), Vec2::splat(0.75)));
    }
    for _ in 0..140 {
        let p = rand_pos(&mut rng);
        let s = rng.0.random_range(14.0..34.0);
        commands.spawn((GameEntity, shape(&assets.circle, &m.rock, p.extend(0.3), Vec2::splat(s))));
    }

    for _ in 0..VILLAGE_COUNT {
        let p = village_point(&mut rng, Vec2::ZERO, 500.0);
        spawn_village(&mut commands, &assets, &mut rng, p);
    }
}

/// Random point on the map at least `min_dist` from `avoid`.
fn village_point(rng: &mut GameRng, avoid: Vec2, min_dist: f32) -> Vec2 {
    let lim = MAP_HALF - 100.0;
    loop {
        let p = Vec2::new(rng.0.random_range(-lim..lim), rng.0.random_range(-lim..lim));
        if p.distance(avoid) > min_dist {
            return p;
        }
    }
}

fn spawn_village(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, pos: Vec2) {
    let m = &assets.m;
    commands
        .spawn((GameEntity, Village, Transform::from_translation(pos.extend(1.0)), Visibility::Visible))
        .with_children(|p| {
            for i in 0..4 {
                let a = i as f32 / 4.0 * std::f32::consts::TAU + rng.0.random::<f32>() * 0.6;
                let off = Vec2::from_angle(a) * rng.0.random_range(14.0..26.0);
                let s = rng.0.random_range(18.0..26.0);
                p.spawn(shape(&assets.square, &m.house, off.extend(0.0), Vec2::splat(s)));
                p.spawn(shape(&assets.square, &m.house_roof, off.extend(0.1), Vec2::new(s, s * 0.45)));
            }
        });
}

fn respawn_villages(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut timer: ResMut<VillageTimer>,
    villages: Query<(), With<Village>>,
    castle: Query<&Transform, With<Castle>>,
) {
    timer.0 += time.delta_secs();
    if timer.0 < VILLAGE_RESPAWN {
        return;
    }
    timer.0 = 0.0;
    if villages.iter().count() >= VILLAGE_COUNT {
        return;
    }
    let avoid = castle.single().map(|t| t.translation.truncate()).unwrap_or_default();
    let p = village_point(&mut rng, avoid, 900.0);
    spawn_village(&mut commands, &assets, &mut rng, p);
}

/// Driving over a village absorbs it: XP and a bit of repair.
#[allow(clippy::too_many_arguments)]
fn absorb_villages(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<RunStats>,
    mut shake: ResMut<Shake>,
    villages: Query<(Entity, &Transform), (With<Village>, Without<Castle>)>,
    mut castle: Query<(&Transform, &mut Castle)>,
) {
    let Ok((ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let cpos = ctf.translation.truncate();
    for (e, tf) in &villages {
        let pos = tf.translation.truncate();
        if pos.distance(cpos) < c.radius + VILLAGE_RADIUS * 0.6 {
            commands.entity(e).despawn();
            c.xp += VILLAGE_XP;
            c.hp = (c.hp + c.def().max_hp * 0.08).min(c.def().max_hp);
            stats.villages += 1;
            shake.0 += 3.0;
            spawn_burst(&mut commands, &assets, &assets.m.gem, &mut rng, pos, 14, 260.0, 10.0, 0.5);
        }
    }
}

fn follow_camera(
    time: Res<Time>,
    shake: Res<Shake>,
    mut rng: ResMut<GameRng>,
    castle: Query<(&Transform, &Castle), Without<Camera2d>>,
    mut cam: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (Ok((ctf, c)), Ok((mut tf, mut proj))) = (castle.single(), cam.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    let ease = 1.0 - (-6.0 * dt).exp();
    // Look a little ahead of where the castle is driving.
    let target = ctf.translation.truncate() + c.velocity * 0.25;
    let mut p = tf.translation.truncate().lerp(target, ease);
    if shake.0 > 0.0 {
        p += Vec2::new(rng.0.random_range(-1.0..1.0), rng.0.random_range(-1.0..1.0)) * shake.0.min(30.0);
    }
    tf.translation.x = p.x;
    tf.translation.y = p.y;

    if let Projection::Orthographic(o) = &mut *proj {
        let want = c.def().cam_scale;
        o.scale += (want - o.scale) * (1.0 - (-2.0 * dt).exp());
    }
}
