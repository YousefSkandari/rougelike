//! The boss's server hall in 3D: rows of server racks forming the maze, the
//! floor, the boss's dais, the vents bots climb out of, the lights, and the
//! room's mood.
//!
//! The racks' lights move with the boss. They breathe slowly while it
//! sleeps, dim while it winds up an attack, and a red wave rolls out through
//! the whole hall every time it fires. Every attack heats the room, and the
//! hotter it gets the more the air fills with haze and dust. A stomp knocks
//! the power out for a moment.
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::GameCamera;
use crate::fight::{Fight, MAX_HEAT, Outcome};
use crate::maze::{Axis, CHAMBER, FAR, Rng, Segment, ring};
use crate::visuals::CamRig;

/// The colour of the dark the hall fades into.
pub const FOG: Color = Color::srgb(0.018, 0.026, 0.05);
const RACK_H: f32 = 2.25;
const RACK_TALL: f32 = 2.75;
/// Texture pixels per world unit. The camera shows about 20 screen pixels
/// per unit, so textures land close to one texel per pixel.
const PX: f32 = 16.0;
const ATLAS_W: f32 = 128.0;
const ATLAS_H: f32 = 48.0;
/// Rings of racks that light up one after another in an attack wave.
const BANDS: usize = 9;
/// Racks blink in a few groups, out of step with each other.
const GROUPS: usize = 3;
/// Racks further out than this are scenery and never need cutting away.
const SCENERY: f32 = 32.5;
/// How fast an attack's light wave rolls out through the hall.
const WAVE_SPEED: f32 = 26.0;

/// Height of the floor: the boss's dais has two low steps.
pub fn floor_y(p: Vec2) -> f32 {
    let r = ring(p);
    if r < 7.0 {
        0.2
    } else if r < 8.0 {
        0.1
    } else {
        0.0
    }
}

fn band_of(p: Vec2) -> usize {
    (((ring(p) - CHAMBER).max(0.0) / 4.4) as usize).min(BANDS - 1)
}

fn band_radius(b: usize) -> f32 {
    CHAMBER + 2.2 + b as f32 * 4.4
}

// ------------------------------------------------------------------ meshes

/// Builds one mesh out of many quads.
#[derive(Default)]
pub struct MeshBuilder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

/// A texture region: left, top, right, bottom (0..1).
#[derive(Clone, Copy)]
pub struct Uv(pub f32, pub f32, pub f32, pub f32);

impl Uv {
    pub const WHITE: Uv = Uv(0.0, 0.0, 0.0, 0.0);
}

impl MeshBuilder {
    /// A quad from 4 corners, counter-clockwise seen from the front:
    /// bottom-left, bottom-right, top-right, top-left.
    pub fn quad(&mut self, c: [Vec3; 4], uv: Uv, col: [f32; 4]) {
        let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or_zero();
        let base = self.pos.len() as u32;
        let uvs = [[uv.0, uv.3], [uv.2, uv.3], [uv.2, uv.1], [uv.0, uv.1]];
        for i in 0..4 {
            self.pos.push(c[i].into());
            self.nrm.push(n.into());
            self.uv.push(uvs[i]);
            self.col.push(col);
        }
        self.idx
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// An axis-aligned box. `faces` gives the texture region for
    /// +Z, -Z, +X, -X, +Y, -Y; `None` leaves that face out.
    pub fn cuboid(&mut self, min: Vec3, max: Vec3, faces: [Option<Uv>; 6], col: [f32; 4]) {
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        let v = Vec3::new;
        let quads = [
            [v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)],
            [v(x1, y0, z0), v(x0, y0, z0), v(x0, y1, z0), v(x1, y1, z0)],
            [v(x1, y0, z1), v(x1, y0, z0), v(x1, y1, z0), v(x1, y1, z1)],
            [v(x0, y0, z0), v(x0, y0, z1), v(x0, y1, z1), v(x0, y1, z0)],
            [v(x0, y1, z1), v(x1, y1, z1), v(x1, y1, z0), v(x0, y1, z0)],
            [v(x0, y0, z0), v(x1, y0, z0), v(x1, y0, z1), v(x0, y0, z1)],
        ];
        for (q, f) in quads.into_iter().zip(faces) {
            if let Some(uv) = f {
                self.quad(q, uv, col);
            }
        }
    }

    /// Copy another builder in, moved and turned by `tf`.
    pub fn append(&mut self, other: &MeshBuilder, tf: &Transform) {
        let base = self.pos.len() as u32;
        for p in &other.pos {
            self.pos.push(tf.transform_point(Vec3::from(*p)).into());
        }
        for n in &other.nrm {
            self.nrm.push((tf.rotation * Vec3::from(*n)).into());
        }
        self.uv.extend_from_slice(&other.uv);
        self.col.extend_from_slice(&other.col);
        self.idx.extend(other.idx.iter().map(|i| i + base));
    }

    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32(self.idx))
    }
}

const WHITE4: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

// ------------------------------------------------------------------ textures

