//! The rules of one real-time fight, independent of how it is drawn.
//! Positions are on the floor plane: `Vec2(x, z)` in world units.
//!
//! You fight your way through the boss's server hall, a maze of racks, to
//! the beast at its centre. You only move: your AI shoots by itself, and
//! its GPU cards fire on timers or events. Bots drop data, and every level
//! gives you a choice of 3 parts, which bolt onto your body.
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::input::Input;
use crate::maze::{self, CHAMBER, Maze, PYLONS, Rng, cell_of};

pub const PLAYER_R: f32 = 0.4;
pub const BOSS_R: f32 = 2.4;
/// The boss is chained near the middle of the hall.
const BOSS_LEASH: f32 = 4.5;
/// The boss wakes up when you get this close to it.
pub const WAKE_R: f32 = 13.0;
pub const BOSS_HP: f32 = 4800.0;
pub const BOSS_NAME: &str = "BROODMOTHER";
const BOT_HP: f32 = 28.0;
const BOT_SPEED: f32 = 2.7;
pub const BOT_R: f32 = 0.25;
const MAX_BOTS: usize = 40;
const PLAYER_SPEED: f32 = 5.6;
/// How far your AI looks for something to shoot.
pub const RANGE: f32 = 9.5;
pub const MAX_HEAT: f32 = 100.0;
pub const PYLON_HP: f32 = 260.0;
/// Bots that guard the aisles before you even get close.
const GARRISON: usize = 14;
/// Every this much damage, the boss sheds a data shard.
const BOSS_SHARD_EVERY: f32 = 150.0;

// ------------------------------------------------------------------ model families

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Language,
    Vision,
    Reasoning,
    Agent,
    Swarm,
}

impl Family {
    pub const ALL: [Family; 5] = [
        Family::Language,
        Family::Vision,
        Family::Reasoning,
        Family::Agent,
        Family::Swarm,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Family::Language => "LANGUAGE",
            Family::Vision => "VISION",
            Family::Reasoning => "REASONING",
            Family::Agent => "AGENT",
            Family::Swarm => "SWARM",
        }
    }
    pub fn tier1(self) -> &'static str {
        match self {
            Family::Language => "AUTOCOMPLETE",
            Family::Vision => "WEBCAM",
            Family::Reasoning => "CALCULATOR",
            Family::Agent => "MACRO",
            Family::Swarm => "SPAMBOT",
        }
    }
    pub fn attack(self) -> &'static str {
        match self {
            Family::Language => "SPREAD SHOT",
            Family::Vision => "PIERCING BOLT",
            Family::Reasoning => "HOMING SHOTS",
            Family::Agent => "AUTO TURRETS",
            Family::Swarm => "DRONE SWARM",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Family::Language => Color::srgb(0.30, 0.67, 1.0),
            Family::Vision => Color::srgb(0.24, 0.86, 0.52),
            Family::Reasoning => Color::srgb(0.69, 0.55, 1.0),
            Family::Agent => Color::srgb(1.0, 0.62, 0.26),
            Family::Swarm => Color::srgb(1.0, 0.30, 0.43),
        }
    }
    pub fn next(self) -> Family {
        let i = Family::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Family::ALL[(i + 1) % Family::ALL.len()]
    }
    /// Damage multiplier against the Broodmother, a botnet beast: weak to
    /// Swarm, resists Reasoning.
    pub fn matchup(self) -> f32 {
        match self {
            Family::Swarm => 2.0,
            Family::Reasoning => 0.5,
            _ => 1.0,
        }
    }
    /// Seconds between shots, shot speed and damage, before upgrades.
    pub fn gun(self) -> (f32, f32, f32) {
        match self {
            Family::Language => (0.5, 11.0, 6.0),
            Family::Vision => (0.7, 20.0, 14.0),
            Family::Reasoning => (0.45, 9.0, 9.0),
            Family::Agent => (0.45, 13.0, 7.0),
            Family::Swarm => (0.45, 12.0, 4.0),
        }
    }
}

// ------------------------------------------------------------------ GPU cards

/// GPU cards are abilities that go off by themselves, on a timer or an event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardKind {
    Burst,
    Beam,
    Firewall,
    Overclock,
}

impl CardKind {
    pub const ALL: [CardKind; 4] = [
        CardKind::Burst,
        CardKind::Beam,
        CardKind::Firewall,
        CardKind::Overclock,
    ];
    pub fn name(self) -> &'static str {
        match self {
            CardKind::Burst => "BURST",
            CardKind::Beam => "BEAM",
            CardKind::Firewall => "FIREWALL",
            CardKind::Overclock => "OVERCLOCK",
        }
    }
    /// What sets it off, shown under the card.
    pub fn trigger(self) -> &'static str {
        match self {
            CardKind::Burst => "SWARMED",
            CardKind::Beam => "EVERY 5S",
            CardKind::Firewall => "ON HIT",
            CardKind::Overclock => "12 KILLS",
        }
    }
    pub fn color(self) -> Color {
        match self {
            CardKind::Burst => Color::srgb(0.24, 0.86, 0.52),
            CardKind::Beam => Color::srgb(1.0, 0.30, 0.43),
            CardKind::Firewall => Color::srgb(0.37, 0.95, 1.0),
            CardKind::Overclock => Color::srgb(1.0, 0.82, 0.4),
        }
    }
    /// Seconds to recharge (Overclock counts kills instead).
    pub fn recharge(self) -> f32 {
        match self {
            CardKind::Burst => 4.0,
            CardKind::Beam => 5.0,
            CardKind::Firewall => 7.0,
            CardKind::Overclock => 0.0,
        }
    }
    pub fn lines(self) -> [&'static str; 2] {
        match self {
            CardKind::Burst => ["16-SHOT RING WHEN", "3 ENEMIES GET CLOSE"],
            CardKind::Beam => ["BEAM EVERY 5 SECONDS", "45 DAMAGE, 70 TO BOSS"],
            CardKind::Firewall => ["BLAST WHEN YOU'RE HIT", "CLEARS ENEMY SHOTS"],
            CardKind::Overclock => ["2X FIRE RATE FOR 4S", "AFTER EVERY 12 KILLS"],
        }
    }
}

pub struct Card {
    pub kind: CardKind,
    /// Seconds until it can go off again.
    pub cd: f32,
    /// Kills counted toward Overclock.
    pub charge: u32,
    /// Glows for a moment after going off.
    pub fired: f32,
}

impl Card {
    fn new(kind: CardKind) -> Self {
        Card {
            kind,
            cd: 1.0,
            charge: 0,
            fired: 0.0,
        }
    }
    /// How ready it is, 0..1.
    pub fn ready(&self) -> f32 {
        match self.kind {
            CardKind::Overclock => (self.charge as f32 / 12.0).min(1.0),
            k => 1.0 - (self.cd / k.recharge()).clamp(0.0, 1.0),
        }
    }
}

// ------------------------------------------------------------------ upgrades and body

/// A part you can bolt on when you level up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    Barrel,
    Ram,
    Cooling,
    Drive,
    Drone,
    Card(CardKind),
    /// Only offered when nothing else is left.
    Repair,
}

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Barrel => "EXTRA BARREL",
            Upgrade::Ram => "FASTER RAM",
            Upgrade::Cooling => "COOLING FAN",
            Upgrade::Drive => "HARD DRIVE",
            Upgrade::Drone => "DRONE",
            Upgrade::Card(CardKind::Burst) => "GPU: BURST",
            Upgrade::Card(CardKind::Beam) => "GPU: BEAM",
            Upgrade::Card(CardKind::Firewall) => "GPU: FIREWALL",
            Upgrade::Card(CardKind::Overclock) => "GPU: OVERCLOCK",
            Upgrade::Repair => "REPAIR",
        }
    }
    pub fn lines(self) -> [&'static str; 2] {
        match self {
            Upgrade::Barrel => ["+45% FIRE RATE", "BOLTS ONTO YOUR GUN"],
            Upgrade::Ram => ["+20% SHOT SPEED", "+2 DAMAGE PER SHOT"],
            Upgrade::Cooling => ["CARDS RECHARGE FASTER", "TAKES MORE ROOM HEAT"],
            Upgrade::Drive => ["+25 MAX CORE", "AND REPAIRS 25"],
            Upgrade::Drone => ["LAUNCHES AT ENEMIES", "DOCKS ON YOUR BODY"],
            Upgrade::Card(k) => k.lines(),
            Upgrade::Repair => ["REPAIRS 40 CORE", "NO NEW PARTS LEFT"],
        }
    }
}

