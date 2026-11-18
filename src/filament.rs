use crate::{
    math::Camera,
    scene::{Scene, Vertex},
    sdl::{Display, Ptr},
    ui::{Settings, UiVertex},
};
use anyhow::{ensure, Result};
use glam::Mat4;
use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
    ptr::NonNull,
    sync::Arc,
};

#[repr(C)]
struct Draw {
    transform: [f32; 16],
    color: [f32; 4],
    emissive: [f32; 3],
    metallic: f32,
    roughness: f32,
    coat: f32,
    mesh: u32,
    flags: u32,
}
extern "C" {
    fn smash_fila_create(
        path: *const c_char,
        samples: i32,
        error: *mut c_char,
        capacity: usize,
    ) -> Ptr;
    fn smash_fila_destroy(renderer: Ptr);
    fn smash_fila_window(renderer: Ptr, acquired: Ptr);
    fn smash_fila_texture(
        renderer: Ptr,
        image: *const u8,
        width: u32,
        height: u32,
        hud: bool,
    ) -> u32;
    fn smash_fila_mesh(
        renderer: Ptr,
        vertices: *const Vertex,
        vertex_count: u32,
        indices: *const u32,
        index_count: u32,
        texture: u32,
    ) -> u32;
    fn smash_fila_frame(
        renderer: Ptr,
        draw: *const Draw,
        draw_count: u32,
        hud: *const UiVertex,
        hud_count: u32,
        width: u32,
        height: u32,
        eye: *const f32,
        forward: *const f32,
        up: *const f32,
        fovy: f32,
        view_width: f32,
        bloom: f32,
        threshold: f32,
        exposure: f32,
    ) -> bool;
}
pub struct Renderer {
    handle: NonNull<std::ffi::c_void>,
    draws: Vec<Draw>,
}
impl Renderer {
    pub fn new(scene: &Scene, samples: i32) -> Result<Self> {
        let mut error = [0; 512];
        let handle = unsafe {
            smash_fila_create(
                c"assets/filament".as_ptr(),
                samples,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        let handle = NonNull::new(handle).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
            )
        })?;
        let result = Self {
            handle,
            draws: Vec::new(),
        };
        let mut textures = HashMap::new();
        for (id, p) in scene.primitives.iter().enumerate() {
            let texture = p.texture.as_ref().map_or(0, |image| {
                *textures
                    .entry(Arc::as_ptr(image))
                    .or_insert_with(|| unsafe {
                        smash_fila_texture(
                            handle.as_ptr(),
                            image.rgba.as_ptr(),
                            image.width as u32,
                            image.height as u32,
                            false,
                        )
                    })
            });
            let mesh = unsafe {
                smash_fila_mesh(
                    handle.as_ptr(),
                    p.vertices.as_ptr(),
                    p.vertices.len().try_into()?,
                    p.indices.as_ptr(),
                    p.indices.len().try_into()?,
                    texture,
                )
            };
            ensure!(mesh as usize == id, "Filament mesh index mismatch");
        }
        let atlas = crate::scene::load_image_path("assets/start-screen/atlas.png")?;
        unsafe {
            smash_fila_texture(
                handle.as_ptr(),
                atlas.rgba.as_ptr(),
                atlas.width as u32,
                atlas.height as u32,
                true,
            );
        }
        Ok(result)
    }
    pub fn attach(&mut self, display: &Display) -> Result<bool, String> {
        let window = display.acquire_native_window();
        unsafe {
            smash_fila_window(self.handle.as_ptr(), window);
        }
        Ok(!window.is_null())
    }
    pub fn detach(&mut self) {
        unsafe {
            smash_fila_window(self.handle.as_ptr(), std::ptr::null_mut());
        }
    }
    pub fn render(
        &mut self,
        scene: &Scene,
        world: &[Mat4],
        c: &Camera,
        settings: &Settings,
        pixels: (u32, u32),
        hud: &[UiVertex],
    ) -> Result<()> {
        ensure!(
            scene.nodes.len() == world.len(),
            "Filament render nodes and transforms differ"
        );
        self.draws.clear();
        for (node, transform) in scene.nodes.iter().zip(world) {
            for &mesh in &node.primitives {
                let p = &scene.primitives[mesh];
                let m = node.material.unwrap_or(p.material);
                self.draws.push(Draw {
                    transform: transform.to_cols_array(),
                    color: m.color,
                    emissive: m.emissive,
                    metallic: m.metallic,
                    roughness: m.roughness,
                    coat: m.clear_coat,
                    mesh: mesh as u32,
                    flags: u32::from(m.blend)
                        | u32::from(m.unlit) << 1
                        | u32::from(m.double_sided || m.glass) << 2
                        | u32::from(m.use_texture && p.texture.is_some()) << 3
                        | u32::from(mesh == scene.backdrop) << 4
                        | u32::from(mesh == scene.water) << 5,
                });
            }
        }
        let eye = [c.eye.x, c.eye.y, c.eye.z];
        let forward = [c.forward.x, c.forward.y, c.forward.z];
        let up = [c.up.x, c.up.y, c.up.z];
        unsafe {
            smash_fila_frame(
                self.handle.as_ptr(),
                self.draws.as_ptr(),
                self.draws.len().try_into()?,
                hud.as_ptr(),
                hud.len().try_into()?,
                pixels.0,
                pixels.1,
                eye.as_ptr(),
                forward.as_ptr(),
                up.as_ptr(),
                2. * (400. / c.zoom).atan().to_degrees(),
                c.width,
                settings.strength,
                settings.threshold,
                settings.exposure,
            );
        }
        Ok(())
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            smash_fila_destroy(self.handle.as_ptr());
        }
    }
}
