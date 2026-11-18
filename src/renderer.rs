// Adapted from Ion Bloom (MIT); see third_party/ion-bloom-LICENSE.
use crate::{
    scene::{Material, Scene, Vertex},
    ui::{Settings, UiVertex},
};
use anyhow::{bail, Context, Result};
use glam::{Mat3, Mat4, Vec3};
use glow::HasContext;
use std::{fs::File, io::BufWriter, rc::Rc};

type Gl = Rc<glow::Context>;

struct Program {
    gl: Gl,
    id: glow::Program,
}
impl Program {
    fn new(gl: &Gl, vertex: &str, fragment: &str) -> Result<Self> {
        unsafe {
            let id = gl.create_program().map_err(anyhow::Error::msg)?;
            let program = Self { gl: gl.clone(), id };
            let mut shaders = Vec::new();
            for (kind, source) in [
                (glow::VERTEX_SHADER, vertex),
                (glow::FRAGMENT_SHADER, fragment),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(s) => s,
                    Err(e) => {
                        for s in shaders {
                            gl.delete_shader(s);
                        }
                        bail!(e);
                    }
                };
                let source = if gl.get_parameter_string(glow::VERSION).contains("OpenGL ES") {
                    source.to_owned()
                } else {
                    source
                        .replace("#version 300 es", "#version 330 core")
                        .replace("precision highp float;", "")
                };
                gl.shader_source(shader, &source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let log = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    for s in shaders {
                        gl.delete_shader(s);
                    }
                    bail!("GLSL compile failed: {log}");
                }
                gl.attach_shader(id, shader);
                shaders.push(shader);
            }
            gl.link_program(id);
            for s in shaders {
                gl.detach_shader(id, s);
                gl.delete_shader(s);
            }
            if !gl.get_program_link_status(id) {
                bail!("GLSL link failed: {}", gl.get_program_info_log(id));
            }
            Ok(program)
        }
    }
    fn bind(&self) {
        unsafe {
            self.gl.use_program(Some(self.id));
        }
    }
    fn location(&self, name: &str) -> Option<glow::UniformLocation> {
        unsafe { self.gl.get_uniform_location(self.id, name) }
    }
    fn float(&self, name: &str, value: f32) {
        unsafe {
            self.gl.uniform_1_f32(self.location(name).as_ref(), value);
        }
    }
    fn int(&self, name: &str, value: i32) {
        unsafe {
            self.gl.uniform_1_i32(self.location(name).as_ref(), value);
        }
    }
    fn texture(&self, name: &str, unit: u32, texture: glow::Texture) {
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + unit);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        }
        self.int(name, unit as i32);
    }
}
impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.id);
        }
    }
}

struct Target {
    gl: Gl,
    fbo: glow::Framebuffer,
    texture: glow::Texture,
    depth: Option<glow::Renderbuffer>,
    width: i32,
    height: i32,
}
impl Target {
    fn new(gl: &Gl, width: i32, height: i32, hdr: bool, depth: bool) -> Result<Self> {
        unsafe {
            let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
            let fbo = match gl.create_framebuffer() {
                Ok(f) => f,
                Err(e) => {
                    gl.delete_texture(texture);
                    bail!(e);
                }
            };
            let mut target = Self {
                gl: gl.clone(),
                fbo,
                texture,
                depth: None,
                width,
                height,
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (p, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
            }
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                if hdr { glow::RGBA16F } else { glow::RGBA8 } as i32,
                width,
                height,
                0,
                glow::RGBA,
                if hdr {
                    glow::HALF_FLOAT
                } else {
                    glow::UNSIGNED_BYTE
                },
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            if depth {
                let rb = gl.create_renderbuffer().map_err(anyhow::Error::msg)?;
                target.depth = Some(rb);
                gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
                gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH_COMPONENT24, width, height);
                gl.framebuffer_renderbuffer(
                    glow::FRAMEBUFFER,
                    glow::DEPTH_ATTACHMENT,
                    glow::RENDERBUFFER,
                    Some(rb),
                );
            }
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            let error = gl.get_error();
            if status != glow::FRAMEBUFFER_COMPLETE || error != glow::NO_ERROR {
                bail!("{} render target {width}x{height}: framebuffer 0x{status:x}, GL error 0x{error:x}",if hdr { "HDR" } else { "LDR" });
            }
            Ok(target)
        }
    }
    fn bind(&self) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.viewport(0, 0, self.width, self.height);
        }
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        unsafe {
            if let Some(d) = self.depth {
                self.gl.delete_renderbuffer(d);
            }
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.texture);
        }
    }
}