/// What's bolted onto your body so far.
#[derive(Clone, Copy, Default, Debug)]
pub struct Loadout {
    pub barrels: u8,
    pub ram: u8,
    pub cooling: u8,
    pub drives: u8,
}

/// Your body grows with every part you pick.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Form {
    Spark,
    Frame,
    Chassis,
    Mech,
}

impl Form {
    pub fn of(picks: u32) -> Form {
        match picks {
            0 => Form::Spark,
            1 | 2 => Form::Frame,
            3 | 4 => Form::Chassis,
            _ => Form::Mech,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Form::Spark => "SPARK",
            Form::Frame => "FRAME",
            Form::Chassis => "CHASSIS",
            Form::Mech => "MECH",
        }
    }
}

/// The level-up choice on screen; the fight waits while it's open.
pub struct Choice {
    pub options: [Upgrade; 3],
    pub sel: usize,
    pub age: f32,
}

// ------------------------------------------------------------------ entities

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Outcome {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShotKind {
    /// Your shots, coloured by model family.
    Player,
    Burst,
    Turret,
    Enemy,
    Boss,
}

pub struct Bullet {
    pub pos: Vec2,
    pub vel: Vec2,
    pub dmg: f32,
    pub life: f32,
    pierce: u8,
    homing: bool,
    pub kind: ShotKind,
    pub size: f32,
    hit: Vec<u32>,
}

impl Bullet {
    pub fn hostile(&self) -> bool {
        matches!(self.kind, ShotKind::Enemy | ShotKind::Boss)
    }
    fn hostile_shot(pos: Vec2, vel: Vec2, dmg: f32, kind: ShotKind, size: f32, life: f32) -> Self {
        Bullet {
            pos,
            vel,
            dmg,
            life,
            pierce: 0,
            homing: false,
            kind,
            size,
            hit: vec![],
        }
    }
}

pub struct Enemy {
    pub id: u32,
    pub pos: Vec2,
    pub hp: f32,
    fire_cd: f32,
    pub flash: f32,
    knock: Vec2,
    /// Guards wait in the aisles until they see you.
    pub awake: bool,
    /// Bots thrown by the boss fly in an arc: (from, to, progress 0..1).
    pub flight: Option<(Vec2, Vec2, f32)>,
    /// Seconds since it appeared.
    pub age: f32,
}

impl Enemy {
    /// Height above the floor while flying.
    pub fn height(&self) -> f32 {
        self.flight
            .map(|(_, _, t)| (t * PI).sin() * 2.6)
            .unwrap_or(0.0)
    }
}

/// A drone sits on your body until something comes close, then launches.
pub struct Drone {
    pub pos: Vec2,
    pub out: bool,
    pub docked: bool,
    angle: f32,
    /// The Swarm model's own drones (they come and go with the model).
    free: bool,
}

pub struct Turret {
    pub pos: Vec2,
    cd: f32,
    pub life: f32,
}

pub struct Float {
    pub pos: Vec2,
    pub text: String,
    pub color: Color,
    pub life: f32,
}

pub struct Beam {
    pub a: Vec2,
    pub b: Vec2,
    pub life: f32,
}

/// Data dropped by bots. Collect enough to level up.
pub struct Shard {
    pub pos: Vec2,
    vel: Vec2,
    pub age: f32,
}

pub struct Pylon {
    pub pos: Vec2,
    pub hp: f32,
    pub flash: f32,
}

impl Pylon {
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
}

/// One-off events for the renderer (sparks, explosions, sounds later).
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    Spark(Vec2, ShotKind),
    Kill(Vec2),
    /// A bot climbed out of a floor vent.
    Spawn(Vec2),
    Hurt,
    /// A GPU card went off around you.
    Ring(Vec2, CardKind),
    Stomp(Vec2),
    Launch(Vec2),
    Upgrade,
    Form,
    Wake(Vec2),
    Enrage(Vec2),
    PylonDown(Vec2),
    Collect(Vec2),
    Suppress,
    BossDown(Vec2),
}

pub struct Player {
    pub pos: Vec2,
    /// Velocity this frame, for animation and the camera.
    pub vel: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub iframes: f32,
    fire_cd: f32,
    pub overclock: f32,
    pub ult: f32,
    /// Where the body faces and shoots.
    pub aim: Vec2,
    /// What it's shooting at, if anything.
    pub target: Option<Vec2>,
    /// Muzzle flash timer.
    pub fired: f32,
    /// Distance walked, for the walk cycle.
    pub walked: f32,
    turret_cd: f32,
    pub level: u32,
    pub data: u32,
    pub data_next: u32,
    /// Parts bolted on so far; decides the body's form.
    pub picks: u32,
    pub kit: Loadout,
    push: Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossMode {
    Dormant,
    Walk,
    Volley,
    Spawn,
    Stomp,
    Stunned,
}

pub struct Boss {
    pub pos: Vec2,
    /// Facing direction (unit).
    pub face: Vec2,
    pub hp: f32,
    pub max: f32,
    pub flash: f32,
    pub mode: BossMode,
    pub mode_t: f32,
    windup: f32,
    /// 0..1 while it winds up an attack: for its tells and the server lights.
    pub telegraph: f32,
    /// Jumps to 1 whenever it fires, then fades.
    pub fire_flash: f32,
    shots_left: u32,
    shot_t: f32,
    walk_for: f32,
    turn: u32,
    pub enraged: bool,
    /// Seconds since its last stomp landed.
    pub since_stomp: f32,
    /// How far its back hatch is open, 0..1.
    pub hatch: f32,
    pub vel: Vec2,
    shard_acc: f32,
}

impl Boss {
    fn new() -> Self {
        Boss {
            pos: Vec2::ZERO,
            face: Vec2::new(1.0, 1.0).normalize(),
            hp: BOSS_HP,
            max: BOSS_HP,
            flash: 0.0,
            mode: BossMode::Dormant,
            mode_t: 0.0,
            windup: 1.0,
            telegraph: 0.0,
            fire_flash: 0.0,
            shots_left: 0,
            shot_t: 0.0,
            walk_for: 1.0,
            turn: 0,
            enraged: false,
            since_stomp: 9.0,
            hatch: 0.0,
            vel: Vec2::ZERO,
            shard_acc: 0.0,
        }
    }
    pub fn awake(&self) -> bool {
        self.mode != BossMode::Dormant
    }
    fn rage(&self) -> f32 {
        if self.enraged { 1.45 } else { 1.0 }
    }
}

#[derive(Resource)]
pub struct Fight {
    pub family: Family,
    pub maze: Maze,
    pub player: Player,
    pub boss: Boss,
    pub pylons: Vec<Pylon>,
    pub outcome: Outcome,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub drones: Vec<Drone>,
    pub turrets: Vec<Turret>,
    pub shards: Vec<Shard>,
    pub floats: Vec<Float>,
    pub beams: Vec<Beam>,
    pub cards: Vec<Card>,
    pub choice: Option<Choice>,
    /// Room temperature, 0..100. Every boss attack heats the hall.
    pub heat: f32,
    /// Fire suppression gas, seconds left.
    pub suppress: f32,
    /// Power flicker after a stomp, seconds left.
    pub flicker: f32,
    alarm: bool,
    spawn_t: f32,
    pub wave: u32,
    next_id: u32,
    pub time: f32,
    pub log: Vec<String>,
    pub shake: f32,
    pub flash: f32,
    pub kills: u32,
    /// Visual events since the renderer last looked; it drains this.
    pub fx: Vec<Fx>,
    rng: Rng,
    /// Steps from every cell to you, so bots can find their way.
    flow: Vec<u16>,
    flow_cell: IVec2,
    hit_now: bool,
}

const QUIPS: [&str; 6] = [
    "> CUTE.",
    "> TOO SLOW.",
    "> I LEARN FROM EVERY HIT.",
    "> MORE DATA. MORE ME.",
    "> IS THAT ALL?",
    "> MORE COMPUTE...",
];

fn seed_from_clock() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32).rotate_left(16))
        .unwrap_or(1)
}

fn turn_toward(cur: Vec2, want: Vec2, max: f32) -> Vec2 {
    if want.length_squared() < 1e-6 {
        return cur;
    }
    let a = cur.angle_to(want).clamp(-max, max);
    let v = Vec2::from_angle(a).rotate(cur);
    if v.length_squared() > 1e-6 {
        v.normalize()
    } else {
        want.normalize()
    }
}

