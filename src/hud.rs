//! On-screen info: XP and level, boss timer and boss health, castle health and
//! nitro, ability slots with cooldowns, artifacts, banners and end screens.

use crate::art::{p2, Icon, Icons};
use crate::director::Director;
use crate::enemy::{Boss, Enemy};
use crate::items::{stage_info, Loadout, Stats};
use crate::modules::{Cooldowns, ABILITY_LABELS};
use crate::player::Castle;
use crate::progression::Progress;
use crate::{Banner, GameState, RunStats};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_hud.run_if(resource_added::<Icons>))
            .add_systems(Update, (update_bars, update_boss, update_slots, update_artifacts, update_info, update_banner, update_overlay));
    }
}

#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct NitroFill;
#[derive(Component)]
struct StageText;
#[derive(Component)]
struct BossTimer;
#[derive(Component)]
struct BossTimerText;
#[derive(Component)]
struct BossBar;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct Slot(usize);
#[derive(Component)]
struct SlotIcon(usize);
#[derive(Component)]
struct SlotCooldown(usize);
#[derive(Component)]
struct SlotCdText(usize);
#[derive(Component)]
struct SlotLevel(usize);
#[derive(Component)]
struct NitroSlotFill;
#[derive(Component)]
struct ArtifactColumn;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct BannerText;
#[derive(Component)]
struct BannerSub;
#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct OverlayText;

const FRAME: Color = Color::srgb(0.16, 0.13, 0.13);
const GOLD: Color = Color::srgb(0.98, 0.72, 0.22);
const SLOT_BG: Color = Color::srgb(0.20, 0.17, 0.20);

fn text(s: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont::from_font_size(size), TextColor(color))
}

fn round(px: f32) -> BorderRadius {
    BorderRadius::all(CornerRadius::circular(Val::Px(px)))
}