/// A small RGBA picture we paint pixel by pixel.
struct Tex {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Tex {
    fn new(w: usize, h: usize) -> Self {
        Tex {
            w,
            h,
            px: vec![0; w * h * 4],
        }
    }
    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 4;
            self.px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }
    fn image(self) -> Image {
        let mut img = Image::new(
            Extent3d {
                width: self.w as u32,
                height: self.h as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            self.px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        img.sampler = ImageSampler::nearest();
        img
    }
}

fn shade(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

/// Raised-floor tiles: a solid one and a perforated one, side by side.
fn floor_texture() -> Image {
    let mut t = Tex::new(32, 16);
    let mut rng = Rng::new(77);
    for tile in 0..2 {
        let x0 = tile * 16;
        for y in 0..16 {
            for x in 0..16 {
                let n = rng.range(0.94, 1.06);
                let mut c = shade([46, 53, 70], n);
                if tile == 1 && (2..14).contains(&x) && (2..14).contains(&y) {
                    c = shade([52, 60, 78], n);
                    if x % 3 == 2 && y % 3 == 2 {
                        c = [17, 21, 30];
                    }
                }
                if x == 0 || y == 0 {
                    c = [66, 76, 98];
                }
                if x == 15 || y == 15 {
                    c = [20, 24, 34];
                }
                t.set(x0 + x, y, c);
            }
        }
        if tile == 0 {
            for (x, y) in [(2, 2), (13, 2), (2, 13), (13, 13)] {
                t.set(x0 + x, y, [28, 33, 45]);
            }
        }
    }
    t.image()
}

// Colours for the racks.
const FRAME: [u8; 3] = [22, 25, 33];
const LIP: [u8; 3] = [64, 70, 88];
const UNIT_A: [u8; 3] = [42, 48, 62];
const UNIT_B: [u8; 3] = [50, 56, 72];
const BLANK: [u8; 3] = [29, 32, 41];
const VENT: [u8; 3] = [16, 18, 24];
/// LED colours. They are pale on purpose: the hall tints them all at once,
/// red during an attack, so they need some of every colour in them.
const LEDS: [[[u8; 3]; 2]; 4] = [
    [[140, 255, 170], [140, 190, 255]],
    [[150, 245, 255], [235, 240, 255]],
    [[140, 255, 170], [255, 214, 150]],
    [[140, 190, 255], [205, 175, 255]],
];

/// Server fronts (4 styles), the cabled back, the side panel and the top,
/// all in one atlas; plus a matching map of what glows.
fn rack_textures() -> (Image, Image) {
    let mut base = Tex::new(ATLAS_W as usize, ATLAS_H as usize);
    let mut glow = Tex::new(ATLAS_W as usize, ATLAS_H as usize);
    let mut rng = Rng::new(4242);
    // ---- fronts
    for style in 0..4 {
        let x0 = style * 16;
        base.fill(x0, 0, 16, 48, FRAME);
        base.fill(x0, 0, 16, 1, LIP);
        // status strip across the top: this is what flashes red
        base.fill(x0 + 2, 2, 12, 1, [70, 74, 84]);
        glow.fill(x0 + 2, 2, 12, 1, [230, 230, 230]);
        let mut y = 4;
        while y < 46 {
            let h = [3, 3, 3, 3, 4, 5][rng.below(6)].min(46 - y);
            let blank = rng.chance(0.12);
            let c = if blank {
                BLANK
            } else if rng.chance(0.5) {
                UNIT_A
            } else {
                UNIT_B
            };
            base.fill(x0 + 1, y, 14, h - 1, c);
            if !blank {
                base.fill(x0 + 1, y, 14, 1, shade(c, 1.3));
                if h >= 4 {
                    for x in (2..10).step_by(2) {
                        base.set(x0 + x, y + 1, VENT);
                        base.set(x0 + x + 1, y + 2, VENT);
                    }
                } else {
                    for x in (2..9).step_by(3) {
                        base.fill(x0 + x, y + 1, 2, 1, VENT);
                    }
                }
                let n = 1 + rng.below(2);
                for k in 0..n {
                    let led = LEDS[style][rng.below(2)];
                    let lx = x0 + 13 - k * 2;
                    base.set(lx, y + 1, shade(led, 0.35));
                    glow.set(lx, y + 1, led);
                }
            }
            y += h;
        }
        base.fill(x0, 46, 16, 2, [14, 16, 21]);
    }
    // ---- back: bundles of cables, power strip, power supplies
    let cables: [[u8; 3]; 6] = [
        [46, 96, 200],
        [210, 170, 40],
        [180, 50, 64],
        [110, 118, 135],
        [220, 120, 40],
        [60, 160, 90],
    ];
    let x0 = 64;
    base.fill(x0, 0, 16, 48, FRAME);
    base.fill(x0, 0, 16, 1, LIP);
    base.fill(x0 + 2, 2, 12, 1, [70, 74, 84]);
    glow.fill(x0 + 2, 2, 12, 1, [230, 230, 230]);
    for x in 3..15 {
        if rng.chance(0.25) {
            continue;
        }
        let c = cables[rng.below(cables.len())];
        let top = 4 + rng.below(6);
        for y in top..38 {
            let tie = y % 7 == 0;
            base.set(x0 + x, y, if tie { shade(c, 0.45) } else { c });
        }
    }
    base.fill(x0 + 1, 3, 1, 43, [12, 13, 17]);
    for y in (6..44).step_by(5) {
        base.set(x0 + 1, y, [120, 70, 20]);
        glow.set(x0 + 1, y, [255, 170, 80]);
    }
    for px in [x0 + 3, x0 + 9] {
        base.fill(px, 38, 5, 7, [40, 44, 54]);
        base.fill(px, 38, 5, 1, [60, 66, 80]);
        base.set(px + 1, 40, [40, 120, 60]);
        glow.set(px + 1, 40, [140, 255, 170]);
    }
    // ---- side panel
    let x0 = 80;
    base.fill(x0, 0, 16, 48, [34, 39, 52]);
    base.fill(x0, 0, 16, 1, LIP);
    base.fill(x0 + 7, 1, 1, 46, [44, 50, 66]);
    for y in (9..16).chain(30..37) {
        if y % 2 == 0 {
            base.fill(x0 + 2, y, 4, 1, VENT);
            base.fill(x0 + 10, y, 4, 1, VENT);
        }
    }
    base.fill(x0, 46, 16, 2, [14, 16, 21]);
    // ---- top: fan grille and cable holes
    let x0 = 96;
    base.fill(x0, 0, 16, 16, [30, 34, 44]);
    base.fill(x0, 0, 16, 1, [48, 54, 70]);
    base.fill(x0, 0, 1, 16, [48, 54, 70]);
    for y in 0..16 {
        for x in 0..16 {
            let d = Vec2::new(x as f32 - 7.5, y as f32 - 7.5).length();
            if (3.0..5.2).contains(&d) {
                base.set(x0 + x, y, [18, 20, 26]);
            }
            if d < 3.0 && (x == 7 || y == 7) {
                base.set(x0 + x, y, [44, 50, 64]);
            }
        }
    }
    for (x, y) in [(2, 2), (13, 13)] {
        base.fill(x0 + x - 1, y - 1, 2, 2, [8, 9, 12]);
    }
    (base.image(), glow.image())
}

fn atlas(x: f32, y: f32, w: f32, h: f32) -> Uv {
    Uv(
        x / ATLAS_W,
        y / ATLAS_H,
        (x + w) / ATLAS_W,
        (y + h) / ATLAS_H,
    )
}

/// One run of `cabinets` server cabinets along local X, fronts facing +Z,
/// standing on the origin.
fn rack_mesh(cabinets: u8, tall: bool, style: u8) -> MeshBuilder {
    let h = if tall { RACK_TALL } else { RACK_H };
    let rows = h * PX;
    let front = atlas(style as f32 * 16.0, 0.0, 16.0, rows);
    let back = atlas(64.0, 0.0, 16.0, rows);
    let side = atlas(80.0, 0.0, 16.0, rows);
    let top = atlas(96.0, 0.0, 16.0, 16.0);
    let mut mb = MeshBuilder::default();
    let n = cabinets as i32;
    for i in 0..n {
        let cx = i as f32 - (n - 1) as f32 / 2.0;
        let first = i == 0;
        let last = i == n - 1;
        mb.cuboid(
            Vec3::new(cx - 0.5, 0.0, -0.46),
            Vec3::new(cx + 0.5, h, 0.46),
            [
                Some(front),
                Some(back),
                last.then_some(side),
                first.then_some(side),
                Some(top),
                None,
            ],
            WHITE4,
        );
    }
    mb
}

/// Which way a segment's fronts face. Radial rows alternate, so between
/// them you get cold aisles (rack fronts and their lights) and hot aisles
/// (backs and cables), like a real datacentre. The main aisles to the
/// boss are cold aisles, lined with lights.
fn rack_yaw(s: &Segment) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let c = s.center;
    let (radial, lat, out) = match s.axis {
        // Runs along x. Radial on the ±x sides, where the lateral is z.
        Axis::X => (c.x.abs() >= c.y.abs(), c.y, c.y),
        Axis::Z => (c.y.abs() >= c.x.abs(), c.x, c.x),
    };
    // +1: the front faces +z (Axis::X) or +x (Axis::Z).
    let facing = if radial {
        let k = ((lat.abs() - 3.5) / 4.0).round() as i32;
        let inward = k % 2 == 0;
        let toward = if inward { -lat.signum() } else { lat.signum() };
        if toward >= 0.0 { 1.0 } else { -1.0 }
    } else if out >= 0.0 {
        1.0
    } else {
        -1.0
    };
    match (s.axis, facing > 0.0) {
        (Axis::X, true) => 0.0,
        (Axis::X, false) => PI,
        (Axis::Z, true) => FRAC_PI_2,
        (Axis::Z, false) => -FRAC_PI_2,
    }
}

// ------------------------------------------------------------------ resources

#[derive(Resource)]
pub struct HallAssets {
    /// Indexed by `variant(cabinets, tall, style)`.
    racks: Vec<MeshBuilder>,
    pub rack_meshes: Vec<Handle<Mesh>>,
    /// Indexed by `band * GROUPS + group`.
    pub led_mats: Vec<Handle<StandardMaterial>>,
    floor_mat: Handle<StandardMaterial>,
    paint_mat: Handle<StandardMaterial>,
    dais_glow: Handle<StandardMaterial>,
    vent_glow: Handle<StandardMaterial>,
    dust_mat: Handle<StandardMaterial>,
}

fn variant(cabinets: u8, tall: bool, style: u8) -> usize {
    ((cabinets.clamp(1, 2) - 1) as usize * 2 + tall as usize) * 4 + (style % 4) as usize
}

/// Everything that belongs to one hall layout, removed on rebuild.
#[derive(Component)]
pub struct HallPiece;

/// A rack that can be cut down when it stands between you and the camera.
#[derive(Component)]
pub struct Rack {
    pos: Vec2,
    /// Half its size along world x and z.
    half: Vec2,
    h: f32,
    cut: f32,
}

#[derive(Component)]
pub struct HallLight {
    base: f32,
    chamber: bool,
}

#[derive(Component)]
pub struct Dust {
    pos: Vec3,
    vel: Vec3,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (rack_base, rack_glow) = rack_textures();
    let rack_base = images.add(rack_base);
    let rack_glow = images.add(rack_glow);
    let floor_tex = images.add(floor_texture());

    let mut racks = vec![];
    for cab in 1..=2u8 {
        for tall in [false, true] {
            for style in 0..4u8 {
                racks.push(rack_mesh(cab, tall, style));
            }
        }
    }
    let rack_meshes = racks
        .iter()
        .map(|mb| {
            let mut copy = MeshBuilder::default();
            copy.append(mb, &Transform::IDENTITY);
            meshes.add(copy.build())
        })
        .collect();
    let led_mats = (0..BANDS * GROUPS)
        .map(|_| {
            mats.add(StandardMaterial {
                base_color_texture: Some(rack_base.clone()),
                emissive_texture: Some(rack_glow.clone()),
                emissive: LinearRgba::rgb(2.0, 2.4, 3.0),
                perceptual_roughness: 0.55,
                metallic: 0.35,
                ..default()
            })
        })
        .collect();
    let floor_mat = mats.add(StandardMaterial {
        base_color_texture: Some(floor_tex),
        perceptual_roughness: 0.45,
        metallic: 0.3,
        ..default()
    });
    let paint_mat = mats.add(StandardMaterial {
        perceptual_roughness: 0.7,
        ..default()
    });
    let glow = |mats: &mut Assets<StandardMaterial>, c: LinearRgba| {
        mats.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: c,
            ..default()
        })
    };
    let dais_glow = glow(&mut mats, LinearRgba::rgb(3.0, 0.2, 0.3));
    let vent_glow = glow(&mut mats, LinearRgba::rgb(2.5, 0.15, 0.2));
    let dust_mat = mats.add(StandardMaterial {
        base_color: Color::linear_rgba(0.6, 0.6, 0.7, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // Dust in the air around the camera. More of it shows as the room heats.
    let mut rng = Rng::new(99);
    for _ in 0..160 {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(dust_mat.clone()),
            Transform::from_scale(Vec3::splat(0.06)),
            Visibility::Hidden,
            NotShadowCaster,
            Dust {
                pos: Vec3::new(
                    rng.range(-14.0, 14.0),
                    rng.range(0.2, 4.0),
                    rng.range(-14.0, 14.0),
                ),
                vel: Vec3::new(
                    rng.range(-0.3, 0.3),
                    rng.range(0.05, 0.35),
                    rng.range(-0.3, 0.3),
                ),
            },
        ));
    }

    // The key light: cold, from high up, casting the racks' shadows.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.62, 0.72, 1.0),
            illuminance: 1600.0,
            shadow_maps_enabled: true,
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 10.0,
            maximum_distance: 110.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-8.0, 20.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(HallAssets {
        racks,
        rack_meshes,
        led_mats,
        floor_mat,
        paint_mat,
        dais_glow,
        vent_glow,
        dust_mat,
    });
}

/// Build (or rebuild, for a new layout) the hall's 3D scene.
pub fn rebuild(
    mut commands: Commands,
    fight: Res<Fight>,
    assets: Res<HallAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<HallPiece>>,
    mut built: Local<Option<u32>>,
) {
    let maze = &fight.maze;
    if *built == Some(maze.seed) {
        return;
    }
    *built = Some(maze.seed);
    for e in &old {
        commands.entity(e).despawn();
    }

    // ---- racks: the playable ones one by one (so they can be cut away),
    // the endless rows outside merged into a few big meshes.
    let mut scenery: Vec<MeshBuilder> = (0..BANDS * GROUPS)
        .map(|_| MeshBuilder::default())
        .collect();
    let mut rng = Rng::new(maze.seed ^ 0xBEEF);
    for s in &maze.segments {
        let v = variant(s.cabinets, s.tall, s.style);
        let yaw = rack_yaw(s);
        let group = rng.below(GROUPS);
        let mat = band_of(s.center) * GROUPS + group;
        let tf = Transform::from_xyz(s.center.x, 0.0, s.center.y)
            .with_rotation(Quat::from_rotation_y(yaw));
        if ring(s.center) >= SCENERY {
            scenery[mat].append(&assets.racks[v], &tf);
            continue;
        }
        let len = s.cabinets as f32 * 0.5;
        let half = match s.axis {
            Axis::X => Vec2::new(len, 0.46),
            Axis::Z => Vec2::new(0.46, len),
        };
        commands.spawn((
            Mesh3d(assets.rack_meshes[v].clone()),
            MeshMaterial3d(assets.led_mats[mat].clone()),
            tf,
            Rack {
                pos: s.center,
                half,
                h: if s.tall { RACK_TALL } else { RACK_H },
                cut: 1.0,
            },
            HallPiece,
        ));
    }
    for (i, mb) in scenery.into_iter().enumerate() {
        if mb.is_empty() {
            continue;
        }
        commands.spawn((
            Mesh3d(meshes.add(mb.build())),
            MeshMaterial3d(assets.led_mats[i].clone()),
            Transform::default(),
            HallPiece,
        ));
    }

    // ---- floor tiles, out to the far dark
    let solid = Uv(0.0, 0.0, 0.5, 1.0);
    let perf = Uv(0.5, 0.0, 1.0, 1.0);
    let mut floor = MeshBuilder::default();
    let far = FAR.ceil() as i32 + 2;
    for z in -far..far {
        for x in -far..far {
            let p = Vec2::new(x as f32 + 0.5, z as f32 + 0.5);
            let r = ring(p);
            let y = floor_y(p);
            let lat = if p.x.abs() >= p.y.abs() {
                p.y.abs()
            } else {
                p.x.abs()
            };
            let seam = (p.x.abs() - p.y.abs()).abs() < 1.6;
            let cold = !seam && (lat < 3.5 || ((lat - 3.5) / 4.0).floor() as i32 % 2 == 1);
            let n = 0.9 + 0.2 * Rng::new((x * 92821 + z * 68917) as u32).f();
            let (uv, col) = if r < 7.0 {
                (solid, [0.55 * n, 0.42 * n, 0.48 * n, 1.0])
            } else if r < CHAMBER {
                (perf, [0.7 * n, 0.62 * n, 0.66 * n, 1.0])
            } else if cold {
                (perf, [0.82 * n, 0.92 * n, 1.08 * n, 1.0])
            } else {
                (solid, [1.0 * n, 0.95 * n, 0.9 * n, 1.0])
            };
            let (x0, x1, z0, z1) = (x as f32, x as f32 + 1.0, z as f32, z as f32 + 1.0);
            floor.quad(
                [
                    Vec3::new(x0, y, z1),
                    Vec3::new(x1, y, z1),
                    Vec3::new(x1, y, z0),
                    Vec3::new(x0, y, z0),
                ],
                uv,
                col,
            );
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(floor.build())),
        MeshMaterial3d(assets.floor_mat.clone()),
        Transform::default(),
        HallPiece,
    ));

