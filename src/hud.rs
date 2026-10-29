//! On-screen info: XP and level, boss timer and boss health, castle health and
//! nitro, ability slots with cooldowns, artifacts, banners and end screens.
//!
//! The look: dark leather panels edged in bronze, glossy bars that leave a
//! pale trail when they drop, and small animations that answer what happens
//! in the game (a level ring that pops, an ability that flashes when ready,
//! the screen edges flushing red when the walls are hit).

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
use bevy::ui::{
    AngularColorStop, BackgroundGradient, ColorStop, ConicGradient, LinearGradient, RadialGradient, RadialGradientShape, UiPosition,
    UiTransform, Val2,
};
use std::f32::consts::TAU;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudAnim>()
            .add_systems(Update, spawn_hud.run_if(resource_added::<Icons>))
            .add_systems(OnEnter(GameState::Starting), |mut a: ResMut<HudAnim>| *a = HudAnim::default())
            .add_systems(
                Update,
                (update_bars, update_boss, update_slots, update_artifacts, update_info, update_banner, update_overlay, update_vignette),
            );
    }
}

#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct LevelRing;
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpGhost;
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
struct BossGhost;
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
struct NitroSlot;
#[derive(Component)]
struct NitroSlotFill;
#[derive(Component)]
struct ArtifactColumn;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct BannerBox;
#[derive(Component)]
struct BannerText;
#[derive(Component)]
struct BannerSub;
#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct OverlayTitle;
#[derive(Component)]
struct OverlayText;
#[derive(Component)]
struct HurtVignette;

const FRAME: Color = Color::srgb(0.12, 0.09, 0.08);
const BRONZE: Color = Color::srgb(0.58, 0.44, 0.25);
const BRONZE_DIM: Color = Color::srgb(0.30, 0.22, 0.13);
const GOLD: Color = Color::srgb(0.98, 0.74, 0.26);
const PARCHMENT: Color = Color::srgb(1.0, 0.9, 0.7);
const HP_RED: Color = Color::srgb(0.78, 0.13, 0.11);
const SLOT_BG: Color = Color::srgb(0.13, 0.11, 0.12);

use crate::assets::{body as text, title};

fn round(px: f32) -> BorderRadius {
    BorderRadius::all(CornerRadius::circular(Val::Px(px)))
}

fn shadow() -> TextShadow {
    TextShadow { offset: Vec2::new(0.0, 2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) }
}

/// Dark leather: a little lighter at the top, like light falling on it.
fn leather() -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::auto(Color::srgba(0.17, 0.12, 0.09, 0.9)),
        ColorStop::auto(Color::srgba(0.07, 0.05, 0.04, 0.88)),
    ]))
}

/// Soft drop shadow under panels.
fn panel_shadow() -> BoxShadow {
    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.55), Val::Px(0.0), Val::Px(4.0), Val::Px(0.0), Val::Px(10.0))
}

/// Light from above, shade below: makes a flat bar look rounded.
fn gloss() -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.32), 0.0),
            ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.06), 45.0),
            ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.0), 55.0),
            ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.3), 100.0),
        ])),
    )
}

/// Thin notches dividing a bar into quarters.
fn ticks() -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        children![tick(25.0), tick(50.0), tick(75.0)],
    )
}

fn tick(at: f32) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, left: Val::Percent(at), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
    )
}

fn fill_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

