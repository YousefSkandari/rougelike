//! Draws the fight: the follow camera, bots, shots, data shards, turrets,
//! beams and all the sparks. Your body is in `mech.rs`, the boss in
//! `boss.rs` and the room in `hall.rs`.
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use std::f32::consts::{FRAC_PI_4, PI, TAU};

use crate::GameCamera;
use crate::fight::{CardKind, Family, Fight, Fx, Outcome, ShotKind};
use crate::hall::floor_y;
use crate::input::Input;
use crate::maze::START;

/// How far the camera sits from what it looks at. The view is
/// orthographic, so this only matters for fog and depth.
pub const CAM_DIST: f32 = 40.0;
const SHOT_Y: f32 = 0.55;
const BOT_Y: f32 = 0.22;

pub fn glow(c: Color, k: f32) -> LinearRgba {
    let l = c.to_linear();
    LinearRgba::rgb(l.red * k, l.green * k, l.blue * k)
}

pub fn fam_index(f: Family) -> usize {
    Family::ALL.iter().position(|x| *x == f).unwrap_or(0)
}

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
        metallic: 0.45,
        ..default()
    })
}

// ------------------------------------------------------------------ building blocks

/// Shared meshes for building things out of boxes.
#[derive(Resource, Clone)]
pub struct Kit {
    pub cube: Handle<Mesh>,
    pub ball: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    /// Black, drawn inside out: a one-pixel outline around a shape.
    pub outline: Handle<StandardMaterial>,
    pub flash: Handle<StandardMaterial>,
}

/// A box of `size` at `at` (inside `parent`, if any), with a black pixel
/// outline `line` thick. Everything with a body is made of these.
pub fn block(
    c: &mut Commands,
    kit: &Kit,
    parent: Option<Entity>,
    size: Vec3,
    at: Vec3,
    mat: &Handle<StandardMaterial>,
    line: f32,
) -> Entity {
    let mut e = c.spawn((
        Mesh3d(kit.cube.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(at).with_scale(size),
        Visibility::default(),
    ));
    if let Some(p) = parent {
        e.insert(ChildOf(p));
    }
    let id = e.id();
    if line > 0.0 {
        c.spawn((
            Mesh3d(kit.cube.clone()),
            MeshMaterial3d(kit.outline.clone()),
            Transform::from_scale(Vec3::ONE + 2.0 * line / size),
            NotShadowCaster,
            ChildOf(id),
        ));
    }
    id
}

/// An empty joint that other parts hang from.
pub fn pivot(c: &mut Commands, parent: Option<Entity>, at: Vec3) -> Entity {
    let mut e = c.spawn((Transform::from_translation(at), Visibility::default()));
    if let Some(p) = parent {
        e.insert(ChildOf(p));
    }
    e.id()
}

/// Transform for a stick of `thick` from `a` to `b`.
pub fn stick(a: Vec3, b: Vec3, thick: f32) -> Transform {
    let len = a.distance(b).max(0.001);
    let mid = (a + b) / 2.0;
    let dir = (b - a) / len;
    let up = if dir.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
    Transform::from_translation(mid)
        .looking_to(dir, up)
        .with_scale(Vec3::new(thick, thick, len))
}

// ------------------------------------------------------------------ camera

/// The follow camera. Moving the mouse (or the right stick) turns it
/// around you, the wheel zooms.
#[derive(Resource)]
pub struct CamRig {
    /// Angle round the up axis; 45 degrees looks in from the start corner.
    pub yaw: f32,
    /// How steeply it looks down.
    pub pitch: f32,
    pub want_zoom: f32,
    /// What the camera shows right now (after smoothing and the intro).
    pub zoom: f32,
    pub focus: Vec3,
    follow: Vec3,
    smooth_zoom: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        let start = Vec3::new(START.x, 0.6, START.y);
        CamRig {
            yaw: FRAC_PI_4,
            pitch: 0.86,
            want_zoom: 10.0,
            zoom: 10.0,
            focus: start,
            follow: start,
            smooth_zoom: 10.0,
        }
    }
}

