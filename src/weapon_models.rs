//! The castle's weapons, built as rigs: a fixed mount plus moving parts
//! (turrets, barrels, bow arms, strings, the ram log, the mine feeder) that
//! `weapons.rs` animates. Weapons face +X, except the side cannons, which
//! face +Z (out of the right-hand wall). Sizes are meters at castle stage 1.

use crate::art::rgb;
use crate::items::ModuleKind;
use crate::models::{archer_figure, leather, royal, sh, Mat, Model, W};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// What a moving part does when the weapon is animated.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    /// Turns about its local Y to aim, and jolts back on each shot.
    Turret,
    /// Bombard barrel, pivoting on its trunnions (local Z).
    Barrel,
    /// Gun carriage that rolls back along local -Z when it fires.
    Carriage,
    /// Carriage wheel of this radius; spins as the carriage rolls.
    Wheel(f32),
    /// Ballista bow arm on the left (-1) or right (+1).
    Arm(f32),
    /// Ballista slider whose claw holds the string.
    Claw,
    /// Invisible point where the two halves of the bowstring meet.
    Nock,
    /// The bolt lying ready in the ballista's groove.
    Bolt,
    /// Winch roller turning about local Z.
    Windlass,
    /// Rope or string stretched between two anchors.
    Rope(Anchor, Anchor),
    /// Archer that turns toward the target (plus this offset) and leans into each shot.
    Archer(f32),
    /// Ram log hanging level on two chains.
    RamLog,
    /// Chain that hangs the ram from a roof beam.
    RamChain,
    /// Star wheel that lets one mine out at a time.
    Feeder,
    /// A mine sliding down the chute.
    SlidingMine,
    /// Hinged flap at the end of the chute.
    Flap,
}

/// A point fixed to a part (`None` = the mount itself).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Anchor {
    pub part: Option<usize>,
    pub at: Vec3,
}

pub struct RigPart {
    pub role: Role,
    pub parent: Option<usize>,
    pub rest: Transform,
    pub model: Model,
}

/// A weapon: the fixed mount and its moving parts.
pub struct Rig {
    pub base: Model,
    pub parts: Vec<RigPart>,
}

impl Rig {
    fn new() -> Self {
        Self { base: Model::new(), parts: Vec::new() }
    }

    fn add(&mut self, role: Role, parent: Option<usize>, rest: Transform, model: Model) -> usize {
        self.parts.push(RigPart { role, parent, rest, model });
        self.parts.len() - 1
    }

    fn at(&mut self, role: Role, parent: Option<usize>, at: Vec3, model: Model) -> usize {
        self.add(role, parent, Transform::from_translation(at), model)
    }

    /// A rope between two anchors: a unit rod stretched into place each frame.
    fn rope(&mut self, parent: Option<usize>, a: Anchor, b: Anchor, r: f32, color: Color) {
        let mut m = Model::new();
        m.rod(Mat::Paint, Vec3::ZERO, Vec3::Y, r, r, 5, color, false);
        let i = self.add(Role::Rope(a, b), parent, Transform::IDENTITY, m);
        let parents = self.parents();
        let local: Vec<Transform> = self.parts.iter().map(|p| p.rest).collect();
        self.parts[i].rest = rope_pose(&parents, &local, parent, a, b);
    }

    pub fn parents(&self) -> Vec<Option<usize>> {
        self.parts.iter().map(|p| p.parent).collect()
    }

    /// Everything in its rest pose as one model (for the card portraits).
    pub fn combined(self) -> Model {
        let parents = self.parents();
        let local: Vec<Transform> = self.parts.iter().map(|p| p.rest).collect();
        let mut out = self.base;
        for (i, part) in self.parts.into_iter().enumerate() {
            let tf = mount_space(&parents, &local, i);
            // Hidden parts (scaled to nothing) stay out of the picture.
            if tf.scale.min_element() > 0.001 {
                out.append(part.model, tf);
            }
        }
        out
    }
}

/// Transform of part `i` relative to the mount, given every part's local transform.
pub fn mount_space(parents: &[Option<usize>], local: &[Transform], i: usize) -> Transform {
    let mut tf = local[i];
    let mut p = parents[i];
    while let Some(j) = p {
        tf = local[j].mul_transform(tf);
        p = parents[j];
    }
    tf
}

/// Where a rope between `a` and `b` sits, in the space of its `parent`.
pub fn rope_pose(parents: &[Option<usize>], local: &[Transform], parent: Option<usize>, a: Anchor, b: Anchor) -> Transform {
    let world = |an: Anchor| match an.part {
        Some(i) => mount_space(parents, local, i).transform_point(an.at),
        None => an.at,
    };
    let (mut pa, mut pb) = (world(a), world(b));
    if let Some(p) = parent {
        let inv = mount_space(parents, local, p).compute_affine().inverse();
        pa = inv.transform_point3(pa);
        pb = inv.transform_point3(pb);
    }
    let d = pb - pa;
    let len = d.length().max(1e-4);
    Transform { translation: pa, rotation: Quat::from_rotation_arc(Vec3::Y, d / len), scale: Vec3::new(1.0, len, 1.0) }
}

pub fn rig(kind: ModuleKind) -> Rig {
    match kind {
        ModuleKind::Ballista => ballista(),
        ModuleKind::Mortar => bombard(),
        ModuleKind::Broadside => cannon(),
        ModuleKind::ArrowLoft => archer_loft(),
        ModuleKind::Ram => ram(),
        ModuleKind::MineLayer => mine_chute(),
    }
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

fn rope_color() -> Color {
    rgb(0.62, 0.51, 0.35)
}

/// Cast bronze, darkened with age.
fn bronze() -> Color {
    rgb(0.74, 0.58, 0.42)
}

fn dark_wood() -> Color {
    sh(W, 0.72)
}

/// Heavy plank platform on two sleepers, with iron corner plates. Top at `top`.
fn platform(m: &mut Model, half: Vec2, top: f32) {
    for z in [-1.0f32, 1.0] {
        m.bev(Mat::Wood, Vec3::new(half.x * 2.0 + 0.1, top - 0.09, 0.22), Vec3::new(0.0, (top - 0.09) / 2.0, z * (half.y - 0.22)), 0.025, dark_wood());
    }
    m.bev(Mat::Wood, Vec3::new(half.x * 2.0, 0.09, half.y * 2.0), Vec3::new(0.0, top - 0.045, 0.0), 0.02, W);
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let c = Vec3::new(sx * (half.x - 0.13), top + 0.008, sz * (half.y - 0.13));
            m.bev(Mat::Iron, Vec3::new(0.3, 0.02, 0.08), c + Vec3::new(0.0, 0.0, sz * 0.09), 0.006, W);
            m.bev(Mat::Iron, Vec3::new(0.08, 0.02, 0.3), c + Vec3::new(sx * 0.09, 0.0, 0.0), 0.006, W);
            for o in [Vec3::new(-sx * 0.05, 0.0, sz * 0.09), Vec3::new(sx * 0.09, 0.0, -sz * 0.05), Vec3::new(sx * 0.09, 0.0, sz * 0.09)] {
                m.rivet(Mat::Iron, c + o + Vec3::Y * 0.01, Vec3::Y, 0.022, sh(W, 1.3));
            }
        }
    }
}

