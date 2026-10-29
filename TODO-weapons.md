# Weapons rework: things to check before merging

Branch: `worktree-weapon-mounts`

## What changed

- All six weapons rebuilt in `src/weapon_models.rs`: wider and shorter, with
  chamfered timbers, iron straps, rivets, turned bronze barrels, ropes and chains.
- Weapons are now rigs: a fixed mount plus moving parts, animated in `src/weapons.rs`:
  - **Ballista**: turret turns to the target, arms snap forward, the claw slides up,
    catches the string and the windlass winches it back, then a new bolt drops in.
  - **Bronze Bombard**: turns to the target, raises the barrel for near targets,
    kicks on each shot.
  - **Bronze Cannons**: carriage recoils and is run out again, wheels roll,
    breeching ropes stretch, smoke from the muzzle.
  - **Archer Loft**: archers turn to the target and lean back as they loose.
  - **Battering Ram**: log swings on its chains when the castle speeds up or brakes,
    big swings during Charge.
  - **Mine Chute**: star wheel turns, a mine slides down the chute and drops,
    the flap swings open.
- Mounting (`src/art.rs`, `src/models.rs`, `src/player.rs`):
  - New stone gun platforms on corbels on the side walls (stages 2 and 3), so the
    side-wall weapons no longer hang off a thin wall. Weapons there face outward.
  - Side cannons moved out of the curtain walls onto braced platforms outside them.
  - Keep-roof slot (stage 1) moved off the chimney; stage 3 front slot centered.
- Test helpers in `src/debug.rs`: `SIEGE_STAGE`, `SIEGE_VARIANT`, `SIEGE_LOADOUT`,
  `SIEGE_CAM` (close-up camera) and `SIEGE_ARMORY` (all weapons on the grass, firing
  on a loop).

## Status

- Release build compiles with no warnings.
- The first round of the new weapons was checked in-game (screenshots).
- Looked at in-game on October 30, 2026 (Iris Xe): armory mode with the close-up
  camera (bombard turns and elevates, all six weapons fire on a loop), all six
  weapons on stages 1-3 from the front-right, and a level-up card screen. The wall
  platforms, outward-facing side weapons, outside cannon platforms with braces and
  the ram and chute all sit right; portraits frame each weapon. No crash.
- Stage 3 with all weapons: 20-27 FPS on this branch, which still has the old
  shadows (the GPU is the limit, so the weapons are not). Weapon parts now only
  write their transform when it changes.
- Clippy: two over-indented doc lines fixed; no other findings.

## How to look

```sh
# All six weapons on the grass, firing on a loop (camera: zoom, turn, tilt).
WGPU_BACKEND=dx12 SIEGE_ARMORY=1 SIEGE_GOD=1 SIEGE_CAM=0.35,180,30 target/release/siegecastle.exe

# A castle stage with chosen weapons, seen from the front-right.
WGPU_BACKEND=dx12 SIEGE_GOD=1 SIEGE_STAGE=3 SIEGE_LOADOUT=ballista,mortar,loft,ram,mines,cannons \
  SIEGE_CAM=0.55,135,40 target/release/siegecastle.exe
```

`SIEGE_CAM` turn: 0 = behind the castle, 90 = its right side, 180 = the front.
The close-up camera only starts working about 10 seconds in, once the card-portrait
cameras are gone (the normal follow camera has the same delay on `main`).

## Checklist

### Animation (use `SIEGE_ARMORY=1`)
- [ ] Ballista: both string halves meet at the claw in every phase; no jump when the
      claw catches the string; windlass turns back while the claw goes forward, then
      winds; the bolt only shows once the string is drawn back.
- [ ] Ballista: the turret follows enemies smoothly and points where bolts fly.
- [ ] Bombard: the barrel does not sink into the bed cheeks at its steepest angle
      plus the firing kick.
- [ ] Cannons: the carriage rolls back (away from the muzzle) and the wheels turn the
      right way; the left-wall copy only fires when the left side fires.
- [ ] Cannon muzzle smoke appears at the muzzle, also on the mirrored left gun.
- [ ] Archers: they don't swing through the roof posts or back wall when they turn far.
- [ ] Ram: the chains stay attached to the log through the whole swing; the swing
      looks right when braking hard and during Charge (Q/E/R/Space).
- [ ] Mine chute: one quarter turn of the star wheel per mine; the mine slides down
      and falls off the end; the flap opens as it passes.

### Mounting, per castle stage (use `SIEGE_STAGE` + `SIEGE_LOADOUT`)
- [ ] Stage 1: the keep-roof weapon is clear of the chimney and battlements.
- [ ] Stage 1: the second top slot (deck behind the keep) is clear of both towers.
- [x] Stage 2 and 3: the new stone wall platforms look right (corbels, lip, inner
      arch) and the weapons on them face outward.
- [ ] Stage 2 and 3: the ballista arms don't cut into the hall roof eaves when it
      aims inward from a side-wall platform.
- [ ] Stage 3: the front slot is clear of the two front round towers. The Archer
      Loft is the widest weapon, so check it there.
- [x] All stages: the cannon platforms sit outside the walls, the braces reach the
      wall or deck, and nothing floats or cuts into the wheels.
- [x] Ram housing and mine chute sit flush against the front and back on every stage.

### Look and performance
- [x] Bronze looks like aged bronze, not flat yellow.
- [x] Card portraits (level-up cards, ability slots) frame each weapon well
      (`src/previews.rs`).
- [x] FPS with all weapons at stage 3: every moving part is its own entity now.

### Merging into `main`
- [ ] `main` has uncommitted DX12 / graphics changes that are not in this branch.
      Expect conflicts in `src/main.rs` (module list and plugins), `src/debug.rs`,
      and `src/modules.rs` (`NotShadowCaster` removed on `main`).
- [ ] `dxcompiler.dll` and `dxil.dll` were copied in for local runs; keep them out of git.
