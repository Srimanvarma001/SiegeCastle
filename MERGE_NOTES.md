# Merge notes

How the parallel branches were finished and merged into `main` (October 30, 2026).
Every branch and worktree is kept; nothing was force-pushed.

## Branches found

| Branch | Worktree | What it builds |
|---|---|---|
| `smoother-play` | (main folder) | Faster play on laptop graphics: DirectX 12 fixes, no shadows, quality presets, wider camera, mouse-wheel zoom |
| `worktree-enemy-visuals` | `.claude/worktrees/enemy-visuals` | New models for regular troops, walk cycles, archer aiming pose, falling bodies |
| `worktree-bosses` | `.claude/worktrees/bosses` | Five colossal bosses: new models, moving parts, attacks, phases, collapse, camera framing |
| `graphics-polish` / `worktree-graphics-polish` | `.claude/worktrees/graphics-polish` | New effects (`vfx.rs`), new HUD, castle and weapon animation (`anim.rs`) |
| `worktree-weapon-mounts` | `.claude/worktrees/weapon-mounts` | All six weapons rebuilt and animated (`weapons.rs`), proper wall platforms |
| `merge-all` | scratchpad | Temporary branch used to do the merges; same commit as `main` |

## Order

1. `smoother-play` (fast-forward)
2. `worktree-enemy-visuals` (no conflicts)
3. `worktree-bosses`
4. `graphics-polish`
5. `worktree-weapon-mounts`

When two branches rebuilt the same part of the game, the more complete
rewrite of that part was kept, and the other branch's version of it removed,
so nothing runs twice:

| Part | Kept from | Dropped from |
|---|---|---|
| Troop models, walk cycle, bodies | `worktree-enemy-visuals` | `graphics-polish` (`animate_troops`, its corpses) |
| Boss models, moving parts, collapse | `worktree-bosses` | `graphics-polish` (boss pieces on the old boss models, `BossAnim`, wrecks) |
| Effects | `graphics-polish` (`vfx.rs`) | the old effects in `combat.rs` |
| Weapon models and animation | `worktree-weapon-mounts` (`weapon_models.rs`, `weapons.rs`) | `graphics-polish` (its weapon rigs and weapon animation in `anim.rs`) |
| Castle wheels, flags, sway | `graphics-polish` (`anim.rs`) | — |
| Lighting | `smoother-play` (no shadows) | shadow markers in new code |

## 1. smoother-play

**What it does:** makes the game run well on laptop graphics.
- DirectX 12 on Windows by default (some Intel Vulkan drivers run out of
  memory), and finds the DXC shader compiler next to the exe, not only in
  the working folder.
- Fixes a DirectX 12 crash when the window is minimized and restored.
- Graphics presets (`graphics.rs`): Low / Medium / High picked from the GPU,
  F9 cycles. The 3D world renders at 67-100% of the window and is stretched
  over it, while the HUD stays sharp.
- No shadows anywhere, a higher camera (58 m), mouse-wheel / `-` `=` zoom,
  a larger ground so the edge never shows.
- The card portraits are drawn one camera per frame instead of all at once
  (removed a stutter in the first seconds).
- The follow camera only looks for the game camera (`MainCamera`), so it no
  longer waits ~10 s for the portrait cameras to go away.
- Test switches `SIEGE_PROFILE`, `SIEGE_VSYNC`, `SIEGE_QUALITY`, `SIEGE_GFX`;
  `play.bat` launcher.

**Incomplete:** nothing found. No TODO file; the code matched its commit message.

**Review and test (on the branch):** builds with no warnings; 40 s autopilot
run with a boss: no crash, world on screen 4.9 s after launch, 41-53 FPS on
Iris Xe (Medium preset, vsync on). Main thread ~3 ms per frame, render thread
~10 ms preparing and ~10 ms submitting, GPU ~11 ms (opaque pass 6-7 ms,
bloom ~1.7 ms). The render thread is the limit; see PERF_NOTES.md.

**Finished / polished:** README now mentions `play.bat`.

**Performance:** no problems in this branch's code; it is the performance
branch. The render-thread cost is looked at in the final profiling pass.

**Later fix:** clippy was installed during the enemy-visuals step and found
two small things in this branch's code (an over-indented doc line in
`debug.rs`, a manual `is_multiple_of` in `previews.rs`); fixed on the branch
and merged again.

**Merge:** first merged as a fast-forward; both follow-ups merged cleanly.

## 2. worktree-enemy-visuals

**What it does:** new crimson-and-gold models for the regular enemies, told
apart from above: spearmen (kettle hat, round shield) and men-at-arms (pointed
helm, kite shield) picked at random, hooded archers with a second "bow drawn"
model, knights with a striped lance, and war carts under a striped canopy.
Troops get a walk / gallop / rumble cycle, lunge at the walls, squash when
hit, and leave a body that topples over and sinks (charred wreck for carts,
at most 160). `SIEGE_TROOPS=1` (`studio.rs`) photographs every look.