// Render geometry with sample coverage, then resolve to the texture used by bloom.
// Window-system MSAA alone cannot antialias an offscreen postprocessing target.
struct MultisampleTarget {
    gl: Gl,
    fbo: glow::Framebuffer,
    buffers: Vec<glow::Renderbuffer>,
    samples: i32,
}
impl MultisampleTarget {
    fn new(gl: &Gl, width: i32, height: i32, hdr: bool, samples: i32) -> Result<Self> {
        unsafe {
            let fbo = gl.create_framebuffer().map_err(anyhow::Error::msg)?;
            let mut target = Self {
                gl: gl.clone(),
                fbo,
                buffers: Vec::new(),
                samples,
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            for (attachment, format) in [
                (
                    glow::COLOR_ATTACHMENT0,
                    if hdr { glow::RGBA16F } else { glow::RGBA8 },
                ),
                (glow::DEPTH_ATTACHMENT, glow::DEPTH_COMPONENT24),
            ] {
                let buffer = gl.create_renderbuffer().map_err(anyhow::Error::msg)?;
                target.buffers.push(buffer);
                gl.bind_renderbuffer(glow::RENDERBUFFER, Some(buffer));
                gl.renderbuffer_storage_multisample(
                    glow::RENDERBUFFER,
                    samples,
                    format,
                    width,
                    height,
                );
                gl.framebuffer_renderbuffer(
                    glow::FRAMEBUFFER,
                    attachment,
                    glow::RENDERBUFFER,
                    Some(buffer),
                );
            }
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            // Some formats support fewer samples than GL_MAX_SAMPLES. Failed
            // probes may leave multiple errors; consume all before retrying.
            let mut error = gl.get_error();
            let failed = error != glow::NO_ERROR;
            while error != glow::NO_ERROR {
                error = gl.get_error();
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            if failed || status != glow::FRAMEBUFFER_COMPLETE {
                bail!("{samples}x MSAA unavailable for this framebuffer format");
            }
            target.samples =
                gl.get_renderbuffer_parameter_i32(glow::RENDERBUFFER, glow::RENDERBUFFER_SAMPLES);
            if target.samples > samples {
                bail!(
                    "Driver rounded {samples}x MSAA up to {}x; trying a lower setting",
                    target.samples
                );
            }
            Ok(target)
        }
    }
    fn choose(gl: &Gl, width: i32, height: i32, hdr: bool, requested: i32) -> Option<Self> {
        let max = unsafe { gl.get_parameter_i32(glow::MAX_SAMPLES) };
        for samples in [8, 4, 2] {
            if samples <= requested && samples <= max {
                match Self::new(gl, width, height, hdr, samples) {
                    Ok(target) => return Some(target),
                    Err(error) => eprintln!("{error:#}"),
                }
            }
        }
        if requested > 0 {
            eprintln!("MSAA unavailable; continuing with single-sample rendering.");
        }
        None
    }
    fn bind(&self, width: i32, height: i32) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.viewport(0, 0, width, height);
        }
    }
    fn resolve(&self, destination: &Target) {
        unsafe {
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.fbo));
            self.gl
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(destination.fbo));
            self.gl.blit_framebuffer(
                0,
                0,
                destination.width,
                destination.height,
                0,
                0,
                destination.width,
                destination.height,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
        }
    }
}
impl Drop for MultisampleTarget {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            for &buffer in &self.buffers {
                self.gl.delete_renderbuffer(buffer);
            }
        }
    }
}

