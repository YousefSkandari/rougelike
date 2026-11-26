//! Draws the fight: keeps 3D models in step with the simulation in
//! `fight.rs`, plus particles, camera shake and the botnet tethers.
use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::GameCamera;
use crate::fight::{Family, Fight, Fx, NODE_POS, Outcome, PORTS, ShotKind};
use crate::scene::{Spin, emissive, metal};

const PLAYER_Y: f32 = 0.65;
const SHOT_Y: f32 = 0.55;
const BOT_Y: f32 = 0.4;
const NODE_Y: f32 = 1.0;
const FAMILIES: [Family; 5] = [
    Family::Language,
    Family::Vision,
    Family::Reasoning,
    Family::Agent,
    Family::Swarm,
];

fn glow(c: Color, k: f32) -> LinearRgba {
    let l = c.to_linear();
    LinearRgba::rgb(l.red * k, l.green * k, l.blue * k)
}

fn fam_index(f: Family) -> usize {
    FAMILIES.iter().position(|x| *x == f).unwrap_or(0)
}

/// Meshes and materials shared by everything spawned during the fight.
#[derive(Resource)]
pub struct VisAssets {
    shot: Handle<Mesh>,
    cube: Handle<Mesh>,
    bot_body: Handle<Mesh>,
    bot_eye: Handle<Mesh>,
    bot_ant: Handle<Mesh>,
    disc: Handle<Mesh>,
    turret_base: Handle<Mesh>,
    beam: Handle<Mesh>,
    ring: Handle<Mesh>,
    m_family_shot: [Handle<StandardMaterial>; 5],
    m_burst: Handle<StandardMaterial>,
    m_turret_shot: Handle<StandardMaterial>,
    m_enemy_shot: Handle<StandardMaterial>,
    m_bot: Handle<StandardMaterial>,
    m_flash: Handle<StandardMaterial>,
    m_red_eye: Handle<StandardMaterial>,
    m_dark: Handle<StandardMaterial>,
    m_red_glow: Handle<StandardMaterial>,
    m_drone_perm: Handle<StandardMaterial>,
    m_drone_temp: Handle<StandardMaterial>,
    m_turret: Handle<StandardMaterial>,
    m_turret_top: Handle<StandardMaterial>,
    m_beam_core: Handle<StandardMaterial>,
    m_beam_glow: Handle<StandardMaterial>,
    m_debris: Handle<StandardMaterial>,
    m_spark_red: Handle<StandardMaterial>,
    m_spark_green: Handle<StandardMaterial>,
    m_spark_white: Handle<StandardMaterial>,
}

/// Entities that mirror the simulation, reused frame to frame.
#[derive(Resource)]
pub struct Vis {
    player: Entity,
    core_mat: Handle<StandardMaterial>,
    ring_mat: Handle<StandardMaterial>,
    player_light: Entity,
    node: Entity,
    node_mat: Handle<StandardMaterial>,
    node_light: Entity,
    node_ring: Entity,
    bots: Vec<Entity>,
    bullets: Vec<Entity>,
    drones: Vec<Entity>,
    turrets: Vec<Entity>,
    beams: Vec<Entity>,
    family: Option<Family>,
}

#[derive(Component)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
    gravity: bool,
}

