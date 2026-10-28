//! The player's castle: movement, 3 growth stages, towers and the barrage.

use crate::assets::{GameAssets, shape};
use crate::combat::{Projectile, spawn_burst};
use crate::debug::Autopilot;
use crate::enemy::Enemy;
use crate::grid::Grid;
use crate::{Banner, GameEntity, GameRng, GameState, MAP_HALF, Shake, Sim};
use bevy::prelude::*;

pub const Z_CASTLE: f32 = 5.0;

pub struct CastlePlugin;

impl Plugin for CastlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Starting), spawn_castle)
            .add_systems(Update, (castle_move, barrage).in_set(Sim::Input))
            .add_systems(Update, towers_fire.in_set(Sim::Combat))
            .add_systems(
                Update,
                (contact_damage, castle_growth, castle_death)
                    .chain()
                    .in_set(Sim::Resolve),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerKind {
    Archer,
    Cannon,
    Mage,
}

pub struct TowerStats {
    pub cooldown: f32,
    pub damage: f32,
    pub range: f32,
    pub speed: f32,
    pub radius: f32,
    pub splash: f32,
    pub pierce: u32,
}

impl TowerKind {
    pub fn stats(self) -> TowerStats {
        match self {
            TowerKind::Archer => TowerStats {
                cooldown: 0.3,
                damage: 11.0,
                range: 430.0,
                speed: 950.0,
                radius: 5.0,
                splash: 0.0,
                pierce: 0,
            },
            TowerKind::Cannon => TowerStats {
                cooldown: 1.3,
                damage: 28.0,
                range: 500.0,
                speed: 700.0,
                radius: 7.0,
                splash: 75.0,
                pierce: 0,
            },
            TowerKind::Mage => TowerStats {
                cooldown: 0.9,
                damage: 20.0,
                range: 560.0,
                speed: 750.0,
                radius: 8.0,
                splash: 0.0,
                pierce: 4,
            },
        }
    }
}

pub struct StageDef {
    pub name: &'static str,
    pub max_hp: f32,
    /// Collision radius.
    pub radius: f32,
    pub speed: f32,
    /// Total XP needed to reach the next stage. `None` on the last stage.
    pub xp_next: Option<u32>,
    /// Camera zoom (bigger = see more of the map).
    pub cam_scale: f32,
    /// Gems inside this distance fly to the castle.
    pub magnet: f32,
    pub barrage_shots: u32,
    pub damage_mult: f32,
}

pub const STAGES: [StageDef; 3] = [
    StageDef {
        name: "Wooden Keep",
        max_hp: 250.0,
        radius: 40.0,
        speed: 240.0,
        xp_next: Some(100),
        cam_scale: 1.0,
        magnet: 260.0,
        barrage_shots: 16,
        damage_mult: 1.0,
    },
    StageDef {
        name: "Stone Fort",
        max_hp: 600.0,
        radius: 70.0,
        speed: 225.0,
        xp_next: Some(650),
        cam_scale: 1.35,
        magnet: 340.0,
        barrage_shots: 24,
        damage_mult: 1.25,
    },
    StageDef {
        name: "Grand Citadel",
        max_hp: 1400.0,
        radius: 105.0,
        speed: 210.0,
        xp_next: None,
        cam_scale: 1.75,
        magnet: 440.0,
        barrage_shots: 36,
        damage_mult: 1.6,
    },
];

pub const BARRAGE_COOLDOWN: f32 = 8.0;

#[derive(Component)]
pub struct Castle {
    /// 0, 1 or 2.
    pub stage: usize,
    pub hp: f32,
    pub xp: u32,
    pub radius: f32,
    pub velocity: Vec2,
    pub barrage_cd: f32,
}

impl Castle {
    pub fn def(&self) -> &'static StageDef {
        &STAGES[self.stage]
    }
}

#[derive(Component)]
pub struct Tower {
    pub kind: TowerKind,
    pub timer: f32,
}