fn spawn_hud(mut commands: Commands, icons: Res<Icons>) {
    // Darkened screen edges, drawn under everything else.
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundGradient::from(RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![
                ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.0), 55.0),
                ColorStop::percent(Color::srgba(0.02, 0.01, 0.0, 0.42), 100.0),
            ],
        )),
        GlobalZIndex(-2),
        bevy::picking::Pickable::IGNORE,
    ));
    commands.spawn((
        HurtVignette,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundGradient::default(),
        GlobalZIndex(-1),
        bevy::picking::Pickable::IGNORE,
    ));

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
                            height: Val::Px(20.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(10.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.06, 0.05, 0.05)),
                        BorderColor::all(BRONZE),
                        panel_shadow(),
                        children![
                            (
                                XpFill,
                                Node { width: Val::Percent(0.0), ..fill_node() },
                                BackgroundGradient::from(LinearGradient::to_right(vec![
                                    ColorStop::auto(Color::srgb(0.78, 0.48, 0.10)),
                                    ColorStop::auto(Color::srgb(1.0, 0.80, 0.30)),
                                ])),
                                children![gloss()],
                            ),
                            ticks(),
                            (XpText, text("", 12.0, Color::srgba(1.0, 0.95, 0.85, 0.9)), shadow()),
                        ],
                    ),
                    (
                        LevelRing,
                        Node {
                            width: Val::Px(46.0),
                            height: Val::Px(46.0),
                            border: UiRect::all(Val::Px(3.0)),
                            border_radius: round(23.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundGradient::from(RadialGradient::new(
                            UiPosition::CENTER,
                            RadialGradientShape::ClosestSide,
                            vec![ColorStop::auto(Color::srgb(0.22, 0.15, 0.07)), ColorStop::auto(Color::srgb(0.05, 0.04, 0.04))],
                        )),
                        BorderColor::all(GOLD),
                        BoxShadow::new(Color::srgba(1.0, 0.7, 0.2, 0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(12.0)),
                        UiTransform::IDENTITY,
                        children![(LevelText, title("1", 21.0, GOLD), shadow())],
                    ),
                ],
            ),
            (
                BossTimer,
                UiTransform::IDENTITY,
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
                children![
                    (ImageNode::new(icons.get(Icon::Skull)), Node { width: Val::Px(24.0), height: Val::Px(24.0), ..default() }),
                    (BossTimerText, title("", 19.0, Color::srgb(0.95, 0.32, 0.24)), shadow()),
                ],
            ),
            (
                BossBar,
                Visibility::Hidden,
                UiTransform::IDENTITY,
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), ..default() },
                children![
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() },
                        children![
                            (Node { width: Val::Px(60.0), height: Val::Px(1.0), ..default() }, BackgroundColor(BRONZE)),
                            (BossName, title("", 18.0, PARCHMENT), shadow()),
                            (Node { width: Val::Px(60.0), height: Val::Px(1.0), ..default() }, BackgroundColor(BRONZE)),
                        ],
                    ),
                    (
                        Node {
                            width: Val::Px(460.0),
                            height: Val::Px(14.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(7.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.07, 0.03, 0.03)),
                        BorderColor::all(BRONZE_DIM),
                        panel_shadow(),
                        children![
                            (BossGhost, fill_node(), BackgroundColor(Color::srgba(1.0, 0.85, 0.6, 0.8))),
                            (
                                BossFill,
                                fill_node(),
                                BackgroundGradient::from(LinearGradient::to_right(vec![
                                    ColorStop::auto(Color::srgb(0.45, 0.04, 0.04)),
                                    ColorStop::auto(Color::srgb(0.85, 0.14, 0.10)),
                                ])),
                                children![gloss()],
                            ),
                            ticks(),
                        ],
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
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: round(8.0),
            ..default()
        },
        leather(),
        BorderColor::all(BRONZE),
        panel_shadow(),
        children![
            (StageText, title("", 14.0, PARCHMENT), shadow()),
            (
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(22.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: round(5.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.03, 0.03)),
                BorderColor::all(FRAME),
                children![
                    (HpGhost, fill_node(), BackgroundColor(Color::srgba(1.0, 0.86, 0.62, 0.85))),
                    (HpFill, fill_node(), BackgroundColor(HP_RED), children![gloss()]),
                    ticks(),
                    (HpText, text("", 14.0, Color::WHITE), shadow()),
                ],
            ),
            (
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(9.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: round(4.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.03, 0.05, 0.1)),
                BorderColor::all(FRAME),
                children![(
                    NitroFill,
                    fill_node(),
                    BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::auto(Color::srgb(0.12, 0.32, 0.85)),
                        ColorStop::auto(Color::srgb(0.45, 0.78, 1.0)),
                    ])),
                    children![gloss()],
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
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(12.0), Val::Px(8.0)),
                column_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: round(10.0),
                ..default()
            },
            leather(),
            BorderColor::all(BRONZE),
            panel_shadow(),
        ))
        .id();
    let nitro = commands
        .spawn((
            NitroSlot,
            slot_node(),
            BackgroundColor(SLOT_BG),
            BorderColor::all(BRONZE_DIM),
            BoxShadow::new(Color::srgba(0.3, 0.6, 1.0, 0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(10.0)),
            children![
                (
                    NitroSlotFill,
                    Node { top: Val::Auto, bottom: Val::Px(0.0), ..fill_node() },
                    BackgroundGradient::from(LinearGradient::to_top(vec![
                        ColorStop::auto(Color::srgba(0.2, 0.45, 1.0, 0.45)),
                        ColorStop::auto(Color::srgba(0.4, 0.75, 1.0, 0.15)),
                    ])),
                ),
                (ImageNode::new(icons.get(Icon::Nitro)), Node { width: Val::Px(46.0), height: Val::Px(46.0), ..default() }),
                key_label("Shift"),
            ],
        ))
        .id();
    commands.entity(bar).add_child(nitro);
    for (i, label) in ABILITY_LABELS.iter().enumerate() {
        let slot = commands
            .spawn((
                Slot(i),
                slot_node(),
                BackgroundColor(SLOT_BG),
                BorderColor::all(BRONZE_DIM),
                BoxShadow::new(Color::srgba(1.0, 0.75, 0.3, 0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(12.0)),
                UiTransform::IDENTITY,
                children![
                    (SlotIcon(i), ImageNode::default(), Visibility::Hidden, Node { width: Val::Px(52.0), height: Val::Px(52.0), ..default() }),
                    (
                        SlotCooldown(i),
                        Node { border_radius: round(8.0), ..fill_node() },
                        BackgroundGradient::default(),
                    ),
                    (SlotCdText(i), title("", 22.0, Color::WHITE), shadow()),
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-5.0),
                            bottom: Val::Px(-5.0),
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(10.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.06, 0.05)),
                        BorderColor::all(GOLD),
                        children![(SlotLevel(i), text("", 11.0, GOLD))],
                    ),
                    key_label(label),
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
            bottom: Val::Px(108.0),
            max_height: Val::Px(250.0),
            flex_direction: FlexDirection::ColumnReverse,
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(5.0),
            column_gap: Val::Px(10.0),
            ..default()
        },
    ));

    // Run info (top right).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(14.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: round(8.0),
            ..default()
        },
        leather(),
        BorderColor::all(BRONZE),
        panel_shadow(),
        children![(InfoText, text("", 14.0, PARCHMENT), shadow(), TextLayout::justify(Justify::Right))],
    ));

    // Banner.
    commands.spawn((
        BannerBox,
        UiTransform::IDENTITY,
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
            (BannerText, title("", 44.0, Color::WHITE), TextShadow { offset: Vec2::new(0.0, 3.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) }),
            (BannerSub, text("", 22.0, Color::WHITE), shadow()),
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
        BackgroundGradient::from(RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![ColorStop::auto(Color::srgba(0.0, 0.0, 0.0, 0.45)), ColorStop::auto(Color::srgba(0.0, 0.0, 0.0, 0.85))],
        )),
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(56.0), Val::Px(30.0)),
                border: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::auto(Color::srgba(0.08, 0.05, 0.04, 0.0)),
                ColorStop::auto(Color::srgba(0.08, 0.05, 0.04, 0.85)),
                ColorStop::auto(Color::srgba(0.08, 0.05, 0.04, 0.85)),
                ColorStop::auto(Color::srgba(0.08, 0.05, 0.04, 0.0)),
            ])),
            BorderColor::all(BRONZE),
            children![
                (OverlayTitle, title("", 54.0, PARCHMENT), TextShadow { offset: Vec2::new(0.0, 4.0), color: Color::BLACK }),
                (OverlayText, text("", 22.0, Color::srgb(0.93, 0.88, 0.78)), shadow(), TextLayout::justify(Justify::Center)),
            ],
        )],
    ));
}