fn spawn_hud(mut commands: Commands, icons: Res<Icons>) {
    // XP bar and level (top center), boss timer under it.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        children![
            (
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() },
                children![
                    (
                        Node {
                            width: Val::Px(520.0),
                            height: Val::Px(22.0),
                            border: UiRect::all(Val::Px(3.0)),
                            border_radius: round(11.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.07, 0.07)),
                        BorderColor::all(FRAME),
                        children![
                            (
                                XpFill,
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(0.0),
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    border_radius: round(8.0),
                                    ..default()
                                },
                                BackgroundColor(GOLD),
                            ),
                            (XpText, text("", 13.0, Color::srgb(0.25, 0.15, 0.05))),
                        ],
                    ),
                    (
                        Node {
                            width: Val::Px(44.0),
                            height: Val::Px(44.0),
                            border: UiRect::all(Val::Px(3.0)),
                            border_radius: round(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.07, 0.06, 0.06)),
                        BorderColor::all(GOLD),
                        children![(LevelText, text("1", 20.0, GOLD))],
                    ),
                ],
            ),
            (
                BossTimer,
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
                children![
                    (ImageNode::new(icons.get(Icon::Skull)), Node { width: Val::Px(26.0), height: Val::Px(26.0), ..default() }),
                    (BossTimerText, text("", 22.0, Color::srgb(0.95, 0.25, 0.20))),
                ],
            ),
            (
                BossBar,
                Visibility::Hidden,
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(3.0), ..default() },
                children![
                    (BossName, text("", 18.0, Color::srgb(1.0, 0.85, 0.75))),
                    (
                        Node {
                            width: Val::Px(440.0),
                            height: Val::Px(14.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(7.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.05, 0.05)),
                        BorderColor::all(FRAME),
                        children![(
                            BossFill,
                            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: round(5.0), ..default() },
                            BackgroundColor(Color::srgb(0.80, 0.12, 0.10)),
                        )],
                    ),
                ],
            ),
        ],
    ));

    // Health and nitro (top left).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(8.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            border_radius: round(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        children![
            (StageText, text("", 15.0, Color::srgb(1.0, 0.86, 0.55))),
            (
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(22.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: round(6.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.10, 0.04, 0.04)),
                BorderColor::all(FRAME),
                children![
                    (
                        HpFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border_radius: round(4.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.85, 0.20, 0.18)),
                    ),
                    (HpText, text("", 14.0, Color::WHITE)),
                ],
            ),
            (
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(10.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: round(5.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.06, 0.12)),
                BorderColor::all(FRAME),
                children![(
                    NitroFill,
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: round(3.0), ..default() },
                    BackgroundColor(Color::srgb(0.30, 0.60, 1.0)),
                )],
            ),
        ],
    ));

    // Ability slots (bottom left): nitro + 4 module slots.
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                bottom: Val::Px(14.0),
                padding: UiRect::all(Val::Px(6.0)),
                column_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: round(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.10, 0.11)),
            BorderColor::all(FRAME),
        ))
        .id();
    let nitro = commands
        .spawn((
            slot_node(),
            BackgroundColor(SLOT_BG),
            children![
                (
                    NitroSlotFill,
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(0.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: round(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.3, 0.6, 1.0, 0.25)),
                ),
                (ImageNode::new(icons.get(Icon::Nitro)), Node { width: Val::Px(48.0), height: Val::Px(48.0), ..default() }),
                key_label("Shift"),
            ],
        ))
        .id();
    commands.entity(bar).add_child(nitro);
    for i in 0..4 {
        let slot = commands
            .spawn((
                Slot(i),
                slot_node(),
                BackgroundColor(SLOT_BG),
                children![
                    (SlotIcon(i), ImageNode::default(), Visibility::Hidden, Node { width: Val::Px(48.0), height: Val::Px(48.0), ..default() }),
                    (
                        SlotCooldown(i),
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(0.0),
                            left: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(0.0),
                            border_radius: round(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    ),
                    (SlotCdText(i), text("", 20.0, Color::WHITE)),
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-4.0),
                            bottom: Val::Px(-4.0),
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(10.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.07, 0.07)),
                        BorderColor::all(GOLD),
                        children![(SlotLevel(i), text("", 11.0, GOLD))],
                    ),
                    key_label(ABILITY_LABELS[i]),
                ],
            ))
            .id();
        commands.entity(bar).add_child(slot);
    }

    // Artifacts (left side, above the slots).
    commands.spawn((
        ArtifactColumn,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            bottom: Val::Px(100.0),
            flex_direction: FlexDirection::ColumnReverse,
            row_gap: Val::Px(4.0),
            ..default()
        },
    ));

    // Run info (top right).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(8.0)),
            border_radius: round(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        children![(InfoText, text("", 14.0, Color::WHITE), TextLayout::justify(Justify::Right))],
    ));

    // Banner.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(24.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(6.0),
            ..default()
        },
        children![
            (BannerText, text("", 40.0, Color::WHITE)),
            (BannerSub, text("", 20.0, Color::WHITE)),
        ],
    ));

    // Pause / game over.
    commands.spawn((
        Overlay,
        Visibility::Hidden,
        GlobalZIndex(5),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![(OverlayText, text("", 30.0, Color::WHITE), TextLayout::justify(Justify::Center))],
    ));
}

fn slot_node() -> Node {
    Node {
        width: Val::Px(64.0),
        height: Val::Px(64.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.0)),
        border_radius: round(10.0),
        ..default()
    }
}

fn key_label(k: &str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(-2.0),
            top: Val::Px(-8.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
            border_radius: round(4.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.30, 0.30, 0.34)),
        children![text(k.to_string(), 11.0, Color::WHITE)],
    )
}

/// Only touch the text when it really changes, so Bevy skips re-layout.
fn set(t: &mut Mut<Text>, s: String) {
    if t.0 != s {
        t.0 = s;
    }
}

