//! Your AI's body. It starts as a spark of light and grows into a chunky
//! hardware mech, and every part you pick is bolted on where you can see
//! it: barrels on the gun, RAM sticks and GPU cards on the back, fans that
//! spin faster as the room heats up, hard drives on the legs, and drones
//! that sit on your body until there's something to attack.
//!
//! Forms: SPARK (no parts) > FRAME (1-2) > CHASSIS (3-4) > MECH (5+).
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::fight::{CardKind, Fight, Form, Outcome};
use crate::hall::floor_y;
use crate::visuals::{Kit, block, glow, metal, pivot};

#[derive(Clone, Copy, PartialEq)]
enum Rule {
    Only(Form),
    From(Form),
    Range(Form, Form),
    Barrel(u8),
    Ram(u8),
    Fan(u8),
    Drive(u8),
    Card(u8),
}

impl Rule {
    fn on(self, f: &Fight) -> bool {
        let form = f.form();
        let k = f.player.kit;
        match self {
            Rule::Only(x) => form == x,
            Rule::From(x) => form >= x,
            Rule::Range(a, b) => form >= a && form <= b,
            Rule::Barrel(n) => k.barrels >= n,
            Rule::Ram(n) => k.ram >= n,
            Rule::Fan(n) => k.cooling >= n,
            Rule::Drive(n) => k.drives >= n,
            Rule::Card(n) => f.cards.len() >= n as usize,
        }
    }
}

fn form_index(f: Form) -> usize {
    match f {
        Form::Spark => 0,
        Form::Frame => 1,
        Form::Chassis => 2,
        Form::Mech => 3,
    }
}

/// A part of the body that shows up when its rule says so, with a pop.
#[derive(Component)]
pub struct Part {
    rule: Rule,
    size: Vec3,
    /// Where it sits for each form (spark, frame, chassis, mech).
    at: [Vec3; 4],
    pop: f32,
    shown: bool,
}

#[derive(Component)]
pub struct Rotor {
    axis: Vec3,
}

struct DroneVis {
    root: Entity,
    pos: Vec3,
}

#[derive(Resource)]
pub struct MechVis {
    root: Entity,
    hips: Entity,
    torso: Entity,
    thighs: [Entity; 2],
    knees: [Entity; 2],
    core: Entity,
    shell: Entity,
    halo: Entity,
    sparklets: Vec<Entity>,
    muzzle: Entity,
    light: Entity,
    /// A ring of light on the floor round your feet, so you never lose
    /// yourself among the racks.
    ground: Entity,
    ground_mat: Handle<StandardMaterial>,
    fam: Handle<StandardMaterial>,
    shell_mat: Handle<StandardMaterial>,
    rgb: Handle<StandardMaterial>,
    hub: Handle<StandardMaterial>,
    cards: [Handle<StandardMaterial>; 4],
    drones: Vec<DroneVis>,
    drone_kit: (Handle<StandardMaterial>, Handle<StandardMaterial>),
    yaw: f32,
    spin: f32,
    scale: f32,
    family: Option<crate::fight::Family>,
}

/// Dock points for drones, in the torso's space, per form.
fn dock(i: usize, form: usize) -> Vec3 {
    let s = if i.is_multiple_of(2) { -1.0 } else { 1.0 };
    if i < 2 {
        [
            Vec3::new(s * 0.12, 0.3, 0.2),
            Vec3::new(s * 0.12, 0.62, 0.2),
            Vec3::new(s * 0.13, 0.67, 0.22),
            Vec3::new(s * 0.17, 0.86, 0.27),
        ][form]
    } else {
        [
            Vec3::new(s * 0.2, 0.0, 0.0),
            Vec3::new(s * 0.27, 0.05, 0.06),
            Vec3::new(s * 0.31, 0.05, 0.06),
            Vec3::new(s * 0.38, 0.05, 0.08),
        ][form]
    }
}