**Incomplete:** the branch had never been compiled or run (its TODO said so).

**Review and test (on the branch):**
- Builds with no warnings; clippy (installed for this) finds nothing.
- `SIEGE_TROOPS=1` close-up, top and game-distance shots: every look is right
  (shields, helmets, hoods, quivers, rider on the horse, cart spikes) and
  each type is easy to tell apart from the game camera.
- Triangles: footmen 567 / 782, archers 602 (622 aiming), knights 1299,
  carts 2086.
- 150 s endless stress run (3x clock): no crash; 20-27 FPS with 30 troops
  and the same with 727, so troop drawing is not the bottleneck (this branch
  still has the old shadows and SSAO, which is why its FPS is lower).
- Code review of the walk cycle, archer pose switch and corpse timing found
  no problems; carts sink 2.6 m before being removed, so nothing pops out.

**Finished / polished:**
- On the branch: TODO checklist updated with the results; `SIEGE_TROOPS`
  documented in the README; the studio is kept as a test tool.
- On `main` after merging (needs the bosses branch): the Siege Tower's crew
  now uses the new red-hooded enemy archer (bow drawn) instead of the
  castle's own archer figure, the TODO's "Later" item.

**Performance:** nothing to fix; per-troop animation only moves the whole
mesh, mesh and material swaps happen only when the state changes, and all
troops share one material so they batch.

**Merge:** first merge had no conflicts. The follow-up commit merged cleanly
too (`TODO.md` auto-merged with the bosses checklist).

## 3. worktree-bosses

**What it does:** replaces the five bosses with colossal ones (3-10x the
castle): new models built as rigs of moving parts (`boss_models.rs`), 3-4
attacks each announced by red rings and lanes, harder phases at low health,
capsule hit shapes you cannot drive through, burning ground, boulders that
crush troops, a collapse animation per boss with gold at the end
(`boss.rs`, `boss_ai.rs`), and a camera that pulls back to frame a near boss.

**Incomplete (from its TODO):** not rebuilt after its last cleanup; FPS not
measured (one test showed 2-3 FPS); camera pull-back unconfirmed; bosses not
played; health and damage not tuned; warning visibility unchecked; not merged;
no README screenshot.

**Review and test (on the branch):**
- Builds with no warnings; clippy had one finding (`is_multiple_of`), fixed.
- Each boss alone for 25 s (`SIEGE_BOSS=1..5`), then all five together
  (`SIEGE_GALLERY=1`): no crash; 13-41 FPS on this branch, which still has
  the old shadows. The 2-3 FPS came from a build running at the same time.
- Screenshots: Trebuchet arm swing and its red target rings show clearly on
  grass and roads; the Juggernaut and castle stay framed together.
- The camera pulled back but cut off tall bosses: the Siege Tower's top and
  the Warlord's spire ran off the top of the screen.
- A full autopilot game (2x clock, god mode) beat every boss in 20-40 s:
  Siege Tower ~40 s, War Wagons ~30 s, Juggernaut ~40 s, Trebuchet ~20 s,
  Warlord ~30 s. The checklist asks for one to two minutes.

**Finished / polished (on the branch):**
- Camera framing now projects the castle, the near part of the boss and the
  boss's top through the real camera and raises the camera until all of them
  fit below the HUD bars (`boss::on_screen`).
- Base boss health raised in two tuning runs. First pass (2800 / 2000 /
  5200 / 10000 / 20000) gave fights of ~110 / 55 / 78 / 130 / 155 s on
  `main`; the final values are Siege Tower 1400 -> 2400, War Wagons
  800 -> 2400, Juggernaut 2600 -> 5200, Trebuchet 3000 -> 8000, Warlord
  7500 -> 15000, aiming at roughly 65-115 s each. That full 15-minute run
  had no crash and 23-66 FPS (median 41) with up to 1013 troops.
- The README boss screenshot (`docs/bosses.png`) is retaken after the
  weapons branch is merged, so it shows the final castle (see section 5).
- TODO.md records what was checked.

**Performance:** boss parts wrote their transform every frame even when they
did not move, so Bevy re-propagated every boss mesh every frame. They now
only write when the pose changes. Troop crushing already uses the grid.

**Second merge:** one conflict in `boss.rs` (the line that divided by the
camera height). Kept the new projected framing, with its base height set to
`world::CAMERA_HEIGHT` (58 m on `main`).

**First merge — conflicts and decisions:**

- `world.rs` (camera): kept `smoother-play`'s higher camera (58 m), stage zoom
  and mouse-wheel zoom, and added the boss pull-back and focus shift from
  `worktree-bosses`. The boss framing in `boss.rs` assumed the old 40 m
  camera; it now divides by `world::CAMERA_HEIGHT`. Dropped the bosses
  branch's own haze scaling, since `smoother-play` already scales the haze
  with camera height.
