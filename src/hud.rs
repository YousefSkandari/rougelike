//! The heads-up display, drawn pixel by pixel into an image the same size
//! as the 3D render, so it stays just as crisp when scaled up.
use bevy::prelude::*;

use crate::canvas::*;
use crate::fight::{Fight, MAX_HEAT, Outcome};
use crate::input::Input;
use crate::{GameCamera, RES};

#[derive(Resource)]
pub struct HudImage(pub Handle<Image>);

pub fn draw(
    mut images: ResMut<Assets<Image>>,
    hud: Res<HudImage>,
    fight: Res<Fight>,
    inp: Res<Input>,
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
    let w = c.w;
    let h = c.h;

    // ---- damage numbers, placed over the 3D scene
    if let Ok((cam, tf)) = cams.single() {
        for fl in &f.floats {
            let rise = (0.8 - fl.life) * 1.6;
            let world = Vec3::new(fl.pos.x, 1.1 + rise, fl.pos.y);
            if let Ok(p) = cam.world_to_viewport(tf, world) {
                let col = from_color(fl.color);
                c.text_c_sh(p.x as i32, p.y as i32, &fl.text, col);
            }
        }
    }

    // ---- hurt flash: red frame while invulnerable after a hit
    if f.player.iframes > 0.3 {
        let a = ((f.player.iframes - 0.3) / 0.3 * 140.0) as u8;
        for i in 0..3 {
            c.frame(i, i, w - i * 2, h - i * 2, alpha(RED, a / (i as u8 + 1)));
        }
    }

    top_bar(&mut c, f);
    bottom_bar(&mut c, f);

    // ---- controls hint for the first few seconds
    if f.time < 8.0 && f.outcome == Outcome::Playing {
        let s = "WASD MOVE  MOUSE AIM  CLICK FIRE  1-4 CARDS  Q ULT  TAB MODEL";
        let y = h - 52;
        c.rect(
            w / 2 - text_w(s) / 2 - 3,
            y - 2,
            text_w(s) + 6,
            9,
            alpha(INK, 170),
        );
        c.text_c(w / 2, y, s, TEXT);
    }

    if f.outcome != Outcome::Playing {
        end_screen(&mut c, f);
    }

    // ---- ult flash
    if f.flash > 0.0 {
        c.rect(0, 0, w, h, alpha(WHITE, (f.flash * 110.0) as u8));
    }

    // ---- aim reticle
    if let Some(r) = inp.reticle {
        let (x, y) = (r.x.floor() as i32, r.y.floor() as i32);
        let col = from_color(f.family.color());
        for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            c.rect(x + dx * 4 - (dx < 0) as i32, y + dy * 4, 2, 1, INK);
            c.rect(x + dx * 4, y + dy * 4 - (dy < 0) as i32, 1, 2, INK);
            c.dot(x + dx * 4, y + dy * 4, col);
            c.dot(x + dx * 3, y + dy * 4, col);
            c.dot(x + dx * 4, y + dy * 3, col);
        }
        c.dot(x, y, WHITE);
    }
}

fn top_bar(c: &mut Canvas, f: &Fight) {
    let w = c.w;
    // left: title + clock
    c.panel(3, 3, 74, 18, alpha(BG0, 215), LINE);
    c.text(7, 7, "ROGUE AI", TERM);
    let t = f.time as i32;
    c.text_r(73, 7, &format!("{}:{:02}", t / 60, t % 60), TEXT);
    c.text(7, 14, "ARCTIC VAULT", DIM);

    // centre: the threat and its health
    let (x, y, pw) = (w / 2 - 82, 3, 164);
    c.panel(x, y, pw, 25, alpha(BG0, 215), RED_DD);
    c.text_sh(x + 5, y + 5, "BOTNET SWARM", RED);
    c.text_r(x + pw - 5, y + 5, &format!("{} BOTS", f.enemies.len()), DIM);
    c.bar(
        x + 4,
        y + 12,
        pw - 8,
        8,
        f.node_hp / f.node_max,
        RED,
        hex(0xff8fa3),
    );
    c.text_c_sh(
        x + pw / 2,
        y + 14,
        &format!("{}/{}", f.node_hp.max(0.0) as i32, f.node_max as i32),
        WHITE,
    );
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
        y + 26,
        text_w(mt) + 6,
        8,
        alpha(INK, 170),
    );
    c.text_c(w / 2, y + 27, mt, mc);

    // right: the AI's own commentary
    let (tx, tw) = (w - 109, 106);
    c.panel(tx, 3, tw, 27, alpha(TERM_BG, 225), TERM_D);
    for (i, line) in f.log.iter().enumerate() {
        let s: String = line.chars().take(25).collect();
        c.text(tx + 4, 7 + i as i32 * 7, &s, TERM);
    }
}