impl CamRig {
    /// Screen-up and screen-right as directions on the floor.
    pub fn ground_axes(&self) -> (Vec2, Vec2) {
        let h = Vec2::new(self.yaw.sin(), self.yaw.cos());
        (-h, Vec2::new(h.y, -h.x))
    }
    fn offset(&self, pitch: f32) -> Vec3 {
        let h = Vec2::new(self.yaw.sin(), self.yaw.cos());
        Vec3::new(h.x * pitch.cos(), pitch.sin(), h.y * pitch.cos())
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn camera(
    time: Res<Time>,
    inp: Res<Input>,
    fight: Res<Fight>,
    mut rig: ResMut<CamRig>,
    mut q: Query<(&mut Transform, &mut Projection), With<GameCamera>>,
) {
    let dt = time.delta_secs().min(0.05);
    rig.yaw = (rig.yaw + inp.orbit.x).rem_euclid(TAU);
    rig.pitch = (rig.pitch + inp.orbit.y).clamp(0.5, 1.35);
    rig.want_zoom = (rig.want_zoom * (1.0 + 0.12 * inp.zoom)).clamp(6.0, 28.0);

    // Follow you, a little ahead of where you're going; in the boss fight,
    // lean toward the boss so you can see both and pull back a bit.
    let p = fight.player.pos;
    let mut target = p + fight.player.vel.clamp_length_max(6.0) * 0.16;
    let mut zoom = rig.want_zoom;
    let b = &fight.boss;
    if b.awake() && fight.outcome == Outcome::Playing {
        let d = b.pos.distance(p);
        let k = (1.0 - (d - 8.0) / 10.0).clamp(0.0, 1.0);
        target += (b.pos - p) * 0.36 * k;
        zoom += 5.0 * k;
    }
    let goal = Vec3::new(target.x, floor_y(p) + 0.6, target.y);
    let k = 1.0 - (-6.0 * dt).exp();
    rig.follow = rig.follow.lerp(goal, k);
    rig.smooth_zoom += (zoom - rig.smooth_zoom) * (6.0 * dt).min(1.0);

    // Intro: start high over the whole hall, looking at the boss, then
    // swoop down to you.
    let intro = smoothstep((fight.time - 0.5) / 2.8);
    let focus = Vec3::new(0.0, 0.0, 0.0).lerp(rig.follow, intro);
    let zoom = 64.0 + (rig.smooth_zoom - 64.0) * intro;
    let pitch = 1.1 + (rig.pitch - 1.1) * intro;
    rig.zoom = zoom;
    rig.focus = focus;

    let off = rig.offset(pitch);
    let t = time.elapsed_secs();
    for (mut tf, mut proj) in &mut q {
        let look = Transform::from_translation(focus + off * CAM_DIST).looking_at(focus, Vec3::Y);
        let s = fight.shake * 0.035 * (zoom / 14.0);
        let jitter = look.right() * (t * 71.0).sin() * s + look.up() * (t * 53.0).cos() * s;
        *tf = look.with_translation(look.translation + jitter);
        if let Projection::Orthographic(o) = &mut *proj {
            o.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
                viewport_height: zoom,
            };
        }
    }
}

// ------------------------------------------------------------------ shared assets

#[derive(Resource)]
pub struct VisAssets {
    m_family_shot: [Handle<StandardMaterial>; 5],
    m_burst: Handle<StandardMaterial>,
    m_turret_shot: Handle<StandardMaterial>,
    m_enemy_shot: Handle<StandardMaterial>,
    m_boss_shot: Handle<StandardMaterial>,
    m_bot: Handle<StandardMaterial>,
    m_bot_dark: Handle<StandardMaterial>,
    m_eye: Handle<StandardMaterial>,
    m_eye_dim: Handle<StandardMaterial>,
    m_shard: Handle<StandardMaterial>,
    m_turret: Handle<StandardMaterial>,
    m_turret_top: Handle<StandardMaterial>,
    m_beam_core: Handle<StandardMaterial>,
    m_beam_glow: Handle<StandardMaterial>,
    m_debris: Handle<StandardMaterial>,
    m_spark_red: Handle<StandardMaterial>,
    m_spark_green: Handle<StandardMaterial>,
    m_spark_white: Handle<StandardMaterial>,
    m_spark_gold: Handle<StandardMaterial>,
    m_spark_cyan: Handle<StandardMaterial>,
    m_gas: Handle<StandardMaterial>,
    m_card: [Handle<StandardMaterial>; 4],
    m_ring_red: Handle<StandardMaterial>,
    m_ring_gold: Handle<StandardMaterial>,
    m_ring_white: Handle<StandardMaterial>,
}

fn card_index(k: CardKind) -> usize {
    CardKind::ALL.iter().position(|x| *x == k).unwrap_or(0)
}

/// Entities that mirror the simulation, reused frame to frame.
#[derive(Resource, Default)]
pub struct Vis {
    bots: Vec<BotVis>,
    bullets: Vec<Entity>,
    shards: Vec<Entity>,
    turrets: Vec<Entity>,
    beams: Vec<Entity>,
}

struct BotVis {
    root: Entity,
    body: Entity,
    eyes: [Entity; 2],
    legs: [Entity; 4],
}

#[derive(Component)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
    gravity: bool,
}

