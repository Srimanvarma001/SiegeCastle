//! Decides what attacks and when: marching formations that grow over time,
//! a boss every couple of minutes, and after the 5th boss an endless siege
//! where enemy numbers and strength grow exponentially.

use crate::art::p2;
use crate::assets::GameAssets;
use crate::combat::BossDefeated;
use crate::enemy::{spawn_enemy, Enemy, EnemyKind, Strength};
use crate::items::Loadout;
use crate::player::Castle;
use crate::{Banner, GameRng, GameState, MAP_HALF, RunStats, Sim};
use bevy::prelude::*;
use rand::Rng;

/// Seconds between bosses before endless mode.
pub const BOSS_INTERVAL: f32 = 150.0;
pub const FIRST_BOSS: f32 = 120.0;
/// Seconds between bosses in endless mode.
const ENDLESS_BOSS_INTERVAL: f32 = 90.0;
/// Hard cap on troops alive, so the game stays smooth. Strength keeps growing.
pub const MAX_ENEMIES: usize = 2200;

/// The five story bosses, in order. War Wagons come as a pair.
pub const BOSSES: [EnemyKind; 5] = [
    EnemyKind::SiegeTower,
    EnemyKind::WarWagon,
    EnemyKind::Juggernaut,
    EnemyKind::Trebuchet,
    EnemyKind::Warlord,
];

pub struct DirectorPlugin;

impl Plugin for DirectorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Director::default())
            .add_systems(OnEnter(GameState::Starting), |mut d: ResMut<Director>| *d = Director::default())
            .add_systems(Update, (run_director, on_boss_defeated).in_set(Sim::Spawn));
    }
}

#[derive(Resource)]
pub struct Director {
    /// How many story bosses have been sent (0..=5).
    pub boss_index: usize,
    /// Seconds until the next boss arrives.
    pub boss_timer: f32,
    pub endless: bool,
    pub endless_time: f32,
    pub strength: Strength,
    /// Story bosses beaten so far (the twin War Wagons count once).
    pub defeated: Vec<EnemyKind>,
    group_timer: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self {
            boss_index: 0,
            boss_timer: FIRST_BOSS,
            endless: false,
            endless_time: 0.0,
            strength: Strength { hp: 1.0, dmg: 1.0 },
            defeated: Vec::new(),
            group_timer: 2.0,
        }
    }
}

impl Director {
    /// Endless growth steps (one every 30 seconds).
    fn endless_steps(&self) -> f32 {
        self.endless_time / 30.0
    }

    pub fn next_boss(&self) -> Option<EnemyKind> {
        if self.endless { None } else { BOSSES.get(self.boss_index).copied() }
    }
}

fn spawn_point(cpos: Vec2, angle: f32, dist: f32) -> Vec2 {
    let lim = MAP_HALF - 8.0;
    (cpos + Vec2::from_angle(angle) * dist).clamp(Vec2::splat(-lim), Vec2::splat(lim))
}

/// A block of troops in rows, marching toward the castle.
fn spawn_formation(
    commands: &mut Commands,
    assets: &GameAssets,
    rng: &mut GameRng,
    center: Vec2,
    facing: Vec2,
    count: usize,
    mix: &[(EnemyKind, f32)],
    s: Strength,
) {
    let cols = ((count as f32).sqrt() * 1.4).ceil().max(1.0) as usize;
    let side = Vec2::new(-facing.y, facing.x);
    let spacing = 1.5;
    for i in 0..count {
        let (row, col) = (i / cols, i % cols);
        let off = side * (col as f32 - (cols - 1) as f32 / 2.0) * spacing - facing * row as f32 * spacing;
        let roll = rng.0.random::<f32>();
        let mut acc = 0.0;
        let mut kind = mix[0].0;
        for &(k, w) in mix {
            acc += w;
            if roll < acc {
                kind = k;
                break;
            }
        }
        // Archers march at the back.
        let off = if kind == EnemyKind::Archer { off - facing * 2.0 } else { off };
        let pos = center + off;
        spawn_enemy(commands, assets, kind, pos, facing.to_angle(), off, s);
    }
}

