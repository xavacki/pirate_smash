use std::ops::{Add, Mul, Sub};
#[derive(Clone, Copy, Debug, Default)]
pub struct V {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl V {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, b: Self) -> f32 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
    pub fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn unit(self) -> Self {
        self * (1. / self.len().max(0.00001))
    }
}
impl Add for V {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for V {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl Mul<f32> for V {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self::new(self.x * b, self.y * b, self.z * b)
    }
}
pub fn rotate(v: V, q: [f32; 4]) -> V {
    let u = V::new(q[0], q[1], q[2]);
    v + u.cross(v) * (2. * q[3]) + u.cross(u.cross(v)) * 2.
}
pub const VIEW_HEIGHT: f32 = 800.;
pub const DESKTOP_WIDTH: f32 = 1100.;

pub struct Camera {
    pub eye: V,
    pub forward: V,
    pub right: V,
    pub up: V,
    pub zoom: f32,
    pub width: f32,
}
impl Camera {
    pub fn new() -> Self {
        // Gameplay Main Camera from level2: yaw 0, pitch +3 degrees, perspective FOV 30.
        // Unity +Z forward is preserved; reflecting X matches UnityPy's OBJ export.
        let eye = V::new(0., 5.83, -39.5);
        let angle = 3_f32.to_radians();
        let forward = V::new(0., -angle.sin(), angle.cos());
        let right = forward.cross(V::new(0., 1., 0.)).unit();
        let up = right.cross(forward);
        Self {
            eye,
            forward,
            right,
            up,
            zoom: VIEW_HEIGHT / 2. / 15_f32.to_radians().tan(),
            width: DESKTOP_WIDTH,
        }
    }
    pub fn resize(&mut self, width: f32, editing: bool) {
        self.width = width;
        if editing && self.portrait() {
            // Keep the editable objects above the bottom inspector.
            let angle = 6_f32.to_radians();
            self.forward = V::new(0., -angle.sin(), angle.cos());
            self.up = self.right.cross(self.forward);
        }
    }
    pub fn portrait(&self) -> bool {
        self.width < VIEW_HEIGHT
    }
    pub fn project(&self, p: V) -> Option<(crate::sdl::Point, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        if z < 0.1 {
            return None;
        }
        Some((
            crate::sdl::Point {
                x: self.width / 2. + d.dot(self.right) * self.zoom / z,
                y: 400. - d.dot(self.up) * self.zoom / z,
            },
            z,
        ))
    }
    pub fn view_projection(&self) -> glam::Mat4 {
        let eye = glam::Vec3::new(self.eye.x, self.eye.y, self.eye.z);
        let forward = glam::Vec3::new(self.forward.x, self.forward.y, self.forward.z);
        let up = glam::Vec3::new(self.up.x, self.up.y, self.up.z);
        let view = glam::Mat4::look_at_rh(eye, eye + forward, up);
        let projection = glam::Mat4::perspective_rh_gl(
            2. * (400. / self.zoom).atan(),
            self.width / VIEW_HEIGHT,
            0.3,
            1000.,
        );
        projection * view
    }
    pub fn ray(&self, x: f32, y: f32) -> V {
        (self.forward
            + self.right * ((x - self.width / 2.) / self.zoom)
            + self.up * ((400. - y) / self.zoom))
            .unit()
    }
    pub fn target(&self, x: f32, y: f32) -> V {
        let dir = self.ray(x, y);
        let t = -self.eye.z / dir.z;
        let p = self.eye + dir * t;
        V::new(p.x.clamp(-5., 5.), p.y.clamp(0.6, 8.), 0.)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovered_camera_is_centered_and_preserves_unity_screen_right() {
        let camera = Camera::new();
        assert_eq!(camera.eye.x, 0.);
        assert!((camera.forward.y.atan2(camera.forward.z).to_degrees() + 3.).abs() < 0.0001);
        assert!((2. * (400. / camera.zoom).atan().to_degrees() - 30.).abs() < 0.0001);
        // UnityPy reflects mesh X. A positive source X must remain right of screen center.
        let screen = camera.project(V::new(-1., 2., 0.)).unwrap().0;
        assert!(screen.x > 550.);
    }
    #[test]
    fn quaternion_rotation() {
        let v = rotate(
            V::new(1., 0., 0.),
            [
                0.,
                0.,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
        );
        assert!(v.x.abs() < 1e-5);
        assert!((v.y - 1.).abs() < 1e-5);
    }
    #[test]
    fn gpu_projection_matches_aiming() {
        for (width, editing) in [(DESKTOP_WIDTH, false), (370., false), (370., true)] {
            let mut c = Camera::new();
            c.resize(width, editing);
            let vp = c.view_projection();
            for p in [
                V::new(0., 2., 0.),
                V::new(3., 5., -2.),
                V::new(-2., 0.5, 3.),
            ] {
                let clip = vp * glam::Vec4::new(p.x, p.y, p.z, 1.);
                let ndc = clip.truncate() / clip.w;
                let screen = glam::Vec2::new((ndc.x + 1.) * width / 2., (1. - ndc.y) * 400.);
                let expected = c.project(p).unwrap().0;
                assert!((screen.x - expected.x).abs() < 0.001);
                assert!((screen.y - expected.y).abs() < 0.001);
                let dir = c.ray(screen.x, screen.y);
                assert!(dir.cross((p - c.eye).unit()).len() < 0.00001);
            }
        }
    }
    #[test]
    fn camera_ray_roundtrip() {
        let c = Camera::new();
        let p = V::new(1., 3., 0.);
        let (s, _) = c.project(p).unwrap();
        assert!((c.target(s.x, s.y) - p).len() < 1e-4);
    }
}

/// Low or high ballistic arc, matching the discriminant recovered from Cannon.TryCalculateBallisticVelocity.
/// Callers supply speed; gameplay uses the recovered Cannon.BallSpeed constant.
pub fn ballistic(start: V, target: V, speed: f32, gravity: f32, high: bool) -> Option<V> {
    let delta = target - start;
    let horizontal = V::new(delta.x, 0., delta.z);
    let range = horizontal.len();
    if range < 1e-4 {
        return None;
    }
    let v2 = speed * speed;
    let disc = v2 * v2 - gravity * (gravity * range * range + 2. * delta.y * v2);
    if disc < 0. {
        return None;
    }
    let tangent = (v2 + if high { disc.sqrt() } else { -disc.sqrt() }) / (gravity * range);
    let cos = 1. / (1. + tangent * tangent).sqrt();
    Some(horizontal.unit() * (speed * cos) + V::new(0., speed * cos * tangent, 0.))
}
#[cfg(test)]
mod ballistic_tests {
    use super::*;
    #[test]
    fn both_arcs_reach_target() {
        let start = V::new(0., 1., 10.);
        let target = V::new(3., 4., -2.);
        for high in [false, true] {
            let v = ballistic(start, target, 24., 9.81, high).unwrap();
            let horizontal = V::new(v.x, 0., v.z).len();
            let t = V::new(target.x - start.x, 0., target.z - start.z).len() / horizontal;
            let end = start + v * t + V::new(0., -4.905 * t * t, 0.);
            assert!((end - target).len() < 0.001);
        }
    }
    #[test]
    fn unreachable_target() {
        assert!(ballistic(V::default(), V::new(1000., 1000., 0.), 5., 9.81, false).is_none());
    }
}

pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = a;
    let [u, v, t, s] = b;
    [
        w * u + x * s + y * t - z * v,
        w * v - x * t + y * s + z * u,
        w * t + x * v - y * u + z * s,
        w * s - x * u - y * v - z * t,
    ]
}
