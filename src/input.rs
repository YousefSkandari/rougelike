//! Keyboard + mouse and gamepad (Steam Deck / Xbox / PlayStation) input,
//! merged into one per-frame `Input` snapshot.
//!
//! You only move: your AI aims and shoots by itself. Moving the mouse turns
//! the camera around you (the game hides the cursor and holds it; Esc lets
//! it go, a click takes it back). The wheel zooms. On a gamepad the left
//! stick moves, the right stick turns the camera and the bumpers zoom.
//! When you level up, pick a part with 1-3, A/D and Space, a click, or the
//! d-pad and A.
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::fight::{Fight, Outcome};
use crate::visuals::CamRig;

#[derive(Resource, Default, Clone)]
pub struct Input {
    /// Movement on the floor, `Vec2(x, z)`, length 0..1.
    pub mv: Vec2,
    /// Camera turn this frame: (yaw, pitch) in radians.
    pub orbit: Vec2,
    /// Zoom steps this frame; negative zooms in.
    pub zoom: f32,
    /// A level-up card picked directly (by key or click).
    pub pick: Option<usize>,
    /// Move the level-up selection left (-1) or right (+1).
    pub pick_move: i32,
    /// Take the selected level-up card.
    pub pick_confirm: bool,
    /// Which level-up card the mouse is over.
    pub hover: Option<usize>,
    pub cycle_model: bool,
    pub restart: bool,
    /// The mouse, in low-res pixels, when the game isn't holding it.
    pub pointer: Option<Vec2>,
}

/// Whether the game holds the mouse (hidden, turning the camera).
#[derive(Resource)]
pub struct Captured(pub bool);

const DEADZONE: f32 = 0.2;
const MOUSE_YAW: f32 = 0.005;
const MOUSE_PITCH: f32 = 0.003;

/// Turn the window cursor into a pixel of the low-res image.
pub fn cursor_to_pixel(win: &Window, cursor: Vec2) -> Vec2 {
    let k = crate::pixel_scale(win);
    let size = crate::RES.as_vec2() * k;
    let origin = (Vec2::new(win.width(), win.height()) - size) / 2.0;
    (cursor - origin) / k
}

#[allow(clippy::too_many_arguments)]
pub fn read_input(
    mut inp: ResMut<Input>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    pads: Query<&Gamepad>,
    windows: Query<&Window, With<PrimaryWindow>>,
    captured: Res<Captured>,
    rig: Res<CamRig>,
    fight: Res<Fight>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut n = Input::default();

    // ---- moving: "up" on screen is "up" whatever the camera angle
    let mut screen = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        screen.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        screen.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        screen.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        screen.x += 1.0;
    }

    // ---- the camera: mouse while the game holds it (or right button held)
    if captured.0 || mouse.pressed(MouseButton::Right) {
        n.orbit = Vec2::new(-motion.delta.x * MOUSE_YAW, motion.delta.y * MOUSE_PITCH);
    }
    n.zoom = -match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if keys.pressed(KeyCode::KeyQ) {
        n.orbit.x += 2.0 * dt;
    }
    if keys.pressed(KeyCode::KeyE) {
        n.orbit.x -= 2.0 * dt;
    }

    // ---- level-up picks
    let choosing = fight.choice.is_some();
    if choosing {
        for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
            .iter()
            .enumerate()
        {
            if keys.just_pressed(*k) {
                n.pick = Some(i);
            }
        }
        if keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            n.pick_move -= 1;
        }
        if keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            n.pick_move += 1;
        }
        n.pick_confirm = keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]);
    }
    n.cycle_model = keys.just_pressed(KeyCode::Tab);
    n.restart = keys.just_pressed(KeyCode::KeyR);

    // ---- the mouse pointer, when it's free: hover and click the cards
    if (!captured.0 || choosing || fight.outcome != Outcome::Playing)
        && let Ok(win) = windows.single()
            && let Some(c) = win.cursor_position()
        {
            let px = cursor_to_pixel(win, c);
            n.pointer = Some(px);
            if choosing {
                let p = px.as_ivec2();
                n.hover = (0..3).find(|i| crate::hud::choice_card(*i).contains(p));
                if mouse.just_pressed(MouseButton::Left) {
                    n.pick = n.hover;
                }
            }
        }

    // ---- gamepad (first one connected)
    if let Some(pad) = pads.iter().next() {
        let l = pad.left_stick();
        if l.length() > DEADZONE {
            screen += l;
        }
        let r = pad.right_stick();
        if r.length() > DEADZONE {
            n.orbit += Vec2::new(-r.x * 2.4, -r.y * 1.4) * dt;
        }
        if pad.pressed(GamepadButton::LeftTrigger) {
            n.zoom += 3.0 * dt;
        }
        if pad.pressed(GamepadButton::RightTrigger) {
            n.zoom -= 3.0 * dt;
        }
        if choosing {
            if pad.just_pressed(GamepadButton::DPadLeft) {
                n.pick_move -= 1;
            }
            if pad.just_pressed(GamepadButton::DPadRight) {
                n.pick_move += 1;
            }
            n.pick_confirm |= pad.just_pressed(GamepadButton::South);
        }
        n.cycle_model |= pad.just_pressed(GamepadButton::Select);
        n.restart |= pad.just_pressed(GamepadButton::Start);
    }

    let (fwd, right) = rig.ground_axes();
    n.mv = (right * screen.x + fwd * screen.y).clamp_length_max(1.0);

    // `AUTOPLAY=1` (and screenshots) let the AI play itself, for demos.
    if crate::demo() {
        let auto = fight.autopilot();
        n.mv = auto.mv;
        n.pick_confirm = auto.pick_confirm;
        n.pointer = None;
        n.orbit = Vec2::ZERO;
        if let Ok(spin) = std::env::var("ORBIT") {
            n.orbit.x = spin.parse::<f32>().unwrap_or(0.0) * dt;
        }
    }
    *inp = n;
}

/// Hold the mouse while you play, so moving it turns the camera; let it go
/// for menus, when you press Esc, or when the window loses focus.
pub fn cursor_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    fight: Res<Fight>,
    mut captured: ResMut<Captured>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(mut opts) = cursor.single_mut() else {
        return;
    };
    let focused = windows.single().map(|w| w.focused).unwrap_or(false);
    if keys.just_pressed(KeyCode::Escape) {
        captured.0 = false;
    }
    let menu = fight.choice.is_some() || fight.outcome != Outcome::Playing;
    if !captured.0 && !menu && focused && mouse.just_pressed(MouseButton::Left) {
        captured.0 = true;
    }
    let hold = captured.0 && !menu && focused && !crate::demo();
    let (grab, visible) = if hold {
        (CursorGrabMode::Locked, false)
    } else {
        (CursorGrabMode::None, true)
    };
    if opts.grab_mode != grab {
        opts.grab_mode = grab;
    }
    if opts.visible != visible {
        opts.visible = visible;
    }
}
