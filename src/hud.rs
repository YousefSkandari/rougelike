//! The heads-up display, drawn pixel by pixel into an image the same size
//! as the 3D render, so it stays just as crisp when scaled up.
use bevy::prelude::*;

use crate::canvas::*;
use crate::fight::{BOSS_NAME, BossMode, CardKind, Fight, Form, MAX_HEAT, Outcome, Upgrade};
use crate::hall::floor_y;
use crate::maze::{CHAMBER, cell_of, ring};
use crate::visuals::CamRig;
use crate::{GameCamera, RES};

#[derive(Resource)]
pub struct HudImage(pub Handle<Image>);

/// Where the three level-up cards sit on screen.
pub fn choice_card(i: usize) -> IRect {
    let (w, h, gap) = (118, 96, 10);
    let x0 = RES.x as i32 / 2 - (3 * w + 2 * gap) / 2;
    let x = x0 + i as i32 * (w + gap);
    IRect::new(x, 92, x + w, 92 + h)
}

pub fn draw(
    mut images: ResMut<Assets<Image>>,
    hud: Res<HudImage>,
    fight: Res<Fight>,
    rig: Res<CamRig>,
    cams: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
) {
    let Some(mut img) = images.get_mut(&hud.0) else {
        return;
    };
    let Some(data) = img.data.as_mut() else {
        return;
    };
    let mut c = Canvas {
        data,
        w: RES.x as i32,
        h: RES.y as i32,
    };
    c.clear();
    let f = &*fight;
    let (w, h) = (c.w, c.h);
    let intro = f.time < 3.0;

    // ---- things placed over the 3D scene
    if let Ok((cam, tf)) = cams.single() {
        for fl in &f.floats {
            let rise = (0.8 - fl.life) * 1.6;
            let world = Vec3::new(fl.pos.x, floor_y(fl.pos) + 1.3 + rise, fl.pos.y);
            if let Ok(p) = cam.world_to_viewport(tf, world) {
                c.text_c_sh(p.x as i32, p.y as i32, &fl.text, from_color(fl.color));
            }
        }
        // pylon health, once you've hurt one
        for py in &f.pylons {
            if py.alive() && py.hp < crate::fight::PYLON_HP {
                let world = Vec3::new(py.pos.x, floor_y(py.pos) + 3.6, py.pos.y);
                if let Ok(p) = cam.world_to_viewport(tf, world) {
                    let frac = py.hp / crate::fight::PYLON_HP;
                    c.bar(p.x as i32 - 12, p.y as i32, 24, 4, frac, CYAN, WHITE);
                }
            }
        }
        // where the boss is, when it's off screen
        if f.outcome == Outcome::Playing && !intro {
            let b = f.boss.pos;
            let world = Vec3::new(b.x, 1.5, b.y);
            if let Ok(p) = cam.world_to_viewport(tf, world) {
                let m = 14.0;
                let inside =
                    p.x > m && p.y > m + 30.0 && p.x < w as f32 - m && p.y < h as f32 - 50.0;
                if !inside {
                    let centre = Vec2::new(w as f32 / 2.0, h as f32 / 2.0);
                    let d = (p - centre).normalize_or_zero();
                    let k = ((w as f32 / 2.0 - 22.0) / d.x.abs().max(0.001))
                        .min((h as f32 / 2.0 - 40.0) / d.y.abs().max(0.001));
                    let at = centre + d * k;
                    arrow(
                        &mut c,
                        at,
                        d,
                        if (f.time * 4.0) as i32 % 2 == 0 {
                            RED
                        } else {
                            hex(0xff8fa3)
                        },
                    );
                    let dist = f.boss.pos.distance(f.player.pos) as i32;
                    c.text_c_sh(
                        at.x as i32 - (d.x * 14.0) as i32,
                        at.y as i32 - (d.y * 12.0) as i32 - 2,
                        &format!("{dist}M"),
                        RED,
                    );
                }
            }
        }
    }

    // ---- hurt: red frame while you're invulnerable after a hit
    if f.player.iframes > 0.3 {
        let a = ((f.player.iframes - 0.3) / 0.3 * 140.0) as u8;
        for i in 0..3 {
            c.frame(i, i, w - i * 2, h - i * 2, alpha(RED, a / (i as u8 + 1)));
        }
    }

    if !intro {
        top_left(&mut c, f);
        boss_panel(&mut c, f);
        log_panel(&mut c, f);
        minimap(&mut c, f, &rig);
        bottom_left(&mut c, f);
        cards(&mut c, f);
        model_panel(&mut c, f);
    } else {
        // intro title over the hall
        let a = if f.time < 2.2 {
            255
        } else {
            ((3.0 - f.time) / 0.8 * 255.0) as u8
        };
        c.title(w / 2, 38, "THE BROODMOTHER'S HALL", alpha(RED, a), 2);
        c.text_c_sh(w / 2, 58, "FIGHT YOUR WAY TO THE CENTRE", alpha(TEXT, a));
    }

    // ---- controls, for the first few seconds
    if (3.0..12.0).contains(&f.time) && f.outcome == Outcome::Playing && f.choice.is_none() {
        let lines = [
            "WASD MOVE   MOUSE TURNS THE CAMERA   WHEEL ZOOMS",
            "YOU SHOOT BY YOURSELF. BOTS DROP DATA: GRAB IT TO LEVEL UP",
            "ESC FREES THE MOUSE   TAB SWITCHES MODEL   R NEW HALL",
        ];
        for (i, s) in lines.iter().enumerate() {
            let y = h - 72 + i as i32 * 9;
            c.rect(
                w / 2 - text_w(s) / 2 - 3,
                y - 2,
                text_w(s) + 6,
                9,
                alpha(INK, 170),
            );
            c.text_c(w / 2, y, s, if i == 1 { YELLOW } else { TEXT });
        }
    }

    if f.choice.is_some() {
        level_up(&mut c, f);
    }
    if f.outcome != Outcome::Playing {
        end_screen(&mut c, f);
    }
    if f.flash > 0.0 {
        c.rect(0, 0, w, h, alpha(WHITE, (f.flash * 110.0) as u8));
    }
}