fn slot_node() -> Node {
    Node {
        width: Val::Px(64.0),
        height: Val::Px(64.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.0)),
        border_radius: round(9.0),
        ..default()
    }
}

/// A small keycap.
fn key_label(k: &str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(-4.0),
            top: Val::Px(-10.0),
            padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
            border: UiRect::new(Val::Px(1.0), Val::Px(1.0), Val::Px(1.0), Val::Px(2.0)),
            border_radius: round(4.0),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::auto(Color::srgb(0.36, 0.33, 0.31)),
            ColorStop::auto(Color::srgb(0.2, 0.18, 0.17)),
        ])),
        BorderColor::all(Color::srgb(0.08, 0.07, 0.07)),
        children![text(k.to_string(), 11.0, Color::srgb(0.95, 0.92, 0.86))],
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

/// Springy overshoot for pops (0 → 1).
fn ease_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + t * t * (2.7 * t + 1.7)
}

/// What the HUD remembers between frames for its animations.
#[derive(Resource)]
struct HudAnim {
    hp_ghost: f32,
    hp_hold: f32,
    xp_shown: f32,
    level: u32,
    level_pop: f32,
    boss_ghost: f32,
    boss_hold: f32,
    boss_kind: Option<crate::enemy::EnemyKind>,
    boss_pop: f32,
    boss_shake: f32,
    last_boss_hp: f32,
    slot_cd: [f32; 4],
    slot_pop: [f32; 4],
    slot_flash: [f32; 4],
    hurt: f32,
    prev_hp: f32,
    banner: String,
    banner_t: f32,
}

