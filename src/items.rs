//! Weapon modules, artifacts, castle stages, and the stats they add up to.

use crate::art::Variant;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

pub const MAX_MODULE_LEVEL: u32 = 5;
/// Level-up that grows the castle to stage 2 and stage 3.
pub const STAGE_LEVELS: [u32; 2] = [6, 14];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mount {
    Top,
    Front,
    Back,
    Side,
}

impl Mount {
    pub fn label(self) -> &'static str {
        match self {
            Mount::Top => "TOP",
            Mount::Front => "FRONT",
            Mount::Back => "BACK",
            Mount::Side => "SIDES",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ModuleKind {
    ArrowLoft,
    Mortar,
    Ram,
    MineLayer,
    Broadside,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 5] = [
        ModuleKind::ArrowLoft,
        ModuleKind::Mortar,
        ModuleKind::Ram,
        ModuleKind::MineLayer,
        ModuleKind::Broadside,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModuleKind::ArrowLoft => "Arrow Loft",
            ModuleKind::Mortar => "Bombard",
            ModuleKind::Ram => "Spiked Ram",
            ModuleKind::MineLayer => "Mine Chute",
            ModuleKind::Broadside => "Broadside Guns",
        }
    }

    pub fn mount(self) -> Mount {
        match self {
            ModuleKind::ArrowLoft | ModuleKind::Mortar => Mount::Top,
            ModuleKind::Ram => Mount::Front,
            ModuleKind::MineLayer => Mount::Back,
            ModuleKind::Broadside => Mount::Side,
        }
    }

    /// (what it does on its own, what its ability key does)
    pub fn describe(self) -> (&'static str, &'static str) {
        match self {
            ModuleKind::ArrowLoft => (
                "Archers fire volleys of arrows at the nearest enemies.",
                "Arrow Storm: arrows rain down all around the castle.",
            ),
            ModuleKind::Mortar => (
                "Lobs explosive shells into enemy crowds.",
                "Carpet Shelling: a line of shells behind the castle.",
            ),
            ModuleKind::Ram => (
                "Driving into enemies hurts them. Faster = more damage.",
                "Charge: burst of speed with triple ram damage.",
            ),
            ModuleKind::MineLayer => (
                "Drops mines behind the castle as it drives.",
                "Minefield: scatters a dozen mines at once.",
            ),
            ModuleKind::Broadside => (
                "Cannons fire to both sides, piercing enemy lines.",
                "Full Broadside: a wide volley from both sides.",
            ),
        }
    }

    /// Seconds between automatic attacks at level 1.
    pub fn auto_cooldown(self) -> f32 {
        match self {
            ModuleKind::ArrowLoft => 1.0,
            ModuleKind::Mortar => 2.2,
            ModuleKind::Ram => 0.0,
            ModuleKind::MineLayer => 1.3,
            ModuleKind::Broadside => 1.7,
        }
    }

    pub fn ability_cooldown(self) -> f32 {
        match self {
            ModuleKind::ArrowLoft => 12.0,
            ModuleKind::Mortar => 14.0,
            ModuleKind::Ram => 9.0,
            ModuleKind::MineLayer => 12.0,
            ModuleKind::Broadside => 9.0,
        }
    }

    /// Base damage per hit at level 1.
    pub fn damage(self) -> f32 {
        match self {
            ModuleKind::ArrowLoft => 9.0,
            ModuleKind::Mortar => 26.0,
            ModuleKind::Ram => 30.0,
            ModuleKind::MineLayer => 34.0,
            ModuleKind::Broadside => 22.0,
        }
    }