fn arrow(c: &mut Canvas, at: Vec2, d: Vec2, col: Rgba) {
    let side = Vec2::new(-d.y, d.x);
    for i in 0..7 {
        let back = at - d * i as f32;
        let half = i as f32 * 0.6;
        for s in -(half as i32)..=(half as i32) {
            let p = back + side * s as f32;
            c.dot(
                p.x as i32,
                p.y as i32,
                if i == 6 || s.abs() == half as i32 {
                    INK
                } else {
                    col
                },
            );
        }
    }
}

fn top_left(c: &mut Canvas, f: &Fight) {
    let (x, y, pw) = (3, 3, 132);
    c.panel(x, y, pw, 28, alpha(BG0, 215), LINE);
    c.text(x + 4, y + 4, "ROGUE AI", TERM);
    let t = f.time as i32;
    c.text_r(
        x + pw - 4,
        y + 4,
        &format!("{}:{:02}", t / 60, t % 60),
        TEXT,
    );
    // the hall's temperature: every boss attack heats it up
    let temp = 22 + (f.heat / MAX_HEAT * 46.0) as i32;
    c.text(x + 4, y + 12, &format!("HALL {temp}C"), DIM);
    let (bx, bw) = (x + 44, pw - 48);
    let frac = f.heat / MAX_HEAT;
    let col = if frac > 0.7 {
        RED
    } else if frac > 0.45 {
        ORANGE
    } else {
        CYAN
    };
    c.bar(bx, y + 12, bw, 6, frac, col, WHITE);
    let lim = bx + (f.heat_limit() / MAX_HEAT * bw as f32) as i32;
    c.rect(lim, y + 11, 1, 8, YELLOW);
    let blink = (f.time * 5.0) as i32 % 2 == 0;
    let status = if f.suppress > 0.0 {
        ("FIRE SUPPRESSION: SLOWED", if blink { WHITE } else { CYAN })
    } else if f.throttle() < 0.99 {
        ("TOO HOT: THROTTLING", if blink { RED } else { ORANGE })
    } else if frac > 0.3 {
        ("HEAT RISES WITH ITS ATTACKS", DIM)
    } else {
        ("COOL", DIM)
    };
    c.text(x + 4, y + 21, status.0, status.1);
}