impl Default for HudAnim {
    fn default() -> Self {
        Self {
            hp_ghost: 1.0,
            hp_hold: 0.0,
            xp_shown: 0.0,
            level: 1,
            level_pop: 9.0,
            boss_ghost: 1.0,
            boss_hold: 0.0,
            boss_kind: None,
            boss_pop: 9.0,
            boss_shake: 0.0,
            last_boss_hp: 1.0,
            slot_cd: [0.0; 4],
            slot_pop: [9.0; 4],
            slot_flash: [9.0; 4],
            hurt: 0.0,
            prev_hp: f32::MAX,
            banner: String::new(),
            banner_t: 9.0,
        }
    }
}

/// A trailing bar: holds still briefly after a drop, then slides down.
fn trail(ghost: &mut f32, hold: &mut f32, value: f32, dt: f32) {
    if value >= *ghost {
        *ghost = value;
        *hold = 0.0;
    } else {
        *hold += dt;
        if *hold > 0.45 {
            *ghost = (*ghost - dt * 0.6).max(value);
        }
    }
}

fn update_bars(
    time: Res<Time>,
    progress: Res<Progress>,
    stats: Res<Stats>,
    loadout: Res<Loadout>,
    mut anim: ResMut<HudAnim>,
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
        Query<&mut Node, With<HpGhost>>,
        Query<&mut Node, With<NitroFill>>,
        Query<&mut Node, With<NitroSlotFill>>,
    )>,
    mut ring: Query<(&mut UiTransform, &mut BoxShadow), With<LevelRing>>,
    mut nitro_slot: Query<&mut BoxShadow, (With<NitroSlot>, Without<LevelRing>)>,
) {
    let Ok(c) = castle.single() else {
        return;
    };
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
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

    // XP fills smoothly; a new level empties it and pops the ring.
    let xp = (progress.xp / progress.need).clamp(0.0, 1.0);
    if progress.level != anim.level {
        if progress.level > anim.level {
            anim.level_pop = 0.0;
        }
        anim.level = progress.level;
        anim.xp_shown = 0.0;
    }
    anim.xp_shown += (xp - anim.xp_shown) * (1.0 - (-10.0 * dt).exp());
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent(anim.xp_shown * 100.0);
    }
    anim.level_pop += dt;
    if let Ok((mut tf, mut glow)) = ring.single_mut() {
        let p = anim.level_pop;
        let s = if p < 0.5 { 1.0 + 0.45 * (1.0 - ease_back(p / 0.5)) + 0.25 * (1.0 - p / 0.5) } else { 1.0 };
        tf.scale = Vec2::splat(s);
        glow.0[0].color = Color::srgba(1.0, 0.72, 0.25, (1.0 - p / 1.2).clamp(0.0, 1.0) * 0.9);
        glow.0[0].spread_radius = Val::Px(2.0 + (1.0 - p.min(1.2) / 1.2) * 8.0);
    }

    // Health: a pale trail shows how much a hit took; low health pulses.
    let hp = (c.hp / stats.max_hp).clamp(0.0, 1.0);
    let (mut ghost, mut hold) = (anim.hp_ghost, anim.hp_hold);
    trail(&mut ghost, &mut hold, hp, dt);
    anim.hp_ghost = ghost;
    anim.hp_hold = hold;
    if c.hp < anim.prev_hp - 0.5 {
        anim.hurt = (anim.hurt + (anim.prev_hp - c.hp) / stats.max_hp * 6.0).min(1.0);
    }
    anim.prev_hp = c.hp;
    if let Ok((mut n, mut bg)) = fills.p1().single_mut() {
        n.width = Val::Percent(hp * 100.0);
        let pulse = if hp < 0.3 { 0.5 + 0.5 * (now * 7.0).sin() } else { 0.0 };
        bg.0 = if c.hurt_flash > 0.15 {
            Color::srgb(1.0, 0.55, 0.45)
        } else {
            HP_RED.mix(&Color::srgb(1.0, 0.25, 0.18), pulse)
        };
    }
    if let Ok(mut n) = fills.p2().single_mut() {
        n.width = Val::Percent(anim.hp_ghost * 100.0);
    }
    let nf = (c.nitro / stats.nitro_max).clamp(0.0, 1.0) * 100.0;
    if let Ok(mut n) = fills.p3().single_mut() {
        n.width = Val::Percent(nf);
    }
    if let Ok(mut n) = fills.p4().single_mut() {
        n.height = Val::Percent(nf);
    }
    if let Ok(mut glow) = nitro_slot.single_mut() {
        let a = if c.boosting { 0.55 + 0.25 * (now * 18.0).sin() } else { 0.0 };
        glow.0[0].color = Color::srgba(0.3, 0.6, 1.0, a);
    }
}

