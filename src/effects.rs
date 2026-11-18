//! Pirate fire, smoke, embers and material-specific debris.
use crate::{
    math::{rotate, Camera, V},
    scene::{self, Material, Scene},
};
use glam::{Mat4, Quat, Vec3};

const MAX_PARTICLES: usize = 1800;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Family {
    Metal,
    Jar,
    Stone,
    Wood,
    Ice,
    Tnt,
    Wormhole,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Style {
    Shard,
    Smoke,
    Spark,
    Splash,
    Fire,
}
struct Layer {
    style: Style,
    count: [usize; 2],
    life: [f32; 2],
    speed: [f32; 2],
    size: [f32; 2],
    gravity: [f32; 2],
    color: [f32; 3],
}
const BURST: [Layer; 3] = [
    Layer {
        style: Style::Fire,
        count: [5, 7],
        life: [0.24, 0.41],
        speed: [1.2, 3.4],
        size: [0.67, 1.08],
        gravity: [-0.27, -0.12],
        color: [1., 0.31, 0.045],
    },
    Layer {
        style: Style::Smoke,
        count: [5, 7],
        life: [1.0, 1.65],
        speed: [0.45, 1.8],
        size: [0.75, 1.2],
        gravity: [-0.18, -0.10],
        color: [0.022, 0.016, 0.013],
    },
    Layer {
        style: Style::Spark,
        count: [9, 14],
        life: [0.38, 0.82],
        speed: [3.8, 8.5],
        size: [0.045, 0.085],
        gravity: [0.6, 0.9],
        color: [1., 0.25, 0.02],
    },
];

include!("../assets/effects/profiles.rs");
mod native_fragments {
    include!("../assets/objects/fragments.rs");
}
pub fn family(kind: u8) -> Family {
    match kind {
        11..=16 => Family::Jar,
        21 | 22 => Family::Stone,
        41 | 42 => Family::Wood,
        51 => Family::Tnt,
        31 => Family::Wormhole,
        61 | 62 => Family::Ice,
        _ => Family::Metal,
    }
}
struct Particle {
    p: V,
    v: V,
    age: f32,
    life: f32,
    size: V,
    rotation: Quat,
    spin: Vec3,
    color: [f32; 3],
    gravity: f32,
    style: Style,
    family: Family,
    floor: f32,
    fragment: Option<(u8, bool, usize)>,
}
#[derive(Default)]
pub struct Effects {
    particles: Vec<Particle>,
    sequence: u32,
}
struct Random(u32);
impl Random {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / 16777216.
    }
    fn range(&mut self, r: [f32; 2]) -> f32 {
        r[0] + (r[1] - r[0]) * self.next()
    }
    fn direction(&mut self) -> V {
        let y = self.range([-0.7, 1.]);
        let t = self.next() * std::f32::consts::TAU;
        let r = (1. - y * y).sqrt();
        V::new(r * t.cos(), y, r * t.sin())
    }
}
impl Effects {
    pub fn active_count(&self) -> usize {
        self.particles.len()
    }
    pub fn clear(&mut self) {
        self.particles.clear();
        self.sequence = 0;
    }
    pub fn burst(&mut self, kind: u8, p: V, half: V, q: [f32; 4], ground: bool) {
        self.sequence = self.sequence.wrapping_add(1);
        let mut rng = Random(self.sequence.wrapping_mul(747796405) ^ u32::from(kind));
        let family = family(kind);
        for (index, center) in native_fragments::centers(kind, ground).iter().enumerate() {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            let unit = (half.x.min(half.y).min(half.z) * 2.).clamp(0.2, 2.);
            let offset = rotate(
                V::new(
                    center[0] * half.x * 2.,
                    center[1] * half.y * 2.,
                    center[2] * half.z * 2.,
                ),
                q,
            );
            self.particles.push(Particle {
                p: p + offset,
                v: rng.direction() * rng.range([3., 7.]) + V::new(0., 2., 0.),
                age: 0.,
                life: rng.range([0.65, 1.15]),
                size: V::new(unit * 0.5, unit * 0.5, unit * 0.5),
                rotation: Quat::from_array(q),
                spin: Vec3::new(
                    rng.range([-9., 9.]),
                    rng.range([-9., 9.]),
                    rng.range([-9., 9.]),
                ),
                color: [1.; 3],
                gravity: 1.,
                style: Style::Shard,
                family,
                floor: p.y - half.y - if ground { 0. } else { 1.4 },
                fragment: Some((kind, ground, index)),
            });
        }
        // Longer native objects have their own multi-section prefabs. Scale the representative
        // 1x burst modestly and distribute fragments through the actual rotated object bounds.
        let stretch = (half.y * 2.).sqrt().clamp(1., 2.);
        let blast_scale = if family == Family::Tnt { 1.55 } else { 1. };
        for layer in BURST.iter().chain(layers(family, ground)) {
            let count = (rng.range([layer.count[0] as f32, layer.count[1] as f32]) + 0.5) as usize;
            for _ in 0..((count as f32 * stretch) as usize).min(64) {
                if self.particles.len() >= MAX_PARTICLES {
                    return;
                }
                let direction = rng.direction();
                let offset = rotate(
                    V::new(
                        rng.range([-half.x, half.x]),
                        rng.range([-half.y, half.y]),
                        rng.range([-half.z, half.z]),
                    ),
                    q,
                );
                let mut size = rng.range(layer.size).max(0.015) * 0.5;
                if matches!(layer.style, Style::Fire | Style::Smoke) {
                    size *= blast_scale;
                }
                let mut shape = V::new(size, size, size);
                if layer.style == Style::Shard {
                    shape = match family {
                        Family::Wood => V::new(size * 0.28, size * 2.6, size * 0.35),
                        Family::Ice | Family::Jar => V::new(size * 0.8, size * 1.7, size * 0.24),
                        _ => V::new(size, size * 0.8, size * 0.7),
                    };
                }
                let tint = crate::glow_color(kind);
                let color = match (family, layer.style) {
                    (Family::Jar, Style::Splash) => tint,
                    (Family::Jar, Style::Shard) => [
                        0.65 + tint[0] * 0.3,
                        0.7 + tint[1] * 0.25,
                        0.8 + tint[2] * 0.2,
                    ],
                    (Family::Ice, Style::Shard) => [0.45, 0.88, 1.],
                    (Family::Wood, Style::Shard) => [0.68, 0.36, 0.13],
                    (_, Style::Fire | Style::Spark) => [1., rng.range([0.20, 0.49]), 0.035],
                    (_, Style::Smoke) => layer.color.map(|c| c * rng.range([0.7, 1.5])),
                    _ => layer.color,
                };
                let speed = rng.range(layer.speed);
                let mut v = direction * speed;
                if matches!(layer.style, Style::Fire | Style::Smoke | Style::Spark) {
                    v.y += 1.1;
                }
                if ground {
                    v.y = v.y.abs() * 0.6;
                }
                self.particles.push(Particle {
                    fragment: None,
                    p: p + offset
                        * if matches!(layer.style, Style::Fire | Style::Smoke) {
                            0.45
                        } else {
                            0.7
                        },
                    v,
                    age: 0.,
                    life: rng.range(layer.life).clamp(0.05, 2.5),
                    size: shape,
                    rotation: Quat::from_euler(
                        glam::EulerRot::XYZ,
                        rng.next() * 6.,
                        rng.next() * 6.,
                        rng.next() * 6.,
                    ),
                    spin: Vec3::new(
                        rng.range([-8., 8.]),
                        rng.range([-8., 8.]),
                        rng.range([-8., 8.]),
                    ),
                    color,
                    gravity: rng.range(layer.gravity),
                    style: layer.style,
                    family,
                    floor: if ground {
                        p.y - half.y
                    } else {
                        p.y - half.y - 1.4
                    },
                });
            }
        }
    }
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.age += dt;
            p.v.y -= 9.81 * p.gravity * dt;
            let drag = match p.style {
                Style::Smoke => 1.7,
                Style::Fire => 2.4,
                _ => 0.6,
            };
            p.v = p.v * (1. - drag * dt).max(0.);
            p.p = p.p + p.v * dt;
            if p.p.y < p.floor && p.v.y < 0. && matches!(p.style, Style::Shard | Style::Splash) {
                p.p.y = p.floor;
                p.v.y *= if p.style == Style::Splash {
                    -0.04
                } else {
                    -0.25
                };
                p.v.x *= 0.65;
                p.v.z *= 0.65;
            }
            p.rotation = (p.rotation * Quat::from_scaled_axis(p.spin * dt)).normalize();
        }
        self.particles.retain(|p| p.age < p.life);
    }
    pub fn frame(&self, scene: &mut Scene, world: &mut Vec<Mat4>, camera: &Camera) {
        let right = Vec3::new(camera.right.x, camera.right.y, camera.right.z);
        let up = Vec3::new(camera.up.x, camera.up.y, camera.up.z);
        let billboard = Quat::from_mat3(&glam::Mat3::from_cols(right, up, right.cross(up)));
        for p in &self.particles {
            let t = (p.age / p.life).clamp(0., 1.);
            let fade = (1. - t).min(0.3) / 0.3;
            if let Some((kind, ground, index)) = p.fragment {
                if let Some(primitive) = scene.fragment_visual(kind, ground, index) {
                    let mut material = scene.primitives[primitive].material;
                    material.color[3] *= fade;
                    material.blend = true;
                    let scale = Vec3::new(p.size.x, p.size.y, p.size.z) * 2. * (0.6 + 0.4 * fade);
                    scene.instance(
                        world,
                        primitive,
                        Mat4::from_scale_rotation_translation(
                            scale,
                            p.rotation,
                            Vec3::new(p.p.x, p.p.y, p.p.z),
                        ),
                        Some(material),
                    );
                }
                continue;
            }
            let mut size = Vec3::new(p.size.x, p.size.y, p.size.z);
            let mut rotation = p.rotation;
            let mut mat = Material {
                color: [p.color[0], p.color[1], p.color[2], fade],
                blend: true,
                ..Material::default()
            };
            let mesh = match p.style {
                Style::Smoke => {
                    rotation = billboard * Quat::from_rotation_z(p.spin.z + p.age * 0.35);
                    size *= 1. + t * 2.4;
                    mat.color[3] = 0.82 * (p.age / 0.20).min(1.) * (1. - t).powf(0.85);
                    mat.unlit = true;
                    mat.emissive = p.color;
                    scene::PUFF
                }
                Style::Fire => {
                    rotation = billboard * Quat::from_rotation_z(p.spin.z + p.age * 0.9);
                    size *= 1. + t * 1.7;
                    size.y *= 1.24;
                    mat.color[3] = (0.7 + t * 4.).min(1.) * (1. - t).powf(1.25);
                    mat.unlit = true;
                    mat.emissive = [
                        1.85 * (1. - t * 0.75),
                        (0.40 + p.color[1]) * (1. - t).powf(1.35),
                        0.06 * (1. - t),
                    ];
                    scene::PUFF
                }
                Style::Spark => {
                    let direction = Vec3::new(p.v.x, p.v.y, p.v.z).normalize_or_zero();
                    if direction.length_squared() > 0.5 {
                        rotation = Quat::from_rotation_arc(Vec3::Y, direction);
                    }
                    size = Vec3::new(
                        size.x * 0.22,
                        size.y * (2. + p.v.len() * 0.3),
                        size.z * 0.22,
                    ) * fade;
                    mat.unlit = true;
                    mat.emissive = p.color.map(|c| c * (1.6 - t));
                    scene::CUBE
                }
                Style::Splash => {
                    size.y *= 1. + p.v.len() * 0.09;
                    mat.roughness = 0.15;
                    scene::SPHERE
                }
                Style::Shard => {
                    size *= 0.5 + 0.5 * fade;
                    mat.roughness = if p.family == Family::Wood || p.family == Family::Stone {
                        0.85
                    } else {
                        0.12
                    };
                    if matches!(p.family, Family::Ice | Family::Jar) {
                        mat.glass = true;
                        mat.color[3] *= 0.4;
                    }
                    if p.family == Family::Wood {
                        scene::CUBE
                    } else {
                        scene::SHARD
                    }
                }
            };
            scene.instance(
                world,
                mesh,
                Mat4::from_scale_rotation_translation(
                    size * 2.,
                    rotation,
                    Vec3::new(p.p.x, p.p.y, p.p.z),
                ),
                Some(mat),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_recovered_destructible_type_has_textured_debris_on_hit_and_ground() {
        let mut scene = Scene::load().unwrap();
        for kind in [
            1, 2, 3, 11, 12, 13, 14, 15, 16, 21, 22, 31, 41, 42, 51, 61, 62,
        ] {
            for ground in [false, true] {
                let mut fx = Effects::default();
                fx.burst(
                    kind,
                    V::new(0., 2., 0.),
                    V::new(0.5, 0.5, 0.5),
                    [0., 0., 0., 1.],
                    ground,
                );
                assert!(
                    fx.particles.iter().any(|p| p.fragment.is_some()),
                    "missing native debris for type {kind}"
                );
                let mut world = vec![];
                scene.nodes.clear();
                fx.frame(&mut scene, &mut world, &Camera::new());
                assert!(scene
                    .nodes
                    .iter()
                    .flat_map(|n| &n.primitives)
                    .any(|&p| p > scene::PUFF && scene.primitives[p].texture.is_some()));
                for _ in 0..240 {
                    fx.update(1. / 120.);
                }
                assert_eq!(fx.active_count(), 0);
            }
        }
    }
    #[test]
    fn material_bursts_have_distinct_layers_and_expire() {
        for (kind, required) in [
            (1, Style::Spark),
            (11, Style::Splash),
            (21, Style::Shard),
            (41, Style::Shard),
            (51, Style::Fire),
            (61, Style::Shard),
        ] {
            let mut fx = Effects::default();
            fx.burst(
                kind,
                V::new(0., 2., 0.),
                V::new(0.5, 0.5, 0.5),
                [0., 0., 0., 1.],
                false,
            );
            assert!(
                fx.particles.iter().any(|p| p.style == required),
                "missing {required:?} for {kind}"
            );
            for style in [Style::Fire, Style::Smoke, Style::Spark] {
                assert!(fx.particles.iter().any(|p| p.style == style));
            }
            assert!(fx
                .particles
                .iter()
                .all(|p| p.life > 0. && p.life.is_finite()));
            for _ in 0..360 {
                fx.update(1. / 120.);
            }
            assert!(fx.particles.is_empty());
        }
    }
    #[test]
    fn simultaneous_destruction_is_bounded_and_ground_jars_differ() {
        let mut fx = Effects::default();
        for _ in 0..100 {
            fx.burst(
                11,
                V::default(),
                V::new(0.5, 0.5, 0.5),
                [0., 0., 0., 1.],
                false,
            );
        }
        assert!(fx.particles.len() <= MAX_PARTICLES);
        assert!(layers(Family::Jar, true).len() != layers(Family::Jar, false).len());
    }
}