/// Iron race ring for a turntable, studded with bolts.
fn race_ring(m: &mut Model, y: f32, r: f32) {
    m.lathe(Mat::Iron, Vec3::Y * y, Quat::IDENTITY, &[(0.0, r - 0.1), (0.0, r), (0.05, r), (0.07, r - 0.02), (0.07, r - 0.1)], 32, W);
    for k in 0..12 {
        let a = k as f32 / 12.0 * TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        m.rivet(Mat::Iron, Vec3::Y * (y + 0.03) + d * r, d, 0.018, sh(W, 1.3));
    }
}

/// Coopered keg with iron hoops, standing on `base`.
fn keg(m: &mut Model, base: Vec3, r: f32, h: f32) {
    let prof: Vec<(f32, f32)> = (0..=8).map(|i| {
        let t = i as f32 / 8.0;
        (t * h, r * (0.84 + 0.16 * (t * PI).sin()))
    }).collect();
    let mut p = vec![(0.0, 0.0)];
    p.extend(prof);
    p.push((h, r * 0.8));
    p.push((h - 0.02, 0.0));
    m.lathe(Mat::Wood, base, Quat::IDENTITY, &p, 14, W);
    for t in [0.1f32, 0.3, 0.7, 0.9] {
        let rr = r * (0.84 + 0.16 * (t * PI).sin()) + 0.008;
        m.lathe(Mat::Iron, base + Vec3::Y * (t * h - 0.02), Quat::IDENTITY, &[(0.0, rr), (0.04, rr)], 14, W);
    }
}

/// Pyramid of round shot.
fn shot_pile(m: &mut Model, at: Vec3, r: f32) {
    for (x, z) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        m.blob(Mat::Iron, at + Vec3::new(x * r, r, z * r), r, Vec3::ONE, W, 0.0, 1, 2);
    }
    m.blob(Mat::Iron, at + Vec3::new(0.0, r * 2.4, 0.0), r, Vec3::ONE, W, 0.0, 1, 2);
}

/// Coil of rope lying flat.
fn rope_coil(m: &mut Model, at: Vec3, big: f32, thick: f32) {
    for k in 0..2 {
        let c = at + Vec3::Y * (thick + k as f32 * thick * 1.6);
        let prof: Vec<(f32, f32)> = (0..=10).map(|i| {
            let a = i as f32 / 10.0 * TAU;
            (a.sin() * thick, big - k as f32 * thick * 0.6 + a.cos() * thick)
        }).collect();
        m.lathe(Mat::Paint, c, Quat::IDENTITY, &prof, 16, rope_color());
    }
}

/// Twisted rope skein of a torsion engine, standing on `base`.
fn skein(m: &mut Model, base: Vec3, h: f32, r: f32) {
    m.frustum(Mat::Paint, base, r * 0.86, r * 0.86, h, 12, sh(rope_color(), 0.6), true);
    let strands = 8;
    let steps = 7;
    for s in 0..strands {
        let mut prev: Option<Vec3> = None;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let a = s as f32 / strands as f32 * TAU + t * 2.2;
            let p = base + Vec3::new(a.cos() * r * 0.8, t * h, a.sin() * r * 0.8);
            if let Some(q) = prev {
                m.rod(Mat::Paint, q, p, r * 0.24, r * 0.24, 5, sh(rope_color(), 0.9 + 0.2 * (s % 2) as f32), false);
            }
            prev = Some(p);
        }
    }
}

/// Chain hanging down from the origin, `len` long.
fn chain(m: &mut Model, len: f32) {
    let link = 0.07;
    let n = (len / link).round() as i32;
    for i in 0..n {
        let rot = if i % 2 == 0 { Quat::IDENTITY } else { Quat::from_rotation_y(FRAC_PI_2) };
        let c = Vec3::new(0.0, -(i as f32 + 0.5) * link, 0.0);
        for s in [-1.0f32, 1.0] {
            m.bevel(Mat::Iron, Vec3::new(0.014, link + 0.012, 0.014), c + rot * Vec3::new(s * 0.02, 0.0, 0.0), rot, 0.004, W);
        }
        for s in [-1.0f32, 1.0] {
            m.bevel(Mat::Iron, Vec3::new(0.054, 0.014, 0.014), c + Vec3::Y * s * link * 0.5, rot, 0.004, W);
        }
    }
}

// ---------------------------------------------------------------------------
// Ballista
// ---------------------------------------------------------------------------

/// Ballista geometry shared with the animation (turret space).
pub mod ballista {
    use bevy::prelude::*;
    /// Right arm pivot (center of the right skein); the left one mirrors Z.
    pub const ARM_PIVOT: Vec3 = Vec3::new(0.55, 0.62, 0.37);
    pub const ARM_LEN: f32 = 0.64;
    /// Arm angle (forward of straight out to the side) after a shot and when drawn.
    pub const ARM_FIRED: f32 = 0.30;
    pub const ARM_COCKED: f32 = -0.62;
    /// Claw slider height and its X when the string is drawn back.
    pub const CLAW_Y: f32 = 0.60;
    pub const CLAW_BACK: f32 = -0.62;
    /// Where the string sits on the claw.
    pub const NOCK: Vec3 = Vec3::new(0.10, 0.02, 0.0);
    pub const WINDLASS: Vec3 = Vec3::new(-0.76, 0.49, 0.0);

    /// Arm tip (turret space) for arm angle `a` on `side`.
    pub fn tip(a: f32, side: f32) -> Vec3 {
        Vec3::new(ARM_PIVOT.x + ARM_LEN * a.sin(), ARM_PIVOT.y, side * (ARM_PIVOT.z + ARM_LEN * a.cos()))
    }

    pub fn arm_rot(a: f32, side: f32) -> Quat {
        Quat::from_rotation_y(side * a)
    }

    /// Claw X where it can catch the loose string.
    pub fn claw_front() -> f32 {
        tip(ARM_FIRED, 1.0).x - NOCK.x
    }
}

