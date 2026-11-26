//! A tiny software pixel canvas for the HUD: rectangles, bars and a 3x5
//! pixel font, drawn straight into an RGBA image at the game's low resolution.

/// RGBA colour, 0-255, straight alpha.
pub type Rgba = [u8; 4];

pub const fn hex(h: u32) -> Rgba {
    [(h >> 16) as u8, (h >> 8) as u8, h as u8, 255]
}

pub const fn alpha(c: Rgba, a: u8) -> Rgba {
    [c[0], c[1], c[2], a]
}

pub fn from_color(c: bevy::color::Color) -> Rgba {
    let s = c.to_srgba();
    [
        (s.red * 255.0) as u8,
        (s.green * 255.0) as u8,
        (s.blue * 255.0) as u8,
        (s.alpha * 255.0) as u8,
    ]
}

// Palette, matching the concept mockups.
pub const BG0: Rgba = hex(0x0b0e17);
pub const BG1: Rgba = hex(0x121828);
pub const LINE: Rgba = hex(0x34416a);
pub const DIM: Rgba = hex(0x6f7ca3);
pub const TEXT: Rgba = hex(0xe6edf7);
pub const WHITE: Rgba = hex(0xffffff);
pub const INK: Rgba = hex(0x080a10);
pub const GREEN: Rgba = hex(0x3ddc84);
pub const RED: Rgba = hex(0xff4d6d);
pub const RED_DD: Rgba = hex(0x3a1222);
pub const YELLOW: Rgba = hex(0xffd166);
pub const ORANGE: Rgba = hex(0xff9f43);
pub const PINK: Rgba = hex(0xff5fd2);
pub const CYAN: Rgba = hex(0x5ff3ff);
pub const GREY: Rgba = hex(0x8a93a8);
pub const GREY_D: Rgba = hex(0x4a5268);
pub const GREY_DD: Rgba = hex(0x2b3142);
pub const SKIN: Rgba = hex(0xc98d63);
pub const TERM: Rgba = hex(0x59ff8f);
pub const TERM_D: Rgba = hex(0x1e7a42);
pub const TERM_BG: Rgba = hex(0x03120a);

pub struct Canvas<'a> {
    pub data: &'a mut [u8],
    pub w: i32,
    pub h: i32,
}

impl Canvas<'_> {
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    pub fn dot(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || c[3] == 0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let d = &mut self.data[i..i + 4];
        if c[3] == 255 || d[3] == 0 {
            d.copy_from_slice(&c);
            return;
        }
        // straight-alpha "over"
        let sa = c[3] as f32 / 255.0;
        let da = d[3] as f32 / 255.0;
        let oa = sa + da * (1.0 - sa);
        for k in 0..3 {
            let v = (c[k] as f32 * sa + d[k] as f32 * da * (1.0 - sa)) / oa;
            d[k] = v as u8;
        }
        d[3] = (oa * 255.0) as u8;
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.dot(xx, yy, c);
            }
        }
    }

    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y + 1, 1, h - 2, c);
        self.rect(x + w - 1, y + 1, 1, h - 2, c);
    }

    /// Filled box with an ink outline and a coloured inner border.
    pub fn panel(&mut self, x: i32, y: i32, w: i32, h: i32, fill: Rgba, border: Rgba) {
        self.rect(x, y, w, h, fill);
        self.frame(x, y, w, h, INK);
        self.frame(x + 1, y + 1, w - 2, h - 2, border);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, frac: f32, col: Rgba, edge: Rgba) {
        self.rect(x, y, w, h, INK);
        self.rect(x + 1, y + 1, w - 2, h - 2, alpha(GREY_DD, 220));
        let fw = ((w - 2) as f32 * frac.clamp(0.0, 1.0)).round() as i32;
        if fw > 0 {
            self.rect(x + 1, y + 1, fw, h - 2, col);
            self.rect(x + 1, y + 1, fw, 1, edge);
        }
    }

    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r + r / 2 {
                    self.dot(cx + x, cy + y, c);
                }
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Rgba) {
        self.text_scaled(x, y, s, c, 1);
    }

    pub fn text_scaled(&mut self, x: i32, y: i32, s: &str, c: Rgba, scale: i32) {
        let mut cx = x;
        for ch in s.chars() {
            for (row, line) in glyph(ch).iter().enumerate() {
                for (col, b) in line.bytes().enumerate() {
                    if b == b'#' {
                        self.rect(
                            cx + col as i32 * scale,
                            y + row as i32 * scale,
                            scale,
                            scale,
                            c,
                        );
                    }
                }
            }
            cx += 4 * scale;
        }
    }

    /// Text with a one-pixel ink shadow, readable over the 3D scene.
    pub fn text_sh(&mut self, x: i32, y: i32, s: &str, c: Rgba) {
        self.text(x + 1, y + 1, s, INK);
        self.text(x, y, s, c);
    }

    pub fn text_c(&mut self, cx: i32, y: i32, s: &str, c: Rgba) {
        self.text(cx - text_w(s) / 2, y, s, c);
    }

    pub fn text_c_sh(&mut self, cx: i32, y: i32, s: &str, c: Rgba) {
        self.text_sh(cx - text_w(s) / 2, y, s, c);
    }

    pub fn text_r(&mut self, rx: i32, y: i32, s: &str, c: Rgba) {
        self.text(rx - text_w(s), y, s, c);
    }

    /// Big outlined title text.
    pub fn title(&mut self, cx: i32, y: i32, s: &str, c: Rgba, scale: i32) {
        let x = cx - (text_w(s) * scale) / 2;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (1, 1), (2, 2)] {
            self.text_scaled(x + dx, y + dy, s, INK, scale);
        }
        self.text_scaled(x, y, s, c, scale);
    }
}

