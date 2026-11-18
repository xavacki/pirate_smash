mod audio;
mod colliders;
mod effects;
#[cfg(target_os = "android")]
mod filament;
mod levels;
mod material_editor;
mod math;
mod physics;
#[cfg(not(target_os = "android"))]
mod renderer;
mod scene;
mod sdl;
mod start_screen;
mod ui;
use glam::{Mat4, Quat, Vec3};
use levels::{Level, Spec};
use math::{ballistic, rotate, Camera, V};
use physics::World;
use scene::{Material, Scene};
use sdl::Display;
use std::{env, fs};
use ui::Hud;
const BALL_SPEED: f32 = 180.;
const BALL_MASS: f32 = 1.5;
const MUZZLE: V = V::new(0., 4.185_474_4, -33.499_9);
const COLORS: [[u8; 3]; 6] = [
    [244, 118, 91],
    [250, 194, 82],
    [101, 191, 166],
    [112, 160, 229],
    [183, 139, 220],
    [236, 142, 181],
];
struct Body {
    id: i32,
    half: V,
    color: [u8; 3],
    kind: u8,
    object_type: u8,
    dead: bool,
    age: f32,
}
struct Platform {
    id: i32,
    spec: Spec,
    table_index: usize,
}
fn static_material(body: &Body) -> Material {
    Material {
        color: [
            body.color[0] as f32 / 255.,
            body.color[1] as f32 / 255.,
            body.color[2] as f32 / 255.,
            1.,
        ],
        metallic: if body.kind == 2 { 0.7 } else { 0.05 },
        roughness: if body.kind == 2 { 0.23 } else { 0.65 },
        use_texture: false,
        ..Material::default()
    }
}

struct GameplayControls {
    bar: material_editor::Rect,
    materials: material_editor::Rect,
    retry: material_editor::Rect,
}
impl GameplayControls {
    fn new(camera: &Camera) -> Self {
        use material_editor::Rect;
        if camera.portrait() {
            let width = camera.width;
            let half = (width - 34.) / 2.;
            Self {
                bar: Rect(10., 708., width - 20., 78.),
                materials: Rect(14., 740., half, 42.),
                retry: Rect(20. + half, 740., half, 42.),
            }
        } else {
            Self {
                bar: Rect(24., 738., 1052., 42.),
                materials: Rect(688., 740., 146., 37.),
                retry: Rect(978., 740., 91., 37.),
            }
        }
    }
}

