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