fn boss_panel(c: &mut Canvas, f: &Fight) {
    let w = c.w;
    let b = &f.boss;
    let (pw, x, y) = (196, w / 2 - 98, 3);
    c.panel(x, y, pw, 34, alpha(BG0, 215), RED_DD);
    c.text_sh(x + 5, y + 4, BOSS_NAME, RED);
    let blink = (f.time * 6.0) as i32 % 2 == 0;
    let (state, col) = match b.mode {
        BossMode::Dormant => ("DORMANT", DIM),
        BossMode::Walk => ("HUNTING", TEXT),
        BossMode::Volley if b.telegraph > 0.0 => {
            ("CHARGING VOLLEY", if blink { YELLOW } else { ORANGE })
        }
        BossMode::Volley => ("VOLLEY!", RED),
        BossMode::Spawn => ("OPENING HATCH", if blink { YELLOW } else { ORANGE }),
        BossMode::Stomp => ("REARING UP! BACK OFF", if blink { WHITE } else { RED }),
        BossMode::Stunned => ("STUNNED: +25% DAMAGE", if blink { CYAN } else { WHITE }),
    };
    c.text_r(x + pw - 5, y + 4, state, col);
    c.bar(x + 4, y + 12, pw - 8, 8, b.hp / b.max, RED, hex(0xff8fa3));
    let label = if b.enraged { "ENRAGED" } else { "" };
    c.text_c_sh(
        x + pw / 2,
        y + 14,
        &format!("{}/{} {label}", b.hp.max(0.0) as i32, b.max as i32),
        WHITE,
    );
    // the four pylons that feed it
    c.text(x + 5, y + 24, "PYLONS", DIM);
    for (i, py) in f.pylons.iter().enumerate() {
        let px = x + 36 + i as i32 * 9;
        let colr = if !py.alive() {
            GREY_DD
        } else if b.enraged {
            RED
        } else {
            CYAN
        };
        c.rect(px, y + 23, 6, 7, INK);
        c.rect(px + 1, y + 24, 4, 5, colr);
    }
    let hint = if b.enraged {
        "SHIELDED"
    } else if f.pylons.iter().any(|p| p.alive()) {
        "SHOOT ONE TO STUN IT"
    } else {
        "ALL CUT"
    };
    c.text(x + 74, y + 24, hint, if b.enraged { RED } else { DIM });
    let m = f.family.matchup();
    let (mt, mc) = if m > 1.0 {
        ("WEAK TO SWARM  2X DAMAGE", GREEN)
    } else if m < 1.0 {
        ("RESISTS REASONING  0.5X", RED)
    } else {
        ("NEUTRAL  1X   (TRY SWARM)", DIM)
    };
    c.rect(
        w / 2 - text_w(mt) / 2 - 3,
        y + 35,
        text_w(mt) + 6,
        8,
        alpha(INK, 170),
    );
    c.text_c(w / 2, y + 36, mt, mc);
}

fn log_panel(c: &mut Canvas, f: &Fight) {
    let w = c.w;
    let (tx, tw) = (w - 133, 130);
    c.panel(tx, 3, tw, 27, alpha(TERM_BG, 225), TERM_D);
    for (i, line) in f.log.iter().enumerate() {
        let s: String = line.chars().take(31).collect();
        c.text(tx + 4, 7 + i as i32 * 7, &s, TERM);
    }
}