/// An expanding ring on the floor.
#[derive(Component)]
pub struct RingFx {
    life: f32,
    max: f32,
    radius: f32,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let add = |mats: &mut Assets<StandardMaterial>, c: LinearRgba| emissive(mats, c);
    let kit = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        ball: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        ring: meshes.add(Torus::new(0.93, 1.0)),
        outline: mats.add(StandardMaterial {
            base_color: Color::BLACK,
            unlit: true,
            cull_mode: Some(Face::Front),
            ..default()
        }),
        flash: add(&mut mats, LinearRgba::rgb(6.0, 6.0, 6.0)),
    };
    let additive = |mats: &mut Assets<StandardMaterial>, c: LinearRgba| {
        mats.add(StandardMaterial {
            base_color: Color::LinearRgba(c),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        })
    };
    let assets = VisAssets {
        m_family_shot: Family::ALL.map(|f| add(&mut mats, glow(f.color(), 9.0))),
        m_burst: add(&mut mats, LinearRgba::rgb(0.8, 8.0, 2.2)),
        m_turret_shot: add(&mut mats, LinearRgba::rgb(8.0, 3.5, 0.6)),
        m_enemy_shot: add(&mut mats, LinearRgba::rgb(9.0, 0.6, 1.0)),
        m_boss_shot: add(&mut mats, LinearRgba::rgb(10.0, 0.5, 2.6)),
        m_bot: metal(&mut mats, Color::srgb(0.32, 0.3, 0.34), 0.45),
        m_bot_dark: metal(&mut mats, Color::srgb(0.1, 0.09, 0.11), 0.6),
        m_eye: add(&mut mats, LinearRgba::rgb(7.0, 0.3, 0.5)),
        m_eye_dim: add(&mut mats, LinearRgba::rgb(0.6, 0.03, 0.05)),
        m_shard: add(&mut mats, LinearRgba::rgb(0.6, 5.0, 4.0)),
        m_turret: metal(&mut mats, Color::srgb(0.55, 0.5, 0.45), 0.35),
        m_turret_top: add(&mut mats, LinearRgba::rgb(6.0, 2.6, 0.4)),
        m_beam_core: add(&mut mats, LinearRgba::rgb(9.0, 9.0, 9.0)),
        m_beam_glow: additive(&mut mats, LinearRgba::rgb(1.6, 0.25, 0.4)),
        m_debris: metal(&mut mats, Color::srgb(0.35, 0.37, 0.43), 0.4),
        m_spark_red: add(&mut mats, LinearRgba::rgb(8.0, 0.8, 1.0)),
        m_spark_green: add(&mut mats, LinearRgba::rgb(1.0, 8.0, 2.5)),
        m_spark_white: add(&mut mats, LinearRgba::rgb(6.0, 6.0, 6.0)),
        m_spark_gold: add(&mut mats, LinearRgba::rgb(8.0, 5.0, 1.0)),
        m_spark_cyan: add(&mut mats, LinearRgba::rgb(0.8, 6.0, 6.0)),
        m_gas: additive(&mut mats, LinearRgba::rgb(0.35, 0.38, 0.42)),
        m_card: CardKind::ALL.map(|k| additive(&mut mats, glow(k.color(), 2.5))),
        m_ring_red: additive(&mut mats, LinearRgba::rgb(3.0, 0.25, 0.3)),
        m_ring_gold: additive(&mut mats, LinearRgba::rgb(3.0, 2.0, 0.5)),
        m_ring_white: additive(&mut mats, LinearRgba::rgb(2.5, 2.5, 2.5)),
    };
    commands.insert_resource(kit);
    commands.insert_resource(assets);
    commands.insert_resource(Vis::default());
}

