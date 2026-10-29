//! Graphics quality presets. The game picks one from the GPU at startup
//! (integrated graphics get a lighter preset) and F9 cycles through them.
//!
//! The 3D world is drawn into an image that can be smaller than the window
//! (render scale), then stretched over the window by a plain screen camera
//! that also draws the HUD, so text and bars stay sharp at any scale.
//!
//! Test options: `SIEGE_QUALITY=low|medium|high` forces a preset, and
//! `SIEGE_GFX=scale=0.7,aa=low,bloom=0` overrides single settings on top of it.

use crate::Banner;
use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::picking::Pickable;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat};
use bevy::render::renderer::RenderAdapterInfo;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::PrimaryWindow;
use bevy::render::view::{ExtractedWindow, ViewTarget, create_surfaces};
use bevy::render::{Render, RenderApp, RenderSystems};

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        let forced = std::env::var("SIEGE_QUALITY").ok().and_then(|v| Preset::parse(&v));
        // Created here so the 3D camera can target it from its own startup system.
        let image = Image::new_target_texture(1, 1, TextureFormat::Rgba8UnormSrgb, None);
        let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        app.insert_resource(SceneImage(image))
            .insert_resource(Graphics { preset: forced.unwrap_or(Preset::High), decided: forced.is_some(), scale: 1.0 })
            .add_systems(Startup, spawn_screen)
            .add_systems(Update, (pick_for_gpu, cycle_with_key, apply, fit_scene_image).chain());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                release_window_views.in_set(RenderSystems::PrepareViews).before(create_surfaces),
            );
        }
    }
}

/// Fixes a crash when the window is minimized and restored on DirectX 12.
/// While minimized, Bevy stops extracting the camera but its old render
/// target keeps a view of the window's swapchain image. Resizing the
/// swapchain then fails ("window is in use") and the game quits. Bevy's own
/// cleanup only covers cameras that are still extracted, so drop every view
/// target whenever a window changes size. They are rebuilt the same frame.
fn release_window_views(
    mut commands: Commands,
    windows: Query<&ExtractedWindow>,
    targets: Query<Entity, With<ViewTarget>>,
) {
    if windows.iter().any(|w| w.size_changed || w.present_mode_changed) {
        for e in &targets {
            commands.entity(e).try_remove::<ViewTarget>();
        }
    }
}

/// The game's 3D camera (not the card-portrait cameras).
#[derive(Component)]
pub struct MainCamera;

/// The image the 3D camera draws into.
#[derive(Resource)]
pub struct SceneImage(pub Handle<Image>);

/// The camera that draws the scene image and the HUD into the window.
fn spawn_screen(mut commands: Commands, scene: Res<SceneImage>) {
    commands.spawn((
        Camera2d,
        Camera { order: 1, ..default() },
        IsDefaultUiCamera,
        Msaa::Off,
        Tonemapping::None,
    ));
    // Under every other UI node, stretched to the window (smoothly filtered).
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        ImageNode::new(scene.0.clone()),
        GlobalZIndex(i32::MIN),
        Pickable::IGNORE,
    ));
}

/// Keeps the scene image at the window size times the render scale.
fn fit_scene_image(
    gfx: Res<Graphics>,
    scene: Res<SceneImage>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(window) = windows.single() else { return };
    let size = window.physical_size();
    // Minimized: keep the last size.
    if size.x == 0 || size.y == 0 {
        return;
    }
    let want = (size.as_vec2() * gfx.scale).round().as_uvec2().max(UVec2::ONE);
    if images.get(&scene.0).is_some_and(|img| img.size() != want)
        && let Some(mut img) = images.get_mut(&scene.0)
    {
        img.resize(Extent3d { width: want.x, height: want.y, depth_or_array_layers: 1 });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Low,
    Medium,
    High,
}

impl Preset {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Low,
        }
    }

    fn settings(self) -> Settings {
        match self {
            Self::High => Settings { scale: 1.0, smaa: Some(SmaaPreset::High), bloom: true },
            Self::Medium => Settings { scale: 0.8, smaa: Some(SmaaPreset::Medium), bloom: true },
            Self::Low => Settings { scale: 0.67, smaa: Some(SmaaPreset::Low), bloom: false },
        }
    }
}

