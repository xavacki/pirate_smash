use crate::{scene::Material, ui::Hud};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

pub type ObjectKey = (usize, usize, usize);
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    pub rgb: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub coat: f32,
    pub opacity: f32,
    pub glow: f32,
    pub texture: bool,
}
impl Finish {
    fn from_material(m: Material) -> Self {
        Self {
            rgb: [m.color[0], m.color[1], m.color[2]],
            metallic: m.metallic,
            roughness: m.roughness,
            coat: m.clear_coat,
            opacity: m.color[3],
            glow: 0.,
            texture: m.use_texture,
        }
    }
    pub fn apply(self, original: Material) -> Material {
        Material {
            color: [self.rgb[0], self.rgb[1], self.rgb[2], self.opacity],
            metallic: self.metallic,
            roughness: self.roughness,
            clear_coat: self.coat,
            emissive: self.rgb.map(|c| c * self.glow),
            blend: self.opacity < 0.999 || (self.texture && original.blend),
            glass: self.opacity < 0.999,
            use_texture: self.texture,
            ..original
        }
    }
}
type Records = BTreeMap<ObjectKey, Finish>;

fn parse(data: &str) -> Result<Records, String> {
    let mut records = Records::new();
    let mut lines = data.lines();
    if lines.next() != Some("# smash-materials v1") {
        return Err("UNSUPPORTED MATERIAL FILE".into());
    }
    for (i, line) in lines.enumerate() {
        let v: Vec<_> = line.split_whitespace().collect();
        if v.is_empty() || v[0].starts_with('#') {
            continue;
        }
        let invalid = || format!("INVALID MATERIAL LINE {}", i + 2);
        if v.len() != 12 {
            return Err(invalid());
        }
        let key = (
            v[0].parse().map_err(|_| invalid())?,
            v[1].parse().map_err(|_| invalid())?,
            v[2].parse().map_err(|_| invalid())?,
        );
        let mut values = [0.; 8];
        for (j, out) in values.iter_mut().enumerate() {
            *out = v[j + 3].parse::<f32>().map_err(|_| invalid())?;
            if !out.is_finite() || !(0. ..=if j == 7 { 5. } else { 1. }).contains(out) {
                return Err(invalid());
            }
        }
        let [r, g, b, metallic, roughness, coat, opacity, glow] = values;
        let texture = match v[11] {
            "0" => false,
            "1" => true,
            _ => return Err(invalid()),
        };
        if records
            .insert(
                key,
                Finish {
                    rgb: [r, g, b],
                    metallic,
                    roughness,
                    coat,
                    opacity,
                    glow,
                    texture,
                },
            )
            .is_some()
        {
            return Err(invalid());
        }
    }
    Ok(records)
}

