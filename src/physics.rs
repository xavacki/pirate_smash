use crate::math::V;
use std::ffi::c_void;
#[repr(C)]
#[derive(Default)]
struct Transform {
    x: f32,
    y: f32,
    z: f32,
    qx: f32,
    qy: f32,
    qz: f32,
    qw: f32,
}
#[repr(C)]
#[derive(Default)]
struct Contact {
    a: i32,
    b: i32,
    impulse: f32,
}
extern "C" {
    fn bp_prepare_stack(w: *mut c_void);
    fn bp_sync_kinematics(w: *mut c_void);
    fn bp_convex(
        w: *mut c_void,
        x: f32,
        y: f32,
        z: f32,
        xyz: *const f32,
        count: i32,
        mass: f32,
    ) -> i32;
    fn bp_material(
        w: *mut c_void,
        id: i32,
        friction: f32,
        restitution: f32,
        linear_drag: f32,
        angular_drag: f32,
    );
    fn bp_set_kinematic(w: *mut c_void, id: i32);
    fn bp_world_new(g: f32) -> *mut c_void;
    fn bp_world_free(w: *mut c_void);
    fn bp_box(w: *mut c_void, x: f32, y: f32, z: f32, hx: f32, hy: f32, hz: f32, m: f32) -> i32;
    fn bp_sphere(w: *mut c_void, x: f32, y: f32, z: f32, r: f32, m: f32) -> i32;
    fn bp_cylinder(
        w: *mut c_void,
        x: f32,
        y: f32,
        z: f32,
        hx: f32,
        hy: f32,
        hz: f32,
        m: f32,
    ) -> i32;
    fn bp_set_pose(
        w: *mut c_void,
        id: i32,
        x: f32,
        y: f32,
        z: f32,
        qx: f32,
        qy: f32,
        qz: f32,
        qw: f32,
    );
    fn bp_step(w: *mut c_void, dt: f32);
    fn bp_transform(w: *mut c_void, id: i32, t: *mut Transform) -> i32;
    fn bp_set_velocity(w: *mut c_void, id: i32, x: f32, y: f32, z: f32);
    fn bp_impulse(w: *mut c_void, id: i32, x: f32, y: f32, z: f32);
    fn bp_remove(w: *mut c_void, id: i32);
    fn bp_contact_count(w: *mut c_void) -> i32;
    fn bp_contact(w: *mut c_void, index: i32, out: *mut Contact) -> i32;
}
pub struct World(*mut c_void);
impl World {
    pub fn prepare_stack(&self) {
        unsafe { bp_prepare_stack(self.0) }
    }
    pub fn sync_kinematics(&self) {
        unsafe { bp_sync_kinematics(self.0) }
    }
    pub fn convex(&self, p: V, points: &[[f32; 3]], mass: f32) -> i32 {
        unsafe {
            bp_convex(
                self.0,
                p.x,
                p.y,
                p.z,
                points.as_ptr().cast(),
                points.len() as i32,
                mass,
            )
        }
    }
    pub fn material(&self, id: i32, friction: f32) {
        unsafe { bp_material(self.0, id, friction, 0., 0., 0.) }
    }
    pub fn kinematic(&self, id: i32) {
        unsafe { bp_set_kinematic(self.0, id) }
    }
    pub fn new() -> Self {
        let w = unsafe { bp_world_new(-9.81) };
        assert!(!w.is_null());
        Self(w)
    }
    pub fn cube(&self, p: V, h: V, m: f32) -> i32 {
        unsafe { bp_box(self.0, p.x, p.y, p.z, h.x, h.y, h.z, m) }
    }
    pub fn sphere(&self, p: V, r: f32, m: f32) -> i32 {
        unsafe { bp_sphere(self.0, p.x, p.y, p.z, r, m) }
    }
    pub fn cylinder(&self, p: V, h: V, m: f32) -> i32 {
        unsafe { bp_cylinder(self.0, p.x, p.y, p.z, h.x, h.y, h.z, m) }
    }
    pub fn set_pose(&self, id: i32, p: V, q: [f32; 4]) {
        unsafe { bp_set_pose(self.0, id, p.x, p.y, p.z, q[0], q[1], q[2], q[3]) }
    }
    pub fn step(&self, dt: f32) {
        unsafe { bp_step(self.0, dt) }
    }
    pub fn pose(&self, id: i32) -> (V, [f32; 4]) {
        let mut t = Transform::default();
        unsafe { bp_transform(self.0, id, &mut t) };
        (V::new(t.x, t.y, t.z), [t.qx, t.qy, t.qz, t.qw])
    }
    pub fn velocity(&self, id: i32, v: V) {
        unsafe { bp_set_velocity(self.0, id, v.x, v.y, v.z) }
    }
    pub fn impulse(&self, id: i32, v: V) {
        unsafe { bp_impulse(self.0, id, v.x, v.y, v.z) }
    }
    pub fn remove(&self, id: i32) {
        unsafe { bp_remove(self.0, id) }
    }
    pub fn contacts(&self) -> Vec<(i32, i32, f32)> {
        let n = unsafe { bp_contact_count(self.0) };
        let mut result = Vec::new();
        for i in 0..n {
            let mut c = Contact::default();
            if unsafe { bp_contact(self.0, i, &mut c) } != 0 {
                result.push((c.a, c.b, c.impulse));
            }
        }
        result
    }
}
impl Drop for World {
    fn drop(&mut self) {
        unsafe { bp_world_free(self.0) }
    }
}