fn update_boss(
    time: Res<Time>,
    director: Res<Director>,
    mut anim: ResMut<HudAnim>,
    castle: Query<&Transform, With<Castle>>,
    bosses: Query<(&Transform, &Enemy), With<Boss>>,
    mut timer: Query<(&mut Visibility, &mut UiTransform), (With<BossTimer>, Without<BossBar>)>,
    mut timer_text: Query<&mut Text, (With<BossTimerText>, Without<BossName>)>,
    mut bar: Query<(&mut Visibility, &mut UiTransform), (With<BossBar>, Without<BossTimer>)>,
    mut name: Query<&mut Text, (With<BossName>, Without<BossTimerText>)>,
    mut fill: Query<&mut Node, (With<BossFill>, Without<BossGhost>)>,
    mut ghost: Query<&mut Node, (With<BossGhost>, Without<BossFill>)>,
) {
    let (Ok((mut tv, mut ttf)), Ok(mut tt), Ok((mut bv, mut btf)), Ok(mut bn), Ok(mut bf), Ok(mut bg)) =
        (timer.single_mut(), timer_text.single_mut(), bar.single_mut(), name.single_mut(), fill.single_mut(), ghost.single_mut())
    else {
        return;
    };
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let cpos = castle.single().map(|t| p2(t.translation)).unwrap_or_default();
    let nearest = bosses
        .iter()
        .min_by(|a, b| p2(a.0.translation).distance(cpos).total_cmp(&p2(b.0.translation).distance(cpos)));

    // The countdown throbs during its last ten seconds.
    let mut throb = 1.0;
    match (director.endless, director.next_boss()) {
        (false, Some(_)) if director.boss_timer.is_finite() => {
            *tv = Visibility::Inherited;
            set(&mut tt, format!("BOSS IN {}", fmt_time(director.boss_timer)));
            if director.boss_timer < 10.0 {
                throb = 1.0 + 0.12 * (now * 6.0).sin().max(0.0);
            }
        }
        (true, _) => {
            *tv = Visibility::Inherited;
            set(&mut tt, format!("ENDLESS  {}   next boss {}", fmt_time(director.endless_time), fmt_time(director.boss_timer)));
            if director.boss_timer < 10.0 {
                throb = 1.0 + 0.12 * (now * 6.0).sin().max(0.0);
            }
        }
        _ => *tv = Visibility::Hidden,
    }
    ttf.scale = Vec2::splat(throb);

    match nearest {
        Some((_, en)) => {
            *bv = Visibility::Inherited;
            if anim.boss_kind != Some(en.kind) {
                anim.boss_kind = Some(en.kind);
                anim.boss_pop = 0.0;
                anim.boss_ghost = 1.0;
                anim.last_boss_hp = 1.0;
            }
            set(&mut bn, en.kind.name().to_uppercase());
            let hp = (en.hp / en.max_hp).clamp(0.0, 1.0);
            if hp < anim.last_boss_hp - 0.004 {
                anim.boss_shake = (anim.boss_shake + (anim.last_boss_hp - hp) * 40.0).min(1.0);
            }
            anim.last_boss_hp = hp;
            let (mut g, mut h) = (anim.boss_ghost, anim.boss_hold);
            trail(&mut g, &mut h, hp, dt);
            anim.boss_ghost = g;
            anim.boss_hold = h;
            bf.width = Val::Percent(hp * 100.0);
            bg.width = Val::Percent(anim.boss_ghost * 100.0);
            anim.boss_pop += dt;
            anim.boss_shake = (anim.boss_shake - dt * 4.0).max(0.0);
            let s = 0.7 + 0.3 * ease_back(anim.boss_pop / 0.45);
            let jx = (now * 70.0).sin() * anim.boss_shake * 3.0;
            btf.scale = Vec2::splat(s);
            btf.translation = Val2::px(jx, 0.0);
        }
        None => {
            *bv = Visibility::Hidden;
            anim.boss_kind = None;
        }
    }
}