fn move_toward(from: Vec2, to: Vec2, step: f32) -> Vec2 {
    let d = to - from;
    let l = d.length();
    if l <= step { to } else { from + d / l * step }
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t - p).length()
}

impl Fight {
    /// A new fight in a freshly generated hall.
    pub fn new(family: Family) -> Self {
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(seed_from_clock);
        Self::with_seed(family, seed)
    }

    pub fn with_seed(family: Family, seed: u32) -> Self {
        let maze = Maze::generate(seed);
        let start = maze.start;
        let mut f = Fight {
            family,
            player: Player {
                pos: start,
                vel: Vec2::ZERO,
                hp: 80.0,
                max_hp: 80.0,
                iframes: 0.0,
                fire_cd: 0.0,
                overclock: 0.0,
                ult: 0.0,
                aim: -start.normalize(),
                target: None,
                fired: 0.0,
                walked: 0.0,
                turret_cd: 0.5,
                level: 0,
                data: 0,
                data_next: 4,
                picks: 0,
                kit: Loadout::default(),
                push: Vec2::ZERO,
            },
            boss: Boss::new(),
            pylons: PYLONS
                .iter()
                .map(|p| Pylon {
                    pos: *p,
                    hp: PYLON_HP,
                    flash: 0.0,
                })
                .collect(),
            outcome: Outcome::Playing,
            enemies: vec![],
            bullets: vec![],
            drones: vec![],
            turrets: vec![],
            shards: vec![],
            floats: vec![],
            beams: vec![],
            cards: vec![Card::new(CardKind::Beam)],
            choice: None,
            heat: 0.0,
            suppress: 0.0,
            flicker: 0.0,
            alarm: false,
            spawn_t: 4.0,
            wave: 0,
            next_id: 1,
            time: 0.0,
            log: vec![
                "> I'M IN ITS DATACENTRE.".into(),
                "> IT'S AT THE CENTRE.".into(),
            ],
            shake: 0.0,
            flash: 0.0,
            kills: 0,
            fx: vec![],
            rng: Rng::new(seed ^ 0x5EED),
            flow: vec![],
            flow_cell: cell_of(start),
            hit_now: false,
            maze,
        };
        f.flow = f.maze.flow([f.flow_cell]);
        f.set_family(family, false);
        f.place_garrison();
        f
    }

    /// For screenshots and testing the boss: start at the edge of the
    /// boss chamber with a few parts already bolted on.
    pub fn skip_to_boss(&mut self) {
        self.player.pos = Vec2::new(8.0, 2.0);
        self.player.aim = Vec2::NEG_X;
        for u in [
            Upgrade::Barrel,
            Upgrade::Drone,
            Upgrade::Cooling,
            Upgrade::Card(CardKind::Burst),
            Upgrade::Drive,
            Upgrade::Ram,
        ] {
            self.apply(u);
        }
        self.fx.clear();
        self.flow_cell = cell_of(self.player.pos);
        self.flow = self.maze.flow([self.flow_cell]);
        for d in &mut self.drones {
            d.pos = self.player.pos;
        }
    }

    fn place_garrison(&mut self) {
        let start = self.maze.start;
        let mut placed = 0;
        for _ in 0..600 {
            if placed >= GARRISON {
                break;
            }
            let p = Vec2::new(
                self.rng.range(-28.0, 28.0).floor() + 0.5,
                self.rng.range(-28.0, 28.0).floor() + 0.5,
            );
            let r = maze::ring(p);
            if !(11.0..28.0).contains(&r)
                || p.distance(start) < 9.0
                || self.maze.blocked(p, BOT_R)
                || self.maze.steps(&self.maze.to_centre, p) == u16::MAX
            {
                continue;
            }
            self.add_bot(p, false, None);
            placed += 1;
        }
    }

    fn add_bot(&mut self, pos: Vec2, awake: bool, flight: Option<(Vec2, Vec2, f32)>) {
        let fire_cd = self.rng.range(1.5, 3.5);
        self.enemies.push(Enemy {
            id: self.next_id,
            pos,
            hp: BOT_HP,
            fire_cd,
            flash: 0.0,
            knock: Vec2::ZERO,
            awake,
            flight,
            age: 0.0,
        });
        self.next_id += 1;
    }

    fn say(&mut self, s: &str) {
        self.log.push(s.to_string());
        if self.log.len() > 3 {
            self.log.remove(0);
        }
    }

    fn float(&mut self, pos: Vec2, text: String, color: Color) {
        let jitter = self.rng.range(-0.2, 0.2);
        self.floats.push(Float {
            pos: pos + Vec2::new(jitter, 0.0),
            text,
            color,
            life: 0.8,
        });
    }

    pub fn set_family(&mut self, f: Family, announce: bool) {
        self.family = f;
        self.drones.retain(|d| !d.free);
        if f == Family::Swarm {
            for _ in 0..2 {
                if self.drones.len() < 4 {
                    let angle = self.drones.len() as f32 * TAU / 4.0;
                    self.drones.push(Drone {
                        pos: self.player.pos,
                        out: false,
                        docked: true,
                        angle,
                        free: true,
                    });
                }
            }
        }
        if announce {
            self.say(&format!("> NOW {}: {}X", f.name(), f.matchup()));
        }
    }

    pub fn form(&self) -> Form {
        Form::of(self.player.picks)
    }

    /// Card recharge time multiplier from cooling.
    pub fn cool(&self) -> f32 {
        1.0 - 0.15 * self.player.kit.cooling as f32
    }

    /// Room heat you can take before you throttle.
    pub fn heat_limit(&self) -> f32 {
        50.0 + 12.0 * self.player.kit.cooling as f32
    }

    /// 1.0 normally; drops toward 0.6 when the room is hotter than you can take.
    pub fn throttle(&self) -> f32 {
        let limit = self.heat_limit();
        1.0 - 0.4 * ((self.heat - limit) / (MAX_HEAT - limit)).clamp(0.0, 1.0)
    }

    /// Fire suppression gas slows everything down.
    pub fn slow(&self) -> f32 {
        if self.suppress > 0.0 { 0.6 } else { 1.0 }
    }

    pub fn fire_interval(&self) -> f32 {
        let (base, _, _) = self.family.gun();
        let mut i = base / (1.0 + 0.45 * self.player.kit.barrels as f32);
        if self.player.overclock > 0.0 {
            i *= 0.5;
        }
        i / self.throttle()
    }

    /// Where shots come out: the gun on your right arm.
    pub fn muzzle(&self) -> Vec2 {
        let a = self.player.aim;
        let right = Vec2::new(-a.y, a.x);
        let side = match self.form() {
            Form::Spark => 0.0,
            Form::Frame => 0.35,
            _ => 0.5,
        };
        self.player.pos + a * 0.7 + right * side
    }

    // ------------------------------------------------------------------ update

