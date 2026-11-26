//! The boss's domain: a vast server hall whose rows of racks form a maze.
//! Every aisle eventually leads to the boss at the centre, and every fight
//! gets a new layout. This file is only the layout and the maths on it
//! (collision, line of sight, path finding); `hall.rs` builds the 3D scene.
//!
//! The hall is a grid of 1x1 floor tiles. Rows of racks run toward the
//! centre on all four sides, like spokes, with 3-wide aisles between them.
//! Two square ring walls cut across the aisles, with only a few gaps, and
//! diagonal avenues run in from the corners. So you can always see roughly
//! where the boss is, but you have to find your way in.
use bevy::prelude::*;
use std::collections::VecDeque;

/// Cells per side of the playable grid. One cell is one floor tile (1 unit).
pub const N: i32 = 64;
const HALF: f32 = (N / 2) as f32;
/// Open floor around the boss (square radius).
pub const CHAMBER: f32 = 9.0;
/// Ring walls that cut across the aisles, with a few gaps in each.
pub const RINGS: [f32; 2] = [14.5, 21.5];
/// Cells at or beyond this square radius are the hall's outer wall.
pub const EDGE: f32 = 30.5;
/// Decorative racks keep going out to here, into the dark.
pub const FAR: f32 = 47.5;
/// Where you come in: the far corner, at the end of a diagonal avenue.
pub const START: Vec2 = Vec2::new(25.5, 25.5);
/// The cable pylons that feed the boss.
pub const PYLONS: [Vec2; 4] = [
    Vec2::new(6.5, 6.5),
    Vec2::new(-6.5, 6.5),
    Vec2::new(-6.5, -6.5),
    Vec2::new(6.5, -6.5),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Open,
    Rack,
    Pylon,
}

/// Which way a run of racks points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Z,
}

/// One run of 1 or 2 server cabinets, for the renderer.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub center: Vec2,
    pub axis: Axis,
    pub cabinets: u8,
    /// LED colour scheme, 0..4.
    pub style: u8,
    pub tall: bool,
}

/// Small xorshift random number generator, so we need no extra crate.
#[derive(Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Scramble the seed so that 1, 2, 3... give unrelated sequences.
        let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 16;
        x = x.wrapping_mul(0x7FEB_352D);
        x ^= x >> 15;
        Rng(x.max(1))
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// 0..1
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n.max(1)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

/// Centre of grid column/row `i` in world units.
pub fn centre_of(i: i32) -> f32 {
    i as f32 - HALF + 0.5
}

pub fn cell_of(p: Vec2) -> IVec2 {
    IVec2::new((p.x + HALF).floor() as i32, (p.y + HALF).floor() as i32)
}

pub fn cell_pos(c: IVec2) -> Vec2 {
    Vec2::new(centre_of(c.x), centre_of(c.y))
}

fn inside(c: IVec2) -> bool {
    c.x >= 0 && c.y >= 0 && c.x < N && c.y < N
}

fn idx(c: IVec2) -> usize {
    (c.y * N + c.x) as usize
}

/// "Square radius": distance from the centre measured like a square ring.
pub fn ring(p: Vec2) -> f32 {
    p.x.abs().max(p.y.abs())
}

/// Rows sit at 3.5, 7.5, 11.5... from the centre line, leaving a wide main
/// aisle down the middle of each side and 3-wide aisles between the rest.
fn is_row(lateral: f32) -> bool {
    let a = lateral.abs() + 0.5;
    a >= 4.0 && (a as i32) % 4 == 0 && a.fract() == 0.0
}

/// Cells within this of a diagonal are the open avenues from the corners.
fn on_seam(p: Vec2) -> bool {
    (p.x.abs() - p.y.abs()).abs() < 1.6
}

/// The direction racks run at `p` for a radial row: toward the centre.
fn radial_axis(p: Vec2) -> Axis {
    if p.x.abs() >= p.y.abs() {
        Axis::X
    } else {
        Axis::Z
    }
}

fn flip(a: Axis) -> Axis {
    match a {
        Axis::X => Axis::Z,
        Axis::Z => Axis::X,
    }
}

pub struct Maze {
    pub seed: u32,
    cells: Vec<Cell>,
    axis: Vec<Axis>,
    /// Every rack to draw, including the endless-looking ones outside.
    pub segments: Vec<Segment>,
    /// Floor hatches that bots climb out of.
    pub vents: Vec<Vec2>,
    pub start: Vec2,
    /// Steps from each cell to the boss chamber (`u16::MAX` if cut off).
    pub to_centre: Vec<u16>,
}