fn ballista() -> Rig {
    use ballista::*;
    let mut r = Rig::new();
    platform(&mut r.base, Vec2::new(0.85, 0.85), 0.23);
    race_ring(&mut r.base, 0.23, 0.66);
    // Spare bolts on a crate, and a coil of rope.
    let crate_at = Vec3::new(-0.56, 0.23, -0.58);
    r.base.bev(Mat::Wood, Vec3::new(0.34, 0.16, 0.26), crate_at + Vec3::Y * 0.08, 0.015, sh(W, 0.85));
    for k in 0..3 {
        let z = crate_at.z - 0.07 + k as f32 * 0.07;
        let y = crate_at.y + 0.19;
        r.base.rod(Mat::Wood, Vec3::new(crate_at.x - 0.3, y, z), Vec3::new(crate_at.x + 0.22, y, z), 0.02, 0.02, 5, rgb(0.85, 0.75, 0.58), true);
        r.base.rod(Mat::Iron, Vec3::new(crate_at.x + 0.22, y, z), Vec3::new(crate_at.x + 0.34, y, z), 0.035, 0.0, 4, W, false);
    }
    rope_coil(&mut r.base, Vec3::new(-0.6, 0.23, 0.58), 0.12, 0.028);

    // Turret: turn plate, pivot post and yoke.
    let mut t = Model::new();
    t.lathe(Mat::Wood, Vec3::ZERO, Quat::IDENTITY, &[(0.0, 0.0), (0.0, 0.52), (0.05, 0.52), (0.08, 0.49), (0.08, 0.0)], 28, W);
    t.lathe(Mat::Wood, Vec3::Y * 0.08, Quat::IDENTITY, &[(0.0, 0.16), (0.2, 0.13), (0.2, 0.0)], 12, W);
    t.lathe(Mat::Iron, Vec3::Y * 0.08, Quat::IDENTITY, &[(0.0, 0.2), (0.03, 0.2), (0.04, 0.17), (0.04, 0.0)], 14, W);
    t.lathe(Mat::Iron, Vec3::Y * 0.24, Quat::IDENTITY, &[(0.0, 0.16), (0.05, 0.16)], 12, W);
    for s in [-1.0f32, 1.0] {
        t.bev(Mat::Wood, Vec3::new(0.46, 0.22, 0.07), Vec3::new(0.0, 0.36, s * 0.19), 0.015, sh(W, 0.9));
        t.rivet(Mat::Iron, Vec3::new(0.0, 0.38, s * 0.226), Vec3::Z * s, 0.035, sh(W, 1.2));
    }
    // Stock with its bolt groove.
    t.bev(Mat::Wood, Vec3::new(1.7, 0.14, 0.30), Vec3::new(0.0, 0.48, 0.0), 0.025, W);
    for s in [-1.0f32, 1.0] {
        t.bev(Mat::Wood, Vec3::new(1.5, 0.06, 0.06), Vec3::new(-0.05, 0.58, s * 0.11), 0.012, sh(W, 0.85));
        for x in [-0.3f32, 0.15] {
            t.bev(Mat::Iron, Vec3::new(0.07, 0.15, 0.012), Vec3::new(x, 0.48, s * 0.152), 0.004, W);
            t.rivet(Mat::Iron, Vec3::new(x, 0.48, s * 0.158), Vec3::Z * s, 0.016, sh(W, 1.3));
        }
    }
    t.bev(Mat::Iron, Vec3::new(0.08, 0.16, 0.32), Vec3::new(-0.82, 0.48, 0.0), 0.01, W);
    // Torsion frame: two skeins of twisted rope between iron-shod beams.
    let (fx, fy0, fy1) = (ARM_PIVOT.x, 0.405, 0.84);
    for y in [fy0 - 0.06, fy1 + 0.06] {
        t.bev(Mat::Wood, Vec3::new(0.3, 0.12, 1.26), Vec3::new(fx, y, 0.0), 0.02, sh(W, 0.92));
        for s in [-1.0f32, 1.0] {
            t.bev(Mat::Iron, Vec3::new(0.31, 0.13, 0.05), Vec3::new(fx, y, s * 0.61), 0.008, W);
        }
    }
    for s in [-1.0f32, 1.0] {
        for (z, w) in [(0.19f32, 0.07f32), (0.56, 0.08)] {
            t.bev(Mat::Wood, Vec3::new(0.28, fy1 - fy0, w), Vec3::new(fx, (fy0 + fy1) / 2.0, s * z), 0.012, sh(W, 0.95));
        }
        let pz = s * ARM_PIVOT.z;
        skein(&mut t, Vec3::new(fx, fy0, pz), fy1 - fy0, 0.12);
        // Washers with tensioning levers, top and bottom.
        for (y, up) in [(fy1 + 0.12, 1.0f32), (fy0 - 0.12, -1.0)] {
            let base = Vec3::new(fx, y, pz);
            let rot = if up > 0.0 { Quat::IDENTITY } else { Quat::from_rotation_x(PI) };
            t.lathe(Mat::Bronze, base, rot, &[(0.0, 0.15), (0.03, 0.15), (0.045, 0.13), (0.045, 0.06), (0.07, 0.05), (0.07, 0.0)], 18, bronze());
            t.bevel(Mat::Iron, Vec3::new(0.36, 0.035, 0.05), base + Vec3::Y * up * 0.065, Quat::from_rotation_y(s * 0.5), 0.008, W);
        }
    }
    // Windlass brackets at the back.
    for s in [-1.0f32, 1.0] {
        t.bev(Mat::Wood, Vec3::new(0.18, 0.24, 0.05), WINDLASS + Vec3::new(0.0, -0.01, s * 0.18), 0.012, sh(W, 0.9));
    }
    let turret = r.at(Role::Turret, None, Vec3::Y * 0.30, t);

    // Bow arms.
    for s in [-1.0f32, 1.0] {
        let mut a = Model::new();
        let tip = Vec3::new(0.0, 0.0, s * ARM_LEN);
        a.rod(Mat::Wood, Vec3::ZERO, tip, 0.068, 0.046, 10, sh(W, 0.95), false);
        a.rod(Mat::Iron, Vec3::new(0.0, 0.0, s * 0.16), Vec3::new(0.0, 0.0, s * 0.22), 0.072, 0.07, 12, W, false);
        a.rod(Mat::Paint, Vec3::new(0.0, 0.0, s * 0.3), Vec3::new(0.0, 0.0, s * 0.42), 0.062, 0.058, 10, leather(), false);
        a.rod(Mat::Iron, tip - Vec3::Z * s * 0.1, tip + Vec3::Z * s * 0.03, 0.05, 0.036, 10, W, true);
        let pivot = Vec3::new(ARM_PIVOT.x, ARM_PIVOT.y, s * ARM_PIVOT.z);
        r.add(Role::Arm(s), Some(turret), Transform::from_translation(pivot).with_rotation(arm_rot(ARM_COCKED, s)), a);
    }
    let (arm_l, arm_r) = (r.parts.len() - 2, r.parts.len() - 1);

    // Claw slider with its trigger.
    let mut c = Model::new();
    c.bev(Mat::Wood, Vec3::new(0.24, 0.1, 0.15), Vec3::ZERO, 0.015, dark_wood());
    c.bev(Mat::Iron, Vec3::new(0.25, 0.02, 0.155), Vec3::Y * 0.055, 0.005, W);
    c.bev(Mat::Iron, Vec3::new(0.06, 0.09, 0.06), Vec3::new(0.09, 0.09, 0.0), 0.012, W);
    c.bev(Mat::Iron, Vec3::new(0.05, 0.03, 0.06), Vec3::new(0.12, 0.125, 0.0), 0.008, W);
    c.rod(Mat::Iron, Vec3::new(-0.06, 0.06, 0.0), Vec3::new(-0.17, 0.17, 0.0), 0.014, 0.012, 6, W, false);
    c.blob(Mat::Iron, Vec3::new(-0.17, 0.17, 0.0), 0.025, Vec3::ONE, W, 0.0, 1, 1);
    let claw = r.at(Role::Claw, Some(turret), Vec3::new(CLAW_BACK, CLAW_Y, 0.0), c);
    let nock = r.at(Role::Nock, Some(turret), Vec3::new(CLAW_BACK, CLAW_Y, 0.0) + NOCK, Model::new());

    // The bolt: ash shaft, forged head, three vanes.
    let mut b = Model::new();
    b.rod(Mat::Wood, Vec3::ZERO, Vec3::X * 1.16, 0.032, 0.03, 8, rgb(0.92, 0.80, 0.62), true);
    b.lathe(Mat::Iron, Vec3::X * 1.12, Quat::from_rotation_z(-FRAC_PI_2), &[(0.0, 0.04), (0.06, 0.04), (0.07, 0.065), (0.3, 0.0)], 4, sh(W, 1.2));
    for k in 0..3 {
        let rot = Quat::from_rotation_x(k as f32 / 3.0 * TAU + FRAC_PI_2);
        let col = if k == 0 { rgb(0.62, 0.1, 0.08) } else { rgb(0.88, 0.86, 0.8) };
        b.cube(Mat::Paint, Vec3::new(0.26, 0.07, 0.008), rot * Vec3::new(0.17, 0.062, 0.0), rot, col);
    }
    r.at(Role::Bolt, Some(turret), Vec3::new(CLAW_BACK + NOCK.x, CLAW_Y + 0.025, 0.0), b);

    // Windlass roller with handspikes.
    let mut w = Model::new();
    w.rod(Mat::Wood, Vec3::new(0.0, 0.0, -0.25), Vec3::new(0.0, 0.0, 0.25), 0.065, 0.065, 12, W, true);
    w.rod(Mat::Paint, Vec3::new(0.0, 0.0, -0.09), Vec3::new(0.0, 0.0, 0.09), 0.078, 0.078, 12, rope_color(), false);
    for z in [-0.22f32, 0.22] {
        w.rod(Mat::Iron, Vec3::new(0.0, 0.0, z - 0.03), Vec3::new(0.0, 0.0, z + 0.03), 0.075, 0.075, 12, W, false);
        for k in 0..4 {
            let a = k as f32 * FRAC_PI_2 + 0.4;
            w.rod(Mat::Wood, Vec3::new(0.0, 0.0, z), Vec3::new(a.cos() * 0.24, a.sin() * 0.24, z), 0.02, 0.016, 6, sh(W, 0.9), true);
        }
    }
    r.at(Role::Windlass, Some(turret), WINDLASS, w);

    // Bowstring halves, and the winch rope to the claw.
    let string = rgb(0.86, 0.8, 0.66);
    for (arm, s) in [(arm_l, -1.0f32), (arm_r, 1.0)] {
        r.rope(Some(turret), Anchor { part: Some(arm), at: Vec3::new(0.0, 0.0, s * ARM_LEN) }, Anchor { part: Some(nock), at: Vec3::ZERO }, 0.014, string);
    }
    r.rope(
        Some(turret),
        Anchor { part: Some(turret), at: WINDLASS + Vec3::Y * 0.075 },
        Anchor { part: Some(claw), at: Vec3::new(-0.12, 0.0, 0.0) },
        0.012,
        rope_color(),
    );
    r
}

