//! The Broodmother: a six-legged machine the size of a server row, chained
//! to the centre of the hall by its power cables. Its belly is built from
//! server racks (their lights pulse with the hall), its back is a hatch that
//! throws bots, and four pylons feed it power through thick cables.
//!
//! Its legs are placed with inverse kinematics and walk in a tripod gait,
//! so it really steps around the dais. You can read its attacks: the jaws
//! open and its mouth glows before a volley, the hatch opens before it
//! throws bots, and it rears up before a stomp.
use bevy::prelude::*;
use std::f32::consts::PI;

use crate::fight::{BossMode, Fight, Outcome, PYLON_HP};
use crate::hall::{HallAssets, floor_y};
use crate::maze::PYLONS;
use crate::visuals::{Kit, block, glow, metal, pivot, stick};

/// How much bigger than its model it is drawn.
const S: f32 = 1.25;
const UPPER: f32 = 2.0 * S;
const LOWER: f32 = 2.35 * S;
const CABLE_SEGS: usize = 10;

struct Leg {
    /// Hip, in the body's space.
    hip: Vec3,
    /// Where the foot rests, in the root's space (on the floor).
    rest: Vec2,
    side: f32,
    tripod: usize,
    /// Planted foot in the world, and the step it is taking.
    foot: Vec3,
    from: Vec3,
    to: Vec3,
    step: f32,
    upper: Entity,
    lower: Entity,
    knee: Entity,
    claw: Entity,
}

struct PylonVis {
    column: Entity,
    cap: Entity,
    rings: [Entity; 3],
}

#[derive(Resource)]
pub struct BossVis {
    root: Entity,
    body: Entity,
    hatch: [Entity; 2],
    jaws: [Entity; 2],
    mouth: Entity,
    hulls: Vec<(Entity, Handle<StandardMaterial>)>,
    hit: Handle<StandardMaterial>,
    eyes: Handle<StandardMaterial>,
    strips: Handle<StandardMaterial>,
    inner: Handle<StandardMaterial>,
    legs: Vec<Leg>,
    light: Entity,
    pylons: Vec<PylonVis>,
    ring_live: Handle<StandardMaterial>,
    ring_rage: Handle<StandardMaterial>,
    ring_dead: Handle<StandardMaterial>,
    cables: Vec<[Entity; CABLE_SEGS]>,
    cable_dark: Handle<StandardMaterial>,
    cable_lit: Handle<StandardMaterial>,
    planted: bool,
}

