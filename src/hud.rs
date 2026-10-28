//! On-screen info: health, growth, wave, enemies, FPS, banners and end screens.

use crate::castle::{BARRAGE_COOLDOWN, Castle, STAGES};
use crate::enemy::Enemy;
use crate::waves::{Phase, TOTAL_WAVES, WaveDirector};
use crate::{Banner, GameState, RunStats};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_hud, update_banner, update_overlay));
    }
}

#[derive(Component)]
struct StageText;
#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct BarrageText;
#[derive(Component)]
struct BannerText;
#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct OverlayText;

fn text(s: &str, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont::from_font_size(size), TextColor(color))
}

fn bar(fill: impl Component, color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Px(280.0),
            height: Val::Px(14.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![(
            fill,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        )],
    )
}

fn spawn_hud(mut commands: Commands) {
    let panel = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35));

    // Top-left: castle status.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        },
        panel,
        children![
            (StageText, text("", 20.0, Color::srgb(1.0, 0.85, 0.4))),
            (HpText, text("", 14.0, Color::WHITE)),
            bar(HpFill, Color::srgb(0.85, 0.2, 0.2)),
            (XpText, text("", 14.0, Color::WHITE)),
            bar(XpFill, Color::srgb(0.3, 0.85, 1.0)),
        ],
    ));

    // Top-right: wave and run info.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            top: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        panel,
        children![(InfoText, text("", 16.0, Color::WHITE))],
    ));

    // Bottom: barrage cooldown and controls.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        },
        children![
            (BarrageText, text("", 20.0, Color::WHITE)),
            text(
                "WASD / Arrows: drive    SPACE: cannon barrage    P: pause    R: restart",
                13.0,
                Color::srgba(1.0, 1.0, 1.0, 0.6),
            ),
        ],
    ));

    // Center banner.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(22.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(BannerText, text("", 38.0, Color::WHITE))],
    ));

    // Pause / game over / victory screen.
    commands.spawn((
        Overlay,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        children![(OverlayText, text("", 30.0, Color::WHITE), TextLayout::justify(Justify::Center))],
    ));
}

fn fmt_time(t: f32) -> String {
    let s = t as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[allow(clippy::too_many_arguments)]
fn update_hud(
    castle: Query<&Castle>,
    enemies: Query<(), With<Enemy>>,
    waves: Res<WaveDirector>,
    stats: Res<RunStats>,
    diagnostics: Res<DiagnosticsStore>,
    mut texts: ParamSet<(
        Query<&mut Text, With<StageText>>,
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<XpText>>,
        Query<&mut Text, With<InfoText>>,
        Query<&mut Text, With<BarrageText>>,
    )>,
    mut fills: ParamSet<(Query<&mut Node, With<HpFill>>, Query<&mut Node, With<XpFill>>)>,
) {
    let Ok(c) = castle.single() else {
        return;
    };
    let def = c.def();

    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format!("Stage {}/3  -  {}", c.stage + 1, def.name);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("Walls {:.0} / {:.0}", c.hp.max(0.0), def.max_hp);
    }
    let (xp_label, xp_frac) = match def.xp_next {
        Some(need) => {
            let prev = if c.stage == 0 { 0 } else { STAGES[c.stage - 1].xp_next.unwrap_or(0) };
            let frac = (c.xp.saturating_sub(prev)) as f32 / (need - prev) as f32;
            (format!("Grow to {}: {} / {} XP", STAGES[c.stage + 1].name, c.xp, need), frac)
        }
        None => (format!("Fully grown!  XP {}", c.xp), 1.0),
    };
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = xp_label;
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent((c.hp / def.max_hp).clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent(xp_frac.clamp(0.0, 1.0) * 100.0);
    }

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let wave_line = match (waves.phase, waves.wave) {
        (Phase::Break, 0) => format!("First wave in {:.0}s", waves.break_left().ceil()),
        (Phase::Break, w) => format!("Wave {}/{TOTAL_WAVES} in {:.0}s", w + 1, waves.break_left().ceil()),
        (Phase::Active, w) => format!("Wave {w}/{TOTAL_WAVES}"),
    };
    if let Ok(mut t) = texts.p3().single_mut() {
        t.0 = format!(
            "{wave_line}\nEnemies  {}\nKills  {}\nVillages  {}\nTime  {}\nFPS  {:.0}",
            enemies.iter().count(),
            stats.kills,
            stats.villages,
            fmt_time(stats.time),
            fps
        );
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        t.0 = if c.barrage_cd <= 0.0 {
            "Barrage READY  [SPACE]".to_string()
        } else {
            format!("Barrage  {:.1}s", c.barrage_cd.min(BARRAGE_COOLDOWN))
        };
    }
}

fn update_banner(banner: Res<Banner>, mut q: Query<(&mut Text, &mut TextColor), With<BannerText>>) {
    let Ok((mut t, mut color)) = q.single_mut() else {
        return;
    };
    if t.0 != banner.text {
        t.0 = banner.text.clone();
    }
    let alpha = banner.timer.min(1.0);
    color.0 = Color::srgba(1.0, 0.95, 0.8, alpha);
}

fn update_overlay(
    state: Res<State<GameState>>,
    stats: Res<RunStats>,
    waves: Res<WaveDirector>,
    mut overlay: Query<&mut Visibility, With<Overlay>>,
    mut text: Query<&mut Text, With<OverlayText>>,
) {
    let (Ok(mut vis), Ok(mut t)) = (overlay.single_mut(), text.single_mut()) else {
        return;
    };
    let summary = format!(
        "Wave {}   Kills {}   Villages {}   Time {}",
        waves.wave,
        stats.kills,
        stats.villages,
        fmt_time(stats.time)
    );
    let msg = match state.get() {
        GameState::Paused => Some("PAUSED\n\nP to resume   R to restart".to_string()),
        GameState::GameOver => Some(format!("YOUR CASTLE HAS FALLEN\n\n{summary}\n\nPress R to try again")),
        GameState::Victory => Some(format!("VICTORY!\nThe Warlord's citadel is rubble.\n\n{summary}\n\nPress R to play again")),
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
