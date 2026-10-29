//! Spatial grid for enemies. Rebuilt every frame so that "who is near me?"
//! only checks a few cells instead of every enemy on the map.

use crate::art::p2;
use crate::enemy::Enemy;
use crate::{MAP_HALF, Sim};
use bevy::prelude::*;

const CELL: f32 = 4.0;
const MARGIN: f32 = 30.0;

/// Largest enemy radius. Searches add this so big enemies are never missed.
pub const MAX_ENEMY_RADIUS: f32 = 9.0;

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
}

#[derive(Resource)]
pub struct Grid {
    cells: Vec<Vec<GridEntry>>,
    size: i32,
    origin: f32,
}

impl Grid {
    fn new() -> Self {
        let span = 2.0 * (MAP_HALF + MARGIN);
        let size = (span / CELL).ceil() as i32;
        Self {
            cells: vec![Vec::new(); (size * size) as usize],
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
    }

    fn insert(&mut self, e: GridEntry) {
        let (x, y) = (self.coord(e.pos.x), self.coord(e.pos.y));
        self.cells[(y * self.size + x) as usize].push(e);
    }

    /// Calls `f` for every enemy whose center is within the cells touching
    /// the square around `p` with half-size `r`. Callers do the exact distance test.
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
    }

    /// Nearest enemy to `p` within `range`.
    pub fn nearest(&self, p: Vec2, range: f32) -> Option<GridEntry> {
        let mut best: Option<GridEntry> = None;
        let mut best_d2 = f32::MAX;
        self.for_each_near(p, range + MAX_ENEMY_RADIUS, |e| {
            let reach = range + e.radius;
            let d2 = e.pos.distance_squared(p);
            if d2 < reach * reach && d2 < best_d2 {
                best_d2 = d2;
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
            });
        }
    }
}
