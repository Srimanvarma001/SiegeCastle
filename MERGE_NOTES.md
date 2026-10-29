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

**Merge:** first merged as a fast-forward; the README fix merged cleanly.

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

Conflicts and decisions:

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

Conflicts and decisions:

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
