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

## Still to do

- [ ] Build again after the last cleanup (two unused helpers removed) and
      fix any errors or clippy findings
- [ ] Measure FPS with each boss on the Intel Iris Xe laptop. The one test
      run showed 2 to 3 FPS, but a build was using all CPU cores at the same
      time. If slow: turn off shadows on boss parts, merge static parts,
      lower wall and tower detail
- [ ] Check the camera pulls back in a normal game (it did not in the one
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
- [ ] Tune boss health, damage and cooldowns so each fight lasts about a
      minute or two
- [ ] Check warning circles and lanes are easy to see on grass and roads
- [ ] Merge with the uncommitted graphics work on `main`. Expect conflicts in
      `src/world.rs` (camera and haze), `src/enemy.rs` and `src/main.rs`
- [ ] New `docs/bosses.png` screenshot for the README