    // ---- painted lines: hazard stripes round the chamber, guide lines and
    // arrows down the main aisles, all pointing at the boss
    let mut paint = MeshBuilder::default();
    let yellow = [0.95, 0.68, 0.12, 1.0];
    let black = [0.05, 0.05, 0.06, 1.0];
    let flat = |mb: &mut MeshBuilder, a: Vec2, b: Vec2, w: f32, col: [f32; 4]| {
        let d = (b - a).normalize_or_zero();
        let s = Vec2::new(d.y, -d.x) * (w / 2.0);
        let y = floor_y((a + b) / 2.0) + 0.006;
        let v = |p: Vec2| Vec3::new(p.x, y, p.y);
        mb.quad([v(a - s), v(b - s), v(b + s), v(a + s)], Uv::WHITE, col);
    };
    // stripes on the outer step's edge
    let r = 8.75;
    let corners = [
        Vec2::new(r, r),
        Vec2::new(-r, r),
        Vec2::new(-r, -r),
        Vec2::new(r, -r),
    ];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let len = a.distance(b);
        let n = (len / 0.5) as i32;
        for i in 0..n {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            let col = if i % 2 == 0 { yellow } else { black };
            flat(&mut paint, a.lerp(b, t0), a.lerp(b, t1), 0.4, col);
        }
    }
    for side in 0..4 {
        let dir = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y][side];
        let lat = Vec2::new(-dir.y, dir.x);
        for l in [-2.6f32, 2.6] {
            flat(
                &mut paint,
                dir * 9.6 + lat * l,
                dir * 30.0 + lat * l,
                0.12,
                [0.8, 0.6, 0.14, 1.0],
            );
        }
        let mut u = 11.0;
        while u < 29.0 {
            // a chevron pointing inward
            let tip = dir * (u - 0.4);
            for s in [-1.0f32, 1.0] {
                flat(
                    &mut paint,
                    tip,
                    tip + dir * 0.8 + lat * 0.8 * s,
                    0.18,
                    [0.75, 0.55, 0.12, 1.0],
                );
            }
            u += 4.0;
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(paint.build())),
        MeshMaterial3d(assets.paint_mat.clone()),
        Transform::default(),
        NotShadowCaster,
        HallPiece,
    ));

    // ---- the dais steps: sides of the raised tiles, with glowing trim
    let mut sides = MeshBuilder::default();
    let mut trim = MeshBuilder::default();
    for (r, top) in [(8.0f32, 0.1f32), (7.0, 0.2)] {
        let below = top - 0.1;
        for k in 0..4 {
            let (a, b) = (
                [
                    Vec2::new(r, r),
                    Vec2::new(-r, r),
                    Vec2::new(-r, -r),
                    Vec2::new(r, -r),
                ][k],
                [
                    Vec2::new(-r, r),
                    Vec2::new(-r, -r),
                    Vec2::new(r, -r),
                    Vec2::new(r, r),
                ][k],
            );
            let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
            // faces outward: from b to a seen from outside
            sides.quad(
                [v(b, below), v(a, below), v(a, top), v(b, top)],
                Uv::WHITE,
                [0.12, 0.12, 0.15, 1.0],
            );
        }
        // a glowing strip along the top of the step's edge, wide enough to
        // stay a solid line in the low-res image
        let r_in = r - 0.09;
        let c = [
            Vec2::new(r_in, r_in),
            Vec2::new(-r_in, r_in),
            Vec2::new(-r_in, -r_in),
            Vec2::new(r_in, -r_in),
        ];
        for k in 0..4 {
            flat(&mut trim, c[k], c[(k + 1) % 4], 0.14, WHITE4);
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(sides.build())),
        MeshMaterial3d(assets.paint_mat.clone()),
        Transform::default(),
        HallPiece,
    ));
    commands.spawn((
        Mesh3d(meshes.add(trim.build())),
        MeshMaterial3d(assets.dais_glow.clone()),
        Transform::default(),
        NotShadowCaster,
        HallPiece,
    ));

    // ---- vents: grates in the floor that bots climb out of
    let mut grate = MeshBuilder::default();
    let mut glow = MeshBuilder::default();
    for v in &maze.vents {
        let y = 0.012;
        let c = Vec3::new(v.x, y, v.y);
        grate.cuboid(
            c + Vec3::new(-0.42, -0.01, -0.42),
            c + Vec3::new(0.42, 0.0, 0.42),
            [None, None, None, None, Some(Uv::WHITE), None],
            [0.05, 0.03, 0.04, 1.0],
        );
        for i in 0..4 {
            let x = -0.3 + i as f32 * 0.2;
            grate.cuboid(
                c + Vec3::new(x - 0.03, 0.0, -0.36),
                c + Vec3::new(x + 0.03, 0.02, 0.36),
                [None, None, None, None, Some(Uv::WHITE), None],
                [0.2, 0.2, 0.24, 1.0],
            );
        }
        for (a, b) in [
            (Vec3::new(-0.46, 0.0, -0.46), Vec3::new(0.46, 0.015, -0.4)),
            (Vec3::new(-0.46, 0.0, 0.4), Vec3::new(0.46, 0.015, 0.46)),
            (Vec3::new(-0.46, 0.0, -0.4), Vec3::new(-0.4, 0.015, 0.4)),
            (Vec3::new(0.4, 0.0, -0.4), Vec3::new(0.46, 0.015, 0.4)),
        ] {
            glow.cuboid(
                c + a,
                c + b,
                [None, None, None, None, Some(Uv::WHITE), None],
                WHITE4,
            );
        }
    }
    if !maze.vents.is_empty() {
        commands.spawn((
            Mesh3d(meshes.add(grate.build())),
            MeshMaterial3d(assets.paint_mat.clone()),
            Transform::default(),
            NotShadowCaster,
            HallPiece,
        ));
        commands.spawn((
            Mesh3d(meshes.add(glow.build())),
            MeshMaterial3d(assets.vent_glow.clone()),
            Transform::default(),
            NotShadowCaster,
            HallPiece,
        ));
    }

    // ---- lights: pools of cold light down the main aisles, and the
    // chamber's lights that follow the boss's mood
    for side in 0..4 {
        let dir = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y][side];
        for u in [13.0f32, 20.0, 27.0] {
            let p = dir * u;
            commands.spawn((
                PointLight {
                    color: Color::srgb(0.55, 0.75, 1.0),
                    intensity: 90_000.0,
                    range: 9.0,
                    ..default()
                },
                Transform::from_xyz(p.x, 4.5, p.y),
                HallLight {
                    base: 90_000.0,
                    chamber: false,
                },
                HallPiece,
            ));
        }
    }
    for (x, z) in [(0.0f32, 0.0f32), (5.0, -5.0), (-5.0, 5.0)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.3, 0.35),
                intensity: 160_000.0,
                range: 12.0,
                ..default()
            },
            Transform::from_xyz(x, 6.0, z),
            HallLight {
                base: 160_000.0,
                chamber: true,
            },
            HallPiece,
        ));
    }
}