/// Grow a pool of entities until it holds at least `n`.
fn grow<T>(pool: &mut Vec<T>, n: usize, mut spawn: impl FnMut() -> T) {
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
    kit: Res<Kit>,
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut Visibility), Without<Particle>>,
    mut mq: Query<&mut MeshMaterial3d<StandardMaterial>>,
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
    let ppos = fight.player.pos;

    // ---- bots: little four-legged crawlers with red eyes
    grow(&mut vis.bots, fight.enemies.len(), || {
        let root = pivot(&mut commands, None, Vec3::ZERO);
        commands.entity(root).insert(Visibility::Hidden);
        let body = block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.42, 0.2, 0.44),
            Vec3::new(0.0, 0.0, 0.0),
            &a.m_bot,
            0.04,
        );
        block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.3, 0.1, 0.3),
            Vec3::new(0.0, 0.12, 0.03),
            &a.m_bot_dark,
            0.0,
        );
        let eyes = [-0.09f32, 0.09].map(|x| {
            block(
                &mut commands,
                &kit,
                Some(root),
                Vec3::new(0.08, 0.06, 0.04),
                Vec3::new(x, 0.03, -0.23),
                &a.m_eye,
                0.0,
            )
        });
        let legs = [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(x, z)| {
            let hip = pivot(&mut commands, Some(root), Vec3::new(x * 0.2, 0.0, z * 0.14));
            block(
                &mut commands,
                &kit,
                Some(hip),
                Vec3::new(0.26, 0.05, 0.05),
                Vec3::new(x * 0.12, -0.08, 0.0),
                &a.m_bot_dark,
                0.0,
            );
            hip
        });
        BotVis {
            root,
            body,
            eyes,
            legs,
        }
    });
    for (i, bv) in vis.bots.iter().enumerate() {
        let Some(en) = fight.enemies.get(i) else {
            place(bv.root, None);
            continue;
        };
        let fy = floor_y(en.pos);
        let h = en.height();
        let asleep = !en.awake && en.flight.is_none();
        let bob = if asleep {
            -0.06
        } else {
            0.04 * (t * 9.0 + en.id as f32).sin().abs()
        };
        let pos = Vec3::new(en.pos.x, fy + BOT_Y + h + bob, en.pos.y);
        let look = if en.flight.is_some() {
            Quat::from_rotation_y(t * 12.0 + en.id as f32) * Quat::from_rotation_x(t * 9.0)
        } else {
            let d = ppos - en.pos;
            Quat::from_rotation_y(f32::atan2(-d.x, -d.y))
        };
        let grow_in = (en.age * 4.0).clamp(0.25, 1.0);
        place(
            bv.root,
            Some(
                Transform::from_translation(pos)
                    .with_rotation(look)
                    .with_scale(Vec3::splat(grow_in)),
            ),
        );
        set_mat(bv.body, if en.flash > 0.0 { &kit.flash } else { &a.m_bot });
        for e in bv.eyes {
            set_mat(e, if asleep { &a.m_eye_dim } else { &a.m_eye });
        }
        for (k, leg) in bv.legs.iter().enumerate() {
            let side = if k % 2 == 0 { -1.0 } else { 1.0 };
            let phase = t * 16.0 + en.id as f32 + k as f32 * PI / 2.0;
            let tuck = if en.flight.is_some() { 1.1 } else { 0.0 };
            let step = if asleep { 0.25 } else { 0.35 * phase.sin() };
            let tf = Transform::from_translation(Vec3::new(
                side * 0.2,
                0.0,
                if k < 2 { -0.14 } else { 0.14 },
            ))
            .with_rotation(
                Quat::from_rotation_z(side * (-0.35 - tuck)) * Quat::from_rotation_y(step),
            );
            place(*leg, Some(tf));
        }
    }

    // ---- bullets
    grow(&mut vis.bullets, fight.bullets.len(), || {
        commands
            .spawn((
                Mesh3d(kit.ball.clone()),
                MeshMaterial3d(a.m_enemy_shot.clone()),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ))
            .id()
    });
    for (i, &e) in vis.bullets.iter().enumerate() {
        let Some(b) = fight.bullets.get(i) else {
            place(e, None);
            continue;
        };
        let (mat, stretch) = match b.kind {
            ShotKind::Player => (&a.m_family_shot[fam_index(fight.family)], 2.6),
            ShotKind::Burst => (&a.m_burst, 2.0),
            ShotKind::Turret => (&a.m_turret_shot, 2.2),
            ShotKind::Enemy => (&a.m_enemy_shot, 1.0),
            ShotKind::Boss => (&a.m_boss_shot, 1.0),
        };
        let y = floor_y(b.pos) + SHOT_Y;
        let pos = Vec3::new(b.pos.x, y, b.pos.y);
        let dir = Vec3::new(b.vel.x, 0.0, b.vel.y);
        let r = if b.hostile() {
            b.size * (0.85 + 0.15 * (t * 14.0 + i as f32).sin())
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

    // ---- data shards: spinning glowing cubes
    grow(&mut vis.shards, fight.shards.len(), || {
        commands
            .spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(a.m_shard.clone()),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ))
            .id()
    });
    for (i, &e) in vis.shards.iter().enumerate() {
        let tf = fight.shards.get(i).map(|s| {
            let y = floor_y(s.pos) + 0.35 + 0.1 * (t * 4.0 + i as f32).sin();
            Transform::from_xyz(s.pos.x, y, s.pos.y)
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    t * 3.0 + i as f32,
                    0.6,
                    0.6,
                ))
                .with_scale(Vec3::splat(0.16 * (s.age * 6.0).clamp(0.3, 1.0)))
        });
        place(e, tf);
    }

    // ---- Agent turrets
    grow(&mut vis.turrets, fight.turrets.len(), || {
        let root = pivot(&mut commands, None, Vec3::ZERO);
        commands.entity(root).insert(Visibility::Hidden);
        block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.36, 0.26, 0.36),
            Vec3::new(0.0, 0.13, 0.0),
            &a.m_turret,
            0.04,
        );
        block(
            &mut commands,
            &kit,
            Some(root),
            Vec3::new(0.22, 0.12, 0.3),
            Vec3::new(0.0, 0.32, 0.0),
            &a.m_turret_top,
            0.03,
        );
        root
    });
    for (i, &e) in vis.turrets.iter().enumerate() {
        let tf = fight.turrets.get(i).map(|tu| {
            let grow_in = ((9.0 - tu.life) * 6.0).clamp(0.0, 1.0);
            let shrink = (tu.life / 0.4).clamp(0.0, 1.0);
            Transform::from_xyz(tu.pos.x, floor_y(tu.pos), tu.pos.y)
                .with_rotation(Quat::from_rotation_y(t * 3.0))
                .with_scale(Vec3::splat(grow_in.min(shrink).max(0.01)))
        });
        place(e, tf);
    }

    // ---- beams
    grow(&mut vis.beams, fight.beams.len(), || {
        let e = commands
            .spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(a.m_beam_core.clone()),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
            ))
            .id();
        commands.spawn((
            Mesh3d(kit.cube.clone()),
            MeshMaterial3d(a.m_beam_glow.clone()),
            Transform::from_scale(Vec3::new(3.0, 3.0, 1.0)),
            NotShadowCaster,
            ChildOf(e),
        ));
        e
    });
    for (i, &e) in vis.beams.iter().enumerate() {
        let tf = fight.beams.get(i).map(|b| {
            let y = floor_y(b.a) + SHOT_Y;
            let from = Vec3::new(b.a.x, y, b.a.y);
            let to = Vec3::new(b.b.x, y, b.b.y);
            let w = 0.05 + 0.15 * (b.life / 0.25);
            let mut tf = stick(from, to, w);
            tf.scale.x = w;
            tf.scale.y = w;
            tf
        });
        place(e, tf);
    }
}