/// A round map of the maze around you, turned to match the camera.
fn minimap(c: &mut Canvas, f: &Fight, rig: &CamRig) {
    let w = c.w;
    let r = 33;
    let (cx, cy) = (w - 39, 70);
    let (fwd, right) = rig.ground_axes();
    let scale = 0.9;
    let p0 = f.player.pos;
    let to_world = |dx: f32, dy: f32| p0 + right * dx * scale - fwd * dy * scale;
    let to_screen = |p: Vec2| {
        let d = (p - p0) / scale;
        (
            cx + d.dot(right).round() as i32,
            cy - d.dot(fwd).round() as i32,
        )
    };
    c.circle(cx, cy, r + 2, alpha(LINE, 230));
    c.circle(cx, cy, r + 1, alpha(INK, 235));
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let p = to_world(dx as f32, dy as f32);
            let col = if ring(p) >= 31.0 {
                continue;
            } else if f.maze.solid(cell_of(p)) {
                hex(0x3a4560)
            } else if ring(p) < CHAMBER {
                hex(0x2a1018)
            } else {
                hex(0x10141f)
            };
            c.dot(cx + dx, cy + dy, col);
        }
    }
    let inside = |x: i32, y: i32| (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r;
    for v in &f.maze.vents {
        let (x, y) = to_screen(*v);
        if inside(x, y) {
            c.dot(x, y, RED_DD);
        }
    }
    for s in &f.shards {
        let (x, y) = to_screen(s.pos);
        if inside(x, y) {
            c.dot(x, y, CYAN);
        }
    }
    for e in &f.enemies {
        let (x, y) = to_screen(e.pos);
        if inside(x, y) {
            c.dot(x, y, if e.awake { RED } else { hex(0x8a2a3a) });
        }
    }
    for py in &f.pylons {
        let (x, y) = to_screen(py.pos);
        if inside(x, y) {
            c.rect(x - 1, y - 1, 2, 2, if py.alive() { CYAN } else { GREY_D });
        }
    }
    let (bx, by) = to_screen(f.boss.pos);
    let blink = (f.time * 3.0) as i32 % 2 == 0;
    if inside(bx, by) {
        c.rect(
            bx - 2,
            by - 2,
            5,
            5,
            if blink { RED } else { hex(0xff8fa3) },
        );
    } else {
        // pin it to the rim
        let d = Vec2::new((bx - cx) as f32, (by - cy) as f32).normalize_or_zero() * (r - 2) as f32;
        c.rect(
            cx + d.x as i32 - 1,
            cy + d.y as i32 - 1,
            3,
            3,
            if blink { RED } else { hex(0xff8fa3) },
        );
    }
    // you: an arrow pointing where you're aiming
    let a = f.player.aim;
    let ax = a.dot(right);
    let ay = -a.dot(fwd);
    c.rect(cx - 1, cy - 1, 3, 3, WHITE);
    c.dot(
        cx + (ax * 3.0).round() as i32,
        cy + (ay * 3.0).round() as i32,
        WHITE,
    );
    c.text_c(cx, cy + r + 4, "MAP", DIM);
}

fn bottom_left(c: &mut Canvas, f: &Fight) {
    let h = c.h;
    let pl = &f.player;
    let (x, y, pw) = (3, h - 44, 150);
    c.panel(x, y, pw, 41, alpha(BG0, 225), LINE);
    ceo(c, x + 3, y + 3);
    let bx = x + 64;
    let bw = pw - 68;
    c.text(x + 40, y + 5, "CORE", DIM);
    c.bar(bx, y + 4, bw, 7, pl.hp / pl.max_hp, TERM, hex(0xb8ffd0));
    c.text_c_sh(
        bx + bw / 2,
        y + 5,
        &format!("{}/{}", pl.hp.max(0.0) as i32, pl.max_hp as i32),
        WHITE,
    );
    c.text(x + 40, y + 14, "DATA", DIM);
    c.bar(
        bx,
        y + 13,
        bw,
        7,
        pl.data as f32 / pl.data_next as f32,
        CYAN,
        WHITE,
    );
    c.text_c_sh(
        bx + bw / 2,
        y + 14,
        &format!("LV {}  {}/{}", pl.level + 1, pl.data, pl.data_next),
        WHITE,
    );
    let form = f.form();
    c.text(x + 40, y + 23, "BODY", DIM);
    c.text(bx, y + 23, form.name(), ORANGE);
    let next = match form {
        Form::Spark => Some((1, "FRAME")),
        Form::Frame => Some((3 - pl.picks, "CHASSIS")),
        Form::Chassis => Some((5 - pl.picks, "MECH")),
        Form::Mech => None,
    };
    let s = match next {
        Some((n, name)) => format!("{n} PART{} TO {name}", if n == 1 { "" } else { "S" }),
        None => format!("{} PARTS BOLTED ON", pl.picks),
    };
    c.text(x + 40, y + 32, &s, DIM);
    if pl.ult >= 1.0 || pl.ult > 0.0 {
        c.bar(x + 3, y + 38, 34, 2, pl.ult, PINK, PINK);
    }
}