// ---------------------------------------------------------------------------
// Bronze bombard
// ---------------------------------------------------------------------------

/// Trunnion line of the bombard (turret space) and its usual elevation.
pub const BOMBARD_TRUNNION: Vec3 = Vec3::new(-0.04, 0.66, 0.0);
pub const BOMBARD_ELEV: f32 = 0.95;

fn bombard() -> Rig {
    let mut r = Rig::new();
    platform(&mut r.base, Vec2::new(0.85, 0.85), 0.23);
    race_ring(&mut r.base, 0.23, 0.68);
    shot_pile(&mut r.base, Vec3::new(0.6, 0.23, 0.62), 0.1);
    keg(&mut r.base, Vec3::new(-0.62, 0.23, 0.62), 0.14, 0.32);
    // Rammer and sponge lying along the deck.
    r.base.rod(Mat::Wood, Vec3::new(-0.62, 0.27, -0.72), Vec3::new(0.45, 0.27, -0.72), 0.022, 0.022, 6, W, true);
    r.base.rod(Mat::Paint, Vec3::new(0.45, 0.27, -0.72), Vec3::new(0.62, 0.27, -0.72), 0.06, 0.06, 8, rgb(0.25, 0.22, 0.2), true);

    // Turret: turn plate and a heavy timber bed.
    let mut t = Model::new();
    t.lathe(Mat::Wood, Vec3::ZERO, Quat::IDENTITY, &[(0.0, 0.0), (0.0, 0.6), (0.05, 0.6), (0.07, 0.57), (0.07, 0.0)], 28, W);
    for s in [-1.0f32, 1.0] {
        let z = s * 0.5;
        t.bev(Mat::Wood, Vec3::new(1.1, 0.36, 0.16), Vec3::new(0.0, 0.25, z), 0.025, W);
        t.bev(Mat::Wood, Vec3::new(0.5, 0.2, 0.16), Vec3::new(BOMBARD_TRUNNION.x, 0.53, z), 0.025, sh(W, 0.95));
        // Cap square over the trunnion.
        t.bev(Mat::Iron, Vec3::new(0.22, 0.05, 0.18), Vec3::new(BOMBARD_TRUNNION.x, BOMBARD_TRUNNION.y + 0.08, z), 0.01, W);
        for dx in [-0.08f32, 0.08] {
            t.rivet(Mat::Iron, Vec3::new(BOMBARD_TRUNNION.x + dx, BOMBARD_TRUNNION.y + 0.105, z), Vec3::Y, 0.022, sh(W, 1.3));
        }
        // Straps and bolts on the outside of each cheek.
        for x in [-0.42f32, 0.42] {
            t.bev(Mat::Iron, Vec3::new(0.07, 0.37, 0.17), Vec3::new(x, 0.25, z), 0.008, W);
        }
        for x in [-0.25f32, 0.0, 0.25] {
            for y in [0.15f32, 0.35] {
                t.rivet(Mat::Iron, Vec3::new(x, y, z + s * 0.08), Vec3::Z * s, 0.02, sh(W, 1.3));
            }
        }
        // Lifting ring.
        let ring_c = Vec3::new(0.3, 0.22, z + s * 0.1);
        let prof: Vec<(f32, f32)> = (0..=8).map(|i| {
            let a = i as f32 / 8.0 * TAU;
            (a.sin() * 0.012, 0.05 + a.cos() * 0.012)
        }).collect();
        t.lathe(Mat::Iron, ring_c, Quat::from_rotation_x(FRAC_PI_2), &prof, 10, W);
    }
    for x in [-0.42f32, 0.42] {
        t.bev(Mat::Wood, Vec3::new(0.16, 0.24, 0.86), Vec3::new(x, 0.19, 0.0), 0.02, sh(W, 0.85));
    }
    // Elevating wedge under the breech.
    t.bevel(Mat::Wood, Vec3::new(0.34, 0.1, 0.5), Vec3::new(-0.3, 0.2, 0.0), Quat::from_rotation_z(-0.18), 0.015, sh(W, 0.9));
    let turret = r.at(Role::Turret, None, Vec3::Y * 0.23, t);

    // Short, fat bronze barrel: breech, base ring, astragal, muzzle swell, dark bore.
    let mut b = Model::new();
    let prof = [
        (-0.36, 0.0),
        (-0.35, 0.09),
        (-0.31, 0.19),
        (-0.25, 0.27),
        (-0.17, 0.32),
        (-0.08, 0.35),
        (0.0, 0.36),
        (0.0, 0.40),
        (0.07, 0.40),
        (0.07, 0.37),
        (0.42, 0.34),
        (0.42, 0.365),
        (0.47, 0.365),
        (0.47, 0.34),
        (0.70, 0.33),
        (0.76, 0.37),
        (0.86, 0.38),
        (0.88, 0.36),
        (0.88, 0.23),
    ];
    b.lathe(Mat::Bronze, Vec3::ZERO, Quat::IDENTITY, &prof, 28, bronze());
    b.lathe(Mat::Paint, Vec3::ZERO, Quat::IDENTITY, &[(0.88, 0.23), (0.35, 0.23), (0.35, 0.0)], 20, rgb(0.03, 0.025, 0.02));
    b.rod(Mat::Bronze, Vec3::new(0.0, 0.0, -0.58), Vec3::new(0.0, 0.0, 0.58), 0.08, 0.08, 12, bronze(), true);
    // Dolphins (lifting handles) and the touch hole, on top of the barrel (-X).
    for s in [-1.0f32, 1.0] {
        let z = s * 0.11;
        let pts = [Vec3::new(-0.33, 0.18, z), Vec3::new(-0.42, 0.22, z), Vec3::new(-0.42, 0.34, z), Vec3::new(-0.34, 0.38, z)];
        for w in pts.windows(2) {
            b.rod(Mat::Bronze, w[0], w[1], 0.026, 0.026, 6, bronze(), false);
        }
    }
    b.rivet(Mat::Iron, Vec3::new(-0.3, -0.14, 0.0), Vec3::new(-1.0, -0.3, 0.0), 0.03, W);
    let rest = Transform::from_translation(BOMBARD_TRUNNION).with_rotation(Quat::from_rotation_z(-(FRAC_PI_2 - BOMBARD_ELEV)));
    r.add(Role::Barrel, Some(turret), rest, b);
    r
}