/// Shrink racks that stand between you and the camera, so you're never
/// hidden. They sink to a stub, like an architect's cutaway.
pub fn cutaway(
    time: Res<Time>,
    fight: Res<Fight>,
    rig: Res<CamRig>,
    mut racks: Query<(&mut Rack, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);
    let (fwd, right) = rig.ground_axes();
    let p = fight.player.pos;
    let slope = rig.pitch.tan().max(0.3);
    for (mut r, mut tf) in &mut racks {
        let d = r.pos - p;
        let across = d.dot(right).abs();
        let toward = -d.dot(fwd);
        let ex = r.half.x * right.x.abs() + r.half.y * right.y.abs();
        let ey = r.half.x * fwd.x.abs() + r.half.y * fwd.y.abs();
        let hide = across < ex + 1.2 && toward + ey > -0.3 && toward - ey < r.h / slope + 0.6;
        let want = if hide { 0.12 } else { 1.0 };
        if (r.cut - want).abs() > 0.001 {
            r.cut += (want - r.cut) * (12.0 * dt).min(1.0);
            tf.scale.y = r.cut;
        }
    }
}

#[derive(Default)]
pub struct Pulses {
    /// (time it set off, strength)
    list: Vec<(f32, f32)>,
    last_flash: f32,
    last_stomp: f32,
    clock: f32,
}

