//! XP, levels, and the level-up card choices (modules, artifacts, castle growth).

use crate::art::{Icon, Icons, Variant};
use crate::items::{
    compute_stats, stage_info, ArtifactKind, Loadout, ModuleKind, OwnedModule, MAX_MODULE_LEVEL, STAGE_LEVELS,
};
use crate::{GameRng, GameState};
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Progress>()
            .init_resource::<Choice>()
            .init_resource::<ClickedCard>()
            .add_systems(OnEnter(GameState::Starting), |mut p: ResMut<Progress>| *p = Progress::default())
            .add_systems(Update, open_choice.run_if(in_state(GameState::Playing)))
            .add_systems(OnEnter(GameState::Choosing), spawn_choice_ui)
            .add_systems(OnExit(GameState::Choosing), despawn_choice_ui)
            .add_systems(Update, pick_card.run_if(in_state(GameState::Choosing)));
    }
}

pub const MAX_ARTIFACT_STACKS: u32 = 5;

#[derive(Resource)]
pub struct Progress {
    pub level: u32,
    pub xp: f32,
    pub need: f32,
    /// Level-ups waiting for a card choice.
    pub pending: u32,
    pub rerolls: u32,
}

impl Default for Progress {
    fn default() -> Self {
        Self { level: 1, xp: 0.0, need: xp_needed(1), pending: 0, rerolls: 2 }
    }
}

pub fn xp_needed(level: u32) -> f32 {
    18.0 + 11.0 * (level - 1) as f32 + 0.35 * ((level - 1) as f32).powi(2)
}

