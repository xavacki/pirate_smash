// Glyph and triangle helpers adapted from Ion Bloom (MIT); see third_party/ion-bloom-LICENSE.
use bytemuck::{Pod, Zeroable};
#[derive(Clone, Copy)]
pub struct Settings {
    pub threshold: f32,
    pub strength: f32,
    pub radius: f32,
    pub exposure: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            strength: 0.35,
            radius: 0.5,
            exposure: 1.0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}
#[derive(Default)]
pub struct Hud {
    pub vertices: std::cell::RefCell<Vec<UiVertex>>,
}
impl Hud {
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, c: [u8; 3]) {
        if c == [248, 250, 241] || c == [39, 71, 95] {
            let source = if c == [39, 71, 95] {
                [1100., 2304., 768., 140.]
            } else {
                [0., 2304., 1024., 240.]
            };
            self.panel([x, y, w, h], source);
            return;
        }
        let c = rgb(c);
        rect(&mut self.vertices.borrow_mut(), [x, y, w, h], c)
    }
    pub fn text(&self, x: f32, y: f32, s: &str, scale: f32, c: [u8; 3]) {
        let c = if c == [46, 66, 54] {
            [1., 0.86, 0.58, 1.]
        } else if c == [104, 119, 107] || c == [68, 88, 72] {
            [0.92, 0.85, 0.68, 1.]
        } else {
            rgb(c)
        };
        text(&mut self.vertices.borrow_mut(), s, x, y, scale, c)
    }
    pub fn sprite(&self, r: [f32; 4], source: [f32; 4], color: [f32; 4]) {
        sprite(&mut self.vertices.borrow_mut(), r, source, color);
    }
    fn panel(&self, r: [f32; 4], s: [f32; 4]) {
        let border = 9_f32.min(r[2] / 3.).min(r[3] / 3.);
        let dx = [r[0], r[0] + border, r[0] + r[2] - border, r[0] + r[2]];
        let dy = [r[1], r[1] + border, r[1] + r[3] - border, r[1] + r[3]];
        let sx = [s[0], s[0] + 24., s[0] + s[2] - 24., s[0] + s[2]];
        let sy = [s[1], s[1] + 24., s[1] + s[3] - 24., s[1] + s[3]];
        for y in 0..3 {
            for x in 0..3 {
                self.sprite(
                    [dx[x], dy[y], dx[x + 1] - dx[x], dy[y + 1] - dy[y]],
                    [sx[x], sy[y], sx[x + 1] - sx[x], sy[y + 1] - sy[y]],
                    [1.; 4],
                );
            }
        }
    }
    pub fn selection_brackets(&self, point: crate::sdl::Point) {
        const GOLD: [u8; 3] = [221, 171, 87];
        for dx in [-18., 12.] {
            for dy in [-18., 12.] {
                let x = point.x + dx;
                let y = point.y + dy;
                self.rect(x, y + if dy < 0. { 0. } else { 5. }, 6., 1., GOLD);
                self.rect(x + if dx < 0. { 0. } else { 5. }, y, 1., 6., GOLD);
            }
        }
    }
}
fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.,
        c[1] as f32 / 255.,
        c[2] as f32 / 255.,
        1.,
    ]
}
fn rect(out: &mut Vec<UiVertex>, r: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = r;
    for position in [
        [x, y],
        [x + w, y],
        [x + w, y + h],
        [x, y],
        [x + w, y + h],
        [x, y + h],
    ] {
        out.push(UiVertex {
            position,
            color,
            uv: [0., 0.],
        });
    }
}
fn sprite(out: &mut Vec<UiVertex>, r: [f32; 4], s: [f32; 4], color: [f32; 4]) {
    for [x, y] in [[0., 0.], [1., 0.], [1., 1.], [0., 0.], [1., 1.], [0., 1.]] {
        out.push(UiVertex {
            position: [r[0] + x * r[2], r[1] + y * r[3]],
            color,
            uv: [(s[0] + x * s[2]) / 4096., (s[1] + y * s[3]) / 4096.],
        });
    }
}
fn text(out: &mut Vec<UiVertex>, s: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
    for (i, c) in s.chars().enumerate() {
        if !(32..127).contains(&(c as u32)) {
            continue;
        }
        let g = c as u32 - 32;
        let region = [
            (g % 32) as f32 * 48.,
            2688. + (g / 32) as f32 * 64.,
            48.,
            64.,
        ];
        let r = [x + i as f32 * 6. * scale, y - scale, 6. * scale, 9. * scale];
        sprite(
            out,
            [r[0] + 0.6, r[1] + 0.8, r[2], r[3]],
            region,
            [0.08, 0.04, 0.02, color[3]],
        );
        sprite(out, r, region, color);
    }
}
#[allow(dead_code)]
fn glyph(c: char) -> [u8; 5] {
    match c {
        'A' => [0x7e, 0x11, 0x11, 0x11, 0x7e],
        'B' => [0x7f, 0x49, 0x49, 0x49, 0x36],
        'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
        'D' => [0x7f, 0x41, 0x41, 0x22, 0x1c],
        'E' => [0x7f, 0x49, 0x49, 0x49, 0x41],
        'F' => [0x7f, 0x09, 0x09, 0x09, 0x01],
        'G' => [0x3e, 0x41, 0x49, 0x49, 0x7a],
        'H' => [0x7f, 0x08, 0x08, 0x08, 0x7f],
        'I' => [0, 0x41, 0x7f, 0x41, 0],
        'J' => [0x20, 0x40, 0x41, 0x3f, 0x01],
        'K' => [0x7f, 0x08, 0x14, 0x22, 0x41],
        'L' => [0x7f, 0x40, 0x40, 0x40, 0x40],
        'M' => [0x7f, 0x02, 0x0c, 0x02, 0x7f],
        'N' => [0x7f, 0x04, 0x08, 0x10, 0x7f],
        'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
        'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
        'Q' => [0x3e, 0x41, 0x51, 0x21, 0x5e],
        'R' => [0x7f, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        'T' => [0x01, 0x01, 0x7f, 0x01, 0x01],
        'U' => [0x3f, 0x40, 0x40, 0x40, 0x3f],
        'V' => [0x1f, 0x20, 0x40, 0x20, 0x1f],
        'W' => [0x3f, 0x40, 0x38, 0x40, 0x3f],
        'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        'Y' => [0x07, 0x08, 0x70, 0x08, 0x07],
        'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
        '0' => [0x3e, 0x51, 0x49, 0x45, 0x3e],
        '1' => [0, 0x42, 0x7f, 0x40, 0],
        '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => [0x21, 0x41, 0x45, 0x4b, 0x31],
        '4' => [0x18, 0x14, 0x12, 0x7f, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3c, 0x4a, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1e],
        ':' => [0, 0x36, 0x36, 0, 0],
        '!' => [0, 0, 0x5f, 0, 0],
        '.' => [0, 0x60, 0x60, 0, 0],
        '/' => [0x20, 0x10, 0x08, 0x04, 0x02],
        '+' => [0x08, 0x08, 0x3e, 0x08, 0x08],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        _ => [0; 5],
    }
}
