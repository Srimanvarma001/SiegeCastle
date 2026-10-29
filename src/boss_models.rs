//! The five colossal bosses, built in code like every other model.
//!
//! Each boss is a rig: a set of parts with their own pivot so they can move
//! on their own (wheels roll, turrets turn, the ram swings, the trebuchet arm
//! throws, the portcullis lifts). `boss.rs` animates them.
//!
//! Bosses are built at true size (UVs in meters), so stone, timber and iron
//! keep the same texture scale as the player's castle. That is what makes them
//! read as huge: the same bricks, many more of them.

use crate::art::rgb;
use crate::models::{self as md, Mat, Model, W, sh};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// What a part does when animated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartKind {
    /// Does not move by itself.
    Body,
    /// Rolls with the distance driven. `k` is the radius; slot 1 is mirrored.
    Wheel,
    /// The tall body above the chassis: leans a little while moving.
    Sway,
    /// Turns toward the castle by itself. `k` is the muzzle distance.
    Turret,
    /// Slides back when its turret fires. `k` is the recoil distance.
    Barrel,
    /// Drawbridge hinged on local Z. `k` is the open angle.
    Bridge,
    /// Gun port lid hinged on local X. `k` is the open angle.
    Port,
    /// Lifts by `k` meters when open.
    Portcullis,
    /// The battering ram log: moves along a pendulum arc of length `k`.
    RamBeam,
    /// A chain holding the ram: swings with it.
    RamChain,
    /// The trebuchet throwing arm, turning about local Z.
    Arm,
    /// Hangs under the arm: slot 0 counterweight, slot 1 sling.
    Hanging,
    /// The boulder in the sling. Hidden after release.
    Load,
    /// Fire that flickers. Slot 1 only burns in the last phase.
    Glow,
    /// Waves in the wind.
    Flag,
    /// Treadwheel that spins while the trebuchet reloads.
    Treadwheel,
    /// The Warlord's great cannon: pitches up and recoils by `k`.
    Bombard,
}

#[derive(Clone, Copy, Debug)]
pub struct PartDef {
    pub kind: PartKind,
    pub parent: Option<usize>,
    pub at: Vec3,
    pub rot: Quat,
    pub model: usize,
    pub k: f32,
    pub slot: u8,
}

/// Models plus the parts that use them (several wheels share one model).
#[derive(Default)]
pub struct RigBuild {
    pub models: Vec<Model>,
    pub parts: Vec<PartDef>,
}

impl RigBuild {
    fn model(&mut self, m: Model) -> usize {
        self.models.push(m);
        self.models.len() - 1
    }

    fn part(&mut self, kind: PartKind, parent: Option<usize>, at: Vec3, model: usize, k: f32, slot: u8) -> usize {
        self.parts.push(PartDef { kind, parent, at, rot: Quat::IDENTITY, model, k, slot });
        self.parts.len() - 1
    }

    /// The body: everything that does not move by itself.
    fn body(&mut self, m: Model) -> usize {
        let id = self.model(m);
        self.part(PartKind::Body, None, Vec3::ZERO, id, 0.0, 0)
    }

    /// A pair of wheels per axle. Mirrored wheels face out on the left side.
    fn wheels(&mut self, model: usize, xs: &[f32], y: f32, z: f32, r: f32, mirror: bool) {
        for &x in xs {
            self.part(PartKind::Wheel, None, Vec3::new(x, y, z), model, r, 0);
            let i = self.part(PartKind::Wheel, None, Vec3::new(x, y, -z), model, r, mirror as u8);
            if mirror {
                self.parts[i].rot = Quat::from_rotation_y(PI);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

fn black() -> Color {
    rgb(0.03, 0.025, 0.02)
}

fn gold() -> Color {
    rgb(0.85, 0.62, 0.22)
}

/// Rotation that turns local +Z to face `out` (about the vertical only).
fn face_rot(out: Vec3) -> Quat {
    Quat::from_rotation_y(out.x.atan2(out.z))
}

/// A box between two points, `w` by `d` thick.
fn beam(m: &mut Model, mat: Mat, a: Vec3, b: Vec3, w: f32, d: f32, c: Color) {
    let v = b - a;
    let rot = Quat::from_rotation_arc(Vec3::Y, v.normalize());
    m.cube(mat, Vec3::new(w, v.length(), d), (a + b) / 2.0, rot, c);
}

/// A dark arrow slit set into a wall facing `out`.
fn slit(m: &mut Model, center: Vec3, out: Vec3, w: f32, h: f32) {
    m.cube(Mat::Paint, Vec3::new(w, h, 0.14), center + out * 0.03, face_rot(out), black());
}

/// A tall pointed window lit from inside by fire.
fn ember_window(m: &mut Model, center: Vec3, out: Vec3, w: f32, h: f32, frame: Color) {
    let rot = face_rot(out);
    let at = |x: f32, y: f32| center + rot * Vec3::new(x, y, 0.05);
    let hw = w / 2.0;
    let quad = [at(-hw, -h / 2.0), at(hw, -h / 2.0), at(hw, h / 2.0), at(-hw, h / 2.0)];
    m.poly(Mat::Ember, W, &quad, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], out);
    let arch = [at(-hw, h / 2.0), at(hw, h / 2.0), at(0.0, h / 2.0 + w * 0.8)];
    m.poly(Mat::Ember, W, &arch, &[Vec2::ZERO, Vec2::X, Vec2::Y], out);
    m.cube(Mat::Stone, Vec3::new(w + 0.6, 0.35, 0.4), center + rot * Vec3::new(0.0, -h / 2.0 - 0.17, 0.12), rot, frame);
    for s in [-1.0f32, 1.0] {
        m.cube(Mat::Stone, Vec3::new(0.3, h + w * 0.5, 0.3), center + rot * Vec3::new(s * (hw + 0.15), w * 0.25, 0.1), rot, frame);
    }
}

/// Merlons from `a` to `b`, sized for giant walls.
fn crenels(m: &mut Model, mat: Mat, c: Color, a: Vec3, b: Vec3, thick: f32, mh: f32, step: f32) {
    let d = b - a;
    let len = d.length();
    let dir = d / len;
    let rot = Quat::from_rotation_y(-dir.z.atan2(dir.x));
    let n = (len / step).floor().max(1.0) as i32;
    let st = len / n as f32;
    for i in 0..n {
        let p = a + dir * (st * (i as f32 + 0.5)) + Vec3::Y * (mh / 2.0);
        m.cube(mat, Vec3::new(st * 0.55, mh, thick), p, rot, c);
    }
}

/// A curtain wall with a string course, slits, buttresses and merlons.
/// `a` and `b` are the ends on the ground plane (x, z); `out` points outside.
#[allow(clippy::too_many_arguments)]
fn great_wall(m: &mut Model, a: Vec2, b: Vec2, y0: f32, h: f32, thick: f32, c: Color, merlon: f32, out: Vec3) {
    let mid = (a + b) / 2.0;
    let d = b - a;
    let len = d.length();
    let dir3 = Vec3::new(d.x, 0.0, d.y) / len;
    let rot = Quat::from_rotation_y(-d.y.atan2(d.x));
    m.cube(Mat::Stone, Vec3::new(len, h, thick), Vec3::new(mid.x, y0 + h / 2.0, mid.y), rot, c);
    let top = Vec3::new(mid.x, y0 + h, mid.y);
    // String course under the parapet.
    m.cube(Mat::Stone, Vec3::new(len + 0.2, 0.5, thick + 0.5), top - Vec3::Y * 0.6, rot, sh(c, 0.82));
    let pa = Vec3::new(a.x, y0 + h, a.y);
    let pb = Vec3::new(b.x, y0 + h, b.y);
    let side = out * (thick / 2.0 - merlon * 0.2);
    crenels(m, Mat::Stone, c, pa + side, pb + side, merlon * 0.45, merlon, merlon * 1.35);
    crenels(m, Mat::Stone, sh(c, 0.9), pa - side, pb - side, merlon * 0.35, merlon * 0.6, merlon * 1.35);
    // Buttresses and slits along the outer face.
    let n = (len / 7.0).floor().max(1.0) as i32;
    for i in 0..=n {
        let p = Vec3::new(a.x, y0, a.y) + dir3 * (len * i as f32 / n as f32);
        m.cube(Mat::Stone, Vec3::new(1.2, h * 0.7, 1.2), p + out * (thick / 2.0 + 0.4) + Vec3::Y * (h * 0.35), rot, sh(c, 0.94));
        if i < n {
            let q = p + dir3 * (len / n as f32 / 2.0) + out * (thick / 2.0);
            slit(m, q + Vec3::Y * (h * 0.4), out, 0.4, 1.8);
            slit(m, q + Vec3::Y * (h * 0.72), out, 0.35, 1.4);
        }
    }
}

/// Round tower for giant walls: plinth, string courses, slits, a corbelled
/// parapet with merlons, and an optional slate cone.
fn great_tower(m: &mut Model, base: Vec3, r: f32, h: f32, c: Color, cone: Option<(f32, Color)>) {
    let seg = 24;
    m.frustum(Mat::Stone, base, r * 1.12, r * 1.04, 1.4, seg, sh(c, 0.92), true);
    m.frustum(Mat::Stone, base + Vec3::Y * 1.4, r * 1.04, r, h - 2.4, seg, c, true);
    let mut y = 6.0;
    while y < h - 3.0 {
        m.frustum(Mat::Stone, base + Vec3::Y * y, r * 1.025, r * 1.025, 0.4, seg, sh(c, 0.84), true);
        y += 6.5;
    }
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU + 0.3;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let mut y = 4.5;
        while y < h - 3.5 {
            slit(m, base + Vec3::Y * y + out * r, out, 0.4, 1.7);
            y += 6.5;
        }
    }
    let top = base + Vec3::Y * h;
    // Corbels under the parapet.
    m.frustum(Mat::Stone, top - Vec3::Y * 1.0, r, r * 1.16, 1.0, seg, sh(c, 0.86), true);
    let n = ((TAU * r) / 1.3).round() as i32;
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU;
        let p = top - Vec3::Y * 1.4 + Vec3::new(a.cos(), 0.0, a.sin()) * (r * 1.03);
        m.cube(Mat::Stone, Vec3::new(0.7, 0.8, 0.5), p, Quat::from_rotation_y(-a), sh(c, 0.8));
    }
    m.frustum(Mat::Stone, top, r * 1.16, r * 1.16, 1.0, seg, c, true);
    let n = ((TAU * r * 1.16) / 2.2).round() as i32;
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU;
        let p = top + Vec3::new(a.cos(), 0.0, a.sin()) * (r * 1.1) + Vec3::Y * 1.6;
        m.cube(Mat::Stone, Vec3::new(0.6, 1.3, 1.1), p, Quat::from_rotation_y(-a), c);
    }
    if let Some((ch, cc)) = cone {
        m.frustum(Mat::Slate, top + Vec3::Y * 1.0, r * 1.28, 0.0, ch, 12, cc, false);
        m.frustum(Mat::Gold, top + Vec3::Y * (ch + 0.8), 0.12, 0.0, 1.6, 6, W, true);
    }
}