// ---------------------------------------------------------------------------
// Side cannons
// ---------------------------------------------------------------------------

/// How far the carriage rolls back when the cannon fires.
pub const CANNON_RECOIL: f32 = 0.3;
/// Muzzle of the cannon (carriage space), for smoke.
pub const CANNON_MUZZLE: Vec3 = Vec3::new(0.0, 0.62, 1.08);

fn cannon() -> Rig {
    let mut r = Rig::new();
    let m = &mut r.base;
    // Gun deck braced out from the wall, with a sill to fire over.
    m.bev(Mat::Wood, Vec3::new(1.36, 0.1, 1.5), Vec3::new(0.0, 0.05, 0.0), 0.02, W);
    for x in [-0.6f32, 0.0, 0.6] {
        m.bev(Mat::Wood, Vec3::new(0.12, 0.14, 1.5), Vec3::new(x, -0.07, 0.0), 0.02, dark_wood());
    }
    // Ledger beam against the wall, with knee braces down to it.
    m.bev(Mat::Wood, Vec3::new(1.36, 0.2, 0.12), Vec3::new(0.0, -0.1, -0.7), 0.02, dark_wood());
    for s in [-1.0f32, 1.0] {
        let x = s * 0.6;
        let (top, foot) = (Vec3::new(x, -0.14, 0.62), Vec3::new(x, -1.0, -0.7));
        let d = top - foot;
        m.bevel(Mat::Wood, Vec3::new(0.1, 0.1, d.length()), (top + foot) / 2.0, Quat::from_rotation_arc(Vec3::Z, d.normalize()), 0.015, dark_wood());
        m.bev(Mat::Wood, Vec3::new(0.12, 0.95, 0.12), Vec3::new(x, -0.6, -0.7), 0.02, dark_wood());
        m.bev(Mat::Iron, Vec3::new(0.13, 0.08, 0.13), Vec3::new(x, -0.98, -0.7), 0.01, W);
        // Posts each side of the gun with ring bolts for the breeching rope.
        m.bev(Mat::Wood, Vec3::new(0.14, 0.62, 0.14), Vec3::new(s * 0.6, 0.41, 0.66), 0.02, sh(W, 0.9));
        m.bev(Mat::Iron, Vec3::new(0.15, 0.06, 0.15), Vec3::new(s * 0.6, 0.6, 0.66), 0.01, W);
        m.rivet(Mat::Iron, Vec3::new(s * 0.53, 0.44, 0.6), Vec3::new(-s, 0.0, 0.0), 0.03, W);
    }
    m.bev(Mat::Wood, Vec3::new(1.36, 0.18, 0.16), Vec3::new(0.0, 0.19, 0.66), 0.025, sh(W, 0.88));
    m.bev(Mat::Iron, Vec3::new(1.37, 0.03, 0.17), Vec3::new(0.0, 0.29, 0.66), 0.006, W);
    // Shot garland and a rammer.
    m.bev(Mat::Wood, Vec3::new(0.14, 0.06, 0.6), Vec3::new(-0.58, 0.13, -0.3), 0.012, sh(W, 0.85));
    for k in 0..3 {
        m.blob(Mat::Iron, Vec3::new(-0.58, 0.24, -0.5 + k as f32 * 0.2), 0.085, Vec3::ONE, W, 0.0, 1, 2);
    }
    m.rod(Mat::Wood, Vec3::new(0.6, 0.14, -0.7), Vec3::new(0.6, 0.14, 0.3), 0.022, 0.022, 6, W, true);
    m.rod(Mat::Paint, Vec3::new(0.6, 0.14, -0.72), Vec3::new(0.6, 0.14, -0.6), 0.055, 0.055, 8, rgb(0.25, 0.22, 0.2), true);

    // Carriage: stepped cheeks on two axletrees.
    let mut c = Model::new();
    for s in [-1.0f32, 1.0] {
        let x = s * 0.28;
        c.bev(Mat::Wood, Vec3::new(0.13, 0.26, 1.0), Vec3::new(x, 0.3, 0.0), 0.025, W);
        c.bev(Mat::Wood, Vec3::new(0.13, 0.14, 0.66), Vec3::new(x, 0.5, 0.16), 0.025, W);
        c.bev(Mat::Wood, Vec3::new(0.13, 0.1, 0.34), Vec3::new(x, 0.62, 0.3), 0.02, W);
        c.bev(Mat::Iron, Vec3::new(0.14, 0.05, 0.2), Vec3::new(x, 0.7, 0.2), 0.01, W);
        for dz in [0.12f32, 0.28] {
            c.rivet(Mat::Iron, Vec3::new(x, 0.725, dz), Vec3::Y, 0.02, sh(W, 1.3));
        }
        for z in [-0.3f32, 0.3] {
            c.rivet(Mat::Iron, Vec3::new(s * 0.345, 0.32, z), Vec3::X * s, 0.026, W);
        }
        c.bev(Mat::Iron, Vec3::new(0.135, 0.27, 0.05), Vec3::new(x, 0.3, 0.47), 0.008, W);
    }
    for z in [-0.36f32, 0.36] {
        c.bev(Mat::Wood, Vec3::new(0.9, 0.12, 0.13), Vec3::new(0.0, 0.17, z), 0.02, sh(W, 0.85));
    }
    c.bev(Mat::Wood, Vec3::new(0.43, 0.14, 0.12), Vec3::new(0.0, 0.36, 0.42), 0.02, sh(W, 0.9));
    // Quoin wedge under the breech.
    c.bevel(Mat::Wood, Vec3::new(0.26, 0.08, 0.34), Vec3::new(0.0, 0.42, -0.3), Quat::from_rotation_x(-0.12), 0.012, sh(W, 0.8));
    c.rod(Mat::Wood, Vec3::new(0.0, 0.42, -0.47), Vec3::new(0.0, 0.42, -0.56), 0.025, 0.025, 6, W, true);
    // Barrel: cascabel, reinforces, chase, muzzle swell (along +Z).
    let k = 1.12;
    let prof: Vec<(f32, f32)> = [
        (-0.68, 0.0),
        (-0.67, 0.045),
        (-0.63, 0.06),
        (-0.59, 0.045),
        (-0.56, 0.035),
        (-0.53, 0.035),
        (-0.5, 0.17),
        (-0.47, 0.24),
        (-0.45, 0.27),
        (-0.45, 0.295),
        (-0.39, 0.295),
        (-0.39, 0.27),
        (-0.05, 0.25),
        (-0.05, 0.27),
        (0.0, 0.27),
        (0.0, 0.24),
        (0.3, 0.225),
        (0.3, 0.24),
        (0.34, 0.24),
        (0.34, 0.205),
        (0.66, 0.185),
        (0.66, 0.2),
        (0.69, 0.2),
        (0.69, 0.18),
        (0.72, 0.175),
        (0.78, 0.215),
        (0.84, 0.22),
        (0.86, 0.2),
        (0.86, 0.105),
    ]
    .iter()
    .map(|(y, rr)| (*y, rr * k))
    .collect();
    let axis = Quat::from_rotation_x(FRAC_PI_2);
    let trunnion = Vec3::new(0.0, CANNON_MUZZLE.y, 0.2);
    c.lathe(Mat::Bronze, trunnion, axis, &prof, 26, bronze());
    c.lathe(Mat::Paint, trunnion, axis, &[(0.86, 0.105 * k), (0.45, 0.105 * k), (0.45, 0.0)], 16, rgb(0.03, 0.025, 0.02));
    c.rod(Mat::Bronze, trunnion - Vec3::X * 0.36, trunnion + Vec3::X * 0.36, 0.065, 0.065, 10, bronze(), true);
    c.rivet(Mat::Iron, trunnion + Vec3::new(0.0, 0.29, -0.38), Vec3::Y, 0.028, W);
    let carriage = r.at(Role::Carriage, None, Vec3::Y * 0.1, c);

    // Solid truck wheels with iron tires.
    for (x, z) in [(-0.43f32, -0.36f32), (0.43, -0.36), (-0.43, 0.36), (0.43, 0.36)] {
        let mut w = Model::new();
        let rot = Quat::from_rotation_z(-FRAC_PI_2);
        w.lathe(Mat::Wood, Vec3::ZERO, rot, &[(-0.045, 0.0), (-0.045, 0.15), (-0.04, 0.165), (0.04, 0.165), (0.045, 0.15), (0.045, 0.0)], 18, sh(W, 0.9));
        w.lathe(Mat::Iron, Vec3::ZERO, rot, &[(-0.035, 0.17), (0.035, 0.17)], 18, W);
        w.rod(Mat::Iron, Vec3::new(-0.06, 0.0, 0.0), Vec3::new(0.07, 0.0, 0.0), 0.04, 0.04, 8, W, true);
        for i in 0..4 {
            let a = i as f32 / 4.0 * TAU;
            let s = x.signum();
            w.rivet(Mat::Iron, Vec3::new(s * 0.045, a.cos() * 0.1, a.sin() * 0.1), Vec3::X * s, 0.018, sh(W, 1.3));
        }
        r.at(Role::Wheel(0.17), Some(carriage), Vec3::new(x, 0.17, z), w);
    }
    // Breeching ropes from the carriage to the ring bolts.
    for s in [-1.0f32, 1.0] {
        r.rope(None, Anchor { part: Some(carriage), at: Vec3::new(s * 0.36, 0.32, 0.3) }, Anchor { part: None, at: Vec3::new(s * 0.53, 0.44, 0.6) }, 0.022, rope_color());
    }
    r
}

