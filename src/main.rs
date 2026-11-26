//! Rogue AI: a real-time fight through a server hall, in "3D pixel art".
//!
//! The 3D scene renders into a small 480x270 image, which is then scaled up
//! by a whole number with nearest-neighbour filtering. That gives real 3D
//! lighting, shadows and glow, but with chunky, crisp pixels. The HUD is
//! drawn pixel by pixel into a second image of the same size on top.
//!
//! Environment options:
//! - `MODEL=swarm` (or language, vision, reasoning, agent) to pick the model.
//! - `SEED=7` picks the hall layout (otherwise every fight is new).
//! - `DEMO=boss` starts at the boss with a few parts already bolted on.
//! - `AUTOPLAY=1` lets the game play itself (`ORBIT=0.3` turns the camera).
//! - `SHOT=path.png` plays by itself, saves a screenshot and quits
//!   (`SHOT_FRAME` picks the frame, default 150).
//! - `CLIP=folder` plays by itself and saves 90 frames for a short clip
//!   (`CLIP_START` picks the first frame, default 40).
//! - `WIN=960x540` sets the window size.

use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PrimaryWindow, WindowResized};

mod boss;
mod canvas;
mod fight;
mod hall;
mod hud;
mod input;
mod maze;
mod mech;
mod visuals;

use fight::{Family, Fight, Outcome};

/// Size of the low-res render. Everything you see is made of these pixels.
pub const RES: UVec2 = UVec2::new(480, 270);

/// Is the game playing itself (for demos, screenshots and clips)?
pub fn demo() -> bool {
    ["AUTOPLAY", "SHOT", "CLIP"]
        .iter()
        .any(|v| std::env::var(v).is_ok())
}

fn new_fight(family: Family) -> Fight {
    let mut f = Fight::new(family);
    if std::env::var("DEMO").is_ok_and(|d| d == "boss") {
        f.skip_to_boss();
    }
    f
}

/// The 3D camera that renders the arena.
#[derive(Component)]
pub struct GameCamera;

/// Sprites that show a low-res image scaled up to the window.
#[derive(Component)]
struct Upscaled;

fn main() {
    let family = match std::env::var("MODEL")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "vision" => Family::Vision,
        "reasoning" => Family::Reasoning,
        "agent" => Family::Agent,
        "swarm" => Family::Swarm,
        _ => Family::Language,
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rogue AI".into(),
                resolution: window_size().into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(new_fight(family))
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 2048 })
        .init_resource::<input::Input>()
        .init_resource::<visuals::CamRig>()
        .insert_resource(input::Captured(!demo()))
        .add_systems(
            Startup,
            (
                setup_cameras,
                visuals::setup,
                hall::setup,
                mech::setup,
                boss::setup,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                fit_to_window,
                input::read_input,
                input::cursor_capture,
                run_fight,
                hall::rebuild,
                visuals::camera,
                (hall::cutaway, hall::atmosphere),
                (mech::sync, boss::sync, visuals::sync),
                visuals::effects,
                visuals::particles,
                hud::draw,
                screenshot_and_quit,
            )
                .chain(),
        )
        .run();
}

/// Window size: 1920x1080, or `WIN=960x540` for small test captures.
fn window_size() -> (u32, u32) {
    std::env::var("WIN")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1920, 1080))
}

/// Whole-number scale that fits the low-res image in the window.
pub fn pixel_scale(win: &Window) -> f32 {
    (win.width() / RES.x as f32)
        .min(win.height() / RES.y as f32)
        .floor()
        .max(1.0)
}

fn setup_cameras(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(RES.x, RES.y, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);

    let mut hud = Image::new_fill(
        Extent3d {
            width: RES.x,
            height: RES.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    hud.sampler = ImageSampler::nearest();
    let hud_handle = images.add(hud);
    commands.insert_resource(hud::HudImage(hud_handle.clone()));

    // 3D camera rendering into the small image: an orthographic follow
    // camera that `visuals::camera` moves every frame.
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(hall::FOG),
            ..default()
        },
        RenderTarget::from(handle.clone()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 14.0,
            },
            near: 1.0,
            far: 140.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(30.0, 30.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        GameCamera,
        Msaa::Off,
        Hdr,
        Bloom {
            intensity: 0.25,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: hall::FOG,
            falloff: FogFalloff::Linear {
                start: 45.0,
                end: 60.0,
            },
            ..default()
        },
        AmbientLight {
            color: Color::srgb(0.5, 0.6, 1.0),
            brightness: 70.0,
            ..default()
        },
    ));

    // 2D camera: shows the small images scaled up to the window.
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        Msaa::Off,
        RenderLayers::layer(1),
    ));
    commands.spawn((Sprite::from_image(handle), Upscaled, RenderLayers::layer(1)));
    commands.spawn((
        Sprite::from_image(hud_handle),
        Transform::from_xyz(0.0, 0.0, 1.0),
        Upscaled,
        RenderLayers::layer(1),
    ));
}

/// Keep the upscale a whole number so every game pixel is a perfect square.
fn fit_to_window(
    mut resized: MessageReader<WindowResized>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sprites: Query<&mut Sprite, With<Upscaled>>,
    mut done_once: Local<bool>,
) {
    if resized.read().count() == 0 && *done_once {
        return;
    }
    let Ok(win) = windows.single() else { return };
    *done_once = true;
    let k = pixel_scale(win);
    for mut s in &mut sprites {
        s.custom_size = Some(RES.as_vec2() * k);
    }
}

fn run_fight(mut fight: ResMut<Fight>, inp: Res<input::Input>, time: Res<Time>) {
    if inp.restart {
        let fam = fight.family;
        *fight = new_fight(fam);
        return;
    }
    if fight.outcome != Outcome::Playing && inp.cycle_model {
        let next = fight.family.next();
        fight.set_family(next, true);
    }
    // Clamp the step so a slow frame can't tunnel shots through walls.
    let dt = time.delta_secs().min(1.0 / 20.0);
    fight.update(dt, &inp);
}

fn screenshot_and_quit(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    let f = *frame;
    // CLIP=dir saves every other frame for a short clip, then quits.
    if let Ok(dir) = std::env::var("CLIP") {
        let start = std::env::var("CLIP_START")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(40);
        let count = 90;
        if f >= start && f < start + count * 2 && (f - start).is_multiple_of(2) {
            let path = format!("{dir}/frame_{:04}.png", (f - start) / 2);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        if f == start + count * 2 + 30 {
            exit.write(AppExit::Success);
        }
        return;
    }
    let Ok(path) = std::env::var("SHOT") else {
        return;
    };
    let at: u32 = std::env::var("SHOT_FRAME")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150);
    if f == at {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if f == at + 50 {
        exit.write(AppExit::Success);
    }
}