impl Maze {
    pub fn generate(seed: u32) -> Maze {
        let mut rng = Rng::new(seed);
        let n = (N * N) as usize;
        let mut cells = vec![Cell::Open; n];
        let mut axis = vec![Axis::X; n];

        // ---- rows, ring walls and the outer wall
        for y in 0..N {
            for x in 0..N {
                let c = IVec2::new(x, y);
                let p = cell_pos(c);
                let r = ring(p);
                let i = idx(c);
                if r >= EDGE {
                    cells[i] = Cell::Rack;
                    axis[i] = flip(radial_axis(p));
                } else if r < CHAMBER {
                    continue;
                } else if RINGS.contains(&r) {
                    cells[i] = Cell::Rack;
                    axis[i] = flip(radial_axis(p));
                } else if !on_seam(p) {
                    let lateral = if p.x.abs() >= p.y.abs() { p.y } else { p.x };
                    if is_row(lateral) {
                        cells[i] = Cell::Rack;
                        axis[i] = radial_axis(p);
                    }
                }
            }
        }

        // ---- gaps in the ring walls: two per side, never straight down the
        // avenue you start in, so you always have to look for a way round.
        let set = |cells: &mut Vec<Cell>, p: Vec2, v: Cell| {
            let c = cell_of(p);
            if inside(c) {
                cells[idx(c)] = v;
            }
        };
        for (ri, &r) in RINGS.iter().enumerate() {
            for side in 0..4 {
                // Candidate gap centres along this side, as a lateral offset.
                let mut cands: Vec<f32> = vec![0.0];
                let mut a = 5.5;
                while a < r - 2.0 {
                    cands.push(a);
                    cands.push(-a);
                    a += 4.0;
                }
                cands.push(r);
                cands.push(-r);
                let mut picked = 0;
                let mut tries = 0;
                while picked < 2 && tries < 40 {
                    tries += 1;
                    let v = cands[rng.below(cands.len())];
                    // side 0: +x, 1: -x, 2: +z, 3: -z
                    let at = |lat: f32| match side {
                        0 => Vec2::new(r, lat),
                        1 => Vec2::new(-r, lat),
                        2 => Vec2::new(lat, r),
                        _ => Vec2::new(lat, -r),
                    };
                    let p = at(v);
                    if v.abs() == r {
                        if ri == 1 && p.x > 0.0 && p.y > 0.0 {
                            continue; // the avenue you start in stays blocked
                        }
                        // A corner: open it and the cells beside it on both walls.
                        let s = Vec2::new(p.x.signum(), p.y.signum());
                        set(&mut cells, p, Cell::Open);
                        for k in 1..=2 {
                            let k = k as f32;
                            set(&mut cells, p - Vec2::new(s.x * k, 0.0), Cell::Open);
                            set(&mut cells, p - Vec2::new(0.0, s.y * k), Cell::Open);
                        }
                    } else {
                        let width: &[f32] = if v == 0.0 {
                            &[-1.5, -0.5, 0.5, 1.5]
                        } else {
                            &[-1.0, 0.0, 1.0]
                        };
                        for &o in width {
                            set(&mut cells, at(v + o), Cell::Open);
                        }
                    }
                    picked += 1;
                }
            }
        }

        // ---- gaps in the rows, so you can slip between neighbouring aisles
        let bands = [(CHAMBER, RINGS[0]), (RINGS[0], RINGS[1]), (RINGS[1], EDGE)];
        for side in 0..4 {
            let mut lat = 3.5f32;
            while lat < EDGE {
                for s in [-1.0f32, 1.0] {
                    let v = lat * s;
                    for &(lo, hi) in &bands {
                        if !rng.chance(0.45) {
                            continue;
                        }
                        let u_lo = (lo + 1.5).max(v.abs() + 2.5);
                        let u_hi = hi - 1.5;
                        if u_hi - u_lo < 2.0 {
                            continue;
                        }
                        let u = (rng.range(u_lo, u_hi - 1.0)).floor() + 0.5;
                        for k in 0..2 {
                            let uu = u + k as f32;
                            let p = match side {
                                0 => Vec2::new(uu, v),
                                1 => Vec2::new(-uu, v),
                                2 => Vec2::new(v, uu),
                                _ => Vec2::new(v, -uu),
                            };
                            if ring(p) < EDGE && !RINGS.contains(&ring(p)) {
                                set(&mut cells, p, Cell::Open);
                            }
                        }
                    }
                }
                lat += 4.0;
            }
        }

        // ---- pylons around the boss, and a clear landing spot at the start
        for p in PYLONS {
            set(&mut cells, p, Cell::Pylon);
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                set(
                    &mut cells,
                    START + Vec2::new(dx as f32, dy as f32),
                    Cell::Open,
                );
            }
        }