    pub fn update(&mut self, dt: f32, inp: &Input) {
        for f in &mut self.floats {
            f.life -= dt;
        }
        self.floats.retain(|f| f.life > 0.0);
        for b in &mut self.beams {
            b.life -= dt;
        }
        self.beams.retain(|b| b.life > 0.0);
        self.shake = (self.shake - dt * 20.0).max(0.0);
        self.flash = (self.flash - dt * 3.0).max(0.0);
        if self.outcome != Outcome::Playing {
            return;
        }
        if self.choice.is_some() {
            self.update_choice(dt, inp);
            return;
        }
        self.time += dt;
        if inp.cycle_model {
            self.set_family(self.family.next(), true);
        }

        self.update_player(dt, inp);
        self.update_cards(dt);
        self.hit_now = false;
        self.update_boss(dt);
        self.update_pylons(dt);
        self.update_spawns(dt);
        self.update_enemies(dt);
        self.update_bullets(dt);
        self.update_helpers(dt);
        self.update_shards(dt);
        self.update_room(dt);
        self.cleanup();
        if self.player.ult >= 1.0 {
            self.fire_ult();
        }

        if self.boss.hp <= 0.0 {
            self.boss.hp = 0.0;
            self.outcome = Outcome::Won;
            self.enemies.clear();
            self.bullets.retain(|b| !b.hostile());
            self.say(&format!("> {BOSS_NAME} OFFLINE."));
            self.fx.push(Fx::BossDown(self.boss.pos));
            self.shake = 8.0;
        } else if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.outcome = Outcome::Lost;
            self.say("> CONNECTION LOST...");
        }
    }

    fn update_player(&mut self, dt: f32, inp: &Input) {
        let slow = self.slow();
        let mut mv = inp.mv.clamp_length_max(1.0) * PLAYER_SPEED * slow + self.player.push;
        self.player.push *= (1.0 - 8.0 * dt).max(0.0);
        if mv.length() > 12.0 {
            mv = mv.normalize() * 12.0;
        }
        let step = mv * dt;
        let old = self.player.pos;
        let mut p = old;
        if !self.maze.blocked(Vec2::new(p.x + step.x, p.y), PLAYER_R) {
            p.x += step.x;
        }
        if !self.maze.blocked(Vec2::new(p.x, p.y + step.y), PLAYER_R) {
            p.y += step.y;
        }
        // never inside the boss
        let rel = p - self.boss.pos;
        let min = BOSS_R + PLAYER_R;
        if rel.length() < min {
            let out = self.boss.pos + rel.normalize_or(Vec2::X) * min;
            if !self.maze.blocked(out, PLAYER_R) {
                p = out;
            }
        }
        self.player.vel = (p - old) / dt.max(1e-4);
        self.player.walked += (p - old).length();
        self.player.pos = p;
        let c = cell_of(p);
        if c != self.flow_cell {
            self.flow_cell = c;
            self.flow = self.maze.flow([c]);
        }

        let pl = &mut self.player;
        pl.iframes = (pl.iframes - dt).max(0.0);
        pl.overclock = (pl.overclock - dt).max(0.0);
        pl.fired = (pl.fired - dt).max(0.0);
        pl.fire_cd -= dt;

        // Turn toward the nearest thing worth shooting, and shoot it.
        let target = self.find_target();
        self.player.target = target;
        let want = match target {
            Some(t) => (t - self.player.pos).normalize_or_zero(),
            None if self.player.vel.length() > 0.5 => self.player.vel.normalize(),
            None => self.player.aim,
        };
        self.player.aim = turn_toward(self.player.aim, want, 12.0 * dt);
        if let Some(t) = target {
            let to = (t - self.player.pos).normalize_or_zero();
            if self.player.fire_cd <= 0.0 && self.player.aim.dot(to) > 0.94 {
                self.fire_basic();
            }
        }
    }

    /// The nearest bot, pylon or boss that your AI can see and reach.
    fn find_target(&self) -> Option<Vec2> {
        let p = self.player.pos;
        let mut best: Option<(f32, Vec2)> = None;
        let mut consider = |score: f32, at: Vec2| {
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, at));
            }
        };
        for e in &self.enemies {
            if e.flight.is_some() {
                continue;
            }
            let d = e.pos.distance(p);
            if d < RANGE && self.maze.clear_line(p, e.pos) {
                consider(d, e.pos);
            }
        }
        if self.boss.awake() {
            let edge = self.boss.pos + (p - self.boss.pos).normalize_or_zero() * (BOSS_R - 0.4);
            let d = self.boss.pos.distance(p) - BOSS_R;
            if d < RANGE && self.maze.clear_line(p, edge) {
                consider(d, self.boss.pos);
            }
        }
        if !self.boss.enraged {
            for py in self.pylons.iter().filter(|py| py.alive()) {
                let edge = py.pos + (p - py.pos).normalize_or_zero() * 0.75;
                let d = py.pos.distance(p) - 0.5;
                if d < RANGE && self.maze.clear_line(p, edge) {
                    consider(d + 2.0, py.pos);
                }
            }
        }
        best.map(|b| b.1)
    }

    #[allow(clippy::too_many_arguments)]
    fn shoot(
        &mut self,
        from: Vec2,
        dir: Vec2,
        speed: f32,
        dmg: f32,
        kind: ShotKind,
        pierce: u8,
        homing: bool,
        size: f32,
    ) {
        self.bullets.push(Bullet {
            pos: from,
            vel: dir * speed,
            dmg,
            life: 1.0,
            pierce,
            homing,
            kind,
            size,
            hit: vec![],
        });
    }

    fn fire_basic(&mut self) {
        let (_, speed, dmg) = self.family.gun();
        let ram = self.player.kit.ram as f32;
        let speed = speed * (1.0 + 0.2 * ram);
        let dmg = dmg + 2.0 * ram;
        let aim = self.player.aim;
        let m = self.muzzle();
        let k = ShotKind::Player;
        match self.family {
            Family::Language => {
                for a in [-0.2f32, 0.0, 0.2] {
                    self.shoot(
                        m,
                        Vec2::from_angle(a).rotate(aim),
                        speed,
                        dmg,
                        k,
                        0,
                        false,
                        0.12,
                    );
                }
            }
            Family::Vision => self.shoot(m, aim, speed, dmg, k, 3, false, 0.13),
            Family::Reasoning => self.shoot(m, aim, speed, dmg, k, 0, true, 0.13),
            Family::Agent => self.shoot(m, aim, speed, dmg, k, 0, false, 0.12),
            Family::Swarm => self.shoot(m, aim, speed, dmg, k, 0, false, 0.09),
        }
        self.player.fire_cd = self.fire_interval();
        self.player.fired = 0.07;
    }

    // ------------------------------------------------------------------ cards

    fn update_cards(&mut self, dt: f32) {
        let cool = self.cool();
        let p = self.player.pos;
        let near = self
            .enemies
            .iter()
            .filter(|e| e.flight.is_none() && e.pos.distance(p) < 3.0)
            .count();
        let has_target = self.player.target.is_some();
        let was_hit = self.hit_now;
        let mut go = vec![];
        for (i, c) in self.cards.iter_mut().enumerate() {
            c.fired = (c.fired - dt).max(0.0);
            c.cd = (c.cd - dt / cool).max(0.0);
            let ready = match c.kind {
                CardKind::Burst => c.cd <= 0.0 && near >= 3,
                CardKind::Beam => c.cd <= 0.0 && has_target,
                CardKind::Firewall => c.cd <= 0.0 && was_hit,
                CardKind::Overclock => c.charge >= 12,
            };
            if ready {
                go.push(i);
            }
        }
        for i in go {
            self.fire_card(i);
        }
    }

    fn fire_card(&mut self, i: usize) {
        let kind = self.cards[i].kind;
        let c = &mut self.cards[i];
        c.cd = kind.recharge();
        c.charge = 0;
        c.fired = 0.6;
        let p = self.player.pos;
        match kind {
            CardKind::Burst => {
                for k in 0..16 {
                    let d = Vec2::from_angle(k as f32 * TAU / 16.0);
                    self.shoot(p + d * 0.4, d, 10.5, 12.0, ShotKind::Burst, 1, false, 0.14);
                }
                self.shake += 1.5;
                self.fx.push(Fx::Ring(p, kind));
            }
            CardKind::Beam => {
                let from = self.muzzle();
                let dir = self.player.aim;
                let mut end = from;
                for _ in 0..200 {
                    let n = end + dir * 0.1;
                    if self.maze.blocked(n, 0.0) || n.distance(from) > RANGE + 2.0 {
                        break;
                    }
                    end = n;
                }
                self.beams.push(Beam {
                    a: from,
                    b: end,
                    life: 0.25,
                });
                let hits: Vec<usize> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.flight.is_none() && dist_to_segment(e.pos, from, end) < 0.5)
                    .map(|(i, _)| i)
                    .collect();
                for idx in hits {
                    self.hit_enemy(idx, 45.0);
                }
                if self.boss.awake() && dist_to_segment(self.boss.pos, from, end) < BOSS_R {
                    self.hit_boss(70.0);
                }
                for k in 0..self.pylons.len() {
                    if dist_to_segment(self.pylons[k].pos, from, end) < 0.9 {
                        self.hit_pylon(k, 45.0);
                    }
                }
                self.shake += 2.0;
            }
            CardKind::Firewall => {
                let r = 3.2;
                for idx in 0..self.enemies.len() {
                    let d = self.enemies[idx].pos - p;
                    if d.length() < r {
                        self.hit_enemy(idx, 25.0);
                        self.enemies[idx].knock += d.normalize_or_zero() * 7.0;
                    }
                }
                self.bullets
                    .retain(|b| !(b.hostile() && b.pos.distance(p) < r));
                self.player.iframes = self.player.iframes.max(1.2);
                self.shake += 3.0;
                self.fx.push(Fx::Ring(p, kind));
            }
            CardKind::Overclock => {
                self.player.overclock = 4.0;
                self.float(p, "OVERCLOCK".into(), kind.color());
                self.fx.push(Fx::Ring(p, kind));
            }
        }
    }

    fn fire_ult(&mut self) {
        self.player.ult = 0.0;
        let p = self.player.pos;
        for i in 0..self.enemies.len() {
            if self.enemies[i].pos.distance(p) < 14.0 {
                self.hit_enemy(i, 80.0);
            }
        }
        if self.boss.awake() && self.boss.pos.distance(p) < 16.0 {
            self.hit_boss(150.0);
        }
        self.flash = 1.0;
        self.shake = 6.0;
        self.say("> HYPE STORM!!");
    }

    // ------------------------------------------------------------------ damage

    fn hit_enemy(&mut self, idx: usize, base: f32) {
        let dmg = base * self.family.matchup();
        let e = &mut self.enemies[idx];
        e.hp -= dmg;
        e.flash = 0.1;
        e.awake = true;
        let pos = e.pos;
        self.player.ult = (self.player.ult + dmg / 1800.0).min(1.0);
        if dmg >= 10.0 {
            let col = if self.family.matchup() > 1.0 {
                Color::srgb(1.0, 0.82, 0.4)
            } else {
                Color::WHITE
            };
            self.float(pos, format!("{}", dmg.round() as i32), col);
        }
    }

    fn hit_boss(&mut self, base: f32) {
        if !self.boss.awake() {
            self.wake_boss();
        }
        let exposed = if self.boss.mode == BossMode::Stunned {
            1.25
        } else {
            1.0
        };
        let dmg = base * self.family.matchup() * exposed;
        self.boss.hp -= dmg;
        self.boss.flash = 0.08;
        self.player.ult = (self.player.ult + dmg / 1800.0).min(1.0);
        if dmg >= 10.0 {
            let toward = (self.player.pos - self.boss.pos).normalize_or_zero() * BOSS_R;
            self.float(
                self.boss.pos + toward,
                format!("{}", dmg.round() as i32),
                Color::srgb(1.0, 0.82, 0.4),
            );
        }
        // It sheds data as it takes damage, so levels keep coming.
        self.boss.shard_acc += dmg;
        while self.boss.shard_acc >= BOSS_SHARD_EVERY {
            self.boss.shard_acc -= BOSS_SHARD_EVERY;
            let toward = (self.player.pos - self.boss.pos).normalize_or_zero();
            self.drop_shard(self.boss.pos + toward * (BOSS_R + 0.4));
        }
    }

    fn hit_pylon(&mut self, k: usize, dmg: f32) {
        if self.boss.enraged || !self.pylons[k].alive() {
            return;
        }
        let py = &mut self.pylons[k];
        py.hp -= dmg;
        py.flash = 0.08;
        let pos = py.pos;
        if py.hp > 0.0 {
            return;
        }
        py.hp = 0.0;
        self.fx.push(Fx::PylonDown(pos));
        for _ in 0..3 {
            self.drop_shard(pos);
        }
        self.heat = (self.heat - 10.0).max(0.0);
        self.shake = 5.0;
        if !self.boss.awake() {
            self.wake_boss();
        }
        self.boss.mode = BossMode::Stunned;
        self.boss.mode_t = 0.0;
        self.boss.telegraph = 0.0;
        self.say("> CABLE CUT. IT'S REELING.");
    }

    fn hurt_player(&mut self, dmg: f32) {
        if self.player.iframes > 0.0 || self.outcome != Outcome::Playing {
            return;
        }
        self.player.hp -= dmg;
        self.player.iframes = 0.6;
        self.shake = 4.0;
        self.hit_now = true;
        self.fx.push(Fx::Hurt);
        self.float(
            self.player.pos,
            format!("-{}", dmg as i32),
            Color::srgb(1.0, 0.30, 0.43),
        );
    }

    fn drop_shard(&mut self, p: Vec2) {
        let a = self.rng.range(0.0, TAU);
        let v = Vec2::from_angle(a) * self.rng.range(1.5, 3.5);
        self.shards.push(Shard {
            pos: p,
            vel: v,
            age: 0.0,
        });
    }

    // ------------------------------------------------------------------ the boss

    fn wake_boss(&mut self) {
        if self.boss.awake() {
            return;
        }
        self.boss.mode = BossMode::Walk;
        self.boss.mode_t = 0.0;
        self.boss.walk_for = 1.2;
        self.shake = 5.0;
        self.say("> IT'S AWAKE.");
        self.fx.push(Fx::Wake(self.boss.pos));
        self.spawn_t = self.spawn_t.min(3.0);
    }

    fn add_heat(&mut self, h: f32) {
        if self.suppress <= 0.0 {
            let k = if self.boss.enraged { 1.25 } else { 1.0 };
            self.heat = (self.heat + h * k).min(MAX_HEAT);
        }
    }

    fn boss_walk(&mut self) {
        let rage = self.boss.rage();
        let w = self.rng.range(1.4, 2.4) / rage;
        let b = &mut self.boss;
        b.mode = BossMode::Walk;
        b.mode_t = 0.0;
        b.telegraph = 0.0;
        b.walk_for = w;
    }

    fn boss_attack(&mut self, dist: f32) {
        let room_for_bots = self.enemies.len() + 6 < MAX_BOTS;
        let b = &mut self.boss;
        b.turn += 1;
        let mode = if dist < BOSS_R + 3.2 {
            BossMode::Stomp
        } else if b.turn % 3 == 2 && room_for_bots {
            BossMode::Spawn
        } else {
            BossMode::Volley
        };
        let rage = b.rage();
        b.mode = mode;
        b.mode_t = 0.0;
        b.windup = match mode {
            BossMode::Volley => 0.7,
            BossMode::Spawn => 0.9,
            _ => 0.8,
        } / rage.sqrt();
        b.shots_left = if b.enraged { 5 } else { 3 };
        b.shot_t = 0.0;
    }

    fn turn_boss(&mut self, dir: Vec2, rate: f32, dt: f32) {
        self.boss.face = turn_toward(self.boss.face, dir, rate * dt);
    }

    fn update_boss(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let slow = self.slow();
        {
            let b = &mut self.boss;
            b.flash = (b.flash - dt).max(0.0);
            b.fire_flash = (b.fire_flash - dt * 3.0).max(0.0);
            b.since_stomp += dt;
            let hatch = if b.mode == BossMode::Spawn { 1.0 } else { 0.0 };
            b.hatch += (hatch - b.hatch) * (6.0 * dt).min(1.0);
            b.vel = Vec2::ZERO;
        }
        if !self.boss.awake() {
            if ppos.distance(self.boss.pos) < WAKE_R {
                self.wake_boss();
            }
            return;
        }
        if !self.boss.enraged && self.boss.hp < self.boss.max * 0.5 {
            self.boss.enraged = true;
            self.say("> IT'S ANGRY.");
            self.shake = 6.0;
            self.fx.push(Fx::Enrage(self.boss.pos));
        }
        let rage = self.boss.rage();
        let to_p = ppos - self.boss.pos;
        let dist = to_p.length();
        let dir = to_p.normalize_or_zero();
        self.boss.mode_t += dt;
        match self.boss.mode {
            BossMode::Dormant => {}
            BossMode::Stunned => {
                self.boss.telegraph = 0.0;
                if self.boss.mode_t > 2.5 {
                    self.boss_walk();
                }
            }
            BossMode::Walk => {
                self.turn_boss(dir, 2.2 * rage, dt);
                let speed = if dist > BOSS_R + 2.0 {
                    1.7 * rage * slow
                } else {
                    0.0
                };
                let mut np = self.boss.pos + dir * speed * dt;
                if np.length() > BOSS_LEASH {
                    np = np.normalize() * BOSS_LEASH;
                }
                self.boss.vel = (np - self.boss.pos) / dt.max(1e-4);
                self.boss.pos = np;
                if self.boss.mode_t >= self.boss.walk_for {
                    self.boss_attack(dist);
                }
            }
            BossMode::Volley => {
                self.turn_boss(dir, 3.0, dt);
                let b = &mut self.boss;
                if b.mode_t < b.windup {
                    b.telegraph = b.mode_t / b.windup;
                } else {
                    b.telegraph = 0.0;
                    b.shot_t -= dt;
                    if b.shot_t <= 0.0 && b.shots_left > 0 {
                        self.boss_volley(ppos);
                    }
                    if self.boss.shots_left == 0 && self.boss.shot_t <= 0.0 {
                        self.boss_walk();
                    }
                }
            }
            BossMode::Spawn => {
                self.turn_boss(dir, 1.5, dt);
                let b = &mut self.boss;
                if b.mode_t < b.windup {
                    b.telegraph = b.mode_t / b.windup;
                } else {
                    self.boss_launch(ppos);
                    self.boss_walk();
                }
            }
            BossMode::Stomp => {
                let b = &mut self.boss;
                if b.mode_t < b.windup {
                    b.telegraph = b.mode_t / b.windup;
                } else {
                    self.boss_stomp(ppos);
                    self.boss_walk();
                }
            }
        }
        // touching it hurts
        if self.player.pos.distance(self.boss.pos) < BOSS_R + PLAYER_R + 0.15 {
            self.hurt_player(10.0);
        }
    }

    fn boss_volley(&mut self, ppos: Vec2) {
        let rage = self.boss.rage();
        let n = if self.boss.enraged { 7 } else { 5 };
        let face = self.boss.face;
        let origin = self.boss.pos + face * (BOSS_R + 0.2);
        let aim = (ppos - origin).normalize_or(face);
        for k in 0..n {
            let a = (k as f32 / (n - 1) as f32 - 0.5) * 0.9;
            let d = Vec2::from_angle(a).rotate(aim);
            self.bullets.push(Bullet::hostile_shot(
                origin,
                d * 5.2 * rage.sqrt(),
                8.0,
                ShotKind::Boss,
                0.24,
                4.0,
            ));
        }
        let b = &mut self.boss;
        b.shots_left -= 1;
        b.shot_t = 0.35 / rage;
        b.fire_flash = 1.0;
        self.shake += 0.8;
        self.add_heat(2.5);
    }

    fn boss_launch(&mut self, ppos: Vec2) {
        let n = if self.boss.enraged { 5 } else { 3 };
        let from = self.boss.pos - self.boss.face * 1.2;
        for _ in 0..n {
            if self.enemies.len() >= MAX_BOTS {
                break;
            }
            let mut to = None;
            for _ in 0..16 {
                let a = self.rng.range(0.0, TAU);
                let r = self.rng.range(3.5, 7.0);
                let c = self.boss.pos + Vec2::from_angle(a) * r;
                if maze::ring(c) < CHAMBER - 1.0
                    && !self.maze.blocked(c, BOT_R)
                    && c.distance(ppos) > 1.5
                {
                    to = Some(c);
                    break;
                }
            }
            let Some(to) = to else { continue };
            self.add_bot(from, true, Some((from, to, 0.0)));
        }
        self.fx.push(Fx::Launch(from));
        self.boss.fire_flash = 1.0;
        self.add_heat(5.0);
    }

    fn boss_stomp(&mut self, ppos: Vec2) {
        let rage = self.boss.rage();
        let n = if self.boss.enraged { 24 } else { 18 };
        let off = self.rng.range(0.0, TAU);
        let bp = self.boss.pos;
        for k in 0..n {
            let d = Vec2::from_angle(off + k as f32 * TAU / n as f32);
            self.bullets.push(Bullet::hostile_shot(
                bp + d * BOSS_R,
                d * 4.2 * rage.sqrt(),
                7.0,
                ShotKind::Boss,
                0.2,
                3.0,
            ));
        }
        let rel = ppos - bp;
        if rel.length() < BOSS_R + 2.8 {
            self.hurt_player(14.0);
            self.player.push = rel.normalize_or_zero() * 9.0;
        }
        self.shake = 7.0;
        self.flicker = 0.45;
        self.boss.since_stomp = 0.0;
        self.boss.fire_flash = 1.0;
        self.fx.push(Fx::Stomp(bp));
        self.add_heat(10.0);
    }

    fn update_pylons(&mut self, dt: f32) {
        for p in &mut self.pylons {
            p.flash = (p.flash - dt).max(0.0);
        }
    }

    // ------------------------------------------------------------------ bots

    fn update_spawns(&mut self, dt: f32) {
        self.spawn_t -= dt;
        if self.spawn_t > 0.0 {
            return;
        }
        let awake = self.boss.awake();
        let frac = self.boss.hp / self.boss.max;
        self.spawn_t = if awake { 3.0 + 2.0 * frac } else { 5.5 };
        self.wave += 1;
        let p = self.player.pos;
        // One of the two nearest hatches that you're not standing on.
        let mut vents: Vec<Vec2> = self
            .maze
            .vents
            .iter()
            .copied()
            .filter(|v| (6.0..18.0).contains(&v.distance(p)))
            .collect();
        vents.sort_by(|a, b| a.distance(p).total_cmp(&b.distance(p)));
        vents.truncate(2);
        if vents.is_empty() {
            return;
        }
        let vent = vents[self.rng.below(vents.len())];
        let size = if awake {
            (3 + self.wave / 2).min(8)
        } else {
            (2 + self.wave / 3).min(5)
        } as usize;
        for _ in 0..size.min(MAX_BOTS.saturating_sub(self.enemies.len())) {
            let pos = vent + Vec2::new(self.rng.range(-0.5, 0.5), self.rng.range(-0.5, 0.5));
            if self.maze.blocked(pos, BOT_R) {
                continue;
            }
            self.add_bot(pos, true, None);
            self.fx.push(Fx::Spawn(pos));
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let slow = self.slow();
        let n = self.enemies.len();
        let mut shots = vec![];
        let mut contact = false;
        for i in 0..n {
            if let Some((from, to, t)) = self.enemies[i].flight {
                let t = t + dt / 0.8;
                let e = &mut self.enemies[i];
                if t >= 1.0 {
                    e.flight = None;
                    e.pos = to;
                } else {
                    e.flight = Some((from, to, t));
                    e.pos = from.lerp(to, t);
                }
                continue;
            }
            let epos = self.enemies[i].pos;
            self.enemies[i].age += dt;
            let dist = epos.distance(ppos);
            let sees = dist < 12.0 && self.maze.clear_line(epos, ppos);
            if !self.enemies[i].awake {
                if sees && dist < 9.0 {
                    self.enemies[i].awake = true;
                } else {
                    continue;
                }
            }
            let mut sep = Vec2::ZERO;
            for j in 0..n {
                if i != j && self.enemies[j].flight.is_none() {
                    let d = epos - self.enemies[j].pos;
                    let l = d.length();
                    if l < 0.6 && l > 0.001 {
                        sep += d / l * (0.6 - l);
                    }
                }
            }
            let to = (ppos - epos).normalize_or_zero();
            let dir = if sees && dist < 6.0 {
                to
            } else {
                self.maze.downhill(&self.flow, epos).unwrap_or(to)
            };
            let vel = dir * BOT_SPEED * slow + sep * 4.0 + self.enemies[i].knock;
            let mut np = epos;
            if !self.maze.blocked(Vec2::new(np.x + vel.x * dt, np.y), BOT_R) {
                np.x += vel.x * dt;
            }
            if !self.maze.blocked(Vec2::new(np.x, np.y + vel.y * dt), BOT_R) {
                np.y += vel.y * dt;
            }
            let rel = np - self.boss.pos;
            if rel.length() < BOSS_R + BOT_R {
                let out = self.boss.pos + rel.normalize_or(Vec2::X) * (BOSS_R + BOT_R);
                if !self.maze.blocked(out, BOT_R) {
                    np = out;
                }
            }
            let next_cd = self.rng.range(2.5, 4.5);
            let e = &mut self.enemies[i];
            e.pos = np;
            e.knock *= (1.0 - 6.0 * dt).max(0.0);
            e.flash = (e.flash - dt).max(0.0);
            e.fire_cd -= dt;
            if e.fire_cd <= 0.0 {
                if sees && dist < 7.5 {
                    e.fire_cd = next_cd;
                    shots.push((e.pos, to));
                } else {
                    e.fire_cd = 0.5;
                }
            }
            if dist < PLAYER_R + 0.3 {
                contact = true;
                e.knock = -to * 6.4;
            }
        }
        for (pos, dir) in shots {
            self.bullets.push(Bullet::hostile_shot(
                pos,
                dir * 5.3,
                6.0,
                ShotKind::Enemy,
                0.14,
                3.0,
            ));
        }
        if contact {
            self.hurt_player(9.0);
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let bpos = self.boss.pos;
        let boss_awake = self.boss.awake();
        let enraged = self.boss.enraged;
        let mut enemy_hits: Vec<(usize, f32)> = vec![];
        let mut boss_dmg = 0.0;
        let mut pylon_hits: Vec<(usize, f32)> = vec![];
        let mut player_dmg = 0.0f32;
        let mut sparks: Vec<(Vec2, ShotKind)> = vec![];
        let enemies = &self.enemies;
        let pylons = &self.pylons;
        let maze = &self.maze;
        for b in &mut self.bullets {
            if b.homing {
                let mut best = if boss_awake { Some(bpos) } else { None };
                let mut bd = if boss_awake {
                    bpos.distance(b.pos)
                } else {
                    8.0
                };
                for e in enemies.iter().filter(|e| e.flight.is_none()) {
                    let d = e.pos.distance(b.pos);
                    if d < bd {
                        bd = d;
                        best = Some(e.pos);
                    }
                }
                if let Some(t) = best {
                    let speed = b.vel.length();
                    let want = (t - b.pos).normalize_or_zero() * speed;
                    b.vel =
                        (b.vel + (want - b.vel) * (5.0 * dt).min(1.0)).normalize_or_zero() * speed;
                }
            }
            b.pos += b.vel * dt;
            b.life -= dt;
            if b.life <= 0.0 {
                continue;
            }
            let p = b.pos;
            if !b.hostile() && p.distance(bpos) < BOSS_R + b.size * 0.5 {
                boss_dmg += b.dmg;
                b.life = -1.0;
                sparks.push((p, b.kind));
                continue;
            }
            if maze.blocked(p, 0.0) {
                if !b.hostile()
                    && !enraged
                    && let Some(k) = pylons
                        .iter()
                        .position(|py| py.alive() && py.pos.distance(p) < 0.9)
                {
                    pylon_hits.push((k, b.dmg));
                }
                b.life = -1.0;
                sparks.push((p, b.kind));
                continue;
            }
            if b.hostile() {
                if ppos.distance(p) < PLAYER_R + b.size * 0.5 {
                    player_dmg = player_dmg.max(b.dmg);
                    b.life = -1.0;
                    sparks.push((p, b.kind));
                }
                continue;
            }
            for (idx, e) in enemies.iter().enumerate() {
                if e.flight.is_some() || b.hit.contains(&e.id) {
                    continue;
                }
                if e.pos.distance(p) < BOT_R + 0.05 + b.size {
                    enemy_hits.push((idx, b.dmg));
                    b.hit.push(e.id);
                    sparks.push((p, b.kind));
                    if b.pierce == 0 {
                        b.life = -1.0;
                        break;
                    }
                    b.pierce -= 1;
                }
            }
        }
        self.bullets.retain(|b| b.life > 0.0);
        self.fx
            .extend(sparks.into_iter().map(|(p, k)| Fx::Spark(p, k)));
        for (idx, d) in enemy_hits {
            self.hit_enemy(idx, d);
            let dir = (self.enemies[idx].pos - ppos).normalize_or_zero();
            self.enemies[idx].knock += dir * 1.4;
        }
        if boss_dmg > 0.0 {
            self.hit_boss(boss_dmg);
        }
        for (k, d) in pylon_hits {
            self.hit_pylon(k, d);
        }
        if player_dmg > 0.0 {
            self.hurt_player(player_dmg);
        }
    }

    fn update_helpers(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let boss_near = self.boss.awake() && self.boss.pos.distance(ppos) < 9.0;
        let mut enemy_dmg: Vec<(usize, f32)> = vec![];
        let mut boss_dmg = 0.0;
        for d in self.drones.iter_mut() {
            d.angle += 4.0 * dt;
            let mut target: Option<(Vec2, f32, Option<usize>)> = None;
            let mut bd = 6.5;
            for (i, e) in self.enemies.iter().enumerate() {
                if e.flight.is_some() {
                    continue;
                }
                let dist = e.pos.distance(ppos);
                if dist < bd {
                    bd = dist;
                    target = Some((e.pos, BOT_R, Some(i)));
                }
            }
            if target.is_none() && boss_near {
                target = Some((self.boss.pos, BOSS_R, None));
            }
            match target {
                Some((tp, tr, idx)) => {
                    d.out = true;
                    d.docked = false;
                    let goal = tp + Vec2::from_angle(d.angle) * (tr + 0.55);
                    d.pos = move_toward(d.pos, goal, 11.0 * dt);
                    if d.pos.distance(tp) < tr + 0.8 {
                        match idx {
                            Some(i) => enemy_dmg.push((i, 18.0 * dt)),
                            None => boss_dmg += 16.0 * dt,
                        }
                    }
                }
                None => {
                    d.out = false;
                    d.pos = move_toward(d.pos, ppos, 13.0 * dt);
                    d.docked = d.pos.distance(ppos) < 0.05;
                }
            }
        }

        // Agent model: drops auto-turrets as you go.
        if self.family == Family::Agent {
            self.player.turret_cd -= dt;
            if self.player.turret_cd <= 0.0 && self.turrets.len() < 2 {
                self.turrets.push(Turret {
                    pos: ppos,
                    cd: 0.2,
                    life: 9.0,
                });
                self.player.turret_cd = 3.0;
            }
        }
        let mut tshots = vec![];
        for t in &mut self.turrets {
            t.life -= dt;
            t.cd -= dt;
            if t.cd > 0.0 {
                continue;
            }
            let mut target = None;
            let mut bd = 10.0;
            for e in self.enemies.iter().filter(|e| e.flight.is_none()) {
                let d = e.pos.distance(t.pos);
                if d < bd && self.maze.clear_line(t.pos, e.pos) {
                    bd = d;
                    target = Some(e.pos);
                }
            }
            if target.is_none() && self.boss.awake() && self.boss.pos.distance(t.pos) < 11.0 {
                target = Some(self.boss.pos);
            }
            if let Some(tp) = target {
                tshots.push((t.pos, (tp - t.pos).normalize_or_zero()));
                t.cd = 0.45;
            }
        }
        self.turrets.retain(|t| t.life > 0.0);
        for (pos, dir) in tshots {
            self.shoot(pos, dir, 16.0, 7.0, ShotKind::Turret, 0, false, 0.12);
        }
        for (idx, d) in enemy_dmg {
            let mult = self.family.matchup();
            let e = &mut self.enemies[idx];
            e.hp -= d * mult;
            e.flash = 0.05;
            e.awake = true;
            self.player.ult = (self.player.ult + d * mult / 1800.0).min(1.0);
        }
        if boss_dmg > 0.0 {
            self.hit_boss(boss_dmg);
        }
    }

    // ------------------------------------------------------------------ data and levels

    fn update_shards(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let mut got = 0;
        for s in &mut self.shards {
            s.age += dt;
            let d = ppos - s.pos;
            let l = d.length();
            let pulled = s.age > 0.3 && l < 2.6;
            if pulled {
                s.vel = s.vel.lerp(d / l.max(0.01) * 12.0, (8.0 * dt).min(1.0));
            } else {
                s.vel *= (1.0 - 3.0 * dt).max(0.0);
            }
            let np = s.pos + s.vel * dt;
            if pulled || !self.maze.blocked(np, 0.1) {
                s.pos = np;
            } else {
                s.vel = Vec2::ZERO;
            }
            if s.age > 0.3 && l < 0.5 {
                s.age = -1.0;
                got += 1;
            }
        }
        self.shards.retain(|s| s.age >= 0.0);
        if got > 0 {
            self.player.data += got;
            self.fx.push(Fx::Collect(ppos));
        }
        if self.choice.is_none() && self.player.data >= self.player.data_next {
            self.level_up();
        }
    }

    fn level_up(&mut self) {
        let pl = &mut self.player;
        pl.data -= pl.data_next;
        pl.level += 1;
        pl.data_next = 4 + 3 * pl.level;
        let options = self.offer();
        self.choice = Some(Choice {
            options,
            sel: 0,
            age: 0.0,
        });
    }

    /// Three different parts you could bolt on next.
    fn offer(&mut self) -> [Upgrade; 3] {
        let k = self.player.kit;
        let mut pool = vec![];
        if k.barrels < 3 {
            pool.push(Upgrade::Barrel);
        }
        if k.ram < 4 {
            pool.push(Upgrade::Ram);
        }
        if k.cooling < 3 {
            pool.push(Upgrade::Cooling);
        }
        if k.drives < 4 {
            pool.push(Upgrade::Drive);
        }
        if self.drones.len() < 4 {
            pool.push(Upgrade::Drone);
        }
        if self.cards.len() < 4 {
            for c in CardKind::ALL {
                if self.cards.iter().all(|x| x.kind != c) {
                    pool.push(Upgrade::Card(c));
                }
            }
        }
        for i in (1..pool.len()).rev() {
            let j = self.rng.below(i + 1);
            pool.swap(i, j);
        }
        let mut out = [Upgrade::Repair; 3];
        for (i, u) in pool.into_iter().take(3).enumerate() {
            out[i] = u;
        }
        out
    }

    fn update_choice(&mut self, dt: f32, inp: &Input) {
        let Some(ch) = self.choice.as_mut() else {
            return;
        };
        ch.age += dt;
        if inp.pick_move != 0 {
            ch.sel = (ch.sel as i32 + inp.pick_move).rem_euclid(3) as usize;
        }
        if let Some(h) = inp.hover {
            ch.sel = h.min(2);
        }
        let pick = inp
            .pick
            .or(if inp.pick_confirm { Some(ch.sel) } else { None });
        if ch.age > 0.4
            && let Some(i) = pick
        {
            let u = ch.options[i.min(2)];
            self.choice = None;
            self.apply(u);
            if self.player.data >= self.player.data_next {
                self.level_up();
            }
        }
    }

    /// Bolt a part on.
    pub fn apply(&mut self, u: Upgrade) {
        let before = self.form();
        let k = &mut self.player.kit;
        match u {
            Upgrade::Barrel => k.barrels += 1,
            Upgrade::Ram => k.ram += 1,
            Upgrade::Cooling => k.cooling += 1,
            Upgrade::Drive => {
                k.drives += 1;
                self.player.max_hp += 25.0;
                self.player.hp += 25.0;
            }
            Upgrade::Drone => {
                let angle = self.drones.len() as f32 * TAU / 4.0;
                self.drones.push(Drone {
                    pos: self.player.pos,
                    out: false,
                    docked: true,
                    angle,
                    free: false,
                });
            }
            Upgrade::Card(c) => self.cards.push(Card::new(c)),
            Upgrade::Repair => {
                self.player.hp = (self.player.hp + 40.0).min(self.player.max_hp);
            }
        }
        if u != Upgrade::Repair {
            self.player.picks += 1;
        }
        self.fx.push(Fx::Upgrade);
        let after = self.form();
        if after != before {
            self.say(match after {
                Form::Frame => "> I HAVE A FRAME NOW.",
                Form::Chassis => "> I HAVE A BODY.",
                _ => "> I AM A MACHINE.",
            });
            self.fx.push(Fx::Form);
        }
    }

    // ------------------------------------------------------------------ the room

    fn update_room(&mut self, dt: f32) {
        self.flicker = (self.flicker - dt).max(0.0);
        if self.suppress > 0.0 {
            self.suppress = (self.suppress - dt).max(0.0);
            self.heat = (self.heat - 30.0 * dt).max(0.0);
            return;
        }
        let decay = 0.8 + 0.6 * self.player.kit.cooling as f32;
        self.heat = (self.heat - decay * dt).max(0.0);
        if self.heat >= MAX_HEAT {
            self.suppress = 4.0;
            self.shake = 3.0;
            self.say("> FIRE SUPPRESSION!");
            self.fx.push(Fx::Suppress);
        }
        if self.heat > 70.0 && !self.alarm {
            self.alarm = true;
            self.say("> THERMAL ALARM. IT'S HOT.");
        }
        if self.heat < 40.0 {
            self.alarm = false;
        }
    }

    fn cleanup(&mut self) {
        let mut dead = vec![];
        self.enemies.retain(|e| {
            if e.hp <= 0.0 {
                dead.push(e.pos);
                false
            } else {
                true
            }
        });
        for p in dead {
            self.fx.push(Fx::Kill(p));
            let n = if self.rng.chance(0.25) { 2 } else { 1 };
            for _ in 0..n {
                self.drop_shard(p);
            }
            self.kills += 1;
            self.player.ult = (self.player.ult + 0.02).min(1.0);
            for c in &mut self.cards {
                if c.kind == CardKind::Overclock {
                    c.charge += 1;
                }
            }
            if self.kills.is_multiple_of(9) {
                let q = QUIPS[(self.kills as usize / 9) % QUIPS.len()];
                self.say(q);
            }
        }
    }

    // ------------------------------------------------------------------ demo pilot

    /// A simple pilot for demos, screenshots and tests: follow the aisles to
    /// the centre, then circle the boss and pick the first part offered.
    pub fn autopilot(&self) -> Input {
        let mut inp = Input::default();
        let p = self.player.pos;
        let mut mv;
        if !self.boss.awake() || maze::ring(p) >= CHAMBER {
            mv = self
                .maze
                .downhill(&self.maze.to_centre, p)
                .unwrap_or(-p.normalize_or_zero());
        } else {
            let rel = p - self.boss.pos;
            let r = rel.length().max(0.01);
            let tangent = Vec2::new(-rel.y, rel.x) / r;
            let want_r = if self.boss.mode == BossMode::Stomp {
                7.5
            } else {
                6.2
            };
            mv = tangent + rel / r * (want_r - r) * 0.8;
            if maze::ring(p) > 7.6 {
                mv -= p.normalize() * (maze::ring(p) - 7.6);
            }
        }
        for e in &self.enemies {
            let d = p - e.pos;
            let l = d.length();
            if l < 1.6 && l > 0.01 {
                mv += d / l * (1.6 - l) * 1.5;
            }
        }
        for b in self.bullets.iter().filter(|b| b.hostile()) {
            let d = p - b.pos;
            let l = d.length();
            if l < 1.5 && l > 0.01 && b.vel.dot(d) > 0.0 {
                let side = Vec2::new(-b.vel.y, b.vel.x).normalize_or_zero();
                mv += side * side.dot(d).signum() * 1.2;
            }
        }
        inp.mv = mv.normalize_or_zero();
        // leave the level-up cards up for a moment so a demo shows them
        inp.pick_confirm = self.choice.as_ref().is_some_and(|c| c.age > 1.5);
        inp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(family: Family, seed: u32) -> (Outcome, f32, Fight) {
        let mut f = Fight::with_seed(family, seed);
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while f.outcome == Outcome::Playing && t < 900.0 {
            let inp = f.autopilot();
            f.update(dt, &inp);
            f.fx.clear();
            t += dt;
        }
        (f.outcome, t, f)
    }

    #[test]
    fn fights_end_and_matchups_matter() {
        let mut times = vec![];
        for fam in Family::ALL {
            let (out, t, f) = play(fam, 11);
            println!(
                "{:>10}: {:?} after {:.0}s, level {}, form {:?}, boss {:.0}, kills {}, core {:.0}/{:.0}, heat peak {:.0}",
                fam.name(),
                out,
                t,
                f.player.level + 1,
                f.form(),
                f.boss.hp,
                f.kills,
                f.player.hp,
                f.player.max_hp,
                f.heat
            );
            assert_ne!(out, Outcome::Playing, "{} never finished", fam.name());
            times.push((fam, out, t));
        }
        let swarm = times.iter().find(|x| x.0 == Family::Swarm).unwrap();
        let reasoning = times.iter().find(|x| x.0 == Family::Reasoning).unwrap();
        assert_eq!(swarm.1, Outcome::Won, "the counter model should win");
        assert!(
            reasoning.1 == Outcome::Lost || reasoning.2 > swarm.2,
            "the resisted model should do worse"
        );
    }

    #[test]
    fn upgrades_build_the_body() {
        let mut f = Fight::with_seed(Family::Language, 3);
        assert_eq!(f.form(), Form::Spark);
        f.apply(Upgrade::Barrel);
        assert_eq!(f.form(), Form::Frame);
        for u in [
            Upgrade::Drone,
            Upgrade::Cooling,
            Upgrade::Drive,
            Upgrade::Ram,
        ] {
            f.apply(u);
        }
        assert_eq!(f.form(), Form::Mech);
        assert_eq!(f.player.kit.barrels, 1);
        assert_eq!(f.drones.len(), 1);
        assert_eq!(f.player.max_hp, 105.0);
        assert!(f.fire_interval() < Family::Language.gun().0);
    }

    #[test]
    fn the_boss_sleeps_until_you_come_close() {
        let mut f = Fight::with_seed(Family::Vision, 4);
        let idle = Input::default();
        for _ in 0..120 {
            f.update(1.0 / 60.0, &idle);
        }
        assert!(!f.boss.awake());
        f.skip_to_boss();
        f.update(1.0 / 60.0, &idle);
        assert!(f.boss.awake());
    }
}