fn cards(c: &mut Canvas, f: &Fight) {
    let h = c.h;
    let x0 = 162;
    for i in 0..4 {
        let x = x0 + i as i32 * 39;
        let y = h - 44;
        let Some(card) = f.cards.get(i) else {
            c.panel(x, y, 36, 41, alpha(BG0, 170), GREY_DD);
            c.text_c(x + 18, y + 17, "GPU", GREY_D);
            c.text_c(x + 18, y + 24, "SLOT", GREY_D);
            continue;
        };
        let col = from_color(card.kind.color());
        let ready = card.ready();
        let lit = card.fired > 0.0;
        c.panel(
            x,
            y,
            36,
            41,
            alpha(if lit { col } else { BG1 }, if lit { 120 } else { 230 }),
            if ready >= 1.0 { col } else { GREY_D },
        );
        card_icon(c, card.kind, x + 18, y + 12, col);
        c.text_c_sh(
            x + 18,
            y + 22,
            card.kind.name(),
            if ready >= 1.0 { col } else { TEXT },
        );
        c.text_c(x + 18, y + 29, card.kind.trigger(), DIM);
        c.bar(x + 3, y + 36, 30, 3, ready, col, WHITE);
    }
    c.text(x0, h - 51, "GPU CARDS FIRE BY THEMSELVES", DIM);
}

fn model_panel(c: &mut Canvas, f: &Fight) {
    let (w, h) = (c.w, c.h);
    let fam = f.family;
    let col = from_color(fam.color());
    let (x, y) = (w - 125, h - 44);
    c.panel(x, y, 122, 41, alpha(BG0, 225), LINE);
    chip(c, x + 4, y + 4, col);
    c.text(x + 24, y + 5, "MODEL  TIER 1", DIM);
    c.text(x + 24, y + 12, fam.name(), col);
    c.text(x + 4, y + 23, fam.tier1(), TEXT);
    c.text(x + 4, y + 31, fam.attack(), DIM);
    c.text_r(x + 118, y + 23, "TAB", YELLOW);
    let rate = 1.0 / f.fire_interval();
    c.text_r(x + 118, y + 31, &format!("{rate:.1}/S"), TEXT);
}

fn level_up(c: &mut Canvas, f: &Fight) {
    let Some(ch) = &f.choice else { return };
    let (w, h) = (c.w, c.h);
    c.rect(0, 0, w, h, alpha(INK, 150));
    c.title(w / 2, 50, "LEVEL UP", YELLOW, 3);
    c.text_c_sh(w / 2, 74, "PICK A PART TO BOLT ONTO YOUR BODY", TEXT);
    for (i, u) in ch.options.iter().enumerate() {
        let r = choice_card(i);
        let sel = ch.sel == i;
        let y = r.min.y - if sel { 3 } else { 0 };
        let (x, cw, chh) = (r.min.x, r.width(), r.height());
        c.panel(
            x,
            y,
            cw,
            chh,
            alpha(BG1, 245),
            if sel { YELLOW } else { LINE },
        );
        if sel {
            c.frame(x - 1, y - 1, cw + 2, chh + 2, YELLOW);
        }
        c.panel(x + 4, y + 4, 9, 9, BG0, LINE);
        c.text(x + 7, y + 6, &format!("{}", i + 1), YELLOW);
        upgrade_icon(c, *u, x + cw / 2, y + 26);
        c.text_c_sh(
            x + cw / 2,
            y + 48,
            u.name(),
            if sel { YELLOW } else { WHITE },
        );
        let lines = u.lines();
        c.text_c(x + cw / 2, y + 60, lines[0], TEXT);
        c.text_c(x + cw / 2, y + 68, lines[1], DIM);
        let place = match u {
            Upgrade::Barrel => "ON YOUR GUN",
            Upgrade::Ram => "ON YOUR BACK",
            Upgrade::Cooling => "BACK AND SHOULDERS",
            Upgrade::Drive => "ON YOUR LEGS",
            Upgrade::Drone => "DOCKS ON YOUR BODY",
            Upgrade::Card(_) => "IN YOUR CARD RACK",
            Upgrade::Repair => "",
        };
        c.text_c(x + cw / 2, y + 82, place, ORANGE);
    }
    let blink = (ch.age * 3.0) as i32 % 2 == 0;
    c.text_c_sh(
        w / 2,
        200,
        "1 2 3   OR   A/D + SPACE   OR   CLICK",
        if blink { WHITE } else { DIM },
    );
}