fn update_slots(
    time: Res<Time>,
    loadout: Res<Loadout>,
    cds: Res<Cooldowns>,
    icons: Res<Icons>,
    mut anim: ResMut<HudAnim>,
    mut slot_bg: Query<(&Slot, &mut BackgroundColor, &mut BorderColor, &mut BoxShadow, &mut UiTransform)>,
    mut slot_icons: Query<(&SlotIcon, &mut ImageNode, &mut Visibility)>,
    mut overlays: Query<(&SlotCooldown, &mut BackgroundGradient)>,
    mut cd_texts: Query<(&SlotCdText, &mut Text), Without<SlotLevel>>,
    mut levels: Query<(&SlotLevel, &mut Text), Without<SlotCdText>>,
) {
    let dt = time.delta_secs();
    // Cooldown left (0..1) per slot, and pops when used or ready again.
    let mut frac = [0.0f32; 4];
    for (i, f) in frac.iter_mut().enumerate() {
        *f = loadout
            .modules
            .get(i)
            .and_then(|m| cds.0.get(&m.kind).map(|t| t.ability / m.kind.ability_cooldown()))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let prev = anim.slot_cd[i];
        if *f > prev + 0.2 {
            anim.slot_pop[i] = 0.0;
        }
        if prev > 0.0 && *f <= 0.0 {
            anim.slot_flash[i] = 0.0;
        }
        anim.slot_cd[i] = *f;
        anim.slot_pop[i] += dt;
        anim.slot_flash[i] += dt;
    }

    for (s, mut bg, mut border, mut glow, mut tf) in &mut slot_bg {
        let i = s.0;
        let open = i < loadout.max_slots();
        bg.0 = if open { SLOT_BG } else { Color::srgb(0.05, 0.04, 0.04) };
        let flash = (1.0 - anim.slot_flash[i] / 0.6).clamp(0.0, 1.0);
        *border = BorderColor::all(BRONZE_DIM.mix(&GOLD, flash));
        glow.0[0].color = Color::srgba(1.0, 0.75, 0.3, flash * 0.9);
        // Squeeze when used, swell when ready.
        let p = anim.slot_pop[i];
        let used = if p < 0.3 { 0.86 + 0.14 * ease_back(p / 0.3) } else { 1.0 };
        let ready = 1.0 + 0.12 * flash;
        tf.scale = Vec2::splat(used * ready);
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
    // A clock-hand sweep shows the time left.
    for (s, mut g) in &mut overlays {
        let f = frac[s.0];
        let want: BackgroundGradient = if f <= 0.0 {
            BackgroundGradient::default()
        } else {
            let a = f * TAU;
            let dark = Color::srgba(0.0, 0.0, 0.0, 0.62);
            let clear = Color::srgba(0.0, 0.0, 0.0, 0.0);
            BackgroundGradient::from(ConicGradient::new(
                UiPosition::CENTER,
                vec![
                    AngularColorStop::new(dark, 0.0),
                    AngularColorStop::new(dark, a),
                    AngularColorStop::new(clear, a),
                    AngularColorStop::new(clear, TAU),
                ],
            ))
        };
        if *g != want {
            *g = want;
        }
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
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() },
                children![
                    (
                        Node {
                            width: Val::Px(34.0),
                            height: Val::Px(34.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: round(17.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.05, 0.08, 0.85)),
                        BorderColor::all(BRONZE),
                        panel_shadow(),
                        children![(
                            ImageNode::new(icons.get(Icon::Artifact(a))),
                            Node { width: Val::Px(28.0), height: Val::Px(28.0), border_radius: round(14.0), ..default() },
                        )],
                    ),
                    (text(if n > 1 { format!("x{n}") } else { String::new() }, 14.0, PARCHMENT), shadow()),
                ],
            ))
            .id();
        commands.entity(col).add_child(item);
    }
}

