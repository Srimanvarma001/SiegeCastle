//! Spatial grid for enemies. Rebuilt every frame so that "who is near me?"
//! only checks a few cells instead of every enemy on the map.
//!
//! Troops are small circles stored in cells. Bosses are huge capsules (a
//! segment thickened by a radius) and are kept in a short separate list that
//! every search also looks at.

use crate::art::p2;
use crate::enemy::Enemy;
use crate::{MAP_HALF, Sim};
use bevy::prelude::*;

const CELL: f32 = 4.0;
const MARGIN: f32 = 30.0;

/// Largest troop radius. Searches add this so big troops are never missed.
/// Bosses are not limited by it: they live in their own list.
pub const MAX_ENEMY_RADIUS: f32 = 2.0;

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::new())
            .add_systems(Update, rebuild_grid.in_set(Sim::Grid));
    }
}

#[derive(Clone, Copy)]
pub struct GridEntry {
    pub entity: Entity,
    pub pos: Vec2,
    pub radius: f32,
    /// Half of the capsule's length, along its facing. Zero for troops.
    pub axis: Vec2,
}

impl GridEntry {
    /// Point of the entry's center line closest to `p`.
    pub fn closest(&self, p: Vec2) -> Vec2 {
        if self.axis == Vec2::ZERO {
            return self.pos;
        }
        closest_on_segment(self.pos - self.axis, self.pos + self.axis, p)
    }

    /// Squared distance from `p` to the center line.
    pub fn dist2(&self, p: Vec2) -> f32 {
        self.closest(p).distance_squared(p)
    }
}

pub fn closest_on_segment(a: Vec2, b: Vec2, p: Vec2) -> Vec2 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    a + ab * t
}

#[derive(Resource)]
pub struct Grid {
    cells: Vec<Vec<GridEntry>>,
    /// Bosses: few, huge, always checked.
    big: Vec<GridEntry>,
    size: i32,
    origin: f32,
}

impl Grid {
    fn new() -> Self {
        let span = 2.0 * (MAP_HALF + MARGIN);
        let size = (span / CELL).ceil() as i32;
        Self {
            cells: vec![Vec::new(); (size * size) as usize],
            big: Vec::new(),
            size,
            origin: -(MAP_HALF + MARGIN),
        }
    }

    fn coord(&self, v: f32) -> i32 {
        (((v - self.origin) / CELL) as i32).clamp(0, self.size - 1)
    }

    fn clear(&mut self) {
        for c in &mut self.cells {
            c.clear();
        }
        self.big.clear();
    }

    fn insert(&mut self, e: GridEntry) {
        if e.radius > MAX_ENEMY_RADIUS || e.axis != Vec2::ZERO {
            self.big.push(e);
            return;
        }
        let (x, y) = (self.coord(e.pos.x), self.coord(e.pos.y));
        self.cells[(y * self.size + x) as usize].push(e);
    }

    /// Calls `f` for every troop whose center is within the cells touching
    /// the square around `p` with half-size `r`, and for every boss that
    /// might reach that far. Callers do the exact distance test.
    pub fn for_each_near(&self, p: Vec2, r: f32, mut f: impl FnMut(&GridEntry)) {
        let (x0, x1) = (self.coord(p.x - r), self.coord(p.x + r));
        let (y0, y1) = (self.coord(p.y - r), self.coord(p.y + r));
        for y in y0..=y1 {
            let row = y * self.size;
            for x in x0..=x1 {
                for e in &self.cells[(row + x) as usize] {
                    f(e);
                }
            }
        }
        for e in &self.big {
            let reach = r + e.radius + e.axis.length();
            if e.pos.distance_squared(p) < reach * reach {
                f(e);
            }
        }
    }

    /// Nearest enemy to `p` within `range` (measured to a boss's outline).
    pub fn nearest(&self, p: Vec2, range: f32) -> Option<GridEntry> {
        let mut best: Option<GridEntry> = None;
        let mut best_d = f32::MAX;
        self.for_each_near(p, range + MAX_ENEMY_RADIUS, |e| {
            let d = e.dist2(p).sqrt() - e.radius;
            if d < range && d < best_d {
                best_d = d;
                best = Some(*e);
            }
        });
        best
    }
}

fn rebuild_grid(mut grid: ResMut<Grid>, enemies: Query<(Entity, &Transform, &Enemy)>) {
    grid.clear();
    for (entity, tf, enemy) in &enemies {
        if enemy.hp > 0.0 {
            grid.insert(GridEntry {
                entity,
                pos: p2(tf.translation),
                radius: enemy.radius,
                axis: enemy.axis(tf),
            });
        }
    }
}