fn hash(n: u32) -> f32 {
    Rng::new(n).f()
}

/// The room's mood: rack lights in step with the boss, heat haze, dust,
/// fire suppression gas and power cuts.
#[allow(clippy::too_many_arguments)]
pub fn atmosphere(
    time: Res<Time>,
    fight: Res<Fight>,
    assets: Res<HallAssets>,
    rig: Res<CamRig>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut cams: Query<(&mut DistanceFog, &mut AmbientLight, &mut Camera), With<GameCamera>>,
    mut lights: Query<(&mut PointLight, &HallLight)>,
    mut dust: Query<(&mut Dust, &mut Transform, &mut Visibility)>,
    mut st: Local<Pulses>,
) {
    let dt = time.delta_secs().min(0.05);
    st.clock += dt;
    let t = st.clock;
    let b = &fight.boss;
    let heat = (fight.heat / MAX_HEAT).clamp(0.0, 1.0);

    // Every attack sends a wave of red light out through the racks.
    if b.fire_flash > st.last_flash + 0.5 {
        st.list.push((t, if b.enraged { 1.4 } else { 1.0 }));
    }
    st.last_flash = b.fire_flash;
    if b.since_stomp < st.last_stomp {
        st.list.push((t, 2.0));
    }
    st.last_stomp = b.since_stomp;
    st.list.retain(|(t0, _)| t - t0 < 4.0);
    if fight.outcome == Outcome::Won {
        st.list.clear();
    }

    // Base colour: cool blue-white, warming to amber as the room heats,
    // red once the boss is enraged.
    let cool = Vec3::new(1.7, 2.3, 3.0);
    let hot = Vec3::new(3.0, 1.6, 0.8);
    let mut base = cool.lerp(hot, heat.powf(1.4));
    if b.enraged {
        base = base.lerp(Vec3::new(3.2, 0.7, 0.6), 0.55);
    }
    if fight.outcome == Outcome::Won {
        base = Vec3::new(0.6, 2.6, 1.3);
    }
    let awake = b.awake();
    let red = Vec3::new(7.0, 0.35, 0.3);
    let rates = [1.3f32, 2.1, 3.4];
    let frame = (t * 30.0) as u32;
    for band in 0..BANDS {
        let r = band_radius(band);
        // the hall breathes slowly with the sleeping boss
        let breath = if awake {
            1.0
        } else {
            0.8 + 0.2 * (t * 1.4 - r * 0.15).sin()
        };
        let mut alarm = 0.0;
        for (t0, s) in &st.list {
            let arrive = t0 + (r - 4.0) / WAVE_SPEED;
            if t >= arrive {
                alarm += s * (-(t - arrive) * 3.2).exp();
            }
        }
        let alarm = alarm.min(1.6);
        let hold = 1.0 - 0.7 * b.telegraph;
        for g in 0..GROUPS {
            let blink = 0.8 + 0.2 * (t * rates[g] + g as f32 * 2.1 + band as f32).sin();
            let mut c = base * blink * breath * hold;
            c = c.lerp(red * 0.5, (b.telegraph * 0.6).min(1.0));
            c = c.lerp(red, (alarm * 0.85).min(1.0)) * (1.0 + alarm * 0.6);
            // a stomp knocks the power out for a moment
            if fight.flicker > 0.0 && hash(frame * 31 + band as u32 * 7 + g as u32) < 0.6 {
                c *= 0.04;
            }
            if fight.suppress > 0.0 {
                c *= 0.55;
            }
            if let Some(mut m) = mats.get_mut(&assets.led_mats[band * GROUPS + g]) {
                m.emissive = LinearRgba::rgb(c.x, c.y, c.z);
            }
        }
    }

    // The dais trim and the vents glow with the boss's heartbeat.
    let beat = if awake {
        0.6 + 0.4 * (t * if b.enraged { 7.0 } else { 4.0 }).sin().abs()
    } else {
        0.35 + 0.25 * (t * 1.4).sin().max(0.0)
    };
    let k = beat * (1.0 + 2.0 * b.fire_flash + 1.5 * b.telegraph);
    if let Some(mut m) = mats.get_mut(&assets.dais_glow) {
        m.emissive = LinearRgba::rgb(3.0 * k, 0.18 * k, 0.28 * k);
    }
    if let Some(mut m) = mats.get_mut(&assets.vent_glow) {
        let v = 0.7 + 0.3 * (t * 3.0).sin();
        m.emissive = LinearRgba::rgb(2.2 * v, 0.12 * v, 0.18 * v);
    }

    // Lights: the chamber's go red and pulse; all of them cut out after a stomp.
    for (mut l, hl) in &mut lights {
        let mut i = hl.base;
        if hl.chamber {
            i *= if awake {
                0.8 + 0.6 * b.fire_flash + 0.3 * b.telegraph
            } else {
                0.45
            };
            l.color = if b.enraged {
                Color::srgb(1.0, 0.18, 0.15)
            } else {
                Color::srgb(1.0, 0.32, 0.38)
            };
        } else {
            let warm = Color::srgb(0.55, 0.75, 1.0).mix(&Color::srgb(1.0, 0.6, 0.3), heat);
            l.color = warm;
        }
        if fight.flicker > 0.0 && hash(frame * 13 + (hl.base as u32 % 97)) < 0.7 {
            i *= 0.05;
        }
        l.intensity = i;
    }

    // Haze: the hotter it gets, the thicker and warmer the air.
    let d = crate::visuals::CAM_DIST;
    let v = rig.zoom;
    let fog_cool = Vec3::new(0.018, 0.026, 0.05);
    let fog_hot = Vec3::new(0.15, 0.075, 0.035);
    let mut fc = fog_cool.lerp(fog_hot, heat.powf(1.3));
    if fight.suppress > 0.0 {
        fc = fc.lerp(
            Vec3::new(0.3, 0.33, 0.37),
            (fight.suppress / 1.5).min(1.0) * 0.8,
        );
    }
    let fog_col = Color::srgb(fc.x, fc.y, fc.z);
    let start = d + 2.0 + 0.22 * v - heat * 4.0;
    let end = d + 10.0 + 0.7 * v - heat * (4.0 + 0.25 * v);
    for (mut fog, mut amb, mut cam) in &mut cams {
        fog.color = fog_col;
        fog.falloff = FogFalloff::Linear {
            start,
            end: end.max(start + 4.0),
        };
        cam.clear_color = ClearColorConfig::Custom(fog_col);
        amb.color = Color::srgb(0.5, 0.6, 1.0).mix(&Color::srgb(1.0, 0.6, 0.4), heat);
        amb.brightness = 70.0 + 50.0 * heat;
    }

    // Dust: a little always, a lot when it's hot, rising in the heat.
    let shown =
        (18.0 + 142.0 * heat.powf(1.2)) as usize + if fight.suppress > 0.0 { 80 } else { 0 };
    if let Some(mut m) = mats.get_mut(&assets.dust_mat) {
        let c = if fight.suppress > 0.0 {
            Vec3::new(0.7, 0.75, 0.8)
        } else {
            Vec3::new(0.35, 0.4, 0.5).lerp(Vec3::new(1.2, 0.6, 0.25), heat)
        };
        m.base_color = Color::linear_rgba(c.x, c.y, c.z, 1.0);
    }
    let focus = rig.focus;
    let speed = 0.6 + 2.2 * heat;
    for (i, (mut du, mut tf, mut vis)) in dust.iter_mut().enumerate() {
        if i >= shown {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        let half = 14.0;
        let mut p = du.pos + du.vel * dt * speed;
        p.x += (t * 0.7 + i as f32).sin() * dt * 0.3;
        // wrap around the camera focus so the air round you is always full
        p.x = focus.x - half + (p.x - focus.x + half).rem_euclid(2.0 * half);
        p.z = focus.z - half + (p.z - focus.z + half).rem_euclid(2.0 * half);
        if p.y > 4.2 {
            p.y = 0.2;
        }
        du.pos = p;
        tf.translation = p;
    }
}