fn update_info(
    stats: Res<RunStats>,
    director: Res<Director>,
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
        director.defeated.len(),
        fps
    ));
}

fn update_banner(
    time: Res<Time>,
    banner: Res<Banner>,
    mut anim: ResMut<HudAnim>,
    mut boxes: Query<&mut UiTransform, With<BannerBox>>,
    mut main: Query<(&mut Text, &mut TextColor), (With<BannerText>, Without<BannerSub>)>,
    mut sub: Query<(&mut Text, &mut TextColor), (With<BannerSub>, Without<BannerText>)>,
) {
    // New messages swing in large and settle.
    if banner.text != anim.banner && banner.timer > 0.0 {
        anim.banner = banner.text.clone();
        anim.banner_t = 0.0;
    }
    anim.banner_t += time.delta_secs();
    let a = banner.timer.min(1.0);
    if let Ok(mut tf) = boxes.single_mut() {
        let k = ease_back(anim.banner_t / 0.4);
        tf.scale = Vec2::splat(1.0 + 0.35 * (1.0 - k));
    }
    let fade_in = (anim.banner_t / 0.2).min(1.0);
    if let Ok((mut t, mut c)) = main.single_mut() {
        if t.0 != banner.text {
            t.0 = banner.text.clone();
        }
        c.0 = Color::srgba(1.0, 0.93, 0.78, a * fade_in);
    }
    if let Ok((mut t, mut c)) = sub.single_mut() {
        if t.0 != banner.sub {
            t.0 = banner.sub.clone();
        }
        let late = ((anim.banner_t - 0.15) / 0.3).clamp(0.0, 1.0);
        c.0 = Color::srgba(1.0, 1.0, 1.0, a * 0.9 * late);
    }
}

