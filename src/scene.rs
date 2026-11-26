//! The Arctic Vault datacentre arena, built from simple 3D shapes.
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const FOG: Color = Color::srgb(0.025, 0.035, 0.065);

#[derive(Component)]
pub struct Blink {
    on: Handle<StandardMaterial>,
    off: Handle<StandardMaterial>,
    rate: f32,
    phase: f32,
}

#[derive(Component)]
pub struct Spin(pub Vec3);

pub fn emissive(mats: &mut Assets<StandardMaterial>, c: LinearRgba) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: c,
        ..default()
    })
}

pub fn metal(
    mats: &mut Assets<StandardMaterial>,
    c: Color,
    rough: f32,
) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        metallic: 0.5,
        ..default()
    })
}

/// Cheap deterministic "random" in 0..1, so the scene is the same every run.
fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

pub fn build(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // ---------------------------------------------------------------- floor
    let base = metal(&mut mats, Color::srgb(0.02, 0.025, 0.04), 0.9);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(27.0, 0.1, 17.0))),
        MeshMaterial3d(base),
        Transform::from_xyz(0.0, -0.08, 0.0),
    ));
    let tile = meshes.add(Cuboid::new(0.95, 0.1, 0.95));
    let tile_a = metal(&mut mats, Color::srgb(0.11, 0.14, 0.2), 0.45);
    let tile_b = metal(&mut mats, Color::srgb(0.125, 0.16, 0.23), 0.45);
    let tile_vent = metal(&mut mats, Color::srgb(0.07, 0.09, 0.13), 0.7);
    for x in -13..13 {
        for z in -8..8 {
            let i = ((x + 20) * 31 + (z + 20) * 17) as u32;
            let m = if hash(i) < 0.08 {
                tile_vent.clone()
            } else if (x + z) % 2 == 0 {
                tile_a.clone()
            } else {
                tile_b.clone()
            };
            commands.spawn((
                Mesh3d(tile.clone()),
                MeshMaterial3d(m),
                Transform::from_xyz(x as f32 + 0.5, -0.05, z as f32 + 0.5),
            ));
        }
    }

    // ---------------------------------------------------------------- walls + windows
    let wall = metal(&mut mats, Color::srgb(0.08, 0.1, 0.15), 0.8);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(27.0, 5.0, 0.6))),
        MeshMaterial3d(wall.clone()),
        Transform::from_xyz(0.0, 2.5, -8.2),
    ));
    // In the isometric view the +x side faces the camera, so leave it open.
    let iso = std::env::var("VIEW").map(|v| v != "angled").unwrap_or(true);
    let sides: &[f32] = if iso { &[-1.0] } else { &[-1.0, 1.0] };
    for side in sides {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 5.0, 17.0))),
            MeshMaterial3d(wall.clone()),
            Transform::from_xyz(side * 13.3, 2.5, 0.0),
        ));
    }
    let snow_window = mats.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.8, 1.0),
        emissive: LinearRgba::rgb(0.35, 0.6, 1.1),
        ..default()
    });
    let mullion = metal(&mut mats, Color::srgb(0.03, 0.04, 0.06), 0.6);
    for i in 0..5 {
        let x = -10.0 + i as f32 * 5.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(3.6, 1.8, 0.1))),
            MeshMaterial3d(snow_window.clone()),
            Transform::from_xyz(x, 3.4, -7.86),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 1.8, 0.14))),
            MeshMaterial3d(mullion.clone()),
            Transform::from_xyz(x, 3.4, -7.82),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(3.6, 0.1, 0.14))),
            MeshMaterial3d(mullion.clone()),
            Transform::from_xyz(x, 3.4, -7.82),
        ));
    }
    let strip = emissive(&mut mats, LinearRgba::rgb(0.2, 2.2, 2.6));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(26.0, 0.08, 0.08))),
        MeshMaterial3d(strip.clone()),
        Transform::from_xyz(0.0, 0.3, -7.85),
    ));
    // frost piled along the back wall
    let frost = mats.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.93, 1.0),
        perceptual_roughness: 0.9,
        ..default()
    });
    let frost_mesh = meshes.add(Cuboid::new(0.3, 0.12, 0.3));
    for i in 0..70 {
        let x = -12.8 + hash(i * 3) * 25.6;
        let z = -7.8 + hash(i * 7 + 1) * 0.5;
        let s = 0.6 + hash(i * 11 + 2) * 1.2;
        commands.spawn((
            Mesh3d(frost_mesh.clone()),
            MeshMaterial3d(frost.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::new(s, s * 0.6, s)),
        ));
    }

    // ---------------------------------------------------------------- server racks
    let rack_body = metal(&mut mats, Color::srgb(0.17, 0.19, 0.25), 0.35);
    let rack_trim = metal(&mut mats, Color::srgb(0.32, 0.35, 0.42), 0.3);
    let slat = metal(&mut mats, Color::srgb(0.05, 0.06, 0.09), 0.8);
    let led_cols = [
        LinearRgba::rgb(0.2, 3.5, 1.0),
        LinearRgba::rgb(0.3, 1.4, 4.0),
        LinearRgba::rgb(0.2, 3.0, 3.2),
        LinearRgba::rgb(0.2, 3.5, 1.0),
    ];
    let led_on: Vec<_> = led_cols.iter().map(|c| emissive(&mut mats, *c)).collect();
    let led_off = metal(&mut mats, Color::srgb(0.04, 0.05, 0.07), 0.5);
    let rack_mesh = meshes.add(Cuboid::new(1.1, 2.6, 1.0));
    let trim_mesh = meshes.add(Cuboid::new(1.16, 0.08, 1.06));
    let slat_mesh = meshes.add(Cuboid::new(0.85, 0.035, 0.02));
    let led_mesh = meshes.add(Cuboid::new(0.07, 0.07, 0.03));
    let mut rack_id = 0u32;
    let mut spawn_rack = |commands: &mut Commands, pos: Vec3, yaw: f32, height: f32| {
        rack_id += 1;
        let rot = Quat::from_rotation_y(yaw);
        let t = Transform::from_translation(pos + Vec3::Y * height / 2.0)
            .with_rotation(rot)
            .with_scale(Vec3::new(1.0, height / 2.6, 1.0));
        commands
            .spawn((
                Mesh3d(rack_mesh.clone()),
                MeshMaterial3d(rack_body.clone()),
                t,
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(trim_mesh.clone()),
                    MeshMaterial3d(rack_trim.clone()),
                    Transform::from_xyz(0.0, 1.3, 0.0),
                ));
                for r in 0..9 {
                    let y = -1.05 + r as f32 * 0.26;
                    p.spawn((
                        Mesh3d(slat_mesh.clone()),
                        MeshMaterial3d(slat.clone()),
                        Transform::from_xyz(-0.06, y, 0.51),
                    ));
                    for col in 0..2 {
                        let n = rack_id * 100 + r * 2 + col;
                        let on =
                            led_on[(hash(n) * led_on.len() as f32) as usize % led_on.len()].clone();
                        p.spawn((
                            Mesh3d(led_mesh.clone()),
                            MeshMaterial3d(on.clone()),
                            Transform::from_xyz(0.38 + col as f32 * 0.1, y + 0.06, 0.51),
                            Blink {
                                on,
                                off: led_off.clone(),
                                rate: 0.6 + hash(n + 7) * 3.0,
                                phase: hash(n + 13) * TAU,
                            },
                        ));
                    }
                }
            });
    };
    // back row
    for i in 0..17 {
        spawn_rack(
            &mut commands,
            Vec3::new(-10.4 + i as f32 * 1.3, 0.0, -6.9),
            0.0,
            2.6,
        );
    }
    // side rows facing inward
    for i in 0..8 {
        let z = -4.6 + i as f32 * 1.3;
        spawn_rack(&mut commands, Vec3::new(-12.2, 0.0, z), FRAC_PI_2, 2.6);
        if !iso {
            spawn_rack(&mut commands, Vec3::new(12.2, 0.0, z), -FRAC_PI_2, 2.6);
        }
    }
    // cover racks inside the arena (shorter, so you can see over them)
    for (x, z) in [
        (-7.5, -2.8),
        (-7.5, 3.4),
        (-0.8, -3.6),
        (2.4, 3.6),
        (6.0, -3.4),
    ] {
        spawn_rack(&mut commands, Vec3::new(x, 0.0, z), 0.0, 1.6);
    }

    // ---------------------------------------------------------------- set dressing
    // hazard stripes around the fighting area
    let yellow = metal(&mut mats, Color::srgb(0.95, 0.72, 0.15), 0.6);
    let black = metal(&mut mats, Color::srgb(0.04, 0.04, 0.05), 0.6);
    let stripe = meshes.add(Cuboid::new(0.5, 0.02, 0.22));
    let stripe_v = meshes.add(Cuboid::new(0.22, 0.02, 0.5));
    for i in 0..44 {
        let x = -10.75 + i as f32 * 0.5;
        let m = if i % 2 == 0 {
            yellow.clone()
        } else {
            black.clone()
        };
        commands.spawn((
            Mesh3d(stripe.clone()),
            MeshMaterial3d(m),
            Transform::from_xyz(x, 0.01, -5.75),
        ));
    }
    for side in [-1.0f32, 1.0] {
        for i in 0..27 {
            let z = -5.5 + i as f32 * 0.5;
            let m = if i % 2 == 0 {
                yellow.clone()
            } else {
                black.clone()
            };
            commands.spawn((
                Mesh3d(stripe_v.clone()),
                MeshMaterial3d(m),
                Transform::from_xyz(side * 11.1, 0.01, z),
            ));
        }
    }
    // cable trays running across the floor between racks
    let tray = metal(&mut mats, Color::srgb(0.06, 0.07, 0.1), 0.5);
    let cable_a = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.5, 0.15),
        emissive: LinearRgba::rgb(0.6, 0.2, 0.02),
        ..default()
    });
    let cable_b = mats.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.5, 1.0),
        emissive: LinearRgba::rgb(0.05, 0.2, 0.6),
        ..default()
    });
    for (a, b) in [
        ((-7.5f32, -4.2f32), (-7.5f32, -6.3f32)),
        ((6.0, -4.8), (6.0, -6.3)),
        ((-0.8, -4.9), (-0.8, -6.3)),
        ((-7.5, 2.1), (-11.6, 2.1)),
    ] {
        let from = Vec3::new(a.0, 0.03, a.1);
        let to = Vec3::new(b.0, 0.03, b.1);
        let len = from.distance(to);
        let mid = (from + to) / 2.0;
        let t = Transform::from_translation(mid).looking_at(to, Vec3::Y);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.06, len))),
            MeshMaterial3d(tray.clone()),
            t,
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.06, 0.08, len))),
            MeshMaterial3d(cable_a.clone()),
            t.with_translation(mid + t.right() * 0.1),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.06, 0.08, len))),
            MeshMaterial3d(cable_b.clone()),
            t.with_translation(mid - t.right() * 0.1),
        ));
    }
    // warm accents: warning lights and supply crates
    let amber = emissive(&mut mats, LinearRgba::rgb(4.0, 1.4, 0.2));
    for (x, z) in [
        (-12.95f32, -2.0f32),
        (12.95, -2.0),
        (-12.95, 4.5),
        (12.95, 4.5),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 0.35, 0.9))),
            MeshMaterial3d(amber.clone()),
            Transform::from_xyz(x, 3.4, z),
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.6, 0.2),
                intensity: 60_000.0,
                range: 5.0,
                ..default()
            },
            Transform::from_xyz(x * 0.93, 3.0, z),
        ));
    }
    let crate_mat = metal(&mut mats, Color::srgb(0.75, 0.45, 0.12), 0.7);
    let crate_mesh = meshes.add(Cuboid::new(0.8, 0.6, 0.8));
    for (i, (x, z)) in [
        (-10.3f32, 6.6f32),
        (-9.4, 6.9),
        (-10.0, 5.8),
        (10.2, 6.5),
        (9.5, -5.0),
    ]
    .iter()
    .enumerate()
    {
        let h = if i == 2 { 0.9 } else { 0.3 };
        commands.spawn((
            Mesh3d(crate_mesh.clone()),
            MeshMaterial3d(crate_mat.clone()),
            Transform::from_xyz(*x, h, *z)
                .with_rotation(Quat::from_rotation_y(hash(i as u32 + 40) * 0.6)),
        ));
    }

    // ---------------------------------------------------------------- lighting
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.65, 0.75, 1.0),
            illuminance: 2500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-6.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for x in [-8.0f32, 0.0, 8.0] {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.4, 0.9, 1.0),
                intensity: 120_000.0,
                range: 7.0,
                ..default()
            },
            Transform::from_xyz(x, 3.6, -5.6),
        ));
    }
}

pub fn animate(
    time: Res<Time>,
    mut blinks: Query<(&Blink, &mut MeshMaterial3d<StandardMaterial>)>,
    mut spins: Query<(&Spin, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (b, mut m) in &mut blinks {
        let on = (t * b.rate + b.phase).sin() > -0.6;
        let want = if on { &b.on } else { &b.off };
        if m.0 != *want {
            m.0 = want.clone();
        }
    }
    let dt = time.delta_secs();
    for (s, mut tr) in &mut spins {
        tr.rotate(Quat::from_euler(
            EulerRot::XYZ,
            s.0.x * dt,
            s.0.y * dt,
            s.0.z * dt,
        ));
    }
}