/// Square tower for giant gatehouses, with slits and merlons.
fn great_square_tower(m: &mut Model, base: Vec3, w: f32, h: f32, c: Color) {
    m.boxy(Mat::Stone, Vec3::new(w + 0.8, 1.4, w + 0.8), base + Vec3::Y * 0.7, sh(c, 0.92));
    m.boxy(Mat::Stone, Vec3::new(w, h, w), base + Vec3::Y * (h / 2.0), c);
    let top = base + Vec3::Y * h;
    m.boxy(Mat::Stone, Vec3::new(w + 1.0, 1.0, w + 1.0), top - Vec3::Y * 0.5, sh(c, 0.84));
    let hw = w / 2.0 + 0.3;
    for (a, b) in [((-hw, -hw), (hw, -hw)), ((hw, -hw), (hw, hw)), ((hw, hw), (-hw, hw)), ((-hw, hw), (-hw, -hw))] {
        crenels(m, Mat::Stone, c, top + Vec3::new(a.0, 0.0, a.1), top + Vec3::new(b.0, 0.0, b.1), 0.7, 1.4, 2.0);
    }
    for out in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
        let mut y = 4.0;
        while y < h - 3.0 {
            slit(m, base + Vec3::Y * y + out * (w / 2.0), out, 0.45, 1.8);
            y += 5.5;
        }
    }
}

/// Iron studs in a ring on both faces, so turning wheels are easy to see.
fn studs(m: &mut Model, r: f32, w: f32, n: u32) {
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        for s in [-1.0f32, 1.0] {
            let p = Vec3::new(a.cos() * r, a.sin() * r, s * (w / 2.0 + 0.04));
            m.cube(Mat::Iron, Vec3::new(r * 0.12, r * 0.12, 0.12), p, Quat::from_rotation_z(a), W);
        }
    }
}

fn spoked(r: f32, w: f32) -> Model {
    let mut m = Model::new();
    md::spoked_wheel(&mut m, Vec3::ZERO, r, w);
    studs(&mut m, r * 0.97, w, 14);
    m
}

/// Solid timber wheel with an iron cross on each face.
fn solid(r: f32, w: f32) -> Model {
    let mut m = Model::new();
    md::wheel(&mut m, Vec3::ZERO, r, w);
    for s in [-1.0f32, 1.0] {
        for a in [0.0, FRAC_PI_2] {
            m.cube(Mat::Iron, Vec3::new(r * 1.8, r * 0.16, 0.1), Vec3::new(0.0, 0.0, s * (w / 2.0 + 0.05)), Quat::from_rotation_z(a + 0.4), W);
        }
    }
    studs(&mut m, r * 0.96, w + 0.08, 16);
    m
}

/// A banner hanging from its pole top (the pivot), streaming along +X.
fn pennant(len: f32, h: f32, c: Color) -> Model {
    let mut m = Model::new();
    let pts = [Vec3::ZERO, Vec3::new(len, -h * 0.12, 0.0), Vec3::new(len * 0.92, -h * 0.62, 0.0), Vec3::new(0.0, -h, 0.0)];
    m.poly(Mat::Cloth, c, &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], Vec3::Z);
    let s = [Vec3::new(0.0, -h * 0.4, 0.03), Vec3::new(len * 0.96, -h * 0.42, 0.03), Vec3::new(len * 0.95, -h * 0.52, 0.03), Vec3::new(0.0, -h * 0.55, 0.03)];
    m.poly(Mat::Cloth, gold(), &s, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], Vec3::Z);
    m
}

/// A pole standing on `base` whose waving banner is a separate part.
fn flag(rb: &mut RigBuild, body: &mut Model, parent: usize, base: Vec3, pole: f32, len: f32, c: Color, slot: u8) {
    body.frustum(Mat::Iron, base, 0.14, 0.1, pole, 6, W, true);
    body.frustum(Mat::Gold, base + Vec3::Y * pole, 0.2, 0.0, 0.6, 6, W, true);
    let id = rb.model(pennant(len, len * 0.5, c));
    rb.part(PartKind::Flag, Some(parent), base + Vec3::Y * (pole - 0.2), id, 0.0, slot);
}

/// A flickering fire in an iron bowl. The flames are a separate part.
fn brazier(rb: &mut RigBuild, body: &mut Model, parent: usize, base: Vec3, r: f32, slot: u8) {
    body.frustum(Mat::Iron, base, r * 0.5, r, r * 0.8, 12, W, true);
    let mut f = Model::new();
    f.blob(Mat::Ember, Vec3::Y * (r * 0.4), r * 0.75, Vec3::new(1.0, 1.5, 1.0), W, 0.25, 77 + slot as u32, 1);
    f.blob(Mat::Ember, Vec3::Y * (r * 1.2), r * 0.4, Vec3::new(1.0, 1.6, 1.0), W, 0.3, 91 + slot as u32, 1);
    let id = rb.model(f);
    rb.part(PartKind::Glow, Some(parent), base + Vec3::Y * (r * 0.8), id, 0.0, 0);
}

/// Prism with a rectangular bottom (bl × bw) and a smaller top (tl × tw).
#[allow(clippy::too_many_arguments)]
fn sloped(m: &mut Model, mat: Mat, c: Color, base: Vec3, bl: f32, bw: f32, tl: f32, tw: f32, h: f32) {
    let b = |x: f32, z: f32| base + Vec3::new(x, 0.0, z);
    let t = |x: f32, z: f32| base + Vec3::new(x, h, z);
    let (bx, bz, tx, tz) = (bl / 2.0, bw / 2.0, tl / 2.0, tw / 2.0);
    for s in [-1.0f32, 1.0] {
        let slope = Vec2::new(bz - tz, h).length();
        let pts = [b(-bx, s * bz), b(bx, s * bz), t(tx, s * tz), t(-tx, s * tz)];
        let uvs = [Vec2::new(0.0, slope), Vec2::new(bl, slope), Vec2::new((bl + tl) / 2.0, 0.0), Vec2::new((bl - tl) / 2.0, 0.0)];
        m.poly(mat, c, &pts, &uvs, Vec3::new(0.0, bz - tz, s * h));
        let slope = Vec2::new(bx - tx, h).length();
        let pts = [b(s * bx, -bz), b(s * bx, bz), t(s * tx, tz), t(s * tx, -tz)];
        let uvs = [Vec2::new(0.0, slope), Vec2::new(bw, slope), Vec2::new((bw + tw) / 2.0, 0.0), Vec2::new((bw - tw) / 2.0, 0.0)];
        m.poly(mat, c, &pts, &uvs, Vec3::new(s * h, bx - tx, 0.0));
    }
    let top = [t(-tx, -tz), t(tx, -tz), t(tx, tz), t(-tx, tz)];
    m.poly(mat, c, &top, &[Vec2::ZERO, Vec2::new(tl, 0.0), Vec2::new(tl, tw), Vec2::new(0.0, tw)], Vec3::Y);
}

/// Rows of overlapping iron plates on a slope with outward normal `n`.
fn plates(m: &mut Model, from: Vec3, along: Vec3, up: Vec3, n: Vec3, cols: u32, rows: u32, size: Vec2) {
    let rot = Quat::from_rotation_arc(Vec3::Y, n.normalize()) ;
    for r in 0..rows {
        for i in 0..cols {
            let stagger = if r % 2 == 1 { 0.5 } else { 0.0 };
            let p = from + along * ((i as f32 + stagger) * size.x) + up * (r as f32 * size.y * 0.85) + n.normalize() * 0.18;
            m.cube(Mat::Iron, Vec3::new(size.x * 0.96, 0.22, size.y), p, rot, sh(W, 0.86 + 0.1 * ((i + r) % 2) as f32));
        }
    }
}

/// Small iron rivets along a line.
fn rivets(m: &mut Model, a: Vec3, b: Vec3, n: u32, s: f32) {
    for i in 0..n {
        let p = a.lerp(b, (i as f32 + 0.5) / n as f32);
        m.boxy(Mat::Iron, Vec3::splat(s), p, sh(W, 0.7));
    }
}