/// The camera's resting place, so shake never drifts it.
#[derive(Component)]
pub struct CamBase(pub Transform);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let add = |mats: &mut Assets<StandardMaterial>, c: LinearRgba| emissive(mats, c);
    let m_family_shot = FAMILIES.map(|f| add(&mut mats, glow(f.color(), 9.0)));
    let assets = VisAssets {
        shot: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        bot_body: meshes.add(Cuboid::new(0.44, 0.3, 0.44)),
        bot_eye: meshes.add(Cuboid::new(0.09, 0.08, 0.04)),
        bot_ant: meshes.add(Cylinder::new(0.02, 0.2)),
        disc: meshes.add(Cylinder::new(0.45, 0.01)),
        turret_base: meshes.add(Cylinder::new(0.22, 0.3)),
        beam: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        ring: meshes.add(Torus::new(0.34, 0.4)),
        m_family_shot,
        m_burst: add(&mut mats, LinearRgba::rgb(0.8, 8.0, 2.2)),
        m_turret_shot: add(&mut mats, LinearRgba::rgb(8.0, 3.5, 0.6)),
        m_enemy_shot: add(&mut mats, LinearRgba::rgb(9.0, 0.6, 1.0)),
        m_bot: metal(&mut mats, Color::srgb(0.3, 0.32, 0.38), 0.4),
        m_flash: add(&mut mats, LinearRgba::rgb(6.0, 6.0, 6.0)),
        m_red_eye: add(&mut mats, LinearRgba::rgb(6.0, 0.3, 0.5)),
        m_dark: metal(&mut mats, Color::srgb(0.05, 0.05, 0.07), 0.6),
        m_red_glow: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.15, 0.25, 0.35),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
        m_drone_perm: add(&mut mats, glow(Family::Swarm.color(), 6.0)),
        m_drone_temp: add(&mut mats, LinearRgba::rgb(0.6, 2.4, 7.0)),
        m_turret: metal(&mut mats, Color::srgb(0.55, 0.5, 0.45), 0.35),
        m_turret_top: add(&mut mats, LinearRgba::rgb(6.0, 2.6, 0.4)),
        m_beam_core: add(&mut mats, LinearRgba::rgb(9.0, 9.0, 9.0)),
        m_beam_glow: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.3, 0.43, 0.5),
            emissive: LinearRgba::rgb(3.0, 0.4, 0.6),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
        m_debris: metal(&mut mats, Color::srgb(0.35, 0.37, 0.43), 0.4),
        m_spark_red: add(&mut mats, LinearRgba::rgb(8.0, 0.8, 1.0)),
        m_spark_green: add(&mut mats, LinearRgba::rgb(1.0, 8.0, 2.5)),
        m_spark_white: add(&mut mats, LinearRgba::rgb(6.0, 6.0, 6.0)),
    };

    // ---- the player: your rogue AI core
    let core_mat = add(&mut mats, LinearRgba::rgb(0.6, 6.0, 1.8));
    let ring_mat = add(&mut mats, LinearRgba::rgb(0.3, 3.0, 1.0));
    let shell = mats.add(StandardMaterial {
        base_color: Color::srgba(0.3, 1.0, 0.55, 0.22),
        emissive: LinearRgba::rgb(0.05, 0.5, 0.2),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let mut player_light = Entity::PLACEHOLDER;
    let player = commands
        .spawn((
            Transform::from_xyz(-4.5, PLAYER_Y, 0.8),
            Visibility::Visible,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.26))),
                MeshMaterial3d(core_mat.clone()),
            ));
            p.spawn((Mesh3d(meshes.add(Sphere::new(0.42))), MeshMaterial3d(shell)));
            p.spawn((
                Mesh3d(meshes.add(Torus::new(0.56, 0.64))),
                MeshMaterial3d(ring_mat.clone()),
                Transform::from_rotation(Quat::from_rotation_x(0.5)),
                Spin(Vec3::new(0.0, 2.5, 0.0)),
            ));
            player_light = p
                .spawn((
                    PointLight {
                        color: Color::srgb(0.3, 1.0, 0.5),
                        intensity: 200_000.0,
                        range: 8.0,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.6, 0.0),
                ))
                .id();
        })
        .id();

    // ---- the threat: Botnet Swarm command node
    let node_mat = metal(&mut mats, Color::srgb(0.2, 0.2, 0.25), 0.3);
    let node_pos = Vec3::new(NODE_POS.x, NODE_Y, NODE_POS.y);
    let node = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(1.4, 1.2, 1.2))),
            MeshMaterial3d(node_mat.clone()),
            Transform::from_translation(node_pos),
            Visibility::Visible,
        ))
        .with_children(|p| {
            for x in [-0.32f32, 0.32] {
                p.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.3, 0.26, 0.05))),
                    MeshMaterial3d(assets.m_red_eye.clone()),
                    Transform::from_xyz(x, 0.12, -0.61),
                ));
            }
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.7, 0.06, 0.05))),
                MeshMaterial3d(assets.m_dark.clone()),
                Transform::from_xyz(0.0, -0.28, -0.61),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 0.6))),
                MeshMaterial3d(assets.m_dark.clone()),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.09))),
                MeshMaterial3d(assets.m_red_eye.clone()),
                Transform::from_xyz(0.0, 1.18, 0.0),
            ));
        })
        .id();
    let node_ring = commands
        .spawn((
            Mesh3d(meshes.add(Torus::new(1.35, 1.45))),
            MeshMaterial3d(add(&mut mats, LinearRgba::rgb(3.0, 0.15, 0.3))),
            Transform::from_translation(node_pos.with_y(0.03)),
            Visibility::Visible,
        ))
        .id();
    let node_light = commands
        .spawn((
            PointLight {
                color: Color::srgb(1.0, 0.2, 0.3),
                intensity: 300_000.0,
                range: 10.0,
                ..default()
            },
            Transform::from_translation(node_pos + Vec3::Y * 1.2),
        ))
        .id();

    // bot ports: glowing hatches in the floor where waves come in
    let port_ring = meshes.add(Torus::new(0.55, 0.65));
    let port_mat = add(&mut mats, LinearRgba::rgb(2.4, 0.12, 0.25));
    let hatch = meshes.add(Cylinder::new(0.55, 0.04));
    let hatch_mat = metal(&mut mats, Color::srgb(0.06, 0.03, 0.05), 0.5);
    for p in PORTS.iter().skip(1) {
        commands.spawn((
            Mesh3d(port_ring.clone()),
            MeshMaterial3d(port_mat.clone()),
            Transform::from_xyz(p.x, 0.02, p.y),
        ));
        commands.spawn((
            Mesh3d(hatch.clone()),
            MeshMaterial3d(hatch_mat.clone()),
            Transform::from_xyz(p.x, 0.0, p.y),
        ));
    }

    commands.insert_resource(Vis {
        player,
        core_mat,
        ring_mat,
        player_light,
        node,
        node_mat,
        node_light,
        node_ring,
        bots: vec![],
        bullets: vec![],
        drones: vec![],
        turrets: vec![],
        beams: vec![],
        family: None,
    });
    commands.insert_resource(assets);
}