fn spawn_castle(mut commands: Commands, assets: Res<GameAssets>) {
    let def = &STAGES[0];
    commands
        .spawn((
            GameEntity,
            Castle {
                stage: 0,
                hp: def.max_hp,
                xp: 0,
                radius: def.radius,
                velocity: Vec2::ZERO,
                barrage_cd: 0.0,
            },
            Transform::from_xyz(0.0, 0.0, Z_CASTLE),
            Visibility::Visible,
        ))
        .with_children(|p| build_castle(p, &assets, 0));
}

/// Draws the castle for a stage. The castle faces +X (its front).
fn build_castle(p: &mut ChildSpawnerCommands, a: &GameAssets, stage: usize) {
    let m = &a.m;
    let sq = &a.square;
    let ci = &a.circle;
    let tower = |p: &mut ChildSpawnerCommands, kind: TowerKind, x: f32, y: f32, size: f32| {
        let (outer, inner) = match kind {
            TowerKind::Archer => (&m.wood_light, &m.archer),
            TowerKind::Cannon => (&m.stone_light, &m.cannon),
            TowerKind::Mage => (&m.stone_light, &m.mage),
        };
        p.spawn((
            Tower { kind, timer: 0.0 },
            shape(ci, outer, Vec3::new(x, y, 0.5), Vec2::splat(size)),
        ))
        .with_child(shape(ci, inner, Vec3::Z * 0.1, Vec2::splat(0.55)));
    };
    let wheels = |p: &mut ChildSpawnerCommands, xs: &[f32], y: f32, w: f32, h: f32| {
        for &x in xs {
            for s in [-1.0, 1.0] {
                p.spawn(shape(sq, &m.wheel, Vec3::new(x, s * y, -0.2), Vec2::new(w, h)));
            }
        }
    };

    match stage {
        0 => {
            wheels(p, &[-20.0, 20.0], 32.0, 18.0, 10.0);
            p.spawn(shape(sq, &m.wood, Vec3::ZERO, Vec2::new(72.0, 60.0)));
            p.spawn(shape(sq, &m.wood_light, Vec3::Z * 0.1, Vec2::new(36.0, 34.0)));
            p.spawn(shape(sq, &m.roof_red, Vec3::Z * 0.2, Vec2::new(20.0, 20.0)));
            // Front marker so you can see which way the castle drives.
            p.spawn(shape(sq, &m.gold, Vec3::new(36.0, 0.0, 0.2), Vec2::new(8.0, 16.0)));
            tower(p, TowerKind::Archer, -8.0, 30.0, 24.0);
            tower(p, TowerKind::Archer, -8.0, -30.0, 24.0);
            tower(p, TowerKind::Archer, -30.0, 0.0, 22.0);
        }
        1 => {
            wheels(p, &[-40.0, 0.0, 40.0], 58.0, 24.0, 12.0);
            p.spawn(shape(sq, &m.stone, Vec3::ZERO, Vec2::new(130.0, 110.0)));
            p.spawn(shape(sq, &m.wood, Vec3::Z * 0.1, Vec2::new(100.0, 80.0)));
            p.spawn(shape(sq, &m.stone_light, Vec3::Z * 0.2, Vec2::new(54.0, 54.0)));
            p.spawn(shape(sq, &m.roof_blue, Vec3::Z * 0.3, Vec2::new(30.0, 30.0)));
            p.spawn(shape(sq, &m.gold, Vec3::new(65.0, 0.0, 0.3), Vec2::new(10.0, 24.0)));
            tower(p, TowerKind::Cannon, 58.0, 48.0, 32.0);
            tower(p, TowerKind::Cannon, 58.0, -48.0, 32.0);
            tower(p, TowerKind::Archer, -58.0, 48.0, 30.0);
            tower(p, TowerKind::Archer, -58.0, -48.0, 30.0);
        }
        _ => {
            wheels(p, &[-70.0, -24.0, 24.0, 70.0], 90.0, 30.0, 14.0);
            p.spawn(shape(sq, &m.stone, Vec3::ZERO, Vec2::new(200.0, 170.0)));
            p.spawn(shape(sq, &m.wood, Vec3::Z * 0.1, Vec2::new(170.0, 140.0)));
            p.spawn(shape(sq, &m.stone_light, Vec3::Z * 0.2, Vec2::new(120.0, 100.0)));
            p.spawn(shape(sq, &m.stone, Vec3::Z * 0.3, Vec2::new(70.0, 70.0)));
            p.spawn(shape(sq, &m.gold, Vec3::Z * 0.4, Vec2::new(40.0, 40.0)));
            p.spawn(shape(sq, &m.roof_blue, Vec3::Z * 0.5, Vec2::new(20.0, 20.0)));
            p.spawn(shape(sq, &m.gold, Vec3::new(100.0, 0.0, 0.3), Vec2::new(12.0, 34.0)));
            tower(p, TowerKind::Cannon, 92.0, 78.0, 36.0);
            tower(p, TowerKind::Cannon, 92.0, -78.0, 36.0);
            tower(p, TowerKind::Archer, -92.0, 78.0, 34.0);
            tower(p, TowerKind::Archer, -92.0, -78.0, 34.0);
            tower(p, TowerKind::Mage, 0.0, 80.0, 34.0);
            tower(p, TowerKind::Mage, 0.0, -80.0, 34.0);
            tower(p, TowerKind::Archer, 98.0, 0.0, 30.0);
            tower(p, TowerKind::Archer, -98.0, 0.0, 30.0);
        }
    }
}