        let mut maze = Maze {
            seed,
            cells,
            axis,
            segments: vec![],
            vents: vec![],
            start: START,
            to_centre: vec![],
        };

        // ---- make sure the boss can be reached; carve through walls if not
        for _ in 0..200 {
            let to_centre = maze.flow(maze.chamber_cells());
            if to_centre[idx(cell_of(maze.start))] != u16::MAX {
                maze.to_centre = to_centre;
                break;
            }
            let from_start = maze.flow([cell_of(maze.start)]);
            if !maze.carve_once(&from_start, &to_centre, &mut rng) {
                break;
            }
        }
        if maze.to_centre.is_empty() {
            maze.to_centre = maze.flow(maze.chamber_cells());
        }

        // ---- vents: hatches spread through the reachable aisles
        let mut cand = vec![];
        for y in 0..N {
            for x in 0..N {
                let c = IVec2::new(x, y);
                let p = cell_pos(c);
                let r = ring(p);
                if maze.cells[idx(c)] == Cell::Open
                    && maze.to_centre[idx(c)] != u16::MAX
                    && (11.0..29.0).contains(&r)
                    && p.distance(START) > 6.0
                {
                    cand.push(p);
                }
            }
        }
        for _ in 0..400 {
            if cand.is_empty() || maze.vents.len() >= 18 {
                break;
            }
            let p = cand[rng.below(cand.len())];
            if maze.vents.iter().all(|v| v.distance(p) > 6.0) {
                maze.vents.push(p);
            }
        }