pub struct Editor {
    pub active: bool,
    pub selected: Option<usize>,
    pub records: Records,
    path: std::path::PathBuf,
    status: String,
    dirty: bool,
    load_failed: bool,
}
impl Editor {
    pub fn load(path: &Path, active: bool) -> Self {
        let mut editor = Self {
            active,
            selected: None,
            records: Records::new(),
            path: path.into(),
            status: "TAP AN OBJECT TO SELECT".into(),
            dirty: false,
            load_failed: false,
        };
        match fs::read_to_string(path) {
            Ok(data) => match parse(&data) {
                Ok(records) => editor.records = records,
                Err(err) => {
                    editor.status = err;
                    editor.load_failed = true;
                    eprintln!("{}", editor.status);
                }
            },
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                editor.status = "CANNOT READ MATERIAL FILE".into();
                editor.load_failed = true;
                eprintln!("{}: {err}", path.display());
            }
            Err(_) => {}
        }
        editor
    }

    fn save(&mut self) {
        if self.load_failed {
            self.status = "FIX MATERIAL FILE BEFORE SAVING".into();
            return;
        }
        let tmp = self.path.with_extension("txt.part");
        let result = (|| -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            writeln!(f, "# smash-materials v1")?;
            for (&(level, stage, object), m) in &self.records {
                let [r, g, b] = m.rgb;
                writeln!(
                    f,
                    "{level} {stage} {object} {r} {g} {b} {} {} {} {} {} {}",
                    m.metallic,
                    m.roughness,
                    m.coat,
                    m.opacity,
                    m.glow,
                    u8::from(m.texture)
                )?;
            }
            f.sync_all()?;
            fs::rename(&tmp, &self.path)
        })();
        self.status = match result {
            Ok(()) => {
                self.dirty = false;
                "MATERIALS SAVED".into()
            }
            Err(err) => {
                eprintln!("Unable to save {}: {err}", self.path.display());
                "COULD NOT SAVE MATERIALS".into()
            }
        };
    }

    pub fn select(&mut self, object: Option<usize>) {
        self.selected = object;
    }
    pub fn handle(
        &mut self,
        layout: &Layout,
        x: f32,
        y: f32,
        selected: Option<(ObjectKey, Material)>,
    ) -> Action {
        let hit = |r: Rect| r.contains(x, y);
        if hit(layout.play) {
            self.active = false;
            return Action::Consumed;
        }
        if hit(layout.save) {
            self.save();
            return Action::Consumed;
        }
        for (r, action) in layout.navigation.into_iter().zip([
            Action::Previous,
            Action::Next,
            Action::ResetPreview,
        ]) {
            if hit(r) {
                return action;
            }
        }
        if !hit(layout.panel) {
            return if hit(layout.header) || hit(layout.footer) {
                Action::Consumed
            } else {
                Action::Pick
            };
        }
        let Some((key, original)) = selected else {
            return Action::Consumed;
        };
        if hit(layout.restore) {
            if self.records.remove(&key).is_some() {
                self.dirty = true;
            }
            return Action::Consumed;
        }
        let mut m = self
            .records
            .get(&key)
            .copied()
            .unwrap_or_else(|| Finish::from_material(original));
        if let Some(i) = layout.presets.iter().position(|&r| hit(r)) {
            (
                m.metallic,
                m.roughness,
                m.coat,
                m.opacity,
                m.glow,
                m.texture,
            ) = [
                (0., 0.72, 0., 1., 0., true),
                (0.96, 0.27, 0.28, 1., 0., false),
                (0., 0.12, 0.68, 0.3, 0., false),
                (0., 0.38, 0.2, 1., 3., false),
            ][i];
        } else if hit(layout.texture) {
            m.texture = !m.texture;
        } else if let Some((i, value)) = layout.slider(x, y) {
            match i {
                0..=2 => m.rgb[i] = value,
                3 => m.metallic = value,
                4 => m.roughness = value.max(0.045),
                5 => m.coat = value,
                6 => m.opacity = 0.08 + 0.92 * value,
                7 => m.glow = 5. * value,
                _ => unreachable!(),
            }
        } else {
            return Action::Consumed;
        }
        self.records.insert(key, m);
        self.dirty = true;
        Action::Consumed
    }

    pub fn overlay(
        &self,
        layout: &Layout,
        selected: Option<(ObjectKey, Material)>,
        level: usize,
        stage: usize,
        target_count: usize,
    ) -> Vec<crate::ui::UiVertex> {
        let d = Hud::default();
        for r in [layout.header, layout.panel, layout.footer] {
            r.draw(&d, [248, 250, 241]);
        }
        let Rect(x, y, _, _) = layout.header;
        d.text(
            x + 12.,
            y + 13.,
            "LEVEL MATERIALS",
            if layout.portrait { 1.65 } else { 2.7 },
            [46, 66, 54],
        );
        d.text(
            x + 12.,
            y + if layout.portrait { 35. } else { 58. },
            &format!("LEVEL {level}   STAGE {}", stage + 1),
            1.4,
            [104, 119, 107],
        );
        layout
            .save
            .button(&d, if self.dirty { "SAVE *" } else { "SAVE" });
        layout.play.button(&d, "PLAY");
        let Rect(x, y, w, _) = layout.panel;
        if let Some((key, original)) = selected {
            let m = self
                .records
                .get(&key)
                .copied()
                .unwrap_or_else(|| Finish::from_material(original));
            let label = if key.2 < target_count {
                format!("OBJECT {}", key.2 + 1)
            } else {
                format!("PLATFORM {}", key.2 - target_count + 1)
            };
            d.text(x + 12., y + 16., &label, 1.65, [46, 66, 54]);
            d.rect(
                x + w - 52.,
                y + 10.,
                40.,
                22.,
                m.rgb.map(|c| (c * 255.) as u8),
            );
            for (r, name) in layout
                .presets
                .into_iter()
                .zip(["MATTE", "METAL", "GLASS", "GLOW"])
            {
                r.button(&d, name);
            }
            let values = [
                m.rgb[0],
                m.rgb[1],
                m.rgb[2],
                m.metallic,
                m.roughness,
                m.coat,
                m.opacity,
                m.glow,
            ];
            for (i, name) in [
                "RED", "GREEN", "BLUE", "METAL", "ROUGH", "COAT", "ALPHA", "GLOW",
            ]
            .into_iter()
            .enumerate()
            {
                let Rect(x, y, w, h) = layout.sliders[i];
                let value = values[i];
                let fraction = match i {
                    6 => (value - 0.08) / 0.92,
                    7 => value / 5.,
                    _ => value,
                };
                d.text(
                    x - layout.label_width,
                    y + (h - 9.) / 2.,
                    name,
                    1.25,
                    [46, 66, 54],
                );
                d.rect(x, y, w, h, [42, 54, 72]);
                d.rect(x, y, w * fraction.clamp(0., 1.), h, [45, 112, 140]);
                d.text(
                    x + (w - 32.) / 2.,
                    y + (h - 9.) / 2.,
                    &format!("{value:.2}"),
                    1.25,
                    [46, 66, 54],
                );
            }
            layout.texture.button(
                &d,
                if m.texture {
                    "TEXTURE: ON"
                } else {
                    "TEXTURE: OFF"
                },
            );
            layout.restore.button(&d, "RESTORE ORIGINAL");
        } else {
            d.text(
                x + 12.,
                y + 16.,
                "TAP A TARGET OR PLATFORM",
                1.5,
                [104, 119, 107],
            );
        }
        for (r, label) in
            layout
                .navigation
                .into_iter()
                .zip(["PREV STAGE", "NEXT STAGE", "RESET PREVIEW"])
        {
            r.button(&d, label);
        }
        let (x, y) = layout.status;
        d.text(x, y, &self.status, 1.25, [104, 119, 107]);
        d.vertices.into_inner()
    }
}