pub fn setup(
    mut commands: Commands,
    kit: Res<Kit>,
    hall: Res<HallAssets>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let c = &mut commands;
    let armour = metal(&mut mats, Color::srgb(0.16, 0.15, 0.18), 0.4);
    let plate = metal(&mut mats, Color::srgb(0.3, 0.29, 0.33), 0.35);
    let dark = metal(&mut mats, Color::srgb(0.06, 0.055, 0.07), 0.6);
    let red_plate = metal(&mut mats, Color::srgb(0.45, 0.07, 0.1), 0.4);
    let eyes = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(8.0, 0.3, 0.4),
        ..default()
    });
    let strips = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(3.0, 0.15, 0.25),
        ..default()
    });
    let inner = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(4.0, 0.4, 0.2),
        ..default()
    });
    let joint = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(2.4, 0.12, 0.2),
        ..default()
    });
    // being hit brightens its armour a little (it is hit all the time)
    let hit = mats.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.22, 0.25),
        emissive: LinearRgba::rgb(0.3, 0.1, 0.12),
        perceptual_roughness: 0.5,
        ..default()
    });
    let line = 0.06;

    let root = pivot(c, None, Vec3::ZERO);
    let body = pivot(c, Some(root), Vec3::new(0.0, 1.5, 0.0));
    let mut hulls = vec![];

    // thorax with armour plates and heat-sink fins
    let thorax = block(
        c,
        &kit,
        Some(body),
        Vec3::new(2.0, 0.9, 2.3),
        Vec3::ZERO,
        &armour,
        line,
    );
    hulls.push((thorax, armour.clone()));
    for z in [-0.75f32, 0.0, 0.75] {
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(1.6, 0.16, 0.6),
            Vec3::new(0.0, 0.52, z),
            &plate,
            line,
        );
    }
    for z in [-0.5f32, -0.15, 0.2, 0.55] {
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(0.08, 0.36, 0.22),
            Vec3::new(0.0, 0.76, z),
            &dark,
            0.0,
        );
    }
    for x in [-1.03f32, 1.03] {
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(0.06, 0.12, 1.9),
            Vec3::new(x, 0.12, 0.0),
            &strips,
            0.0,
        );
        // and a glowing seam down each side of the abdomen
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(0.06, 0.1, 2.4),
            Vec3::new(x * 1.23, 0.85, 2.35),
            &strips,
            0.0,
        );
    }

    // abdomen: built from server racks, lights and all
    let abdomen = block(
        c,
        &kit,
        Some(body),
        Vec3::new(2.5, 1.45, 2.7),
        Vec3::new(0.0, 0.3, 2.35),
        &dark,
        line,
    );
    hulls.push((abdomen, dark.clone()));
    for side in [-1.0f32, 1.0] {
        c.spawn((
            Mesh3d(hall.rack_meshes[9].clone()),
            MeshMaterial3d(hall.led_mats[0].clone()),
            Transform::from_xyz(side * 1.2, -0.38, 2.35)
                .with_rotation(Quat::from_rotation_y(side * PI / 2.0))
                .with_scale(Vec3::new(1.25, 0.55, 0.3)),
            ChildOf(body),
        ));
    }
    // the rear: cable sockets
    for x in [-0.8f32, 0.8] {
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(0.4, 0.4, 0.2),
            Vec3::new(x, 0.45, 3.75),
            &red_plate,
            line,
        );
    }
    // the hatch on its back that throws bots, and the furnace under it
    block(
        c,
        &kit,
        Some(body),
        Vec3::new(2.1, 0.06, 2.3),
        Vec3::new(0.0, 1.0, 2.35),
        &inner,
        0.0,
    );
    let hatch = [-1.0f32, 1.0].map(|side| {
        let hinge = pivot(c, Some(body), Vec3::new(side * 1.25, 1.06, 2.35));
        block(
            c,
            &kit,
            Some(hinge),
            Vec3::new(1.25, 0.12, 2.5),
            Vec3::new(-side * 0.62, 0.0, 0.0),
            &plate,
            line,
        );
        hinge
    });

    // head, eyes, jaws and the glow in its mouth
    let head = block(
        c,
        &kit,
        Some(body),
        Vec3::new(1.4, 0.8, 1.0),
        Vec3::new(0.0, -0.05, -1.6),
        &armour,
        line,
    );
    hulls.push((head, armour.clone()));
    block(
        c,
        &kit,
        Some(body),
        Vec3::new(1.2, 0.18, 0.1),
        Vec3::new(0.0, 0.3, -2.12),
        &red_plate,
        0.0,
    );
    for (x, y, s) in [
        (-0.32f32, 0.06f32, 0.32f32),
        (0.32, 0.06, 0.32),
        (-0.6, 0.2, 0.16),
        (0.6, 0.2, 0.16),
        (-0.15, 0.27, 0.13),
        (0.15, 0.27, 0.13),
    ] {
        block(
            c,
            &kit,
            Some(body),
            Vec3::new(s, s * 0.8, 0.08),
            Vec3::new(x, y, -2.12),
            &eyes,
            0.0,
        );
    }
    let mouth = block(
        c,
        &kit,
        Some(body),
        Vec3::new(0.5, 0.25, 0.1),
        Vec3::new(0.0, -0.3, -2.13),
        &inner,
        0.0,
    );
    let jaws = [-1.0f32, 1.0].map(|side| {
        let hinge = pivot(c, Some(body), Vec3::new(side * 0.5, -0.38, -2.0));
        block(
            c,
            &kit,
            Some(hinge),
            Vec3::new(0.2, 0.2, 0.95),
            Vec3::new(0.0, 0.0, -0.42),
            &plate,
            line,
        );
        block(
            c,
            &kit,
            Some(hinge),
            Vec3::new(0.12, 0.3, 0.12),
            Vec3::new(-side * 0.05, -0.15, -0.85),
            &dark,
            0.0,
        );
        hinge
    });

    let light = c
        .spawn((
            PointLight {
                color: Color::srgb(1.0, 0.15, 0.2),
                intensity: 0.0,
                range: 10.0,
                ..default()
            },
            Transform::from_xyz(0.0, 0.2, -2.8),
            ChildOf(body),
        ))
        .id();

    // legs: three on each side, hips along the thorax
    let mut legs = vec![];
    for (i, (z, rz)) in [(-0.8f32, -2.4f32), (0.1, 0.0), (1.0, 2.3)]
        .into_iter()
        .enumerate()
    {
        for side in [-1.0f32, 1.0] {
            let tripod = (i + if side < 0.0 { 0 } else { 1 }) % 2;
            let upper = block(
                c,
                &kit,
                None,
                Vec3::new(0.34, 0.34, UPPER),
                Vec3::ZERO,
                &armour,
                line,
            );
            let lower = block(
                c,
                &kit,
                None,
                Vec3::new(0.26, 0.26, LOWER),
                Vec3::ZERO,
                &plate,
                line,
            );
            let knee = block(c, &kit, None, Vec3::splat(0.42), Vec3::ZERO, &joint, 0.05);
            let claw = block(
                c,
                &kit,
                None,
                Vec3::new(0.22, 0.5, 0.22),
                Vec3::ZERO,
                &dark,
                0.05,
            );
            legs.push(Leg {
                hip: Vec3::new(side * 0.95, -0.05, z),
                rest: Vec2::new(side * 3.1, rz),
                side,
                tripod,
                foot: Vec3::ZERO,
                from: Vec3::ZERO,
                to: Vec3::ZERO,
                step: 1.0,
                upper,
                lower,
                knee,
                claw,
            });
        }
    }

    // the pylons and their cables
    let ring_live = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.5, 3.0, 4.0),
        ..default()
    });
    let ring_rage = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(5.0, 0.3, 0.4),
        ..default()
    });
    let ring_dead = metal(&mut mats, Color::srgb(0.08, 0.08, 0.09), 0.7);
    let pylon_body = metal(&mut mats, Color::srgb(0.2, 0.22, 0.27), 0.35);
    let mut pylons = vec![];
    for p in PYLONS {
        let base = pivot(c, None, Vec3::new(p.x, floor_y(p), p.y));
        block(
            c,
            &kit,
            Some(base),
            Vec3::new(1.1, 0.35, 1.1),
            Vec3::new(0.0, 0.17, 0.0),
            &dark,
            line,
        );
        let column = block(
            c,
            &kit,
            Some(base),
            Vec3::new(0.62, 2.6, 0.62),
            Vec3::new(0.0, 1.6, 0.0),
            &pylon_body,
            line,
        );
        let rings = [0.9f32, 1.6, 2.3].map(|y| {
            block(
                c,
                &kit,
                Some(base),
                Vec3::new(0.8, 0.12, 0.8),
                Vec3::new(0.0, y, 0.0),
                &ring_live,
                0.04,
            )
        });
        let cap = block(
            c,
            &kit,
            Some(base),
            Vec3::new(0.9, 0.25, 0.9),
            Vec3::new(0.0, 3.0, 0.0),
            &dark,
            line,
        );
        pylons.push(PylonVis { column, cap, rings });
    }
    let cable_dark = metal(&mut mats, Color::srgb(0.05, 0.05, 0.06), 0.5);
    let cable_lit = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: glow(Color::srgb(0.4, 0.9, 1.0), 4.0),
        ..default()
    });
    let cables = PYLONS
        .iter()
        .map(|_| {
            std::array::from_fn(|_| block(c, &kit, None, Vec3::ONE, Vec3::ZERO, &cable_dark, 0.0))
        })
        .collect();

    commands.insert_resource(BossVis {
        root,
        body,
        hatch,
        jaws,
        mouth,
        hulls,
        hit,
        eyes,
        strips,
        inner,
        legs,
        light,
        pylons,
        ring_live,
        ring_rage,
        ring_dead,
        cables,
        cable_dark,
        cable_lit,
        planted: false,
    });
}