#[derive(Resource)]
pub struct Graphics {
    pub preset: Preset,
    /// False until the preset was chosen from the GPU (or forced).
    decided: bool,
    /// The 3D world's resolution relative to the window.
    scale: f32,
}

#[derive(Clone, Copy)]
struct Settings {
    /// The 3D world's resolution relative to the window.
    scale: f32,
    /// Edge smoothing (anti-aliasing).
    smaa: Option<SmaaPreset>,
    /// Glow around fire and bright effects.
    bloom: bool,
}

impl Settings {
    /// Applies the `SIEGE_GFX` test overrides.
    fn with_overrides(mut self) -> Self {
        let Ok(spec) = std::env::var("SIEGE_GFX") else {
            return self;
        };
        for pair in spec.split(',') {
            let Some((k, v)) = pair.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "aa" => {
                    self.smaa = match v {
                        "low" => Some(SmaaPreset::Low),
                        "medium" => Some(SmaaPreset::Medium),
                        "high" => Some(SmaaPreset::High),
                        _ => None,
                    }
                }
                "bloom" => self.bloom = v != "0",
                "scale" => self.scale = v.parse::<f32>().map_or(self.scale, |x| x.clamp(0.25, 1.0)),
                _ => {}
            }
        }
        self
    }
}

/// Integrated graphics share memory and power with the CPU: start lighter.
fn pick_for_gpu(mut gfx: ResMut<Graphics>, adapter: Option<Res<RenderAdapterInfo>>) {
    if gfx.decided {
        return;
    }
    let Some(adapter) = adapter else { return };
    // wgpu's DeviceType is not re-exported by Bevy, so match on its name.
    let kind = format!("{:?}", adapter.device_type);
    let preset = match kind.as_str() {
        "DiscreteGpu" => Preset::High,
        "Cpu" => Preset::Low,
        _ => Preset::Medium,
    };
    info!("graphics: {} ({kind}) -> {preset:?}", adapter.name);
    gfx.preset = preset;
    gfx.decided = true;
}

fn cycle_with_key(keys: Res<ButtonInput<KeyCode>>, mut gfx: ResMut<Graphics>, mut banner: ResMut<Banner>) {
    if keys.just_pressed(KeyCode::F9) {
        gfx.preset = gfx.preset.next();
        gfx.decided = true;
        banner.show(format!("Graphics: {:?}", gfx.preset), "F9 to change", 2.0);
    }
}

/// Puts the preset's effects on the camera whenever it changes.
fn apply(
    mut commands: Commands,
    mut gfx: ResMut<Graphics>,
    cams: Query<Entity, With<MainCamera>>,
    new_cams: Query<(), Added<MainCamera>>,
) {
    if !gfx.is_changed() && new_cams.is_empty() {
        return;
    }
    let s = gfx.preset.settings().with_overrides();
    let aa = s.smaa.map_or("off", |p| match p {
        SmaaPreset::Low => "low",
        SmaaPreset::Medium => "medium",
        SmaaPreset::High => "high",
        SmaaPreset::Ultra => "ultra",
    });
    info!("graphics: {:?} scale={} aa={aa} bloom={}", gfx.preset, s.scale, s.bloom);
    gfx.scale = s.scale;
    for e in &cams {
        let mut cam = commands.entity(e);
        match s.smaa {
            Some(preset) => {
                cam.insert(Smaa { preset });
            }
            None => {
                cam.remove::<Smaa>();
            }
        }
        if s.bloom {
            cam.insert(Bloom { intensity: 0.12, ..Bloom::NATURAL });
        } else {
            cam.remove::<Bloom>();
        }
    }
}
