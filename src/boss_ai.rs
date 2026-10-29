//! How each of the five bosses fights.
//!
//! Every boss moves like a heavy vehicle (it turns slowly and rolls along its
//! heading) and runs a small state machine of attacks. Every attack that hurts
//! is announced first: red circles where shots will land, red lanes where a
//! charge or a broadside will go.

use crate::art::{p2, w3};
use crate::assets::GameAssets;
use crate::boss::{
    Act, Boss, TREB_REST, TREB_THROWN, sling_pouch, spawn_fire, spec, to_world, warn_circle, warn_lane,
};
use crate::boss_models::{
    TOWER_DECK, TOWER_RAMP_END, TREB_DECK, TREB_TOWER_TOP, TREB_TOWERS, WAGON_DECK, WAGON_HALF_WIDTH, WAGON_PORT_X, WAGON_PORT_Y,
    WARLORD_BOMBARD, WARLORD_GATE_X, WARLORD_TOWER_TOP, WARLORD_TOWERS,
};
use crate::combat::{Bullet, FxKind, Payload, spawn_arc, spawn_fx, spawn_fx_rot};
use crate::director::Director;
use crate::enemy::{Enemy, EnemyKind, Strength, castle_info, spawn_enemy};
use crate::grid::{Grid, MAX_ENEMY_RADIUS, closest_on_segment};
use crate::items::{Loadout, Stats};
use crate::player::Castle;
use crate::{Banner, GameEntity, GameRng, MAP_HALF, Shake};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// What a boss needs to know and touch while deciding what to do.
struct Ctx<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    assets: &'a GameAssets,
    rng: &'a mut GameRng,
    shake: &'a mut f32,
    banner: &'a mut Banner,
    strength: Strength,
    dt: f32,
    cpos: Vec2,
    cvel: Vec2,
    crad: f32,
    /// Damage and push for the castle, applied after all bosses moved.
    hurt: f32,
    knock: Vec2,
}