#[derive(Clone, Copy)]
pub struct Rect(pub f32, pub f32, pub f32, pub f32);
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        (self.0..self.0 + self.2).contains(&x) && (self.1..self.1 + self.3).contains(&y)
    }
    fn draw(self, d: &Hud, color: [u8; 3]) {
        d.rect(self.0, self.1, self.2, self.3, color);
    }
    pub fn button(self, d: &Hud, label: &str) {
        button(d, self.0, self.1, self.2, self.3, label);
    }
}

pub struct Layout {
    portrait: bool,
    header: Rect,
    panel: Rect,
    footer: Rect,
    save: Rect,
    play: Rect,
    presets: [Rect; 4],
    sliders: [Rect; 8],
    label_width: f32,
    texture: Rect,
    restore: Rect,
    navigation: [Rect; 3],
    status: (f32, f32),
}

impl Layout {
    pub fn new(width: f32, portrait: bool) -> Self {
        if portrait {
            let inner = width - 32.;
            let preset = inner / 4.;
            let nav = inner / 3.;
            let column = inner / 2.;
            let label_width = 46.;
            Self {
                portrait,
                header: Rect(10., 16., width - 20., 56.),
                panel: Rect(10., 474., width - 20., 238.),
                footer: Rect(10., 719., width - 20., 67.),
                save: Rect(width - 130., 24., 53., 39.),
                play: Rect(width - 71., 24., 53., 39.),
                presets: std::array::from_fn(|i| {
                    Rect(16. + i as f32 * preset, 515., preset - 4., 32.)
                }),
                sliders: std::array::from_fn(|i| {
                    Rect(
                        16. + (i / 4) as f32 * column + label_width,
                        554. + (i % 4) as f32 * 28.,
                        column - label_width - 5.,
                        24.,
                    )
                }),
                label_width,
                texture: Rect(16., 670., column - 5., 33.),
                restore: Rect(16. + column, 670., column - 5., 33.),
                navigation: std::array::from_fn(|i| {
                    Rect(16. + i as f32 * nav, 725., nav - 4., 34.)
                }),
                status: (20., 767.),
            }
        } else {
            Self {
                portrait,
                header: Rect(24., 22., 1052., 90.),
                panel: Rect(785., 125., 291., 599.),
                footer: Rect(24., 738., 1052., 44.),
                save: Rect(690., 41., 170., 52.),
                play: Rect(875., 41., 184., 52.),
                presets: std::array::from_fn(|i| Rect(796. + 68. * i as f32, 220., 64., 43.)),
                sliders: std::array::from_fn(|i| Rect(908., 278. + 41. * i as f32, 157., 32.)),
                label_width: 107.,
                texture: Rect(800., 605., 264., 45.),
                restore: Rect(800., 661., 264., 47.),
                navigation: [
                    Rect(28., 740., 130., 39.),
                    Rect(164., 740., 132., 39.),
                    Rect(302., 740., 182., 39.),
                ],
                status: (503., 753.),
            }
        }
    }
    pub fn slider(&self, x: f32, y: f32) -> Option<(usize, f32)> {
        self.sliders.iter().position(|r| r.contains(x, y)).map(|i| {
            let Rect(left, _, width, _) = self.sliders[i];
            (i, (x - left) / width)
        })
    }
}