/// A round painted shield facing `out`.
fn shield(m: &mut Model, center: Vec3, out: Vec3, r: f32, c: Color) {
    let rot = Quat::from_rotation_arc(Vec3::Y, out);
    m.frustum_rot(Mat::Paint, center, rot, r, r * 0.92, 0.12, 14, c, true, true);
    m.frustum_rot(Mat::Gold, center + out * 0.12, rot, r * 0.22, r * 0.1, 0.12, 8, W, true, true);
}

// ---------------------------------------------------------------------------
// 1. The Siege Tower: a five-story belfry on eight wheels
// ---------------------------------------------------------------------------

pub const TOWER_DECK: f32 = 3.0;
/// Where footmen step off the lowered ramp (model space).
pub const TOWER_RAMP_END: Vec3 = Vec3::new(16.0, 0.0, 0.0);

pub fn siege_tower() -> RigBuild {
    let mut rb = RigBuild::default();
    let deck = TOWER_DECK;

    // Chassis (root body): deck, iron bands, axles.
    let mut base = Model::new();
    base.boxy(Mat::Wood, Vec3::new(18.0, 1.4, 16.0), Vec3::new(0.0, deck - 0.7, 0.0), W);
    for i in 0..8 {
        let x = -7.7 + i as f32 * 2.2;
        base.boxy(Mat::Iron, Vec3::new(0.28, 1.48, 16.08), Vec3::new(x, deck - 0.7, 0.0), W);
    }
    for x in [-6.3f32, -2.1, 2.1, 6.3] {
        base.frustum_rot(Mat::Iron, Vec3::new(x, 2.0, -8.9), Quat::from_rotation_x(FRAC_PI_2), 0.32, 0.32, 17.8, 8, W, true, false);
    }
    let root = rb.body(base);

    // The tower itself sways on the chassis.
    let mut t = Model::new();
    let hide = [rgb(0.40, 0.28, 0.18), rgb(0.47, 0.34, 0.22), rgb(0.34, 0.24, 0.16)];
    let story = 6.0;
    for i in 0..5 {
        let s = 15.0 - i as f32 * 0.8;
        let hs = s / 2.0;
        let y0 = i as f32 * story;
        t.boxy(Mat::Wood, Vec3::new(s - 0.6, story, s - 0.6), Vec3::new(0.0, y0 + story / 2.0, 0.0), sh(W, 0.6));
        for (cx, cz) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            t.boxy(Mat::Wood, Vec3::new(1.0, story, 1.0), Vec3::new(cx * (hs - 0.4), y0 + story / 2.0, cz * (hs - 0.4)), W);
        }
        t.boxy(Mat::Wood, Vec3::new(s + 0.2, 0.8, s + 0.2), Vec3::new(0.0, y0 + 0.4, 0.0), W);
        t.boxy(Mat::Iron, Vec3::new(s + 0.26, 0.24, s + 0.26), Vec3::new(0.0, y0 + story - 0.18, 0.0), W);
        for (k, out) in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z].into_iter().enumerate() {
            let rot = face_rot(out);
            let along = rot * Vec3::X;
            let door = i == 0 && k == 0;
            let w = hs - 1.4;
            let mid_y = y0 + 0.8 + (story - 1.1) / 2.0;
            for (j, off) in [-1.0f32, 1.0].into_iter().enumerate() {
                if door {
                    continue;
                }
                let c = Vec3::new(0.0, mid_y, 0.0) + out * (hs - 0.2) + along * (off * (w / 2.0 + 0.35));
                t.cube(Mat::Paint, Vec3::new(w, story - 1.3, 0.3), c, rot, hide[(i + j + k) % 3]);
                // Lacing that holds the hides on.
                for l in 0..3 {
                    let y = mid_y - 1.6 + l as f32 * 1.6;
                    t.cube(Mat::Paint, Vec3::new(w * 0.94, 0.08, 0.36), Vec3::new(c.x, y, c.z), rot, rgb(0.20, 0.14, 0.09));
                }
                // Diagonal brace across each panel.
                let tilt = Quat::from_rotation_z(off * 0.7);
                let diag = (w * w + (story - 1.3).powi(2)).sqrt();
                t.cube(Mat::Wood, Vec3::new(diag, 0.4, 0.3), c + out * 0.25, rot * tilt, sh(W, 0.85));
                slit(&mut t, Vec3::new(0.0, y0 + 3.6, 0.0) + out * (hs - 0.02) + along * (off * hs * 0.55), out, 0.42, 1.7);
            }
            t.cube(Mat::Wood, Vec3::new(0.8, story - 0.8, 0.5), Vec3::new(0.0, y0 + 0.4 + (story - 0.8) / 2.0, 0.0) + out * (hs - 0.2), rot, W);
        }
    }
    // The dark doorway behind the ramp.
    t.boxy(Mat::Paint, Vec3::new(0.2, 5.4, 6.8), Vec3::new(7.32, 2.9, 0.0), black());

    // Fighting platform with hoardings, a hide roof and archers.
    let ty = 30.0;
    let ts = 13.4;
    t.boxy(Mat::Wood, Vec3::new(ts, 0.9, ts), Vec3::new(0.0, ty + 0.45, 0.0), W);
    for i in 0..9 {
        let a = -ts / 2.0 + 0.8 + i as f32 * (ts - 1.6) / 8.0;
        for out in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
            let along = face_rot(out) * Vec3::X;
            beam(&mut t, Mat::Wood, along * a + out * 5.6 + Vec3::Y * (ty - 2.0), along * a + out * (ts / 2.0 - 0.2) + Vec3::Y * ty, 0.35, 0.35, timber_dark());
        }
    }
    let h = ts / 2.0 - 0.2;
    for (a, b, out) in [
        (Vec3::new(h, 0.0, -h), Vec3::new(h, 0.0, h), Vec3::X),
        (Vec3::new(-h, 0.0, -h), Vec3::new(-h, 0.0, h), -Vec3::X),
        (Vec3::new(-h, 0.0, h), Vec3::new(h, 0.0, h), Vec3::Z),
        (Vec3::new(-h, 0.0, -h), Vec3::new(h, 0.0, -h), -Vec3::Z),
    ] {
        let lift = Vec3::Y * (ty + 0.9);
        let rot = face_rot(out);
        t.cube(Mat::Wood, Vec3::new(ts, 0.9, 0.35), (a + b) / 2.0 + lift + Vec3::Y * 0.45, rot, W);
        crenels(&mut t, Mat::Wood, W, a + lift + Vec3::Y * 0.9, b + lift + Vec3::Y * 0.9, 0.35, 1.3, 1.9);
        for i in 0..5 {
            let p = a.lerp(b, (i as f32 + 0.5) / 5.0) + lift + Vec3::Y * 0.5 + out * 0.3;
            shield(&mut t, p, out, 0.75, if i % 2 == 0 { md::crimson() } else { md::royal() });
        }
    }
    for (x, z) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (-6.0 / 13.4, -1.0), (-6.0 / 13.4, 1.0)] {
        t.boxy(Mat::Wood, Vec3::new(0.45, 4.2, 0.45), Vec3::new(x * (h - 0.5), ty + 0.9 + 2.1, z * (h - 0.5)), W);
    }
    t.roof(Vec3::new(-3.6, ty + 5.1, 0.0), Quat::from_rotation_y(FRAC_PI_2), ts + 0.4, 6.4, 2.6, 0.5, hide[0], Mat::Wood, W, Mat::Paint);
    for (i, (p, ang)) in [
        (Vec3::new(5.6, 0.0, -3.6), 0.0),
        (Vec3::new(5.6, 0.0, 0.0), 0.0),
        (Vec3::new(5.6, 0.0, 3.6), 0.0),
        (Vec3::new(2.0, 0.0, 5.6), -FRAC_PI_2),
        (Vec3::new(-2.5, 0.0, 5.6), -FRAC_PI_2),
        (Vec3::new(2.0, 0.0, -5.6), FRAC_PI_2),
        (Vec3::new(-2.5, 0.0, -5.6), FRAC_PI_2),
        (Vec3::new(-5.4, 0.0, 1.5), PI),
    ]
    .into_iter()
    .enumerate()
    {
        let mut a = Model::new();
        let tunic = if i % 2 == 0 { rgb(0.24, 0.30, 0.16) } else { md::crimson() };
        md::archer_figure(&mut a, tunic, rgb(0.26, 0.20, 0.13));
        t.append(a, Transform::from_translation(p + Vec3::Y * (ty + 0.9)).with_rotation(Quat::from_rotation_y(ang)).with_scale(Vec3::splat(1.35)));
    }
    let sway = rb.model(Model::new());
    let tower = rb.part(PartKind::Sway, Some(root), Vec3::Y * deck, sway, 0.0, 0);
    // Cauldrons of burning oil at the front corners.
    for z in [-4.6f32, 4.6] {
        brazier(&mut rb, &mut t, tower, Vec3::new(4.8, ty + 0.9, z), 1.3, 0);
    }
    flag(&mut rb, &mut t, tower, Vec3::new(-5.9, ty + 0.9, 5.9), 9.0, 7.0, md::crimson(), 0);
    flag(&mut rb, &mut t, tower, Vec3::new(-5.9, ty + 0.9, -5.9), 9.0, 7.0, md::crimson(), 1);
    rb.models[sway] = t;

    // Ramp: stands closed against the doorway, drops forward onto the ground.
    let mut ramp = Model::new();
    ramp.boxy(Mat::Wood, Vec3::new(0.5, 8.0, 6.6), Vec3::new(0.25, 4.0, 0.0), W);
    for y in [1.0f32, 2.8, 4.6, 6.4] {
        ramp.boxy(Mat::Wood, Vec3::new(0.35, 0.45, 6.9), Vec3::new(0.6, y, 0.0), sh(W, 0.8));
    }
    for z in [-2.4f32, 2.4] {
        ramp.boxy(Mat::Iron, Vec3::new(0.12, 7.6, 0.3), Vec3::new(0.82, 4.0, z), W);
    }
    let ramp_id = rb.model(ramp);
    rb.part(PartKind::Bridge, Some(root), Vec3::new(7.55, deck, 0.0), ramp_id, -(FRAC_PI_2 + 0.39), 0);

    let w = rb.model(spoked(2.0, 0.9));
    rb.wheels(w, &[-6.3, -2.1, 2.1, 6.3], 2.0, 8.5, 2.0, false);
    rb
}