struct Game {
    world: World,
    bodies: Vec<Body>,
    effects: effects::Effects,
    level: usize,
    shots: i32,
    total: usize,
    removed: usize,
    time: f32,
    last_shot: f32,
    won: bool,
    failed: bool,
    score: u32,
    levels: Vec<Level>,
    impacted: std::collections::HashSet<i32>,
    platforms: Vec<Platform>,
    clear_y: f32,
    editor: material_editor::Editor,
}
impl Game {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_editor(material_editor::Editor::load(
            std::path::Path::new("smash-materials.txt"),
            false,
        ))
        .expect("valid level data")
    }
    fn with_editor(editor: material_editor::Editor) -> anyhow::Result<Self> {
        let levels = levels::load()?;
        let mut g = Self {
            world: World::new(),
            bodies: vec![],
            effects: effects::Effects::default(),
            level: 0,
            shots: 0,
            total: 0,
            removed: 0,
            time: 0.,
            last_shot: -10.,
            won: false,
            failed: false,
            score: 0,
            levels,
            impacted: std::collections::HashSet::new(),
            platforms: vec![],
            clear_y: -0.3,
            editor,
        };
        g.load(0);
        Ok(g)
    }
    fn add(&mut self, p: V, h: V, m: f32, color: [u8; 3], kind: u8) {
        let id = self.world.cube(p, h, m);
        self.bodies.push(Body {
            id,
            half: h,
            color,
            kind,
            object_type: 0,
            dead: false,
            age: 0.,
        });
    }
    fn object(&mut self, s: Spec) {
        let color = match s.t {
            1..=3 => [46, 146, 215],
            11 => [91, 121, 200],
            12 => [223, 126, 165],
            13 => [240, 195, 78],
            14 => [230, 70, 60],
            15 => [231, 139, 67],
            16 => [166, 111, 192],
            21 | 22 => [163, 167, 162],
            41 | 42 => [181, 137, 86],
            51 => [220, 74, 62],
            61 | 62 => [147, 211, 229],
            _ => COLORS[s.t as usize % 6],
        };
        let id = if let Some(points) = colliders::points(s.t, s.h) {
            self.world.convex(s.p, &points, 1.)
        } else if matches!(s.t, 1 | 2 | 11..=16 | 22 | 42 | 62 | 71) {
            self.world.cylinder(s.p, s.h, 1.)
        } else {
            self.world.cube(s.p, s.h, 1.)
        };
        self.world
            .material(id, if matches!(s.t, 61 | 62) { 0.05 } else { 0.6 });
        self.world.set_pose(id, s.p, s.q);
        self.bodies.push(Body {
            id,
            half: s.h,
            color,
            kind: 1,
            object_type: s.t,
            dead: false,
            age: 0.,
        });
    }
    fn load(&mut self, index: usize) {
        self.world = World::new();
        self.editor.select(None);
        self.impacted.clear();
        self.platforms.clear();
        self.bodies.clear();
        self.effects.clear();
        self.level = index % self.levels.len();
        self.shots = self.levels[self.level].shots;
        self.total = 0;
        self.removed = 0;
        self.time = 0.;
        self.last_shot = -10.;
        self.won = false;
        self.failed = false;
        let lowest = self.levels[self.level]
            .tables
            .iter()
            .map(|t| t.p.y - t.h.y)
            .fold(f32::INFINITY, f32::min);
        self.clear_y = lowest - 0.5;
        self.add(
            V::new(0., lowest - 1.8, 0.),
            V::new(100., 0.3, 100.),
            0.,
            [30, 42, 59],
            0,
        );
        let tables = self.levels[self.level].tables.clone();
        for (table_index, t) in tables.into_iter().enumerate() {
            self.add(t.p, t.h, 0., [225, 213, 185], 0);
            let id = self.bodies.last().unwrap().id;
            self.world.set_pose(id, t.p, t.q);
            if t.motion[0] != 0. || t.motion[2] != 0. || t.motion[7] != 0. {
                self.world.kinematic(id);
                self.platforms.push(Platform {
                    id,
                    spec: t,
                    table_index,
                });
            }
        }
        let boxes = self.levels[self.level].boxes.clone();
        for spec in boxes {
            self.object(spec);
            self.total += 1;
        }
        self.world.prepare_stack();
    }
    fn advance(&mut self) {
        let moves = self.shots;
        let previous = self.levels[self.level].number;
        self.load(self.level + 1);
        if self.levels[self.level].number == previous {
            self.shots = moves;
        }
        let _ = fs::write(
            ".smash-progress",
            self.levels[self.level].number.to_string(),
        );
    }
    fn restart(&mut self) {
        let number = self.levels[self.level].number;
        let index = self
            .levels
            .iter()
            .position(|l| l.number == number)
            .unwrap_or(0);
        self.load(index);
    }
    fn pick(&self, c: &Camera, x: f32, y: f32, include_platforms: bool) -> Option<(usize, V)> {
        let ray = c.ray(x, y);
        let mut nearest = f32::INFINITY;
        let mut target = None;
        let platform_count = self.levels[self.level].tables.len();
        for (index, b) in self.bodies.iter().enumerate() {
            let platform = include_platforms && (1..=platform_count).contains(&index);
            if b.dead || (b.kind != 1 && !platform) {
                continue;
            }
            let (p, q) = self.world.pose(b.id);
            let inverse = [-q[0], -q[1], -q[2], q[3]];
            let origin = rotate(c.eye - p, inverse);
            let direction = rotate(ray, inverse);
            let mut enter = 0.0f32;
            let mut exit = f32::INFINITY;
            for (o, d, h) in [
                (origin.x, direction.x, b.half.x),
                (origin.y, direction.y, b.half.y),
                (origin.z, direction.z, b.half.z),
            ] {
                if d.abs() < 1e-6 {
                    if o.abs() > h {
                        exit = -1.;
                    }
                } else {
                    let t1 = (-h - o) / d;
                    let t2 = (h - o) / d;
                    enter = enter.max(t1.min(t2));
                    exit = exit.min(t1.max(t2));
                }
            }
            if enter <= exit && enter < nearest {
                nearest = enter;
                target = Some((index, c.eye + ray * enter));
            }
        }
        target
    }
    fn target(&self, c: &Camera, x: f32, y: f32) -> V {
        self.pick(c, x, y, false)
            .map_or_else(|| c.target(x, y), |(_, p)| p)
    }
    fn material_info(
        &self,
        scene: &Scene,
        index: usize,
    ) -> Option<(material_editor::ObjectKey, Material)> {
        let body = self.bodies.get(index)?;
        if body.dead {
            return None;
        }
        let level = &self.levels[self.level];
        if (1..=level.tables.len()).contains(&index) {
            let key = (level.number, level.stage, level.boxes.len() + index - 1);
            return Some((key, scene.primitives[*scene.dock.first()?].material));
        }
        if body.kind != 1 {
            return None;
        }
        let ordinal = index.checked_sub(level.tables.len() + 1)?;
        let m = scene
            .object_visual(body.object_type, body.half)
            .and_then(|(parts, _)| parts.first().map(|&p| scene.primitives[p].material))
            .unwrap_or_else(|| {
                let [r, g, b] = glow_color(body.object_type);
                Material {
                    color: [r, g, b, 0.10],
                    roughness: 0.14,
                    glass: true,
                    blend: true,
                    ..Material::default()
                }
            });
        Some(((level.number, level.stage, ordinal), m))
    }
    fn editor_click(&mut self, scene: &Scene, c: &Camera, x: f32, y: f32) {
        use material_editor::Action;
        let selected = self
            .editor
            .selected
            .and_then(|i| self.material_info(scene, i));
        let layout = material_editor::Layout::new(c.width, c.portrait());
        match self.editor.handle(&layout, x, y, selected) {
            Action::Pick => self.editor.select(self.pick(c, x, y, true).map(|(i, _)| i)),
            Action::Previous => self.load(self.level.saturating_sub(1)),
            Action::Next => self.load((self.level + 1).min(self.levels.len() - 1)),
            Action::ResetPreview => self.load(self.level),
            Action::Consumed => {}
        }
    }
    fn camera(&self) -> Camera {
        let mut c = Camera::new();
        c.eye = c.eye + self.scene_offset();
        c
    }
    fn scene_offset(&self) -> V {
        V::new(0., self.levels[self.level].world_y_offset, 0.)
    }
    fn fire(&mut self, target: V) -> bool {
        if self.shots <= 0 || self.won || self.failed {
            return false;
        }
        let start = MUZZLE + self.scene_offset();
        let Some(v) = ballistic(start, target, BALL_SPEED, 9.81, false) else {
            return false;
        };
        if self.last_shot < 0. {
            self.world.prepare_stack();
        }
        let id = self.world.sphere(start, 0.3, BALL_MASS);
        self.world.velocity(id, v);
        self.bodies.push(Body {
            id,
            half: V::new(0.3, 0.3, 0.3),
            color: [65, 74, 84],
            kind: 2,
            object_type: 0,
            dead: false,
            age: 0.,
        });
        self.shots -= 1;
        self.last_shot = self.time;
        true
    }
    fn update(&mut self, dt: f32) {
        self.time += dt;
        let waiting_for_shot = self.last_shot < 0.;
        let mut support_poses = vec![];
        for platform in &self.platforms {
            let s = &platform.spec;
            let m = s.motion;
            let mut p = s.p;
            let mut q = s.q;
            // Motion units and direction signs are an approximation until runtime comparison.
            let offset = |low: f32, high: f32, speed: f32, dir: f32| {
                let span = (high - low).abs();
                if span < 1e-5 {
                    return 0.;
                }
                let t = (self.time * speed.abs() + span * 0.5).rem_euclid(span * 2.);
                let value = if t < span { t } else { 2. * span - t };
                (value - span * 0.5) * if dir == 0. { 1. } else { -1. }
            };
            if m[2] != 0. {
                p.x += offset(m[3], m[4], m[6], m[5]);
            }
            if m[7] != 0. {
                p.y += offset(m[8], m[9], m[11], m[10]);
            }
            if m[0] != 0. {
                let angle = (self.time * m[1]).to_radians() * 0.5;
                let spin = [0., angle.sin(), 0., angle.cos()];
                q = math::quat_mul(spin, q);
            }
            self.world.set_pose(platform.id, p, q);
            if waiting_for_shot {
                support_poses.push((platform.table_index, p, q));
            }
        }
        if waiting_for_shot {
            // Keep the authored puzzle at equilibrium until play begins. Unhit moving
            // stacks follow their support; gravity and collisions run after the first shot.
            for (body, spec) in self
                .bodies
                .iter()
                .filter(|b| b.kind == 1)
                .zip(&self.levels[self.level].boxes)
            {
                if let Some(&(_, p, q)) = support_poses
                    .iter()
                    .find(|(index, _, _)| *index == spec.support)
                {
                    let base = &self.levels[self.level].tables[spec.support];
                    let inverse = [-base.q[0], -base.q[1], -base.q[2], base.q[3]];
                    let delta = math::quat_mul(q, inverse);
                    self.world.set_pose(
                        body.id,
                        p + rotate(spec.p - base.p, delta),
                        math::quat_mul(delta, spec.q),
                    );
                }
            }
            self.world.sync_kinematics();
            return;
        }
        self.world.step(dt);
        let contacts = self.world.contacts();
        let mut breaks = vec![];
        let mut blasts = vec![];
        for (a, b, force) in contacts {
            if force < 2.5 {
                continue;
            }
            let ball_a = self
                .bodies
                .iter()
                .any(|x| x.id == a && x.kind == 2 && !x.dead);
            let ball_b = self
                .bodies
                .iter()
                .any(|x| x.id == b && x.kind == 2 && !x.dead);
            if ball_a || ball_b {
                let ball = if ball_a { a } else { b };
                if self.impacted.insert(ball) {
                    blasts.push(self.world.pose(ball).0);
                }
                let victim = if ball_a { b } else { a };
                if self
                    .bodies
                    .iter()
                    .any(|x| x.id == victim && x.kind == 1 && !x.dead)
                {
                    breaks.push(victim);
                }
            }
        }
        for origin in blasts {
            for b in &self.bodies {
                if b.dead || b.kind != 1 {
                    continue;
                }
                let (p, _) = self.world.pose(b.id);
                let d = p - origin;
                let distance = d.len();
                if distance < 2.5 {
                    self.world.impulse(
                        b.id,
                        (d.unit() + V::new(0., 0.3, 0.)) * (3. * (1. - distance / 2.5)),
                    );
                }
            }
        }
        for body in &mut self.bodies {
            if body.dead {
                continue;
            }
            body.age += dt;
            let (p, q) = self.world.pose(body.id);
            let off = body.kind == 1 && (p.y < self.clear_y || p.x.abs() > 50. || p.z.abs() > 50.);
            if body.kind == 1 && (off || breaks.contains(&body.id)) {
                body.dead = true;
                self.world.remove(body.id);
                self.removed += 1;
                self.score += 100;
                self.effects.burst(body.object_type, p, body.half, q, off);
            } else if body.kind == 2 && (body.age > 6. || p.y < -3.) {
                body.dead = true;
                self.world.remove(body.id);
            }
        }
        self.effects.update(dt);
        if self.removed == self.total {
            self.won = true;
        }
        if self.shots == 0 && self.time - self.last_shot > 6.5 && !self.won {
            self.failed = true;
        }
    }
    fn frame(&self, scene: &mut Scene, c: &Camera) -> (Vec<Mat4>, Vec<ui::UiVertex>) {
        scene.nodes.clear();
        let mut world = vec![];
        let object_visuals: Vec<_> = self
            .bodies
            .iter()
            .map(|b| scene.object_visual(b.object_type, b.half))
            .collect();
        let level = &self.levels[self.level];
        // A camera-facing environment plate leaves all cargo and moving docks in 3D.
        let v = |p: V| Vec3::new(p.x, p.y, p.z);
        let distance = 500.;
        let height = 800. * distance / c.zoom;
        let model = Mat4::from_cols(
            (v(c.right) * (height * c.width / 800.)).extend(0.),
            (v(c.up) * height).extend(0.),
            (-v(c.forward)).extend(0.),
            v(c.eye + c.forward * distance).extend(1.),
        );
        scene.instance(&mut world, scene.backdrop, model, None);
        scene.instance(
            &mut world,
            scene.water,
            Mat4::from_translation(v(c.eye)),
            None,
        );
        let primitives = &scene.primitives;
        let nodes = &mut scene.nodes;
        let mut instance =
            |primitive: usize, p: V, h: V, q: [f32; 4], material: Option<Material>| {
                let model = Mat4::from_scale_rotation_translation(
                    Vec3::new(h.x * 2., h.y * 2., h.z * 2.),
                    Quat::from_array(q),
                    Vec3::new(p.x, p.y, p.z),
                );
                nodes.push(scene::Node {
                    primitives: vec![primitive],
                    material,
                });
                world.push(model);
            };
        for (body_index, b) in self.bodies.iter().enumerate() {
            if b.dead || body_index == 0 {
                continue;
            }
            let (p, q) = self.world.pose(b.id);
            if b.kind == 1 {
                if let Some((parts, h)) = &object_visuals[body_index] {
                    for &primitive in parts {
                        let key = (
                            level.number,
                            level.stage,
                            body_index - level.tables.len() - 1,
                        );
                        let material = self
                            .editor
                            .records
                            .get(&key)
                            .map(|m| m.apply(primitives[primitive].material));
                        instance(primitive, p, *h, q, material);
                    }
                    continue;
                }
                let (face, frame) = match b.object_type {
                    1 | 22 | 42 | 62 | 71 => (scene::CYLINDER, scene::FRAME_CYLINDER),
                    2 => (scene::CONE, scene::FRAME_CONE),
                    11..=16 => (scene::GLASS_JAR, scene::FRAME_JAR),
                    _ => (scene::CUBE, scene::FRAME_CUBE),
                };
                let (shape_half, shape_q) = round_shape_pose(b.object_type, b.half, q);
                let color = glow_color(b.object_type);
                let key = (
                    level.number,
                    level.stage,
                    body_index - level.tables.len() - 1,
                );
                let finish = self.editor.records.get(&key);
                if let Some(finish) = finish {
                    instance(
                        face,
                        p,
                        shape_half,
                        shape_q,
                        Some(finish.apply(Material {
                            color: [color[0], color[1], color[2], 0.10],
                            roughness: 0.14,
                            glass: true,
                            blend: true,
                            ..Material::default()
                        })),
                    );
                    continue;
                }
                instance(
                    face,
                    p,
                    shape_half,
                    shape_q,
                    Some(Material {
                        color: [color[0], color[1], color[2], 0.10],
                        metallic: 0.,
                        roughness: 0.14,
                        blend: true,
                        glass: true,
                        ..Material::default()
                    }),
                );
                instance(
                    frame,
                    p,
                    shape_half,
                    shape_q,
                    Some(Material {
                        color: [0., 0., 0., 1.],
                        emissive: color.map(|c| c * 5.),
                        unlit: true,
                        ..Material::default()
                    }),
                );
            } else {
                if (1..=level.tables.len()).contains(&body_index) {
                    let key = (
                        level.number,
                        level.stage,
                        level.boxes.len() + body_index - 1,
                    );
                    for &primitive in &scene.dock {
                        let material = self
                            .editor
                            .records
                            .get(&key)
                            .map(|m| m.apply(primitives[primitive].material));
                        instance(primitive, p, b.half, q, material);
                    }
                    continue;
                }
                let primitive = if b.kind == 2 {
                    scene::SPHERE
                } else {
                    scene::CUBE
                };
                let mut material = static_material(b);
                if (1..=level.tables.len()).contains(&body_index) {
                    let key = (
                        level.number,
                        level.stage,
                        level.boxes.len() + body_index - 1,
                    );
                    if let Some(finish) = self.editor.records.get(&key) {
                        material = finish.apply(material);
                    }
                }
                instance(primitive, p, b.half, q, Some(material));
            }
        }
        for &part in &scene.cannon_base {
            instance(
                part,
                V::new(0., 3.1, -35.) + self.scene_offset(),
                V::new(1.6, 1.6, 1.6),
                [0., 0., 0., 1.],
                None,
            );
        }
        for &part in &scene.cannon_barrel {
            instance(
                part,
                V::new(0., 3.8, -35.) + self.scene_offset(),
                V::new(1.6, 1.6, 1.6),
                [0., 0., 0., 1.],
                None,
            );
        }
        // Give the shot a visible cue at the cannon while the ball is in flight.
        let flash_age = self.time - self.last_shot;
        const FLASH_DURATION: f32 = 0.115;
        if (0. ..FLASH_DURATION).contains(&flash_age) {
            let progress = flash_age / FLASH_DURATION;
            let size = 0.48 + progress * 0.52;
            let center = MUZZLE + self.scene_offset() + V::new(0., 0.25, 0.85);
            let model = Mat4::from_cols(
                (v(c.right) * size).extend(0.),
                (v(c.up) * size * 1.25).extend(0.),
                (-v(c.forward)).extend(0.),
                v(center).extend(1.),
            );
            scene.instance(
                &mut world,
                scene::PUFF,
                model,
                Some(Material {
                    color: [1., 0.58, 0.16, (1. - progress).powi(2)],
                    emissive: [1.85, 0.65, 0.10],
                    unlit: true,
                    blend: true,
                    ..Material::default()
                }),
            );
        }
        let d = Hud::default();
        if self.editor.active {
            if let Some(i) = self.editor.selected {
                if let Some((point, _)) = c.project(self.world.pose(self.bodies[i].id).0) {
                    d.selection_brackets(point);
                }
            }
            self.effects.frame(scene, &mut world, c);
            let selected = self
                .editor
                .selected
                .and_then(|i| self.material_info(scene, i));
            d.vertices.borrow_mut().extend(self.editor.overlay(
                &material_editor::Layout::new(c.width, c.portrait()),
                selected,
                level.number,
                level.stage,
                level.boxes.len(),
            ));
            return (world, d.vertices.into_inner());
        }
        let controls = GameplayControls::new(c);
        if c.portrait() {
            d.rect(10., 16., c.width - 20., 66., [248, 250, 241]);
            d.sprite(
                [c.width / 2. - 21., 3., 42., 42.],
                [1920., 2304., 256., 256.],
                [1.; 4],
            );
            d.text(
                20.,
                27.,
                &format!("LEVEL {:02}", level.number),
                1.9,
                [46, 66, 54],
            );
            d.text(
                20.,
                52.,
                &format!(
                    "STAGE {}/{}   {}/{}",
                    level.stage + 1,
                    level.stage_count,
                    self.removed,
                    self.total
                ),
                1.42,
                [104, 119, 107],
            );
            d.text(
                c.width - 105.,
                27.,
                &format!("{} SHOTS", self.shots),
                1.7,
                [46, 66, 54],
            );
            d.text(
                c.width - 105.,
                52.,
                &format!("SCORE {}", self.score),
                1.42,
                [104, 119, 107],
            );
        } else {
            d.rect(24., 22., 1052., 88., [248, 250, 241]);
            d.text(44., 38., "PIRATE SMASH", 3., [46, 66, 54]);
            d.text(46., 77., "CLAIM THE TREASURE", 1., [104, 119, 107]);
            d.text(
                470.,
                40.,
                &format!("LEVEL {:02}", self.levels[self.level].number),
                2.,
                [46, 66, 54],
            );
            d.text(
                470.,
                74.,
                &format!(
                    "STAGE {}/{}  {}/{}",
                    self.levels[self.level].stage + 1,
                    self.levels[self.level].stage_count,
                    self.removed,
                    self.total
                ),
                1.5,
                [104, 119, 107],
            );
            d.text(
                810.,
                40.,
                &format!("{} SHOTS", self.shots),
                2.,
                [46, 66, 54],
            );
        }
        let material_editor::Rect(x, y, w, h) = controls.bar;
        d.rect(x, y, w, h, [248, 250, 241]);
        if c.portrait() {
            d.text(20., 721., "TAP TO FIRE", 1.4, [68, 88, 72]);
        } else {
            d.text(
                42.,
                751.,
                "CLICK TO FIRE     R: RESTART     E: MATERIALS",
                1.5,
                [68, 88, 72],
            );
            d.text(
                850.,
                751.,
                &format!("SCORE {}", self.score),
                1.5,
                [68, 88, 72],
            );
        }
        controls.materials.button(&d, "MATERIALS");
        controls.retry.button(&d, "RETRY");
        if self.won || self.failed {
            let width = 480_f32.min(c.width - 24.);
            let x = (c.width - width) / 2.;
            d.rect(x, 285., width, 175., [248, 250, 241]);
            d.text(
                x + 22.,
                314.,
                if self.won {
                    "BOOTY CLAIMED!"
                } else {
                    "OUT OF SHOTS"
                },
                3.,
                [46, 66, 54],
            );
            d.text(
                x + 22.,
                370.,
                if self.won {
                    "TAP TO CONTINUE"
                } else {
                    "TAP TO TRY AGAIN"
                },
                2.,
                [104, 119, 107],
            );
            d.text(
                x + 22.,
                414.,
                &format!("{} OBJECTS CLEARED", self.removed),
                1.5,
                [104, 119, 107],
            );
        }
        self.effects.frame(scene, &mut world, c);
        (world, d.vertices.into_inner())
    }
}
fn round_shape_pose(kind: u8, h: V, q: [f32; 4]) -> (V, [f32; 4]) {
    if !matches!(kind, 1 | 11..=16 | 22 | 42 | 62) {
        return (h, q);
    }
    let (h, axis) = if h.x > h.y && h.x >= h.z {
        (
            V::new(h.y, h.x, h.z),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        )
    } else if h.z > h.y && h.z > h.x {
        (
            V::new(h.x, h.z, h.y),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        )
    } else {
        return (h, q);
    };
    (h, (Quat::from_array(q) * axis).to_array())
}
fn glow_color(kind: u8) -> [f32; 3] {
    match kind {
        1 => [0.12, 0.85, 1.],
        2 => [1., 0.25, 0.6],
        3 => [0.65, 0.3, 1.],
        11 => [0.22, 0.45, 1.],
        12 => [1., 0.3, 0.65],
        13 => [1., 0.8, 0.2],
        14 => [1., 0.14, 0.1],
        15 => [1., 0.48, 0.15],
        16 => [0.7, 0.28, 1.],
        31 => [0.6, 0.15, 1.],
        21 | 22 => [0.95, 0.5, 0.2],
        41 | 42 => [1., 0.7, 0.2],
        61 | 62 => [0.3, 0.9, 1.],
        _ => [0.2, 1., 0.75],
    }
}
pub fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    let headless = args.iter().any(|a| a == "--headless");
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let material_path = args
        .windows(2)
        .find(|p| p[0] == "--materials")
        .map_or("smash-materials.txt", |p| p[1].as_str());
    let mut game = Game::with_editor(material_editor::Editor::load(
        std::path::Path::new(material_path),
        args.iter().any(|a| a == "--editor"),
    ))
    .map_err(|error| format!("{error:#}"))?;
    if let Some(value) = args.windows(2).find(|p| p[0] == "--audit-starts") {
        let moving_only = args.iter().any(|a| a == "--audit-moving");
        let mut rows = vec![];
        let mut losses = 0;
        let mut affected = 0;
        for i in 0..game.levels.len() {
            if moving_only
                && !game.levels[i]
                    .tables
                    .iter()
                    .any(|t| t.motion[0] != 0. || t.motion[2] != 0. || t.motion[7] != 0.)
            {
                continue;
            }
            game.load(i);
            for _ in 0..240 {
                game.update(1. / 120.);
            }
            losses += game.removed;
            affected += usize::from(game.removed > 0);
            rows.push(format!(
                "{{\"level\":{},\"objects\":{},\"lost\":{}}}",
                game.levels[i].number, game.total, game.removed
            ));
            if rows.len() % 50 == 0 {
                eprintln!("Audited {} gameplay starts", rows.len());
            }
        }
        fs::write(&value[1],format!("{{\"seconds\":2,\"moving_only\":{moving_only},\"levels\":{},\"levels_with_losses\":{affected},\"objects_lost\":{losses},\"results\":[{}]}}\n",rows.len(),rows.join(",\n"))).map_err(|e|e.to_string())?;
        println!(
            "Audited {} gameplay starts: {affected} levels lost {losses} objects before firing",
            rows.len()
        );
        return Ok(());
    }
    if let Some(value) = args.windows(2).find(|p| p[0] == "--level") {
        let requested = value[1].parse::<usize>().map_err(|_| "invalid --level")?;
        let index = game
            .levels
            .iter()
            .position(|l| l.number == requested)
            .ok_or_else(|| format!("level {requested} not found"))?;
        game.load(index);
    } else if !headless && !smoke {
        if let Ok(progress) = fs::read_to_string(".smash-progress") {
            if let Ok(number) = progress.trim().parse::<usize>() {
                if let Some(index) = game.levels.iter().position(|l| l.number == number) {
                    game.load(index);
                }
            }
        }
    }
    if args.iter().any(|a| a == "--validate-levels") {
        let stages = game.levels.len();
        let count: usize = game
            .levels
            .iter()
            .map(|l| l.boxes.len() + l.tables.len())
            .sum();
        println!(
            "Validated {} levels, {stages} stages, {count} body definitions",
            game.levels.last().unwrap().number
        );
        return Ok(());
    }
    if headless {
        let n = game.total;
        game.fire(V::new(0., 1.5, 0.));
        for _ in 0..1000 {
            game.update(1. / 120.);
        }
        println!(
            "Bullet simulation: level={} bodies={} cleared={} shots={}",
            game.level + 1,
            n,
            game.removed,
            game.shots
        );
        if game.removed == 0 {
            return Err("smoke shot did not hit or dislodge any object".into());
        }
        return Ok(());
    }
    let gles = cfg!(target_os = "android") || args.iter().any(|a| a == "--gles");
    let check_video = args.iter().any(|a| a == "--check-video");
    let display = Display::new(gles, check_video)?;
    let mut cannon_audio = audio::CannonAudio::new()
        .map_err(|error| eprintln!("Cannon audio unavailable: {error}"))
        .ok();
    #[cfg(target_os = "android")]
    sdl::log_info("Smash: using Filament");
    let mut scene = Scene::load().map_err(|e| format!("{e:#}"))?;
    let value = |name: &str| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone());
    let effect_time = value("--effects-preview")
        .map(|s| s.parse::<f32>())
        .transpose()
        .map_err(|_| "invalid --effects-preview")?;
    if effect_time.is_some_and(|t| !t.is_finite() || !(0.0..=3.).contains(&t)) {
        return Err("--effects-preview expects a time from 0 through 3 seconds".into());
    }
    if let Some(time) = effect_time {
        game.effects.clear();
        for (i, kind) in [1, 11, 21, 41, 61, 51].into_iter().enumerate() {
            game.effects.burst(
                kind,
                effect_preview_position(i),
                V::new(0.5, 0.5, 0.5),
                [0., 0., 0., 1.],
                false,
            );
        }
        let steps = (time * 120.).ceil() as usize;
        for _ in 0..steps {
            game.effects.update(time / steps as f32);
        }
        eprintln!(
            "Effect preview: {} live particles at {time:.3}s",
            game.effects.active_count()
        );
    }
    let samples = value("--msaa")
        .map(|s| s.parse::<i32>())
        .transpose()
        .map_err(|_| "invalid --msaa")?
        .unwrap_or(4);
    if ![0, 2, 4, 8].contains(&samples) {
        return Err("--msaa must be 0, 2, 4 or 8".into());
    }
    #[cfg(not(target_os = "android"))]
    let mut renderer = renderer::Renderer::new(
        display.gl(),
        &scene,
        args.iter().any(|a| a == "--ldr"),
        samples,
    )
    .map_err(|e| format!("{e:#}"))?;
    #[cfg(target_os = "android")]
    let mut renderer = filament::Renderer::new(&scene, samples).map_err(|e| format!("{e:#}"))?;
    let mut settings = ui::Settings::default();
    if let Some(s) = value("--strength") {
        settings.strength = s.parse().map_err(|_| "invalid --strength")?;
        if !settings.strength.is_finite() || !(0.0..=3.0).contains(&settings.strength) {
            return Err("--strength must be between 0 and 3".into());
        }
    }
    let frame_limit = value("--frames")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| "invalid --frames")?;
    if frame_limit == Some(0) {
        return Err("--frames must be positive".into());
    }
    let no_ui = args.iter().any(|a| a == "--no-ui");
    let menu_test = args.iter().any(|a| a == "--start-screen-test");
    let menu_time = value("--menu-time")
        .map(|s| s.parse::<f32>())
        .transpose()
        .map_err(|_| "invalid --menu-time")?;
    if menu_time.is_some_and(|t| !t.is_finite() || t < 0.) {
        return Err("--menu-time must be finite and nonnegative".into());
    }
    let mut menu = start_screen::StartScreen::new(
        (!args.iter().any(|a| a == "--level") && effect_time.is_none())
            || args.iter().any(|a| a == "--start-screen")
            || menu_time.is_some()
            || menu_test,
    );
    if let Some(t) = menu_time {
        menu.seek(t);
    }
    if game.editor.active {
        menu.phase = start_screen::Phase::Game;
    }

    let mut event = sdl::Event([0; 128]);
    let mut last = unsafe { sdl::SDL_GetTicks() };
    let mut accumulator = 0.;
    let mut editing_drag = false;
    #[cfg(target_os = "android")]
    let mut background = false;
    let mut mouse = (display.view_width() / 2., 400.);
    let mut frames = 0;
    let shot_path = args
        .windows(2)
        .find(|p| p[0] == "--screenshot")
        .map(|p| p[1].clone());
    'running: loop {
        if menu_test {
            if frames == 0 {
                display.push_click(display.view_width() / 2., 400.)?;
            }
            if frames == 30 {
                display.push_click(display.view_width() / 2., 622.)?;
            }
        }
        let view_width = display.view_width();
        let mut camera = game.camera();
        camera.resize(view_width, game.editor.active);
        let editor_layout = material_editor::Layout::new(view_width, camera.portrait());
        let controls = GameplayControls::new(&camera);
        unsafe {
            while sdl::SDL_PollEvent(&mut event) {
                match event.kind() {
                    0x100 => break 'running,
                    #[cfg(target_os = "android")]
                    0x103 | 0x104 => {
                        background = true;
                        renderer.detach();
                        editing_drag = false;
                    }
                    #[cfg(target_os = "android")]
                    0x106 => {
                        background = false;
                        last = sdl::SDL_GetTicks();
                    }
                    0x300 => match event.key() {
                        27 | 0x4000011a if game.editor.active => {
                            game.editor.active = false;
                        }
                        101 if !menu.active() => {
                            game.editor.active = !game.editor.active;
                            game.load(game.level);
                        }
                        27 | 0x4000011a => {
                            if !menu.escape() {
                                break 'running;
                            }
                        }
                        13 | 32 if menu.active() => menu.begin(),
                        114 if !menu.active() => game.restart(),
                        110 if !menu.active() => game.advance(),
                        98 => settings.strength = if settings.strength > 0. { 0. } else { 0.35 },
                        _ => {}
                    },
                    0x400 => {
                        mouse = display.mouse(&event, view_width);
                        if editing_drag
                            && game.editor.active
                            && editor_layout.slider(mouse.0, mouse.1).is_some()
                        {
                            game.editor_click(&scene, &camera, mouse.0, mouse.1);
                        }
                    }
                    0x402 => editing_drag = false,
                    0x401 => {
                        if event.0[24] != 1 {
                            continue;
                        }
                        mouse = display.mouse(&event, view_width);
                        if menu.active() {
                            if menu.click_view(mouse.0, mouse.1, view_width) {
                                settings.strength = if settings.strength > 0. { 0. } else { 0.35 };
                            }
                        } else if game.editor.active {
                            editing_drag = editor_layout.slider(mouse.0, mouse.1).is_some();
                            game.editor_click(&scene, &camera, mouse.0, mouse.1);
                        } else if controls.bar.contains(mouse.0, mouse.1) {
                            if controls.materials.contains(mouse.0, mouse.1) {
                                game.load(game.level);
                                game.editor.active = true;
                            } else if controls.retry.contains(mouse.0, mouse.1) {
                                game.restart();
                            }
                        } else if game.failed {
                            game.restart();
                        } else if game.won {
                            game.advance()
                        } else if game.fire(game.target(&camera, mouse.0, mouse.1)) {
                            if let Some(audio) = &mut cannon_audio {
                                if let Err(error) = audio.play() {
                                    eprintln!("Could not play cannon sound: {error}");
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let now = unsafe { sdl::SDL_GetTicks() };
        let dt = if menu_test {
            1. / 60.
        } else {
            ((now - last) as f32 / 1000.).min(0.1)
        };
        last = now;
        if menu_time.is_none() {
            menu.tick(dt);
        }
        if menu.active() || effect_time.is_some() || game.editor.active {
            accumulator = 0.;
        } else {
            accumulator += dt;
        }
        while accumulator >= 1. / 120. {
            game.update(1. / 120.);
            accumulator -= 1. / 120.;
        }
        let mut camera = game.camera();
        camera.resize(display.view_width(), game.editor.active);
        if effect_time.is_some() {
            camera.eye = V::new(0., 6., -27.);
        }
        #[cfg(target_os = "android")]
        if background || !renderer.attach(&display)? {
            unsafe { sdl::SDL_Delay(16) };
            continue;
        }
        let (pixels, _) = display.sizes();
        if pixels.0 == 0 || pixels.1 == 0 {
            unsafe { sdl::SDL_Delay(16) };
            continue;
        }
        let (mut world, mut hud) = if effect_time.is_some() {
            scene.nodes.clear();
            let mut world = vec![];
            game.effects.frame(&mut scene, &mut world, &camera);
            let hud = Hud::default();
            for (i, name) in ["METAL", "JAM JAR", "STONE", "WOOD", "ICE", "TNT"]
                .into_iter()
                .enumerate()
            {
                if let Some((p, _)) = camera.project(effect_preview_position(i)) {
                    hud.text(
                        p.x - name.len() as f32 * 6.,
                        p.y + 100.,
                        name,
                        2.,
                        [220, 230, 245],
                    );
                }
            }
            (world, hud.vertices.into_inner())
        } else if menu.active() && menu.phase != start_screen::Phase::Leaving {
            scene.nodes.clear();
            (vec![], vec![])
        } else {
            game.frame(&mut scene, &camera)
        };
        if menu.active() {
            hud.extend(menu.frame(
                game.levels[game.level].number,
                mouse,
                settings.strength > 0.,
                camera.width,
            ));
        }
        // Diagnostic: opaque output should not depend on submission order with a depth buffer.
        if args.iter().any(|a| a == "--reverse-draw") {
            scene.nodes.reverse();
            world.reverse();
        }
        #[cfg(not(target_os = "android"))]
        renderer
            .render(
                &scene,
                &world,
                &camera,
                now as f32 / 1000.,
                &settings,
                pixels,
                (camera.width, 800.),
                if no_ui { &[] } else { &hud },
            )
            .map_err(|e| format!("{e:#}"))?;
        #[cfg(target_os = "android")]
        renderer
            .render(
                &scene,
                &world,
                &camera,
                &settings,
                pixels,
                if no_ui { &[] } else { &hud },
            )
            .map_err(|e| format!("{e:#}"))?;
        frames += 1;
        if menu_test && frames == 60 {
            if menu.active() || game.shots != game.levels[game.level].shots {
                return Err("Start-screen click flow failed or consumed a gameplay shot".into());
            }
            eprintln!("Start-screen SDL click test passed: splash -> home -> gameplay, no accidental shot");
            break;
        }
        #[cfg(target_os = "android")]
        let _ = &shot_path;
        let done = check_video
            || frame_limit.is_some_and(|n| frames >= n)
            || (smoke && frames >= 5)
            || (shot_path.is_some() && frame_limit.is_none() && frames >= 3);
        #[cfg(not(target_os = "android"))]
        if done {
            if let Some(path) = &shot_path {
                renderer
                    .screenshot(path, pixels.0, pixels.1)
                    .map_err(|e| format!("{e:#}"))?;
            }
        }
        #[cfg(not(target_os = "android"))]
        display.present()?;
        if done {
            break;
        }
    }
    Ok(())
}
fn effect_preview_position(i: usize) -> V {
    V::new(5. - (i % 3) as f32 * 5., if i < 3 { 7.5 } else { 2.5 }, 0.)
}

#[cfg(test)]
mod gameplay_tests {
    use super::*;
    #[test]
    fn moving_start_stack_follows_support_then_responds_to_a_shot() {
        let mut game = Game::new();
        let index = game.levels.iter().position(|l| l.number == 81).unwrap();
        game.load(index);
        let before: Vec<_> = game
            .bodies
            .iter()
            .filter(|b| b.kind == 1)
            .map(|b| game.world.pose(b.id).0)
            .collect();
        for _ in 0..120 {
            game.update(1. / 120.);
        }
        let targets: Vec<_> = game
            .bodies
            .iter()
            .filter(|b| b.kind == 1)
            .map(|b| game.world.pose(b.id).0)
            .collect();
        assert!(
            before
                .iter()
                .zip(&targets)
                .any(|(a, b)| (*a - *b).len() > 0.1),
            "rotating support did not carry its stack"
        );
        assert_eq!(game.removed, 0);
        game.fire(targets[0]);
        for _ in 0..240 {
            game.update(1. / 120.);
        }
        assert!(
            game.removed > 0,
            "moving start stack remained locked after the first shot"
        );
    }
    #[test]
    fn previously_unstable_stacks_do_not_clear_themselves_at_start() {
        let mut game = Game::new();
        for number in [169, 508, 515, 593, 595, 619, 707, 800, 857, 980] {
            let index = game.levels.iter().position(|l| l.number == number).unwrap();
            game.load(index);
            for (b, spec) in game
                .bodies
                .iter()
                .filter(|b| b.kind == 1)
                .zip(&game.levels[index].boxes)
            {
                let p = game.world.pose(b.id).0;
                assert!(
                    (p - spec.p).len() < 0.0001,
                    "level {number} changed before first frame"
                );
            }
            for _ in 0..600 {
                game.update(1. / 120.);
            }
            assert_eq!(
                game.removed, 0,
                "level {number} lost objects without firing"
            );
        }
    }
    #[test]
    fn original_speed_reaches_stack_in_under_point_three_seconds() {
        let mut game = Game::new();
        game.fire(V::new(0., 1.5, 0.));
        for _ in 0..36 {
            game.update(1. / 120.);
            if game.removed > 0 {
                assert!(
                    game.effects.active_count() > 0,
                    "destruction did not emit particles"
                );
                return;
            }
        }
        panic!("180 units/s shot did not hit the stack within 0.3 seconds");
    }
    #[test]
    fn fast_ball_does_not_tunnel_through_thin_target() {
        let world = World::new();
        world.cube(V::new(0., 0., 0.), V::new(1., 1., 0.015), 0.);
        let ball = world.sphere(V::new(0., 0., -1.), 0.3, BALL_MASS);
        world.velocity(ball, V::new(0., 0., BALL_SPEED));
        for _ in 0..3 {
            world.step(1. / 120.);
        }
        assert!(
            world.pose(ball).0.z < 0.,
            "CCD failed at the original launch speed"
        );
    }
    #[test]
    fn consecutive_clicks_are_not_blocked_by_a_port_only_cooldown() {
        let mut game = Game::new();
        game.fire(V::new(0., 1.5, 0.));
        game.fire(V::new(1., 1.5, 0.));
        assert_eq!(game.shots, 18);
    }
    #[test]
    fn target_objects_use_recovered_texture_and_uv_meshes() {
        let game = Game::new();
        let mut scene = Scene::load().unwrap();
        game.frame(&mut scene, &game.camera());
        let targets: Vec<_> = scene
            .nodes
            .iter()
            .flat_map(|n| &n.primitives)
            .filter(|&&id| id > scene::PUFF)
            .collect();
        assert!(targets.len() >= game.total);
        assert!(targets
            .iter()
            .all(|&&id| scene.primitives[id].texture.is_some()));
        for kind in [
            1, 2, 3, 11, 12, 13, 14, 15, 16, 21, 22, 31, 41, 42, 51, 61, 62,
        ] {
            let (parts, _) = scene.object_visual(kind, V::new(0.5, 0.5, 0.5)).unwrap();
            assert!(!parts.is_empty());
            for id in parts {
                let mesh = &scene.primitives[id];
                assert!(mesh.texture.is_some());
                assert!(
                    mesh.vertices.iter().any(|v| v.uv != [0.; 2]),
                    "missing source UVs for {kind}"
                );
            }
        }
    }
    #[test]
    fn playable_first_level() {
        let mut game = Game::new();
        assert_eq!(game.total, 18);
        assert_eq!(game.shots, 20);
        for _ in 0..20 {
            if game.won {
                break;
            }
            let target = game
                .bodies
                .iter()
                .find(|b| b.kind == 1 && !b.dead)
                .map(|b| game.world.pose(b.id).0);
            if let Some(target) = target {
                game.fire(target);
            }
            for _ in 0..300 {
                game.update(1. / 120.);
            }
        }
        assert!(
            game.won,
            "first level could not be cleared: {}/{}",
            game.removed, game.total
        );
        game.restart();
        assert_eq!(game.removed, 0);
        assert_eq!(game.shots, 20);
        assert!(!game.won);
        // Do not write progress during an automated test.
        game.load(1);
        assert_eq!(game.levels[game.level].number, 2);
        assert_eq!(game.total, 21);
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn SDL_main(_argc: i32, _argv: *mut *mut std::ffi::c_char) -> i32 {
    use std::ffi::{c_char, CStr, CString};
    extern "C" {
        fn SDL_SetMainReady();
        fn SDL_GetAndroidInternalStoragePath() -> *const c_char;
        fn SDL_LogMessage(category: i32, priority: i32, format: *const c_char, ...);
    }
    let result = (|| {
        unsafe {
            SDL_SetMainReady();
            let path = SDL_GetAndroidInternalStoragePath();
            if path.is_null() {
                return Err("Android private storage is unavailable".to_string());
            }
            std::env::set_current_dir(CStr::from_ptr(path).to_string_lossy().as_ref())
                .map_err(|e| e.to_string())?;
        }
        run()
    })();
    if let Err(error) = result {
        let message = CString::new(error.replace('\0', " ")).unwrap();
        unsafe {
            SDL_LogMessage(0, 5, c"%s".as_ptr(), message.as_ptr());
        }
        1
    } else {
        0
    }
}