/// How the boss wants to move this frame.
struct Drive {
    heading: f32,
    speed: f32,
    turn: f32,
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn approach(cur: f32, to: f32, step: f32) -> f32 {
    cur + (to - cur).clamp(-step, step)
}

fn approach_angle(cur: f32, to: f32, step: f32) -> f32 {
    wrap(cur + wrap(to - cur).clamp(-step, step))
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn perp(v: Vec2) -> Vec2 {
    Vec2::new(-v.y, v.x)
}

/// Distance between the castle's edge and the boss's outline.
fn gap(cx: &Ctx, b: &Boss, en: &Enemy, pos: Vec2) -> f32 {
    let axis = Vec2::from_angle(b.heading) * en.half_len;
    closest_on_segment(pos - axis, pos + axis, cx.cpos).distance(cx.cpos) - en.radius - cx.crad
}

/// Where the castle will be when a shot from `from` at `speed` arrives.
fn lead(cx: &Ctx, from: Vec2, speed: f32) -> Vec2 {
    let t = (from.distance(cx.cpos) / speed).min(2.5);
    cx.cpos + cx.cvel * t
}

fn in_lane(p: Vec2, from: Vec2, dir: Vec2, len: f32, half_w: f32) -> bool {
    let rel = p - from;
    let along = rel.dot(dir);
    along >= -half_w && along <= len && rel.dot(perp(dir)).abs() <= half_w
}

fn disc(rng: &mut GameRng, r: f32) -> Vec2 {
    Vec2::from_angle(rng.0.random::<f32>() * TAU) * r * rng.0.random::<f32>().sqrt()
}

/// A heavy cannonball flying flat toward the castle.
fn cannon(cx: &mut Ctx, from: Vec3, vel: Vec2, dmg: f32, size: f32) {
    cx.commands.spawn((
        GameEntity,
        Bullet::hostile(vel, dmg, size * 0.45, 3.5),
        Mesh3d(cx.assets.ball.clone()),
        MeshMaterial3d(cx.assets.m.cannonball.clone()),
        Transform::from_translation(from).with_scale(Vec3::splat(size)),
        NotShadowCaster,
    ));
    let v = w3(vel.normalize_or_zero() * 3.0, 1.5);
    spawn_fx(cx.commands, cx.assets, FxKind::Smoke, from, size * 2.2, v);
    spawn_fx(cx.commands, cx.assets, FxKind::Smoke, from, size * 1.6, v * 1.6);
    spawn_fx(cx.commands, cx.assets, FxKind::Spark, from, size * 1.1, Vec3::ZERO);
}

/// A ballista bolt.
fn bolt(cx: &mut Ctx, from: Vec3, vel: Vec2, dmg: f32) {
    cx.commands.spawn((
        GameEntity,
        Bullet::hostile(vel, dmg, 0.7, 2.0),
        Mesh3d(cx.assets.enemy_arrow.clone()),
        MeshMaterial3d(cx.assets.m.vc.clone()),
        Transform::from_translation(from).with_rotation(crate::art::yaw(vel.to_angle())).with_scale(Vec3::splat(3.2)),
        NotShadowCaster,
    ));
}

fn summon(cx: &mut Ctx, kind: EnemyKind, at: Vec2) {
    let facing = (cx.cpos - at).to_angle();
    spawn_enemy(cx.commands, cx.assets, kind, at, facing, Vec2::ZERO, cx.strength);
}

fn dust(cx: &mut Ctx, at: Vec2, size: f32) {
    let off = disc(cx.rng, size * 0.4);
    spawn_fx(cx.commands, cx.assets, FxKind::Smoke, w3(at + off, 0.8), size, Vec3::new(off.x, 1.5, -off.y));
}

/// Locks a charge direction and paints its lane.
fn start_windup(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2, dir: Vec2, len: f32, total: f32) {
    b.start(Act::Windup);
    b.dir = dir;
    warn_lane(cx.commands, cx.assets, pos + dir * en.half_len, dir, len, en.radius * 2.0, total);
}

// ---------------------------------------------------------------------------
// The system
// ---------------------------------------------------------------------------

pub fn boss_ai(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    director: Res<Director>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    grid: Res<Grid>,
    mut rng: ResMut<GameRng>,
    mut shake: ResMut<Shake>,
    mut banner: ResMut<Banner>,
    mut castle: Query<(&mut Transform, &mut Castle), Without<Enemy>>,
    mut bosses: Query<(&mut Transform, &mut Enemy, &mut Boss)>,
    mut troops: Query<&mut Enemy, Without<Boss>>,
) {
    let Ok((mut ctf, mut c)) = castle.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = time.elapsed_secs();
    let ci = castle_info(&ctf, &c, &stats, &loadout);
    let mut cpos = p2(ctf.translation);
    let wagons: Vec<(Vec2, f32)> = bosses
        .iter()
        .filter(|(_, e, _)| e.kind == EnemyKind::WarWagon && e.hp > 0.0)
        .map(|(t, _, b)| (p2(t.translation), b.side))
        .collect();
    let (mut hurt, mut knock) = (0.0, Vec2::ZERO);
    let mut crush: Vec<(Vec2, Vec2, f32)> = Vec::new();

    for (mut tf, mut en, mut b) in &mut bosses {
        if en.hp <= 0.0 {
            continue;
        }
        let pos = p2(tf.translation);
        b.t += dt;
        for cd in &mut b.cd {
            *cd -= dt;
        }
        for g in &mut b.guns {
            *g -= dt;
        }
        en.attack -= dt;

        let mut cx = Ctx {
            commands: &mut commands,
            assets: &assets,
            rng: &mut rng,
            shake: &mut shake.0,
            banner: &mut banner,
            strength: director.strength,
            dt,
            cpos,
            cvel: c.vel,
            crad: c.radius,
            hurt: 0.0,
            knock: Vec2::ZERO,
        };
        let side = b.side;
        let partner = wagons.iter().find(|(_, s)| *s != side).map(|(p, _)| *p);
        let drive = match en.kind {
            EnemyKind::SiegeTower => tower(&mut cx, &mut b, &en, pos),
            EnemyKind::WarWagon => wagon(&mut cx, &mut b, &en, pos, partner),
            EnemyKind::Juggernaut => juggernaut(&mut cx, &mut b, &en, pos),
            EnemyKind::Trebuchet => trebuchet(&mut cx, &mut b, &en, pos),
            _ => warlord(&mut cx, &mut b, &en, pos),
        };

        // Doors, recoil and fires settle over time.
        let rate = match en.kind {
            EnemyKind::WarWagon => 2.2,
            EnemyKind::Warlord => 0.75,
            _ => 0.9,
        };
        b.pose.hatch = approach(b.pose.hatch, b.pose.hatch_to, rate * dt);
        for r in &mut b.pose.recoil {
            *r = (*r - dt * 2.2).max(0.0);
        }

        // Turn and roll like a heavy vehicle.
        let diff = wrap(drive.heading - b.heading);
        b.heading = wrap(b.heading + diff.clamp(-drive.turn * dt, drive.turn * dt));
        let acc = if b.act == Act::Charge { 30.0 } else { 5.0 };
        b.speed += (drive.speed - b.speed).clamp(-acc * dt, acc * dt);
        let fwd = Vec2::from_angle(b.heading);
        let mut next = pos + fwd * b.speed * dt + en.knock * dt;
        en.knock *= (-3.0 * dt).exp();
        let lim = MAP_HALF - en.radius - en.half_len;
        let clamped = next.clamp(Vec2::splat(-lim), Vec2::splat(lim));
        if clamped != next && b.act == Act::Charge {
            // Crashed into the edge of the world.
            b.start(Act::Stunned);
            b.chain = 0;
            b.speed = 0.0;
            *cx.shake += 1.2;
            for _ in 0..8 {
                dust(&mut cx, next + fwd * (en.half_len + en.radius), 6.0);
            }
        }
        next = clamped;

        // The castle cannot drive through a boss: it gets pushed out.
        let axis = fwd * en.half_len;
        let q = closest_on_segment(next - axis, next + axis, cpos);
        let d = cpos - q;
        let dl = d.length();
        let min = en.radius + c.radius;
        en.touching = false;
        if dl < min {
            let away = if dl > 0.01 { d / dl } else { (cpos - next).normalize_or(Vec2::X) };
            let push = away * (min - dl);
            let lim_c = MAP_HALF - c.radius - 6.0;
            let moved = (cpos + push).clamp(Vec2::splat(-lim_c), Vec2::splat(lim_c));
            next -= push - (moved - cpos);
            cpos = moved;
            cx.cpos = cpos;
            en.touching = true;
            // Ramming hurts bosses too, at most twice a second.
            if ci.speed > 3.0 && (-away).dot(ci.heading) > 0.3 && en.attack <= 0.0 {
                en.hit(ci.ram_hit * 2.0);
                en.attack = 0.5;
                *cx.shake += 0.4;
            }
            if b.act == Act::Charge {
                let dmg = match en.kind {
                    EnemyKind::Juggernaut => 34.0,
                    EnemyKind::Warlord => 40.0,
                    _ => 24.0,
                };
                cx.hurt += dmg * en.dmg_mult;
                cx.knock += b.dir * 26.0 + away * 8.0;
                *cx.shake += 1.6;
                b.start(Act::Stunned);
                b.chain = 0;
                b.speed *= 0.2;
                spawn_fx(cx.commands, cx.assets, FxKind::Shockwave, w3(q + away * en.radius, 1.0), 14.0, Vec3::ZERO);
            }
        }
        if b.act == Act::Charge {
            crush.push((next, axis, en.radius));
        }

        // Hit sparks on the side facing the castle.
        en.flash = (en.flash - dt).max(0.0);
        let want = en.flash > 0.0;
        if want && !en.flashing {
            let q = closest_on_segment(next - axis, next + axis, cpos);
            let at = q + (cpos - q).normalize_or(Vec2::X) * en.radius * 0.9;
            let h = cx.rng.0.random_range(2.0..(spec(en.kind).height * 0.35).max(3.0));
            spawn_fx(cx.commands, cx.assets, FxKind::Spark, w3(at, h), 1.4, Vec3::ZERO);
        }
        en.flashing = want;

        // Wheels, lean, chimney smoke and deep wheel ruts.
        b.pose.wheel += b.speed * dt;
        let lean = -b.speed * 0.004 + if b.speed.abs() > 0.3 { (now * 1.6).sin() * 0.005 } else { 0.0 };
        b.pose.lean = approach(b.pose.lean, lean, dt * 0.05);
        let s = spec(en.kind);
        b.smoke_acc += dt;
        let every = if b.act == Act::Charge || b.act == Act::Windup { 0.08 } else { 0.3 };
        while b.smoke_acc > every {
            b.smoke_acc -= every;
            for p in s.smoke {
                let at = to_world(next, b.heading, *p);
                let kind = if en.kind == EnemyKind::Juggernaut { FxKind::DarkSmoke } else { FxKind::Smoke };
                spawn_fx(cx.commands, cx.assets, kind, at, 3.4, Vec3::new(0.0, 5.0, 0.0) - w3(fwd * b.speed * 0.4, 0.0));
            }
        }
        b.track_acc += b.speed.abs() * dt;
        while b.track_acc > 1.6 {
            b.track_acc -= 1.6;
            let back = next - fwd * (en.half_len + en.radius * 0.5);
            for sgn in [-1.0f32, 1.0] {
                let p = back + perp(fwd) * (sgn * s.track);
                spawn_fx_rot(cx.commands, cx.assets, FxKind::Track, w3(p, 0.03), s.track_size, b.heading);
            }
        }

        hurt += cx.hurt;
        knock += cx.knock;
        en.vel = fwd * b.speed;
        tf.translation = w3(next, 0.0);
        tf.rotation = crate::art::yaw(b.heading);
    }

    if hurt > 0.0 {
        c.hurt(hurt);
    }
    c.knock += knock;
    ctf.translation = w3(cpos, 0.0);

    // Charging bosses flatten their own troops.
    for (p, axis, r) in crush {
        let mut hit = Vec::new();
        grid.for_each_near(p, r + axis.length() + MAX_ENEMY_RADIUS, |g| {
            if g.radius <= MAX_ENEMY_RADIUS && g.axis == Vec2::ZERO {
                let q = closest_on_segment(p - axis, p + axis, g.pos);
                if q.distance(g.pos) < r + g.radius {
                    hit.push(g.entity);
                }
            }
        });
        for e in hit {
            if let Ok(mut t) = troops.get_mut(e) {
                t.hit(1000.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 1. The Siege Tower
// ---------------------------------------------------------------------------

fn tower(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2) -> Drive {
    let to = cx.cpos - pos;
    let g = gap(cx, b, en, pos);
    let mut d = Drive { heading: to.to_angle(), speed: if g > 16.0 { en.speed } else { 0.0 }, turn: 0.5 };
    if b.phase == 0 && en.hp < en.max_hp * 0.5 {
        b.phase = 1;
        cx.banner.show("The Siege Tower rages!", "More soldiers pour from its ramp", 2.5);
    }
    let fwd = Vec2::from_angle(b.heading);
    match b.act {
        Act::Idle => {
            if b.cd[2] <= 0.0 && g < 7.0 {
                b.start(Act::Oil);
                b.cd[2] = 9.0;
                warn_circle(cx.commands, cx.assets, pos, en.radius + 9.0, 1.4);
            } else if b.cd[0] <= 0.0 && g < 45.0 {
                b.start(Act::Unload);
                b.cd[0] = if b.phase > 0 { 8.0 } else { 10.0 };
            } else if b.cd[1] <= 0.0 && g < 60.0 {
                b.start(Act::Volley);
                b.cd[1] = 4.5;
            }
        }
        Act::Unload => {
            d.speed = 0.0;
            b.pose.hatch_to = 1.0;
            let waves = 3 + b.phase as u32;
            if b.step < waves && b.t > 1.5 + b.step as f32 * 0.7 {
                let end = pos + fwd * TOWER_RAMP_END.x;
                let knights = b.phase > 0 && b.step + 1 == waves;
                for i in 0..4 {
                    let p = end + perp(fwd) * (i as f32 * 1.6 - 2.4) + fwd * (b.step as f32 * 0.8);
                    summon(cx, if knights { EnemyKind::Knight } else { EnemyKind::Footman }, p);
                }
                dust(cx, end, 4.0);
                b.step += 1;
            }
            let done = 1.5 + waves as f32 * 0.7;
            if b.t > done + 0.4 {
                b.pose.hatch_to = 0.0;
            }
            if b.t > done + 1.8 {
                b.start(Act::Idle);
            }
        }
        Act::Volley => {
            let bursts = 3 + b.phase as u32;
            if b.step < bursts && b.t > b.step as f32 * 0.5 {
                b.step += 1;
                let top = to_world(pos, b.heading, Vec3::new(0.0, TOWER_DECK + 32.5, 0.0));
                for _ in 0..7 {
                    let dur = 1.5 + to.length() / 70.0;
                    let target = cx.cpos + cx.cvel * dur + disc(cx.rng, 8.0);
                    let from = top + w3(disc(cx.rng, 5.0), 0.0);
                    spawn_arc(cx.commands, cx.assets, from, target, dur, 6.0, Payload::EnemyArrow { dmg: 3.0 * en.dmg_mult });
                }
            }
            if b.step >= bursts && b.t > bursts as f32 * 0.5 + 0.3 {
                b.start(Act::Idle);
            }
        }
        Act::Oil => {
            d.speed = 0.0;
            b.pose.glow = 1.0;
            if b.t > 1.4 && b.step == 0 {
                b.step = 1;
                let r = en.radius + 4.5;
                for i in 0..14 {
                    let a = i as f32 / 14.0 * TAU;
                    spawn_fire(cx.commands, cx.assets, pos + Vec2::from_angle(a) * r, 4.2, 16.0 * en.dmg_mult, 6.0);
                }
                // Burning oil streams down the walls.
                for i in 0..12 {
                    let a = i as f32 / 12.0 * TAU;
                    let p = to_world(pos, b.heading, Vec3::new(a.cos() * 7.2, TOWER_DECK + 6.0 + (i % 4) as f32 * 7.0, a.sin() * 7.2));
                    spawn_fx(cx.commands, cx.assets, FxKind::Fireball, p, 3.0, Vec3::ZERO);
                }
                *cx.shake += 0.6;
            }
            if b.t > 2.0 {
                b.pose.glow = 0.0;
                b.start(Act::Idle);
            }
        }
        _ => b.start(Act::Idle),
    }
    d
}

// ---------------------------------------------------------------------------
// 2. The twin War Wagons
// ---------------------------------------------------------------------------

fn wagon(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2, partner: Option<Vec2>) -> Drive {
    let to = cx.cpos - pos;
    let dist = to.length().max(0.01);
    if !b.enraged && (partner.is_none() || en.hp < en.max_hp * 0.35) {
        b.enraged = true;
        if partner.is_none() {
            cx.banner.show("The last War Wagon goes berserk!", "Scythes out: it will charge you", 3.0);
        }
    }
    let k = if b.enraged { 1.3 } else { 1.0 };
    // Circle the castle; the second wagon takes the opposite side.
    let around = (pos - cx.cpos).to_angle();
    let want = match partner {
        Some(pp) if b.side < 0.0 => (pp - cx.cpos).to_angle() + PI,
        _ => around,
    } + 0.45;
    let target = cx.cpos + Vec2::from_angle(want) * (30.0 + en.radius);
    let mut d = Drive { heading: (target - pos).to_angle(), speed: en.speed * k, turn: 0.9 };

    // The turret follows the castle.
    let tur = to_world(pos, b.heading, Vec3::new(1.5, WAGON_DECK + 9.5, 0.0));
    let tpos = p2(tur);
    let aim = lead(cx, tpos, 30.0);
    let want_yaw = wrap((aim - tpos).to_angle() - b.heading);
    b.pose.turret[0] = approach_angle(b.pose.turret[0], want_yaw, 1.8 * cx.dt);
    let aligned = wrap(b.pose.turret[0] - want_yaw).abs() < 0.15;
    let fwd = Vec2::from_angle(b.heading);
    let left = perp(fwd);

    match b.act {
        Act::Idle => {
            let sd = (to / dist).dot(left);
            if b.cd[1] <= 0.0 && dist < 48.0 && sd.abs() > 0.75 {
                b.start(Act::Broadside);
                b.cd[1] = 6.5;
                b.dir = left * sd.signum();
                b.pose.hatch_to = 1.0;
                for x in WAGON_PORT_X {
                    warn_lane(cx.commands, cx.assets, pos + fwd * x + b.dir * WAGON_HALF_WIDTH, b.dir, 46.0, 3.4, 1.1);
                }
            } else if b.enraged && b.cd[2] <= 0.0 && dist < 70.0 && fwd.dot(to / dist) > 0.85 {
                b.cd[2] = 7.0;
                start_windup(cx, b, en, pos, fwd, 60.0, 1.0 + 1.6);
            } else if b.cd[0] <= 0.0 && dist < 60.0 && aligned {
                b.cd[0] = if b.enraged { 1.5 } else { 2.4 };
                let dir = Vec2::from_angle(b.heading + b.pose.turret[0]);
                cannon(cx, tur + w3(dir, 0.0) * 10.5, dir * 30.0, 12.0 * en.dmg_mult, 2.2);
                b.pose.recoil[0] = 1.0;
                *cx.shake += 0.2;
            }
        }
        Act::Broadside => {
            d.speed *= 0.4;
            if b.t > 1.1 && b.step == 0 {
                b.step = 1;
                for x in WAGON_PORT_X {
                    for s in [-1.0f32, 0.0, 1.0] {
                        let dir = Vec2::from_angle(b.dir.to_angle() + s * 0.07);
                        let from = w3(pos + fwd * x + b.dir * (WAGON_HALF_WIDTH + 1.0), WAGON_PORT_Y);
                        cannon(cx, from, dir * 28.0, 9.0 * en.dmg_mult, 1.6);
                    }
                }
                *cx.shake += 0.6;
            }
            if b.t > 2.0 {
                b.pose.hatch_to = 0.0;
                b.start(Act::Idle);
            }
        }
        Act::Windup => {
            d.speed = 0.0;
            d.heading = b.dir.to_angle();
            d.turn = 0.0;
            if b.t > 1.0 {
                b.start(Act::Charge);
            }
        }
        Act::Charge => {
            d.speed = 30.0;
            d.heading = b.dir.to_angle();
            d.turn = 0.0;
            if b.t > 1.6 {
                b.start(Act::Idle);
            }
        }
        Act::Stunned => {
            d.speed = 0.0;
            if b.t > 1.0 {
                b.start(Act::Idle);
            }
        }
        _ => b.start(Act::Idle),
    }
    d
}

// ---------------------------------------------------------------------------
// 3. The Iron Juggernaut
// ---------------------------------------------------------------------------

fn juggernaut(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2) -> Drive {
    let to = cx.cpos - pos;
    let dist = to.length().max(0.01);
    let fwd = Vec2::from_angle(b.heading);
    let ahead = fwd.dot(to / dist);
    let g = gap(cx, b, en, pos);
    if b.phase == 0 && en.hp < en.max_hp * 0.5 {
        b.phase = 1;
        cx.banner.show("The Juggernaut's furnace roars!", "It charges twice now, trailing fire", 3.0);
    }
    let mut d = Drive { heading: to.to_angle(), speed: if g > 4.0 { en.speed } else { 1.0 }, turn: 0.4 };
    let hot = matches!(b.act, Act::Windup | Act::Charge | Act::Vent);
    b.pose.glow = approach(b.pose.glow, if hot { 1.0 } else { 0.0 }, cx.dt * 2.0);
    let front = pos + fwd * (en.half_len + en.radius - 2.0);
    match b.act {
        Act::Idle => {
            b.pose.swing = approach(b.pose.swing, (b.t * 1.4).sin() * 0.06, cx.dt);
            if b.cd[1] <= 0.0 && ahead > 0.6 && g < 16.0 {
                b.start(Act::Swing);
                b.cd[1] = 3.2;
                warn_lane(cx.commands, cx.assets, front, fwd, 20.0, 11.0, 0.95);
            } else if b.cd[2] <= 0.0 && g < 10.0 && ahead < 0.6 {
                b.start(Act::Vent);
                b.cd[2] = 6.0;
                warn_circle(cx.commands, cx.assets, pos, en.radius + en.half_len + 10.0, 1.0);
            } else if b.cd[0] <= 0.0 && g > 14.0 && g < 80.0 && ahead > 0.9 {
                b.chain = if b.phase > 0 { 2 } else { 1 };
                start_windup(cx, b, en, pos, fwd, 80.0, 1.4 + 2.3);
            }
        }
        Act::Windup => {
            d.speed = 0.0;
            d.heading = b.dir.to_angle();
            d.turn = 1.6;
            b.pose.swing = approach(b.pose.swing, -0.25, cx.dt);
            if cx.rng.0.random::<f32>() < 0.5 {
                let back = pos - fwd * en.half_len + perp(fwd) * (if cx.rng.0.random::<bool>() { 8.3 } else { -8.3 });
                dust(cx, back, 4.0);
            }
            if b.t > 1.4 {
                b.start(Act::Charge);
                *cx.shake += 0.6;
            }
        }
        Act::Charge => {
            d.speed = 28.0;
            d.heading = b.dir.to_angle();
            d.turn = 3.0;
            b.pose.swing = approach(b.pose.swing, 0.35, cx.dt * 2.0);
            // In its last phase it leaves a trail of burning oil.
            if b.phase > 0 && b.t > b.step as f32 * 0.22 {
                b.step += 1;
                spawn_fire(cx.commands, cx.assets, pos - fwd * en.half_len, 3.6, 10.0 * en.dmg_mult, 5.0);
            }
            if b.t > 2.3 {
                b.chain = b.chain.saturating_sub(1);
                if b.chain > 0 {
                    let dir = (cx.cpos - pos).normalize_or(fwd);
                    start_windup(cx, b, en, pos, dir, 80.0, 1.2 + 2.3);
                    b.t = 0.2;
                } else {
                    b.cd[0] = 6.0;
                    b.start(Act::Idle);
                }
            }
        }
        Act::Stunned => {
            d.speed = 0.0;
            if cx.rng.0.random::<f32>() < 0.2 {
                let at = pos + disc(cx.rng, en.radius);
                dust(cx, at, 3.0);
            }
            if b.t > 2.0 {
                b.cd[0] = b.cd[0].max(4.0);
                b.start(Act::Idle);
            }
        }
        Act::Swing => {
            d.speed = 0.0;
            let t = b.t;
            b.pose.swing = if t < 0.8 {
                -0.45 * ease(t / 0.8)
            } else if t < 1.05 {
                -0.45 + 1.45 * ease((t - 0.8) / 0.25)
            } else {
                1.0 * (1.0 - ease((t - 1.05) / 0.95))
            };
            if t >= 0.95 && b.step == 0 {
                b.step = 1;
                if in_lane(cx.cpos, front, fwd, 20.0 + cx.crad, 5.5 + cx.crad) {
                    cx.hurt += 30.0 * en.dmg_mult;
                    cx.knock += fwd * 24.0;
                    *cx.shake += 1.4;
                }
                let tip = front + fwd * 12.0;
                spawn_fx(cx.commands, cx.assets, FxKind::Shockwave, w3(tip, 1.0), 10.0, Vec3::ZERO);
                for _ in 0..4 {
                    dust(cx, tip, 5.0);
                }
            }
            if t > 2.0 {
                b.start(Act::Idle);
            }
        }
        Act::Vent => {
            d.speed = 0.0;
            if b.t >= 1.0 && b.step == 0 {
                b.step = 1;
                let reach = en.radius + en.half_len + 10.0;
                for i in 0..18 {
                    let a = i as f32 / 18.0 * TAU;
                    let dir = Vec2::from_angle(a);
                    let at = pos + dir * (en.radius + 2.0) + fwd * (dir.dot(fwd) * en.half_len);
                    spawn_fx(cx.commands, cx.assets, FxKind::Smoke, w3(at, 3.0), 6.0, w3(dir * 12.0, 3.0));
                }
                spawn_fx(cx.commands, cx.assets, FxKind::Shockwave, w3(pos, 1.2), reach, Vec3::ZERO);
                if pos.distance(cx.cpos) < reach + cx.crad {
                    cx.hurt += 16.0 * en.dmg_mult;
                    cx.knock += (cx.cpos - pos).normalize_or(Vec2::X) * 24.0;
                    *cx.shake += 0.9;
                }
            }
            if b.t > 1.5 {
                b.start(Act::Idle);
            }
        }
        _ => b.start(Act::Idle),
    }
    d
}

// ---------------------------------------------------------------------------
// 4. The Trebuchet Fortress
// ---------------------------------------------------------------------------

fn trebuchet(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2) -> Drive {
    let to = cx.cpos - pos;
    let g = gap(cx, b, en, pos);
    if b.phase == 0 && en.hp < en.max_hp * 0.5 {
        b.phase = 1;
        cx.banner.show("The Trebuchet loads cluster stones!", "Each throw now splits into five", 3.0);
    }
    let speed = if g > 75.0 {
        en.speed
    } else if g < 42.0 {
        -en.speed * 0.8
    } else {
        0.0
    };
    let mut d = Drive { heading: to.to_angle(), speed, turn: 0.35 };

    // Ballistas on the corner towers guard it up close.
    for (i, (x, z)) in TREB_TOWERS.into_iter().enumerate() {
        let slot = i + 1;
        let tw = to_world(pos, b.heading, Vec3::new(x, TREB_TOWER_TOP + 1.5, z));
        let tp = p2(tw);
        let aim = lead(cx, tp, 50.0);
        let want = wrap((aim - tp).to_angle() - b.heading);
        b.pose.turret[slot] = approach_angle(b.pose.turret[slot], want, 2.0 * cx.dt);
        if b.guns[slot] <= 0.0 && tp.distance(cx.cpos) < 60.0 && wrap(b.pose.turret[slot] - want).abs() < 0.12 {
            b.guns[slot] = if b.phase > 0 { 1.3 } else { 1.9 };
            let dir = Vec2::from_angle(b.heading + b.pose.turret[slot]);
            bolt(cx, tw + w3(dir, 0.0) * 3.0, dir * 50.0, 7.0 * en.dmg_mult);
        }
    }

    let throw_time = 0.9;
    let flight = 2.6;
    match b.act {
        Act::Idle => {
            if b.cd[0] <= 0.0 && b.pose.loaded && g < 130.0 {
                b.start(Act::Throw);
                b.cd[0] = 1.5;
                let target = cx.cpos + cx.cvel * (throw_time + flight) * 0.8;
                b.marks.clear();
                if b.phase > 0 {
                    b.marks.push(target);
                    for k in 0..4 {
                        b.marks.push(target + Vec2::from_angle(k as f32 * FRAC_PI_2 + 0.4) * 10.0);
                    }
                } else {
                    b.marks.push(target);
                }
                let r = if b.phase > 0 { 4.5 } else { 7.0 };
                for m in b.marks.clone() {
                    warn_circle(cx.commands, cx.assets, m, r, throw_time + flight);
                }
            } else if b.cd[1] <= 0.0 && g < 100.0 {
                b.start(Act::Pots);
                b.cd[1] = if b.phase > 0 { 7.0 } else { 9.5 };
            }
        }
        Act::Throw => {
            d.speed = 0.0;
            let k = (b.t / 1.15).min(1.0);
            b.pose.arm = TREB_REST + (TREB_THROWN - TREB_REST) * ease(k);
            if b.pose.loaded {
                b.pose.sling = 0.25 + (1.24 - 0.25) * ease((k - 0.15) / 0.65);
            } else {
                b.pose.sling = approach(b.pose.sling, -FRAC_PI_2, cx.dt * 2.5);
            }
            if b.t >= throw_time && b.pose.loaded {
                b.pose.loaded = false;
                let from = to_world(pos, b.heading, sling_pouch(&b.pose));
                let (dmg, r) = if b.phase > 0 { (16.0, 4.5) } else { (30.0, 7.0) };
                for (i, m) in b.marks.clone().into_iter().enumerate() {
                    spawn_arc(cx.commands, cx.assets, from, m, flight - i as f32 * 0.04, 42.0, Payload::Boulder { dmg: dmg * en.dmg_mult, radius: r });
                }
                *cx.shake += 0.7;
            }
            if b.t > 1.9 {
                b.start(Act::Reload);
            }
        }
        Act::Reload => {
            let k = (b.t / 3.2).min(1.0);
            b.pose.arm = TREB_THROWN + (TREB_REST - TREB_THROWN) * ease(k);
            b.pose.reel += cx.dt * 1.6;
            let rest = if k > 0.8 { 0.25 } else { -FRAC_PI_2 };
            b.pose.sling = approach(b.pose.sling, rest, cx.dt * 2.5);
            if k >= 1.0 {
                b.pose.loaded = true;
                b.pose.sling = 0.25;
                b.start(Act::Idle);
            }
        }
        Act::Pots => {
            d.speed = 0.0;
            if b.t > 0.4 && b.step == 0 {
                b.step = 1;
                let n = if b.phase > 0 { 8 } else { 6 };
                for _ in 0..n {
                    let target = cx.cpos + cx.cvel * 1.6 + disc(cx.rng, 16.0);
                    let z = cx.rng.0.random_range(-8.0..8.0);
                    let from = to_world(pos, b.heading, Vec3::new(16.5, TREB_DECK + 8.0, z));
                    warn_circle(cx.commands, cx.assets, target, 4.5, 1.8);
                    spawn_arc(cx.commands, cx.assets, from, target, 1.8, 18.0, Payload::FirePot { dmg: 9.0 * en.dmg_mult, radius: 4.5 });
                }
            }
            if b.t > 1.0 {
                b.start(Act::Idle);
            }
        }
        _ => b.start(Act::Idle),
    }
    d
}

// ---------------------------------------------------------------------------
// 5. The Warlord's Citadel
// ---------------------------------------------------------------------------

fn warlord(cx: &mut Ctx, b: &mut Boss, en: &Enemy, pos: Vec2) -> Drive {
    let hp = en.hp / en.max_hp;
    let phase = if hp < 0.33 {
        2
    } else if hp < 0.66 {
        1
    } else {
        0
    };
    if phase > b.phase {
        b.phase = phase;
        *cx.shake += 1.5;
        if phase == 1 {
            cx.banner.show("The Warlord wheels out the great bombard!", "Watch for the cross of red circles", 3.5);
        } else {
            cx.banner.show("THE CITADEL BURNS", "The Warlord calls down fire. Keep moving!", 3.5);
            b.pose.glow = 1.0;
        }
    }
    let to = cx.cpos - pos;
    let dist = to.length().max(0.01);
    let fwd = Vec2::from_angle(b.heading);
    let ahead = fwd.dot(to / dist);
    let g = gap(cx, b, en, pos);
    let speed = en.speed * (1.0 + 0.25 * b.phase as f32);
    let mut d = Drive { heading: to.to_angle(), speed: if g > 12.0 { speed } else { 0.0 }, turn: 0.2 };

    // Cannon turrets on the four corner towers.
    for (i, (x, z)) in WARLORD_TOWERS.into_iter().enumerate() {
        let slot = i + 1;
        let tw = to_world(pos, b.heading, Vec3::new(x, WARLORD_TOWER_TOP + 1.7, z));
        let tp = p2(tw);
        let aim = lead(cx, tp, 30.0);
        let want = wrap((aim - tp).to_angle() - b.heading);
        b.pose.turret[slot] = approach_angle(b.pose.turret[slot], want, 1.4 * cx.dt);
        if b.guns[slot] <= 0.0 && tp.distance(cx.cpos) < 75.0 && wrap(b.pose.turret[slot] - want).abs() < 0.12 {
            b.guns[slot] = [3.0, 2.4, 1.7][b.phase as usize];
            let dir = Vec2::from_angle(b.heading + b.pose.turret[slot]);
            cannon(cx, tw + w3(dir, 0.0) * 10.0, dir * 30.0, 11.0 * en.dmg_mult, 2.0);
            b.pose.recoil[slot] = 1.0;
        }
    }

    match b.act {
        Act::Idle => {
            b.pose.pitch = approach(b.pose.pitch, 0.0, cx.dt * 0.3);
            if b.phase >= 2 && b.cd[3] <= 0.0 {
                b.start(Act::Pots);
                b.cd[3] = 10.0;
            } else if b.phase >= 1 && b.cd[1] <= 0.0 && g < 110.0 {
                b.start(Act::Bombard);
                b.cd[1] = 9.0;
                let target = cx.cpos + cx.cvel * 2.6;
                b.marks.clear();
                b.marks.push(target);
                for k in 0..4 {
                    b.marks.push(target + Vec2::from_angle(k as f32 * FRAC_PI_2) * 11.0);
                }
                for m in b.marks.clone() {
                    warn_circle(cx.commands, cx.assets, m, 6.0, 1.2 + 2.4);
                }
            } else if b.phase >= 1 && b.cd[2] <= 0.0 && g > 14.0 && g < 80.0 && ahead > 0.85 {
                b.cd[2] = 13.0;
                start_windup(cx, b, en, pos, fwd, 70.0, 2.0 + 3.0);
            } else if b.cd[0] <= 0.0 && g < 80.0 {
                b.start(Act::Sally);
                b.cd[0] = 16.0;
            }
        }
        Act::Sally => {
            d.speed = 0.0;
            b.pose.hatch_to = 1.0;
            if b.step < 3 && b.t > 1.4 + b.step as f32 * 0.6 {
                let gate = pos + fwd * (WARLORD_GATE_X + 3.0);
                for i in 0..3 {
                    summon(cx, EnemyKind::Knight, gate + perp(fwd) * (i as f32 * 2.0 - 2.0) + fwd * (b.step as f32 * 1.5));
                }
                if b.phase >= 1 {
                    for i in 0..2 {
                        summon(cx, EnemyKind::Footman, gate + perp(fwd) * (i as f32 * 2.4 - 1.2) - fwd);
                    }
                }
                dust(cx, gate, 4.0);
                b.step += 1;
            }
            if b.t > 3.4 {
                b.pose.hatch_to = 0.0;
            }
            if b.t > 4.6 {
                b.start(Act::Idle);
            }
        }
        Act::Bombard => {
            d.speed *= 0.3;
            b.pose.pitch = approach(b.pose.pitch, 0.22, cx.dt * 0.4);
            if b.t >= 1.2 && b.step == 0 {
                b.step = 1;
                let muzzle = to_world(pos, b.heading, WARLORD_BOMBARD + Quat::from_rotation_z(b.pose.pitch) * Vec3::new(15.5, 0.0, 0.0));
                for m in b.marks.clone() {
                    spawn_arc(cx.commands, cx.assets, muzzle, m, 2.4, 34.0, Payload::Boulder { dmg: 22.0 * en.dmg_mult, radius: 6.0 });
                }
                b.pose.recoil[0] = 1.0;
                for i in 0..6 {
                    let v = w3(fwd * (4.0 + i as f32 * 2.0), 2.0);
                    spawn_fx(cx.commands, cx.assets, FxKind::Smoke, muzzle, 6.0, v);
                }
                spawn_fx(cx.commands, cx.assets, FxKind::Fireball, muzzle, 6.0, Vec3::ZERO);
                *cx.shake += 1.2;
            }
            if b.t > 3.0 {
                b.start(Act::Idle);
            }
        }
        Act::Windup => {
            d.speed = 0.0;
            d.heading = b.dir.to_angle();
            d.turn = 0.0;
            if cx.rng.0.random::<f32>() < 0.6 {
                let back = pos - fwd * en.half_len + perp(fwd) * cx.rng.0.random_range(-23.0..23.0);
                dust(cx, back, 6.0);
            }
            if b.t > 2.0 {
                b.start(Act::Charge);
                *cx.shake += 1.0;
            }
        }
        Act::Charge => {
            d.speed = 15.0;
            d.heading = b.dir.to_angle();
            d.turn = 0.0;
            if b.t > 3.0 {
                b.start(Act::Idle);
            }
        }
        Act::Stunned => {
            d.speed = 0.0;
            if b.t > 1.5 {
                b.start(Act::Idle);
            }
        }
        Act::Pots => {
            // Fire rains from the burning spire.
            d.speed = 0.0;
            if b.step < 16 && b.t > b.step as f32 * 0.18 {
                b.step += 1;
                let target = cx.cpos + cx.cvel * 1.0 + disc(cx.rng, 30.0);
                let from = to_world(pos, b.heading, Vec3::new(-4.0, 62.0, 0.0));
                warn_circle(cx.commands, cx.assets, target, 5.0, 1.6);
                spawn_arc(cx.commands, cx.assets, from, target, 1.6, 12.0, Payload::FirePot { dmg: 9.0 * en.dmg_mult, radius: 5.0 });
            }
            if b.t > 3.4 {
                b.start(Act::Idle);
            }
        }
        _ => b.start(Act::Idle),
    }
    d
}
