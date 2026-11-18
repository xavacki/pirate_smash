import { cubeVertices, fetchAsset, meshVertices } from "./data.js";

const F = () => window.Filament;
// TextureUsage values from Filament 1.70 DriverEnums.h.
const MIP_TEXTURE_USAGE = 0x0008 | 0x0010 | 0x0200;
const cross = ([ax, ay, az], [bx, by, bz]) => [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx];
const normalize = (v) => {
    const length = Math.hypot(...v);
    return length > 1e-10 ? v.map((x) => x / length) : null;
};

function modelMatrix(position, rotation, scale) {
    const [x, y, z, w] = normalize(rotation) ?? [0,0,0,1];
    return [
        (1-2*(y*y+z*z))*scale[0], 2*(x*y+w*z)*scale[0], 2*(x*z-w*y)*scale[0], 0,
        2*(x*y-w*z)*scale[1], (1-2*(x*x+z*z))*scale[1], 2*(y*z+w*x)*scale[1], 0,
        2*(x*z+w*y)*scale[2], 2*(y*z-w*x)*scale[2], (1-2*(x*x+y*y))*scale[2], 0,
        ...position, 1,
    ];
}

function tangentFrame(normal) {
    const [x, y, z] = normalize(normal) ?? [0, 1, 0];
    if (z < -.999999) return [1,0,0,1/32767];
    return normalize([-y, x, 0, 1 + z]);
}

export class Preview {
    static async create(canvas, onPick) {
        if (!F()) throw new Error("Filament Web is missing. Run web-editor/setup-filament.sh.");
        await new Promise((resolve, reject) => {
            const timeout = setTimeout(() => reject(new Error("Filament Web did not initialize.")), 12000);
            F().init([], () => { clearTimeout(timeout); resolve(); });
        });
        checkWebGL(canvas);
        const assets = await Promise.all(["lit.filamat", "glass.filamat", "studio.ktx"].map(async (asset) =>
            new Uint8Array(await (await fetchAsset(`assets/filament/${asset}`)).arrayBuffer())));
        return new Preview(canvas, onPick, ...assets);
    }

    constructor(canvas, onPick, lit, glass, studio) {
        this.canvas = canvas;
        this.onPick = onPick;
        const filament = F();
        const engine = this.engine = filament.Engine.create(canvas, { alpha: false });
        this.entities = filament.EntityManager.get();
        this.renderer = engine.createRenderer();
        this.renderer.setClearOptions({ clear: true, clearColor: [.018, .027, .046, 1] });
        this.chain = engine.createSwapChain();
        this.scene = engine.createScene();
        this.view = engine.createView();
        this.camera = engine.createCamera(this.entities.create());
        this.camera.setExposure(16, 1 / 125, 100);
        this.view.setCamera(this.camera);
        this.view.setScene(this.scene);
        this.view.setAntiAliasing(filament.View$AntiAliasing.FXAA);
        this.view.setMultiSampleAntiAliasingOptions({ enabled: true, sampleCount: 4 });
        this.view.setBloomOptions({ enabled: true, strength: .35, threshold: true, highlight: 24, quality: filament.View$QualityLevel.MEDIUM });
        this.view.setTransparentPickingEnabled(true);
        this.scene.setSkybox(filament.Skybox.Builder().color([.027,.045,.08,1]).build(engine));
        const ibl = engine.createIblFromKtx1(studio);
        this.scene.setIndirectLight(filament.IndirectLight.Builder()
            .reflections(ibl.getReflectionsTexture()).irradianceSh(3, new Float32Array(ibl.shfloats))
            .intensity(15000).build(engine));
        engine.destroyIndirectLight(ibl);
        this.addLight(filament.LightManager$Type.SUN, [.5, -1, .45], [1, .76, .48], 52000, true);
        this.addLight(filament.LightManager$Type.DIRECTIONAL, [-.65, -.4, -.55], [.52, .73, 1], 21000, false);

        this.lit = engine.createMaterial(lit);
        this.glass = engine.createMaterial(glass);
        this.white = filament.Texture.Builder().width(1).height(1).levels(1)
            .format(filament.Texture$InternalFormat.SRGB8_A8).build(engine);
        this.white.setImage(engine, 0, filament.PixelBuffer(new Uint8Array([255,255,255,255]),
            filament.PixelDataFormat.RGBA, filament.PixelDataType.UBYTE));
        this.sampler = new filament.TextureSampler(filament.MinFilter.LINEAR_MIPMAP_LINEAR,
            filament.MagFilter.LINEAR, filament.WrapMode.REPEAT);
        this.cube = this.makeMesh(cubeVertices());
        this.meshes = new Map();
        this.textures = new Map();
        this.current = [];
        this.pickable = new Map();
        this.epoch = 0;
        this.center = [0, 1, 0];
        this.distance = 8;
        this.yaw = -.32;
        this.pitch = .19;
        this.fov = 34;
        this.resizeObserver = new ResizeObserver(() => this.resize());
        this.resizeObserver.observe(canvas);
        this.bindControls();
        this.resize();
    }

