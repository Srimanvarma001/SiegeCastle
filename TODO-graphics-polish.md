# Graphics polish: things to check before merging

Played and screenshotted on October 30, 2026 (Iris Xe): no crash, no
compiler or clippy warnings after the cleanup below. Fixed while testing:
- Banner drop shadows never faded, so "Defend the realm!" and boss
  announcements stayed on screen as dark text. They now fade with the text.
- The XP numbers were pale text on the pale gold fill; they now sit on a
  small dark pill.
Items left open need someone to watch the motion in the game. When merged
into `main`, the troop and boss animation here is replaced by the
enemy-visuals and bosses branches (see MERGE_NOTES.md).

## Look in game

- [x] Smoke, dust and fire sprites: right size and brightness against the grass
      (they are unlit; tune the colors in `vfx::build_assets` if they look too
      dark or too bright).
- [ ] Explosion flash lights: not too bright, and FPS holds up when many go off
      at once (capped at 10 lights, `MAX_LIGHTS` in `vfx.rs`).
- [x] Wheel ruts behind the castle: no dark seams between segments.
- [ ] Blood splats and scorch marks: no flickering against the ground (z-fighting).
- [ ] Castle: wheels roll the right way, body lean feels right when turning and
      speeding up, rocks when hit.
- [ ] Weapons: ballista, bombard and archers turn toward targets; barrels recoil;
      ballista bolt hides and reloads; side cannons only flash on the side that fires.
- [ ] Ram log swing looks right and does not clip through the frame.
- [ ] Troops: walking bob, attack lunges at the walls, knockback tumble, corpses
      fall over and sink (max 260 at once).
- [ ] Bosses (`SIEGE_GALLERY=1` spawns all five):
  - [ ] Siege Tower drawbridge drops toward the front and back up.
  - [ ] War Wagon drives sideways around you while its turret tracks you.
  - [ ] Juggernaut drill spins up before charging; sparks at the tip.
  - [ ] Trebuchet arm swing direction and timing match the boulder throw.
  - [ ] Warlord guns recoil on each volley.
  - [ ] Defeated bosses sink in fire and smoke instead of vanishing.
- [ ] Enemy cannonballs now start at barrel height and drop down; make sure they
      still look like they hit the castle.

## UI

- [ ] Vignette strength (screen edges) and the red flush when hit.
- [ ] Health and boss bars: pale trail after damage, low-health pulse.
- [ ] Ability slots: clock-hand cooldown sweep, gold flash when ready.
- [ ] Level ring pop on level-up.
- [x] Level-up cards rise in, lift and glow on hover, still clickable.
- [ ] Pause and game-over screens.

## Merging with main

- [ ] Expect conflicts with the other session's work in `combat.rs`, `enemy.rs`,
      `modules.rs`, `main.rs` and maybe `world.rs` / `previews.rs`.
      - `combat.rs`: the effects code moved to `vfx.rs`; port any changes made
        to `spawn_fx` / `update_fx` / `explosion` there.
      - `main.rs`: keep both sets of new modules and plugins.
- [ ] If the graphics presets (low/medium/high) land on main, turn down effects
      on low: fewer flash lights, smoke and corpses.

## Cleanup

- [x] Remove the unused old effect materials from `assets::Mats` (fire, spark,
      smoke, smoke_dark, track, scorch, dirt, blood and the fade lists) to clear
      the compiler warning.
- [ ] Update the README screenshots once it looks right.

## Build note

On this machine, building with all 16 cores sometimes crashes the compiler
(`STATUS_STACK_BUFFER_OVERRUN`). `cargo build -j 4` and re-running after a
crash finishes the build.