pub fn setup(
    mut commands: Commands,
    kit: Res<Kit>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let c = &mut commands;
    // a touch of glow keeps the armour bright orange even in the boss's red light
    let orange = mats.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.42, 0.08),
        emissive: LinearRgba::rgb(0.22, 0.07, 0.01),
        perceptual_roughness: 0.5,
        metallic: 0.15,
        ..default()
    });
    let orange_dark = metal(&mut mats, Color::srgb(0.5, 0.18, 0.04), 0.5);
    let steel = metal(&mut mats, Color::srgb(0.45, 0.48, 0.54), 0.3);
    let gunmetal = metal(&mut mats, Color::srgb(0.3, 0.33, 0.38), 0.35);
    let dark = metal(&mut mats, Color::srgb(0.07, 0.075, 0.09), 0.55);
    let silver = mats.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.8, 0.84),
        perceptual_roughness: 0.25,
        metallic: 0.85,
        ..default()
    });
    let pcb = metal(&mut mats, Color::srgb(0.07, 0.36, 0.15), 0.6);
    let pcb_dark = metal(&mut mats, Color::srgb(0.05, 0.06, 0.08), 0.5);
    let led = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.3, 4.0, 1.0),
        ..default()
    });
    let fam = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(1.0, 4.0, 6.0),
        ..default()
    });
    let shell_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.8, 1.0, 0.25),
        emissive: LinearRgba::rgb(0.1, 0.4, 0.6),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let rgb = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(3.0, 0.5, 3.0),
        ..default()
    });
    let hub = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.6, 3.0, 4.0),
        ..default()
    });
    let cards = CardKind::ALL.map(|k| {
        mats.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: glow(k.color(), 3.0),
            ..default()
        })
    });

    let root = pivot(c, None, Vec3::ZERO);
    let thighs = [-1.0f32, 1.0].map(|s| pivot(c, Some(root), Vec3::new(s * 0.15, 0.6, 0.0)));
    let knees = thighs.map(|t| pivot(c, Some(t), Vec3::new(0.0, -0.3, 0.0)));
    let hips = pivot(c, Some(root), Vec3::new(0.0, 0.6, 0.0));
    let torso = pivot(c, Some(hips), Vec3::new(0.0, 0.08, 0.0));

    let all = |p: Vec3| [p; 4];
    let add = |c: &mut Commands,
               parent: Entity,
               size: Vec3,
               at: [Vec3; 4],
               mat: &Handle<StandardMaterial>,
               rule: Rule,
               line: f32| {
        let e = block(c, &kit, Some(parent), size, at[1], mat, line);
        c.entity(e).insert((
            Part {
                rule,
                size,
                at,
                pop: 1.0,
                shown: false,
            },
            Visibility::Hidden,
        ));
        e
    };
    use Form::*;
    let v = Vec3::new;

    // ---- legs
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (th, kn) = (thighs[i], knees[i]);
        add(
            c,
            th,
            v(0.1, 0.3, 0.1),
            all(v(0.0, -0.15, 0.0)),
            &steel,
            Rule::From(Frame),
            0.04,
        );
        add(
            c,
            th,
            v(0.17, 0.22, 0.19),
            all(v(0.0, -0.13, 0.0)),
            &gunmetal,
            Rule::Only(Chassis),
            0.04,
        );
        add(
            c,
            th,
            v(0.22, 0.26, 0.24),
            all(v(0.0, -0.13, 0.0)),
            &orange,
            Rule::From(Mech),
            0.05,
        );
        add(
            c,
            kn,
            v(0.09, 0.3, 0.09),
            all(v(0.0, -0.15, 0.0)),
            &steel,
            Rule::From(Frame),
            0.04,
        );
        add(
            c,
            kn,
            v(0.18, 0.24, 0.2),
            all(v(0.0, -0.14, 0.01)),
            &gunmetal,
            Rule::Only(Chassis),
            0.04,
        );
        add(
            c,
            kn,
            v(0.24, 0.3, 0.27),
            all(v(0.0, -0.13, 0.0)),
            &orange,
            Rule::From(Mech),
            0.05,
        );
        add(
            c,
            kn,
            v(0.14, 0.12, 0.08),
            all(v(0.0, 0.0, -0.15)),
            &orange_dark,
            Rule::From(Mech),
            0.04,
        );
        add(
            c,
            kn,
            v(0.15, 0.07, 0.26),
            all(v(0.0, -0.28, -0.04)),
            &gunmetal,
            Rule::Range(Frame, Chassis),
            0.04,
        );
        add(
            c,
            kn,
            v(0.26, 0.1, 0.38),
            all(v(0.0, -0.26, -0.05)),
            &dark,
            Rule::From(Mech),
            0.05,
        );
        // hard drives: thighs first, then shins
        let n = i as u8 + 1;
        let thigh_at = [
            v(s * 0.08, -0.15, 0.0),
            v(s * 0.08, -0.15, 0.0),
            v(s * 0.115, -0.13, 0.0),
            v(s * 0.14, -0.13, 0.0),
        ];
        let shin_at = [
            v(s * 0.075, -0.15, 0.0),
            v(s * 0.075, -0.15, 0.0),
            v(s * 0.12, -0.14, 0.01),
            v(s * 0.145, -0.13, 0.0),
        ];
        for (pv, at, k) in [(th, thigh_at, n), (kn, shin_at, n + 2)] {
            add(
                c,
                pv,
                v(0.06, 0.15, 0.22),
                at,
                &silver,
                Rule::Drive(k),
                0.04,
            );
            let led_at = at.map(|p| p + v(s * 0.032, 0.05, -0.07));
            add(
                c,
                pv,
                v(0.012, 0.025, 0.025),
                led_at,
                &led,
                Rule::Drive(k),
                0.0,
            );
        }
    }

    // ---- hips
    add(
        c,
        hips,
        v(0.36, 0.12, 0.2),
        all(v(0.0, 0.0, 0.0)),
        &gunmetal,
        Rule::From(Frame),
        0.04,
    );
    add(
        c,
        hips,
        v(0.36, 0.16, 0.06),
        all(v(0.0, -0.04, -0.14)),
        &orange,
        Rule::From(Mech),
        0.04,
    );
    add(
        c,
        hips,
        v(0.36, 0.16, 0.06),
        all(v(0.0, -0.04, 0.14)),
        &orange,
        Rule::From(Mech),
        0.04,
    );

    // ---- torso: a bare frame, then grey plating, then orange armour
    add(
        c,
        torso,
        v(0.09, 0.36, 0.09),
        all(v(0.0, 0.18, 0.03)),
        &gunmetal,
        Rule::Only(Frame),
        0.04,
    );
    for y in [0.2f32, 0.28, 0.36] {
        add(
            c,
            torso,
            v(0.34, 0.035, 0.18),
            all(v(0.0, y, 0.0)),
            &steel,
            Rule::Only(Frame),
            0.03,
        );
    }
    add(
        c,
        torso,
        v(0.56, 0.06, 0.08),
        all(v(0.0, 0.42, 0.02)),
        &gunmetal,
        Rule::Only(Frame),
        0.04,
    );
    add(
        c,
        torso,
        v(0.3, 0.36, 0.04),
        all(v(0.0, 0.26, 0.12)),
        &dark,
        Rule::Only(Frame),
        0.03,
    );
    add(
        c,
        torso,
        v(0.46, 0.36, 0.32),
        all(v(0.0, 0.26, 0.0)),
        &gunmetal,
        Rule::Only(Chassis),
        0.05,
    );
    add(
        c,
        torso,
        v(0.3, 0.1, 0.03),
        all(v(0.0, 0.16, -0.17)),
        &dark,
        Rule::Only(Chassis),
        0.0,
    );
    add(
        c,
        torso,
        v(0.58, 0.42, 0.38),
        all(v(0.0, 0.28, 0.0)),
        &orange,
        Rule::From(Mech),
        0.05,
    );
    add(
        c,
        torso,
        v(0.44, 0.15, 0.05),
        all(v(0.0, 0.37, -0.205)),
        &dark,
        Rule::From(Mech),
        0.0,
    );
    add(
        c,
        torso,
        v(0.38, 0.12, 0.3),
        all(v(0.0, 0.03, 0.0)),
        &dark,
        Rule::From(Mech),
        0.04,
    );
    add(
        c,
        torso,
        v(0.36, 0.06, 0.3),
        all(v(0.0, 0.51, 0.02)),
        &dark,
        Rule::From(Mech),
        0.0,
    );

    // ---- head: a sensor, then a small head, then a helmet with a visor
    add(
        c,
        torso,
        v(0.14, 0.1, 0.14),
        all(v(0.0, 0.5, 0.0)),
        &gunmetal,
        Rule::Only(Frame),
        0.04,
    );
    add(
        c,
        torso,
        v(0.06, 0.03, 0.02),
        all(v(0.0, 0.5, -0.065)),
        &fam,
        Rule::Only(Frame),
        0.0,
    );
    add(
        c,
        torso,
        v(0.2, 0.15, 0.2),
        all(v(0.0, 0.52, 0.0)),
        &gunmetal,
        Rule::Only(Chassis),
        0.04,
    );
    add(
        c,
        torso,
        v(0.15, 0.035, 0.02),
        all(v(0.0, 0.53, -0.105)),
        &fam,
        Rule::Only(Chassis),
        0.0,
    );
    add(
        c,
        torso,
        v(0.3, 0.26, 0.3),
        all(v(0.0, 0.65, 0.0)),
        &orange,
        Rule::From(Mech),
        0.05,
    );
    add(
        c,
        torso,
        v(0.24, 0.06, 0.03),
        all(v(0.0, 0.67, -0.15)),
        &fam,
        Rule::From(Mech),
        0.0,
    );
    add(
        c,
        torso,
        v(0.2, 0.08, 0.06),
        all(v(0.0, 0.55, -0.14)),
        &dark,
        Rule::From(Mech),
        0.03,
    );
    add(
        c,
        torso,
        v(0.06, 0.08, 0.28),
        all(v(0.0, 0.8, 0.01)),
        &orange_dark,
        Rule::From(Mech),
        0.03,
    );
    for s in [-1.0f32, 1.0] {
        add(
            c,
            torso,
            v(0.04, 0.12, 0.1),
            all(v(s * 0.17, 0.65, 0.0)),
            &dark,
            Rule::From(Mech),
            0.0,
        );
    }

    // ---- shoulders
    for s in [-1.0f32, 1.0] {
        add(
            c,
            torso,
            v(0.15, 0.1, 0.2),
            all(v(s * 0.31, 0.44, 0.0)),
            &gunmetal,
            Rule::Only(Chassis),
            0.04,
        );
        add(
            c,
            torso,
            v(0.3, 0.24, 0.38),
            all(v(s * 0.43, 0.5, 0.0)),
            &orange,
            Rule::From(Mech),
            0.05,
        );
        add(
            c,
            torso,
            v(0.31, 0.05, 0.39),
            all(v(s * 0.43, 0.39, 0.0)),
            &orange_dark,
            Rule::From(Mech),
            0.0,
        );
    }

    // ---- gun arm (right) and its barrels
    let arm = [
        v(0.29, 0.33, 0.0),
        v(0.29, 0.33, 0.0),
        v(0.3, 0.33, 0.0),
        v(0.43, 0.3, 0.0),
    ];
    add(
        c,
        torso,
        v(0.09, 0.22, 0.09),
        arm,
        &gunmetal,
        Rule::From(Frame),
        0.04,
    );
    add(
        c,
        torso,
        v(0.12, 0.12, 0.36),
        all(v(0.3, 0.22, -0.13)),
        &gunmetal,
        Rule::Range(Frame, Chassis),
        0.04,
    );
    add(
        c,
        torso,
        v(0.2, 0.2, 0.52),
        all(v(0.44, 0.2, -0.17)),
        &dark,
        Rule::From(Mech),
        0.05,
    );
    add(
        c,
        torso,
        v(0.21, 0.06, 0.3),
        all(v(0.44, 0.31, -0.1)),
        &orange,
        Rule::From(Mech),
        0.0,
    );
    let barrel = [
        v(0.3, 0.23, -0.41),
        v(0.3, 0.23, -0.41),
        v(0.3, 0.23, -0.41),
        v(0.44, 0.22, -0.54),
    ];
    add(
        c,
        torso,
        v(0.05, 0.05, 0.22),
        barrel,
        &steel,
        Rule::From(Frame),
        0.03,
    );
    for (k, off) in [
        v(0.065, 0.065, 0.0),
        v(-0.065, 0.065, 0.0),
        v(0.0, -0.07, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let at = barrel.map(|b| b + off + v(0.0, 0.0, -0.02));
        add(
            c,
            torso,
            v(0.045, 0.045, 0.24),
            at,
            &steel,
            Rule::Barrel(k as u8 + 1),
            0.03,
        );
    }

    // ---- left arm
    add(
        c,
        torso,
        v(0.08, 0.22, 0.08),
        all(v(-0.3, 0.33, 0.0)),
        &dark,
        Rule::Only(Chassis),
        0.04,
    );
    add(
        c,
        torso,
        v(0.12, 0.1, 0.18),
        all(v(-0.31, 0.2, -0.06)),
        &gunmetal,
        Rule::Only(Chassis),
        0.04,
    );
    add(
        c,
        torso,
        v(0.13, 0.26, 0.13),
        all(v(-0.43, 0.3, 0.0)),
        &dark,
        Rule::From(Mech),
        0.04,
    );
    add(
        c,
        torso,
        v(0.22, 0.24, 0.28),
        all(v(-0.45, 0.13, -0.05)),
        &orange,
        Rule::From(Mech),
        0.05,
    );

    // ---- RAM sticks along the top of the back, with RGB lights
    for k in 0..4u8 {
        let x = -0.1 + 0.066 * k as f32;
        let at = [
            v(x, 0.42, 0.1),
            v(x, 0.5, 0.12),
            v(x, 0.52, 0.16),
            v(x, 0.6, 0.21),
        ];
        add(
            c,
            torso,
            v(0.03, 0.17, 0.12),
            at,
            &pcb,
            Rule::Ram(k + 1),
            0.03,
        );
        add(
            c,
            torso,
            v(0.032, 0.03, 0.12),
            at.map(|p| p + v(0.0, 0.1, 0.0)),
            &rgb,
            Rule::Ram(k + 1),
            0.0,
        );
    }

    // ---- GPU cards on the back, glowing in their card's colour
    for k in 0..4u8 {
        let x = -0.12 + 0.08 * k as f32;
        let at = [
            v(x, 0.2, 0.25),
            v(x, 0.27, 0.27),
            v(x, 0.27, 0.3),
            v(x, 0.29, 0.33),
        ];
        add(
            c,
            torso,
            v(0.035, 0.2, 0.26),
            at,
            &pcb_dark,
            Rule::Card(k + 1),
            0.03,
        );
        add(
            c,
            torso,
            v(0.037, 0.03, 0.26),
            at.map(|p| p + v(0.0, 0.11, 0.0)),
            &cards[k as usize],
            Rule::Card(k + 1),
            0.0,
        );
    }
    for s in [-1.0f32, 1.0] {
        add(
            c,
            torso,
            v(0.03, 0.26, 0.3),
            all(v(s * 0.2, 0.28, 0.33)),
            &orange_dark,
            Rule::From(Mech),
            0.03,
        );
    }

    // ---- cooling fans: one on the lower back, then one on each shoulder
    let fan_specs = [
        (
            1u8,
            [
                v(0.0, 0.08, 0.1),
                v(0.0, 0.1, 0.1),
                v(0.0, 0.1, 0.18),
                v(0.0, 0.1, 0.21),
            ],
            Vec3::Z,
        ),
        (
            2,
            [
                v(-0.25, 0.44, 0.02),
                v(-0.28, 0.46, 0.02),
                v(-0.31, 0.5, 0.0),
                v(-0.43, 0.63, 0.0),
            ],
            Vec3::Y,
        ),
        (
            3,
            [
                v(0.25, 0.44, 0.02),
                v(0.28, 0.46, 0.02),
                v(0.31, 0.5, 0.0),
                v(0.43, 0.63, 0.0),
            ],
            Vec3::Y,
        ),
    ];
    for (k, at, axis) in fan_specs {
        let (house, blade_a, blade_b, push) = if axis == Vec3::Z {
            (
                v(0.2, 0.2, 0.05),
                v(0.17, 0.03, 0.012),
                v(0.03, 0.17, 0.012),
                v(0.0, 0.0, 0.03),
            )
        } else {
            (
                v(0.17, 0.04, 0.17),
                v(0.15, 0.012, 0.03),
                v(0.03, 0.012, 0.15),
                v(0.0, 0.025, 0.0),
            )
        };
        add(c, torso, house, at, &dark, Rule::Fan(k), 0.03);
        let rotor = add(
            c,
            torso,
            Vec3::ONE,
            at.map(|p| p + push),
            &steel,
            Rule::Fan(k),
            0.0,
        );
        c.entity(rotor).remove::<Mesh3d>().insert(Rotor { axis });
        block(c, &kit, Some(rotor), blade_a, Vec3::ZERO, &steel, 0.0);
        block(c, &kit, Some(rotor), blade_b, Vec3::ZERO, &steel, 0.0);
        block(
            c,
            &kit,
            Some(rotor),
            v(0.05, 0.05, 0.05)
                * if axis == Vec3::Z {
                    v(1.0, 1.0, 0.4)
                } else {
                    v(1.0, 0.4, 1.0)
                },
            Vec3::ZERO,
            &hub,
            0.0,
        );
    }

    // ---- the core: your AI itself, a ball of light in the family colour
    let core = c
        .spawn((
            Mesh3d(kit.ball.clone()),
            MeshMaterial3d(fam.clone()),
            Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::splat(0.17)),
            ChildOf(torso),
        ))
        .id();
    let shell = c
        .spawn((
            Mesh3d(kit.ball.clone()),
            MeshMaterial3d(shell_mat.clone()),
            Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::splat(0.3)),
            bevy::light::NotShadowCaster,
            ChildOf(torso),
        ))
        .id();
    let halo = c
        .spawn((
            Mesh3d(kit.ring.clone()),
            MeshMaterial3d(fam.clone()),
            Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::new(0.42, 0.6, 0.42)),
            bevy::light::NotShadowCaster,
            ChildOf(torso),
        ))
        .id();
    let sparklets = (0..3)
        .map(|_| {
            block(
                c,
                &kit,
                Some(torso),
                Vec3::splat(0.05),
                Vec3::ZERO,
                &fam,
                0.0,
            )
        })
        .collect();
    let muzzle = block(
        c,
        &kit,
        Some(torso),
        Vec3::splat(0.12),
        Vec3::ZERO,
        &fam,
        0.0,
    );
    let light = c
        .spawn((
            PointLight {
                color: Color::srgb(0.3, 0.7, 1.0),
                intensity: 70_000.0,
                range: 5.5,
                ..default()
            },
            Transform::from_xyz(0.0, 1.1, 0.0),
            ChildOf(root),
        ))
        .id();

    let ground_mat = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.3, 1.2, 1.8),
        ..default()
    });
    let ground = c
        .spawn((
            Mesh3d(meshes.add(Annulus::new(0.5, 0.62))),
            MeshMaterial3d(ground_mat.clone()),
            Transform::from_rotation(Quat::from_rotation_x(-PI / 2.0)),
            bevy::light::NotShadowCaster,
        ))
        .id();

    let drone_body = metal(&mut mats, Color::srgb(0.12, 0.13, 0.16), 0.4);
    commands.insert_resource(MechVis {
        root,
        hips,
        torso,
        thighs,
        knees,
        core,
        shell,
        halo,
        sparklets,
        muzzle,
        light,
        ground,
        ground_mat,
        fam,
        shell_mat,
        rgb,
        hub,
        cards,
        drones: vec![],
        drone_kit: (drone_body, steel),
        yaw: PI * 0.25,
        spin: 0.0,
        scale: 1.0,
        family: None,
    });
}