/// Turn the simulation's one-off events into sparks, rings and debris.
pub fn effects(
    mut commands: Commands,
    mut fight: ResMut<Fight>,
    a: Res<VisAssets>,
    kit: Res<Kit>,
    parts: Query<(), With<Particle>>,
) {
    if fight.fx.is_empty() {
        return;
    }
    let mut budget = 600usize.saturating_sub(parts.iter().count());
    let mut seed = (fight.time * 1000.0) as u32 | 1;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let cube = kit.cube.clone();
    let mut burst = |commands: &mut Commands,
                     at: Vec3,
                     n: usize,
                     speed: f32,
                     size: f32,
                     life: f32,
                     mat: &Handle<StandardMaterial>,
                     gravity: bool,
                     rnd: &mut dyn FnMut() -> f32| {
        let n = n.min(budget);
        budget -= n;
        for _ in 0..n {
            let ang = rnd() * TAU;
            let up = if gravity {
                1.5 + rnd() * 3.5
            } else {
                (rnd() - 0.3) * 2.0
            };
            let sp = speed * (0.4 + rnd() * 0.8);
            let l = life * (0.6 + rnd() * 0.6);
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(size)),
                NotShadowCaster,
                Particle {
                    vel: Vec3::new(ang.cos() * sp, up, ang.sin() * sp),
                    life: l,
                    max: l,
                    size,
                    gravity,
                },
            ));
        }
    };
    let ring = |commands: &mut Commands,
                at: Vec2,
                radius: f32,
                life: f32,
                mat: &Handle<StandardMaterial>| {
        commands.spawn((
            Mesh3d(kit.ring.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(at.x, floor_y(at) + 0.08, at.y).with_scale(Vec3::splat(0.1)),
            NotShadowCaster,
            RingFx {
                life,
                max: life,
                radius,
            },
        ));
    };
    let fam_mat = a.m_family_shot[fam_index(fight.family)].clone();
    let pl = fight.player.pos;
    let at = |p: Vec2, y: f32| Vec3::new(p.x, floor_y(p) + y, p.y);
    for fx in std::mem::take(&mut fight.fx) {
        match fx {
            Fx::Spark(p, kind) => {
                let m = match kind {
                    ShotKind::Enemy | ShotKind::Boss => &a.m_spark_red,
                    ShotKind::Burst => &a.m_spark_green,
                    ShotKind::Turret => &a.m_turret_shot,
                    ShotKind::Player => &fam_mat,
                };
                burst(
                    &mut commands,
                    at(p, SHOT_Y),
                    3,
                    3.0,
                    0.06,
                    0.25,
                    m,
                    false,
                    &mut rnd,
                );
            }
            Fx::Kill(p) => {
                burst(
                    &mut commands,
                    at(p, 0.3),
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
                    at(p, 0.3),
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
                    at(p, 0.3),
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
                burst(
                    &mut commands,
                    at(p, 0.1),
                    6,
                    1.2,
                    0.07,
                    0.5,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
                ring(&mut commands, p, 0.9, 0.4, &a.m_ring_red);
            }
            Fx::Hurt => {
                burst(
                    &mut commands,
                    at(pl, 0.8),
                    8,
                    3.5,
                    0.07,
                    0.35,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
            }
            Fx::Ring(p, k) => {
                let r = match k {
                    CardKind::Burst => 3.0,
                    CardKind::Firewall => 3.2,
                    _ => 1.6,
                };
                ring(&mut commands, p, r, 0.45, &a.m_card[card_index(k)]);
            }
            Fx::Stomp(p) => {
                ring(&mut commands, p, 6.0, 0.6, &a.m_ring_red);
                ring(&mut commands, p, 3.5, 0.4, &a.m_ring_white);
                burst(
                    &mut commands,
                    at(p, 0.2),
                    24,
                    7.0,
                    0.12,
                    1.0,
                    &a.m_debris,
                    true,
                    &mut rnd,
                );
            }
            Fx::Launch(p) => {
                burst(
                    &mut commands,
                    at(p, 2.2),
                    12,
                    3.0,
                    0.09,
                    0.5,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
            }
            Fx::Upgrade => {
                burst(
                    &mut commands,
                    at(pl, 0.9),
                    16,
                    3.0,
                    0.07,
                    0.6,
                    &a.m_spark_gold,
                    false,
                    &mut rnd,
                );
                ring(&mut commands, pl, 1.4, 0.5, &a.m_ring_gold);
            }
            Fx::Form => {
                burst(
                    &mut commands,
                    at(pl, 0.9),
                    30,
                    5.0,
                    0.08,
                    0.8,
                    &a.m_spark_white,
                    false,
                    &mut rnd,
                );
                ring(&mut commands, pl, 3.0, 0.7, &a.m_ring_white);
            }
            Fx::Wake(p) | Fx::Enrage(p) => {
                ring(&mut commands, p, 9.0, 0.9, &a.m_ring_red);
                burst(
                    &mut commands,
                    at(p, 1.6),
                    30,
                    6.0,
                    0.1,
                    0.8,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
            }
            Fx::PylonDown(p) => {
                burst(
                    &mut commands,
                    at(p, 2.0),
                    30,
                    6.0,
                    0.12,
                    1.2,
                    &a.m_debris,
                    true,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at(p, 2.0),
                    30,
                    8.0,
                    0.08,
                    0.6,
                    &a.m_spark_cyan,
                    false,
                    &mut rnd,
                );
                ring(&mut commands, p, 4.0, 0.6, &a.m_ring_white);
            }
            Fx::Collect(p) => {
                burst(
                    &mut commands,
                    at(p, 0.8),
                    4,
                    2.0,
                    0.05,
                    0.3,
                    &a.m_spark_cyan,
                    false,
                    &mut rnd,
                );
            }
            Fx::Suppress => {
                for _ in 0..40 {
                    let off = Vec2::new(rnd() - 0.5, rnd() - 0.5) * 14.0;
                    burst(
                        &mut commands,
                        at(pl + off, 2.5),
                        1,
                        1.0,
                        0.5,
                        2.0,
                        &a.m_gas,
                        false,
                        &mut rnd,
                    );
                }
            }
            Fx::BossDown(p) => {
                burst(
                    &mut commands,
                    at(p, 1.5),
                    60,
                    7.0,
                    0.16,
                    1.8,
                    &a.m_debris,
                    true,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at(p, 1.5),
                    60,
                    10.0,
                    0.1,
                    1.0,
                    &a.m_spark_red,
                    false,
                    &mut rnd,
                );
                burst(
                    &mut commands,
                    at(p, 1.5),
                    30,
                    5.0,
                    0.2,
                    0.6,
                    &a.m_spark_white,
                    false,
                    &mut rnd,
                );
                ring(&mut commands, p, 12.0, 1.2, &a.m_ring_white);
            }
        }
    }
}

pub fn particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform), Without<RingFx>>,
    mut rings: Query<(Entity, &mut RingFx, &mut Transform), Without<Particle>>,
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
    for (e, mut r, mut tr) in &mut rings {
        r.life -= dt;
        if r.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = 1.0 - r.life / r.max;
        let s = r.radius * (1.0 - (1.0 - k) * (1.0 - k));
        let thin = (1.0 - k).max(0.05);
        tr.scale = Vec3::new(s.max(0.05), thin * 2.0, s.max(0.05));
    }
}
