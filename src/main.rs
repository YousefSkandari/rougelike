//! Rogue AI: "3D pixel art" look test.
//!
//! The 3D scene renders into a small 480x270 image, which is then scaled up
//! by a whole number with nearest-neighbour filtering. That gives real 3D
//! lighting, shadows and glow, but with chunky, crisp pixels.
//!
//! Set `SHOT=path.png` to save a screenshot after a couple of seconds and quit.

use bevy::camera::Hdr;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResized;

mod scene;

/// Size of the low-res render. Everything you see is made of these pixels.
pub const RES: UVec2 = UVec2::new(384, 216);

#[derive(Resource)]
struct PixelTarget(Handle<Image>);

#[derive(Component)]
struct Upscaled;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rogue AI".into(),
                resolution: (1920u32, 1080u32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (setup_cameras, scene::build).chain())
        .add_systems(
            Update,
            (
                fit_to_window,
                scene::animate,
                scene::bob,
                screenshot_and_quit,
            ),
        )
        .run();
}

fn setup_cameras(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(RES.x, RES.y, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    commands.insert_resource(PixelTarget(handle.clone()));

    // 3D camera rendering into the small image. Isometric diorama view by
    // default; VIEW=angled gives a Hades-style angled perspective instead.
    let iso = std::env::var("VIEW").map(|v| v != "angled").unwrap_or(true);
    let (projection, view) = if iso {
        (
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: 17.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(15.0, 15.5, 15.0).looking_at(Vec3::new(-0.5, 0.0, -0.5), Vec3::Y),
        )
    } else {
        (
            Projection::Perspective(PerspectiveProjection {
                fov: 34f32.to_radians(),
                ..default()
            }),
            Transform::from_xyz(0.0, 16.0, 15.0).looking_at(Vec3::new(0.0, 0.0, 0.6), Vec3::Y),
        )
    };
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(scene::FOG),
            ..default()
        },
        RenderTarget::from(handle.clone()),
        projection,
        view,
        Msaa::Off,
        Hdr,
        Bloom {
            intensity: 0.22,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: scene::FOG,
            falloff: FogFalloff::Linear {
                start: 22.0,
                end: 42.0,
            },
            ..default()
        },
        AmbientLight {
            color: Color::srgb(0.55, 0.65, 1.0),
            brightness: 90.0,
            ..default()
        },
    ));

    // 2D camera: shows the small image scaled up to the window.
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
}

/// Keep the upscale a whole number so every game pixel is a perfect square.
fn fit_to_window(
    mut resized: MessageReader<WindowResized>,
    windows: Query<&Window>,
    mut sprites: Query<&mut Sprite, With<Upscaled>>,
    mut done_once: Local<bool>,
) {
    if resized.read().count() == 0 && *done_once {
        return;
    }
    let Ok(win) = windows.single() else { return };
    *done_once = true;
    let k = (win.width() / RES.x as f32)
        .min(win.height() / RES.y as f32)
        .floor()
        .max(1.0);
    for mut s in &mut sprites {
        s.custom_size = Some(RES.as_vec2() * k);
    }
}

fn screenshot_and_quit(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(path) = std::env::var("SHOT") else {
        return;
    };
    *frame += 1;
    if *frame == 150 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if *frame == 200 {
        exit.write(AppExit::Success);
    }
}