fn pop_curve(p: f32) -> f32 {
    if p < 0.6 {
        1.3 * (p / 0.6)
    } else {
        1.3 - 0.3 * ((p - 0.6) / 0.4).min(1.0)
    }
}

fn angle_lerp(a: f32, b: f32, k: f32) -> f32 {
    let d = (b - a + PI).rem_euclid(TAU) - PI;
    a + d * k
}

#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    time: Res<Time>,
    fight: Res<Fight>,
    kit: Res<Kit>,
    mut vis: ResMut<MechVis>,
    mut parts: Query<(&mut Part, &mut Transform, &mut Visibility, Option<&Rotor>)>,
    mut q: Query<(&mut Transform, &mut Visibility), Without<Part>>,
    mut lights: Query<&mut PointLight>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let vis = &mut *vis;
    let pl = &fight.player;
    let form = fight.form();
    let fi = form_index(form);
    let spark = form == Form::Spark;

    // ---- colours that follow the model family
    if vis.family != Some(fight.family) {
        vis.family = Some(fight.family);
        let col = fight.family.color();
        if let Some(mut m) = mats.get_mut(&vis.fam) {
            m.emissive = glow(col, 5.0);
        }
        if let Some(mut m) = mats.get_mut(&vis.shell_mat) {
            m.base_color = col.with_alpha(0.25);
            m.emissive = glow(col, 0.4);
        }
        if let Ok(mut l) = lights.get_mut(vis.light) {
            l.color = col;
        }
        if let Some(mut m) = mats.get_mut(&vis.ground_mat) {
            m.emissive = glow(col, 1.4);
        }
    }
    if let Some(mut m) = mats.get_mut(&vis.rgb) {
        m.emissive = glow(Color::hsl((t * 120.0) % 360.0, 1.0, 0.55), 3.0);
    }
    let hot = fight.throttle() < 0.98;
    if let Some(mut m) = mats.get_mut(&vis.hub) {
        m.emissive = if hot {
            LinearRgba::rgb(5.0, 1.0, 0.2)
        } else {
            LinearRgba::rgb(0.6, 3.0, 4.0)
        };
    }
    for (k, h) in vis.cards.iter().enumerate() {
        if let (Some(card), Some(mut m)) = (fight.cards.get(k), mats.get_mut(h)) {
            let l = 0.4 + 1.6 * card.ready() + 6.0 * card.fired;
            m.emissive = glow(card.kind.color(), 2.0 * l);
        }
    }
    if let Ok(mut l) = lights.get_mut(vis.light) {
        l.intensity = if pl.overclock > 0.0 {
            160_000.0
        } else {
            70_000.0
        };
    }

    // ---- where the body is and which way it faces: the legs follow your
    // movement, the upper body turns to whatever it's shooting
    let aim_yaw = f32::atan2(-pl.aim.x, -pl.aim.y);
    if spark {
        vis.yaw = aim_yaw;
    } else if pl.vel.length() > 0.5 {
        let want = f32::atan2(-pl.vel.x, -pl.vel.y);
        vis.yaw = angle_lerp(vis.yaw, want, (12.0 * dt).min(1.0));
    }
    let want_scale = [1.3, 1.05, 1.15, 1.3][fi];
    vis.scale += (want_scale - vis.scale) * (6.0 * dt).min(1.0);
    let speed = (pl.vel.length() / 5.6).min(1.0);
    let phase = pl.walked * 7.0;
    let hover = if spark {
        0.1 + 0.06 * (t * 3.0).sin()
    } else {
        0.0
    };
    let lost = fight.outcome == Outcome::Lost;
    let mut root_tf = Transform::from_xyz(pl.pos.x, floor_y(pl.pos) + hover, pl.pos.y)
        .with_rotation(Quat::from_rotation_y(vis.yaw))
        .with_scale(Vec3::splat(vis.scale));
    if lost {
        root_tf.rotation *= Quat::from_rotation_x(1.3);
        root_tf.translation.y += 0.25;
    }
    let blink = pl.iframes > 0.0 && (t * 24.0) as i32 % 2 == 0 && !lost;
    if let Ok((mut tf, mut v)) = q.get_mut(vis.ground) {
        let r = vis.scale * if spark { 0.8 } else { 1.0 } * (1.0 + 0.04 * (t * 4.0).sin());
        *tf = Transform::from_xyz(pl.pos.x, floor_y(pl.pos) + 0.015, pl.pos.y)
            .with_rotation(Quat::from_rotation_x(-PI / 2.0))
            .with_scale(Vec3::new(r, r, 1.0));
        v.set_if_neq(if lost {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    if let Ok((mut tf, mut v)) = q.get_mut(vis.root) {
        *tf = root_tf;
        v.set_if_neq(if blink {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let bob = 0.03 * (phase * 2.0).sin().abs() * speed;
    let hips_tf = Transform::from_xyz(0.0, 0.6 + bob, 0.0);
    if let Ok((mut tf, _)) = q.get_mut(vis.hips) {
        *tf = hips_tf;
    }
    let twist = angle_lerp(0.0, aim_yaw - vis.yaw, 1.0);
    let torso_tf = Transform::from_xyz(0.0, 0.08, 0.0).with_rotation(Quat::from_rotation_y(twist));
    if let Ok((mut tf, _)) = q.get_mut(vis.torso) {
        *tf = torso_tf;
    }
    for (i, (&th, &kn)) in vis.thighs.iter().zip(&vis.knees).enumerate() {
        let s = if i == 0 { 1.0 } else { -1.0 };
        let swing = (phase).sin() * 0.7 * speed * s;
        let bend = ((phase + if i == 0 { 0.0 } else { PI }).cos().min(0.0)) * 0.9 * speed;
        if let Ok((mut tf, _)) = q.get_mut(th) {
            tf.rotation = Quat::from_rotation_x(swing);
        }
        if let Ok((mut tf, _)) = q.get_mut(kn) {
            tf.rotation = Quat::from_rotation_x(bend);
        }
    }

    // ---- parts pop on as you pick them
    vis.spin +=
        dt * (10.0 + 40.0 * (fight.heat / 100.0)) * if pl.overclock > 0.0 { 2.0 } else { 1.0 };
    let spin = vis.spin;
    for (mut p, mut tf, mut v, rotor) in &mut parts {
        if let Some(r) = rotor {
            tf.rotation = Quat::from_axis_angle(r.axis, spin);
        }
        let on = p.rule.on(&fight);
        if on && !p.shown {
            p.shown = true;
            p.pop = 0.0;
        }
        if !on {
            p.shown = false;
            v.set_if_neq(Visibility::Hidden);
            continue;
        }
        v.set_if_neq(Visibility::Inherited);
        p.pop = (p.pop + dt / 0.35).min(1.0);
        let target = p.at[fi];
        tf.translation = tf.translation.lerp(target, (10.0 * dt).min(1.0));
        if p.pop < 1.0 {
            tf.translation = target;
        }
        tf.scale = p.size * pop_curve(p.pop).max(0.01);
    }

    // ---- the core, and the spark's shell and halo
    let (core_at, core_r) = [
        (Vec3::new(0.0, 0.08, 0.0), 0.17),
        (Vec3::new(0.0, 0.28, 0.0), 0.14),
        (Vec3::new(0.0, 0.3, -0.16), 0.11),
        (Vec3::new(0.0, 0.37, -0.22), 0.085),
    ][fi];
    let pulse = 1.0 + 0.08 * (t * 6.0).sin() + if pl.overclock > 0.0 { 0.2 } else { 0.0 };
    if let Ok((mut tf, mut v)) = q.get_mut(vis.core) {
        tf.translation = tf.translation.lerp(core_at, (8.0 * dt).min(1.0));
        tf.scale = Vec3::splat(core_r * pulse);
        v.set_if_neq(if lost {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let show_spark = spark && !lost;
    if let Ok((mut tf, mut v)) = q.get_mut(vis.shell) {
        tf.scale = Vec3::splat(0.3 * pulse);
        v.set_if_neq(if show_spark {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if let Ok((mut tf, mut v)) = q.get_mut(vis.halo) {
        tf.rotation = Quat::from_rotation_y(t * 2.5) * Quat::from_rotation_x(0.5);
        v.set_if_neq(if show_spark {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (k, &e) in vis.sparklets.iter().enumerate() {
        if let Ok((mut tf, mut v)) = q.get_mut(e) {
            let a = t * 3.0 + k as f32 * TAU / 3.0;
            tf.translation =
                Vec3::new(a.cos() * 0.36, 0.08 + 0.1 * (a * 1.7).sin(), a.sin() * 0.36);
            v.set_if_neq(if show_spark {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    let muzzle_at = [
        Vec3::new(0.0, 0.08, -0.32),
        Vec3::new(0.3, 0.23, -0.55),
        Vec3::new(0.3, 0.23, -0.55),
        Vec3::new(0.44, 0.22, -0.7),
    ][fi];
    if let Ok((mut tf, mut v)) = q.get_mut(vis.muzzle) {
        tf.translation = muzzle_at;
        tf.scale = Vec3::splat(0.09 + pl.fired * 1.2);
        v.set_if_neq(if pl.fired > 0.0 && !lost {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }

    // ---- drones: docked on your back and hips, flying out to fight
    let torso_world = root_tf.mul_transform(hips_tf).mul_transform(torso_tf);
    while vis.drones.len() < fight.drones.len() {
        let root = pivot(&mut commands, None, torso_world.translation);
        block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.2, 0.06, 0.2),
            Vec3::ZERO,
            &vis.drone_kit.0,
            0.03,
        );
        for (x, z) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            block(
                &mut commands,
                &kit,
                Some(root),
                Vec3::new(0.09, 0.015, 0.09),
                Vec3::new(x * 0.11, 0.04, z * 0.11),
                &vis.drone_kit.1,
                0.0,
            );
        }
        block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.07, 0.03, 0.02),
            Vec3::new(0.0, 0.0, -0.105),
            &vis.fam,
            0.0,
        );
        vis.drones.push(DroneVis {
            root,
            pos: torso_world.translation,
        });
    }
    for (i, dv) in vis.drones.iter_mut().enumerate() {
        let Some(d) = fight.drones.get(i) else {
            if let Ok((_, mut v)) = q.get_mut(dv.root) {
                v.set_if_neq(Visibility::Hidden);
            }
            continue;
        };
        let (want, rot) = if d.docked || (!d.out && d.pos.distance(pl.pos) < 0.6) {
            (
                torso_world.transform_point(dock(i, fi)),
                torso_world.rotation,
            )
        } else {
            let y = floor_y(d.pos) + 0.95 + 0.08 * (t * 5.0 + i as f32).sin();
            (
                Vec3::new(d.pos.x, y, d.pos.y),
                Quat::from_rotation_y(t * 4.0 + i as f32),
            )
        };
        let k = if d.docked { 1.0 } else { (18.0 * dt).min(1.0) };
        dv.pos = dv.pos.lerp(want, k);
        if let Ok((mut tf, mut v)) = q.get_mut(dv.root) {
            *tf = Transform::from_translation(dv.pos)
                .with_rotation(rot)
                .with_scale(Vec3::splat(vis.scale));
            v.set_if_neq(if lost || blink {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
    }
}