/// Width in pixels of a string in the 3x5 font.
pub fn text_w(s: &str) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 { 0 } else { n * 4 - 1 }
}

pub fn glyph(ch: char) -> [&'static str; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'B' => ["##.", "#.#", "##.", "#.#", "##."],
        'C' => [".##", "#..", "#..", "#..", ".##"],
        'D' => ["##.", "#.#", "#.#", "#.#", "##."],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'F' => ["###", "#..", "##.", "#..", "#.."],
        'G' => [".##", "#..", "#.#", "#.#", ".##"],
        'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'J' => ["..#", "..#", "..#", "#.#", ".#."],
        'K' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'L' => ["#..", "#..", "#..", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'O' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'P' => ["##.", "#.#", "##.", "#..", "#.."],
        'Q' => [".#.", "#.#", "#.#", "##.", ".##"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'S' => [".##", "#..", ".#.", "..#", "##."],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'V' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        'W' => ["#.#", "#.#", "###", "###", "#.#"],
        'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        'Y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        'Z' => ["###", "..#", ".#.", "#..", "###"],
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["##.", "..#", ".#.", "#..", "###"],
        '3' => ["##.", "..#", ".#.", "..#", "##."],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "##.", "..#", "##."],
        '6' => [".##", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", ".#.", ".#.", ".#."],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "##."],
        '.' => ["...", "...", "...", "...", ".#."],
        ',' => ["...", "...", "...", ".#.", "#.."],
        '!' => [".#.", ".#.", ".#.", "...", ".#."],
        '?' => ["##.", "..#", ".#.", "...", ".#."],
        ':' => ["...", ".#.", "...", ".#.", "..."],
        '-' => ["...", "...", "###", "...", "..."],
        '+' => ["...", ".#.", "###", ".#.", "..."],
        '*' => ["...", "#.#", ".#.", "#.#", "..."],
        '$' => [".##", "##.", ".#.", ".##", "##."],
        '%' => ["#.#", "..#", ".#.", "#..", "#.#"],
        '/' => ["..#", "..#", ".#.", "#..", "#.."],
        '(' => [".#.", "#..", "#..", "#..", ".#."],
        ')' => [".#.", "..#", "..#", "..#", ".#."],
        '>' => ["#..", ".#.", "..#", ".#.", "#.."],
        '<' => ["..#", ".#.", "#..", ".#.", "..#"],
        '\'' => [".#.", ".#.", "...", "...", "..."],
        '=' => ["...", "###", "...", "###", "..."],
        '_' => ["...", "...", "...", "...", "###"],
        '#' => ["#.#", "###", "#.#", "###", "#.#"],
        _ => ["...", "...", "...", "...", "..."],
    }
}
