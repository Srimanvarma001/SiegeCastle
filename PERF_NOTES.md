# Performance notes

Profiling of `main` after all branches were merged (October 30, 2026).

**Machine:** Intel Core i5-1240P laptop, Intel Iris Xe graphics (DirectX 12),
Windows 11, "Balanced" power plan. The game picks its **Medium** preset here
(80% render scale, SMAA medium, bloom). The final measurements ran **on
battery**, which makes everything slower than on mains power, so compare
before and after with each other, not with numbers taken earlier in the day.

## How it was measured

- Release builds (`cargo build --release`, thin LTO) of `main` before the
  performance work (`0d33c65`) and after it.
- `bench.py` (in the session scratchpad) alternates the two builds run by
  run (before, after, before, after...) so both see the same battery and heat
  state, and averages:
  - **load:** seconds from launching the exe to the first frame of play
    (the `t=0 Starting` log line), 3 runs each;
  - **bosses:** all five bosses at once (`SIEGE_GALLERY=1`), stage-3 castle
    with all six weapons, 40 s, 2 runs each;
  - **crowd:** endless mode at 3x speed for 150 s (up to ~770 troops);
  - **normal:** an ordinary start, 45 s, 2 runs each.
- All runs: autopilot, god mode, vsync off (`SIEGE_VSYNC=0`) so the frame
  rate is not capped at 60, and `SIEGE_PROFILE=1`, which prints main-thread
  and render-thread times.
- To find the causes: the asset and world loading now log how long each step
  takes, `SIEGE_PROFILE` prints mesh and effect counts (visible meshes
  grouped by what they belong to), and a build with Bevy's `trace_chrome`
  feature recorded every system for 20 s (summarised with `trace_top.py`).

## Results

Release builds, alternating runs on battery (`bench3.log`). FPS is the mean
of the 5-second samples from 10 s on; "prepare" and "submit" are the
render thread's two phases per frame.

| Scenario | Build | Load (s) | FPS mean | FPS min | Main thread | Render prepare | Render submit |
|---|---|---|---|---|---|---|---|
| Load (3 runs) | before | **3.8** (4.7, 3.0, 3.8) | | | | | |
| | after | **1.9** (2.8, 1.2, 1.6) | | | | | |
| Normal play (3 runs) | before | 2.8 | 53.9 | 37 | 3.5 ms | 9.0 ms | 9.2 ms |
| | after | 1.3 | **60.3** | **48** | 3.4 ms | 8.4 ms | 8.3 ms |
| Crowd, up to ~900 troops (1 run) | before | 2.8 | 49.6 | 36 | 3.8 ms | 10.5 ms | 9.6 ms |
| | after | 1.0 | 51.7 | 32 | 3.8 ms | 9.7 ms | 8.9 ms |
| Five bosses at once (3 runs) | before | 4.0 | 22.3 | 15 | 7.3 ms | 23.6 ms | 22.8 ms |
| | after | 2.2 | 23.1 | 13 | 6.5 ms | 24.3 ms | 22.4 ms |

In short:
- **Load time halved or better**: 3.8 s -> 1.9 s to the first frame of play
  (an earlier set of runs, with the laptop more throttled, gave 5.0 s ->
  1.7 s). Asset creation alone: ~2.0 s -> ~0.5 s; world: ~0.49 s -> ~0.26 s.
- **Normal play ~12% faster, and the worst moments ~30% better** (lowest
  sample 37 -> 48 FPS); visible meshes ~720 -> ~150-200.
- **Five-bosses-at-once is unchanged** (~22 FPS). That scene is limited by
  the sheer number of boss parts (see "What is left"). In the real game only
  one boss is now on the field at a time (a change asked for separately).
- One "after" bosses run ended early: that was a crash in the new boss health
  bars when the window was minimized, found by this benchmark and fixed
  afterwards (see MERGE_NOTES.md). The fix does not change performance.