pub enum Action {
    Consumed,
    Pick,
    Previous,
    Next,
    ResetPreview,
}
pub fn button(d: &Hud, x: f32, y: f32, w: f32, h: f32, label: &str) {
    d.rect(x, y, w, h, [39, 71, 95]);
    let size = ((w - 8.) / (label.len() as f32 * 6.)).min(1.5);
    d.text(
        x + (w - label.len() as f32 * 6. * size) * 0.5,
        y + (h - 7. * size) * 0.5,
        label,
        size,
        [46, 66, 54],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_survive_save_reload_and_preserve_other_objects() {
        let root = std::env::temp_dir().join(format!("smash-material-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("materials.txt");
        let mut e = Editor::load(&path, true);
        let key = (42, 0, 2);
        let before = Material::default();
        let layout = Layout::new(1100., false);
        e.handle(&layout, 892., 240., Some((key, before))); // metal preset
        e.handle(&layout, 1000., 296., Some((key, before))); // red channel
        e.save();
        let reloaded = Editor::load(&path, false);
        let changed = reloaded.records[&key].apply(before);
        assert_eq!(changed.metallic, 0.96);
        assert!((changed.color[0] - 92. / 157.).abs() < 1e-5);
        assert!(!changed.use_texture);
        assert!(!reloaded.records.contains_key(&(42, 0, 3)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_invalid_or_duplicate_material_data() {
        let line = "1 0 0 1 0 1 0 0.8 0 1 0 1";
        assert!(parse(&format!("# smash-materials v1\n{line}\n{line}")).is_err());
        assert!(parse("# smash-materials v1\n1 0 0 NaN 0 1 0 0 0 1 0 0").is_err());
    }
}
