use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::{collections::HashMap, fs, io::BufReader, sync::Arc};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}
pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Material,
    pub texture: Option<Arc<Image>>,
}
#[derive(Clone, Copy)]
pub struct Material {
    pub color: [f32; 4],
    pub emissive: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub clear_coat: f32,
    pub use_texture: bool,
    pub blend: bool,
    pub double_sided: bool,
    pub glass: bool,
    pub unlit: bool,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            color: [1.; 4],
            emissive: [0.; 3],
            metallic: 0.,
            roughness: 0.65,
            clear_coat: 0.,
            use_texture: true,
            blend: false,
            double_sided: false,
            glass: false,
            unlit: false,
        }
    }
}
pub struct Node {
    pub primitives: Vec<usize>,
    pub material: Option<Material>,
}
pub struct Scene {
    pub primitives: Vec<Primitive>,
    pub nodes: Vec<Node>,
    pub dock: Vec<usize>,
    pub cannon_base: Vec<usize>,
    pub cannon_barrel: Vec<usize>,
    pub backdrop: usize,
    pub water: usize,
    objects: Vec<ObjectVisual>,
    fragments: HashMap<(u8, bool), Vec<usize>>,
}
struct ObjectVisual {
    kind: u8,
    size: [f32; 3],
    parts: Vec<usize>,
}
pub const CUBE: usize = 0;
pub const CYLINDER: usize = 1;
pub const CONE: usize = 2;
pub const SPHERE: usize = 3;
// Slots 4 and 5 retain the original cannon meshes for compatibility.
pub const GLASS_JAR: usize = 6;
pub const FRAME_CUBE: usize = 7;
pub const FRAME_CYLINDER: usize = 8;
pub const FRAME_CONE: usize = 9;
pub const FRAME_JAR: usize = 10;
pub const SHARD: usize = 11;
pub const PUFF: usize = 12;
impl Scene {
    pub fn load() -> Result<Self> {
        let mut primitives = vec![cube(), cylinder(1.), cylinder(0.72), sphere()];
        for (mesh, tex, metal, rough) in [
            ("CannonBottom", "Cannon_1_65", 0.55, 0.3),
            ("Cannfon3", "Cannon_1_65", 0.55, 0.3),
        ] {
            let mut p = load_obj(mesh)?;
            p.texture = Some(Arc::new(load_image(tex)?));
            p.material.metallic = metal;
            p.material.roughness = rough;
            primitives.push(p);
        }
        primitives.extend([
            lathe(&JAR_PROFILE),
            cube_frame(),
            round_frame(&[(-0.5, 0.5), (0.5, 0.5)]),
            round_frame(&[(-0.5, 0.5), (0.5, 0.36)]),
            round_frame(&JAR_PROFILE),
            shard(),
            puff(),
        ]);
        let mut objects: Vec<ObjectVisual> = vec![];
        let mut fragments: HashMap<(u8, bool), Vec<usize>> = HashMap::new();
        let mut images: HashMap<String, Arc<Image>> = HashMap::new();
        for line in fs::read_to_string("assets/objects/manifest.txt")?
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let a: Vec<_> = line.split_whitespace().collect();
            let kind = a[0].parse()?;
            let size = [a[1].parse()?, a[2].parse()?, a[3].parse()?];
            let id = load_visual_part(&a[4..], &mut primitives, &mut images, kind)?;
            if let Some(v) = objects
                .iter_mut()
                .find(|v| v.kind == kind && v.size == size)
            {
                v.parts.push(id);
            } else {
                objects.push(ObjectVisual {
                    kind,
                    size,
                    parts: vec![id],
                });
            }
        }
        for line in fs::read_to_string("assets/objects/fragments.txt")?
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let a: Vec<_> = line.split_whitespace().collect();
            let kind = a[0].parse()?;
            let ground = a[1] == "1";
            let id = load_visual_part(&a[2..], &mut primitives, &mut images, kind)?;
            fragments.entry((kind, ground)).or_default().push(id);
        }
        let mut scenery = |name: &str| -> Result<Vec<usize>> {
            let mut parts = Vec::new();
            for surface in ["wood", "redwood", "iron", "brass", "ivory"] {
                let mesh = format!("assets/pirate/{name}-{surface}.obj");
                if std::path::Path::new(&mesh).exists() {
                    let texture = format!("assets/pirate/{surface}.png");
                    parts.push(load_visual_part(
                        &[&mesh, &texture, "1", "1", "1", "1"],
                        &mut primitives,
                        &mut images,
                        0,
                    )?);
                }
            }
            anyhow::ensure!(!parts.is_empty(), "Missing pirate scenery {name}");
            Ok(parts)
        };
        let dock = scenery("dock")?;
        let cannon_base = scenery("cannon-carriage")?;
        let cannon_barrel = scenery("cannon-barrel")?;
        let backdrop = primitives.len();
        let mut background = empty();
        let points = [
            [-0.5, -0.5, 0.],
            [0.5, -0.5, 0.],
            [0.5, 0.5, 0.],
            [-0.5, 0.5, 0.],
        ];
        let uvs = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
        for i in [0, 1, 2, 0, 2, 3] {
            background.indices.push(background.vertices.len() as u32);
            background.vertices.push(Vertex {
                position: points[i],
                normal: [0., 0., 1.],
                color: [1.; 4],
                uv: uvs[i],
            });
        }
        background.texture = Some(Arc::new(load_image_path("assets/pirate/harbor.png")?));
        background.material.roughness = 1.;
        background.material.unlit = true;
        background.material.emissive = [1.; 3];
        background.material.double_sided = true;
        primitives.push(background);

        // The far surface is anchored to the harbor horizon and kept behind the cargo.
        let water = primitives.len();
        let mut sea = empty();
        sea.texture = Some(Arc::new(load_image_path("assets/pirate/water-mask.png")?));
        let shore = 33.;
        let horizon = 485.;
        let surface_y = |depth| -9.975 + 0.05 * depth;
        let points = [
            Vec3::new(-350., surface_y(shore), shore),
            Vec3::new(-350., surface_y(horizon), horizon),
            Vec3::new(350., surface_y(horizon), horizon),
            Vec3::new(350., surface_y(shore), shore),
        ];
        let normal = Vec3::new(0., 1., -0.05).normalize();
        for face in [[0, 1, 2], [0, 2, 3]] {
            triangle(&mut sea, face.map(|index| points[index]), [normal; 3]);
        }
        primitives.push(sea);
        Ok(Self {
            dock,
            cannon_base,
            cannon_barrel,
            backdrop,
            water,
            primitives,
            nodes: vec![],
            objects,
            fragments,
        })
    }
    pub fn object_visual(
        &self,
        kind: u8,
        half: crate::math::V,
    ) -> Option<(Vec<usize>, crate::math::V)> {
        let size = [half.x * 2., half.y * 2., half.z * 2.];
        let score = |v: &ObjectVisual| {
            size.iter()
                .zip(v.size)
                .map(|(a, b)| (a / b).ln().abs())
                .sum::<f32>()
        };
        let v = self
            .objects
            .iter()
            .filter(|v| v.kind == kind)
            .min_by(|a, b| score(a).total_cmp(&score(b)))?;
        Some((
            v.parts.clone(),
            crate::math::V::new(half.x / v.size[0], half.y / v.size[1], half.z / v.size[2]),
        ))
    }
    pub fn fragment_visual(&self, kind: u8, ground: bool, index: usize) -> Option<usize> {
        self.fragments
            .get(&(kind, ground))
            .or_else(|| self.fragments.get(&(kind, false)))?
            .get(index)
            .copied()
    }
    pub fn instance(
        &mut self,
        world: &mut Vec<Mat4>,
        primitive: usize,
        model: Mat4,
        material: Option<Material>,
    ) {
        self.nodes.push(Node {
            primitives: vec![primitive],
            material,
        });
        world.push(model);
    }
}
fn load_visual_part(
    a: &[&str],
    primitives: &mut Vec<Primitive>,
    images: &mut HashMap<String, Arc<Image>>,
    kind: u8,
) -> Result<usize> {
    let mut p = load_obj_path(a[0])?;
    let image = if let Some(image) = images.get(a[1]) {
        image.clone()
    } else {
        let image = Arc::new(load_image_path(a[1])?);
        images.insert(a[1].to_owned(), image.clone());
        image
    };
    p.material.color = [a[2].parse()?, a[3].parse()?, a[4].parse()?, a[5].parse()?];
    p.material.metallic = if matches!(kind, 1..=3) { 0.35 } else { 0. };
    p.material.roughness = if matches!(kind, 11..=16 | 61 | 62) {
        0.18
    } else {
        0.6
    };
    if a[1].starts_with("assets/pirate/") {
        let (metal, rough, coat) = if a[1].ends_with("brass.png") {
            (0.76, 0.46, 0.05)
        } else if a[1].ends_with("iron.png") {
            (0.72, 0.4, 0.1)
        } else if a[1].ends_with("blue.png")
            || a[1].ends_with("jade.png")
            || a[1].ends_with("purple.png")
        {
            (0.18, 0.16, 0.75)
        } else if a[1].ends_with("cloth.png") {
            (0., 0.92, 0.)
        } else {
            (0., 0.68, 0.)
        };
        p.material.metallic = metal;
        p.material.roughness = rough;
        p.material.clear_coat = coat;
    }
    p.material.blend =
        p.material.color[3] < 0.999 || image.rgba.chunks_exact(4).any(|p| p[3] < 255);
    p.texture = Some(image);
    let id = primitives.len();
    primitives.push(p);
    Ok(id)
}
fn load_image(name: &str) -> Result<Image> {
    load_image_path(&format!("assets/recovered/textures/{name}.png"))
}
pub(crate) fn load_image_path(path: &str) -> Result<Image> {
    let mut decoder = png::Decoder::new(BufReader::new(
        fs::File::open(&path).with_context(|| path.to_owned())?,
    ));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes)?;
    let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
    let bytes = &bytes[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(bytes),
        png::ColorType::Rgb => {
            for p in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &p in bytes {
                rgba.extend_from_slice(&[p, p, p, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in bytes.chunks_exact(2) {
                rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
            }
        }
        _ => anyhow::bail!("Unsupported texture encoding {path}"),
    };
    Ok(Image {
        width: info.width as i32,
        height: info.height as i32,
        rgba,
    })
}
fn load_obj(name: &str) -> Result<Primitive> {
    load_obj_path(&format!("assets/recovered/meshes/{name}.obj"))
}
fn load_obj_path(path: &str) -> Result<Primitive> {
    let text = fs::read_to_string(path).with_context(|| path.to_owned())?;
    let (mut positions, mut normals, mut uvs) = (vec![], vec![], vec![]);
    let mut out = empty();
    for line in text.lines() {
        let a: Vec<_> = line.split_whitespace().collect();
        if a.is_empty() {
            continue;
        }
        let values = || {
            a[1..]
                .iter()
                .map(|s| s.parse::<f32>())
                .collect::<std::result::Result<Vec<_>, _>>()
        };
        match a[0] {
            "v" => {
                let v = values()?;
                positions.push([v[0], v[1], v[2]]);
            }
            "vn" => {
                let v = values()?;
                normals.push([v[0], v[1], v[2]]);
            }
            "vt" => {
                let v = values()?;
                uvs.push([v[0], 1. - v[1]]);
            }
            "f" => {
                let points: Vec<_> = a[1..]
                    .iter()
                    .map(|s| {
                        s.split('/')
                            .map(|i| i.parse::<usize>().map(|i| i - 1))
                            .collect::<std::result::Result<Vec<_>, _>>()
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for i in 1..points.len() - 1 {
                    for p in [&points[0], &points[i], &points[i + 1]] {
                        out.indices.push(out.vertices.len() as u32);
                        out.vertices.push(Vertex {
                            position: positions[p[0]],
                            normal: normals[p[2]],
                            uv: uvs[p[1]],
                            color: [1.; 4],
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}
fn empty() -> Primitive {
    Primitive {
        vertices: vec![],
        indices: vec![],
        material: Material::default(),
        texture: None,
    }
}
fn shard() -> Primitive {
    let mut p = empty();
    let v = [
        Vec3::new(-0.5, -0.4, -0.25),
        Vec3::new(0.5, -0.3, -0.2),
        Vec3::new(-0.15, 0.5, -0.1),
        Vec3::new(0.1, 0., 0.5),
    ];
    for [a, b, c] in [[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]] {
        let n = (v[b] - v[a]).cross(v[c] - v[a]).normalize();
        triangle(&mut p, [v[a], v[b], v[c]], [n; 3]);
    }
    p
}
// Soft, irregular billboard shared by fire and smoke. Rotating overlapping puffs breaks up the silhouette.
fn puff() -> Primitive {
    let mut p = empty();
    for ring in 0..4 {
        let r0 = ring as f32 / 4.;
        let r1 = (ring + 1) as f32 / 4.;
        for i in 0..24 {
            let a = i as f32 * std::f32::consts::TAU / 24.;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 24.;
            let start = p.vertices.len();
            for (r, t) in [(r0, a), (r1, a), (r1, b), (r0, a), (r1, b), (r0, b)] {
                p.indices.push(p.vertices.len() as u32);
                let radius = r * (0.5 + 0.066 * (t * 3.).sin() + 0.045 * (t * 5.).cos());
                let shade = 1. - 0.28 * r + 0.16 * r * (t * 4. + r * 7.).sin();
                p.vertices.push(Vertex {
                    position: [radius * t.cos(), radius * t.sin(), 0.],
                    normal: [0., 0., 1.],
                    uv: [0.; 2],
                    color: [shade, shade, shade, (1. - r * r).powi(2)],
                });
            }
            debug_assert_eq!(p.vertices.len() - start, 6);
        }
    }
    p
}
fn triangle(p: &mut Primitive, points: [Vec3; 3], normals: [Vec3; 3]) {
    for i in 0..3 {
        p.indices.push(p.vertices.len() as u32);
        p.vertices.push(Vertex {
            position: points[i].to_array(),
            normal: normals[i].to_array(),
            color: [1.; 4],
            uv: [0.; 2],
        });
    }
}
fn cube() -> Primitive {
    let mut p = empty();
    let c = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, 0.5, -0.5),
        Vec3::new(-0.5, 0.5, -0.5),
        Vec3::new(-0.5, -0.5, 0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(0.5, 0.5, 0.5),
        Vec3::new(-0.5, 0.5, 0.5),
    ];
    for ids in [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [3, 7, 6, 2],
        [0, 1, 5, 4],
    ] {
        let n = (c[ids[1]] - c[ids[0]])
            .cross(c[ids[2]] - c[ids[0]])
            .normalize();
        for t in [[0, 1, 2], [0, 2, 3]] {
            triangle(&mut p, t.map(|i| c[ids[i]]), [n; 3]);
        }
    }
    p
}
fn cylinder(top: f32) -> Primitive {
    let mut p = empty();
    let n = 32;
    let ring = |i: usize, y: f32, r: f32| {
        let a = i as f32 * std::f32::consts::TAU / n as f32;
        Vec3::new(a.cos() * r * 0.5, y, a.sin() * r * 0.5)
    };
    for i in 0..n {
        let j = (i + 1) % n;
        let a = ring(i, -0.5, 1.);
        let b = ring(i, 0.5, top);
        let c = ring(j, 0.5, top);
        let d = ring(j, -0.5, 1.);
        let norm = |v: Vec3| Vec3::new(v.x, (1. - top) * 0.25, v.z).normalize();
        triangle(&mut p, [a, b, c], [norm(a), norm(b), norm(c)]);
        triangle(&mut p, [a, c, d], [norm(a), norm(c), norm(d)]);
        triangle(&mut p, [Vec3::new(0., 0.5, 0.), c, b], [Vec3::Y; 3]);
        triangle(&mut p, [Vec3::new(0., -0.5, 0.), a, d], [-Vec3::Y; 3]);
    }
    p
}
fn sphere() -> Primitive {
    let mut p = empty();
    let point = |i: usize, j: usize| {
        let a = i as f32 * std::f32::consts::TAU / 24.;
        let b = j as f32 * std::f32::consts::PI / 16.;
        Vec3::new(b.sin() * a.cos(), b.cos(), b.sin() * a.sin()) * 0.5
    };
    for j in 0..16 {
        for i in 0..24 {
            let a = point(i, j);
            let b = point(i + 1, j);
            let c = point(i + 1, j + 1);
            let d = point(i, j + 1);
            for v in [[a, b, c], [a, c, d]] {
                triangle(&mut p, v, v.map(|v| v.normalize()));
            }
        }
    }
    p
}
// Clean frames follow object boundaries; they do not expose triangulation diagonals.
const JAR_PROFILE: [(f32, f32); 4] = [(-0.5, 0.5), (0.25, 0.5), (0.4, 0.34), (0.5, 0.34)];
fn lathe(profile: &[(f32, f32)]) -> Primitive {
    let mut p = empty();
    let n = 32;
    let point = |i: usize, y: f32, r: f32| {
        let a = i as f32 * std::f32::consts::TAU / n as f32;
        Vec3::new(a.cos() * r, y, a.sin() * r)
    };
    for pair in profile.windows(2) {
        let [(y0, r0), (y1, r1)] = [pair[0], pair[1]];
        let slope = -(r1 - r0) / (y1 - y0);
        for i in 0..n {
            let j = (i + 1) % n;
            let a = point(i, y0, r0);
            let b = point(i, y1, r1);
            let c = point(j, y1, r1);
            let d = point(j, y0, r0);
            let normal = |i| {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                Vec3::new(a.cos(), slope, a.sin()).normalize()
            };
            triangle(&mut p, [a, b, c], [normal(i), normal(i), normal(j)]);
            triangle(&mut p, [a, c, d], [normal(i), normal(j), normal(j)]);
        }
    }
    let (y0, r0) = profile[0];
    let (y1, r1) = *profile.last().unwrap();
    for i in 0..n {
        triangle(
            &mut p,
            [
                Vec3::new(0., y1, 0.),
                point(i + 1, y1, r1),
                point(i, y1, r1),
            ],
            [Vec3::Y; 3],
        );
        triangle(
            &mut p,
            [
                Vec3::new(0., y0, 0.),
                point(i, y0, r0),
                point(i + 1, y0, r0),
            ],
            [-Vec3::Y; 3],
        );
    }
    p
}
fn tube(p: &mut Primitive, start: Vec3, end: Vec3, radius: f32) {
    let axis = (end - start).normalize();
    let reference = if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(reference).normalize();
    let v = axis.cross(u);
    let n = 6;
    let normal = |i: usize| {
        let angle = i as f32 * std::f32::consts::TAU / n as f32;
        u * angle.cos() + v * angle.sin()
    };
    for i in 0..n {
        let ni = normal(i);
        let nj = normal(i + 1);
        let a = start + ni * radius;
        let b = end + ni * radius;
        let c = end + nj * radius;
        let d = start + nj * radius;
        triangle(p, [a, c, b], [ni, nj, ni]);
        triangle(p, [a, d, c], [ni, nj, nj]);
        triangle(p, [start, d, a], [-axis; 3]);
        triangle(p, [end, b, c], [axis; 3]);
    }
}
fn cube_frame() -> Primitive {
    let mut p = empty();
    for a in [-0.5, 0.5] {
        for b in [-0.5, 0.5] {
            tube(&mut p, Vec3::new(-0.5, a, b), Vec3::new(0.5, a, b), 0.009);
            tube(&mut p, Vec3::new(a, -0.5, b), Vec3::new(a, 0.5, b), 0.009);
            tube(&mut p, Vec3::new(a, b, -0.5), Vec3::new(a, b, 0.5), 0.009);
        }
    }
    p
}
fn round_frame(profile: &[(f32, f32)]) -> Primitive {
    let mut p = empty();
    let n = 32;
    let sides = 6;
    let radius = 0.009;
    for &(y, r) in profile {
        let sample = |i: usize, j: usize| {
            let a = i as f32 * std::f32::consts::TAU / n as f32;
            let b = j as f32 * std::f32::consts::TAU / sides as f32;
            let radial = Vec3::new(a.cos(), 0., a.sin());
            let normal = radial * b.cos() + Vec3::Y * b.sin();
            (radial * r + Vec3::Y * y + normal * radius, normal)
        };
        for i in 0..n {
            for j in 0..sides {
                let (a, na) = sample(i, j);
                let (b, nb) = sample(i + 1, j);
                let (c, nc) = sample(i + 1, j + 1);
                let (d, nd) = sample(i, j + 1);
                triangle(&mut p, [a, c, b], [na, nc, nb]);
                triangle(&mut p, [a, d, c], [na, nd, nc]);
            }
        }
    }
    for i in 0..4 {
        let angle = (i as f32 + 0.25) * std::f32::consts::TAU / 4.;
        let point = |y: f32, r: f32| Vec3::new(angle.cos() * r, y, angle.sin() * r);
        for pair in profile.windows(2) {
            tube(
                &mut p,
                point(pair[0].0, pair[0].1),
                point(pair[1].0, pair[1].1),
                radius,
            );
        }
    }
    p
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovered_gpu_meshes_are_valid() {
        let s = Scene::load().unwrap();
        assert!(s.primitives.len() > 60);
        assert!(s.primitives.iter().filter(|p| p.texture.is_some()).count() > 60);
        for p in &s.primitives {
            assert_eq!(p.indices.len() % 3, 0);
            for &i in &p.indices {
                let v = p.vertices[i as usize];
                assert!(v
                    .position
                    .iter()
                    .chain(v.normal.iter())
                    .all(|x| x.is_finite()));
            }
            if let Some(t) = &p.texture {
                assert_eq!(t.rgba.len(), t.width as usize * t.height as usize * 4);
            }
        }
    }
}