A note on an earlier claim: the commit that fixed the screen camera says
"uncapped FPS ~25 -> ~62". Those were two runs minutes apart while the laptop
changed power state, not a fair comparison. The alternating runs above are
the numbers to trust.

## What was slow, and what changed

### Load time

The new `startup:` log line showed asset creation took about 2.0 s, of which
**1.8 s was textures**, and world generation another 0.5 s.

1. **Foliage atlas: 1.4 s.** Every leaf, needle and grass blade was painted by
   testing every pixel of a square as wide as the leaf is long. Grass blades
   are up to 440 px long but only 6-11 px wide, so each blade tested ~780 000
   pixels to colour a few thousand. The painter now scans a tight box around
   the leaf. The image is the same: checked against the old painter on 2000
   random leaves (the only "difference" was a NaN that both produced; see 3).
2. **All generated textures in parallel.** The nine surfaces (stone, slate,
   wood, iron, grass, mud, wheat, plaster, bark), the foliage atlas and the
   road are each built on their own thread, then handed to Bevy.
   Textures: 1.83 s -> ~0.24 s.
3. **Bug found on the way:** at the very tip of a leaf `sin(pi)` is a hair
   below zero, and a negative number to the power 0.8 is NaN, which ended up
   as black pixels in the foliage texture. Fixed.
4. **Scenery in parallel.** The trees, firs, bushes, rocks and 9000 grass
   tufts are still placed one after another with the shared random numbers
   (so the world is the same), but each of the 72 map-chunk meshes is then
   built on its own thread. World: ~485 ms -> ~260 ms.

### Frame rate

The profile showed the main thread spends ~3-4 ms per frame on the game and
then waits; the **render thread** was the limit (~18 ms per frame in the trace,
~20 ms "prepare" plus ~20 ms "submit" on battery). The trace and the mesh
census pointed at three things:

1. **The screen camera made the whole map "visible" (the big one).** Since the
   graphics presets, the 3D world is drawn into an image and a 2D camera
   draws that image plus the HUD. That 2D camera was on the default render
   layer, and a 2D camera's view box spans thousands of units, so every 3D
   mesh on the map counted as visible: 725 of 726 meshes, including all 42
   farms, all 112 sheep and every scenery chunk, although the game camera sees
   only ~60 x 40 m of a 440 m map. The renderer prepared, uploaded instance
   data for and submitted all of them every frame. The screen camera now has
   its own render layer (it only draws UI). Visible meshes in normal play:
   ~720 -> ~150-200; normal play +12% FPS, lowest FPS +30% (table above).
2. **Effects.** In boss fights up to 950 effect sprites were alive, more than
   half of all meshes. The largest group was shot trails (up to 560): one puff
   every 0.03 s behind every cannonball. Now one every 0.06 s, a bit larger
   (same look, half the entities). Castle wheel ruts are laid as 1.6 m
   segments instead of 0.8 m (half the decals). Effects at the boss-fight
   peak: 949 -> 714.
3. **Effects scale with the preset** (graphics-polish item): High keeps 3200
   effects / 10 flash lights / 160 bodies, Medium 2200 / 6 / 110, Low 1200 / 3
   / 60 and skips 40% of purely cosmetic smoke and sparks.

Smaller fixes made during the branch reviews, also in the "after" build:
boss and weapon parts only write their transform when it changes (static
parts no longer make Bevy recompute their meshes every frame).

## What is left

- With the culling fixed, the render thread is still the limit. Its biggest
  single costs in the trace were handing the frame to the driver
  (`queue_submit`, ~5 ms) and writing per-instance data for opaque meshes
  (~3.5 ms). Both shrink with fewer separate mesh entities: bosses and the
  castle are built from many small parts, each split per material. Merging
  the parts that never move would help most.
- Bloom costs ~0.9 ms of render-thread time and ~1-3 ms of GPU time per
  frame; Medium keeps it for the look. Turning it off on Medium is an easy
  extra ~5-10%.
- The first ~5 s of play run slower while DirectX compiles shaders and the
  card-portrait studio renders.