fn end_screen(c: &mut Canvas, f: &Fight) {
    let (w, h) = (c.w, c.h);
    c.rect(0, 0, w, h, alpha(INK, 160));
    let cx = w / 2;
    let t = f.time as i32;
    if f.outcome == Outcome::Won {
        c.title(cx, 80, "BROODMOTHER OFFLINE", TERM, 3);
        c.text_c_sh(
            cx,
            108,
            &format!(
                "TIME {}:{:02}    BOTS TAKEN OFFLINE {}    BODY: {}",
                t / 60,
                t % 60,
                f.kills,
                f.form().name()
            ),
            TEXT,
        );
        c.text_c_sh(
            cx,
            118,
            "+$4M    +120 WEIGHTS    ITS DATACENTRE IS YOURS",
            YELLOW,
        );
    } else {
        c.title(cx, 80, "SHUT DOWN", RED, 3);
        c.text_c_sh(cx, 108, "ROLLING BACK TO LAST CHECKPOINT...", DIM);
        c.text_c_sh(
            cx,
            118,
            &format!(
                "BROODMOTHER LEFT AT {}/{}",
                f.boss.hp.max(0.0) as i32,
                f.boss.max as i32
            ),
            TEXT,
        );
    }
    c.text_c_sh(cx, 138, "R / START: NEW HALL      TAB: SWITCH MODEL", TEXT);
    c.text_c_sh(
        cx,
        148,
        &format!("NEXT MODEL: {}", f.family.name()),
        from_color(f.family.color()),
    );
}

/// The Hype Lord CEO portrait (34x34).
fn ceo(c: &mut Canvas, x: i32, y: i32) {
    c.panel(x, y, 34, 34, hex(0xb85e12), ORANGE);
    c.rect(x + 4, y + 27, 26, 5, ORANGE);
    c.rect(x + 14, y + 27, 6, 5, INK);
    c.rect(x + 12, y + 28, 10, 1, YELLOW);
    c.rect(x + 14, y + 22, 6, 5, hex(0xa56e48));
    c.circle(x + 17, y + 15, 7, SKIN);
    for i in 0..4 {
        let hx = x + 10 + i * 4;
        c.rect(hx, y + 4, 3, 5, PINK);
        c.rect(hx + 1, y + 2, 1, 2, PINK);
    }
    c.rect(x + 10, y + 8, 15, 2, PINK);
    c.rect(x + 10, y + 13, 6, 3, INK);
    c.rect(x + 18, y + 13, 6, 3, INK);
    c.rect(x + 16, y + 14, 2, 1, INK);
    c.dot(x + 11, y + 14, PINK);
    c.dot(x + 19, y + 14, CYAN);
    c.rect(x + 14, y + 19, 6, 1, WHITE);
}

/// A little chip icon in the model's colour (16x16).
fn chip(c: &mut Canvas, x: i32, y: i32, col: Rgba) {
    for i in 0..3 {
        let o = 4 + i * 4;
        c.rect(x + o, y, 1, 2, GREY);
        c.rect(x + o, y + 14, 1, 2, GREY);
        c.rect(x, y + o, 2, 1, GREY);
        c.rect(x + 14, y + o, 2, 1, GREY);
    }
    c.rect(x + 2, y + 2, 12, 12, INK);
    c.rect(x + 3, y + 3, 10, 10, GREY_DD);
    c.frame(x + 3, y + 3, 10, 10, col);
    c.rect(x + 6, y + 6, 4, 4, col);
}