- `enemy.rs`: boss AI moved to `boss_ai.rs` (bosses branch). Troops keep the
  several looks, walk cycle, archer pose and corpses (enemy-visuals).
- `combat.rs`: `enemy_deaths` hands bosses to `boss::begin_wreck` (bosses
  branch) and still leaves a corpse for each troop (enemy-visuals). Coins
  go through the new `drop_coins`.
- `assets.rs`: troop looks list + archer aiming mesh, and boss rigs.
- `debug.rs`: kept both sets of test switches.
- `TODO.md`: both checklists, one after the other.

## 4. graphics-polish

**What it does:** new effects in `vfx.rs` (soft smoke, dust and fire sprites
painted at startup, flash lights, shockwave rings, sparks, embers, muzzle
blasts, shot trails, fading blood and scorch decals, wheel ruts), a new HUD
(leather-and-bronze panels, glossy bars with a damage trail, cooldown sweep,
level ring, rising cards, red flush when hit) and `anim.rs` (castle lean,
squat and rock, wheels, flags, weapon aim and recoil, ballista reload, ram
swing, troop and boss animation, corpses and wrecks).

**Incomplete (from its TODO):** never played or screenshotted; one compiler
warning (unused old effect materials); "turn effects down on Low" waiting
for the graphics presets; README screenshots.

**Review and test (on the branch):** builds; clippy only repeats the unused
materials warning. Two runs (level 1 and level 10, with a boss and a card
screen): no crash, 17-29 FPS on this branch (old shadows and SSAO still on).
Screenshots showed:
- the banner's drop shadow never faded, so "Defend the realm!" and "The Siege
  Tower approaches!" stayed on screen as dark text;
- the XP numbers were pale text on the pale gold fill and hard to read;
- cards, bars, ability slots, smoke puffs and ruts looked right.

**Finished / polished:**
- On the branch: banner shadows fade with the text; XP numbers sit on a
  small dark pill; unused materials removed (no warnings left); checklist
  updated.
- On `main` (needs the presets from smoother-play): effects scale with the
  preset. High keeps 3200 effects / 10 flash lights / 160 bodies, Medium
  2200 / 6 / 110, Low 1200 / 3 / 60 and also skips 40% of purely cosmetic
  smoke and sparks. README preset table updated. Test: Low 49-58 FPS,
  High 37-50 FPS with the Juggernaut on screen.

**Performance:** the HUD only rewrites text when it changes, effects are
capped, flashes have no shadows. The big cost of effects is overdraw from
large see-through sprites, which the per-preset limits address. Its weapon
animation is replaced by the weapons branch (section 5), so it was not tuned.

**Second merge:** one conflict in `assets.rs`: the branch deleted `fire`,
which `main` still needs for the bosses' fire pots, next to `main`'s
`charred` (cart wrecks). Kept both.

**First merge — conflicts and decisions:**

- `combat.rs`: the effects code moved to `vfx.rs`. The bosses branch's new
  effects were ported there: `Zone` (red disc under a warning ring) and
  `Lane` (red strip for charges). `vfx.rs` already had a `Shockwave`; it is a
  ground decal, so it now stays on the ground when bosses spawn it higher up.