/// Two-bone inverse kinematics: where the knee goes for a leg from `hip`
/// to `foot`, bending toward `pole`.
fn knee(hip: Vec3, foot: Vec3, pole: Vec3) -> Vec3 {
    let d = (foot - hip)
        .length()
        .clamp((UPPER - LOWER).abs() + 0.01, UPPER + LOWER - 0.01);
    let u = (foot - hip).normalize_or(Vec3::NEG_Y);
    let x = (UPPER * UPPER - LOWER * LOWER + d * d) / (2.0 * d);
    let h = (UPPER * UPPER - x * x).max(0.0).sqrt();
    let p = (pole - u * pole.dot(u)).normalize_or(Vec3::Y);
    hip + u * x + p * h
}

#[allow(clippy::too_many_arguments)]
pub fn sync(
    time: Res<Time>,
    fight: Res<Fight>,
    kit: Res<Kit>,
    mut vis: ResMut<BossVis>,
    mut q: Query<(&mut Transform, &mut Visibility)>,
    mut mq: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut lights: Query<&mut PointLight>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let b = &fight.boss;
    let won = fight.outcome == Outcome::Won;
    let vis = &mut *vis;

    // ---- root: where it stands and which way it faces
    let fy = floor_y(b.pos);
    let yaw = f32::atan2(-b.face.x, -b.face.y);
    let root_tf = Transform::from_xyz(b.pos.x, fy, b.pos.y)
        .with_rotation(Quat::from_rotation_y(yaw))
        .with_scale(Vec3::splat(S));

    // ---- body height and tilt tell you what it's about to do
    let moving = b.vel.length() > 0.1;
    let (mut height, mut pitch, mut roll) = (1.55, 0.0, 0.0);
    match b.mode {
        BossMode::Dormant => {
            height = 0.95 + 0.05 * (t * 1.4).sin();
        }
        BossMode::Stunned => {
            height = 0.8;
            roll = 0.12 * (t * 9.0).sin();
            pitch = 0.1;
        }
        BossMode::Stomp => {
            height = 1.55 + 0.7 * b.telegraph;
            pitch = -0.28 * b.telegraph;
        }
        BossMode::Volley => {
            pitch = -0.08 * b.telegraph;
        }
        _ => {}
    }
    if moving {
        height += 0.07 * (t * 9.0).sin();
    }
    height -= 0.4 * (-b.since_stomp * 6.0).exp();
    if won {
        height = 0.45;
        roll = 0.25;
        pitch = 0.15;
    }
    let body_tf = Transform::from_xyz(0.0, height, 0.0).with_rotation(Quat::from_euler(
        EulerRot::XYZ,
        pitch,
        0.0,
        roll,
    ));
    let body_world = root_tf.mul_transform(body_tf);
    if let Ok((mut tf, mut v)) = q.get_mut(vis.root) {
        *tf = root_tf;
        v.set_if_neq(Visibility::Inherited);
    }
    if let Ok((mut tf, _)) = q.get_mut(vis.body) {
        let shake = if b.flash > 0.0 {
            0.04 * (t * 60.0).sin()
        } else {
            0.0
        };
        *tf = body_tf.with_translation(body_tf.translation + Vec3::X * shake);
    }

    // ---- hatch, jaws and mouth
    for (i, &h) in vis.hatch.iter().enumerate() {
        let side = if i == 0 { -1.0 } else { 1.0 };
        if let Ok((mut tf, _)) = q.get_mut(h) {
            tf.rotation = Quat::from_rotation_z(-side * b.hatch * 1.9);
        }
    }
    let open = if b.mode == BossMode::Volley {
        b.telegraph.max(b.fire_flash)
    } else {
        b.fire_flash * 0.5
    };
    for (i, &j) in vis.jaws.iter().enumerate() {
        let side = if i == 0 { -1.0 } else { 1.0 };
        if let Ok((mut tf, _)) = q.get_mut(j) {
            tf.rotation = Quat::from_rotation_y(side * (0.15 + 0.6 * open));
        }
    }
    if let Ok((mut tf, _)) = q.get_mut(vis.mouth) {
        let s = 0.3 + 1.2 * open;
        tf.scale = Vec3::new(0.5 * s, 0.25 * s, 0.1);
    }

    // ---- glow: eyes, side strips, the furnace under the hatch
    let awake = b.awake() && !won;
    let rage = if b.enraged { 1.6 } else { 1.0 };
    let eye_k = if won {
        0.0
    } else if !b.awake() {
        0.25 + 0.15 * (t * 1.4).sin()
    } else if b.mode == BossMode::Stunned {
        if (t * 14.0).sin() > 0.3 { 1.0 } else { 0.05 }
    } else {
        (1.0 + 1.5 * b.telegraph + 2.0 * b.fire_flash) * rage
    };
    if let Some(mut m) = mats.get_mut(&vis.eyes) {
        m.emissive = LinearRgba::rgb(8.0 * eye_k, 0.3 * eye_k, 0.4 * eye_k);
    }
    let strip_k = if awake {
        (0.6 + 0.4 * (t * 6.0 * rage).sin()) * rage + b.fire_flash * 2.0
    } else {
        0.15
    };
    if let Some(mut m) = mats.get_mut(&vis.strips) {
        m.emissive = LinearRgba::rgb(3.0 * strip_k, 0.15 * strip_k, 0.25 * strip_k);
    }
    let inner_k = if won {
        0.0
    } else {
        0.4 + 1.1 * b.hatch + 0.9 * open
    };
    if let Some(mut m) = mats.get_mut(&vis.inner) {
        m.emissive = LinearRgba::rgb(4.0 * inner_k, 0.45 * inner_k, 0.2 * inner_k);
    }
    for (e, base) in &vis.hulls {
        if let Ok(mut mm) = mq.get_mut(*e) {
            let want = if b.flash > 0.0 { &vis.hit } else { base };
            if mm.0 != *want {
                mm.0 = want.clone();
            }
        }
    }
    if let Ok(mut l) = lights.get_mut(vis.light) {
        l.intensity = if awake {
            (60_000.0 + 180_000.0 * b.telegraph + 300_000.0 * b.fire_flash) * rage
        } else if won {
            0.0
        } else {
            15_000.0
        };
    }

    // ---- legs: tripod gait with inverse kinematics
    let rest_world = |leg: &Leg| {
        let p = root_tf.transform_point(Vec3::new(leg.rest.x, 0.0, leg.rest.y));
        let lead = Vec3::new(b.vel.x, 0.0, b.vel.y) * 0.35;
        let q = p + lead;
        Vec3::new(q.x, floor_y(Vec2::new(q.x, q.z)), q.z)
    };
    if !vis.planted || won {
        for leg in &mut vis.legs {
            let mut r = rest_world(leg);
            if won {
                r = root_tf.transform_point(Vec3::new(leg.rest.x * 1.25, 0.0, leg.rest.y * 1.2));
            }
            leg.foot = r;
            leg.step = 1.0;
        }
        vis.planted = true;
    }
    let step_time = if b.enraged { 0.2 } else { 0.28 };
    for g in 0..2 {
        let other_planted = vis
            .legs
            .iter()
            .filter(|l| l.tripod != g)
            .all(|l| l.step >= 1.0);
        let needs = vis
            .legs
            .iter()
            .filter(|l| l.tripod == g && l.step >= 1.0)
            .any(|l| l.foot.distance(rest_world(l)) > 0.85 * S);
        if other_planted && needs && !won {
            for leg in vis.legs.iter_mut().filter(|l| l.tripod == g) {
                let r = rest_world(leg);
                if leg.foot.distance(r) > 0.2 * S {
                    leg.from = leg.foot;
                    leg.to = r;
                    leg.step = 0.0;
                }
            }
        }
    }
    for (i, leg) in vis.legs.iter_mut().enumerate() {
        if leg.step < 1.0 {
            leg.step = (leg.step + dt / step_time).min(1.0);
            let k = leg.step;
            leg.foot = leg.from.lerp(leg.to, k) + Vec3::Y * (k * PI).sin() * 0.6 * S;
        }
        let mut foot = leg.foot;
        // front legs lift high before a stomp, then slam down
        if b.mode == BossMode::Stomp && i < 2 {
            let raised =
                rest_world(leg) + (Vec3::Y * 2.0 + Vec3::new(b.face.x, 0.0, b.face.y) * 0.8) * S;
            foot = foot.lerp(raised, b.telegraph);
        }
        let hip = body_world.transform_point(leg.hip);
        let out = body_world.rotation * Vec3::new(leg.side, 0.0, 0.0);
        let k = knee(hip, foot, out + Vec3::Y * 1.6);
        if let Ok((mut tf, _)) = q.get_mut(leg.upper) {
            let mut s = stick(hip, k, 0.34 * S);
            s.scale.z = UPPER;
            *tf = s;
        }
        if let Ok((mut tf, _)) = q.get_mut(leg.lower) {
            let mut s = stick(k, foot, 0.26 * S);
            s.scale.z = LOWER;
            *tf = s;
        }
        if let Ok((mut tf, _)) = q.get_mut(leg.knee) {
            *tf = Transform::from_translation(k).with_scale(Vec3::splat(0.42 * S));
        }
        if let Ok((mut tf, _)) = q.get_mut(leg.claw) {
            *tf = Transform::from_translation(foot + Vec3::Y * 0.06)
                .with_rotation(root_tf.rotation)
                .with_scale(Vec3::new(0.42, 0.12, 0.5) * S);
        }
    }

    // ---- pylons and the cables that feed it
    for (k, (pv, py)) in vis.pylons.iter().zip(&fight.pylons).enumerate() {
        let alive = py.alive();
        if let Ok((mut tf, _)) = q.get_mut(pv.column) {
            if alive {
                *tf = Transform::from_xyz(0.0, 1.6, 0.0).with_scale(Vec3::new(0.62, 2.6, 0.62));
            } else {
                *tf = Transform::from_xyz(0.0, 0.85, 0.0)
                    .with_rotation(Quat::from_rotation_z(0.15))
                    .with_scale(Vec3::new(0.62, 1.1, 0.62));
            }
        }
        if let Ok((_, mut v)) = q.get_mut(pv.cap) {
            v.set_if_neq(if alive {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        let ring_mat = if !alive {
            &vis.ring_dead
        } else if py.flash > 0.0 {
            &kit.flash
        } else if b.enraged {
            &vis.ring_rage
        } else {
            &vis.ring_live
        };
        for (n, r) in pv.rings.iter().enumerate() {
            if let Ok(mut mm) = mq.get_mut(*r)
                && mm.0 != *ring_mat
            {
                mm.0 = ring_mat.clone();
            }
            if let Ok((mut tf, mut v)) = q.get_mut(*r) {
                v.set_if_neq(if alive || n == 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
                let hp = py.hp / PYLON_HP;
                let pulse = if alive {
                    1.0 + 0.08 * (t * 5.0 + n as f32 + k as f32).sin() * hp
                } else {
                    1.0
                };
                tf.scale = Vec3::new(0.8 * pulse, 0.12, 0.8 * pulse);
            }
        }
        // cable from the top of the pylon to the socket on its back
        let top = Vec3::new(py.pos.x, floor_y(py.pos) + 2.9, py.pos.y);
        let right = body_world.rotation * Vec3::X;
        let side = if (top - body_world.translation).dot(right) > 0.0 {
            0.8
        } else {
            -0.8
        };
        let socket_local = Vec3::new(side, 0.45, 3.85);
        let socket = body_world.transform_point(socket_local);
        let len = top.distance(socket);
        let sag = 0.5 + 0.12 * len;
        let point = |s: f32| top.lerp(socket, s) - Vec3::Y * sag * 4.0 * s * (1.0 - s);
        let lit = ((t * 7.0 + k as f32 * 2.5) as usize) % CABLE_SEGS;
        for (n, seg) in vis.cables[k].iter().enumerate() {
            let a = point(n as f32 / CABLE_SEGS as f32);
            let bb = point((n + 1) as f32 / CABLE_SEGS as f32);
            if let Ok((mut tf, mut v)) = q.get_mut(*seg) {
                if alive && !won {
                    *tf = stick(a, bb, 0.16);
                    tf.scale.z += 0.06;
                    v.set_if_neq(Visibility::Inherited);
                } else {
                    v.set_if_neq(Visibility::Hidden);
                }
            }
            if let Ok(mut mm) = mq.get_mut(*seg) {
                let want = if CABLE_SEGS - 1 - n == lit {
                    &vis.cable_lit
                } else {
                    &vis.cable_dark
                };
                if mm.0 != *want {
                    mm.0 = want.clone();
                }
            }
        }
    }
}