fn timber_dark() -> Color {
    rgb(0.55, 0.45, 0.38)
}

// ---------------------------------------------------------------------------
// 2. War Wagon: an armored battle wagon with a bombard turret
// ---------------------------------------------------------------------------

pub const WAGON_DECK: f32 = 3.6;
pub const WAGON_PORT_X: [f32; 3] = [-6.0, 0.0, 6.0];
pub const WAGON_PORT_Y: f32 = 7.2;
pub const WAGON_HALF_WIDTH: f32 = 5.0;

pub fn war_wagon() -> RigBuild {
    let mut rb = RigBuild::default();
    let deck = WAGON_DECK;
    let mut m = Model::new();
    m.boxy(Mat::Wood, Vec3::new(25.0, 1.4, 10.4), Vec3::new(0.0, deck - 0.7, 0.0), W);
    for x in [-8.5f32, -3.0, 3.0, 8.5] {
        m.frustum_rot(Mat::Iron, Vec3::new(x, 2.7, -6.4), Quat::from_rotation_x(FRAC_PI_2), 0.4, 0.4, 12.8, 8, W, true, false);
    }
    // Armored hull: straight lower walls, sloped upper armor.
    m.boxy(Mat::Iron, Vec3::new(24.0, 5.0, 10.0), Vec3::new(0.0, deck + 2.5, 0.0), W);
    sloped(&mut m, Mat::Iron, sh(W, 0.92), Vec3::new(0.0, deck + 5.0, 0.0), 23.6, 10.0, 18.0, 6.6, 3.0);
    let top = deck + 8.0;
    for i in 0..10 {
        let x = -10.8 + i as f32 * 2.4;
        for s in [-1.0f32, 1.0] {
            m.boxy(Mat::Iron, Vec3::new(0.32, 5.0, 0.14), Vec3::new(x, deck + 2.5, s * 5.04), sh(W, 0.75));
            rivets(&mut m, Vec3::new(x, deck + 0.4, s * 5.13), Vec3::new(x, deck + 4.8, s * 5.13), 6, 0.14);
        }
    }
    rivets(&mut m, Vec3::new(-11.8, deck + 5.0, 5.08), Vec3::new(11.8, deck + 5.0, 5.08), 30, 0.16);
    rivets(&mut m, Vec3::new(-11.8, deck + 5.0, -5.08), Vec3::new(11.8, deck + 5.0, -5.08), 30, 0.16);
    // Painted pavises along the lower hull, the war wagon's signature.
    for i in 0..8 {
        let x = -10.5 + i as f32 * 3.0;
        for s in [-1.0f32, 1.0] {
            let c = if i % 2 == 0 { md::crimson() } else { rgb(0.80, 0.74, 0.60) };
            m.boxy(Mat::Paint, Vec3::new(2.5, 2.6, 0.2), Vec3::new(x, deck + 1.6, s * 5.15), c);
            m.boxy(Mat::Paint, Vec3::new(0.5, 2.4, 0.22), Vec3::new(x, deck + 1.6, s * 5.17), if i % 2 == 0 { gold() } else { md::crimson() });
        }
    }
    // Gun ports with bronze muzzles; the lids are separate parts.
    for x in WAGON_PORT_X {
        for s in [-1.0f32, 1.0] {
            m.boxy(Mat::Paint, Vec3::new(2.0, 1.6, 0.1), Vec3::new(x, WAGON_PORT_Y, s * 5.06), black());
            m.frustum_rot(Mat::Bronze, Vec3::new(x, WAGON_PORT_Y, s * 4.4), Quat::from_rotation_x(s * FRAC_PI_2), 0.5, 0.44, 0.6, 12, W, true, true);
        }
    }
    // Spiked iron prow.
    for s in [-1.0f32, 1.0] {
        m.cube(Mat::Iron, Vec3::new(5.8, 5.0, 0.45), Vec3::new(13.4, deck + 1.6, s * 2.5), Quat::from_rotation_y(s * 1.06), sh(W, 0.85));
    }
    for (y, z) in [(deck + 0.6, 0.0), (deck + 2.6, 0.0), (deck + 1.6, 1.6), (deck + 1.6, -1.6)] {
        m.frustum_rot(Mat::Iron, Vec3::new(14.6, y, z), Quat::from_rotation_z(-FRAC_PI_2), 0.35, 0.0, 2.2, 6, W, false, false);
    }
    // Rear door, ladder and smokestack.
    m.boxy(Mat::Wood, Vec3::new(0.2, 3.6, 2.6), Vec3::new(-12.05, deck + 2.0, 0.0), md::timber());
    for y in 0..5 {
        m.boxy(Mat::Wood, Vec3::new(0.2, 0.14, 1.4), Vec3::new(-12.2, deck + 0.6 + y as f32 * 0.9, -3.4), W);
    }
    m.frustum(Mat::Iron, Vec3::new(-8.8, top - 1.0, 0.0), 0.7, 0.62, 6.0, 12, sh(W, 0.6), true);
    m.frustum(Mat::Iron, Vec3::new(-8.8, top + 5.0, 0.0), 0.62, 1.1, 0.9, 12, sh(W, 0.6), true);
    let root = rb.body(Model::new());
    flag(&mut rb, &mut m, root, Vec3::new(-11.0, top - 1.4, 3.6), 7.0, 5.0, md::crimson(), 0);
    flag(&mut rb, &mut m, root, Vec3::new(-11.0, top - 1.4, -3.6), 7.0, 5.0, md::crimson(), 1);
    rb.models[rb.parts[root].model] = m;

    // Port lids, hinged at their top edge.
    let mut lid = Model::new();
    lid.boxy(Mat::Iron, Vec3::new(2.3, 1.9, 0.18), Vec3::new(0.0, -0.95, 0.0), sh(W, 0.8));
    rivets(&mut lid, Vec3::new(-1.0, -1.6, 0.12), Vec3::new(1.0, -1.6, 0.12), 4, 0.12);
    let lid = rb.model(lid);
    for x in WAGON_PORT_X {
        for s in [-1.0f32, 1.0] {
            rb.part(PartKind::Port, Some(root), Vec3::new(x, WAGON_PORT_Y + 0.95, s * 5.16), lid, -s * 1.35, 0);
        }
    }

    // Turret with a great bronze bombard.
    let mut tur = Model::new();
    tur.frustum(Mat::Iron, Vec3::ZERO, 3.4, 3.1, 2.8, 22, W, true);
    tur.frustum(Mat::Iron, Vec3::Y * 2.8, 3.25, 3.25, 0.5, 22, sh(W, 0.8), true);
    for k in 0..10 {
        let a = k as f32 / 10.0 * TAU;
        tur.cube(Mat::Iron, Vec3::new(0.5, 0.9, 1.2), Vec3::new(a.cos() * 3.0, 3.75, a.sin() * 3.0), Quat::from_rotation_y(-a), W);
    }
    for k in 0..16 {
        let a = k as f32 / 16.0 * TAU;
        tur.boxy(Mat::Iron, Vec3::splat(0.16), Vec3::new(a.cos() * 3.3, 1.4, a.sin() * 3.3), sh(W, 0.7));
    }
    for z in [-1.4f32, 1.4] {
        tur.boxy(Mat::Paint, Vec3::new(0.12, 0.3, 1.2), Vec3::new(3.2, 2.0, z), black());
    }
    let tur_id = rb.model(Model::new());
    let turret = rb.part(PartKind::Turret, Some(root), Vec3::new(1.5, top, 0.0), tur_id, 10.5, 0);
    flag(&mut rb, &mut tur, turret, Vec3::new(-2.6, 3.3, 0.0), 6.0, 4.0, md::crimson(), 2);
    rb.models[tur_id] = tur;
    let mut gun = Model::new();
    gun.blob(Mat::Bronze, Vec3::ZERO, 1.3, Vec3::new(1.1, 1.0, 1.0), W, 0.0, 3, 2);
    gun.frustum_rot(Mat::Bronze, Vec3::ZERO, Quat::from_rotation_z(-FRAC_PI_2), 1.1, 0.85, 8.8, 18, W, true, true);
    gun.frustum_rot(Mat::Bronze, Vec3::new(8.2, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.15, 1.15, 0.7, 18, W, true, true);
    gun.frustum_rot(Mat::Paint, Vec3::new(8.85, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.7, 0.7, 0.02, 14, black(), true, true);
    for x in [1.6f32, 3.8, 6.0] {
        gun.frustum_rot(Mat::Iron, Vec3::new(x, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.08 - x * 0.025, 1.08 - x * 0.025, 0.4, 18, W, true, false);
    }
    let gun = rb.model(gun);
    rb.part(PartKind::Barrel, Some(turret), Vec3::new(1.4, 1.5, 0.0), gun, 1.6, 0);

    // Scythed wheels.
    let mut wheel = spoked(2.7, 1.0);
    wheel.frustum_rot(Mat::Iron, Vec3::ZERO, Quat::from_rotation_x(FRAC_PI_2), 0.3, 0.18, 1.9, 8, W, true, true);
    for a in [0.0f32, PI] {
        let tip = Vec3::new(0.0, 0.0, 1.85);
        let rot = Quat::from_rotation_z(a);
        wheel.cube(Mat::Iron, Vec3::new(2.2, 0.32, 0.08), tip + rot * Vec3::new(1.1, 0.0, 0.0), rot, sh(W, 1.2));
        wheel.cube(Mat::Iron, Vec3::new(0.9, 0.28, 0.08), tip + rot * Vec3::new(2.1, 0.35, 0.0), rot * Quat::from_rotation_z(0.6), sh(W, 1.2));
    }
    let w = rb.model(wheel);
    rb.wheels(w, &[-8.5, -3.0, 3.0, 8.5], 2.7, 5.9, 2.7, true);
    rb
}

// ---------------------------------------------------------------------------
// 3. Iron Juggernaut: a furnace-driven iron fortress with a hanging ram
// ---------------------------------------------------------------------------

pub const JUG_DECK: f32 = 4.4;

pub fn juggernaut() -> RigBuild {
    let mut rb = RigBuild::default();
    let deck = JUG_DECK;
    let mut m = Model::new();
    let iron = sh(W, 0.82);
    m.boxy(Mat::Wood, Vec3::new(36.0, 1.6, 14.6), Vec3::new(0.0, deck - 0.8, 0.0), W);
    for x in [-14.5f32, -8.7, -2.9, 2.9, 8.7, 14.5] {
        m.frustum_rot(Mat::Iron, Vec3::new(x, 3.2, -8.8), Quat::from_rotation_x(FRAC_PI_2), 0.5, 0.5, 17.6, 8, W, true, false);
    }
    m.boxy(Mat::Iron, Vec3::new(34.0, 5.0, 15.0), Vec3::new(0.0, deck + 2.5, 0.0), iron);
    let roof_y = deck + 5.0;
    sloped(&mut m, Mat::Iron, iron, Vec3::new(0.0, roof_y, 0.0), 33.0, 15.0, 24.0, 6.0, 6.0);
    // Overlapping armor plates on the long slopes.
    for s in [-1.0f32, 1.0] {
        let n = Vec3::new(0.0, 4.5, s * 6.0);
        let up = Vec3::new(0.0, 6.0, -s * 4.5).normalize();
        plates(&mut m, Vec3::new(-13.0, roof_y + 0.9, s * 6.8), Vec3::X, up, n, 9, 3, Vec2::new(3.0, 2.2));
    }
    rivets(&mut m, Vec3::new(-16.8, roof_y - 0.2, 7.58), Vec3::new(16.8, roof_y - 0.2, 7.58), 36, 0.2);
    rivets(&mut m, Vec3::new(-16.8, roof_y - 0.2, -7.58), Vec3::new(16.8, roof_y - 0.2, -7.58), 36, 0.2);
    for i in 0..9 {
        let x = -10.0 + i as f32 * 2.5;
        m.frustum(Mat::Iron, Vec3::new(x, roof_y + 6.0, 0.0), 0.55, 0.0, 2.0, 6, W, false);
    }
    // Furnace mouth with grille and angry glowing eyes.
    m.boxy(Mat::Paint, Vec3::new(0.2, 3.4, 8.0), Vec3::new(17.02, deck + 1.9, 0.0), black());
    for i in 0..9 {
        let z = -3.6 + i as f32 * 0.9;
        m.boxy(Mat::Iron, Vec3::new(0.3, 3.6, 0.22), Vec3::new(17.2, deck + 1.9, z), sh(W, 0.6));
    }
    for s in [-1.0f32, 1.0] {
        m.cube(Mat::Ember, Vec3::new(0.2, 0.55, 2.6), Vec3::new(17.08, deck + 4.2, s * 2.6), Quat::from_rotation_x(s * 0.32), W);
    }
    // Spiked plow.
    for s in [-1.0f32, 1.0] {
        m.cube(Mat::Iron, Vec3::new(8.6, 3.0, 0.5), Vec3::new(19.0, 1.6, s * 3.6), Quat::from_rotation_y(s * 0.96), iron);
    }
    for (x, z) in [(21.2f32, 0.0f32), (19.8, 2.0), (19.8, -2.0), (18.4, 4.0), (18.4, -4.0)] {
        m.frustum_rot(Mat::Iron, Vec3::new(x, 1.6, z), Quat::from_rotation_z(-FRAC_PI_2), 0.4, 0.0, 2.0, 6, W, false, false);
    }
    // Two great smokestacks.
    for z in [-3.4f32, 3.4] {
        m.frustum(Mat::Iron, Vec3::new(-11.0, roof_y + 2.0, z), 1.15, 0.95, 12.0, 14, sh(W, 0.55), true);
        m.frustum(Mat::Iron, Vec3::new(-11.0, roof_y + 14.0, z), 0.95, 1.6, 1.4, 14, sh(W, 0.5), true);
        for y in [5.0f32, 9.0] {
            m.frustum(Mat::Iron, Vec3::new(-11.0, roof_y + y, z), 1.12, 1.12, 0.3, 14, sh(W, 0.75), true);
        }
    }
    // Frame that carries the ram.
    for z in [-4.4f32, 4.4] {
        beam(&mut m, Mat::Iron, Vec3::new(14.0, roof_y + 3.4, z), Vec3::new(25.0, deck + 9.2, z), 0.9, 0.9, iron);
        beam(&mut m, Mat::Iron, Vec3::new(17.0, deck + 4.8, z), Vec3::new(17.0, deck + 9.8, z), 0.8, 0.8, iron);
    }
    for x in [17.5f32, 23.5] {
        m.boxy(Mat::Iron, Vec3::new(0.8, 0.8, 9.6), Vec3::new(x, deck + 9.2 + (24.0 - x) * 0.05, 0.0), iron);
    }
    // Hanging crimson banners.
    for s in [-1.0f32, 1.0] {
        for x in [-6.0f32, 4.0] {
            let pts = [
                Vec3::new(x - 2.0, roof_y - 0.3, s * 7.62),
                Vec3::new(x + 2.0, roof_y - 0.3, s * 7.62),
                Vec3::new(x + 2.0, deck + 0.6, s * 7.62),
                Vec3::new(x, deck - 0.4, s * 7.62),
                Vec3::new(x - 2.0, deck + 0.6, s * 7.62),
            ];
            m.poly(Mat::Cloth, md::crimson(), &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::new(0.5, 1.2), Vec2::Y], Vec3::Z * s);
        }
    }
    let root = rb.body(m);

    // Fire behind the grille.
    let mut fire = Model::new();
    for (i, z) in [-2.6f32, 0.0, 2.6].into_iter().enumerate() {
        fire.blob(Mat::Ember, Vec3::new(0.0, 0.0, z), 1.3, Vec3::new(0.6, 1.2, 1.0), W, 0.2, 60 + i as u32, 1);
    }
    let fire = rb.model(fire);
    rb.part(PartKind::Glow, Some(root), Vec3::new(16.4, deck + 1.6, 0.0), fire, 0.0, 0);

    // Ram chains and the ram itself.
    let chain_len = 5.0;
    let mut chain = Model::new();
    for z in [-1.3f32, 1.3] {
        for i in 0..6 {
            let y = -0.5 - i as f32 * 0.85;
            chain.cube(Mat::Iron, Vec3::new(0.18, 0.9, 0.45), Vec3::new(0.0, y, z), Quat::from_rotation_y(if i % 2 == 0 { 0.0 } else { FRAC_PI_2 }), sh(W, 0.6));
        }
    }
    let chain = rb.model(chain);
    for x in [17.5f32, 23.5] {
        rb.part(PartKind::RamChain, Some(root), Vec3::new(x, deck + 9.2 + (24.0 - x) * 0.05, 0.0), chain, chain_len, 0);
    }
    let mut ram = Model::new();
    ram.frustum_rot(Mat::Wood, Vec3::new(-10.0, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.0, 0.95, 16.5, 14, W, true, true);
    for x in [-8.0f32, -3.0, 0.0, 3.0] {
        ram.frustum_rot(Mat::Iron, Vec3::new(x, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.06, 1.06, 0.5, 14, W, true, false);
    }
    // Bronze ram's head with curled horns.
    ram.blob(Mat::Bronze, Vec3::new(7.4, 0.0, 0.0), 1.6, Vec3::new(1.5, 1.05, 1.0), W, 0.05, 12, 2);
    ram.blob(Mat::Bronze, Vec3::new(9.3, -0.3, 0.0), 0.9, Vec3::new(1.2, 0.9, 0.9), W, 0.05, 13, 2);
    for s in [-1.0f32, 1.0] {
        let mut p = Vec3::new(7.6, 1.0, s * 1.0);
        let mut r = 0.62;
        for i in 0..9 {
            let a = i as f32 * 0.72;
            let dir = Vec3::new(-a.cos() * 0.8, a.sin() * 0.8 - 0.3, s * 0.5).normalize();
            ram.frustum_rot(Mat::Bronze, p, Quat::from_rotation_arc(Vec3::Y, dir), r, r * 0.88, 0.9, 10, sh(W, 0.9), true, false);
            p += dir * 0.85;
            r *= 0.88;
        }
        ram.cube(Mat::Ember, Vec3::new(0.3, 0.3, 0.12), Vec3::new(8.8, 0.35, s * 0.85), Quat::IDENTITY, W);
    }
    let ram = rb.model(ram);
    rb.part(PartKind::RamBeam, Some(root), Vec3::new(20.5, deck + 9.35 - chain_len, 0.0), ram, chain_len, 0);

    let w = rb.model(solid(3.2, 1.6));
    rb.wheels(w, &[-14.5, -8.7, -2.9, 2.9, 8.7, 14.5], 3.2, 8.3, 3.2, false);
    rb
}

// ---------------------------------------------------------------------------
// 4. Trebuchet Fortress: walls, ballista towers and a colossal trebuchet
// ---------------------------------------------------------------------------

pub const TREB_DECK: f32 = 4.0;
pub const TREB_AXLE: Vec3 = Vec3::new(0.0, 26.0, 0.0);
pub const TREB_LONG: f32 = 26.0;
pub const TREB_SHORT: f32 = 7.0;
pub const TREB_SLING: f32 = 7.0;
/// Corner towers (x, z) where the ballistas stand.
pub const TREB_TOWERS: [(f32, f32); 4] = [(15.0, 12.3), (15.0, -12.3), (-15.0, 12.3), (-15.0, -12.3)];
pub const TREB_TOWER_TOP: f32 = 17.0 + 1.0;

pub fn trebuchet() -> RigBuild {
    let mut rb = RigBuild::default();
    let deck = TREB_DECK;
    let stone = rgb(0.86, 0.82, 0.76);
    let mut m = Model::new();
    m.boxy(Mat::Wood, Vec3::new(34.0, 1.8, 28.0), Vec3::new(0.0, deck - 0.9, 0.0), W);
    for i in 0..12 {
        let x = -15.6 + i as f32 * 2.84;
        m.boxy(Mat::Iron, Vec3::new(0.3, 1.86, 28.06), Vec3::new(x, deck - 0.9, 0.0), W);
    }
    for x in [-12.0f32, -4.0, 4.0, 12.0] {
        m.frustum_rot(Mat::Iron, Vec3::new(x, 3.0, -15.0), Quat::from_rotation_x(FRAC_PI_2), 0.45, 0.45, 30.0, 8, W, true, false);
    }
    m.boxy(Mat::Stone, Vec3::new(31.0, 0.2, 25.0), Vec3::new(0.0, deck + 0.1, 0.0), sh(stone, 0.8));
    let (wy, wh) = (deck, 7.0);
    great_wall(&mut m, Vec2::new(16.3, -11.0), Vec2::new(16.3, 11.0), wy, wh, 1.4, stone, 1.1, Vec3::X);
    great_wall(&mut m, Vec2::new(-16.3, -11.0), Vec2::new(-16.3, 11.0), wy, wh, 1.4, stone, 1.1, -Vec3::X);
    great_wall(&mut m, Vec2::new(-13.5, 13.3), Vec2::new(13.5, 13.3), wy, wh, 1.4, stone, 1.1, Vec3::Z);
    great_wall(&mut m, Vec2::new(-13.5, -13.3), Vec2::new(13.5, -13.3), wy, wh, 1.4, stone, 1.1, -Vec3::Z);
    m.boxy(Mat::Wood, Vec3::new(0.3, 4.5, 4.0), Vec3::new(-17.05, deck + 2.25, 0.0), md::timber());
    for (x, z) in TREB_TOWERS {
        great_tower(&mut m, Vec3::new(x, deck, z), 3.4, 13.0, stone, None);
    }
    // Boulders heaped inside the front wall.
    for i in 0..9 {
        let p = Vec3::new(12.0 - (i / 3) as f32 * 1.6, deck + 0.9 + (i % 2) as f32 * 0.5, -3.0 + (i % 3) as f32 * 2.0);
        m.blob(Mat::Stone, p, 1.0, Vec3::ONE, rgb(0.72, 0.70, 0.66), 0.2, 300 + i, 1);
    }
    // Trebuchet frame: two A-frames, braces, axle bearings.
    let ax = TREB_AXLE;
    for z in [-4.2f32, 4.2] {
        let apex = Vec3::new(0.0, ax.y, z);
        beam(&mut m, Mat::Wood, Vec3::new(-11.0, deck + 0.6, z), Vec3::new(11.0, deck + 0.6, z), 1.3, 1.3, W);
        beam(&mut m, Mat::Wood, Vec3::new(-9.0, deck + 0.8, z), apex, 1.2, 1.2, W);
        beam(&mut m, Mat::Wood, Vec3::new(9.0, deck + 0.8, z), apex, 1.2, 1.2, W);
        beam(&mut m, Mat::Wood, Vec3::new(-5.6, deck + 9.0, z), Vec3::new(5.6, deck + 9.0, z), 0.8, 0.8, W);
        beam(&mut m, Mat::Wood, Vec3::new(-3.0, deck + 15.0, z), Vec3::new(3.0, deck + 15.0, z), 0.7, 0.7, W);
        beam(&mut m, Mat::Wood, Vec3::new(-9.0, deck + 0.8, z), Vec3::new(3.0, deck + 15.0, z), 0.6, 0.6, sh(W, 0.85));
        beam(&mut m, Mat::Wood, Vec3::new(9.0, deck + 0.8, z), Vec3::new(-3.0, deck + 15.0, z), 0.6, 0.6, sh(W, 0.85));
        m.boxy(Mat::Iron, Vec3::new(1.8, 1.6, 1.6), apex, W);
    }
    for x in [-10.5f32, 10.5] {
        beam(&mut m, Mat::Wood, Vec3::new(x, deck + 0.6, -4.2), Vec3::new(x, deck + 0.6, 4.2), 0.7, 0.7, sh(W, 0.85));
    }
    m.frustum_rot(Mat::Iron, Vec3::new(0.0, ax.y, -5.2), Quat::from_rotation_x(FRAC_PI_2), 0.6, 0.6, 10.4, 10, W, true, true);
    // Treadwheel stands.
    for z in [-8.4f32, 8.4] {
        for dz in [-1.5f32, 1.5] {
            beam(&mut m, Mat::Wood, Vec3::new(-7.0, deck, z + dz), Vec3::new(-7.0, deck + 9.6, z + dz), 0.6, 0.6, W);
        }
    }
    let root = rb.body(Model::new());
    flag(&mut rb, &mut m, root, Vec3::new(0.0, ax.y + 0.8, 4.2), 5.0, 6.0, md::crimson(), 0);
    flag(&mut rb, &mut m, root, Vec3::new(0.0, ax.y + 0.8, -4.2), 5.0, 6.0, md::royal(), 1);
    flag(&mut rb, &mut m, root, Vec3::new(-16.3, deck + 7.0, 6.0), 6.0, 5.0, md::crimson(), 2);
    flag(&mut rb, &mut m, root, Vec3::new(-16.3, deck + 7.0, -6.0), 6.0, 5.0, md::crimson(), 3);
    rb.models[rb.parts[root].model] = m;

    // Treadwheels that wind the arm back down.
    let mut tw = Model::new();
    let tr = 5.0;
    for s in [-1.0f32, 1.0] {
        for i in 0..24 {
            let a = i as f32 / 24.0 * TAU;
            let rot = Quat::from_rotation_z(a + FRAC_PI_2);
            let seg = TAU * tr / 24.0 * 1.06;
            tw.cube(Mat::Wood, Vec3::new(seg, 0.4, 0.3), Vec3::new(a.cos() * tr, a.sin() * tr, s * 1.0), rot, W);
        }
        for i in 0..8 {
            let a = i as f32 / 8.0 * TAU;
            tw.frustum_rot(Mat::Wood, Vec3::new(0.0, 0.0, s * 1.0), Quat::from_rotation_z(a - FRAC_PI_2), 0.18, 0.15, tr, 5, W, true, false);
        }
    }
    for i in 0..18 {
        let a = i as f32 / 18.0 * TAU;
        tw.boxy(Mat::Wood, Vec3::new(0.35, 0.18, 2.2), Vec3::new(a.cos() * (tr - 0.15), a.sin() * (tr - 0.15), 0.0), sh(W, 0.85));
    }
    tw.frustum_rot(Mat::Iron, Vec3::new(0.0, 0.0, -1.6), Quat::from_rotation_x(FRAC_PI_2), 0.35, 0.35, 3.2, 8, W, true, true);
    let tw = rb.model(tw);
    for z in [-8.4f32, 8.4] {
        rb.part(PartKind::Treadwheel, Some(root), Vec3::new(-7.0, deck + 5.4, z), tw, tr, 0);
    }

    // Ballistas on the corner towers.
    let mut bal = Model::new();
    bal.frustum(Mat::Wood, Vec3::new(0.0, -0.2, 0.0), 0.5, 0.4, 1.2, 8, W, true);
    bal.boxy(Mat::Wood, Vec3::new(4.0, 0.45, 0.5), Vec3::new(0.6, 1.2, 0.0), W);
    for s in [-1.0f32, 1.0] {
        bal.cube(Mat::Wood, Vec3::new(0.3, 0.3, 2.2), Vec3::new(2.1, 1.3, s * 1.0), Quat::from_rotation_y(s * -0.35), sh(W, 0.85));
        beam(&mut bal, Mat::Paint, Vec3::new(1.5, 1.3, s * 2.0), Vec3::new(-0.6, 1.4, 0.0), 0.05, 0.05, rgb(0.85, 0.80, 0.68));
    }
    bal.frustum_rot(Mat::Wood, Vec3::new(-0.6, 1.55, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.08, 0.08, 3.4, 5, W, true, false);
    bal.frustum_rot(Mat::Iron, Vec3::new(2.8, 1.55, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.16, 0.0, 0.6, 5, W, false, false);
    let bal = rb.model(bal);
    for (i, (x, z)) in TREB_TOWERS.into_iter().enumerate() {
        rb.part(PartKind::Turret, Some(root), Vec3::new(x, TREB_TOWER_TOP, z), bal, 3.0, i as u8 + 1);
    }

    // The throwing arm, its counterweight and its sling.
    let mut arm = Model::new();
    arm.boxy(Mat::Iron, Vec3::new(2.4, 2.4, 2.4), Vec3::ZERO, W);
    arm.cube(Mat::Wood, Vec3::new(TREB_LONG + TREB_SHORT, 1.4, 1.2), Vec3::new((TREB_SHORT - TREB_LONG) / 2.0, 0.0, 0.0), Quat::IDENTITY, W);
    arm.boxy(Mat::Wood, Vec3::new(TREB_SHORT + 2.0, 2.0, 1.5), Vec3::new(TREB_SHORT / 2.0, 0.0, 0.0), sh(W, 0.9));
    let mut x = TREB_SHORT - 0.5;
    while x > -TREB_LONG + 1.0 {
        arm.boxy(Mat::Iron, Vec3::new(0.3, 1.55, 1.35), Vec3::new(x, 0.0, 0.0), W);
        x -= 3.0;
    }
    arm.frustum_rot(Mat::Iron, Vec3::new(-TREB_LONG - 0.6, 0.0, 0.0), Quat::from_rotation_z(FRAC_PI_2), 0.3, 0.1, 1.2, 6, W, true, false);
    let arm = rb.model(arm);
    let arm = rb.part(PartKind::Arm, Some(root), ax, arm, 0.0, 0);

    let mut cw = Model::new();
    for z in [-2.0f32, 2.0] {
        cw.boxy(Mat::Iron, Vec3::new(0.35, 2.6, 0.35), Vec3::new(0.0, -1.2, z), W);
    }
    cw.boxy(Mat::Wood, Vec3::new(5.6, 5.0, 5.6), Vec3::new(0.0, -5.0, 0.0), W);
    for y in [-3.0f32, -5.0, -7.0] {
        cw.boxy(Mat::Iron, Vec3::new(5.7, 0.3, 5.7), Vec3::new(0.0, y, 0.0), W);
    }
    for i in 0..6 {
        cw.blob(Mat::Stone, Vec3::new(-1.5 + (i % 3) as f32 * 1.5, -2.3, -1.2 + (i / 3) as f32 * 2.4), 0.9, Vec3::ONE, rgb(0.7, 0.68, 0.64), 0.2, 400 + i, 1);
    }
    let cw = rb.model(cw);
    rb.part(PartKind::Hanging, Some(arm), Vec3::new(TREB_SHORT, 0.0, 0.0), cw, 0.0, 0);

    let mut sling = Model::new();
    for z in [-0.5f32, 0.5] {
        beam(&mut sling, Mat::Paint, Vec3::new(0.0, 0.0, z * 0.4), Vec3::new(0.0, -TREB_SLING, z * 1.8), 0.1, 0.1, rgb(0.62, 0.52, 0.36));
    }
    sling.blob(Mat::Paint, Vec3::new(0.0, -TREB_SLING - 0.4, 0.0), 1.2, Vec3::new(1.1, 0.5, 1.2), md::leather(), 0.0, 5, 1);
    let sling = rb.model(sling);
    let sling = rb.part(PartKind::Hanging, Some(arm), Vec3::new(-TREB_LONG, 0.0, 0.0), sling, 0.0, 1);
    let mut rock = Model::new();
    rock.blob(Mat::Stone, Vec3::ZERO, 1.6, Vec3::ONE, rgb(0.72, 0.70, 0.66), 0.18, 501, 2);
    let rock = rb.model(rock);
    rb.part(PartKind::Load, Some(sling), Vec3::new(0.0, -TREB_SLING, 0.0), rock, 0.0, 0);
    rb
}

// ---------------------------------------------------------------------------
// 5. The Warlord's Citadel: a moving fortress city
// ---------------------------------------------------------------------------

pub const WARLORD_DECK: f32 = 5.5;
/// Corner towers (x, z) with cannon turrets on top.
pub const WARLORD_TOWERS: [(f32, f32); 4] = [(27.5, 19.0), (27.5, -19.0), (-27.5, 19.0), (-27.5, -19.0)];
pub const WARLORD_TOWER_TOP: f32 = WARLORD_DECK + 26.0 + 1.0;
pub const WARLORD_GATE_X: f32 = 33.2;
pub const WARLORD_BOMBARD: Vec3 = Vec3::new(8.0, 28.6, 0.0);

pub fn warlord() -> RigBuild {
    let mut rb = RigBuild::default();
    let deck = WARLORD_DECK;
    let dark = rgb(0.46, 0.43, 0.45);
    let darker = rgb(0.34, 0.31, 0.33);
    let roof_c = rgb(0.52, 0.36, 0.38);
    let mut m = Model::new();
    m.boxy(Mat::Wood, Vec3::new(62.0, 2.6, 44.0), Vec3::new(0.0, deck - 1.3, 0.0), W);
    m.boxy(Mat::Iron, Vec3::new(62.3, 1.0, 44.3), Vec3::new(0.0, deck - 2.1, 0.0), sh(W, 0.7));
    for x in [-25.0f32, -15.0, -5.0, 5.0, 15.0, 25.0] {
        m.frustum_rot(Mat::Iron, Vec3::new(x, 4.2, -24.0), Quat::from_rotation_x(FRAC_PI_2), 0.7, 0.7, 48.0, 10, W, true, false);
    }
    for i in 0..9 {
        let z = -16.0 + i as f32 * 4.0;
        m.frustum_rot(Mat::Iron, Vec3::new(31.0, deck - 1.4, z), Quat::from_rotation_z(-FRAC_PI_2), 0.6, 0.0, 3.0, 6, W, false, false);
    }
    m.boxy(Mat::Stone, Vec3::new(58.0, 0.2, 40.0), Vec3::new(0.0, deck + 0.1, 0.0), sh(dark, 0.7));

    // Curtain walls; the front one is split by the gatehouse.
    let (wy, wh, wt) = (deck, 14.0, 2.4);
    great_wall(&mut m, Vec2::new(29.8, 11.5), Vec2::new(29.8, 15.0), wy, wh, wt, dark, 1.8, Vec3::X);
    great_wall(&mut m, Vec2::new(29.8, -15.0), Vec2::new(29.8, -11.5), wy, wh, wt, dark, 1.8, Vec3::X);
    great_wall(&mut m, Vec2::new(-29.8, -15.0), Vec2::new(-29.8, 15.0), wy, wh, wt, dark, 1.8, -Vec3::X);
    great_wall(&mut m, Vec2::new(-22.0, 21.0), Vec2::new(22.0, 21.0), wy, wh, wt, dark, 1.8, Vec3::Z);
    great_wall(&mut m, Vec2::new(-22.0, -21.0), Vec2::new(22.0, -21.0), wy, wh, wt, dark, 1.8, -Vec3::Z);
    for (x, z) in WARLORD_TOWERS {
        great_tower(&mut m, Vec3::new(x, deck, z), 6.0, 26.0, dark, None);
    }
    // Gatehouse with the Warlord standing above the gate.
    for z in [-7.5f32, 7.5] {
        great_square_tower(&mut m, Vec3::new(29.0, deck, z), 8.0, 22.0, darker);
    }
    m.boxy(Mat::Stone, Vec3::new(7.0, 5.0, 7.2), Vec3::new(29.5, deck + 13.5, 0.0), darker);
    crenels(&mut m, Mat::Stone, darker, Vec3::new(33.0, deck + 16.0, -3.6), Vec3::new(33.0, deck + 16.0, 3.6), 0.6, 1.2, 1.8);
    m.boxy(Mat::Paint, Vec3::new(0.3, 11.0, 7.2), Vec3::new(32.85, deck + 5.5, 0.0), black());
    let mut lord = Model::new();
    md::soldier(&mut lord, md::crimson(), rgb(0.12, 0.12, 0.13));
    lord.blob(Mat::Paint, Vec3::new(-0.02, 1.62, 0.0), 0.12, Vec3::new(1.6, 0.5, 1.6), gold(), 0.0, 9, 1);
    lord.poly(Mat::Paint, md::crimson(), &[Vec3::new(-0.25, 1.15, -0.32), Vec3::new(-0.25, 1.15, 0.32), Vec3::new(-0.5, 0.1, 0.45), Vec3::new(-0.5, 0.1, -0.45)], &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], -Vec3::X);
    lord.cube(Mat::Paint, Vec3::new(0.08, 1.6, 0.16), Vec3::new(0.35, 1.3, -0.35), Quat::from_rotation_z(0.15), md::steel());
    m.append(lord, Transform::from_translation(Vec3::new(30.0, deck + 16.0, 0.0)).with_scale(Vec3::splat(3.2)));

    // The great keep.
    let (kx, kw, kd, kh) = (-4.0, 24.0, 20.0, 34.0);
    m.boxy(Mat::Stone, Vec3::new(kw + 1.2, 2.0, kd + 1.2), Vec3::new(kx, deck + 1.0, 0.0), sh(darker, 0.9));
    m.boxy(Mat::Stone, Vec3::new(kw, kh, kd), Vec3::new(kx, deck + kh / 2.0, 0.0), darker);
    for i in 0..7 {
        let x = kx - kw / 2.0 + i as f32 * kw / 6.0;
        for s in [-1.0f32, 1.0] {
            m.boxy(Mat::Stone, Vec3::new(1.4, kh * 0.8, 1.6), Vec3::new(x, deck + kh * 0.4, s * (kd / 2.0 + 0.6)), sh(darker, 0.92));
            if i < 6 {
                for y in [deck + 12.0, deck + 24.0] {
                    ember_window(&mut m, Vec3::new(x + kw / 12.0, y, s * kd / 2.0), Vec3::Z * s, 1.2, 4.2, dark);
                }
            }
        }
    }
    for z in [-6.5f32, -2.2, 2.2, 6.5] {
        if z.abs() > 3.0 {
            ember_window(&mut m, Vec3::new(kx + kw / 2.0, deck + 14.0, z), Vec3::X, 1.2, 4.2, dark);
        }
        ember_window(&mut m, Vec3::new(kx - kw / 2.0, deck + 14.0, z), -Vec3::X, 1.2, 4.2, dark);
    }
    m.boxy(Mat::Stone, Vec3::new(kw + 1.0, 0.8, kd + 1.0), Vec3::new(kx, deck + kh - 0.4, 0.0), sh(darker, 0.82));
    m.roof(Vec3::new(kx, deck + kh, 0.0), Quat::IDENTITY, kw, kd, 11.0, 0.8, roof_c, Mat::Stone, darker, Mat::Slate);
    for (x, z) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let p = Vec3::new(kx + x * (kw / 2.0 - 0.6), deck + kh - 2.0, z * (kd / 2.0 - 0.6));
        m.boxy(Mat::Stone, Vec3::new(2.6, 6.0, 2.6), p + Vec3::Y * 3.0, darker);
        m.frustum(Mat::Slate, p + Vec3::Y * 6.0, 2.0, 0.0, 6.0, 4, roof_c, false);
    }
    // Giant banners down the keep's flanks.
    for s in [-1.0f32, 1.0] {
        for x in [kx - 7.0, kx + 7.0] {
            let z = s * (kd / 2.0 + 1.5);
            let top = deck + kh - 2.0;
            let pts = [Vec3::new(x - 3.0, top, z), Vec3::new(x + 3.0, top, z), Vec3::new(x + 3.0, top - 14.0, z), Vec3::new(x, top - 16.0, z), Vec3::new(x - 3.0, top - 14.0, z)];
            m.poly(Mat::Cloth, md::crimson(), &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::new(0.5, 1.1), Vec2::Y], Vec3::Z * s);
            let pts = [Vec3::new(x - 0.6, top, z + s * 0.03), Vec3::new(x + 0.6, top, z + s * 0.03), Vec3::new(x + 0.6, top - 15.0, z + s * 0.03), Vec3::new(x - 0.6, top - 15.0, z + s * 0.03)];
            m.poly(Mat::Cloth, gold(), &pts, &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y], Vec3::Z * s);
        }
    }
    // Front balcony carrying the great bombard.
    let b = WARLORD_BOMBARD;
    m.boxy(Mat::Stone, Vec3::new(4.0, 1.2, 9.0), Vec3::new(b.x + 0.2, b.y - 2.6, 0.0), dark);
    for z in [-3.0f32, 3.0] {
        m.boxy(Mat::Wood, Vec3::new(5.0, 1.6, 0.8), Vec3::new(b.x + 1.0, b.y - 1.4, z), W);
    }
    // The great spire.
    let sp = Vec3::new(kx, deck + kh, 0.0);
    m.frustum(Mat::Stone, sp, 5.0, 4.5, 14.0, 20, darker, true);
    m.frustum(Mat::Stone, sp + Vec3::Y * 14.0, 4.5, 5.2, 1.0, 20, sh(darker, 0.85), true);
    m.frustum(Mat::Slate, sp + Vec3::Y * 15.0, 5.6, 0.0, 14.0, 10, roof_c, false);
    m.frustum(Mat::Gold, sp + Vec3::Y * 28.6, 0.25, 0.0, 2.4, 6, W, true);
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        slit(&mut m, sp + Vec3::Y * 6.0 + out * 4.85, out, 0.5, 2.4);
    }
    // Barracks in the courtyard.
    for s in [-1.0f32, 1.0] {
        m.boxy(Mat::Wood, Vec3::new(10.0, 4.0, 5.0), Vec3::new(17.0, deck + 2.0, s * 13.5), W);
        m.roof(Vec3::new(17.0, deck + 4.0, s * 13.5), Quat::IDENTITY, 10.0, 5.0, 2.6, 0.4, roof_c, Mat::Wood, W, Mat::Slate);
    }
    let root = rb.body(Model::new());
    for z in [-7.5f32, 7.5] {
        brazier(&mut rb, &mut m, root, Vec3::new(31.0, deck + 22.0, z), 1.6, 0);
    }
    flag(&mut rb, &mut m, root, sp + Vec3::Y * 30.6, 5.0, 12.0, md::crimson(), 0);
    flag(&mut rb, &mut m, root, Vec3::new(26.6, deck + 22.0, 7.5), 7.0, 6.0, md::crimson(), 1);
    flag(&mut rb, &mut m, root, Vec3::new(26.6, deck + 22.0, -7.5), 7.0, 6.0, md::crimson(), 2);
    rb.models[rb.parts[root].model] = m;

    // A ring of fire around the spire: only lit in the last phase.
    let mut ring = Model::new();
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        ring.blob(Mat::Ember, Vec3::new(a.cos() * 5.4, 0.0, a.sin() * 5.4), 1.1, Vec3::new(1.0, 2.0, 1.0), W, 0.3, 700 + k, 1);
    }
    let ring = rb.model(ring);
    rb.part(PartKind::Glow, Some(root), sp + Vec3::Y * 15.6, ring, 0.0, 1);

    // Portcullis.
    let mut pc = Model::new();
    for i in 0..8 {
        let z = -3.15 + i as f32 * 0.9;
        pc.boxy(Mat::Iron, Vec3::new(0.3, 11.0, 0.3), Vec3::new(0.0, 5.5, z), sh(W, 0.6));
        pc.frustum_rot(Mat::Iron, Vec3::new(0.0, 0.0, z), Quat::from_rotation_x(PI), 0.18, 0.0, 0.8, 4, W, false, false);
    }
    for i in 0..7 {
        pc.boxy(Mat::Iron, Vec3::new(0.3, 0.3, 7.0), Vec3::new(0.0, 0.8 + i as f32 * 1.6, 0.0), sh(W, 0.6));
    }
    let pc = rb.model(pc);
    rb.part(PartKind::Portcullis, Some(root), Vec3::new(WARLORD_GATE_X, deck, 0.0), pc, 10.0, 0);

    // Corner cannon turrets.
    let mut cup = Model::new();
    cup.frustum(Mat::Stone, Vec3::ZERO, 4.2, 4.0, 3.4, 20, darker, true);
    cup.frustum(Mat::Slate, Vec3::Y * 3.4, 4.8, 0.0, 5.0, 10, roof_c, false);
    cup.boxy(Mat::Paint, Vec3::new(0.3, 2.0, 2.4), Vec3::new(4.05, 1.7, 0.0), black());
    let cup = rb.model(cup);
    let mut gun = Model::new();
    gun.frustum_rot(Mat::Bronze, Vec3::ZERO, Quat::from_rotation_z(-FRAC_PI_2), 0.85, 0.65, 6.4, 14, W, true, true);
    gun.frustum_rot(Mat::Bronze, Vec3::new(5.9, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 0.85, 0.85, 0.5, 14, W, true, true);
    let gun = rb.model(gun);
    for (i, (x, z)) in WARLORD_TOWERS.into_iter().enumerate() {
        let t = rb.part(PartKind::Turret, Some(root), Vec3::new(x, WARLORD_TOWER_TOP, z), cup, 10.0, i as u8 + 1);
        rb.part(PartKind::Barrel, Some(t), Vec3::new(3.0, 1.7, 0.0), gun, 1.3, i as u8 + 1);
    }

    // The great bombard.
    let mut bom = Model::new();
    bom.blob(Mat::Bronze, Vec3::ZERO, 2.6, Vec3::new(1.1, 1.0, 1.0), W, 0.0, 4, 2);
    bom.frustum_rot(Mat::Bronze, Vec3::ZERO, Quat::from_rotation_z(-FRAC_PI_2), 2.4, 1.9, 15.0, 22, W, true, true);
    bom.frustum_rot(Mat::Bronze, Vec3::new(14.0, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 2.5, 2.5, 1.2, 22, W, true, true);
    bom.frustum_rot(Mat::Paint, Vec3::new(15.25, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 1.5, 1.5, 0.02, 16, black(), true, true);
    for x in [2.5f32, 6.0, 9.5, 12.5] {
        bom.frustum_rot(Mat::Iron, Vec3::new(x, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), 2.38 - x * 0.03, 2.38 - x * 0.03, 0.6, 22, W, true, false);
    }
    let bom = rb.model(bom);
    rb.part(PartKind::Bombard, Some(root), b, bom, 2.2, 0);

    let mut wheel = spoked(4.2, 2.0);
    for i in 0..10 {
        let a = i as f32 / 10.0 * TAU;
        let p = Vec3::new(a.cos(), a.sin(), 0.0) * 4.3;
        wheel.frustum_rot(Mat::Iron, p, Quat::from_rotation_arc(Vec3::Y, Vec3::new(a.cos(), a.sin(), 0.0)), 0.3, 0.0, 0.6, 5, W, false, false);
    }
    let w = rb.model(wheel);
    rb.wheels(w, &[-25.0, -15.0, -5.0, 5.0, 15.0, 25.0], 4.2, 23.0, 4.2, false);
    rb
}
