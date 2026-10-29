# SiegeCastle

A fast 3D medieval action roguelike made with **Rust + Bevy 0.20**, with a realistic look: a gothic castle of cut stone and slate spires, textured materials and atmospheric light.

Drive a castle on wheels across the countryside, plow through marching armies, and bolt new weapons onto your walls as you level up. Your castle grows through 3 stages. Beat 5 bosses, then survive the endless siege for as long as you can, as enemies grow exponentially in numbers and strength.

![Gameplay](docs/gameplay.png)

| Level-up cards | Grand Citadel vs. the Warlord |
|---|---|
| ![Cards](docs/cards.png) | ![Bosses](docs/bosses.png) |

## Run it

You need [Rust](https://rustup.rs) 1.97.1 or newer.

```bash
cargo run --release
```

The first build takes a few minutes because it compiles Bevy. Later builds are quick.

On Windows you can also double-click `play.bat`: it builds the game the first time, then starts it from the project folder so it finds `assets/` and the DirectX files below.

On Windows the game uses DirectX 12. Put `dxcompiler.dll` and `dxil.dll` in the project folder (or next to the `.exe`). They come with the Windows SDK, in `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64\`. Without them DirectX falls back to an old shader compiler, and the world can stay empty for minutes after launch. Set `WGPU_BACKEND=vulkan` to use Vulkan instead.

If the first build crashes with an out-of-memory error, build with fewer parallel jobs: `cargo build --release -j 4`.

On Linux, install these first (Ubuntu/Debian):

```bash
sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-0
```

## Controls

| Key | Action |
|---|---|
| WASD / Arrow keys | Steer. The castle turns toward where you steer and rolls forward like a vehicle |
| Shift | Nitro boost (refills over time) |
| Q, E, R, Space | Use the ability of the module in slot 1, 2, 3, 4 |
| 1 / 2 / 3 or click | Pick a level-up card |
| T | Reroll the cards (2 per run) |
| P / Esc | Pause |
| R | Restart (after game over or while paused) |
| Mouse wheel, or - and = | Zoom the camera out and in |
| F9 | Switch graphics quality: Low, Medium, High |
| F12 | Save a screenshot to `screenshots/` |

## How it plays

**XP and levels.** Enemies drop gold coins. Driving over farms gives a lot of XP and repairs your walls, and scooping up sheep gives a little. Every level-up opens a card choice: even levels offer weapon modules, odd levels offer artifacts.

**The castle grows in 3 stages.** At level 6 and level 14 you choose how it grows:

| Stage | Defense choice | Movement choice | Module slots |
|---|---|---|---|
| 1 | Wagon Keep | | 2 |
| 2 (level 6) | Bulwark Fort: tough, heavy, big magnet | Outrider Fort: fast, more nitro | 3 |
| 3 (level 14) | Iron Citadel | Thunder Citadel | 4 |

Growing fully repairs the castle and blasts nearby enemies away.

**Weapon modules** (6). Each has an automatic attack plus an ability on its key. Picking one again levels it up, to a maximum of level 5.

| Module | Slot | Automatic | Ability |
|---|---|---|---|
| Archer Loft | Top | Archers on a covered hoarding fire arcing volleys | Arrow Storm: arrows rain all around |
| Ballista | Top | Heavy bolts that skewer whole ranks at long range | Bolt Storm: a fan of bolts ahead |
| Bronze Bombard | Top | Explosive shells into crowds | Carpet Shelling: a line of shells behind you |
| Battering Ram | Front | Driving into enemies hurts them, more at speed | Charge: speed burst with triple ram damage |
| Mine Chute | Back | Drops mines as you drive | Minefield: a dozen mines at once |
| Bronze Cannons | Sides | Piercing cannonballs to both sides | Full Broadside: a wide volley each side |

**Artifacts** (stack up to 5 times): Whetstone (damage), Greased Axles (speed), Lodestone (coin magnet), Reinforced Hull (health), Quick Hands (attack speed), Black Powder (explosion size), Hawk Eye (range), Tinker's Kit (ability cooldowns), Herbal Poultice (repair over time), Golden Fleece (XP), Nitro Tank (nitro), Iron Spikes (enemies touching your walls take damage).

**Enemies** march in formations and swarm you up close: footmen, archers who shoot back, knights on horseback, and spiked war carts.

**Bosses** arrive on a countdown (shown under the XP bar). They are colossal, many times the size of your castle: the camera pulls back when one is near, and a red arrow points to it when it is out of sight. You cannot drive through them, but ramming still hurts them. Every attack is announced first: red circles where shots will land, red lanes where a charge or a broadside will go. Each boss gets more dangerous as it loses health, and collapses in its own way when destroyed.

| Boss | Size | Attacks | When hurt |
|---|---|---|---|
| **The Siege Tower** | 18 m wide, 40 m tall | Lowers its ramp and sends out squads. Archers on top rain arrows. Pours burning oil all around its base if you hug it. | Sends bigger squads, with knights |
| **Twin War Wagons** | 26 m long each | Flank you from opposite sides. The turret bombard tracks you. Gun ports open for a broadside to one side. | The survivor goes berserk and charges with its scythed wheels |
| **The Iron Juggernaut** | 40 m long | Charges down a red lane, flattening its own troops. Swings its ram at anything in front. Blasts steam if you sit beside it. | Charges twice in a row, leaving burning oil behind |
| **The Trebuchet Fortress** | 34 m wide, arm reaches 48 m | Keeps its distance. The arm swings and hurls a huge boulder, then treadwheels wind it back. Lobs fire pots. Ballistas on its towers guard it up close. | Each boulder splits into five |
| **The Warlord's Citadel** | 64 m long, 76 m tall | Cannon turrets on four towers. The portcullis rises and knights ride out. | Great bombard fires a cross of shells and the citadel tramples forward; at last, fire rains from its burning spire |

**Endless siege.** After the Warlord falls there is no more countdown to a finish. Every 30 seconds enemies get about 22% more health, 10% more damage, and spawn 15% faster in bigger groups, and random bosses keep returning. The run ends when your castle falls.

## How it looks

Everything is generated in code, with no art files to download:

- **Gothic castle:** round towers with corbelled parapets and steep slate spires, a great hall with buttresses, lancet windows and a rose window, curtain walls with battlements, a gatehouse with a portcullis, and a clock tower at stage 3. It all rides on a timber-and-iron siege platform.
- **Realistic materials:** cut limestone blocks with mortar, overlapping slate shingles with moss, weathered planks, forged iron with rust, meadow grass, mud, wheat and plaster. Each surface has color, relief (normal map) and roughness/metal maps, tiles seamlessly, and has mipmaps for crisp distance views.
- **Countryside:** trees, firs and bushes built from leaf, needle and grass cards with a generated foliage texture; grass tufts with wildflowers; winding dirt roads with wheel ruts; farmsteads with timber-framed houses and wheat fields; ponds and a river.
- **Weapons:** a bronze cannon on a spoked-wheel carriage, a torsion ballista, archers drawing longbows on a covered hoarding, a bronze mortar, and a covered battering ram.
- **Cards:** velvet-and-gold level-up cards (crimson for weapons, sapphire for artifacts, emerald for castle growth) showing real 3D renders of each item, made in an off-screen studio at startup.
- **Lighting:** a warm late-afternoon sun, cool sky light, distance haze, bloom on fire, and SMAA anti-aliasing. There are no shadows, which keeps it fast on laptop graphics.
- **Effects:** soft smoke, dust and fire sprites painted at startup, flashes of light that briefly light up the ground, shockwave rings, sparks and embers, muzzle blasts, smoke trails behind cannonballs and shells, blood splatter and scorch marks that fade away, and wheel ruts behind the castle.
- **UI:** Cinzel titles and EB Garamond text (both under the SIL Open Font License, see `assets/fonts/`). Leather-and-bronze panels, glossy bars that leave a pale trail when you take damage, a clock-hand sweep on ability cooldowns, a level ring that pops on level-up, cards that rise into place and lift when hovered, and screen edges that flush red when the walls are hit.

## How it moves

- **Castle:** wheels roll with your speed, the body leans into turns, squats when you speed up, rocks and sheds stone chips when hit, and kicks up dust. Flags wave.
- **Weapons:** the ballista, bombard and archers turn to track their target. Barrels slam back when they fire, the ballista bolt vanishes and is winched back in, and the ram log swings as you speed up and brake, then pounds back and forth while charging.
- **Troops:** march with a stride of their own, horses gallop, carts rumble, soldiers lunge and hack at your walls, archers stop and draw their bows once in range, and everyone squashes when hit. The fallen topple over and sink away; war carts are left as charred wrecks.
- **Bosses:** wheels, turrets, ramps, gun ports, rams and the trebuchet arm all move with each attack (see the boss table above), and each boss collapses in its own way when destroyed.

## Why it is fast

- **ECS (Entity Component System):** Bevy stores game data in a cache-friendly layout and runs systems in parallel.
- **Parallel troop movement:** every enemy's movement, formation keeping and crowding runs across all CPU cores.
- **Spatial grid:** enemies are sorted into small cells every frame, so targeting, collisions and explosions only check nearby cells.
- **One mesh per unit:** each model is many colored shapes merged into a single mesh, and nearly everything shares one material, so thousands of units draw in a few batched calls. Static scenery is merged into a few big chunks.
- **Caps:** at most 2200 troops and 900 coins at once. In endless mode enemies keep getting stronger past the cap, and extra XP goes straight to you.
- **Graphics presets:** the game picks Low, Medium or High from your GPU (F9 switches). Medium and Low draw the 3D world at 80% or 67% of the window resolution and stretch it to fit, while the HUD stays at full sharpness. Integrated graphics start on Medium.

| Preset | 3D resolution | Anti-aliasing | Bloom | Effects |
|---|---|---|---|---|
| High | 100% | SMAA high | on | up to 3200 effects, 10 flash lights, 160 fallen troops |
| Medium | 80% | SMAA medium | on | up to 2200 effects, 6 flash lights, 110 fallen troops |
| Low | 67% | SMAA low | off | up to 1200 effects (some smoke and sparks left out), 3 flash lights, 60 fallen troops |

## Code layout

```
src/
  main.rs         app setup, game states, frame ordering
  art.rs          coordinates, castle mount points, terrain meshes
  models.rs       every 3D model (castle, troops, bosses, farms, trees)
  texgen.rs       realistic stone/slate/wood/iron/grass textures, made in code
  icons.rs        fallback UI icons
  previews.rs     3D portraits of weapons, artifacts and castles for the cards
  studio.rs       close-up troop screenshots for testing (`SIEGE_TROOPS=1`)
  assets.rs       builds models and materials once at startup
  items.rs        modules, artifacts, castle stages, stat math
  player.rs       castle driving, nitro, health, looks, growth
  modules.rs      weapon auto-attacks and abilities
  enemy.rs        troops, formations, walk cycles, corpses
  boss.rs         boss rigs, animation, camera framing, burning ground, collapse
  boss_ai.rs      boss attacks and phases
  boss_models.rs  the five colossal boss models
  director.rs     spawning, boss countdown, endless scaling
  combat.rs       shots, explosions, mines, coins
  vfx.rs          smoke, fire, sparks, flashes, debris and ground marks
  anim.rs         moving parts of the castle and weapons, shot trails
  progression.rs  XP, levels, card choices
  world.rs        terrain, forests, water, farms, sheep, camera, sun
  graphics.rs     quality presets, render scale, the screen camera
  hud.rs          on-screen bars, slots, banners, end screens
  grid.rs         spatial grid for fast "who is near me?" checks
  debug.rs        autopilot and screenshot helpers for testing
```

## Testing helpers

All off by default. For example:

```bash
SIEGE_AUTOPILOT=1 SIEGE_LOG=1 SIEGE_SHOTS=30,120 SIEGE_EXIT_AFTER=150 cargo run --release
```

- `SIEGE_AUTOPILOT=1`: drives, uses abilities and picks cards by itself
- `SIEGE_SHOTS=30,120`: screenshots at these run times (seconds); `SIEGE_SHOT_CHOICE=1` also captures a card screen
- `SIEGE_SHOT_DIR=path`: where screenshots go
- `SIEGE_EXIT_AFTER=150`: quit after this many seconds
- `SIEGE_SPEED=2`: run the game clock faster
- `SIEGE_LOG=1`: print a status line every 5 seconds
- `SIEGE_START_LEVEL=8`: start with that many levels' worth of XP
- `SIEGE_BOSS_IN=10`: first boss after this many seconds
- `SIEGE_ENDLESS=1`: start in endless mode
- `SIEGE_GALLERY=1`: spawn all five bosses at once
- `SIEGE_TROOPS=1`: line up every troop look at several angles, save close-up, top and game-distance screenshots, then quit
- `SIEGE_GOD=1`: castle cannot be destroyed
- `SIEGE_LOWRES=1`: small window, for quick tests on machines without a GPU
- `SIEGE_QUALITY=low|medium|high`: force a graphics preset
- `SIEGE_GFX=scale=0.7,aa=low,bloom=0`: override single graphics settings
- `SIEGE_PROFILE=1`: with `SIEGE_LOG`, also print main-thread, render-thread and GPU pass times
- `SIEGE_VSYNC=0`: turn vsync off to see the uncapped frame rate

Inspired by the "castle on wheels" idea from Wanderburg and by gothic castle architecture. All code, models, textures, names and text in this project are original. Fonts: Cinzel and EB Garamond, SIL Open Font License 1.1.