fn run_director(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    stats: Res<RunStats>,
    loadout: Res<Loadout>,
    mut d: ResMut<Director>,
    mut rng: ResMut<GameRng>,
    mut banner: ResMut<Banner>,
    castle: Query<(&Transform, &Castle)>,
    enemies: Query<(), With<Enemy>>,
) {
    let Ok((ctf, c)) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let cpos = p2(ctf.translation);
    let minutes = stats.time / 60.0;
    if d.endless {
        d.endless_time += dt;
    }
    let e = d.endless_steps();

    // Strength grows slowly during the story, exponentially once endless.
    let mut hp = 1.0 + 0.22 * minutes;
    let mut dmg = 1.0 + 0.12 * minutes;
    let mut rate = 1.0;
    let mut size = 1.0;
    if d.endless {
        hp *= 1.22f32.powf(e);
        dmg *= 1.10f32.powf(e);
        rate = 1.15f32.powf(e);
        size = 1.10f32.powf(e);
    }
    d.strength = Strength { hp, dmg };

    // How far away troops appear: just outside the camera view.
    let view = 30.0 * [1.0, 1.3, 1.65][loadout.stage.min(2)] + 6.0;
    let alive = enemies.iter().count();

    d.group_timer -= dt;
    if d.group_timer <= 0.0 {
        d.group_timer = ((4.2 - 0.3 * minutes).max(1.2) / rate).max(0.25);
        if alive < MAX_ENEMIES {
            // Often come from ahead, so you drive into them.
            let ahead = c.vel.length() > 4.0 && rng.0.random::<f32>() < 0.45;
            let angle = if ahead {
                c.heading + rng.0.random_range(-0.7..0.7)
            } else {
                rng.0.random::<f32>() * std::f32::consts::TAU
            };
            let center = spawn_point(cpos, angle, view + rng.0.random_range(0.0..15.0));
            let facing = (cpos - center).normalize_or(Vec2::X);
            let roll = rng.0.random::<f32>();
            let budget = MAX_ENEMIES - alive;
            let (count, mix): (f32, Vec<(EnemyKind, f32)>) = if minutes > 5.0 && roll < 0.12 {
                (2.0 + minutes * 0.4, vec![(EnemyKind::Cart, 1.0)])
            } else if minutes > 2.5 && roll < 0.35 {
                (5.0 + minutes * 2.0, vec![(EnemyKind::Knight, 1.0)])
            } else if minutes > 1.0 {
                (12.0 + minutes * 5.0, vec![(EnemyKind::Footman, 0.75), (EnemyKind::Archer, 0.25)])
            } else {
                (8.0 + minutes * 6.0, vec![(EnemyKind::Footman, 1.0)])
            };
            let n = ((count * size) as usize).clamp(1, 80).min(budget);
            spawn_formation(&mut commands, &assets, &mut rng, center, facing, n, &mix, d.strength);
        }
    }

    // Bosses on a countdown.
    d.boss_timer -= dt;
    if d.boss_timer > 0.0 {
        return;
    }
    let boss_strength = Strength { hp: d.strength.hp.max(1.0), dmg: d.strength.dmg };
    let kind = if d.endless {
        d.boss_timer = ENDLESS_BOSS_INTERVAL;
        BOSSES[rng.0.random_range(0..BOSSES.len())]
    } else if d.boss_index < BOSSES.len() {
        let k = BOSSES[d.boss_index];
        d.boss_index += 1;
        d.boss_timer = if d.boss_index < BOSSES.len() { BOSS_INTERVAL } else { f32::INFINITY };
        k
    } else {
        d.boss_timer = f32::INFINITY;
        return;
    };
    let count = if kind == EnemyKind::WarWagon { 2 } else { 1 };
    let angle = rng.0.random::<f32>() * std::f32::consts::TAU;
    for i in 0..count {
        let p = spawn_point(cpos, angle + i as f32 * 0.5, view + 12.0);
        spawn_enemy(&mut commands, &assets, kind, p, (cpos - p).to_angle(), Vec2::ZERO, boss_strength);
    }
    let title = if count > 1 { "Twin War Wagons approach!".to_string() } else { format!("{} approaches!", kind.name()) };
    banner.show(title, "Follow the red arrow", 4.0);
}

fn on_boss_defeated(
    mut msgs: MessageReader<BossDefeated>,
    mut d: ResMut<Director>,
    mut banner: ResMut<Banner>,
    bosses: Query<&Enemy, With<crate::enemy::Boss>>,
) {
    for BossDefeated(kind) in msgs.read() {
        let others_left = bosses.iter().any(|e| e.kind == *kind && e.hp > 0.0);
        if others_left {
            banner.show(format!("{} destroyed!", kind.name()), "One more to go", 3.0);
            continue;
        }
        if !d.endless && !d.defeated.contains(kind) {
            d.defeated.push(*kind);
        }
        if *kind == EnemyKind::Warlord && !d.endless {
            d.endless = true;
            d.endless_time = 0.0;
            d.boss_timer = ENDLESS_BOSS_INTERVAL;
            banner.show("ALL 5 BOSSES DEFEATED!", "The endless siege begins. How long can you last?", 6.0);
        } else {
            banner.show(format!("{} destroyed!", kind.name()), "", 3.0);
        }
    }
}