/// Grow a pool of entities until it holds at least `n`.
fn grow(pool: &mut Vec<Entity>, n: usize, mut spawn: impl FnMut() -> Entity) {
    while pool.len() < n {
        pool.push(spawn());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    fight: Res<Fight>,
    mut vis: ResMut<Vis>,
    a: Res<VisAssets>,
    time: Res<Time>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&mut Transform, &mut Visibility), Without<Particle>>,
    mut mq: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut lights: Query<&mut PointLight>,
    mut gizmos: Gizmos,
) {
    let t = time.elapsed_secs();
    let vis = &mut *vis;
    let mut place = |e: Entity, tf: Option<Transform>| {
        if let Ok((mut tr, mut v)) = q.get_mut(e) {
            match tf {
                Some(tf) => {
                    *tr = tf;
                    v.set_if_neq(Visibility::Inherited);
                }
                None => {
                    v.set_if_neq(Visibility::Hidden);
                }
            }
        }
    };
    let mut set_mat = |e: Entity, m: &Handle<StandardMaterial>| {
        if let Ok(mut mm) = mq.get_mut(e)
            && mm.0 != *m
        {
            mm.0 = m.clone();
        }
    };

    // ---- player core, coloured by the active model family
    if vis.family != Some(fight.family) {
        vis.family = Some(fight.family);
        let c = fight.family.color();
        if let Some(mut m) = mats.get_mut(&vis.core_mat) {
            m.emissive = glow(c, 6.0);
        }
        if let Some(mut m) = mats.get_mut(&vis.ring_mat) {
            m.emissive = glow(c, 3.0);
        }
        if let Ok(mut l) = lights.get_mut(vis.player_light) {
            l.color = c;
        }
    }
    let pl = &fight.player;
    let ppos = Vec3::new(pl.pos.x, PLAYER_Y + 0.08 * (t * 3.0).sin(), pl.pos.y);
    let blink = pl.iframes > 0.0 && (t * 24.0) as i32 % 2 == 0;
    let alive = fight.outcome != Outcome::Lost;
    place(
        vis.player,
        (alive && !blink).then(|| Transform::from_translation(ppos)),
    );
    if let Ok(mut l) = lights.get_mut(vis.player_light) {
        l.intensity = if pl.overclock > 0.0 {
            420_000.0
        } else {
            200_000.0
        };
    }
    if alive {
        // aim line on the floor
        let a = pl.aim;
        let from = Vec3::new(pl.pos.x + a.x * 0.6, 0.06, pl.pos.y + a.y * 0.6);
        let to = Vec3::new(pl.pos.x + a.x * 1.4, 0.06, pl.pos.y + a.y * 1.4);
        let c = fight.family.color().with_alpha(0.7);
        gizmos.line(from, to, c);
    }

    // ---- command node
    let node_alive = fight.outcome != Outcome::Won;
    let shake = if fight.node_hp < fight.node_max * 0.3 {
        0.04 * (t * 37.0).sin()
    } else {
        0.0
    };
    let node_pos = Vec3::new(
        NODE_POS.x + shake,
        NODE_Y + 0.15 * (t * 1.6 + 1.0).sin(),
        NODE_POS.y,
    );
    let face = Vec3::new(pl.pos.x, node_pos.y, pl.pos.y);
    place(
        vis.node,
        node_alive.then(|| Transform::from_translation(node_pos).looking_at(face, Vec3::Y)),
    );
    place(
        vis.node_ring,
        node_alive.then(|| {
            Transform::from_translation(node_pos.with_y(0.03))
                .with_rotation(Quat::from_rotation_y(-0.8 * t))
        }),
    );
    set_mat(
        vis.node,
        if fight.node_flash > 0.0 {
            &a.m_flash
        } else {
            &vis.node_mat
        },
    );
    if let Ok(mut l) = lights.get_mut(vis.node_light) {
        l.intensity = if node_alive {
            260_000.0 + 80_000.0 * (t * 4.0).sin()
        } else {
            0.0
        };
    }

    // ---- bots
    let n = fight.enemies.len();
    grow(&mut vis.bots, n, || {
        commands
            .spawn((
                Mesh3d(a.bot_body.clone()),
                MeshMaterial3d(a.m_bot.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                for ex in [-0.1f32, 0.1] {
                    p.spawn((
                        Mesh3d(a.bot_eye.clone()),
                        MeshMaterial3d(a.m_red_eye.clone()),
                        Transform::from_xyz(ex, 0.03, -0.23),
                    ));
                }
                p.spawn((
                    Mesh3d(a.bot_ant.clone()),
                    MeshMaterial3d(a.m_dark.clone()),
                    Transform::from_xyz(0.0, 0.25, 0.0),
                ));
                p.spawn((
                    Mesh3d(a.disc.clone()),
                    MeshMaterial3d(a.m_red_glow.clone()),
                    Transform::from_xyz(0.0, -0.33, 0.0),
                ));
            })
            .id()
    });
    for (i, &e) in vis.bots.iter().enumerate() {
        let Some(en) = fight.enemies.get(i) else {
            place(e, None);
            continue;
        };
        let pos = Vec3::new(
            en.pos.x,
            BOT_Y + 0.07 * (t * 4.0 + en.id as f32).sin(),
            en.pos.y,
        );
        let look = Vec3::new(pl.pos.x, pos.y, pl.pos.y);
        let tf = if look.distance(pos) > 0.01 {
            Transform::from_translation(pos).looking_at(look, Vec3::Y)
        } else {
            Transform::from_translation(pos)
        };
        place(e, Some(tf));
        set_mat(e, if en.flash > 0.0 { &a.m_flash } else { &a.m_bot });
        gizmos.line(pos, node_pos, Color::srgba(0.5, 0.05, 0.12, 0.35));
    }

    // ---- bullets
    grow(&mut vis.bullets, fight.bullets.len(), || {
        commands
            .spawn((
                Mesh3d(a.shot.clone()),
                MeshMaterial3d(a.m_enemy_shot.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id()
    });
    for (i, &e) in vis.bullets.iter().enumerate() {
        let Some(b) = fight.bullets.get(i) else {
            place(e, None);
            continue;
        };
        let (mat, y, stretch) = match b.kind {
            ShotKind::Player => (&a.m_family_shot[fam_index(fight.family)], SHOT_Y, 2.6),
            ShotKind::Burst => (&a.m_burst, SHOT_Y, 2.0),
            ShotKind::Turret => (&a.m_turret_shot, SHOT_Y, 2.2),
            ShotKind::Enemy => (&a.m_enemy_shot, 0.6, 1.0),
        };
        let pos = Vec3::new(b.pos.x, y, b.pos.y);
        let dir = Vec3::new(b.vel.x, 0.0, b.vel.y);
        let r = if b.kind == ShotKind::Enemy {
            b.size * (0.85 + 0.15 * (t * 14.0).sin())
        } else {
            b.size * 0.7
        };
        let mut tf = Transform::from_translation(pos).with_scale(Vec3::new(r, r, r * stretch));
        if dir.length_squared() > 0.0001 {
            tf = tf.looking_to(dir, Vec3::Y);
        }
        place(e, Some(tf));
        set_mat(e, mat);
    }

    // ---- orbiting drones
    grow(&mut vis.drones, fight.drones.len(), || {
        commands
            .spawn((
                Mesh3d(a.cube.clone()),
                MeshMaterial3d(a.m_drone_perm.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id()
    });
    for (i, &e) in vis.drones.iter().enumerate() {
        let Some(d) = fight.drones.get(i) else {
            place(e, None);
            continue;
        };
        let off = Vec2::from_angle(d.angle) * d.radius;
        let pos = Vec3::new(
            pl.pos.x + off.x,
            0.7 + 0.08 * (d.angle * 2.0).sin(),
            pl.pos.y + off.y,
        );
        let fade = if d.life > 0.0 && d.life < 0.6 {
            d.life / 0.6
        } else {
            1.0
        };
        let s = if d.life < 0.0 { 0.16 } else { 0.2 } * fade;
        let tf = Transform::from_translation(pos)
            .with_rotation(Quat::from_euler(
                EulerRot::XYZ,
                d.angle * 2.0,
                d.angle * 3.0,
                0.0,
            ))
            .with_scale(Vec3::splat(s));
        place(e, alive.then_some(tf));
        set_mat(
            e,
            if d.life < 0.0 {
                &a.m_drone_perm
            } else {
                &a.m_drone_temp
            },
        );
    }

    // ---- Agent turrets
    grow(&mut vis.turrets, fight.turrets.len(), || {
        commands
            .spawn((
                Mesh3d(a.turret_base.clone()),
                MeshMaterial3d(a.m_turret.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(a.cube.clone()),
                    MeshMaterial3d(a.m_turret_top.clone()),
                    Transform::from_xyz(0.0, 0.26, 0.0).with_scale(Vec3::new(0.32, 0.18, 0.32)),
                    Spin(Vec3::new(0.0, 4.0, 0.0)),
                ));
                p.spawn((
                    Mesh3d(a.ring.clone()),
                    MeshMaterial3d(a.m_turret_top.clone()),
                    Transform::from_xyz(0.0, -0.13, 0.0),
                ));
            })
            .id()
    });
    for (i, &e) in vis.turrets.iter().enumerate() {
        let tf = fight.turrets.get(i).map(|tu| {
            let grow_in = ((9.0 - tu.life) * 6.0).clamp(0.0, 1.0);
            let shrink = (tu.life / 0.4).clamp(0.0, 1.0);
            Transform::from_xyz(tu.pos.x, 0.15, tu.pos.y)
                .with_scale(Vec3::splat(grow_in.min(shrink).max(0.01)))
        });
        place(e, tf);
    }

    // ---- beams
    grow(&mut vis.beams, fight.beams.len(), || {
        commands
            .spawn((
                Mesh3d(a.beam.clone()),
                MeshMaterial3d(a.m_beam_core.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(a.beam.clone()),
                    MeshMaterial3d(a.m_beam_glow.clone()),
                    Transform::from_scale(Vec3::new(3.0, 3.0, 1.0)),
                ));
            })
            .id()
    });
    for (i, &e) in vis.beams.iter().enumerate() {
        let tf = fight.beams.get(i).map(|b| {
            let from = Vec3::new(b.a.x, SHOT_Y, b.a.y);
            let to = Vec3::new(b.b.x, SHOT_Y, b.b.y);
            let len = from.distance(to).max(0.01);
            let w = 0.04 + 0.14 * (b.life / 0.25);
            let mut tf =
                Transform::from_translation((from + to) / 2.0).with_scale(Vec3::new(w, w, len));
            if len > 0.02 {
                tf = tf.looking_at(to, Vec3::Y);
            }
            tf
        });
        place(e, tf);
    }
}

/// Turn the simulation's one-off events into bursts of little cubes.
pub fn effects(
    mut commands: Commands,
    mut fight: ResMut<Fight>,
    a: Res<VisAssets>,
    parts: Query<(), With<Particle>>,
) {
    if fight.fx.is_empty() {
        return;
    }
    let mut budget = 500usize.saturating_sub(parts.iter().count());
    let mut seed = (fight.time * 1000.0) as u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed = seed.wrapping_add(0x9E37_79B9);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut burst = |commands: &mut Commands,
                     at: Vec3,
                     n: usize,
                     speed: f32,
                     size: f32,
                     life: f32,
                     mat: &Handle<StandardMaterial>,
                     gravity: bool,
                     rnd: &mut dyn FnMut() -> f32| {
        for _ in 0..n.min(budget) {
            let ang = rnd() * TAU;
            let up = if gravity {
                1.5 + rnd() * 3.0
            } else {
                (rnd() - 0.5) * 1.5
            };
            let sp = speed * (0.4 + rnd() * 0.8);
            let vel = Vec3::new(ang.cos() * sp, up, ang.sin() * sp);
            let l = life * (0.6 + rnd() * 0.6);
            commands.spawn((
                Mesh3d(a.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(size)),
                Particle {
                    vel,
                    life: l,
                    max: l,
                    size,
                    gravity,
                },
            ));
        }
        budget = budget.saturating_sub(n);
    };
    let fam_mat = a.m_family_shot[fam_index(fight.family)].clone();
    for fx in std::mem::take(&mut fight.fx) {
        match fx {
            Fx::Spark(p, kind) => {
                let at = Vec3::new(p.x, SHOT_Y, p.y);
                let m = match kind {
                    ShotKind::Enemy => &a.m_spark_red,
                    ShotKind::Burst => &a.m_spark_green,
                    ShotKind::Turret => &a.m_turret_shot,
                    ShotKind::Player => &fam_mat,
                };
                burst(&mut commands, at, 3, 3.0, 0.06, 0.25, m, false, &mut rnd);
            }
            Fx::Kill(p) => {
                let at = Vec3::new(p.x, BOT_Y, p.y);
                burst(
                    &mut commands,
                    at,
                    7,
                    3.0,
                    0.1,
                    0.9,
                    &a.m_debris,
                    true,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at,
                    6,
                    5.0,
                    0.07,
                    0.4,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at,
                    3,
                    2.0,
                    0.12,
                    0.25,
                    &a.m_spark_white,
                    false,
                    &mut rnd,
                );
            }
            Fx::Spawn(p) => {
                let at = Vec3::new(p.x, BOT_Y, p.y);
                burst(
                    &mut commands,
                    at,
                    4,
                    1.5,
                    0.06,
                    0.3,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
            }
            Fx::Burst(p) => {
                let at = Vec3::new(p.x, SHOT_Y, p.y);
                burst(
                    &mut commands,
                    at,
                    24,
                    7.0,
                    0.08,
                    0.35,
                    &a.m_spark_green,
                    false,
                    &mut rnd,
                );
            }
            Fx::Hurt => {
                let p = fight.player.pos;
                let at = Vec3::new(p.x, PLAYER_Y, p.y);
                burst(
                    &mut commands,
                    at,
                    8,
                    3.5,
                    0.07,
                    0.35,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
            }
            Fx::NodeDown => {
                let at = Vec3::new(NODE_POS.x, NODE_Y, NODE_POS.y);
                burst(
                    &mut commands,
                    at,
                    40,
                    5.0,
                    0.16,
                    1.6,
                    &a.m_debris,
                    true,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at,
                    40,
                    8.0,
                    0.1,
                    0.8,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at,
                    20,
                    4.0,
                    0.2,
                    0.5,
                    &a.m_spark_white,
                    false,
                    &mut rnd,
                );
            }
        }
    }
}

pub fn particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut p, mut tr) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if p.gravity {
            p.vel.y -= 14.0 * dt;
        } else {
            p.vel *= (1.0 - 4.0 * dt).max(0.0);
        }
        tr.translation += p.vel * dt;
        if tr.translation.y < p.size / 2.0 {
            tr.translation.y = p.size / 2.0;
            p.vel.y = -p.vel.y * 0.35;
            p.vel.x *= 0.6;
            p.vel.z *= 0.6;
        }
        let k = if p.gravity {
            (p.life / p.max * 3.0).min(1.0)
        } else {
            p.life / p.max
        };
        tr.scale = Vec3::splat(p.size * k.max(0.05));
        tr.rotate_y(dt * 6.0);
    }
}

/// Shake the camera when things explode; `fight.shake` decays by itself.
pub fn camera_shake(
    fight: Res<Fight>,
    time: Res<Time>,
    mut q: Query<(&CamBase, &mut Transform), With<GameCamera>>,
) {
    let t = time.elapsed_secs();
    for (base, mut tr) in &mut q {
        let s = fight.shake * 0.05;
        let off = base.0.right() * (t * 71.0).sin() * s + base.0.up() * (t * 53.0).cos() * s;
        tr.translation = base.0.translation + off;
    }
}
