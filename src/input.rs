//! Keyboard + mouse and gamepad (Steam Deck / Xbox / PlayStation) input,
//! merged into one per-frame `Input` snapshot in world space.
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::fight::{Fight, NODE_POS};
use crate::{GameCamera, RES};

#[derive(Resource, Default, Clone)]
pub struct Input {
    /// Movement on the floor, `Vec2(x, z)`, length 0..1.
    pub mv: Vec2,
    /// Floor point being aimed at, `Vec2(x, z)`.
    pub aim: Option<Vec2>,
    /// Where the reticle sits, in low-res pixels (for the HUD).
    pub reticle: Option<Vec2>,
    pub fire: bool,
    /// Ability slots 1-4, true only on the frame they are pressed.
    pub ability: [bool; 4],
    pub ult: bool,
    pub cycle_model: bool,
    pub restart: bool,
    using_pad: bool,
    pad_aim: Vec2,
    last_cursor: Vec2,
}

const DEADZONE: f32 = 0.2;
/// Height of shots and bots above the floor; the mouse ray aims at this plane.
pub const AIM_HEIGHT: f32 = 0.5;

/// Turn the window cursor into a pixel of the low-res image.
pub fn cursor_to_pixel(win: &Window, cursor: Vec2) -> Vec2 {
    let k = crate::pixel_scale(win);
    let size = RES.as_vec2() * k;
    let origin = (Vec2::new(win.width(), win.height()) - size) / 2.0;
    (cursor - origin) / k
}

#[allow(clippy::too_many_arguments)]
pub fn read_input(
    mut inp: ResMut<Input>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    fight: Res<Fight>,
    time: Res<Time>,
) {
    let Ok((cam, cam_tf)) = cams.single() else {
        return;
    };
    // Screen directions projected onto the floor, so "up" on screen is "up"
    // whatever the camera angle is.
    let fwd = cam_tf.forward().xz().normalize_or_zero();
    let right = cam_tf.right().xz().normalize_or_zero();
    let to_floor = |screen: Vec2| right * screen.x + fwd * screen.y;

    // ---- keyboard + mouse
    let mut screen_mv = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        screen_mv.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        screen_mv.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        screen_mv.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        screen_mv.x += 1.0;
    }
    let mut fire = mouse.pressed(MouseButton::Left) || keys.pressed(KeyCode::Space);
    let mut ability = [
        keys.just_pressed(KeyCode::Digit1),
        keys.just_pressed(KeyCode::Digit2) || mouse.just_pressed(MouseButton::Right),
        keys.just_pressed(KeyCode::Digit3),
        keys.just_pressed(KeyCode::Digit4),
    ];
    let mut ult = keys.just_pressed(KeyCode::KeyQ);
    let mut cycle = keys.just_pressed(KeyCode::Tab);
    let mut restart = keys.just_pressed(KeyCode::KeyR);

    let mut cursor_px = None;
    if let Ok(win) = windows.single()
        && let Some(c) = win.cursor_position()
    {
        if (c - inp.last_cursor).length_squared() > 4.0 {
            inp.using_pad = false;
        }
        inp.last_cursor = c;
        cursor_px = Some(cursor_to_pixel(win, c));
    }

    // ---- gamepad (first one connected)
    if let Some(pad) = pads.iter().next() {
        let l = pad.left_stick();
        if l.length() > DEADZONE {
            screen_mv += l;
            inp.using_pad = true;
        }
        let r = pad.right_stick();
        if r.length() > 0.3 {
            inp.pad_aim = to_floor(r).normalize_or_zero();
            inp.using_pad = true;
            fire = true; // twin-stick: aiming fires
        }
        if pad.pressed(GamepadButton::RightTrigger2) {
            fire = true;
        }
        let jp = |b| pad.just_pressed(b);
        ability[0] |= jp(GamepadButton::South);
        ability[1] |= jp(GamepadButton::West);
        ability[2] |= jp(GamepadButton::North);
        ability[3] |= jp(GamepadButton::East);
        ult |= jp(GamepadButton::RightTrigger);
        cycle |= jp(GamepadButton::Select);
        restart |= jp(GamepadButton::Start);
        if [
            GamepadButton::South,
            GamepadButton::West,
            GamepadButton::North,
            GamepadButton::East,
            GamepadButton::RightTrigger,
            GamepadButton::RightTrigger2,
        ]
        .iter()
        .any(|b| pad.pressed(*b))
        {
            inp.using_pad = true;
        }
    }

    let mut mv = to_floor(screen_mv);
    if mv.length() > 1.0 {
        mv = mv.normalize();
    }

    let ppos = fight.player.pos;
    let (aim, reticle) = if inp.using_pad {
        let target = ppos + inp.pad_aim * 4.0;
        let px = cam
            .world_to_viewport(cam_tf, Vec3::new(target.x, AIM_HEIGHT, target.y))
            .ok();
        (Some(target), px)
    } else if let Some(px) = cursor_px {
        let hit = cam.viewport_to_world(cam_tf, px).ok().and_then(|ray| {
            ray.intersect_plane(Vec3::Y * AIM_HEIGHT, InfinitePlane3d::new(Vec3::Y))
                .map(|d| ray.get_point(d).xz())
        });
        (hit, Some(px))
    } else {
        (None, None)
    };

    *inp = Input {
        mv,
        aim,
        reticle,
        fire,
        ability,
        ult,
        cycle_model: cycle,
        restart,
        ..inp.clone()
    };

    // `AUTOPLAY=1` (and screenshots) let the AI play itself, for demos.
    if ["AUTOPLAY", "SHOT", "CLIP"].iter().any(|v| std::env::var(v).is_ok()) {
        autoplay(&mut inp, &fight, time.elapsed_secs(), cam, cam_tf);
    }
}

/// A simple demo pilot: strafe in a loop, shoot the nearest bot, use cards.
fn autoplay(inp: &mut Input, fight: &Fight, t: f32, cam: &Camera, cam_tf: &GlobalTransform) {
    let p = fight.player.pos;
    let target = fight
        .enemies
        .iter()
        .min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))
        .map(|e| e.pos)
        .unwrap_or(NODE_POS);
    let home = Vec2::new(-3.0 + 2.5 * (t * 0.5).cos(), 1.0 + 2.5 * (t * 0.7).sin());
    let to_home = home - p;
    inp.mv = if to_home.length() > 0.3 {
        to_home.normalize() * 0.8
    } else {
        Vec2::ZERO
    };
    inp.aim = Some(target);
    inp.reticle = cam
        .world_to_viewport(cam_tf, Vec3::new(target.x, AIM_HEIGHT, target.y))
        .ok();
    inp.fire = true;
    let near = fight
        .enemies
        .iter()
        .filter(|e| e.pos.distance(p) < 4.0)
        .count();
    for (i, ab) in fight.abilities.iter().enumerate() {
        let useful = match i {
            0 => near >= 3,
            1 => !fight.enemies.is_empty(),
            _ => true,
        };
        inp.ability[i] = useful && ab.cd <= 0.0 && fight.player.heat + ab.heat < 90.0;
    }
    inp.ult = fight.player.ult >= 1.0;
}