// ---------------------------------------------------------------------------
// Archer loft
// ---------------------------------------------------------------------------

fn archer_loft() -> Rig {
    let mut r = Rig::new();
    let m = &mut r.base;
    let (hx, hz) = (0.9f32, 1.15f32);
    for x in [-0.6f32, 0.6] {
        m.bev(Mat::Wood, Vec3::new(0.2, 0.08, 2.3), Vec3::new(x, 0.04, 0.0), 0.02, dark_wood());
    }
    m.bev(Mat::Wood, Vec3::new(hx * 2.0, 0.07, hz * 2.0), Vec3::new(0.0, 0.115, 0.0), 0.015, W);
    let floor = 0.15;
    // Corner posts: tall at the back for the roof, short in front.
    for (x, h) in [(-hx + 0.08, 1.62f32), (hx - 0.08, 0.86)] {
        for s in [-1.0f32, 1.0] {
            m.bev(Mat::Wood, Vec3::new(0.16, h, 0.16), Vec3::new(x, floor + h / 2.0, s * (hz - 0.08)), 0.025, sh(W, 0.9));
            m.bev(Mat::Iron, Vec3::new(0.17, 0.06, 0.17), Vec3::new(x, floor + h - 0.1, s * (hz - 0.08)), 0.008, W);
        }
    }
    // Back wall with arrow slits.
    m.bev(Mat::Wood, Vec3::new(0.08, 1.3, 2.1), Vec3::new(-hx + 0.08, floor + 0.65, 0.0), 0.015, W);
    for z in [-0.6f32, 0.0, 0.6] {
        m.boxy(Mat::Paint, Vec3::new(0.09, 0.42, 0.05), Vec3::new(-hx + 0.08, floor + 0.75, z), rgb(0.03, 0.03, 0.03));
    }
    // Crenellated parapets: tall and short planks in turn.
    let parapet = |m: &mut Model, a: Vec3, b: Vec3, n: usize| {
        for i in 0..n {
            let t = (i as f32 + 0.5) / n as f32;
            let c = a.lerp(b, t);
            let h = if i % 2 == 0 { 0.74 } else { 0.5 };
            let along = (b - a).normalize();
            let w = (b - a).length() / n as f32 - 0.015;
            let size = if along.x.abs() > 0.5 { Vec3::new(w, h, 0.07) } else { Vec3::new(0.07, h, w) };
            m.bev(Mat::Wood, size, Vec3::new(c.x, floor + h / 2.0, c.z), 0.015, if i % 2 == 0 { W } else { sh(W, 0.9) });
        }
    };
    parapet(m, Vec3::new(hx - 0.06, 0.0, -hz + 0.16), Vec3::new(hx - 0.06, 0.0, hz - 0.16), 7);
    for s in [-1.0f32, 1.0] {
        parapet(m, Vec3::new(-hx + 0.16, 0.0, s * (hz - 0.06)), Vec3::new(hx - 0.16, 0.0, s * (hz - 0.06)), 5);
    }
    // Pavises (painted shields) hung outside the front parapet.
    for z in [-0.72f32, 0.0, 0.72] {
        let c = Vec3::new(hx + 0.02, floor + 0.36, z);
        m.bevel(Mat::Paint, Vec3::new(0.05, 0.58, 0.38), c, Quat::from_rotation_z(-0.12), 0.02, royal());
        m.bevel(Mat::Paint, Vec3::new(0.055, 0.5, 0.07), c + Vec3::X * 0.005, Quat::from_rotation_z(-0.12), 0.01, rgb(0.85, 0.7, 0.3));
        m.bevel(Mat::Paint, Vec3::new(0.055, 0.07, 0.3), c + Vec3::new(0.012, 0.08, 0.0), Quat::from_rotation_z(-0.12), 0.01, rgb(0.85, 0.7, 0.3));
    }
    // Pent roof over the back half, on two front posts.
    for s in [-1.0f32, 1.0] {
        m.bev(Mat::Wood, Vec3::new(0.13, 1.26, 0.13), Vec3::new(-0.12, floor + 0.63, s * (hz - 0.08)), 0.02, sh(W, 0.9));
    }
    m.bev(Mat::Wood, Vec3::new(0.14, 0.14, hz * 2.0 + 0.1), Vec3::new(-0.12, floor + 1.3, 0.0), 0.02, dark_wood());
    m.bev(Mat::Wood, Vec3::new(0.14, 0.14, hz * 2.0 + 0.1), Vec3::new(-hx + 0.08, floor + 1.66, 0.0), 0.02, dark_wood());
    let (x0, y0, x1, y1) = (-hx - 0.12, floor + 1.8, 0.02, floor + 1.4);
    let slope = ((y0 - y1) / (x1 - x0)).atan();
    let len = Vec2::new(x1 - x0, y0 - y1).length();
    m.bevel(Mat::Slate, Vec3::new(len, 0.07, hz * 2.0 + 0.3), Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, 0.0), Quat::from_rotation_z(-slope), 0.02, sh(W, 0.85));
    // Arrow kegs.
    for z in [-0.72f32, 0.72] {
        let base = Vec3::new(-0.55, floor, z);
        keg(m, base, 0.13, 0.36);
        for k in 0..7 {
            let a = k as f32 * 2.4;
            let o = Vec3::new(a.cos(), 0.0, a.sin()) * 0.05 * (k as f32).sqrt();
            let top = base + Vec3::Y * 0.62 + o * 1.6;
            m.rod(Mat::Wood, base + Vec3::Y * 0.2 + o, top, 0.01, 0.01, 4, rgb(0.85, 0.75, 0.58), false);
            m.cube(Mat::Paint, Vec3::new(0.012, 0.1, 0.05), top - Vec3::Y * 0.04, Quat::from_rotation_y(a), rgb(0.9, 0.9, 0.86));
        }
    }

    // Archers, each free to turn and draw.
    for (x, z, off) in [(0.32f32, -0.62f32, -0.1f32), (0.46, 0.0, 0.0), (0.32, 0.62, 0.1)] {
        let mut fig = Model::new();
        let mut a = Model::new();
        archer_figure(&mut a, rgb(0.14, 0.24, 0.44), rgb(0.16, 0.18, 0.26));
        fig.append(a, Transform::from_scale(Vec3::splat(0.82)));
        r.at(Role::Archer(off), None, Vec3::new(x, floor, z), fig);
    }
    r
}