struct Geometry {
    gl: Gl,
    vao: glow::VertexArray,
    buffers: Vec<glow::Buffer>,
    count: i32,
    center: Vec3,
}
impl Geometry {
    fn empty(gl: &Gl) -> Result<Self> {
        let vao = unsafe { gl.create_vertex_array().map_err(anyhow::Error::msg)? };
        Ok(Self {
            gl: gl.clone(),
            vao,
            buffers: Vec::new(),
            count: 0,
            center: Vec3::ZERO,
        })
    }
    fn buffer(&mut self, target: u32, bytes: &[u8], usage: u32) -> Result<()> {
        unsafe {
            let b = self.gl.create_buffer().map_err(anyhow::Error::msg)?;
            self.buffers.push(b);
            self.gl.bind_buffer(target, Some(b));
            self.gl.buffer_data_u8_slice(target, bytes, usage);
        }
        Ok(())
    }
    fn mesh(gl: &Gl, vertices: &[Vertex], indices: &[u32]) -> Result<Self> {
        let mut mesh = Self::empty(gl)?;
        unsafe {
            gl.bind_vertex_array(Some(mesh.vao));
            mesh.buffer(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(vertices),
                glow::STATIC_DRAW,
            )?;
            mesh.buffer(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(indices),
                glow::STATIC_DRAW,
            )?;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 48, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 48, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, 48, 24);
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 2, glow::FLOAT, false, 48, 40);
            gl.bind_vertex_array(None);
        }
        mesh.count = indices.len().try_into().context("Too many indices")?;
        if !vertices.is_empty() {
            let min = vertices.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
                a.min(Vec3::from_array(v.position))
            });
            let max = vertices
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |a, v| {
                    a.max(Vec3::from_array(v.position))
                });
            mesh.center = (min + max) * 0.5;
        }
        Ok(mesh)
    }
}
impl Drop for Geometry {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.vao);
            for &b in &self.buffers {
                self.gl.delete_buffer(b);
            }
        }
    }
}