    addLight(type, direction, color, intensity, shadows) {
        const entity = this.entities.create();
        const builder = F().LightManager.Builder(type).direction(direction).color(color).intensity(intensity).castShadows(shadows);
        builder.build(this.engine, entity);
        this.scene.addEntity(entity);
    }

    makeMesh(vertices) {
        const filament = F();
        const count = vertices.length;
        const data = new Float32Array(count * 13);
        const indices = new Uint32Array(count);
        const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
        vertices.forEach(({ position, normal, uv }, i) => {
            data.set([...position, ...tangentFrame(normal), 1,1,1,1, ...uv], i * 13);
            indices[i] = i;
            position.forEach((v, axis) => { min[axis] = Math.min(min[axis], v); max[axis] = Math.max(max[axis], v); });
        });
        const attr = filament.VertexAttribute, type = filament.VertexBuffer$AttributeType;
        const vb = filament.VertexBuffer.Builder().vertexCount(count).bufferCount(1)
            .attribute(attr.POSITION, 0, type.FLOAT3, 0, 52)
            .attribute(attr.TANGENTS, 0, type.FLOAT4, 12, 52)
            .attribute(attr.COLOR, 0, type.FLOAT4, 28, 52)
            .attribute(attr.UV0, 0, type.FLOAT2, 44, 52)
            .build(this.engine);
        const ib = filament.IndexBuffer.Builder().indexCount(count)
            .bufferType(filament.IndexBuffer$IndexType.UINT).build(this.engine);
        vb.setBufferAt(this.engine, 0, data);
        ib.setBuffer(this.engine, indices);
        return { vb, ib, bounds: { center: min.map((v, i) => (v + max[i]) / 2),
            halfExtent: min.map((v, i) => Math.max(.001, (max[i] - v) / 2)) } };
    }

    texture(path) {
        if (!path) return Promise.resolve({ texture: this.white, alpha: false });
        if (!this.textures.has(path)) {
            const pending = fetchAsset(path).then((r) => r.arrayBuffer()).then((buffer) => {
                const filament = F();
                const source = filament.Buffer(new Uint8Array(buffer));
                let decoded;
                try { decoded = filament.decodeImage(source, 4); }
                finally { source.delete(); }
                const pixels = new Uint8Array(decoded.data.getBytes());
                let alpha = false;
                for (let i = 3; i < pixels.length && !alpha; i += 4) alpha = pixels[i] < 255;
                const texture = filament.Texture.Builder().width(decoded.width).height(decoded.height).levels(255).usage(MIP_TEXTURE_USAGE)
                    .format(filament.Texture$InternalFormat.SRGB8_A8).build(this.engine);
                const image = filament.PixelBuffer(pixels, filament.PixelDataFormat.RGBA, filament.PixelDataType.UBYTE);
                texture.setImage(this.engine, 0, image);
                texture.generateMipmaps(this.engine);
                decoded.data.delete();
                return { texture, alpha };
            }).catch((error) => { this.textures.delete(path); throw error; });
            this.textures.set(path, pending);
        }
        return this.textures.get(path);
    }