// ---------------------------------------------------------------------------
// Battering ram
// ---------------------------------------------------------------------------

/// Chain length from the roof beam to the log.
pub const RAM_CHAIN: f32 = 0.5;

fn ram() -> Rig {
    let mut r = Rig::new();
    let m = &mut r.base;
    let (x0, x1, hz, top) = (-0.42f32, 1.02f32, 0.66f32, 1.28f32);
    let xm = (x0 + x1) / 2.0;
    let len = x1 - x0;
    for s in [-1.0f32, 1.0] {
        let z = s * hz;
        m.bev(Mat::Wood, Vec3::new(len + 0.16, 0.14, 0.15), Vec3::new(xm, 0.07, z), 0.02, dark_wood());
        m.bev(Mat::Wood, Vec3::new(len + 0.16, 0.14, 0.15), Vec3::new(xm, top, z), 0.02, sh(W, 0.9));
        for x in [x0, x1] {
            m.bev(Mat::Wood, Vec3::new(0.15, top, 0.15), Vec3::new(x, top / 2.0, z), 0.025, W);
            for y in [0.14f32, top - 0.1] {
                m.bev(Mat::Iron, Vec3::new(0.16, 0.08, 0.16), Vec3::new(x, y, z), 0.008, W);
            }
        }
        // Cross bracing on the sides.
        let d = Vec2::new(len, top - 0.2);
        let a = d.y.atan2(d.x);
        for sa in [-1.0f32, 1.0] {
            m.bevel(Mat::Wood, Vec3::new(d.length(), 0.09, 0.07), Vec3::new(xm, top / 2.0, z + s * 0.03), Quat::from_rotation_z(sa * a), 0.015, sh(W, 0.85));
        }
        // Iron straps bolting the housing to the castle.
        for y in [0.3f32, 1.05] {
            m.bev(Mat::Iron, Vec3::new(0.5, 0.08, 0.05), Vec3::new(x0 - 0.2, y, z), 0.01, W);
            for dx in [-0.35f32, -0.1] {
                m.rivet(Mat::Iron, Vec3::new(x0 + dx, y, z + s * 0.025), Vec3::Z * s, 0.018, sh(W, 1.3));
            }
        }
    }
    for x in [x0, 0.0, 0.75, x1] {
        m.bev(Mat::Wood, Vec3::new(0.14, 0.14, hz * 2.0 + 0.1), Vec3::new(x, top, 0.0), 0.02, sh(W, 0.88));
    }
    // Shackles for the chains.
    for x in [0.0f32, 0.75] {
        m.bev(Mat::Iron, Vec3::new(0.08, 0.06, 0.16), Vec3::new(x, top - 0.09, 0.0), 0.01, W);
    }
    // Roof of wet hides over planks, with stitched seams.
    let hide = rgb(0.42, 0.30, 0.2);
    m.roof(Vec3::new(xm, top + 0.07, 0.0), Quat::IDENTITY, len + 0.2, hz * 2.0 + 0.1, 0.5, 0.1, hide, Mat::Wood, W, Mat::Paint);
    let ow = hz + 0.05 + 0.1;
    let drop = 0.5 * 0.1 / (hz + 0.05);
    for s in [-1.0f32, 1.0] {
        let slope = (0.5 + drop).atan2(ow);
        let mid = Vec3::new(0.0, top + 0.07 + (0.5 - drop) / 2.0 + 0.012, s * ow / 2.0);
        for x in [x0 + 0.28, xm, x1 - 0.28] {
            m.bevel(Mat::Paint, Vec3::new(0.04, 0.02, ow * 2.0 / slope.cos().max(0.5) * 0.5), Vec3::new(x, mid.y, mid.z), Quat::from_rotation_x(s * slope), 0.006, sh(hide, 0.6));
        }
    }

    // The log, hung level from the beams.
    let mut log = Model::new();
    log.rod(Mat::Bark, Vec3::new(-0.86, 0.0, 0.0), Vec3::new(0.62, 0.0, 0.0), 0.21, 0.2, 12, W, true);
    for x in [-0.66f32, -0.12, 0.4] {
        log.lathe(Mat::Iron, Vec3::new(x, 0.0, 0.0), Quat::from_rotation_z(-FRAC_PI_2), &[(0.0, 0.215), (0.0, 0.228), (0.06, 0.228), (0.06, 0.215)], 16, W);
    }
    // Iron ram's head with curled horns.
    log.lathe(
        Mat::Iron,
        Vec3::new(0.6, 0.0, 0.0),
        Quat::from_rotation_z(-FRAC_PI_2),
        &[(0.0, 0.0), (0.0, 0.235), (0.08, 0.245), (0.2, 0.23), (0.3, 0.18), (0.36, 0.1), (0.38, 0.0)],
        16,
        sh(W, 1.1),
    );
    log.blob(Mat::Iron, Vec3::new(0.96, -0.03, 0.0), 0.11, Vec3::new(1.1, 0.85, 0.8), sh(W, 1.1), 0.0, 1, 2);
    for s in [-1.0f32, 1.0] {
        let c = Vec3::new(0.72, 0.04, s * 0.2);
        let mut prev: Option<(Vec3, f32)> = None;
        for k in 0..=12 {
            let t = k as f32 / 12.0;
            let a = t * 5.2;
            let rad = 0.16 * (1.0 - t * 0.65);
            let p = c + Vec3::new(-a.sin() * rad, a.cos() * rad, s * t * 0.06);
            let w = 0.065 * (1.0 - t * 0.6);
            if let Some((q, wq)) = prev {
                log.rod(Mat::Iron, q, p, wq, w, 8, sh(W, 1.15), false);
            }
            log.blob(Mat::Iron, p, w, Vec3::ONE, sh(W, 1.15), 0.0, 1, 1);
            prev = Some((p, w));
        }
    }
    for x in [-0.35f32, 0.4] {
        log.rivet(Mat::Iron, Vec3::new(x, 0.2, 0.0), Vec3::Y, 0.04, W);
    }
    let log_y = top - 0.09 - RAM_CHAIN - 0.2;
    r.at(Role::RamLog, None, Vec3::new(0.35, log_y, 0.0), log);
    for x in [0.0f32, 0.75] {
        let mut c = Model::new();
        chain(&mut c, RAM_CHAIN);
        r.at(Role::RamChain, None, Vec3::new(x, top - 0.12, 0.0), c);
    }
    r
}

