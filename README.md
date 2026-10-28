# SiegeCastle

A fast medieval action roguelike made with **Rust + Bevy 0.20**.

You drive a castle on wheels across one big battlefield. Its towers fire on their own, enemies keep coming in bigger and bigger waves, and your castle grows through 3 stages as you collect XP. Survive 10 waves and destroy the Warlord's citadel to win.

## Run it

You need [Rust](https://rustup.rs) 1.97.1 or newer.

```bash
cargo run --release
```

The first build takes a few minutes because it compiles Bevy. Later builds are quick.

On Linux, install these first (Ubuntu/Debian):

```bash
sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-0
```

## Controls

| Key | Action |
|---|---|
| WASD / Arrow keys | Drive the castle |
| Space | Cannon barrage (ring of cannonballs, 8s cooldown) |
| P / Esc | Pause |
| R | Restart (after game over, victory, or while paused) |
| F12 | Save a screenshot to `screenshots/` |

## How it plays

**Your castle grows in 3 stages.** Kill enemies to drop XP gems, and drive over villages for XP plus some wall repair. Each new stage fully repairs the castle, adds towers, makes it bigger, and zooms the camera out. Growing also sends out a shockwave that hits nearby enemies.

| Stage | Walls | Towers | XP needed |
|---|---|---|---|
| 1. Wooden Keep | 250 | 3 archers | start |
| 2. Stone Fort | 600 | 2 archers, 2 cannons | 100 |
| 3. Grand Citadel | 1400 | 4 archers, 2 cannons, 2 mage towers | 650 |

**Towers**
- **Archer**: fast arrows, single target
- **Cannon**: slow, shells arc over enemies and explode over a whole area
- **Mage**: magic bolts that pass through up to 5 enemies

**Enemies come in 10 waves, and the map fills up.** Each wave lasts 40 seconds with a short break between. Enemies that survive stay on the map, so the swarm keeps growing. Each wave spawns faster and gives enemies more health. Groups attack from one direction at a time, and big hordes rush in from a single side.

| Enemy | Shows up | Notes |
|---|---|---|
| Raider (red circle) | wave 1 | fast, weak |
| Knight (grey square) | wave 2 | armored |
| Battering Ram (brown) | wave 4 | slow, very tough, hits walls hard |
| Rival Castle (boss) | waves 5 and 8 | shoots back, drops lots of XP |
| Warlord's Citadel (final boss) | wave 10 | destroy it to win |

A red arrow next to your castle points to the nearest boss. Cannon and mage towers aim at bosses first: cannon shells arc over the crowd, and magic bolts aimed at a boss pass through the swarm.

## Why it is fast

- **ECS (Entity Component System):** Bevy stores game data in a cache-friendly layout and runs systems in parallel.
- **Parallel swarm movement:** every enemy's movement and crowding runs across all CPU cores (`par_iter_mut`).
- **Spatial grid:** enemies are put in 64px cells every frame, so targeting, collisions and explosions only check nearby cells instead of every enemy.
- **Shared meshes and materials:** all shapes reuse a few handles, so Bevy draws thousands of enemies in a handful of batched draw calls.
- **Hard caps:** at most 3500 enemies and 1200 gems on the ground. Extra XP goes straight to the castle.
- Release builds use thin LTO and `opt-level = 3`. Debug builds still optimize Bevy itself, so `cargo run` stays playable.

## Code layout

```
src/
  main.rs    app setup, game states, frame ordering
  assets.rs  shared meshes and colors
  castle.rs  player castle: movement, 3 stages, towers, barrage
  enemy.rs   enemy types, swarm movement, boss attacks
  combat.rs  projectiles, explosions, deaths, XP gems, particles
  waves.rs   wave director: escalating spawns, hordes, bosses
  grid.rs    spatial grid for fast "who is near me?" checks
  world.rs   map, trees, villages, follow camera
  hud.rs     health/XP bars, wave info, banners, end screens
  debug.rs   autopilot and screenshot helpers for testing
```

## Testing helpers

These environment variables are all off by default:

```bash
SIEGE_AUTOPILOT=1 SIEGE_LOG=1 SIEGE_SHOTS=30,120 SIEGE_EXIT_AFTER=150 cargo run --release
```

- `SIEGE_AUTOPILOT=1`: the castle drives itself in a loop and fires barrages
- `SIEGE_SHOTS=30,120`: save screenshots at these run times (seconds)
- `SIEGE_SHOT_DIR=path`: where screenshots go
- `SIEGE_EXIT_AFTER=150`: quit after this many seconds
- `SIEGE_SPEED=2`: run the game clock faster
- `SIEGE_LOG=1`: print a status line every 5 seconds
- `SIEGE_START_WAVE=10`: jump straight to a wave (castle starts fully grown)
- `SIEGE_GOD=1`: castle cannot be destroyed

Inspired by the "castle on wheels" idea from Wanderburg. All code and art in this project are original.