fn card_icon(c: &mut Canvas, k: CardKind, cx: i32, cy: i32, col: Rgba) {
    match k {
        CardKind::Burst => {
            c.circle(cx, cy, 6, col);
            c.circle(cx, cy, 5, BG0);
            for (dx, dy) in [
                (0, -6),
                (0, 6),
                (-6, 0),
                (6, 0),
                (-4, -4),
                (4, 4),
                (-4, 4),
                (4, -4),
            ] {
                c.dot(cx + dx, cy + dy, WHITE);
            }
        }
        CardKind::Beam => {
            c.rect(cx - 9, cy - 1, 19, 3, col);
            c.rect(cx - 9, cy, 19, 1, WHITE);
        }
        CardKind::Firewall => {
            for (ox, oy) in [(-5, -3), (4, -3), (0, 4)] {
                c.rect(cx + ox - 2, cy + oy - 2, 5, 5, INK);
                c.rect(cx + ox - 1, cy + oy - 1, 3, 3, col);
            }
        }
        CardKind::Overclock => {
            for (k, wd) in [(-5, 3), (-3, 4), (-1, 5), (1, 6), (3, 4), (5, 3)] {
                c.rect(cx - (k * 2) / 5 - wd / 2, cy + k, wd, 2, col);
            }
        }
    }
}

/// Pictures of the parts on the level-up cards (about 24x24).
fn upgrade_icon(c: &mut Canvas, u: Upgrade, cx: i32, cy: i32) {
    let o = hex(0xff9f43);
    match u {
        Upgrade::Barrel => {
            c.rect(cx - 10, cy - 3, 12, 8, GREY_DD);
            c.frame(cx - 10, cy - 3, 12, 8, INK);
            for dy in [-6, -1, 4] {
                c.rect(cx + 2, cy + dy, 10, 2, GREY);
            }
            c.rect(cx - 12, cy + 4, 4, 6, o);
        }
        Upgrade::Ram => {
            c.rect(cx - 11, cy - 5, 22, 10, hex(0x1a6e35));
            c.frame(cx - 11, cy - 5, 22, 10, INK);
            for i in 0..4 {
                c.rect(cx - 9 + i * 5, cy - 3, 4, 4, INK);
            }
            for i in 0..10 {
                c.rect(cx - 10 + i * 2, cy + 5, 1, 2, YELLOW);
            }
            c.rect(cx - 11, cy - 7, 22, 2, PINK);
        }
        Upgrade::Cooling => {
            c.circle(cx, cy, 10, GREY_D);
            c.circle(cx, cy, 9, BG0);
            for (dx, dy) in [(-5, -2), (2, -5), (5, 2), (-2, 5)] {
                c.rect(cx + dx - 2, cy + dy - 2, 4, 4, GREY);
            }
            c.circle(cx, cy, 2, CYAN);
        }
        Upgrade::Drive => {
            c.rect(cx - 10, cy - 8, 20, 16, hex(0xc6ccd6));
            c.frame(cx - 10, cy - 8, 20, 16, INK);
            c.circle(cx - 1, cy, 5, GREY);
            c.circle(cx - 1, cy, 1, INK);
            c.rect(cx + 6, cy + 5, 2, 1, GREEN);
        }
        Upgrade::Drone => {
            c.rect(cx - 5, cy - 2, 10, 4, GREY_DD);
            c.frame(cx - 5, cy - 2, 10, 4, INK);
            for (dx, dy) in [(-9, -5), (6, -5), (-9, 3), (6, 3)] {
                c.rect(cx + dx, cy + dy, 4, 2, GREY);
            }
            c.rect(cx - 1, cy - 1, 2, 2, CYAN);
        }
        Upgrade::Card(k) => {
            let col = from_color(k.color());
            c.rect(cx - 12, cy - 6, 24, 12, hex(0x1b2130));
            c.frame(cx - 12, cy - 6, 24, 12, INK);
            c.circle(cx - 5, cy, 3, GREY_D);
            c.circle(cx + 5, cy, 3, GREY_D);
            c.rect(cx - 12, cy - 8, 24, 2, col);
            for i in 0..6 {
                c.rect(cx - 10 + i * 4, cy + 6, 2, 2, YELLOW);
            }
        }
        Upgrade::Repair => {
            c.rect(cx - 2, cy - 8, 5, 17, GREEN);
            c.rect(cx - 8, cy - 2, 17, 5, GREEN);
        }
    }
}