    pub fn range(self) -> f32 {
        match self {
            ModuleKind::ArrowLoft => 30.0,
            ModuleKind::Mortar => 36.0,
            ModuleKind::Broadside => 32.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArtifactKind {
    Whetstone,
    GreasedAxles,
    Lodestone,
    ReinforcedHull,
    QuickHands,
    BlackPowder,
    HawkEye,
    TinkersKit,
    Poultice,
    GoldenFleece,
    NitroTank,
    IronSpikes,
}

impl ArtifactKind {
    pub const ALL: [ArtifactKind; 12] = [
        ArtifactKind::Whetstone,
        ArtifactKind::GreasedAxles,
        ArtifactKind::Lodestone,
        ArtifactKind::ReinforcedHull,
        ArtifactKind::QuickHands,
        ArtifactKind::BlackPowder,
        ArtifactKind::HawkEye,
        ArtifactKind::TinkersKit,
        ArtifactKind::Poultice,
        ArtifactKind::GoldenFleece,
        ArtifactKind::NitroTank,
        ArtifactKind::IronSpikes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ArtifactKind::Whetstone => "Whetstone",
            ArtifactKind::GreasedAxles => "Greased Axles",
            ArtifactKind::Lodestone => "Lodestone",
            ArtifactKind::ReinforcedHull => "Reinforced Hull",
            ArtifactKind::QuickHands => "Quick Hands",
            ArtifactKind::BlackPowder => "Black Powder",
            ArtifactKind::HawkEye => "Hawk Eye",
            ArtifactKind::TinkersKit => "Tinker's Kit",
            ArtifactKind::Poultice => "Herbal Poultice",
            ArtifactKind::GoldenFleece => "Golden Fleece",
            ArtifactKind::NitroTank => "Nitro Tank",
            ArtifactKind::IronSpikes => "Iron Spikes",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            ArtifactKind::Whetstone => "+20% damage for all modules",
            ArtifactKind::GreasedAxles => "+12% top speed, +15% turning",
            ArtifactKind::Lodestone => "+60% coin pickup range",
            ArtifactKind::ReinforcedHull => "+30% max health",
            ArtifactKind::QuickHands => "Modules attack 18% faster",
            ArtifactKind::BlackPowder => "+30% explosion size",
            ArtifactKind::HawkEye => "+20% attack range",
            ArtifactKind::TinkersKit => "Abilities recharge 20% faster",
            ArtifactKind::Poultice => "Repair 0.8% of max health per second",
            ArtifactKind::GoldenFleece => "+25% XP from everything",
            ArtifactKind::NitroTank => "+50% nitro and nitro refill",
            ArtifactKind::IronSpikes => "Enemies touching the walls take 20 damage/s",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnedModule {
    pub kind: ModuleKind,
    pub level: u32,
}

/// Everything the castle has picked up this run.
#[derive(Resource, Default, Clone)]
pub struct Loadout {
    /// 0, 1 or 2.
    pub stage: usize,
    pub variant: Variant,
    pub modules: Vec<OwnedModule>,
    pub artifacts: HashMap<ArtifactKind, u32>,
}

impl Loadout {
    pub fn starting() -> Self {
        Self {
            stage: 0,
            variant: Variant::Base,
            modules: vec![OwnedModule { kind: ModuleKind::ArrowLoft, level: 1 }],
            artifacts: HashMap::default(),
        }
    }

    pub fn max_slots(&self) -> usize {
        2 + self.stage
    }

    pub fn module(&self, kind: ModuleKind) -> Option<&OwnedModule> {
        self.modules.iter().find(|m| m.kind == kind)
    }

    pub fn stacks(&self, a: ArtifactKind) -> u32 {
        self.artifacts.get(&a).copied().unwrap_or(0)
    }
}

/// Final numbers after stage, variant and artifacts.
#[derive(Resource, Clone, Debug, Default)]
pub struct Stats {
    pub max_hp: f32,
    pub speed: f32,
    pub turn: f32,
    pub nitro_max: f32,
    pub nitro_regen: f32,
    pub magnet: f32,
    pub radius: f32,
    pub mass: f32,
    pub damage: f32,
    pub rate: f32,
    pub range: f32,
    pub area: f32,
    pub ability_cd: f32,
    pub regen: f32,
    pub xp_mult: f32,
    pub spikes_dps: f32,
}

pub struct StageInfo {
    pub name: &'static str,
    pub max_hp: f32,
    pub speed: f32,
    pub turn: f32,
    pub nitro_max: f32,
    pub nitro_regen: f32,
    pub magnet: f32,
    pub radius: f32,
    pub mass: f32,
}

pub fn stage_info(stage: usize, variant: Variant) -> StageInfo {
    let (name, max_hp, speed, turn, nitro_max, nitro_regen, magnet, radius, mass) = match (stage, variant) {
        (0, _) => ("Wagon Keep", 150.0, 13.0, 2.6, 100.0, 12.0, 12.0, 3.2, 1.0),
        (1, Variant::Movement) => ("Outrider Fort", 190.0, 17.0, 2.9, 140.0, 16.0, 15.0, 4.8, 1.6),
        (1, _) => ("Bulwark Fort", 260.0, 13.5, 2.3, 100.0, 12.0, 17.0, 4.8, 3.0),
        (_, Variant::Movement) => ("Thunder Citadel", 400.0, 20.0, 2.5, 180.0, 20.0, 19.0, 7.0, 3.0),
        (_, _) => ("Iron Citadel", 560.0, 14.0, 2.0, 100.0, 12.0, 22.0, 7.0, 6.0),
    };
    StageInfo { name, max_hp, speed, turn, nitro_max, nitro_regen, magnet, radius, mass }
}

pub fn compute_stats(l: &Loadout) -> Stats {
    let s = stage_info(l.stage, l.variant);
    let k = |a| l.stacks(a) as f32;
    Stats {
        max_hp: s.max_hp * (1.0 + 0.30 * k(ArtifactKind::ReinforcedHull)),
        speed: s.speed * (1.0 + 0.12 * k(ArtifactKind::GreasedAxles)),
        turn: s.turn * (1.0 + 0.15 * k(ArtifactKind::GreasedAxles)),
        nitro_max: s.nitro_max * (1.0 + 0.5 * k(ArtifactKind::NitroTank)),
        nitro_regen: s.nitro_regen * (1.0 + 0.5 * k(ArtifactKind::NitroTank)),
        magnet: s.magnet * (1.0 + 0.6 * k(ArtifactKind::Lodestone)),
        radius: s.radius,
        mass: s.mass,
        damage: 1.0 + 0.20 * k(ArtifactKind::Whetstone),
        rate: 1.0 + 0.18 * k(ArtifactKind::QuickHands),
        range: 1.0 + 0.20 * k(ArtifactKind::HawkEye),
        area: 1.0 + 0.30 * k(ArtifactKind::BlackPowder),
        ability_cd: 0.8f32.powf(k(ArtifactKind::TinkersKit)),
        regen: 0.003 + 0.008 * k(ArtifactKind::Poultice),
        xp_mult: 1.0 + 0.25 * k(ArtifactKind::GoldenFleece),
        spikes_dps: 20.0 * k(ArtifactKind::IronSpikes),
    }
}

/// Damage multiplier from a module's level.
pub fn level_mult(level: u32) -> f32 {
    1.0 + 0.3 * (level.saturating_sub(1)) as f32
}
