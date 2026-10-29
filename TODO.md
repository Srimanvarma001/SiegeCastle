# TODO: enemy visuals branch

This branch gives the regular enemies (footmen, archers, knights, war carts)
new models, animation and death falls. Bosses are unchanged.

Checked on October 30, 2026 (Intel Iris Xe): builds with no warnings or
clippy findings, the studio screenshots look right, and a 150 s endless run
(up to 727 troops on screen) stays at 20-27 FPS, the same as with 30 troops,
so the new models are not the bottleneck. Triangles per troop: footmen 567
and 782, archers 602 (622 aiming), knights 1299, carts 2086. The
`SIEGE_TROOPS` studio is kept as a test tool (now in the README). Items left
open need someone to watch the motion in the game.

## 1. Build

- [x] `cargo build --release` passes. Fix any errors (most likely in
      `src/models.rs` or `src/enemy.rs`).
- [x] `cargo clippy` has no new warnings.

## 2. Close-up look (`SIEGE_TROOPS=1`)

Run with `SIEGE_TROOPS=1`. It saves `troops_close.png`, `troops_top.png` and
`troops_game.png` to `screenshots/`, then quits. Rows are each troop look; columns
are side, front-three-quarter, front and back views.

- [x] Spearman: shield sits on the left arm, spear points up and forward,
      steel hat brim is visible.
- [x] Man-at-arms: kite shield and raised sword look right, and the helmet
      doesn't hide the face.
- [x] Archer: face shows under the red hood, quiver feathers are visible,
      and the bow is held properly when walking and when aiming.
- [x] Knight: rider sits on the horse (not inside it), the lance points
      forward, and the pennant, crest and shield are in place.
- [x] War cart: canopy stripes don't flicker, spikes stick out of the iron
      front, and the two pushers' hands touch the back of the cart.
- [x] Nothing looks see-through or inside-out from the game camera angle.

## 3. In the game

- [x] From the normal camera, each troop type is easy to tell apart.
- [ ] Troops are drawn 1.2x bigger (knights 1.12x, carts 1.05x). Check that
      marching formations (1.5m spacing) don't overlap too much. If they do,
      lower `INFANTRY_SCALE` in `src/models.rs`.
- [ ] Walking bob and sway isn't too strong, and the knights' gallop looks
      natural. Tune the numbers in `animate()` in `src/enemy.rs`.
- [ ] Enemies turn smoothly and don't jitter when crowded.
- [x] Archers switch to the drawn-bow pose when in range and face the
      castle while backing away.
- [ ] The squash on hit is visible but not distracting.
- [x] Dead troops fall away from the hit and sink into the ground without
      popping out. Wrecked carts sink fully before disappearing.
- [x] Footmen unloaded by the Siege Tower and knights sent by the Warlord
      use the new looks.

## 4. Speed (Intel Iris Xe)

- [x] The studio run prints each troop's triangle count. Keep footmen and
      archers small, since there are hundreds of them.
- [x] FPS stays playable late in a run with lots of enemies, e.g.
      `SIEGE_ENDLESS=1 SIEGE_LOG=1 SIEGE_AUTOPILOT=1`.
- [ ] Big explosions that kill many enemies at once don't stutter. Bodies
      are capped at 160 (`MAX_CORPSES` in `src/enemy.rs`).

## 5. Merging into main

- [ ] `main` has uncommitted changes (including `src/graphics.rs`) that this
      branch doesn't have. Commit those first, then merge.
- [ ] Watch for small conflicts in `src/main.rs` (one `mod` line and one
      plugin line for the studio) and `src/combat.rs` (`enemy_deaths`).
- [x] Decide whether to keep the `SIEGE_TROOPS` studio (`src/studio.rs`) or
      remove it after checking.

## Later (optional)

- [ ] The soldiers on top of the Siege Tower boss still use the old look,
      since bosses were left alone on purpose. They could match the new
      footmen later.

---

# Colossal bosses: checklist

Work on the `worktree-bosses` branch. Code: `src/boss.rs` (rigs, animation,
camera, burning ground, collapse), `src/boss_ai.rs` (attacks),
`src/boss_models.rs` (models).

To test one boss: `SIEGE_BOSS=3 SIEGE_GOD=1 cargo run --release` (1 to 5 picks
the boss). Add `SIEGE_AUTOPILOT=1` to let the castle drive itself.

## Done

- [x] New models for all five bosses, 3 to 10 times the castle's size
- [x] Moving parts: wheels, turrets with recoil, ramp, gun-port lids, ram,
      trebuchet arm with sling and counterweight, treadwheels, portcullis,
      great bombard, flags, fires
- [x] 3 to 4 attacks per boss, each announced by red circles or lanes
- [x] Harder phases as bosses lose health
- [x] Capsule hit shapes; the castle can no longer drive through bosses
- [x] Collapse animation for each boss, gold dropped at the end
- [x] Camera pulls back to frame a nearby boss; haze moves out with it
- [x] Burning ground hazard, fire pots, boulders that crush troops
- [x] README boss table

## Checked on October 30, 2026

- Builds with no warnings; one clippy finding fixed.
- Each boss run alone for 25 s on the Iris Xe laptop (this branch, shadows
  on): 13-41 FPS after the first seconds, no crash. The 2-3 FPS seen before
  came from a build running at the same time. On `main` (no shadows) it is
  33-63 FPS, and a full run kept 31-72 FPS with up to 788 troops.
- The camera does pull back, but the Siege Tower's top and the Warlord's
  spire ran off the top of the screen. The framing now projects the boss's
  top through the real camera and rises until it fits below the HUD.
- Fights were too short: in a full autopilot run (2x clock, god mode) the
  bosses fell in 20-40 s. After two tuning runs the base health is: Siege
  Tower 1400 -> 2400, War Wagons 800 -> 2400, Juggernaut 2600 -> 5200,
  Trebuchet 3000 -> 8000, Warlord 7500 -> 15000 (fights of ~1-2 minutes).
- Warning rings stand out clearly on grass and roads.
- Boss parts now only update their transform when it changes (less work for
  Bevy's transform propagation every frame).

## Still to do

- [x] Build again after the last cleanup (two unused helpers removed) and
      fix any errors or clippy findings
- [x] Measure FPS with each boss on the Intel Iris Xe laptop. The one test
      run showed 2 to 3 FPS, but a build was using all CPU cores at the same
      time. If slow: turn off shadows on boss parts, merge static parts,
      lower wall and tower detail
- [x] Check the camera pulls back in a normal game (it did not in the one
      test run, most likely because of the low frame rate)
- [ ] Play each boss and check:
  - [ ] 1. Siege Tower: ramp drops and squads walk out, archer volleys,
        oil ring when hugged, topples away when destroyed
  - [ ] 2. War Wagons: take opposite sides, turret tracks and fires,
        ports open before a broadside, survivor charges
  - [ ] 3. Juggernaut: charge lane and troop crushing, ram swing,
        steam vent, double charge with fire trail when hurt
  - [ ] 4. Trebuchet: arm, sling and boulder line up on the throw,
        treadwheels spin on reload, cluster stones when hurt, ballistas
  - [ ] 5. Warlord: turrets, portcullis and knights, bombard cross,
        trample charge, burning spire and fire rain in the last phase
- [x] Tune boss health, damage and cooldowns so each fight lasts about a
      minute or two
- [x] Check warning circles and lanes are easy to see on grass and roads
- [ ] Merge with the uncommitted graphics work on `main`. Expect conflicts in
      `src/world.rs` (camera and haze), `src/enemy.rs` and `src/main.rs`
- [ ] New `docs/bosses.png` screenshot for the README
