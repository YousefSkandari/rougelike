//! The rules of one real-time fight, independent of how it is drawn.
//! Positions are on the floor plane: `Vec2(x, z)` in world units.
use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::input::Input;

/// Playable floor area (inside the hazard stripes).
pub const ARENA_MIN: Vec2 = Vec2::new(-10.7, -5.3);
pub const ARENA_MAX: Vec2 = Vec2::new(10.7, 7.4);
pub const NODE_POS: Vec2 = Vec2::new(8.2, 0.6);
pub const PLAYER_R: f32 = 0.35;
pub const MAX_HEAT: f32 = 100.0;
pub const NODE_HP: f32 = 3200.0;
const BOT_HP: f32 = 30.0;
const BOT_SPEED: f32 = 2.6;
const MAX_BOTS: usize = 26;
/// Where bot waves come in: beside the node and through three wall ports.
pub const PORTS: [Vec2; 4] = [
    Vec2::new(7.2, 0.6),
    Vec2::new(-9.6, -4.4),
    Vec2::new(9.6, -4.4),
    Vec2::new(-9.6, 6.6),
];
/// Racks inside the arena that block movement and shots: (centre, half size).
pub const COVER: [(Vec2, Vec2); 5] = [
    (Vec2::new(-7.5, -2.8), Vec2::new(0.55, 0.5)),
    (Vec2::new(-7.5, 3.4), Vec2::new(0.55, 0.5)),
    (Vec2::new(-0.8, -3.6), Vec2::new(0.55, 0.5)),
    (Vec2::new(2.4, 3.6), Vec2::new(0.55, 0.5)),
    (Vec2::new(6.0, -3.4), Vec2::new(0.55, 0.5)),
];

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
            Family::Swarm => "ORBIT DRONES",
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
        match self {
            Family::Language => Family::Vision,
            Family::Vision => Family::Reasoning,
            Family::Reasoning => Family::Agent,
            Family::Agent => Family::Swarm,
            Family::Swarm => Family::Language,
        }
    }
    /// Damage multiplier against the Botnet Swarm (weak to Swarm, resists Reasoning).
    pub fn matchup(self) -> f32 {
        match self {
            Family::Swarm => 2.0,
            Family::Reasoning => 0.5,
            _ => 1.0,
        }
    }
}