        maze.segments = maze.build_segments(&mut rng);
        maze
    }

    fn chamber_cells(&self) -> Vec<IVec2> {
        let mut v = vec![];
        for y in 0..N {
            for x in 0..N {
                let c = IVec2::new(x, y);
                if ring(cell_pos(c)) < CHAMBER && !self.solid(c) {
                    v.push(c);
                }
            }
        }
        v
    }

    /// Open one wall that separates the start's region from the rest.
    fn carve_once(&mut self, from_start: &[u16], to_centre: &[u16], rng: &mut Rng) -> bool {
        let mut best: Vec<(IVec2, IVec2)> = vec![];
        let mut fallback: Vec<(IVec2, IVec2)> = vec![];
        for y in 1..N - 1 {
            for x in 1..N - 1 {
                let c = IVec2::new(x, y);
                if self.cells[idx(c)] != Cell::Rack || ring(cell_pos(c)) >= EDGE {
                    continue;
                }
                for d in [IVec2::X, IVec2::Y] {
                    for (a, b) in [(c - d, c + d), (c + d, c - d)] {
                        if !inside(a) || !inside(b) || self.solid(a) || self.solid(b) {
                            continue;
                        }
                        if from_start[idx(a)] == u16::MAX {
                            continue;
                        }
                        if to_centre[idx(b)] != u16::MAX {
                            best.push((c, d));
                        } else if from_start[idx(b)] == u16::MAX {
                            fallback.push((c, d));
                        }
                    }
                }
            }
        }
        // Never carve open the start avenue's corner of the outer ring
        // unless nothing else works: that would make a straight road in.
        let corner = |c: &IVec2| {
            let p = cell_pos(*c);
            p.x > 17.0 && p.y > 17.0 && (p.x - p.y).abs() < 5.0
        };
        let (best_far, best_corner): (Vec<_>, Vec<_>) =
            best.into_iter().partition(|(c, _)| !corner(c));
        let (fall_far, fall_corner): (Vec<_>, Vec<_>) =
            fallback.into_iter().partition(|(c, _)| !corner(c));
        let Some(list) = [&best_far, &fall_far, &best_corner, &fall_corner]
            .into_iter()
            .find(|l| !l.is_empty())
        else {
            return false;
        };
        let (c, d) = list[rng.below(list.len())];
        let side = IVec2::new(d.y, d.x);
        for k in -1..=1 {
            let e = c + side * k;
            if inside(e) && ring(cell_pos(e)) < EDGE && self.cells[idx(e)] == Cell::Rack {
                self.cells[idx(e)] = Cell::Open;
            }
        }
        true
    }

    /// Group rack cells into 1- or 2-cabinet runs, then add the scenery
    /// rows that carry on outside the walls into the dark.
    fn build_segments(&self, rng: &mut Rng) -> Vec<Segment> {
        let mut segs = vec![];
        let mut used = vec![false; self.cells.len()];
        for y in 0..N {
            for x in 0..N {
                let c = IVec2::new(x, y);
                let i = idx(c);
                if used[i] || self.cells[i] != Cell::Rack {
                    continue;
                }
                used[i] = true;
                let ax = self.axis[i];
                let step = if ax == Axis::X { IVec2::X } else { IVec2::Y };
                let nb = c + step;
                let pair = inside(nb)
                    && !used[idx(nb)]
                    && self.cells[idx(nb)] == Cell::Rack
                    && self.axis[idx(nb)] == ax;
                let mut center = cell_pos(c);
                if pair {
                    used[idx(nb)] = true;
                    center += step.as_vec2() * 0.5;
                }
                segs.push(Segment {
                    center,
                    axis: ax,
                    cabinets: if pair { 2 } else { 1 },
                    style: rng.below(4) as u8,
                    tall: rng.chance(0.18),
                });
            }
        }
        // Scenery: the same rows carry on outward, so the hall looks endless.
        let mut lat = 3.5f32;
        while lat < FAR {
            for s in [-1.0f32, 1.0] {
                let v = lat * s;
                // `u` is the centre of the first cabinet, always on a tile.
                let mut u = (HALF + 0.5).max(v.abs() + 2.0);
                while u + 1.0 < FAR {
                    for side in 0..4 {
                        if rng.chance(0.06) {
                            continue;
                        }
                        let (center, axis) = match side {
                            0 => (Vec2::new(u + 0.5, v), Axis::X),
                            1 => (Vec2::new(-u - 0.5, v), Axis::X),
                            2 => (Vec2::new(v, u + 0.5), Axis::Z),
                            _ => (Vec2::new(v, -u - 0.5), Axis::Z),
                        };
                        segs.push(Segment {
                            center,
                            axis,
                            cabinets: 2,
                            style: rng.below(4) as u8,
                            tall: rng.chance(0.18),
                        });
                    }
                    u += 2.0;
                }
            }
            lat += 4.0;
        }
        segs
    }

    pub fn cell(&self, c: IVec2) -> Cell {
        if inside(c) {
            self.cells[idx(c)]
        } else {
            Cell::Rack
        }
    }

    pub fn solid(&self, c: IVec2) -> bool {
        self.cell(c) != Cell::Open
    }

    /// Does a circle of radius `r` at `p` touch a rack or pylon?
    pub fn blocked(&self, p: Vec2, r: f32) -> bool {
        if r <= 0.0 {
            return self.solid(cell_of(p));
        }
        let lo = cell_of(p - Vec2::splat(r));
        let hi = cell_of(p + Vec2::splat(r));
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let c = IVec2::new(x, y);
                if !self.solid(c) {
                    continue;
                }
                let min = Vec2::new(x as f32 - HALF, y as f32 - HALF);
                let q = p.clamp(min, min + Vec2::ONE);
                if q.distance_squared(p) < r * r {
                    return true;
                }
            }
        }
        false
    }

    /// Can you see (or shoot) from `a` to `b` without a rack in the way?
    pub fn clear_line(&self, a: Vec2, b: Vec2) -> bool {
        let mut c = cell_of(a);
        let end = cell_of(b);
        let d = b - a;
        let sx = if d.x > 0.0 {
            1
        } else if d.x < 0.0 {
            -1
        } else {
            0
        };
        let sy = if d.y > 0.0 {
            1
        } else if d.y < 0.0 {
            -1
        } else {
            0
        };
        let o = a + Vec2::splat(HALF);
        let delta = Vec2::new(
            if sx != 0 {
                1.0 / d.x.abs()
            } else {
                f32::INFINITY
            },
            if sy != 0 {
                1.0 / d.y.abs()
            } else {
                f32::INFINITY
            },
        );
        let first = |o: f32, c: i32, s: i32, dl: f32| {
            if s > 0 {
                ((c + 1) as f32 - o) * dl
            } else if s < 0 {
                (o - c as f32) * dl
            } else {
                f32::INFINITY
            }
        };
        let mut t = Vec2::new(first(o.x, c.x, sx, delta.x), first(o.y, c.y, sy, delta.y));
        for _ in 0..(N * 4) {
            if self.solid(c) {
                return false;
            }
            if c == end {
                return true;
            }
            if t.x < t.y {
                if t.x > 1.0 {
                    return true;
                }
                c.x += sx;
                t.x += delta.x;
            } else {
                if t.y > 1.0 {
                    return true;
                }
                c.y += sy;
                t.y += delta.y;
            }
        }
        true
    }

    /// Breadth-first distances (in steps) from the given cells.
    pub fn flow(&self, seeds: impl IntoIterator<Item = IVec2>) -> Vec<u16> {
        let mut dist = vec![u16::MAX; self.cells.len()];
        let mut q = VecDeque::new();
        for s in seeds {
            if inside(s) && !self.solid(s) && dist[idx(s)] == u16::MAX {
                dist[idx(s)] = 0;
                q.push_back(s);
            }
        }
        while let Some(c) = q.pop_front() {
            let d = dist[idx(c)];
            for off in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let nb = c + off;
                if inside(nb) && !self.solid(nb) && dist[idx(nb)] == u16::MAX {
                    dist[idx(nb)] = d + 1;
                    q.push_back(nb);
                }
            }
        }
        dist
    }

    /// Steps to the flow field's source from `p`.
    pub fn steps(&self, field: &[u16], p: Vec2) -> u16 {
        let c = cell_of(p);
        if inside(c) { field[idx(c)] } else { u16::MAX }
    }

    /// Which way to walk from `p` to go downhill on a flow field.
    pub fn downhill(&self, field: &[u16], p: Vec2) -> Option<Vec2> {
        let c = cell_of(p);
        if !inside(c) {
            return None;
        }
        let mut best = (field[idx(c)], None);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nb = c + IVec2::new(dx, dy);
                if !inside(nb) || self.solid(nb) {
                    continue;
                }
                // no cutting corners round the end of a rack
                if dx != 0
                    && dy != 0
                    && (self.solid(c + IVec2::new(dx, 0)) || self.solid(c + IVec2::new(0, dy)))
                {
                    continue;
                }
                let d = field[idx(nb)];
                if d < best.0 {
                    best = (d, Some(nb));
                }
            }
        }
        best.1.map(|nb| (cell_pos(nb) - p).normalize_or_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layout_reaches_the_boss() {
        for seed in 1..60 {
            let m = Maze::generate(seed);
            assert_ne!(
                m.steps(&m.to_centre, m.start),
                u16::MAX,
                "seed {seed}: no way to the boss"
            );
            assert!(
                m.vents.len() >= 8,
                "seed {seed}: only {} vents",
                m.vents.len()
            );
            for v in &m.vents {
                assert_ne!(m.steps(&m.to_centre, *v), u16::MAX);
            }
            // The avenue you start in is blocked, so you have to find a way round.
            assert!(
                m.solid(cell_of(Vec2::new(21.5, 21.5))),
                "seed {seed}: open avenue"
            );
        }
    }

    #[test]
    fn layouts_change_every_fight() {
        let a = Maze::generate(1);
        let b = Maze::generate(2);
        assert_ne!(a.cells, b.cells);
        assert!(a.segments.len() > 600, "{} racks", a.segments.len());
    }

    #[test]
    fn line_of_sight_and_collision() {
        let m = Maze::generate(5);
        // Inside the open chamber everything can see everything.
        assert!(m.clear_line(Vec2::new(-4.0, -3.0), Vec2::new(4.0, 2.0)));
        assert!(m.blocked(Vec2::new(40.0, 0.0), 0.3));
        assert!(!m.blocked(Vec2::ZERO, 0.4));
    }
}

#[cfg(test)]
mod print {
    use super::*;
    /// The boss's raised platform (square radius), drawn as `o`.
    const DAIS: f32 = 7.0;

    /// `cargo test print_maze -- --nocapture` draws a layout as text.
    #[test]
    fn print_maze() {
        let m = Maze::generate(
            std::env::var("MSEED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7),
        );
        let mut s = String::new();
        for y in 0..N {
            for x in 0..N {
                let c = IVec2::new(x, y);
                let p = cell_pos(c);
                let ch = if cell_of(m.start) == c {
                    'S'
                } else if m.vents.iter().any(|v| cell_of(*v) == c) {
                    'v'
                } else {
                    match m.cell(c) {
                        Cell::Rack => '#',
                        Cell::Pylon => 'P',
                        Cell::Open if ring(p) < DAIS => 'o',
                        Cell::Open => '.',
                    }
                };
                s.push(ch);
            }
            s.push('\n');
        }
        println!("{s}");
        println!("steps from start: {}", m.steps(&m.to_centre, m.start));
    }
}
