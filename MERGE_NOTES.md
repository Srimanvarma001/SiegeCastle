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

Fast-forward, no conflicts.

## 2. worktree-enemy-visuals

Merged without conflicts on top of `smoother-play`. Compiles with no warnings.

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