fn fmt_time(t: f32) -> String {
    let s = t.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn update_bars(
    progress: Res<Progress>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    castle: Query<&Castle>,
    mut texts: ParamSet<(
        Query<&mut Text, With<XpText>>,
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<StageText>>,
    )>,
    mut fills: ParamSet<(
        Query<&mut Node, With<XpFill>>,
        Query<(&mut Node, &mut BackgroundColor), With<HpFill>>,
        Query<&mut Node, With<NitroFill>>,
        Query<&mut Node, With<NitroSlotFill>>,
    )>,
) {
    let Ok(c) = castle.single() else {
        return;
    };
    if let Ok(mut t) = texts.p0().single_mut() {
        set(&mut t, format!("{:.0} / {:.0}", progress.xp.floor(), progress.need));
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        set(&mut t, progress.level.to_string());
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        set(&mut t, format!("{:.0} / {:.0}", c.hp.max(0.0).ceil(), stats.max_hp));
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        set(&mut t, format!("Stage {}/3   {}", loadout.stage + 1, stage_info(loadout.stage, loadout.variant).name));
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent((progress.xp / progress.need).clamp(0.0, 1.0) * 100.0);
    }
    if let Ok((mut n, mut bg)) = fills.p1().single_mut() {
        n.width = Val::Percent((c.hp / stats.max_hp).clamp(0.0, 1.0) * 100.0);
        bg.0 = if c.hurt_flash > 0.0 { Color::srgb(1.0, 0.55, 0.45) } else { Color::srgb(0.85, 0.20, 0.18) };
    }
    let nf = (c.nitro / stats.nitro_max).clamp(0.0, 1.0) * 100.0;
    if let Ok(mut n) = fills.p2().single_mut() {
        n.width = Val::Percent(nf);
    }
    if let Ok(mut n) = fills.p3().single_mut() {
        n.height = Val::Percent(nf);
    }
}

fn update_boss(
    director: Res<Director>,
    castle: Query<&Transform, With<Castle>>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    mut timer: Query<&mut Visibility, (With<BossTimer>, Without<BossBar>)>,
    mut timer_text: Query<&mut Text, (With<BossTimerText>, Without<BossName>)>,
    mut bar: Query<&mut Visibility, (With<BossBar>, Without<BossTimer>)>,
    mut name: Query<&mut Text, (With<BossName>, Without<BossTimerText>)>,
    mut fill: Query<&mut Node, With<BossFill>>,
) {
    let (Ok(mut tv), Ok(mut tt), Ok(mut bv), Ok(mut bn), Ok(mut bf)) =
        (timer.single_mut(), timer_text.single_mut(), bar.single_mut(), name.single_mut(), fill.single_mut())
    else {
        return;
    };
    let cpos = castle.single().map(|t| p2(t.translation)).unwrap_or_default();
    let nearest = bosses
        .iter()
        .min_by(|a, b| p2(a.0.translation).distance(cpos).total_cmp(&p2(b.0.translation).distance(cpos)));

    match (director.endless, director.next_boss()) {
        (false, Some(_)) if director.boss_timer.is_finite() => {
            *tv = Visibility::Inherited;
            set(&mut tt, format!("BOSS IN {}", fmt_time(director.boss_timer)));
        }
        (true, _) => {
            *tv = Visibility::Inherited;
            set(&mut tt, format!("ENDLESS  {}   next boss {}", fmt_time(director.endless_time), fmt_time(director.boss_timer)));
        }
        _ => *tv = Visibility::Hidden,
    }
    match nearest {
        Some((_, en)) => {
            *bv = Visibility::Inherited;
            set(&mut bn, en.kind.name().to_uppercase());
            bf.width = Val::Percent((en.hp / en.max_hp).clamp(0.0, 1.0) * 100.0);
        }
        None => *bv = Visibility::Hidden,
    }
}

fn update_slots(
    loadout: Res<Loadout>,
    cds: Res<Cooldowns>,
    icons: Res<Icons>,
    mut slot_bg: Query<(&Slot, &mut BackgroundColor)>,
    mut slot_icons: Query<(&SlotIcon, &mut ImageNode, &mut Visibility)>,
    mut overlays: Query<(&SlotCooldown, &mut Node)>,
    mut cd_texts: Query<(&SlotCdText, &mut Text), Without<SlotLevel>>,
    mut levels: Query<(&SlotLevel, &mut Text), Without<SlotCdText>>,
) {
    for (s, mut bg) in &mut slot_bg {
        bg.0 = if s.0 < loadout.max_slots() { SLOT_BG } else { Color::srgb(0.08, 0.07, 0.08) };
    }
    for (s, mut img, mut vis) in &mut slot_icons {
        match loadout.modules.get(s.0) {
            Some(m) => {
                let h = icons.get(Icon::Module(m.kind));
                if img.image != h {
                    img.image = h;
                }
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for (s, mut node) in &mut overlays {
        let frac = loadout
            .modules
            .get(s.0)
            .and_then(|m| cds.0.get(&m.kind).map(|t| t.ability / m.kind.ability_cooldown()))
            .unwrap_or(0.0);
        node.height = Val::Percent(frac.clamp(0.0, 1.0) * 100.0);
    }
    for (s, mut t) in &mut cd_texts {
        let left = loadout.modules.get(s.0).and_then(|m| cds.0.get(&m.kind)).map(|t| t.ability).unwrap_or(0.0);
        set(&mut t, if left > 0.0 { format!("{}", left.ceil() as u32) } else { String::new() });
    }
    for (s, mut t) in &mut levels {
        set(&mut t, loadout.modules.get(s.0).map(|m| m.level.to_string()).unwrap_or_default());
    }
}

fn update_artifacts(
    mut commands: Commands,
    loadout: Res<Loadout>,
    icons: Res<Icons>,
    column: Query<Entity, With<ArtifactColumn>>,
) {
    if !loadout.is_changed() {
        return;
    }
    let Ok(col) = column.single() else {
        return;
    };
    commands.entity(col).despawn_related::<Children>();
    let mut list: Vec<_> = loadout.artifacts.iter().map(|(a, n)| (*a, *n)).collect();
    list.sort_by_key(|(a, _)| a.name());
    for (a, n) in list {
        let item = commands
            .spawn((
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() },
                children![
                    (ImageNode::new(icons.get(Icon::Artifact(a))), Node { width: Val::Px(30.0), height: Val::Px(30.0), ..default() }),
                    text(if n > 1 { format!("x{n}") } else { String::new() }, 13.0, Color::WHITE),
                ],
            ))
            .id();
        commands.entity(col).add_child(item);
    }
}

fn update_info(
    stats: Res<RunStats>,
    enemies: Query<(), With<Enemy>>,
    diagnostics: Res<DiagnosticsStore>,
    mut q: Query<&mut Text, With<InfoText>>,
) {
    let Ok(mut t) = q.single_mut() else {
        return;
    };
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    set(&mut t, format!(
        "{}\nKills {}\nEnemies {}\nBosses {}/5\nFPS {:.0}",
        fmt_time(stats.time),
        stats.kills,
        enemies.iter().count(),
        stats.bosses_killed.min(5),
        fps
    ));
}

fn update_banner(
    banner: Res<Banner>,
    mut main: Query<(&mut Text, &mut TextColor), (With<BannerText>, Without<BannerSub>)>,
    mut sub: Query<(&mut Text, &mut TextColor), (With<BannerSub>, Without<BannerText>)>,
) {
    let a = banner.timer.min(1.0);
    if let Ok((mut t, mut c)) = main.single_mut() {
        if t.0 != banner.text {
            t.0 = banner.text.clone();
        }
        c.0 = Color::srgba(1.0, 0.95, 0.82, a);
    }
    if let Ok((mut t, mut c)) = sub.single_mut() {
        if t.0 != banner.sub {
            t.0 = banner.sub.clone();
        }
        c.0 = Color::srgba(1.0, 1.0, 1.0, a * 0.9);
    }
}

fn update_overlay(
    state: Res<State<GameState>>,
    stats: Res<RunStats>,
    progress: Res<Progress>,
    director: Res<Director>,
    mut overlay: Query<&mut Visibility, With<Overlay>>,
    mut text: Query<&mut Text, With<OverlayText>>,
) {
    let (Ok(mut vis), Ok(mut t)) = (overlay.single_mut(), text.single_mut()) else {
        return;
    };
    let msg = match state.get() {
        GameState::Paused => Some("PAUSED\n\nP to resume    R to restart".to_string()),
        GameState::GameOver => {
            let endless = if director.endless {
                format!("\nSurvived the endless siege for {}", fmt_time(director.endless_time))
            } else {
                String::new()
            };
            Some(format!(
                "YOUR CASTLE HAS FALLEN\n\nTime {}    Level {}    Kills {}\nBosses defeated {}/5{}\n\nPress R to try again",
                fmt_time(stats.time),
                progress.level,
                stats.kills,
                stats.bosses_killed.min(5),
                endless
            ))
        }
        _ => None,
    };
    match msg {
        Some(m) => {
            *vis = Visibility::Visible;
            if t.0 != m {
                t.0 = m;
            }
        }
        None => *vis = Visibility::Hidden,
    }
}
