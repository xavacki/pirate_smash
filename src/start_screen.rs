//! Original packaged splash and home screen; native logo curves are exported by recovery tools.
use crate::ui::UiVertex;
mod font {
    include!("../assets/start-screen/font_map.rs");
}
mod atlas {
    include!("../assets/start-screen/atlas_map.rs");
}
mod logo {
    include!("../assets/start-screen/logo_animation.rs");
}
mod motes {
    include!("../assets/effects/home.rs");
}
const SCALE: f32 = 800. / 2436.;
const SPLASH_END: f32 = 1.9;
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Splash,
    Home,
    Leaving,
    Game,
}
pub struct StartScreen {
    pub phase: Phase,
    pub elapsed: f32,
    leaving: f32,
    options: bool,
}
impl StartScreen {
    pub fn new(show: bool) -> Self {
        Self {
            phase: if show { Phase::Splash } else { Phase::Game },
            elapsed: 0.,
            leaving: 0.,
            options: false,
        }
    }
    pub fn active(&self) -> bool {
        self.phase != Phase::Game
    }
    pub fn seek(&mut self, t: f32) {
        self.elapsed = t.max(0.);
        self.phase = if t < SPLASH_END {
            Phase::Splash
        } else {
            Phase::Home
        };
    }
    pub fn home(&mut self) {
        self.phase = Phase::Home;
        self.elapsed = SPLASH_END;
        self.leaving = 0.;
        self.options = false;
    }
    pub fn tick(&mut self, dt: f32) {
        if !self.active() {
            return;
        }
        self.elapsed += dt;
        if self.phase == Phase::Splash && self.elapsed >= SPLASH_END {
            self.phase = Phase::Home;
        }
        if self.phase == Phase::Leaving {
            self.leaving += dt;
            if self.leaving >= 0.35 {
                self.phase = Phase::Game;
            }
        }
    }
    pub fn begin(&mut self) {
        if self.phase == Phase::Splash {
            self.home();
        } else if self.phase == Phase::Home && !self.options {
            self.phase = Phase::Leaving;
            self.leaving = 0.;
        }
    }
    pub fn escape(&mut self) -> bool {
        if self.options {
            self.options = false;
            true
        } else if self.phase == Phase::Game {
            self.home();
            true
        } else {
            false
        }
    }
    pub fn click_view(&mut self, x: f32, y: f32, width: f32) -> bool {
        self.click(x + (1100. - width) / 2., y)
    }
    pub fn click(&mut self, x: f32, y: f32) -> bool {
        if self.phase == Phase::Splash {
            self.home();
            return false;
        }
        if self.phase != Phase::Home {
            return false;
        }
        if self.options {
            if inside([425., 355., 250., 65.], x, y) {
                return true;
            }
            if inside([425., 445., 250., 65.], x, y) {
                self.options = false;
            }
            return false;
        }
        if inside(self.button_rect(), x, y) {
            self.begin();
        } else if inside([610., 724., 125., 76.], x, y) {
            self.options = true;
        }
        false
    }
    fn button_rect(&self) -> [f32; 4] {
        let mut r = play_rect();
        r[1] += (1. - smooth((self.elapsed - SPLASH_END) / 0.35)) * 100.;
        r
    }
    pub fn frame(&self, level: usize, mouse: (f32, f32), glow: bool, width: f32) -> Vec<UiVertex> {
        if self.phase != Phase::Splash {
            return self.pirate_home(level, glow, width);
        }
        let mut out = vec![];
        let alpha = if self.phase == Phase::Leaving {
            1. - smooth(self.leaving / 0.35)
        } else {
            1.
        };
        let mut draw = Draw {
            out: &mut out,
            alpha,
        };
        // Preserve the original square artwork, extending it across the desktop instead of stretching it.
        let art = if self.phase == Phase::Splash {
            "GameSplash"
        } else {
            "HomeBG"
        };
        let background = if width < 800. {
            [150., 0., 800., 800.]
        } else {
            [-0.5, -150., 1101., 1100.]
        };
        draw.sprite(art, background, 0., [1.; 4]);
        if self.phase == Phase::Splash {
            self.logo(&mut draw);
        } else {
            self.home_particles(&mut draw);
            self.home_frame(&mut draw, level, (mouse.0 + (1100. - width) / 2., mouse.1));
        }
        if self.options {
            draw.solid([0., 0., 1100., 800.], [0.015, 0.04, 0.09, 0.65]);
            draw.nine("BlueMainWhitePanel", [370., 230., 360., 350.], 32.);
            draw.nine("GreenButton", [425., 355., 250., 65.], 19.);
            draw.nine("GreenButton", [425., 445., 250., 65.], 19.);
            draw.label("Settings", 550., 282., 30., [1.; 4]);
            draw.label(
                if glow { "Bloom: On" } else { "Bloom: Off" },
                550.,
                387.,
                28.,
                [1.; 4],
            );
            draw.label("Back", 550., 477., 28., [1.; 4]);
        }
        let offset = (1100. - width) / 2.;
        for vertex in &mut out {
            vertex.position[0] -= offset;
        }
        out
    }
    fn pirate_home(&self, level: usize, glow: bool, width: f32) -> Vec<UiVertex> {
        mod pirate {
            include!("../assets/pirate/ui_map.rs");
        }
        let d = crate::ui::Hud::default();
        d.sprite([0., 0., width, 800.], pirate::HOME, [1.; 4]);
        let offset = (1100. - width) / 2.;
        let w = (width - 24.).min(500.);
        d.rect((width - w) / 2., 90., w, 125., [248, 250, 241]);
        d.sprite(
            [width / 2. - 55., 28., 110., 110.],
            [1920., 2304., 256., 256.],
            [1.; 4],
        );
        let title = "PIRATE SMASH";
        let scale = ((w - 30.) / (title.len() as f32 * 6.)).min(4.);
        d.text(
            (width - title.len() as f32 * 6. * scale) / 2.,
            151.,
            title,
            scale,
            [46, 66, 54],
        );
        d.text(
            width / 2. - 94.,
            190.,
            "CLAIM THE TREASURE",
            1.8,
            [104, 119, 107],
        );
        let [x, y, w, h] = self.button_rect();
        crate::material_editor::button(&d, x - offset, y, w, h, &format!("SAIL: LEVEL {level}"));
        crate::material_editor::button(&d, 610. - offset, 724., 125., 65., "SETTINGS");
        if self.options {
            d.rect(370. - offset, 230., 360., 350., [248, 250, 241]);
            d.text(470. - offset, 280., "SETTINGS", 3., [46, 66, 54]);
            crate::material_editor::button(
                &d,
                425. - offset,
                355.,
                250.,
                65.,
                if glow { "BLOOM: ON" } else { "BLOOM: OFF" },
            );
            crate::material_editor::button(&d, 425. - offset, 445., 250., 65., "BACK");
        }
        let mut out = d.vertices.into_inner();
        if self.phase == Phase::Leaving {
            for v in &mut out {
                v.color[3] *= 1. - smooth(self.leaving / 0.35);
            }
        }
        out
    }
    fn logo(&self, d: &mut Draw) {
        let t = self.elapsed.min(logo::LOGO_DURATION);
        let sx = logo::logo_channel(6, t).max(0.) * SCALE;
        let sy = logo::logo_channel(7, t).max(0.) * SCALE;
        let center = [550., 400. - 724. * SCALE];
        let rect = |x: f32, y: f32, w: f32, h: f32| {
            [
                center[0] + x * sx - w * sx / 2.,
                center[1] - y * sy - h * sy / 2.,
                w * sx,
                h * sy,
            ]
        };
        let shadow = logo::logo_channel(15, t).max(0.);
        d.sprite(
            "SmashFestLogoBackground",
            rect(0., -70., 712. * shadow, 489. * shadow),
            0.,
            [0., 0., 0., 0.18],
        );
        d.sprite(
            "SmashFestLogoBackground",
            rect(
                0.,
                -14.,
                712. * logo::logo_channel(12, t).max(0.),
                489. * logo::logo_channel(13, t).max(0.),
            ),
            0.,
            [1.; 4],
        );
        let word_x = logo::logo_channel(0, t);
        let word_y = logo::logo_channel(1, t);
        let angle = -logo::logo_channel(5, t).to_radians();
        let group = logo::logo_channel(18, t).max(0.);
        for (i, name, x, y, w, h) in [
            (0, "LogoLetterFirstS", -260.7, -52.9, 139., 179.),
            (1, "LogoLetterM", -104.9, -43.2, 152., 155.),
            (2, "LogoLetterA", 62.6, -36.7, 118., 128.),
            (3, "LogoLetterSecondS", 167.5, -40., 101., 134.),
            (4, "LogoLetterH", 304.2, -43., 146., 180.),
        ] {
            let lx = x * group;
            let ly = -y * group;
            let rx = lx * angle.cos() - ly * angle.sin();
            let ry = lx * angle.sin() + ly * angle.cos();
            let scale_x = logo::logo_channel(21 + i * 3, t).max(0.);
            let scale_y = logo::logo_channel(22 + i * 3, t).max(0.);
            d.sprite(
                name,
                rect(
                    word_x + rx,
                    word_y - ry,
                    w * scale_x * group,
                    h * scale_y * group,
                ),
                angle,
                [1.; 4],
            );
        }
        d.sprite(
            "SmashFestText",
            rect(
                6.,
                logo::logo_channel(36, t),
                412.56 * logo::logo_channel(9, t).max(0.),
                218.17 * logo::logo_channel(10, t).max(0.),
            ),
            0.,
            [1.; 4],
        );
    }
    fn home_frame(&self, d: &mut Draw, level: usize, mouse: (f32, f32)) {
        let enter = smooth((self.elapsed - SPLASH_END) / 0.35);
        let drop = (1. - enter) * -75.;
        // Native top counters at anchored X ±260, Y1012, size350×145.
        for (x, icon, count) in [(-260., "CoinIcon", "0"), (260., "LifeIcon", "5")] {
            let r = source_rect(x, 1012., 350., 145.);
            d.nine(
                "HomepageLifeCoinButton",
                [r[0], r[1] + drop, r[2], r[3]],
                13.,
            );
            d.sprite(icon, [r[0] - 8., r[1] + 3. + drop, 40., 40.], 0., [1.; 4]);
            d.label(count, r[0] + 65., r[1] + r[3] / 2. + drop, 30., [1.; 4]);
        }
        let mut r = self.button_rect();
        let hover = inside(r, mouse.0, mouse.1);
        if hover {
            r = [r[0] - 3., r[1] - 1.5, r[2] + 6., r[3] + 3.];
        }
        d.nine("GreenButton", r, 28.);
        // Reuse the original highlight sprite for the button glint.
        let phase = (self.elapsed * 0.55).fract();
        if phase < 0.38 {
            let a = (phase / 0.38 * std::f32::consts::PI).sin() * 0.65;
            d.sprite(
                "ButtonShine",
                [
                    r[0] + 20. + phase / 0.38 * (r[2] - 66.),
                    r[1] + 10.,
                    46.,
                    22.,
                ],
                -0.25,
                [1., 1., 1., a],
            );
        }
        d.label(
            &format!("Level {level}"),
            r[0] + r[2] / 2.,
            r[1] + r[3] / 2. - 3.,
            30.,
            [1.; 4],
        );
        let bar_y = 800. - 227. * SCALE + (1. - enter) * 80.;
        d.sprite(
            "BottomPanelBg",
            [0., bar_y, 1100., 800. - bar_y],
            0.,
            [1.; 4],
        );
        for (x, name, label, selected) in [
            (430., "ShopIcon", "Shop", false),
            (550., "HomeIcon", "Home", true),
            (670., "SettingsIcon", "Settings", false),
        ] {
            if selected {
                d.nine("BottomPanelBigBg", [x - 48., bar_y - 7., 96., 90.], 13.);
            }
            d.sprite(name, [x - 22., bar_y + 5., 44., 44.], 0., [1.; 4]);
            d.label(label, x, bar_y + 61., 18., [1.; 4]);
        }
    }
    fn home_particles(&self, d: &mut Draw) {
        // The original FairyLight is prewarmed, 10 particles/sec, ten-second life.
        // Its -90° emitter rotation turns local Z velocity into upward screen drift.
        let time = (self.elapsed - SPLASH_END).max(0.);
        for i in 0..(motes::LIFETIME * motes::RATE) as usize {
            let phase = time + i as f32 / motes::RATE;
            let cycle = (phase / motes::LIFETIME).floor() as u32;
            let seed = (i as u32).wrapping_mul(747796405) ^ cycle.wrapping_mul(2891336453);
            let random = |salt: u32| {
                let mut x = seed ^ salt;
                x = (x ^ (x >> 16)).wrapping_mul(2246822519);
                x = (x ^ (x >> 13)).wrapping_mul(3266489917);
                ((x ^ (x >> 16)) >> 8) as f32 / 16777216.
            };
            let age = phase.rem_euclid(motes::LIFETIME);
            let t = age / motes::LIFETIME;
            let x = 550.
                + (random(1) - 0.5) * 300.
                + age
                    * (motes::DRIFT_X[0] + random(2) * (motes::DRIFT_X[1] - motes::DRIFT_X[0]))
                    * 12.;
            let y = 575.
                - random(3) * 100.
                - age
                    * (motes::DRIFT_UP[0] + random(4) * (motes::DRIFT_UP[1] - motes::DRIFT_UP[0]))
                    * 8.;
            let size = (motes::SIZE[0] + random(5) * (motes::SIZE[1] - motes::SIZE[0])) * 17.;
            let alpha = motes::ALPHA * (1. - t) * smooth(t * 15.);
            d.glow(
                x,
                y,
                size * 2.6,
                [motes::GOLD[0], motes::GOLD[1], motes::GOLD[2], alpha * 0.35],
            );
            d.glow(x, y, size, [1., 0.87, 0.47, alpha]);
        }
    }
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn source_rect(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
    [
        550. + x * SCALE - w * SCALE / 2.,
        400. - y * SCALE - h * SCALE / 2.,
        w * SCALE,
        h * SCALE,
    ]
}
fn play_rect() -> [f32; 4] {
    source_rect(0., -675., 624.696, 301.356)
}
fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}
struct Draw<'a> {
    out: &'a mut Vec<UiVertex>,
    alpha: f32,
}
impl Draw<'_> {
    fn glow(&mut self, x: f32, y: f32, radius: f32, mut color: [f32; 4]) {
        color[3] *= self.alpha;
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 16.;
            for (position, alpha) in [
                ([x, y], color[3]),
                ([x + radius * a.cos(), y + radius * a.sin()], 0.),
                ([x + radius * b.cos(), y + radius * b.sin()], 0.),
            ] {
                self.out.push(UiVertex {
                    position,
                    color: [color[0], color[1], color[2], alpha],
                    uv: [0.; 2],
                });
            }
        }
    }
    fn label(&mut self, text: &str, cx: f32, cy: f32, size: f32, color: [f32; 4]) {
        let Some([atlas_x, atlas_y, _, _]) = atlas::region("NunitoFontAtlas") else {
            return;
        };
        let scale = size / font::FONT_SIZE;
        let mut pen = 0.;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        let mut glyphs = vec![];
        for c in text.chars() {
            if let Some(g) = font::glyph(c) {
                debug_assert!(
                    g.rect[0] + g.rect[2] <= font::FONT_ATLAS_WIDTH as f32
                        && g.rect[1] + g.rect[3] <= font::FONT_ATLAS_HEIGHT as f32
                );
                let x = pen + g.bearing_x;
                let y = -g.bearing_y;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + g.rect[2]);
                max_y = max_y.max(y + g.rect[3]);
                glyphs.push((x, y, g));
                pen += g.advance;
            }
        }
        let origin = [
            cx - (min_x + max_x) * scale / 2.,
            cy - (min_y + max_y) * scale / 2.,
        ];
        for (offset, tint) in [(1.2, [0.08, 0.12, 0.2, 0.65]), (0., color)] {
            for &(x, y, g) in &glyphs {
                let [r_x, r_y, w, h] = g.rect;
                self.quad(
                    [
                        origin[0] + x * scale,
                        origin[1] + y * scale + offset,
                        w * scale,
                        h * scale,
                    ],
                    [atlas_x + r_x, atlas_y + r_y, w, h],
                    0.,
                    tint,
                );
            }
        }
    }
    fn solid(&mut self, r: [f32; 4], color: [f32; 4]) {
        self.quad(r, [0.; 4], 0., color);
    }
    fn sprite(&mut self, name: &str, r: [f32; 4], rotation: f32, color: [f32; 4]) {
        if r[2] <= 0.001 || r[3] <= 0.001 {
            return;
        }
        let region =
            atlas::region(name).unwrap_or_else(|| panic!("Missing original menu sprite: {name}"));
        self.quad(r, region, rotation, color);
    }
    fn quad(&mut self, r: [f32; 4], uv: [f32; 4], rotation: f32, mut color: [f32; 4]) {
        color[3] *= self.alpha;
        let [x, y, w, h] = r;
        let mut p = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let cx = x + w / 2.;
        let cy = y + h / 2.;
        for point in &mut p {
            let dx = point[0] - cx;
            let dy = point[1] - cy;
            point[0] = cx + dx * rotation.cos() - dy * rotation.sin();
            point[1] = cy + dx * rotation.sin() + dy * rotation.cos();
        }
        let [u, v, uw, vh] = uv;
        let t = [[u, v], [u + uw, v], [u + uw, v + vh], [u, v + vh]];
        for i in [0, 1, 2, 0, 2, 3] {
            self.out.push(UiVertex {
                position: p[i],
                color,
                uv: [
                    t[i][0] / atlas::ATLAS_WIDTH as f32,
                    t[i][1] / atlas::ATLAS_HEIGHT as f32,
                ],
            });
        }
    }
    fn nine(&mut self, name: &str, r: [f32; 4], border: f32) {
        let Some([u, v, w, h]) = atlas::region(name) else {
            self.solid(r, [0.08, 0.25, 0.6, 1.]);
            return;
        };
        let [x, y, rw, rh] = r;
        let b = border.min(rw / 2.).min(rh / 2.);
        let xs = [x, x + b, x + rw - b, x + rw];
        let ys = [y, y + b, y + rh - b, y + rh];
        let us = [u, u + w * 0.3, u + w * 0.7, u + w];
        let vs = [v, v + h * 0.3, v + h * 0.7, v + h];
        for j in 0..3 {
            for i in 0..3 {
                self.quad(
                    [xs[i], ys[j], xs[i + 1] - xs[i], ys[j + 1] - ys[j]],
                    [us[i], vs[j], us[i + 1] - us[i], vs[j + 1] - vs[j]],
                    0.,
                    [1.; 4],
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prewarmed_home_particles_move_and_stay_finite() {
        let mut m = StartScreen::new(true);
        let mut a = vec![];
        m.seek(2.5);
        m.home_particles(&mut Draw {
            out: &mut a,
            alpha: 1.,
        });
        let mut b = vec![];
        m.seek(3.5);
        m.home_particles(&mut Draw {
            out: &mut b,
            alpha: 1.,
        });
        assert_eq!(a.len(), 100 * 2 * 16 * 3);
        assert!(a.iter().zip(&b).any(|(a, b)| a.position != b.position));
        assert!(b.iter().all(|v| v
            .position
            .iter()
            .chain(v.color.iter())
            .all(|v| v.is_finite())));
    }
    #[test]
    fn original_logo_curves_reveal_and_settle() {
        assert_eq!(logo::logo_channel(21, 0.), 0.);
        assert!(logo::logo_channel(21, 0.333_333_3) > 1.9);
        assert!((logo::logo_channel(21, 1.4) - 1.4).abs() < 0.001);
        assert!((logo::logo_channel(9, 1.4) - 1.).abs() < 0.001);
    }
    #[test]
    fn start_requires_a_separate_play_action() {
        let mut m = StartScreen::new(true);
        m.begin();
        assert_eq!(m.phase, Phase::Home);
        assert!(m.active());
        m.begin();
        assert_eq!(m.phase, Phase::Leaving);
        m.tick(0.4);
        assert!(!m.active());
    }
    #[test]
    fn menu_uses_original_atlas_and_changes_with_animation() {
        let mut m = StartScreen::new(true);
        m.seek(0.2);
        let a = m.frame(1, (0., 0.), true, 1100.);
        m.seek(1.2);
        let b = m.frame(1, (0., 0.), true, 1100.);
        assert!(b.len() > a.len());
        assert!(b
            .iter()
            .all(|v| v.uv.iter().all(|c| c.is_finite() && *c >= 0. && *c <= 1.)));
    }
}