- `models.rs`: troops from enemy-visuals; the old boss models (with
  graphics-polish's moving pieces) stay deleted as on the bosses branch.
  Removed the boss-only piece roles (`Spin`, `Ramp`, `Arm`).
- `anim.rs`: kept castle sway, wheels, flags, weapon aiming/recoil, shot
  trails and the drop of straight shots. Removed troop animation, troop
  corpses, boss animation and boss wrecks (covered by the branches above).
- `enemy.rs`: graphics-polish's edits to the old boss AI were dropped with it.
- `assets.rs`: removed `boss_parts`; removed old effect materials nothing
  used any more (this also clears graphics-polish's compiler warning).
- `modules.rs`, `README.md`: kept the new shot height; README keeps the
  no-shadows lighting line plus the new effects, UI and "How it moves".
- Found while testing: the banner's drop shadow (added by graphics-polish)
  never faded, so "Defend the realm!" stayed on screen as dark text. Fixed
  in `hud.rs`.

## 5. worktree-weapon-mounts

Found while listing the branches: a parallel session committed and pushed it
during this work (it was still at the old `main` when the first list was
made). No session was running on it any more when it was picked up.

**What it does:** rebuilds all six weapons as rigs, a fixed mount plus
moving parts (`weapon_models.rs`): wider and lower, with chamfered timbers,
iron straps, rivets, turned bronze barrels, ropes and chains. `weapons.rs`
animates them from `WeaponFired` messages sent where the guns really fire:
the ballista turns, looses and is winched back, the bombard turns, elevates
and kicks, the cannons recoil and run out, the archers turn and loose, the
ram swings on its chains, the mine chute feeds one mine at a time. New stone
gun platforms on the side walls (stages 2 and 3), side weapons face
outward, cannons on braced platforms outside the walls. Test switches
`SIEGE_STAGE`, `SIEGE_VARIANT`, `SIEGE_LOADOUT`, `SIEGE_CAM`, `SIEGE_ARMORY`.

**Incomplete (from its TODO):** the last round of changes (platforms, mount
positions, braces, bronze tint, armory, close-up camera) was never looked
at; FPS with every weapon at stage 3 not measured; not merged.

**Review and test (on the branch):**
- Builds with no warnings; clippy: two over-indented doc lines.
- Armory with the close-up camera, one screenshot a second: the bombard
  turns and elevates between frames; all six weapons are there and fire.
- All six weapons on stages 1, 2 and 3 from the front-right: platforms,
  outward-facing wall weapons, outside cannon platforms with braces, ram and
  chute flush with the castle; nothing floats or cuts into the wheels.
- Level-up card portraits frame each weapon well.
- Stage 3 with all weapons: 21-27 FPS on this branch (old shadows on; the
  GPU is the limit, not the weapons).
- One test run hung on a level-up card screen because it ran without the
  autopilot; that was the test, not the game. The test helper now has a hard
  time limit.

**Finished / polished (on the branch):** doc indent fixed; checklist
updated with the results.

**Performance:** weapon parts copied their pose into their transform every
frame even when it had not changed, which makes Bevy recompute each part and
its meshes. They now only write when the pose changes. On this branch the
difference was inside the run-to-run noise (20-25 FPS after, 21-27 before),
because the shadows dominate.

**Merge — conflicts and decisions:** graphics-polish and this branch both
animated the weapons. This branch's weapons were kept; graphics-polish keeps
the castle (wheels, flags, lean and sway).
- `player.rs`: the weapons mount inside graphics-polish's swaying castle
  body, with this branch's positions, facing and mirrored side guns.
- `assets.rs`: `castle_rigs` stay; `module_rigs` replaced by `rigs`. Both
  branches had types called `RigHandle` and `PartHandle`; the weapon ones are
  written with their path (`crate::weapons::...`).
- `models.rs` / `anim.rs`: graphics-polish's weapon models (`models::module`)
  and its weapon animation (`WeaponFire`, module owners, `Turret`, `Recoil`,
  `Archer`, `Ammo`, `Swing` roles, `MountOf`) removed; `anim.rs` now only
  moves the castle and shot trails.
- `debug.rs`: every switch from both sides kept. The close-up camera looked
  for "the" 3D camera; on `main` it now looks for `MainCamera`, because the
  scene is drawn by a second screen camera since smoother-play.
- `modules.rs`: kept the `WeaponFired` messages and graphics-polish's bolt
  height.
- `world.rs`: kept smoother-play's camera (real time, mouse zoom), made
  `follow_camera` and `PlayerZoom` public for the close-up camera's ordering.
- `main.rs`: all new modules; `WeaponsPlugin` moved to the second plugin
  group (Bevy allows 15 per group).
- graphics-polish's muzzle blast had no user left, so the cannons now fire
  with it (flash, light, smoke, sparks) instead of plain smoke.
- README: weapons description, new files, new test switches.

**After merging (on `main`):** builds with no warnings or clippy findings.
Armory, stage 3 with all weapons, and 30 s of normal play at level 12: no
crash; 41-61 FPS in normal play. The Siege Tower's new crew was checked from
above with the close-up camera.

## README screenshots

Retaken on `main` after all merges (asked for by the bosses and
graphics-polish checklists): the Iron Citadel against the Warlord, a
level-up card screen, and a busy endless-mode moment with the Siege Tower.
The README caption said "Grand Citadel", which is not a stage name; it now
says "Iron Citadel".

## Changes asked for after the merges

- **One boss at a time.** The boss countdown now waits while a boss is on
  the field and starts again once it has fallen, in endless mode too, and
  the HUD hides the countdown during a fight. (With the longer fights from
  the bosses step, the next boss could otherwise arrive before the last one
  fell.) The War Wagon, which came as a pair, now comes alone with double
  health (4800) and goes berserk below 40% health instead of when its
  partner dies. Checked in a sped-up run: the Siege Tower fell at ~125 s and
  the War Wagon arrived 150 s after that, alone.
- **Health bars above bosses.** A bar floats above every boss, including the
  building-like ones (Siege Tower, Trebuchet Fortress, Warlord's Citadel).
  Troops and sheep have none. It is part of the HUD (sharp at any zoom),
  stays below the top bars, hides when the boss is off screen and is removed
  when the boss falls.
