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