fn castle_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    autopilot: Option<Res<Autopilot>>,
    mut q: Query<(&mut Transform, &mut Castle)>,
) {
    let Ok((mut tf, mut c)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let pos = tf.translation.truncate();

    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    if let Some(ap) = autopilot {
        dir = ap.direction(pos);
    }

    // Heavy, vehicle-like feel: velocity eases toward the target.
    let target = dir.normalize_or_zero() * c.def().speed;
    let ease = 1.0 - (-6.0 * dt).exp();
    c.velocity = c.velocity.lerp(target, ease);

    let lim = MAP_HALF - c.radius;
    let next = (pos + c.velocity * dt).clamp(Vec2::splat(-lim), Vec2::splat(lim));
    tf.translation.x = next.x;
    tf.translation.y = next.y;

    // Turn the castle to face where it is driving.
    if c.velocity.length_squared() > 400.0 {
        let want = c.velocity.to_angle();
        let have = tf.rotation.to_euler(EulerRot::XYZ).2;
        let diff = (want - have + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let turned = have + diff * (1.0 - (-5.0 * dt).exp());
        tf.rotation = Quat::from_rotation_z(turned);
    }
}

/// SPACE: fire a ring of cannonballs in every direction.
fn barrage(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    autopilot: Option<Res<Autopilot>>,
    assets: Res<GameAssets>,
    mut shake: ResMut<Shake>,
    mut q: Query<(&Transform, &mut Castle)>,
) {
    let Ok((tf, mut c)) = q.single_mut() else {
        return;
    };
    c.barrage_cd = (c.barrage_cd - time.delta_secs()).max(0.0);
    let pressed = keys.just_pressed(KeyCode::Space) || autopilot.is_some();
    if !pressed || c.barrage_cd > 0.0 {
        return;
    }
    c.barrage_cd = BARRAGE_COOLDOWN;
    shake.0 += 10.0;
    let pos = tf.translation.truncate();
    let n = c.def().barrage_shots;
    let dmg = 35.0 * c.def().damage_mult;
    for i in 0..n {
        let dir = Vec2::from_angle(i as f32 / n as f32 * std::f32::consts::TAU);
        commands.spawn((
            GameEntity,
            Projectile::new(dir * 650.0, dmg, 9.0, 70.0, 0, 1.0),
            shape(
                &assets.circle,
                &assets.m.cannonball,
                (pos + dir * c.radius).extend(6.0),
                Vec2::splat(18.0),
            ),
        ));
    }
}

pub(crate) fn towers_fire(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    assets: Res<GameAssets>,
    castle: Query<&Castle>,
    mut towers: Query<(&GlobalTransform, &mut Tower)>,
) {
    let Ok(c) = castle.single() else {
        return;
    };
    let mult = c.def().damage_mult;
    let dt = time.delta_secs();
    for (gtf, mut tower) in &mut towers {
        tower.timer -= dt;
        if tower.timer > 0.0 {
            continue;
        }
        let s = tower.kind.stats();
        let pos = gtf.translation().truncate();
        let Some(target) = grid.nearest(pos, s.range) else {
            tower.timer = 0.05;
            continue;
        };
        tower.timer = s.cooldown;
        let dir = (target.pos - pos).normalize_or(Vec2::X);
        let life = (s.range + 80.0) / s.speed;
        let (mat, size) = match tower.kind {
            TowerKind::Archer => (&assets.m.arrow, Vec2::new(18.0, 4.0)),
            TowerKind::Cannon => (&assets.m.cannonball, Vec2::splat(14.0)),
            TowerKind::Mage => (&assets.m.magic, Vec2::splat(15.0)),
        };
        let mesh = if tower.kind == TowerKind::Archer {
            &assets.square
        } else {
            &assets.circle
        };
        let mut t = shape(mesh, mat, pos.extend(6.0), size);
        t.2.rotation = Quat::from_rotation_z(dir.to_angle());
        commands.spawn((
            GameEntity,
            Projectile::new(dir * s.speed, s.damage * mult, s.radius, s.splash, s.pierce, life),
            t,
        ));
    }
}

/// Enemies pressed against the walls chip away at the castle.
fn contact_damage(time: Res<Time>, enemies: Query<&Enemy>, mut castle: Query<&mut Castle>) {
    let Ok(mut c) = castle.single_mut() else {
        return;
    };
    let dps: f32 = enemies.iter().filter(|e| e.touching).map(|e| e.dps).sum();
    c.hp -= dps * time.delta_secs();
}

pub(crate) fn castle_growth(
    mut commands: Commands,
    assets: Res<GameAssets>,
    grid: Res<Grid>,
    mut rng: ResMut<GameRng>,
    mut banner: ResMut<Banner>,
    mut shake: ResMut<Shake>,
    mut castle: Query<(Entity, &Transform, &mut Castle)>,
    mut enemies: Query<&mut Enemy>,
) {
    let Ok((e, tf, mut c)) = castle.single_mut() else {
        return;
    };
    let Some(need) = c.def().xp_next else {
        return;
    };
    if c.xp < need {
        return;
    }
    c.stage += 1;
    let def = c.def();
    c.radius = def.radius;
    c.hp = def.max_hp;
    banner.show(format!("Your castle grew: {}!", def.name), 3.0);
    shake.0 += 25.0;

    commands
        .entity(e)
        .despawn_related::<Children>()
        .with_children(|p| build_castle(p, &assets, c.stage));

    // Growth shockwave: hurts everything nearby so the castle has room.
    let pos = tf.translation.truncate();
    let blast = def.radius + 160.0;
    grid.for_each_near(pos, blast, |g| {
        if g.pos.distance(pos) < blast + g.radius
            && let Ok(mut en) = enemies.get_mut(g.entity)
        {
            en.hp -= 80.0;
            en.flash = 0.15;
        }
    });
    spawn_burst(&mut commands, &assets, &assets.m.spark, &mut rng, pos, 40, 420.0, 16.0, 0.7);
}

fn castle_death(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    castle: Query<(&Transform, &Castle)>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Ok((tf, c)) = castle.single() else {
        return;
    };
    if c.hp <= 0.0 {
        let pos = tf.translation.truncate();
        spawn_burst(&mut commands, &assets, &assets.m.smoke, &mut rng, pos, 60, 300.0, 30.0, 1.2);
        shake.0 += 30.0;
        next.set(GameState::GameOver);
    }
}