    async display(items) {
        const epoch = ++this.epoch;
        const resources = await Promise.all(items.map((item) => this.texture(item.texture)));
        if (epoch !== this.epoch) return;

        for (const { entity, material } of this.current) {
            this.scene.remove(entity);
            this.engine.destroyEntity(entity);
            this.entities.destroy(entity);
            entity.delete();
            this.engine.destroyMaterialInstance(material);
        }
        this.current = [];
        this.pickable.clear();

        const active = new Set();
        for (const [i, item] of items.entries()) {
            let mesh = this.cube;
            if (item.obj) {
                let cached = this.meshes.get(item.obj);
                const revision = item.obj.revision ?? 0;
                if (cached?.revision !== revision) {
                    if (cached) this.destroyMesh(cached);
                    cached = { ...this.makeMesh(meshVertices(item.obj)), revision };
                    this.meshes.set(item.obj, cached);
                }
                mesh = cached;
                active.add(item.obj);
            }
            const { texture, alpha } = resources[i];
            const blend = alpha || item.color[3] < .999;
            const material = (blend ? this.glass : this.lit).createInstance();
            material.setColor4Parameter("tint", F().RgbaType.sRGB, item.color);
            material.setFloat3Parameter("emit", item.selected ? [120, 75, 15] : [0,0,0]);
            material.setTextureParameter("tex", texture, this.sampler);
            material.setFloatParameter("metal", item.metal ?? 0);
            material.setFloatParameter("rough", item.rough ?? .6);
            material.setFloatParameter("coat", item.coat ?? 0);
            const entity = this.entities.create();
            F().RenderableManager.Builder(1).boundingBox(mesh.bounds)
                .geometry(0, F().RenderableManager$PrimitiveType.TRIANGLES, mesh.vb, mesh.ib)
                .material(0, material).castShadows(!blend && item.id != null)
                .receiveShadows(true).build(this.engine, entity);
            const transforms = this.engine.getTransformManager();
            const instance = transforms.getInstance(entity);
            transforms.setTransform(instance, modelMatrix(item.position, item.rotation, item.scale));
            instance.delete();
            this.scene.addEntity(entity);
            this.current.push({ entity, material });
            if (item.id != null) this.pickable.set(entity.getId(), item.id);
        }
        for (const [obj, mesh] of this.meshes) {
            if (!active.has(obj)) {
                this.destroyMesh(mesh);
                this.meshes.delete(obj);
            }
        }
        const usedTextures = new Set(items.map((item) => item.texture));
        for (const [path, pending] of this.textures) {
            if (usedTextures.has(path)) continue;
            this.textures.delete(path);
            pending.then(({ texture }) => this.engine.destroyTexture(texture), () => {});
        }
        this.requestFrame();
    }

    destroyMesh(mesh) {
        this.engine.destroyVertexBuffer(mesh.vb);
        this.engine.destroyIndexBuffer(mesh.ib);
    }

    fit(center, radius) {
        this.center = center;
        this.yaw = -.32;
        this.pitch = .19;
        const aspect = this.canvas.width / Math.max(1, this.canvas.height);
        this.distance = Math.max(2, radius * 1.28 / Math.sin(Math.atan(Math.tan(this.fov * Math.PI / 360) * Math.min(1, aspect))));
        this.requestFrame();
    }

    resize() {
        const scale = Math.min(window.devicePixelRatio || 1, 2);
        const width = Math.max(1, Math.round(this.canvas.clientWidth * scale));
        const height = Math.max(1, Math.round(this.canvas.clientHeight * scale));
        if (width !== this.canvas.width) this.canvas.width = width;
        if (height !== this.canvas.height) this.canvas.height = height;
        this.view.setViewport([0, 0, width, height]);
        this.camera.setProjectionFov(this.fov, width / height, .1, 1500, F().Camera$Fov.VERTICAL);
        this.requestFrame();
    }

    requestFrame(count = 3) {
        this.frames = Math.max(count, this.frames ?? 0);
        if (!this.framePending) {
            this.framePending = true;
            requestAnimationFrame(() => this.drawFrame());
        }
    }

