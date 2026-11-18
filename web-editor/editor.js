import { closestVariant, exportLevels, exportObj, fetchAsset, parseCatalog, parseObj, shapeName, tuple, validateLevels } from "./data.js";
import { Preview } from "./preview.js";

const $ = (id) => document.getElementById(id);
const el = (tag, text, attrs = {}) => {
    const node = document.createElement(tag);
    if (text != null) node.textContent = text;
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    return node;
};
const section = (title, nodes) => {
    const result = el("section", null, { class: "panel-section" });
    if (title) result.append(el("h3", title));
    result.append(...nodes);
    return result;
};
const basename = (file) => file.split("/").at(-1);
const status = (message) => { $("status").textContent = message; };
const message = (text) => {
    $("canvas-message").textContent = text;
    $("canvas-message").hidden = !text;
};
let levels, catalog, preview;
let mode = "level", stageIndex = 0, selected = null, levelDirty = false, filename = "levels.json";
let meshKey = null, meshInfo = null, vertex = 0;
let renderEpoch = 0, meshEpoch = 0;
const meshes = new Map();
const importedMeshes = new Map();
const stage = () => levels?.[stageIndex];
const meshChanges = new Set();

function openObj(key) {
    if (importedMeshes.has(key)) return Promise.resolve(importedMeshes.get(key).obj);
    if (!meshes.has(key)) {
        const pending = fetchAsset(key).then((response) => response.text()).then(parseObj)
            .catch((error) => { meshes.delete(key); throw error; });
        meshes.set(key, pending);
    }
    return meshes.get(key);
}

function numeric(label, value, change, { min, max, step = "any" } = {}) {
    const input = el("input", null, { type: "number", step, "aria-label": label });
    if (min !== undefined) input.min = min;
    if (max !== undefined) input.max = max;
    input.value = value;
    input.addEventListener("change", () => {
        const number = input.valueAsNumber;
        if (!input.validity.valid || !Number.isFinite(Math.fround(number)) || change(number) === false) {
            input.value = value;
            status(`Invalid value for ${label}.`);
        } else {
            value = number;
        }
    });
    const wrapped = el("label", label, { class: "field" });
    wrapped.append(input);
    return wrapped;
}

function vector(label, values, update, options) {
    const fields = el("div", null, { class: values.length === 4 ? "fields four" : "fields" });
    values.forEach((value, i) => fields.append(numeric("XYZW"[i], value, (next) => {
        const copy = values.slice();
        copy[i] = next;
        if (update(copy) === false) return false;
        values = copy;
    }, options)));
    return section(label, [fields]);
}

function selectControl(name, entries, value, change) {
    const select = el("select", null, { "aria-label": name });
    for (const [id, label] of entries) select.append(el("option", label, { value: id }));
    select.value = value;
    select.addEventListener("change", () => change(Number(select.value)));
    return select;
}

function updateDirty() {
    $("dirty").textContent = [levelDirty ? "Level changes" : "", meshChanges.size ? `${meshChanges.size} edited mesh${meshChanges.size === 1 ? "" : "es"}` : ""]
        .filter(Boolean).join(" · ");
}

function changed() {
    levelDirty = true;
    updateDirty();
}