// ---------------------------------------------------------------------------
// Mine chute (faces -X, away from the castle)
// ---------------------------------------------------------------------------

/// Chute slope (radians) and the path a mine slides along.
pub const CHUTE_SLOPE: f32 = 0.36;
pub const CHUTE_TOP: Vec3 = Vec3::new(0.02, 0.56, 0.0);
pub const CHUTE_END: Vec3 = Vec3::new(-0.9, 0.22, 0.0);

/// Small pot mine like the ones dropped on the field.
fn mine_pot(m: &mut Model, base: Vec3, r: f32) {
    m.lathe(Mat::Iron, base, Quat::IDENTITY, &[(0.0, 0.0), (0.0, r * 0.7), (r * 0.3, r), (r * 0.62, r * 0.9), (r * 0.72, r * 0.45), (r * 0.72, 0.0)], 14, sh(W, 0.6));
    m.frustum(Mat::Paint, base + Vec3::Y * r * 0.7, r * 0.22, r * 0.18, r * 0.25, 8, rgb(0.75, 0.14, 0.07), true);
}

fn mine_chute() -> Rig {
    let mut r = Rig::new();
    let m = &mut r.base;
    // Posts against the castle, braced with iron.
    for s in [-1.0f32, 1.0] {
        let z = s * 0.58;
        m.bev(Mat::Wood, Vec3::new(0.15, 1.15, 0.15), Vec3::new(0.66, 0.575, z), 0.025, sh(W, 0.9));
        for y in [0.25f32, 0.95] {
            m.bev(Mat::Iron, Vec3::new(0.36, 0.07, 0.05), Vec3::new(0.82, y, z + s * 0.08), 0.01, W);
            m.rivet(Mat::Iron, Vec3::new(0.7, y, z + s * 0.105), Vec3::Z * s, 0.018, sh(W, 1.3));
        }
        m.bevel(Mat::Wood, Vec3::new(0.09, 0.8, 0.08), Vec3::new(0.33, 0.25, z * 0.9), Quat::from_rotation_z(-0.85), 0.015, dark_wood());
    }
    // Hopper full of mines.
    let (hx0, hx1, hy0, hy1, hz) = (0.06f32, 0.6f32, 0.48f32, 1.08f32, 0.5f32);
    let (cx, cy) = ((hx0 + hx1) / 2.0, (hy0 + hy1) / 2.0);
    m.bev(Mat::Wood, Vec3::new(0.06, hy1 - hy0, hz * 2.0), Vec3::new(hx1 - 0.03, cy, 0.0), 0.012, W);
    m.bev(Mat::Wood, Vec3::new(0.06, hy1 - hy0 - 0.14, hz * 2.0), Vec3::new(hx0 + 0.03, cy + 0.07, 0.0), 0.012, W);
    for s in [-1.0f32, 1.0] {
        m.bev(Mat::Wood, Vec3::new(hx1 - hx0, hy1 - hy0, 0.06), Vec3::new(cx, cy, s * (hz - 0.03)), 0.012, sh(W, 0.92));
        for x in [hx0, hx1] {
            m.bev(Mat::Iron, Vec3::new(0.08, hy1 - hy0 + 0.02, 0.08), Vec3::new(x, cy, s * hz), 0.01, W);
        }
        for y in [hy0 + 0.15, hy1 - 0.15] {
            m.rivet(Mat::Iron, Vec3::new(cx, y, s * hz), Vec3::Z * s, 0.02, sh(W, 1.3));
        }
    }
    m.bevel(Mat::Wood, Vec3::new(hx1 - hx0, 0.05, hz * 2.0 - 0.1), Vec3::new(cx, hy0 + 0.06, 0.0), Quat::from_rotation_z(CHUTE_SLOPE), 0.01, dark_wood());
    for (x, z) in [(0.2f32, -0.22f32), (0.45, -0.24), (0.2, 0.22), (0.45, 0.22)] {
        mine_pot(m, Vec3::new(x, 0.86, z), 0.17);
    }
    // Sloping chute with rails and straps.
    let rot = Quat::from_rotation_z(CHUTE_SLOPE);
    let mid = (CHUTE_TOP + CHUTE_END) / 2.0;
    let clen = CHUTE_TOP.distance(CHUTE_END) + 0.1;
    let down = rot * -Vec3::Y;
    m.bevel(Mat::Wood, Vec3::new(clen, 0.05, 0.46), mid + down * 0.03, rot, 0.012, W);
    for s in [-1.0f32, 1.0] {
        m.bevel(Mat::Wood, Vec3::new(clen, 0.17, 0.05), mid + rot * Vec3::new(0.0, 0.05, s * 0.255), rot, 0.012, sh(W, 0.9));
    }
    for t in [0.15f32, 0.5, 0.85] {
        let p = CHUTE_TOP.lerp(CHUTE_END, t);
        m.bevel(Mat::Iron, Vec3::new(0.06, 0.03, 0.58), p + down * 0.065, rot, 0.006, W);
    }
    // Strut from the posts down to the chute.
    for s in [-1.0f32, 1.0] {
        let a = Vec3::new(0.62, 0.2, s * 0.5);
        let b = CHUTE_END.lerp(CHUTE_TOP, 0.25) + down * 0.08 + Vec3::Z * s * 0.2;
        m.rod(Mat::Wood, a, b, 0.04, 0.04, 6, dark_wood(), true);
    }
    // Flap frame above the outlet.
    m.bev(Mat::Wood, Vec3::new(0.1, 0.08, 0.6), CHUTE_END + Vec3::new(0.0, 0.25, 0.0), 0.015, sh(W, 0.85));

    // Star wheel at the hopper outlet.
    let mut f = Model::new();
    f.rod(Mat::Iron, Vec3::new(0.0, 0.0, -0.27), Vec3::new(0.0, 0.0, 0.27), 0.035, 0.035, 8, W, true);
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2;
        let d = Vec3::new(a.cos(), a.sin(), 0.0);
        f.bevel(Mat::Wood, Vec3::new(0.16, 0.03, 0.4), d * 0.09, Quat::from_rotation_z(a), 0.008, sh(W, 0.95));
    }
    r.at(Role::Feeder, None, CHUTE_TOP + Vec3::new(0.12, 0.05, 0.0), f);
    // A mine on its way down.
    let mut mn = Model::new();
    mine_pot(&mut mn, Vec3::ZERO, 0.17);
    r.add(Role::SlidingMine, None, Transform::from_translation(CHUTE_TOP).with_rotation(rot).with_scale(Vec3::ZERO), mn);
    // Hinged flap.
    let mut fl = Model::new();
    fl.bev(Mat::Wood, Vec3::new(0.04, 0.27, 0.44), Vec3::new(0.0, -0.135, 0.0), 0.01, sh(W, 0.9));
    for z in [-0.14f32, 0.14] {
        fl.bev(Mat::Iron, Vec3::new(0.05, 0.24, 0.04), Vec3::new(-0.01, -0.12, z), 0.006, W);
    }
    r.at(Role::Flap, None, CHUTE_END + Vec3::new(-0.02, 0.22, 0.0), fl);
    r
}