fn update_overlay(
    state: Res<State<GameState>>,
    stats: Res<RunStats>,
    progress: Res<Progress>,
    director: Res<Director>,
    mut overlay: Query<&mut Visibility, With<Overlay>>,
    mut heading: Query<(&mut Text, &mut TextColor), (With<OverlayTitle>, Without<OverlayText>)>,
    mut text: Query<&mut Text, (With<OverlayText>, Without<OverlayTitle>)>,
) {
    let (Ok(mut vis), Ok((mut h, mut hc)), Ok(mut t)) = (overlay.single_mut(), heading.single_mut(), text.single_mut()) else {
        return;
    };
    let msg = match state.get() {
        GameState::Paused => Some(("Paused".to_string(), PARCHMENT, "P to resume        R to restart".to_string())),
        GameState::GameOver => {
            let endless = if director.endless {
                format!("\nSurvived the endless siege for {}", fmt_time(director.endless_time))
            } else {
                String::new()
            };
            Some((
                "Your castle has fallen".to_string(),
                Color::srgb(0.95, 0.42, 0.32),
                format!(
                    "Time {}     Level {}     Kills {}\nBosses defeated {}/5{}\n\nPress R to try again",
                    fmt_time(stats.time),
                    progress.level,
                    stats.kills,
                    director.defeated.len(),
                    endless
                ),
            ))
        }
        _ => None,
    };
    match msg {
        Some((head, color, body)) => {
            *vis = Visibility::Visible;
            if h.0 != head {
                h.0 = head;
            }
            hc.0 = color;
            if t.0 != body {
                t.0 = body;
            }
        }
        None => *vis = Visibility::Hidden,
    }
}

/// The screen edges flush red when the walls take a beating, and throb
/// while health is low.
fn update_vignette(
    time: Res<Time>,
    stats: Res<Stats>,
    mut anim: ResMut<HudAnim>,
    castle: Query<&Castle>,
    mut q: Query<&mut BackgroundGradient, With<HurtVignette>>,
) {
    let (Ok(c), Ok(mut g)) = (castle.single(), q.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    anim.hurt = (anim.hurt - dt * 1.6).max(0.0);
    let hp = (c.hp / stats.max_hp).clamp(0.0, 1.0);
    let low = if hp < 0.3 { (0.3 - hp) / 0.3 * (0.55 + 0.45 * (time.elapsed_secs() * 5.0).sin()) } else { 0.0 };
    let a = ((anim.hurt * 0.55 + low * 0.45).min(0.7) * 50.0).round() / 50.0;
    let want = if a <= 0.0 {
        BackgroundGradient::default()
    } else {
        BackgroundGradient::from(RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![
                ColorStop::percent(Color::srgba(0.6, 0.0, 0.0, 0.0), 45.0),
                ColorStop::percent(Color::srgba(0.55, 0.02, 0.0, a), 100.0),
            ],
        ))
    };
    if *g != want {
        *g = want;
    }
}