function download(name, text, type) {
    const url = URL.createObjectURL(new Blob([text], { type }));
    const link = el("a", null, { href: url, download: name });
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function fillStages() {
    $("stage").replaceChildren(...levels.map((item, i) => el("option",
        `Level ${item.number}${item.stage_count > 1 ? ` · Stage ${item.stage + 1}` : ""}`, { value: i })));
}

function changeStage(index) {
    stageIndex = Math.max(0, Math.min(levels.length - 1, index));
    selected = { group: "objects", index: 0 };
    renderInterface(true);
}

function selectBody(group, index) {
    if (!stage()?.[group]?.[index]) return;
    selected = { group, index };
    renderInterface();
    $("object-list").querySelector('[aria-pressed="true"]')?.scrollIntoView({ block: "nearest" });
}

function listItem(label, detail, active, select) {
    const button = el("button", null, { type: "button", "aria-pressed": active });
    button.append(el("span", label), el("small", detail));
    button.addEventListener("click", select);
    return button;
}

function fillObjectList() {
    const list = $("object-list");
    const current = stage();
    $("stage").value = stageIndex;
    $("previous").disabled = stageIndex === 0;
    $("next").disabled = stageIndex === levels.length - 1;
    $("shots").value = current.shots;
    $("world-offset").value = current.world_y_offset;
    $("item-count").textContent = current.objects.length + current.platforms.length;
    const items = [];
    for (const [group, title] of [["platforms", "Platforms"], ["objects", "Targets"]]) {
        items.push(el("div", title, { class: "group-label" }));
        current[group].forEach((body, i) => items.push(listItem(group === "platforms" ? `Platform ${i + 1}` : shapeName(body.type),
            group === "objects" ? String(i + 1).padStart(2, "0") : "",
            selected?.group === group && selected.index === i, () => selectBody(group, i))));
    }
    list.replaceChildren(...items);
}

function editBody(field, value) {
    if (!selected) return false;
    const current = stage()[selected.group][selected.index];
    if (field === "rotation") {
        const length = Math.hypot(...value);
        if (length < 1e-10) return false;
        value = value.map((v) => v / length);
    }
    current[field] = value;
    changed();
    refreshPreview();
    return true;
}

function levelInspector() {
    const target = $("inspector-content");
    const current = stage();
    if (!selected || !current[selected.group]?.[selected.index]) {
        $("inspector-title").textContent = "Inspector";
        $("inspector-subtitle").textContent = "Select a target or platform.";
        target.replaceChildren();
        return;
    }
    const { group, index } = selected;
    const body = current[group][index];
    const isTarget = group === "objects";
    $("inspector-title").textContent = isTarget ? shapeName(body.type) : "Platform";
    $("inspector-subtitle").textContent = `${isTarget ? "Target" : "Platform"} ${index + 1}`;
    const controls = [];
    if (isTarget) {
        const kinds = [...new Set([...catalog.kinds.keys(), body.type])].sort((a, b) => a - b);
        const shape = selectControl("Shape", kinds.map((kind) => [kind, `${shapeName(kind)} (${kind})`]), body.type, (value) => {
            editBody("type", value);
            fillObjectList();
            levelInspector();
        });
        const support = selectControl("Supporting platform", current.platforms.map((_, i) => [i, `Platform ${i + 1}`]),
            body.support ?? 0, (value) => editBody("support", value));
        const label = el("label", "Support", { class: "field single" });
        label.append(support);
        const variant = closestVariant(catalog, body.type, body.half_extents);
        const meshButton = el("button", "Edit source mesh", { type: "button" });
        meshButton.disabled = !variant;
        meshButton.addEventListener("click", () => {
            switchMode("mesh", variant.parts[0].mesh);
        });
        controls.push(section("Shape", [shape, label, el("div", null, { class: "note" }), meshButton]));
    }
    controls.push(vector("Position", body.position, (value) => editBody("position", value)));
    controls.push(vector("Size", body.half_extents.map((v) => v * 2),
        (value) => editBody("half_extents", value.map((v) => v / 2)), { min: .001 }));
    controls.push(vector("Rotation · quaternion", body.rotation, (value) => {
        if (!editBody("rotation", value)) return false;
        levelInspector();
    }));
    if (!isTarget) {
        let motion = (body.motion ?? Array(12).fill(0)).slice();
        const details = el("details");
        details.append(el("summary", "Platform motion"));
        const list = el("div", null, { class: "motion-fields" });
        ["Rotate enabled", "Rotation speed", "Horizontal enabled", "Horizontal min", "Horizontal max",
            "Horizontal reverse", "Horizontal speed", "Vertical enabled", "Vertical min", "Vertical max",
            "Vertical reverse", "Vertical speed"].forEach((label, i) => {
            const flag = [0, 2, 5, 7, 10].includes(i);
            const field = numeric(label, motion[i], (value) => {
                const updated = motion.slice();
                updated[i] = value;
                if (editBody("motion", updated)) motion = updated;
            }, flag ? { min: 0, max: 1, step: 1 } : {});
            field.classList.add("single");
            list.append(field);
        });
        details.append(list);
        controls.push(section(null, [details]));
    }
    const clone = el("button", "Duplicate", { type: "button" });
    clone.addEventListener("click", () => addBody(group, true));
    const remove = el("button", "Delete", { type: "button", class: "danger" });
    remove.disabled = current[group].length < 2;
    remove.addEventListener("click", () => {
        current[group].splice(index, 1);
        if (!isTarget) {
            for (const obj of current.objects) {
                const support = obj.support ?? 0;
                obj.support = support === index ? Math.max(0, index - 1) : support > index ? support - 1 : support;
            }
        }
        changed();
        selectBody(group, Math.min(index, current[group].length - 1));
    });
    const actions = el("div", null, { class: "inline-buttons" });
    actions.append(clone, remove);
    controls.push(section(null, [actions]));
    target.replaceChildren(...controls);
}

function addBody(group, duplicate = false) {
    const current = stage();
    if (!current) return;
    const target = group === "objects";
    const base = duplicate && selected?.group === group ? current[group][selected.index] : null;
    const supportIndex = target && selected?.group === "platforms" ? selected.index : 0;
    const support = current.platforms[supportIndex];
    const body = base ? structuredClone(base) : {
        position: target ? [support.position[0], support.position[1] + support.half_extents[1] + .5, support.position[2]] : [0,.03,0],
        half_extents: target ? [.5,.5,.5] : [1.6,.3,1.15],
        rotation: [0,0,0,1],
        ...(target ? { type: 1, support: supportIndex } : {}),
    };
    if (base) body.position[0] += body.half_extents[0] * 2 + .15;
    if (!target && !base) body.position[0] = Math.max(...current.platforms.map((p) => p.position[0] + p.half_extents[0])) + 2;
    current[group].push(body);
    changed();
    selectBody(group, current[group].length - 1);
}

function fillShapes() {
    const filter = $("shape-filter").value.trim().toLowerCase();
    const assets = [...catalog.meshes, ...importedMeshes];
    $("shape-list").replaceChildren(...assets.filter(([key, info]) =>
        `${assetLabel(key, info)} ${basename(key)}`.toLowerCase().includes(filter))
        .map(([key, info]) => listItem(assetLabel(key, info), basename(info.file ?? key),
            key === meshKey, () => loadMesh(key))));
}

function assetLabel(key, info) {
    return info.kind === undefined ? basename(info.file ?? key) : `${shapeName(info.kind)} · ${info.size.join("×")}`;
}

async function loadMesh(key) {
    if (!catalog || !key) return;
    const epoch = ++meshEpoch;
    const info = catalog.meshes.get(key) ?? importedMeshes.get(key);
    try {
        const obj = await openObj(key);
        if (epoch !== meshEpoch || mode !== "mesh") return;
        meshKey = key;
        meshInfo = { ...info, obj };
        vertex = 0;
        renderInterface(true);
        $("shape-list").querySelector('[aria-pressed="true"]')?.scrollIntoView({ block: "nearest" });
    } catch (error) {
        status(error.message);
    }
}

function selectVertex(index) {
    vertex = Math.min(Math.max(0, index), meshInfo.obj.positions.length - 1);
    meshInspector();
    preview?.requestFrame();
}

function marker() {
    const dot = $("vertex-marker");
    const point = mode === "mesh" && meshInfo ? preview.project(meshInfo.obj.positions[vertex]) : null;
    dot.hidden = !point;
    if (!point) return;
    dot.style.left = `${point[0]}px`;
    dot.style.top = `${point[1]}px`;
}

function pickVertex(x, y) {
    if (mode !== "mesh" || !meshInfo) return false;
    let best, bestScore = 13;
    for (const [i, position] of meshInfo.obj.positions.entries()) {
        const point = preview.project(position);
        if (!point) continue;
        const score = Math.hypot(point[0] - x, point[1] - y) + point[2] * .01;
        if (score < bestScore) {
            bestScore = score;
            best = i;
        }
    }
    if (best !== undefined) selectVertex(best);
    return true;
}

function meshInspector() {
    const body = $("inspector-content");
    if (!meshInfo) {
        $("inspector-title").textContent = "Mesh inspector";
        $("inspector-subtitle").textContent = "Select an OBJ from the library.";
        body.replaceChildren();
        return;
    }
    const { obj } = meshInfo;
    $("inspector-title").textContent = assetLabel(meshKey, meshInfo);
    $("inspector-subtitle").textContent = basename(meshInfo.file ?? meshKey);
    const go = (index) => selectVertex(index);
    const picker = numeric("Vertex", vertex + 1, (value) => go(value - 1),
        { min: 1, max: obj.positions.length, step: 1 });
    picker.classList.add("single");
    const previous = el("button", "Previous", { type: "button" });
    const next = el("button", "Next", { type: "button" });
    previous.disabled = vertex === 0;
    next.disabled = vertex === obj.positions.length - 1;
    previous.addEventListener("click", () => go(vertex - 1));
    next.addEventListener("click", () => go(vertex + 1));
    const navigation = el("div", null, { class: "inline-buttons" });
    navigation.append(previous, next);
    const position = vector("Vertex position", obj.positions[vertex], (value) => {
        if (!tuple(value, 3)) return false;
        obj.positions[vertex] = value;
        obj.editedVertices ??= new Set();
        obj.editedVertices.add(vertex);
        obj.revision = (obj.revision ?? 0) + 1;
        meshChanges.add(meshKey);
        updateDirty();
        refreshPreview();
    });
    const reset = el("button", "Restore vertex", { type: "button" });
    reset.addEventListener("click", () => {
        obj.positions[vertex] = obj.lines[obj.vertexLines[vertex]].trim().split(/\s+/).slice(1,4).map(Number);
        obj.editedVertices?.delete(vertex);
        if (!obj.editedVertices?.size) meshChanges.delete(meshKey);
        obj.revision = (obj.revision ?? 0) + 1;
        updateDirty();
        meshInspector();
        refreshPreview();
    });
    body.replaceChildren(section(`${obj.positions.length} vertices · ${obj.faces.length} triangles`, [picker, navigation]),
        position, section(null, [reset]));
}

function surfaceFor(kind, texture = "") {
    if (texture.startsWith("assets/pirate/")) {
        if (texture.endsWith("brass.png")) return {metal:.76,rough:.46,coat:.05};
        if (texture.endsWith("iron.png")) return {metal:.72,rough:.4,coat:.1};
        if (/(blue|jade|purple)\.png$/.test(texture)) return {metal:.18,rough:.16,coat:.75};
        return {metal:0,rough:texture.endsWith("cloth.png")?.92:.68,coat:0};
    }
    return {
        metal: kind >= 1 && kind <= 3 ? .35 : 0,
        rough: (kind >= 11 && kind <= 16) || [61,62].includes(kind) ? .18 : .6,
    };
}

function displayBody(body, id, group, parts = []) {
    const [x,y,z] = body.position;
    const [qx,qy,qz,qw] = body.rotation;
    const position = [-x,y,z];
    const rotation = [qx,-qy,-qz,qw];
    const selection = selected?.group === group && selected.index === id;
    if (!parts.length) return [{
        position, rotation, scale: body.half_extents.map((v) => v * 2),
        color: group === "platforms" ? [225/255,213/255,185/255,1] : [.37,.65,.9,1],
        metal: .05, rough: .65, id: `${group}:${id}`, selected: selection,
    }];
    return parts.map(({ variant, part, obj }) => ({
        position, rotation, scale: body.half_extents.map((v, i) => v * 2 / variant.size[i]),
        color: part.color, texture: part.texture, obj,
        ...surfaceFor(body.type, part.texture),
        id: `${group}:${id}`, selected: selection,
    }));
}

function fitBounds(points) {
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (const [point, half] of points) {
        point.forEach((value, i) => {
            const extent = Array.isArray(half) ? half[i] : half;
            min[i] = Math.min(min[i], value - extent);
            max[i] = Math.max(max[i], value + extent);
        });
    }
    const center = min.map((v, i) => (v + max[i]) / 2);
    const half = min.map((v, i) => (max[i] - v) / 2);
    preview.fit(center, Math.max(.25, Math.hypot(...half)));
}

async function refreshPreview(fit = false) {
    if (!preview || !stage()) return;
    const epoch = ++renderEpoch;
    try {
        if (mode === "mesh") {
            if (!meshInfo) return;
            const { obj } = meshInfo;
            if (fit) fitBounds(obj.positions.map((point) => [point, 0]));
            await preview.display([
                { position: [0,0,0], rotation: [0,0,0,1], scale: [1,1,1], color: meshInfo.color ?? [.84,.86,.95,1],
                    texture: meshInfo.texture, ...surfaceFor(meshInfo.kind, meshInfo.texture), obj },
            ]);
        } else {
            const current = stage();
            if (fit) fitBounds([...current.platforms, ...current.objects].map((body) => [
                [-body.position[0], body.position[1], body.position[2]], body.half_extents.map((v) => v * 1.12),
            ]));
            const lowest = Math.min(...current.platforms.map((p) => p.position[1] - p.half_extents[1]));
            const floor = { position: [0, lowest - 1.8, 0], scale: [200,.6,200], rotation: [0,0,0,1], color: [30/255,42/255,59/255,1], metal: .05, rough: .65 };
            const dockParts = await Promise.all(["wood","iron","brass"].map(async (surface) => {
                const part = {mesh:`assets/pirate/dock-${surface}.obj`, texture:`assets/pirate/${surface}.png`, color:[1,1,1,1]};
                return {variant:{size:[1,1,1]}, part, obj:await openObj(part.mesh)};
            }));
            const platforms = current.platforms.flatMap((p, i) => displayBody(p, i, "platforms", dockParts));
            const targets = await Promise.all(current.objects.map(async (body, i) => {
                const variant = closestVariant(catalog, body.type, body.half_extents);
                const parts = variant ? await Promise.all(variant.parts.map(async (part) => ({
                    variant, part, obj: await openObj(part.mesh),
                }))) : [];
                return displayBody(body, i, "objects", parts);
            }));
            if (epoch !== renderEpoch) return;
            await preview.display([floor, ...platforms, ...targets.flat()]);
        }
        if (epoch === renderEpoch) {
            message("");
            $("preview").dataset.ready = "true";
        }
    } catch (error) {
        if (epoch === renderEpoch) message(error.message || String(error));
    }
}

function renderInterface(fit = false) {
    if (!levels || !catalog) return;
    const levelMode = mode === "level";
    $("level-tab").classList.toggle("active", levelMode);
    $("mesh-tab").classList.toggle("active", !levelMode);
    $("level-tab").setAttribute("aria-pressed", levelMode);
    $("mesh-tab").setAttribute("aria-pressed", !levelMode);
    $("level-outline").hidden = !levelMode;
    $("mesh-outline").hidden = levelMode;
    $("open").textContent = levelMode ? "Open JSON" : "Open OBJ";
    $("save").textContent = levelMode ? "Export JSON" : "Export OBJ";
    $("save").disabled = !levelMode && !meshInfo;
    $("view-title").textContent = levelMode ? `Level ${stage().number} · Stage ${stage().stage + 1}` : "Mesh preview";
    $("view-detail").textContent = levelMode ? `${stage().objects.length} target${stage().objects.length === 1 ? "" : "s"} · ${stage().platforms.length} platform${stage().platforms.length === 1 ? "" : "s"}` :
        meshInfo ? assetLabel(meshKey, meshInfo) : "";
    if (levelMode) $("vertex-marker").hidden = true;
    $("view-hint").textContent = levelMode ? "Drag to orbit · Scroll to zoom · Click to select" : "Drag to orbit · Scroll to zoom · Click a vertex";
    if (levelMode) {
        fillObjectList();
        levelInspector();
    } else {
        fillShapes();
        meshInspector();
    }
    refreshPreview(fit);
}

function switchMode(next, mesh = null) {
    if (!catalog) return;
    mode = next;
    renderInterface(true);
    if (mode === "mesh" && (mesh || !meshInfo)) loadMesh(mesh ?? catalog.meshes.keys().next().value);
}

$("previous").addEventListener("click", () => changeStage(stageIndex - 1));
$("next").addEventListener("click", () => changeStage(stageIndex + 1));
$("stage").addEventListener("change", (event) => changeStage(Number(event.target.value)));
for (const [id, field] of [["shots", "shots"], ["world-offset", "world_y_offset"]]) {
    $(id).addEventListener("change", (event) => {
        const current = stage();
        if (!current) return;
        const value = event.target.valueAsNumber;
        if (event.target.validity.valid && Number.isFinite(Math.fround(value))) {
            current[field] = value;
            changed();
        } else event.target.value = current[field];
    });
}
$("add-object").addEventListener("click", () => addBody("objects"));
$("add-platform").addEventListener("click", () => addBody("platforms"));
$("level-tab").addEventListener("click", () => switchMode("level"));
$("mesh-tab").addEventListener("click", () => switchMode("mesh"));
$("fit").addEventListener("click", () => refreshPreview(true));
$("shape-filter").addEventListener("input", fillShapes);
$("open").addEventListener("click", () => {
    if (!catalog) return;
    $("file").accept = mode === "level" ? ".json,application/json" : ".obj";
    $("file").click();
});
$("file").addEventListener("change", async () => {
    const [file] = $("file").files;
    $("file").value = "";
    if (!file) return;
    try {
        if (mode === "level") {
            const incoming = validateLevels(JSON.parse(await file.text()));
            if (levelDirty && !window.confirm("Replace the open levels and discard their unexported changes?")) return;
            levels = incoming;
            filename = file.name;
            levelDirty = false;
            updateDirty();
            fillStages();
            changeStage(0);
            status(`Opened ${file.name} · ${levels.length} stages`);
        } else {
            const obj = parseObj(await file.text());
            const key = `local:${file.name}`;
            if (meshChanges.has(key) && !window.confirm("Replace this mesh and discard its unexported changes?")) return;
            meshChanges.delete(key);
            updateDirty();
            importedMeshes.set(key, { obj, file: file.name });
            await loadMesh(key);
            status(`Opened ${file.name}`);
        }
    } catch (error) { status(error.message); }
});
$("save").addEventListener("click", () => {
    try {
        if (mode === "level" && levels) {
            download(filename, exportLevels(levels), "application/json");
            levelDirty = false;
            status(`Exported ${filename}`);
        } else if (mode === "mesh" && meshInfo) {
            const name = basename(meshInfo.file ?? meshKey);
            download(name, exportObj(meshInfo.obj), "text/plain");
            meshChanges.delete(meshKey);
            status(`Exported ${name}`);
        }
        updateDirty();
    } catch (error) { status(error.message); }
});
window.addEventListener("beforeunload", (event) => {
    if (!levelDirty && !meshChanges.size) return;
    event.preventDefault();
    event.returnValue = "";
});

async function boot() {
    try {
        const [data, manifest] = await Promise.all([
            fetchAsset("assets/levels.json").then((r) => r.json()),
            fetchAsset("assets/objects/manifest.txt").then((r) => r.text()),
        ]);
        levels = validateLevels(data);
        catalog = parseCatalog(manifest);
        fillStages();
        changeStage(0);
        status(`Loaded levels.json · ${levels.length} stages`);
    } catch (error) {
        status(error.message);
        message(error.message);
        return;
    }
    try {
        preview = await Preview.create($("preview"), (id) => {
            if (mode !== "level") return;
            const [group, index] = id.split(":");
            selectBody(group, Number(index));
        });
        preview.onFrame = marker;
        preview.onViewportClick = pickVertex;
        await refreshPreview(true);
    } catch (error) {
        console.error(error);
        message(error.message || String(error));
    }
}
boot();