struct Targets {
    scene: Target,
    multisample: Option<MultisampleTarget>,
    bright: Target,
    levels: Vec<(Target, Target)>,
}
impl Targets {
    fn new(gl: &Gl, w: i32, h: i32, hdr: bool, samples: i32) -> Result<Self> {
        let multisample = MultisampleTarget::choose(gl, w, h, hdr, samples);
        let scene = Target::new(gl, w, h, hdr, multisample.is_none())?;
        let (mut w, mut h) = ((w / 2).max(1), (h / 2).max(1));
        let bright = Target::new(gl, w, h, hdr, false)?;
        let mut levels = Vec::new();
        for _ in 0..5 {
            levels.push((
                Target::new(gl, w, h, hdr, false)?,
                Target::new(gl, w, h, hdr, false)?,
            ));
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Ok(Self {
            scene,
            multisample,
            bright,
            levels,
        })
    }
}

pub struct Renderer {
    gl: Gl,
    scene_program: Program,
    water_program: Program,
    bright_program: Program,
    blur_program: Program,
    composite_program: Program,
    ui_program: Program,
    meshes: Vec<Geometry>,
    textures: Vec<Option<glow::Texture>>,
    fullscreen: Geometry,
    ui: Geometry,
    ui_atlas: glow::Texture,
    targets: Option<Targets>,
    pub hdr: bool,
    max_size: i32,
    requested_samples: i32,
}

impl Renderer {
    pub fn new(gl: glow::Context, scene: &Scene, force_ldr: bool, samples: i32) -> Result<Self> {
        let gl = Rc::new(gl);
        unsafe {
            eprintln!(
                "OpenGL: {} / {}",
                gl.get_parameter_string(glow::VERSION),
                gl.get_parameter_string(glow::RENDERER)
            );
        }
        let scene_program = Program::new(
            &gl,
            include_str!("shaders/scene.vert"),
            include_str!("shaders/scene.frag"),
        )?;
        let water_program = Program::new(
            &gl,
            include_str!("shaders/scene.vert"),
            concat!(
                "#version 300 es\nprecision highp float;\n",
                include_str!("shaders/water.glsl"),
                "\nin vec3 v_position;\n",
                "uniform vec3 u_eye, u_forward, u_up;\n",
                "uniform vec2 u_half_fov;\nuniform float u_time;\n",
                "uniform sampler2D u_harbor, u_mask;\nout vec4 color;\n",
                "void main() { vec3 water = harborWater(v_position, u_eye, u_forward, u_up, ",
                "u_half_fov, u_time, u_harbor, u_mask); color = vec4(pow(water, vec3(2.2)), 1.0); }"
            ),
        )?;
        let full = include_str!("shaders/fullscreen.vert");
        let bright_program = Program::new(&gl, full, include_str!("shaders/bright.frag"))?;
        let blur_program = Program::new(&gl, full, include_str!("shaders/blur.frag"))?;
        let composite_program = Program::new(&gl, full, include_str!("shaders/composite.frag"))?;
        let ui_program = Program::new(
            &gl,
            include_str!("shaders/ui.vert"),
            include_str!("shaders/ui.frag"),
        )?;
        let meshes = scene
            .primitives
            .iter()
            .map(|p| Geometry::mesh(&gl, &p.vertices, &p.indices))
            .collect::<Result<_>>()?;
        let mut uploaded = std::collections::HashMap::new();
        let textures = scene
            .primitives
            .iter()
            .map(|p| -> Result<_> {
                let Some(image) = &p.texture else {
                    return Ok(None);
                };
                let key = std::sync::Arc::as_ptr(image);
                if let Some(&texture) = uploaded.get(&key) {
                    return Ok(Some(texture));
                }
                unsafe {
                    let t = gl.create_texture().map_err(anyhow::Error::msg)?;
                    gl.bind_texture(glow::TEXTURE_2D, Some(t));
                    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        image.width,
                        image.height,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(&image.rgba)),
                    );
                    gl.generate_mipmap(glow::TEXTURE_2D);
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MIN_FILTER,
                        glow::LINEAR_MIPMAP_LINEAR as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MAG_FILTER,
                        glow::LINEAR as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_S,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_T,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    uploaded.insert(key, t);
                    Ok(Some(t))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let fullscreen = Geometry::empty(&gl)?;
        let mut ui = Geometry::empty(&gl)?;
        unsafe {
            gl.bind_vertex_array(Some(ui.vao));
            ui.buffer(glow::ARRAY_BUFFER, &[], glow::DYNAMIC_DRAW)?;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 32, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, 32, 8);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, 32, 24);
            gl.bind_vertex_array(None);
        }
        let atlas = crate::scene::load_image_path("assets/start-screen/atlas.png")?;
        let ui_atlas = unsafe {
            let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                atlas.width,
                atlas.height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&atlas.rgba)),
            );
            for (param, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, param, value as i32);
            }
            texture
        };
        let max_size = unsafe {
            gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE)
                .min(gl.get_parameter_i32(glow::MAX_RENDERBUFFER_SIZE))
                .min(1920)
        };
        Ok(Self {
            gl,
            scene_program,
            water_program,
            bright_program,
            blur_program,
            composite_program,
            ui_program,
            meshes,
            textures,
            fullscreen,
            ui,
            ui_atlas,
            targets: None,
            hdr: !force_ldr,
            max_size,
            requested_samples: samples,
        })
    }

    fn resize(&mut self, w: u32, h: u32) -> Result<()> {
        // Cap intermediate resolution to control fill rate and mobile memory.
        let scale = (self.max_size as f32 / (w.max(h) as f32)).min(1.0);
        let (w, h) = (
            ((w as f32 * scale) as i32).max(1),
            ((h as f32 * scale) as i32).max(1),
        );
        if self
            .targets
            .as_ref()
            .is_some_and(|t| t.scene.width == w && t.scene.height == h)
        {
            return Ok(());
        }
        // Release the old chain first: rotation shouldn't temporarily double GPU memory use.
        self.targets = None;
        match Targets::new(&self.gl, w, h, self.hdr, self.requested_samples) {
            Ok(t) => self.targets = Some(t),
            Err(e) if self.hdr => {
                eprintln!("{e:#}; falling back to RGBA8 (highlights will clip).");
                self.hdr = false;
                self.targets = Some(Targets::new(&self.gl, w, h, false, self.requested_samples)?);
            }
            Err(e) => return Err(e),
        }
        eprintln!(
            "Bloom targets: {w}x{h}, {}, {}x samples",
            if self.hdr { "RGBA16F" } else { "RGBA8" },
            self.targets
                .as_ref()
                .and_then(|t| t.multisample.as_ref())
                .map_or(1, |t| t.samples)
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // Frame inputs are explicit and share no ownership.
    pub fn render(
        &mut self,
        scene: &Scene,
        world: &[Mat4],
        camera: &crate::math::Camera,
        time: f32,
        settings: &Settings,
        pixels: (u32, u32),
        logical: (f32, f32),
        hud: &[UiVertex],
    ) -> Result<()> {
        let vp = camera.view_projection();
        let eye = Vec3::new(camera.eye.x, camera.eye.y, camera.eye.z);
        self.resize(pixels.0, pixels.1)?;
        let targets = self.targets.as_ref().unwrap();
        let gl = &self.gl;
        unsafe {
            if let Some(msaa) = &targets.multisample {
                msaa.bind(targets.scene.width, targets.scene.height);
            } else {
                targets.scene.bind();
            }
            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.disable(glow::BLEND);
            gl.clear_color(0.018, 0.027, 0.046, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            if let Some((index, _)) = scene
                .nodes
                .iter()
                .enumerate()
                .find(|(_, node)| node.primitives.contains(&scene.water))
            {
                let water = &self.water_program;
                water.bind();
                gl.disable(glow::CULL_FACE);
                gl.uniform_matrix_4_f32_slice(
                    water.location("u_view_projection").as_ref(),
                    false,
                    &vp.to_cols_array(),
                );
                gl.uniform_matrix_4_f32_slice(
                    water.location("u_model").as_ref(),
                    false,
                    &world[index].to_cols_array(),
                );
                gl.uniform_3_f32(water.location("u_eye").as_ref(), eye.x, eye.y, eye.z);
                gl.uniform_3_f32(
                    water.location("u_forward").as_ref(),
                    camera.forward.x,
                    camera.forward.y,
                    camera.forward.z,
                );
                gl.uniform_3_f32(
                    water.location("u_up").as_ref(),
                    camera.up.x,
                    camera.up.y,
                    camera.up.z,
                );
                gl.uniform_2_f32(
                    water.location("u_half_fov").as_ref(),
                    camera.width / (2. * camera.zoom),
                    400. / camera.zoom,
                );
                water.float("u_time", time);
                water.texture("u_harbor", 0, self.textures[scene.backdrop].unwrap());
                water.texture("u_mask", 1, self.textures[scene.water].unwrap());
                let mesh = &self.meshes[scene.water];
                gl.bind_vertex_array(Some(mesh.vao));
                gl.draw_elements(glow::TRIANGLES, mesh.count, glow::UNSIGNED_INT, 0);
            }
            let p = &self.scene_program;
            p.bind();
            gl.uniform_matrix_4_f32_slice(
                p.location("u_view_projection").as_ref(),
                false,
                &vp.to_cols_array(),
            );
            gl.uniform_3_f32(p.location("u_camera").as_ref(), eye.x, eye.y, eye.z);
            let mut transparent = Vec::new();
            for (id, node) in scene.nodes.iter().enumerate() {
                for &primitive in &node.primitives {
                    if primitive == scene.water {
                        continue;
                    }
                    let material = node
                        .material
                        .unwrap_or(scene.primitives[primitive].material);
                    if material.blend {
                        let distance = world[id]
                            .transform_point3(self.meshes[primitive].center)
                            .distance_squared(eye);
                        transparent.push((distance, id, primitive));
                    } else {
                        self.draw_mesh(primitive, world[id], material);
                    }
                }
            }
            transparent.sort_by(|a, b| b.0.total_cmp(&a.0));
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.depth_mask(false);
            for (_, id, primitive) in transparent {
                self.draw_mesh(
                    primitive,
                    world[id],
                    scene.nodes[id]
                        .material
                        .unwrap_or(scene.primitives[primitive].material),
                );
            }
            gl.depth_mask(true);
            gl.disable(glow::BLEND);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            if let Some(msaa) = &targets.multisample {
                msaa.resolve(&targets.scene);
            }
            gl.bind_vertex_array(Some(self.fullscreen.vao));
            targets.bright.bind();
            self.bright_program.bind();
            self.bright_program
                .texture("u_source", 0, targets.scene.texture);
            self.bright_program.float("u_threshold", settings.threshold);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            let mut source = targets.bright.texture;
            self.blur_program.bind();
            for (i, (horizontal, vertical)) in targets.levels.iter().enumerate() {
                self.blur_program.int("u_radius", [3, 5, 7, 9, 11][i]);
                horizontal.bind();
                self.blur_program.texture("u_source", 0, source);
                gl.uniform_2_f32(
                    self.blur_program.location("u_step").as_ref(),
                    1.0 / horizontal.width as f32,
                    0.0,
                );
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                vertical.bind();
                self.blur_program.texture("u_source", 0, horizontal.texture);
                gl.uniform_2_f32(
                    self.blur_program.location("u_step").as_ref(),
                    0.0,
                    1.0 / vertical.height as f32,
                );
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                source = vertical.texture;
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, pixels.0 as i32, pixels.1 as i32);
            let p = &self.composite_program;
            p.bind();
            p.texture("u_scene", 0, targets.scene.texture);
            for (i, (_, vertical)) in targets.levels.iter().enumerate() {
                p.texture(&format!("u_bloom{i}"), i as u32 + 1, vertical.texture);
            }
            p.float("u_strength", settings.strength);
            p.float("u_radius", settings.radius);
            p.float("u_exposure", settings.exposure.powi(4));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            if !hud.is_empty() {
                let p = &self.ui_program;
                p.bind();
                p.texture("u_atlas", 0, self.ui_atlas);
                gl.uniform_2_f32(p.location("u_size").as_ref(), logical.0, logical.1);
                gl.bind_vertex_array(Some(self.ui.vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ui.buffers[0]));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(hud),
                    glow::DYNAMIC_DRAW,
                );
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.draw_arrays(glow::TRIANGLES, 0, hud.len() as i32);
                gl.disable(glow::BLEND);
            }
            gl.bind_vertex_array(None);
            let error = gl.get_error();
            if error != glow::NO_ERROR {
                bail!("Render failed with GL error 0x{error:x}");
            }
        }
        Ok(())
    }

    unsafe fn draw_mesh(&self, primitive: usize, world: Mat4, material: Material) {
        let gl = &self.gl;
        let p = &self.scene_program;
        if material.double_sided {
            gl.disable(glow::CULL_FACE);
        } else {
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
        }
        gl.uniform_matrix_4_f32_slice(
            p.location("u_model").as_ref(),
            false,
            &world.to_cols_array(),
        );
        gl.uniform_matrix_3_f32_slice(
            p.location("u_normal").as_ref(),
            false,
            &Mat3::from_mat4(world).inverse().transpose().to_cols_array(),
        );
        gl.uniform_4_f32_slice(p.location("u_color").as_ref(), &material.color);
        gl.uniform_3_f32_slice(p.location("u_emissive").as_ref(), &material.emissive);
        p.float("u_metallic", material.metallic);
        p.float("u_roughness", material.roughness);
        p.float("u_clear_coat", material.clear_coat);
        p.int("u_glass", i32::from(material.glass));
        p.int("u_unlit", i32::from(material.unlit));
        if let Some(texture) = self.textures[primitive].filter(|_| material.use_texture) {
            p.texture("u_base_map", 0, texture);
            p.int("u_has_texture", 1);
        } else {
            p.int("u_has_texture", 0);
        }
        gl.bind_vertex_array(Some(self.meshes[primitive].vao));
        if material.glass {
            // Draw the back shell before the front shell; keep depth writes disabled.
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::FRONT);
            gl.draw_elements(
                glow::TRIANGLES,
                self.meshes[primitive].count,
                glow::UNSIGNED_INT,
                0,
            );
            gl.cull_face(glow::BACK);
        }
        gl.draw_elements(
            glow::TRIANGLES,
            self.meshes[primitive].count,
            glow::UNSIGNED_INT,
            0,
        );
    }

    pub fn screenshot(&self, path: &str, width: u32, height: u32) -> Result<()> {
        let stride = width as usize * 4;
        let mut pixels = vec![0; stride * height as usize];
        unsafe {
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            let error = self.gl.get_error();
            if error != glow::NO_ERROR {
                bail!("Screenshot readback failed: 0x{error:x}");
            }
        }
        let mut flipped = vec![0; pixels.len()];
        for y in 0..height as usize {
            flipped[y * stride..(y + 1) * stride].copy_from_slice(
                &pixels[(height as usize - 1 - y) * stride..(height as usize - y) * stride],
            );
        }
        let mut encoder = png::Encoder::new(
            BufWriter::new(File::create(path).context("Create screenshot")?),
            width,
            height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&flipped)?;
        eprintln!("Saved {path}");
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_texture(self.ui_atlas);
            for t in self
                .textures
                .iter()
                .flatten()
                .copied()
                .collect::<std::collections::HashSet<_>>()
            {
                self.gl.delete_texture(t);
            }
        }
    }
}