impl Progress {
    pub fn add_xp(&mut self, xp: f32) {
        self.xp += xp;
        while self.xp >= self.need {
            self.xp -= self.need;
            self.level += 1;
            self.need = xp_needed(self.level);
            self.pending += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offer {
    NewModule(ModuleKind),
    UpgradeModule(ModuleKind, u32),
    Artifact(ArtifactKind, u32),
    Grow(Variant),
}

#[derive(Resource, Default)]
pub struct Choice {
    pub title: String,
    pub offers: Vec<Offer>,
    /// The level this choice is for.
    pub level: u32,
}

/// Picks the cards for the level that was just reached.
fn make_choice(level: u32, l: &Loadout, rng: &mut GameRng) -> Choice {
    if let Some(i) = STAGE_LEVELS.iter().position(|&s| s == level)
        && l.stage == i
    {
        return Choice {
            title: "Choose Castle Upgrade!".into(),
            offers: vec![Offer::Grow(Variant::Defense), Offer::Grow(Variant::Movement)],
            level,
        };
    }
    let mut new_mods: Vec<Offer> = Vec::new();
    let mut upgrades: Vec<Offer> = Vec::new();
    for k in ModuleKind::ALL {
        match l.module(k) {
            Some(m) if m.level < MAX_MODULE_LEVEL => upgrades.push(Offer::UpgradeModule(k, m.level + 1)),
            None if l.modules.len() < l.max_slots() => new_mods.push(Offer::NewModule(k)),
            _ => {}
        }
    }
    new_mods.shuffle(&mut rng.0);
    upgrades.shuffle(&mut rng.0);
    // With a free slot, show at least two new weapons.
    let mut modules: Vec<Offer> = new_mods.iter().take(2).copied().collect();
    let mut rest: Vec<Offer> = new_mods.into_iter().skip(2).chain(upgrades).collect();
    rest.shuffle(&mut rng.0);
    modules.extend(rest);
    let mut artifacts: Vec<Offer> = ArtifactKind::ALL
        .iter()
        .filter(|&&a| l.stacks(a) < MAX_ARTIFACT_STACKS)
        .map(|&a| Offer::Artifact(a, l.stacks(a) + 1))
        .collect();
    artifacts.shuffle(&mut rng.0);

    let module_turn = level.is_multiple_of(2);
    let (first, second, title) = if module_turn && !modules.is_empty() {
        (modules, artifacts, "Choose New Module!")
    } else {
        (artifacts, modules, "Choose New Artifact!")
    };
    let mut offers: Vec<Offer> = first.into_iter().take(3).collect();
    offers.shuffle(&mut rng.0);
    offers.extend(second.into_iter().take(3 - offers.len()));
    Choice { title: title.into(), offers, level }
}

fn apply(offer: Offer, l: &mut Loadout) {
    match offer {
        Offer::NewModule(k) => l.modules.push(OwnedModule { kind: k, level: 1 }),
        Offer::UpgradeModule(k, lvl) => {
            if let Some(m) = l.modules.iter_mut().find(|m| m.kind == k) {
                m.level = lvl;
            }
        }
        Offer::Artifact(a, n) => {
            l.artifacts.insert(a, n);
        }
        Offer::Grow(v) => {
            l.stage += 1;
            l.variant = v;
        }
    }
}

fn open_choice(
    mut progress: ResMut<Progress>,
    loadout: Res<Loadout>,
    mut choice: ResMut<Choice>,
    mut rng: ResMut<GameRng>,
    mut next: ResMut<NextState<GameState>>,
) {
    if progress.pending == 0 {
        return;
    }
    let level = progress.level + 1 - progress.pending;
    *choice = make_choice(level, &loadout, &mut rng);
    if choice.offers.is_empty() {
        progress.pending -= 1;
        return;
    }
    next.set(GameState::Choosing);
}

// ---------------------------------------------------------------------------
// Card screen
// ---------------------------------------------------------------------------

#[derive(Component)]
struct ChoiceScreen;

#[derive(Component)]
struct Card;

/// Set by a card's click observer, read by `pick_card`.
#[derive(Resource, Default)]
struct ClickedCard(Option<usize>);

const PANEL: Color = Color::srgb(0.13, 0.11, 0.13);
const PARCHMENT: Color = Color::srgb(0.93, 0.88, 0.78);
const INK: Color = Color::srgb(0.16, 0.12, 0.10);

fn frame_color(offer: Offer) -> Color {
    match offer {
        Offer::NewModule(_) | Offer::UpgradeModule(..) => Color::srgb(0.55, 0.36, 0.22),
        Offer::Artifact(..) => Color::srgb(0.62, 0.64, 0.70),
        Offer::Grow(_) => Color::srgb(0.62, 0.52, 0.36),
    }
}

use crate::assets::{body as text, title};

fn stat_line(label: &str, from: f32, to: f32) -> String {
    format!("{label:<14} {from:>5.0} -> {to:.0}")
}

/// Title, icon and description lines for one card.
fn card_content(offer: Offer, l: &Loadout) -> (String, Icon, String, Vec<String>) {
    match offer {
        Offer::NewModule(k) => {
            let (a, b) = k.describe();
            (k.name().into(), Icon::Module(k), format!("NEW  -  {} SLOT", k.mount().label()), vec![a.into(), b.into()])
        }
        Offer::UpgradeModule(k, lvl) => {
            let (a, _) = k.describe();
            (k.name().into(), Icon::Module(k), format!("LEVEL {} -> {}", lvl - 1, lvl),
                vec![a.into(), "+30% damage, more shots".into()])
        }
        Offer::Artifact(a, n) => {
            let tag = if n > 1 { format!("ARTIFACT  x{n}") } else { "ARTIFACT".into() };
            (a.name().into(), Icon::Artifact(a), tag, vec![a.describe().into()])
        }
        Offer::Grow(v) => {
            let cur = compute_stats(l);
            let mut nl = l.clone();
            nl.stage += 1;
            nl.variant = v;
            let new = compute_stats(&nl);
            let info = stage_info(nl.stage, v);
            let (label, icon) = match v {
                Variant::Movement => ("MOVEMENT", Icon::Movement),
                _ => ("DEFENSE", Icon::Defense),
            };
            (info.name.into(), icon, label.into(), vec![
                stat_line("Health", cur.max_hp, new.max_hp),
                stat_line("Top speed", cur.speed, new.speed),
                stat_line("Nitro", cur.nitro_max, new.nitro_max),
                stat_line("Coin magnet", cur.magnet, new.magnet),
                format!("{:<14} {:>5.1} -> {:.1}", "Turning", cur.turn, new.turn),
                "+1 module slot".into(),
            ])
        }
    }
}

fn spawn_choice_ui(mut commands: Commands, choice: Res<Choice>, loadout: Res<Loadout>, icons: Res<Icons>, progress: Res<Progress>) {
    build_choice_ui(&mut commands, &choice, &loadout, &icons, progress.rerolls);
}

fn build_choice_ui(commands: &mut Commands, choice: &Choice, loadout: &Loadout, icons: &Icons, rerolls: u32) {
    let root = commands
        .spawn((
            ChoiceScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.72)),
            GlobalZIndex(10),
        ))
        .id();
    let heading = commands.spawn(title(choice.title.clone(), 48.0, Color::srgb(0.98, 0.92, 0.80))).id();
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(26.0), ..default() })
        .id();
    commands.entity(root).add_children(&[heading, row]);

    for (i, &offer) in choice.offers.iter().enumerate() {
        let (name, icon, tag, lines) = card_content(offer, loadout);
        let frame = frame_color(offer);
        let card = commands
            .spawn((
                Card,
                Node {
                    width: Val::Px(290.0),
                    height: Val::Px(430.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(6.0)),
                    border_radius: BorderRadius::all(CornerRadius::circular(Val::Px(22.0))),
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(frame),
            ))
            .id();
        commands
            .entity(card)
            .observe(move |_: On<PointerClick>, mut clicked: ResMut<ClickedCard>| clicked.0 = Some(i))
            .observe(move |_: On<PointerOver>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(card) {
                    bg.0 = Color::srgb(0.24, 0.19, 0.22);
                }
            })
            .observe(move |_: On<PointerOut>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(card) {
                    bg.0 = PANEL;
                }
            });
        let banner = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                    border_radius: BorderRadius::all(CornerRadius::circular(Val::Px(6.0))),
                    ..default()
                },
                BackgroundColor(PARCHMENT),
                children![title(name.to_uppercase(), 20.0, INK)],
            ))
            .id();
        let icon_box = commands
            .spawn((
                Node {
                    width: Val::Px(130.0),
                    height: Val::Px(130.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(4.0)),
                    border_radius: BorderRadius::all(CornerRadius::circular(Val::Px(65.0))),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.07, 0.06, 0.08)),
                BorderColor::all(frame),
                children![(
                    ImageNode::new(icons.get(icon)),
                    Node { width: Val::Px(96.0), height: Val::Px(96.0), ..default() },
                )],
            ))
            .id();
        let tag = commands.spawn(title(tag, 14.0, Color::srgb(0.96, 0.78, 0.36))).id();
        commands.entity(card).add_children(&[banner, icon_box, tag]);
        for line in lines {
            let t = commands
                .spawn((
                    text(line, 19.0, Color::srgb(0.90, 0.88, 0.84)),
                    TextLayout::justify(Justify::Center),
                    Node { max_width: Val::Px(250.0), ..default() },
                ))
                .id();
            commands.entity(card).add_child(t);
        }
        let key = commands.spawn(text(format!("[{}]", i + 1), 18.0, Color::srgba(1.0, 1.0, 1.0, 0.5))).id();
        let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
        commands.entity(card).add_children(&[spacer, key]);
        commands.entity(row).add_child(card);
    }
    let hint = if rerolls > 0 {
        format!("Click a card or press 1 / 2 / 3      [T] Reroll ({rerolls} left)")
    } else {
        "Click a card or press 1 / 2 / 3".to_string()
    };
    let hint = commands.spawn(text(hint, 18.0, Color::srgba(1.0, 1.0, 1.0, 0.7))).id();
    commands.entity(root).add_child(hint);
}