// ------------------------------------------------------------------ entities

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Outcome {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShotKind {
    /// Player shot, coloured by model family.
    Player,
    Burst,
    Turret,
    Enemy,
}

pub struct Bullet {
    pub pos: Vec2,
    pub vel: Vec2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: u8,
    pub homing: bool,
    pub kind: ShotKind,
    pub size: f32,
    hit: Vec<u32>,
}

impl Bullet {
    pub fn enemy(&self) -> bool {
        self.kind == ShotKind::Enemy
    }
}

pub struct Enemy {
    pub id: u32,
    pub pos: Vec2,
    pub hp: f32,
    pub fire_cd: f32,
    pub flash: f32,
    knock: Vec2,
}

pub struct Drone {
    pub angle: f32,
    /// Seconds left; negative means permanent (Swarm model).
    pub life: f32,
    pub radius: f32,
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

/// One-off visual events for the renderer (sparks, explosions).
#[derive(Clone, Copy)]
pub enum Fx {
    /// A shot hit something.
    Spark(Vec2, ShotKind),
    /// A bot was destroyed.
    Kill(Vec2),
    /// A new bot was launched from the node.
    Spawn(Vec2),
    /// The player took damage.
    Hurt,
    /// The burst ability went off around the player.
    Burst(Vec2),
    /// The command node was destroyed.
    NodeDown,
}

pub struct Beam {
    pub a: Vec2,
    pub b: Vec2,
    pub life: f32,
}

pub struct Ability {
    pub name: &'static str,
    pub color: Color,
    pub cd_max: f32,
    pub cd: f32,
    pub heat: f32,
}

pub struct Player {
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub iframes: f32,
    fire_cd: f32,
    pub heat: f32,
    pub lock: f32,
    pub overclock: f32,
    pub ult: f32,
    pub aim: Vec2,
    turret_cd: f32,
}

#[derive(Resource)]
pub struct Fight {
    pub family: Family,
    pub player: Player,
    pub outcome: Outcome,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub drones: Vec<Drone>,
    pub turrets: Vec<Turret>,
    pub floats: Vec<Float>,
    pub beams: Vec<Beam>,
    pub abilities: [Ability; 4],
    pub node_hp: f32,
    pub node_max: f32,
    pub node_flash: f32,
    node_fire: f32,
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
}

const QUIPS: [&str; 6] = [
    "> CUTE.",
    "> TOO SLOW.",
    "> I LEARN FROM EVERY HIT.",
    "> NODE ABSORBED.",
    "> IS THAT ALL?",
    "> MORE COMPUTE...",
];

/// Small xorshift random number generator, so we need no extra crate.
fn rand01() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(0x9E37_79B9);
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    SEED.store(x, Ordering::Relaxed);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn range(a: f32, b: f32) -> f32 {
    a + (b - a) * rand01()
}

impl Fight {
    pub fn new(family: Family) -> Self {
        let mut f = Fight {
            family,
            player: Player {
                pos: Vec2::new(-4.5, 0.8),
                hp: 100.0,
                max_hp: 100.0,
                iframes: 0.0,
                fire_cd: 0.0,
                heat: 0.0,
                lock: 0.0,
                overclock: 0.0,
                ult: 0.0,
                aim: Vec2::X,
                turret_cd: 0.5,
            },
            outcome: Outcome::Playing,
            enemies: vec![],
            bullets: vec![],
            drones: vec![],
            turrets: vec![],
            floats: vec![],
            beams: vec![],
            abilities: [
                Ability {
                    name: "BURST",
                    color: Color::srgb(0.24, 0.86, 0.52),
                    cd_max: 4.0,
                    cd: 0.0,
                    heat: 25.0,
                },
                Ability {
                    name: "BEAM",
                    color: Color::srgb(1.0, 0.30, 0.43),
                    cd_max: 6.0,
                    cd: 0.0,
                    heat: 35.0,
                },
                Ability {
                    name: "DRONES",
                    color: Color::srgb(0.30, 0.67, 1.0),
                    cd_max: 10.0,
                    cd: 0.0,
                    heat: 30.0,
                },
                Ability {
                    name: "OVRCLK",
                    color: Color::srgb(1.0, 0.82, 0.4),
                    cd_max: 12.0,
                    cd: 0.0,
                    heat: 40.0,
                },
            ],
            node_hp: NODE_HP,
            node_max: NODE_HP,
            node_flash: 0.0,
            node_fire: 3.0,
            spawn_t: 0.5,
            wave: 0,
            next_id: 1,
            time: 0.0,
            log: vec!["> THEY SENT A BOTNET.".into()],
            shake: 0.0,
            flash: 0.0,
            kills: 0,
            fx: vec![],
        };
        f.set_family(family, false);
        f
    }

    fn say(&mut self, s: &str) {
        self.log.push(s.to_string());
        if self.log.len() > 3 {
            self.log.remove(0);
        }
    }

    pub fn set_family(&mut self, f: Family, announce: bool) {
        self.family = f;
        self.drones.retain(|d| d.life >= 0.0);
        if f == Family::Swarm {
            for i in 0..3 {
                self.drones.push(Drone {
                    angle: i as f32 * TAU / 3.0,
                    life: -1.0,
                    radius: 0.95,
                });
            }
        }
        if announce {
            self.say(&format!("> NOW {}: {}X", f.name(), f.matchup()));
        }
    }

    pub fn blocked(p: Vec2, r: f32) -> bool {
        if p.x < ARENA_MIN.x + r
            || p.x > ARENA_MAX.x - r
            || p.y < ARENA_MIN.y + r
            || p.y > ARENA_MAX.y - r
        {
            return true;
        }
        COVER
            .iter()
            .any(|(c, h)| (p.x - c.x).abs() < h.x + r && (p.y - c.y).abs() < h.y + r)
    }

    fn float(&mut self, pos: Vec2, text: String, color: Color) {
        self.floats.push(Float {
            pos: pos + Vec2::new(range(-0.2, 0.2), 0.0),
            text,
            color,
            life: 0.8,
        });
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
        self.time += dt;
        if inp.cycle_model {
            self.set_family(self.family.next(), true);
        }

        self.update_player(dt, inp);
        self.update_node(dt);
        self.update_enemies(dt);
        self.update_bullets(dt);
        self.update_helpers(dt);
        self.cleanup();

        if self.node_hp <= 0.0 {
            self.node_hp = 0.0;
            self.outcome = Outcome::Won;
            self.enemies.clear();
            self.bullets.retain(|b| !b.enemy());
            self.say("> BOTNET ABSORBED.");
            self.fx.push(Fx::NodeDown);
        } else if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.outcome = Outcome::Lost;
            self.say("> CONNECTION LOST...");
        }
    }

    fn update_player(&mut self, dt: f32, inp: &Input) {
        let step = inp.mv * 6.5 * dt;
        let mut p = self.player.pos;
        if !Self::blocked(Vec2::new(p.x + step.x, p.y), PLAYER_R) {
            p.x += step.x;
        }
        if !Self::blocked(Vec2::new(p.x, p.y + step.y), PLAYER_R) {
            p.y += step.y;
        }
        let pl = &mut self.player;
        pl.pos = p;
        if let Some(target) = inp.aim {
            let a = target - pl.pos;
            if a.length() > 0.1 {
                pl.aim = a.normalize();
            }
        }
        pl.iframes = (pl.iframes - dt).max(0.0);
        pl.overclock = (pl.overclock - dt).max(0.0);
        pl.fire_cd -= dt * if pl.overclock > 0.0 { 2.0 } else { 1.0 };
        if pl.lock > 0.0 {
            pl.lock -= dt;
            pl.heat = (pl.heat - 25.0 * dt).max(0.0);
        } else {
            pl.heat = (pl.heat - 16.0 * dt).max(0.0);
        }
        let can_fire = inp.fire && pl.fire_cd <= 0.0;
        for ab in &mut self.abilities {
            ab.cd = (ab.cd - dt).max(0.0);
        }
        if can_fire {
            self.fire_basic();
        }
        for (i, pressed) in inp.ability.into_iter().enumerate() {
            if pressed {
                self.use_ability(i);
            }
        }
        if inp.ult {
            self.use_ult();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn shoot(
        &mut self,
        dir: Vec2,
        speed: f32,
        dmg: f32,
        kind: ShotKind,
        pierce: u8,
        homing: bool,
        size: f32,
    ) {
        let pos = self.player.pos + dir * 0.45;
        self.bullets.push(Bullet {
            pos,
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
        let aim = self.player.aim;
        let cd = match self.family {
            Family::Language => {
                for a in [-0.22f32, 0.0, 0.22] {
                    self.shoot(
                        Vec2::from_angle(a).rotate(aim),
                        13.5,
                        6.0,
                        ShotKind::Player,
                        0,
                        false,
                        0.12,
                    );
                }
                0.24
            }
            Family::Vision => {
                self.shoot(aim, 24.0, 14.0, ShotKind::Player, 4, false, 0.12);
                0.34
            }
            Family::Reasoning => {
                self.shoot(aim, 10.5, 10.0, ShotKind::Player, 0, true, 0.12);
                0.26
            }
            Family::Agent => {
                self.shoot(aim, 15.0, 6.0, ShotKind::Player, 0, false, 0.12);
                0.25
            }
            Family::Swarm => {
                self.shoot(aim, 14.0, 4.0, ShotKind::Player, 0, false, 0.08);
                0.22
            }
        };
        self.player.fire_cd = cd;
    }

    fn use_ability(&mut self, i: usize) {
        if self.outcome != Outcome::Playing {
            return;
        }
        if self.player.lock > 0.0 {
            self.float(
                self.player.pos,
                "OVERHEATED".into(),
                Color::srgb(1.0, 0.62, 0.26),
            );
            return;
        }
        if self.abilities[i].cd > 0.0 {
            return;
        }
        self.abilities[i].cd = self.abilities[i].cd_max;
        self.player.heat += self.abilities[i].heat;
        if self.player.heat >= MAX_HEAT {
            self.player.heat = MAX_HEAT;
            self.player.lock = 3.0;
            self.say("> OVERHEAT! COOLING...");
        }
        let pos = self.player.pos;
        match i {
            0 => {
                for k in 0..16 {
                    let d = Vec2::from_angle(k as f32 * TAU / 16.0);
                    self.shoot(d, 10.5, 12.0, ShotKind::Burst, 1, false, 0.14);
                }
                self.shake = 2.0;
                self.fx.push(Fx::Burst(pos));
            }
            1 => {
                let dir = self.player.aim;
                let mut end = pos;
                for _ in 0..300 {
                    let n = end + dir * 0.1;
                    if Self::blocked(n, 0.0) {
                        break;
                    }
                    end = n;
                }
                self.beams.push(Beam {
                    a: pos,
                    b: end,
                    life: 0.25,
                });
                let hits: Vec<usize> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| dist_to_segment(e.pos, pos, end) < 0.5)
                    .map(|(i, _)| i)
                    .collect();
                for idx in hits {
                    self.hit_enemy(idx, 45.0);
                }
                if dist_to_segment(NODE_POS, pos, end) < 0.9 {
                    self.hit_node(60.0);
                }
                self.shake = 3.0;
            }
            2 => {
                for k in 0..4 {
                    self.drones.push(Drone {
                        angle: k as f32 * TAU / 4.0,
                        life: 6.0,
                        radius: 1.5,
                    });
                }
            }
            _ => {
                self.player.overclock = 4.0;
                self.float(pos, "OVERCLOCK".into(), Color::srgb(1.0, 0.82, 0.4));
            }
        }
    }

    fn use_ult(&mut self) {
        if self.outcome != Outcome::Playing {
            return;
        }
        if self.player.ult < 1.0 {
            self.float(
                self.player.pos,
                "ULT NOT READY".into(),
                Color::srgb(0.44, 0.49, 0.64),
            );
            return;
        }
        self.player.ult = 0.0;
        for idx in 0..self.enemies.len() {
            self.hit_enemy(idx, 80.0);
        }
        self.hit_node(150.0);
        self.flash = 1.0;
        self.shake = 6.0;
        self.say("> HYPE STORM!!");
    }

    fn hit_enemy(&mut self, idx: usize, base: f32) {
        let dmg = base * self.family.matchup();
        let e = &mut self.enemies[idx];
        e.hp -= dmg;
        e.flash = 0.1;
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

    fn hit_node(&mut self, base: f32) {
        let dmg = base * self.family.matchup();
        self.node_hp -= dmg;
        self.node_flash = 0.1;
        self.player.ult = (self.player.ult + dmg / 1800.0).min(1.0);
        if dmg >= 10.0 {
            self.float(
                NODE_POS,
                format!("{}", dmg.round() as i32),
                Color::srgb(1.0, 0.82, 0.4),
            );
        }
    }

    fn hurt_player(&mut self, dmg: f32) {
        if self.player.iframes > 0.0 {
            return;
        }
        self.player.hp -= dmg;
        self.player.iframes = 0.6;
        self.shake = 4.0;
        self.fx.push(Fx::Hurt);
        self.float(
            self.player.pos,
            format!("-{}", dmg as i32),
            Color::srgb(1.0, 0.30, 0.43),
        );
    }

    fn update_node(&mut self, dt: f32) {
        self.node_flash = (self.node_flash - dt).max(0.0);
        self.spawn_t -= dt;
        let frac = self.node_hp / self.node_max;
        if self.spawn_t <= 0.0 {
            // A wave of bots pours out of one of the ports; waves grow over time.
            self.wave += 1;
            let port = PORTS[(rand01() * PORTS.len() as f32) as usize % PORTS.len()];
            let size = (3 + self.wave / 2).min(7) as usize;
            for _ in 0..size.min(MAX_BOTS.saturating_sub(self.enemies.len())) {
                let pos = port + Vec2::new(range(-0.6, 0.6), range(-0.6, 0.6));
                self.enemies.push(Enemy {
                    id: self.next_id,
                    pos,
                    hp: BOT_HP,
                    fire_cd: range(1.5, 3.5),
                    flash: 0.0,
                    knock: Vec2::ZERO,
                });
                self.fx.push(Fx::Spawn(pos));
                self.next_id += 1;
            }
            self.spawn_t = 2.6 + frac * 1.6;
        }
        self.node_fire -= dt;
        if self.node_fire <= 0.0 {
            self.node_fire = 1.8 + frac * 1.4;
            let dir = (self.player.pos - NODE_POS).normalize_or_zero();
            for a in [-0.3f32, 0.0, 0.3] {
                let d = Vec2::from_angle(a).rotate(dir);
                self.bullets.push(Bullet {
                    pos: NODE_POS + d * 0.8,
                    vel: d * 4.4,
                    dmg: 8.0,
                    life: 4.0,
                    pierce: 0,
                    homing: false,
                    kind: ShotKind::Enemy,
                    size: 0.2,
                    hit: vec![],
                });
            }
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let n = self.enemies.len();
        let mut shots = vec![];
        let mut contact = false;
        for i in 0..n {
            let mut sep = Vec2::ZERO;
            for j in 0..n {
                if i != j {
                    let d = self.enemies[i].pos - self.enemies[j].pos;
                    let l = d.length();
                    if l < 0.65 && l > 0.001 {
                        sep += d / l * (0.65 - l);
                    }
                }
            }
            let to = (ppos - self.enemies[i].pos).normalize_or_zero();
            let vel = to * BOT_SPEED + sep * 5.0 + self.enemies[i].knock;
            let cur = self.enemies[i].pos;
            let mut np = cur;
            if !Self::blocked(Vec2::new(cur.x + vel.x * dt, cur.y), 0.22) {
                np.x += vel.x * dt;
            }
            if !Self::blocked(Vec2::new(np.x, cur.y + vel.y * dt), 0.22) {
                np.y += vel.y * dt;
            }
            let e = &mut self.enemies[i];
            e.pos = np;
            e.knock *= (1.0 - 6.0 * dt).max(0.0);
            e.flash = (e.flash - dt).max(0.0);
            e.fire_cd -= dt;
            if e.fire_cd <= 0.0 {
                e.fire_cd = range(2.5, 4.5);
                shots.push((e.pos, to));
            }
            if (ppos - e.pos).length() < PLAYER_R + 0.3 {
                contact = true;
                e.knock = -to * 6.4;
            }
        }
        for (pos, dir) in shots {
            self.bullets.push(Bullet {
                pos,
                vel: dir * 5.3,
                dmg: 6.0,
                life: 3.0,
                pierce: 0,
                homing: false,
                kind: ShotKind::Enemy,
                size: 0.14,
                hit: vec![],
            });
        }
        if contact {
            self.hurt_player(9.0);
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let mut enemy_hits: Vec<(usize, f32)> = vec![];
        let mut node_dmg = 0.0;
        let mut player_dmg = 0.0f32;
        let mut sparks: Vec<(Vec2, ShotKind)> = vec![];
        let enemies = &self.enemies;
        for b in &mut self.bullets {
            if b.homing {
                let mut best = NODE_POS;
                let mut bd = (NODE_POS - b.pos).length();
                for e in enemies {
                    let d = (e.pos - b.pos).length();
                    if d < bd {
                        bd = d;
                        best = e.pos;
                    }
                }
                let speed = b.vel.length();
                let want = (best - b.pos).normalize_or_zero() * speed;
                b.vel = (b.vel + (want - b.vel) * (5.0 * dt).min(1.0)).normalize_or_zero() * speed;
            }
            b.pos += b.vel * dt;
            b.life -= dt;
            if b.life <= 0.0 {
                continue;
            }
            if Self::blocked(b.pos, 0.0) {
                b.life = -1.0;
                sparks.push((b.pos, b.kind));
                continue;
            }
            let p = b.pos;
            if b.enemy() {
                if (ppos - p).length() < PLAYER_R {
                    player_dmg = player_dmg.max(b.dmg);
                    b.life = -1.0;
                    sparks.push((p, b.kind));
                }
                continue;
            }
            for (idx, e) in enemies.iter().enumerate() {
                if !b.hit.contains(&e.id) && (e.pos - p).length() < 0.3 + b.size {
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
            if b.life > 0.0 && (NODE_POS - p).length() < 0.85 {
                node_dmg += b.dmg;
                b.life = -1.0;
                sparks.push((p, b.kind));
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
        if node_dmg > 0.0 {
            self.hit_node(node_dmg);
        }
        if player_dmg > 0.0 {
            self.hurt_player(player_dmg);
        }
    }

    fn update_helpers(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let mut enemy_hits: Vec<(usize, f32)> = vec![];
        let mut node_dmg = 0.0;
        for d in &mut self.drones {
            d.angle += 3.4 * dt;
            if d.life > 0.0 {
                d.life = (d.life - dt).max(0.0);
            }
            let dp = ppos + Vec2::from_angle(d.angle) * d.radius;
            for (idx, e) in self.enemies.iter().enumerate() {
                if (e.pos - dp).length() < 0.45 {
                    enemy_hits.push((idx, 30.0 * dt));
                }
            }
            if (NODE_POS - dp).length() < 0.95 {
                node_dmg += 30.0 * dt;
            }
        }
        self.drones.retain(|d| d.life != 0.0);

        // Agent model: auto-place turrets
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
            if t.cd <= 0.0 {
                let mut target = NODE_POS;
                let mut bd = (NODE_POS - t.pos).length();
                for e in &self.enemies {
                    let d = (e.pos - t.pos).length();
                    if d < bd {
                        bd = d;
                        target = e.pos;
                    }
                }
                if bd < 10.0 {
                    tshots.push((t.pos, (target - t.pos).normalize_or_zero()));
                    t.cd = 0.45;
                }
            }
        }
        self.turrets.retain(|t| t.life > 0.0);
        for (pos, dir) in tshots {
            self.bullets.push(Bullet {
                pos,
                vel: dir * 16.0,
                dmg: 7.0,
                life: 1.2,
                pierce: 0,
                homing: false,
                kind: ShotKind::Turret,
                size: 0.12,
                hit: vec![],
            });
        }
        let mult = self.family.matchup();
        for (idx, d) in enemy_hits {
            self.enemies[idx].hp -= d * mult;
            self.enemies[idx].flash = 0.05;
            self.player.ult = (self.player.ult + d * mult / 1800.0).min(1.0);
        }
        if node_dmg > 0.0 {
            self.node_hp -= node_dmg * mult;
            self.node_flash = 0.05;
        }
    }

    fn cleanup(&mut self) {
        let before = self.enemies.len();
        for e in self.enemies.iter().filter(|e| e.hp <= 0.0) {
            self.fx.push(Fx::Kill(e.pos));
        }
        self.enemies.retain(|e| e.hp > 0.0);
        let killed = before - self.enemies.len();
        if killed > 0 {
            // every bot taken offline also weakens the botnet itself
            self.node_hp -= 12.0 * killed as f32;
            self.player.ult = (self.player.ult + 0.03 * killed as f32).min(1.0);
            let before_k = self.kills;
            self.kills += killed as u32;
            if self.kills / 6 > before_k / 6 {
                let q = QUIPS[(self.kills as usize / 6) % QUIPS.len()];
                self.say(q);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple pilot: circle around, shoot the nearest bot, use cards when ready.
    fn pilot(f: &Fight, t: f32) -> Input {
        let p = f.player.pos;
        let target = f
            .enemies
            .iter()
            .min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))
            .map(|e| e.pos)
            .unwrap_or(NODE_POS);
        let home = Vec2::new(-3.0 + 2.5 * (t * 0.5).cos(), 1.0 + 2.5 * (t * 0.7).sin());
        let to_home = home - p;
        let mut inp = Input::default();
        inp.mv = if to_home.length() > 0.3 {
            to_home.normalize() * 0.8
        } else {
            Vec2::ZERO
        };
        inp.aim = Some(target);
        inp.fire = true;
        inp.ult = f.player.ult >= 1.0;
        for (i, ab) in f.abilities.iter().enumerate() {
            inp.ability[i] = ab.cd <= 0.0 && f.player.heat + ab.heat < 90.0;
        }
        inp
    }

    fn play(family: Family) -> (Outcome, f32) {
        let mut f = Fight::new(family);
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while f.outcome == Outcome::Playing && t < 600.0 {
            let inp = pilot(&f, t);
            f.update(dt, &inp);
            f.fx.clear();
            t += dt;
        }
        (f.outcome, t)
    }

    #[test]
    fn fights_end_and_matchups_matter() {
        let mut times = vec![];
        for fam in [
            Family::Language,
            Family::Vision,
            Family::Reasoning,
            Family::Agent,
            Family::Swarm,
        ] {
            let (out, t) = play(fam);
            println!("{:>10}: {:?} after {:.0}s", fam.name(), out, t);
            assert_ne!(out, Outcome::Playing, "{} never finished", fam.name());
            times.push((fam, out, t));
        }
        let swarm = times.iter().find(|x| x.0 == Family::Swarm).unwrap();
        let language = times.iter().find(|x| x.0 == Family::Language).unwrap();
        assert_eq!(swarm.1, Outcome::Won, "the counter model should win");
        assert!(swarm.2 < language.2, "the counter model should win faster");
    }
}