    drawFrame() {
        this.framePending = false;
        const cos = Math.cos(this.pitch);
        const offset = [this.distance * Math.sin(this.yaw) * cos,
            this.distance * Math.sin(this.pitch), -this.distance * Math.cos(this.yaw) * cos];
        this.eye = this.center.map((v, i) => v + offset[i]);
        this.camera.lookAt(this.eye, this.center, [0,1,0]);
        this.renderer.render(this.chain, this.view);
        this.onFrame?.();
        this.frames--;
        if (this.frames > 0) this.requestFrame(this.frames);
    }

    project(point) {
        if (!this.eye) return null;
        const forward = normalize(this.center.map((v, i) => v - this.eye[i]));
        const right = normalize(cross(forward, [0,1,0]));
        const up = cross(right, forward);
        const offset = point.map((v, i) => v - this.eye[i]);
        const dot = (a, b) => a.reduce((sum, v, i) => sum + v * b[i], 0);
        const depth = dot(offset, forward);
        if (depth <= .1) return null;
        const px = this.canvas.clientHeight / (2 * depth * Math.tan(this.fov * Math.PI / 360));
        return [this.canvas.clientWidth / 2 + px * dot(offset, right),
            this.canvas.clientHeight / 2 - px * dot(offset, up), depth];
    }

    bindControls() {
        const canvas = this.canvas;
        let drag = null;
        canvas.addEventListener("pointerdown", (event) => {
            if (event.button !== 0) return;
            canvas.setPointerCapture(event.pointerId);
            drag = { x: event.clientX, y: event.clientY, moved: false };
        });
        canvas.addEventListener("pointermove", (event) => {
            if (!drag) return;
            const dx = event.clientX - drag.x, dy = event.clientY - drag.y;
            if (!drag.moved && Math.hypot(dx, dy) < 4) return;
            drag = { x: event.clientX, y: event.clientY, moved: true };
            canvas.classList.add("dragging");
            if (event.shiftKey) {
                const forward = normalize(this.center.map((v, i) => v - this.eye[i]));
                const right = normalize(cross(forward, [0,1,0]));
                const up = cross(right, forward);
                this.center = this.center.map((v, i) => v + this.distance * .0014 * (-dx * right[i] + dy * up[i]));
            } else {
                this.yaw -= dx * .006;
                this.pitch = Math.min(1.52, Math.max(-1.52, this.pitch + dy * .006));
            }
            this.requestFrame();
        });
        canvas.addEventListener("pointercancel", () => { drag = null; canvas.classList.remove("dragging"); });
        canvas.addEventListener("pointerup", (event) => {
            const moved = drag?.moved;
            drag = null;
            canvas.classList.remove("dragging");
            if (moved) return;
            const rect = canvas.getBoundingClientRect();
            if (this.onViewportClick?.(event.clientX - rect.left, event.clientY - rect.top)) return;
            this.view.pick(Math.floor((event.clientX - rect.left) * canvas.width / rect.width),
                Math.floor((rect.bottom - event.clientY) * canvas.height / rect.height), (result) => {
                    const renderable = result.renderable;
                    const index = typeof renderable === "number" ? renderable : renderable?.getId();
                    const id = this.pickable.get(index);
                    if (typeof renderable === "object") renderable?.delete();
                    if (id != null) this.onPick(id);
                });
            this.requestFrame(6);
        });
        canvas.addEventListener("wheel", (event) => {
            event.preventDefault();
            this.distance = Math.max(.2, Math.min(900, this.distance * Math.exp(event.deltaY * .001)));
            this.requestFrame();
        }, { passive: false });
    }
}

function checkWebGL(canvas) {
    const canvasProbe = document.createElement("canvas");
    if (!canvasProbe.getContext("webgl2")) throw new Error("WebGL 2 is unavailable in this browser.");
    canvas.addEventListener("webglcontextlost", () => {
        document.getElementById("canvas-message").hidden = false;
        document.getElementById("canvas-message").textContent = "Graphics context lost. Reload the page to restore the preview.";
    }, { once: true });
}