fn despawn_choice_ui(mut commands: Commands, q: Query<Entity, With<ChoiceScreen>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Picks a card by number key, click, or the test autopilot.
fn pick_card(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<ClickedCard>,
    screens: Query<Entity, With<ChoiceScreen>>,
    auto: Option<Res<crate::debug::AutoPick>>,
    icons: Res<Icons>,
    mut choice: ResMut<Choice>,
    mut loadout: ResMut<Loadout>,
    mut progress: ResMut<Progress>,
    mut rng: ResMut<GameRng>,
    mut next: ResMut<NextState<GameState>>,
    mut wait: Local<f32>,
    time: Res<Time<Real>>,
) {
    let mut picked: Option<usize> = None;
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].iter().enumerate() {
        if keys.just_pressed(*key) && i < choice.offers.len() {
            picked = Some(i);
        }
    }
    if let Some(i) = clicked.0.take()
        && i < choice.offers.len()
    {
        picked = Some(i);
    }
    if auto.is_some() {
        *wait += time.delta_secs();
        if *wait > 1.2 {
            *wait = 0.0;
            picked = Some(rng.0.random_range(0..choice.offers.len()));
        }
    }

    let reroll = keys.just_pressed(KeyCode::KeyT) && progress.rerolls > 0;
    if reroll {
        progress.rerolls -= 1;
        *choice = make_choice(choice.level, &loadout, &mut rng);
    } else if let Some(i) = picked {
        apply(choice.offers[i], &mut loadout);
        progress.pending = progress.pending.saturating_sub(1);
        if progress.pending == 0 {
            next.set(GameState::Playing);
            return;
        }
        let level = progress.level + 1 - progress.pending;
        *choice = make_choice(level, &loadout, &mut rng);
        if choice.offers.is_empty() {
            next.set(GameState::Playing);
            return;
        }
    } else {
        return;
    }
    for e in &screens {
        commands.entity(e).despawn();
    }
    build_choice_ui(&mut commands, &choice, &loadout, &icons, progress.rerolls);
}