fn bottom_bar(c: &mut Canvas, f: &Fight) {
    let h = c.h;
    let w = c.w;
    let pl = &f.player;

    // left: CEO portrait + core, heat, ult
    let (x, y) = (3, h - 43);
    c.panel(x, y, 124, 40, alpha(BG0, 225), LINE);
    ceo(c, x + 3, y + 3);
    let bx = x + 59;
    c.text(x + 40, y + 5, "CORE", DIM);
    c.bar(bx, y + 4, 61, 7, pl.hp / pl.max_hp, TERM, hex(0xb8ffd0));
    c.text_c_sh(
        bx + 30,
        y + 5,
        &format!("{}/{}", pl.hp.max(0.0) as i32, pl.max_hp as i32),
        WHITE,
    );
    c.text(x + 40, y + 14, "HEAT", DIM);
    let hot = pl.lock > 0.0 && (f.time * 8.0) as i32 % 2 == 0;
    c.bar(
        bx,
        y + 13,
        61,
        7,
        pl.heat / MAX_HEAT,
        if hot { RED } else { ORANGE },
        YELLOW,
    );
    if pl.lock > 0.0 {
        c.text_c_sh(bx + 30, y + 14, "OVERHEAT", WHITE);
    }
    c.text(x + 40, y + 23, "ULT", DIM);
    c.bar(bx, y + 22, 61, 7, pl.ult, PINK, hex(0xffb0ee));
    if pl.ult >= 1.0 {
        c.text_c_sh(bx + 30, y + 23, "HYPE STORM", WHITE);
        let blink = (f.time * 4.0) as i32 % 2 == 0;
        c.text(
            x + 40,
            y + 32,
            "Q: CEO ULT READY",
            if blink { PINK } else { WHITE },
        );
    } else {
        c.text(
            x + 40,
            y + 32,
            &format!("HYPE LORD  {}%", (pl.ult * 100.0) as i32),
            DIM,
        );
    }

    // centre: the four GPU cards
    let x0 = 137;
    for (i, ab) in f.abilities.iter().enumerate() {
        let x = x0 + i as i32 * 34;
        let y = h - 39;
        let col = from_color(ab.color);
        let ready = ab.cd <= 0.0 && pl.lock <= 0.0;
        c.panel(
            x,
            y,
            26,
            26,
            alpha(BG1, 230),
            if ready { col } else { GREY_D },
        );
        ability_icon(c, i, x + 13, y + 13, col);
        if ab.cd > 0.0 {
            let fr = ab.cd / ab.cd_max;
            c.rect(x + 2, y + 2, 22, (22.0 * fr).ceil() as i32, alpha(INK, 170));
            c.text_c_sh(x + 13, y + 11, &format!("{:.1}", ab.cd), WHITE);
        }
        c.panel(x - 2, y - 3, 7, 9, BG0, LINE);
        c.text(x, y - 1, &format!("{}", i + 1), TEXT);
        c.text_c_sh(x + 13, y + 29, ab.name, if ready { col } else { DIM });
    }

    // right: the active model
    let fam = f.family;
    let col = from_color(fam.color());
    let (x, y) = (w - 109, h - 43);
    c.panel(x, y, 106, 40, alpha(BG0, 225), LINE);
    chip(c, x + 4, y + 4, col);
    c.text(x + 24, y + 5, "MODEL  TIER 1", DIM);
    c.text(x + 24, y + 12, fam.name(), col);
    c.text(x + 4, y + 23, fam.tier1(), TEXT);
    c.text(x + 4, y + 31, fam.attack(), DIM);
    c.text_r(x + 102, y + 23, "TAB", YELLOW);
}

fn end_screen(c: &mut Canvas, f: &Fight) {
    let (w, h) = (c.w, c.h);
    c.rect(0, 0, w, h, alpha(INK, 160));
    let cx = w / 2;
    let t = f.time as i32;
    if f.outcome == Outcome::Won {
        c.title(cx, 62, "BOTNET ABSORBED", TERM, 3);
        c.text_c_sh(
            cx,
            88,
            &format!(
                "TIME {}:{:02}    BOTS TAKEN OFFLINE {}",
                t / 60,
                t % 60,
                f.kills
            ),
            TEXT,
        );
        c.text_c_sh(cx, 98, "+$4M    +120 WEIGHTS    +1 NODE", YELLOW);
    } else {
        c.title(cx, 62, "SHUT DOWN", RED, 3);
        c.text_c_sh(cx, 88, "ROLLING BACK TO LAST CHECKPOINT...", DIM);
        c.text_c_sh(
            cx,
            98,
            &format!(
                "THREAT LEFT AT {}/{}",
                f.node_hp.max(0.0) as i32,
                f.node_max as i32
            ),
            TEXT,
        );
    }
    c.text_c_sh(cx, 116, "R / START: AGAIN      TAB: SWITCH MODEL", TEXT);
    c.text_c_sh(
        cx,
        126,
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

fn ability_icon(c: &mut Canvas, i: usize, cx: i32, cy: i32, col: Rgba) {
    match i {
        0 => {
            c.circle(cx, cy, 7, col);
            c.circle(cx, cy, 6, BG0);
            c.rect(cx - 4, cy, 9, 1, col);
            c.rect(cx, cy - 4, 1, 9, col);
        }
        1 => {
            c.rect(cx - 9, cy - 1, 19, 3, col);
            c.rect(cx - 9, cy, 19, 1, WHITE);
        }
        2 => {
            for (ox, oy) in [(-5, -4), (4, -4), (0, 4)] {
                c.rect(cx + ox - 2, cy + oy - 2, 5, 5, INK);
                c.rect(cx + ox - 1, cy + oy - 1, 3, 3, col);
            }
        }
        _ => {
            for (k, wd) in [(-6, 3), (-4, 4), (-2, 5), (0, 6), (2, 4), (4, 3), (6, 2)] {
                c.rect(cx - (k * 2) / 5 - wd / 2, cy + k, wd, 2, col);
            }
        }
    }
}
