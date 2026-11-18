export const shapeName = (kind) => ({
    1: "Skull barrel", 2: "Coin sack", 3: "Treasure chest", 11: "Anchor bottle",
    12: "Amethyst bottle", 13: "Gold sack", 14: "Captain's chest", 15: "Rum bottle",
    16: "Jade bottle", 21: "Armored chest", 22: "Ballast barrel", 31: "Cargo crate",
    41: "Braced crate", 42: "Oak barrel", 51: "Powder keg", 61: "Cursed chest", 62: "Sea-glass bottle",
}[kind] ?? `Type ${kind}`);

export async function fetchAsset(path) {
    const response = await fetch(path.startsWith("/") ? path : `/${path}`);
    if (!response.ok) throw new Error(`Could not load ${path} (${response.status})`);
    return response;
}

export function parseCatalog(text) {
    const result = new Map();
    const meshes = new Map();
    for (const line of text.split(/\r?\n/)) {
        if (!line.trim() || line.trimStart().startsWith("#")) continue;
        const [type, x, y, z, mesh, texture, ...color] = line.trim().split(/\s+/);
        const kind = Number(type);
        const size = [x, y, z].map(Number);
        const key = size.join("×");
        const variants = result.get(kind) ?? new Map();
        const parts = variants.get(key)?.parts ?? [];
        const part = { mesh, texture, color: color.map(Number) };
        parts.push(part);
        variants.set(key, { size, parts });
        result.set(kind, variants);
        if (!meshes.has(mesh)) meshes.set(mesh, { ...part, kind, size });
    }
    return { kinds: result, meshes };
}

export function closestVariant(catalog, kind, half) {
    let best = null;
    let bestScore = Infinity;
    for (const variant of catalog.kinds.get(kind)?.values() ?? []) {
        const score = variant.size.reduce((sum, value, i) => sum + Math.abs(Math.log(2 * half[i] / value)), 0);
        if (score < bestScore) {
            best = variant;
            bestScore = score;
        }
    }
    return best;
}

const finite = (value) => typeof value === "number" && Number.isFinite(Math.fround(value));
export const tuple = (value, length) => Array.isArray(value) && value.length === length && value.every(finite);
const integer = (value, low, high = Number.MAX_SAFE_INTEGER) => Number.isSafeInteger(value) && value >= low && value <= high;

export function validateLevels(levels) {
    if (!Array.isArray(levels) || !levels.length) throw new Error("Expected a nonempty array of stages.");
    const seen = new Set();
    for (const stage of levels) {
        const key = `${stage?.number}:${stage?.stage}`;
        if (!integer(stage?.number, 1) || !integer(stage.stage, 0) ||
            !integer(stage.stage_count, stage.stage + 1) || !integer(stage.shots, 1, 2147483647) ||
            !finite(stage.world_y_offset) || seen.has(key) ||
            !Array.isArray(stage.platforms) || !stage.platforms.length ||
            !Array.isArray(stage.objects) || !stage.objects.length) {
            throw new Error(`Invalid or duplicate stage ${key}.`);
        }
        seen.add(key);
        for (const [group, bodies] of [["platforms", stage.platforms], ["objects", stage.objects]]) {
            bodies.forEach((body, i) => {
                if (!tuple(body?.position, 3) || !tuple(body.half_extents, 3) ||
                    body.half_extents.some((value) => Math.fround(value) <= 0) || !tuple(body.rotation, 4)) {
                    throw new Error(`Stage ${key}, ${group}[${i}] has an invalid pose.`);
                }
                if (group === "platforms" && body.motion !== undefined && !tuple(body.motion, 12)) {
                    throw new Error(`Stage ${key}, platform ${i + 1} needs 12 motion values.`);
                }
                if (group === "objects" && (!integer(body.type, 0, 255) ||
                    !integer(body.support === undefined ? 0 : body.support, 0, stage.platforms.length - 1))) {
                    throw new Error(`Stage ${key}, target ${i + 1} has an invalid type or supporting platform.`);
                }
            });
        }
    }
    return levels;
}

export function exportLevels(levels) {
    validateLevels(levels);
    return "[\n" + levels.map((stage) => {
        const lines = Object.entries(stage).map(([key, value]) => {
            if (key === "platforms" || key === "objects") {
                return `    "${key}": [\n${value.map((body) => "      " + JSON.stringify(body)).join(",\n")}\n    ]`;
            }
            return `    ${JSON.stringify(key)}: ${JSON.stringify(value)}`;
        });
        return "  {\n" + lines.join(",\n") + "\n  }";
    }).join(",\n") + "\n]\n";
}

const sub = (a, b) => a.map((v, i) => v - b[i]);
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (v) => {
    const length = Math.hypot(...v);
    return length > 1e-10 ? v.map((x) => x / length) : [0, 1, 0];
};

export function parseObj(text) {
    const lines = text.split(/\r?\n/);
    const positions = [], normals = [], uv = [], faces = [], vertexLines = [];
    for (const [lineIndex, line] of lines.entries()) {
        const [keyword, ...args] = line.split("#", 1)[0].trim().split(/\s+/);
        if (keyword === "v" || keyword === "vn" || keyword === "vt") {
            const length = keyword === "vt" ? 2 : 3;
            const values = args.slice(0, length).map(Number);
            if (!tuple(values, length)) throw new Error(`Invalid ${keyword} at OBJ line ${lineIndex + 1}.`);
            if (keyword === "v") {
                positions.push(values);
                vertexLines.push(lineIndex);
            } else if (keyword === "vt") {
                uv.push([values[0], 1 - values[1]]);
            } else {
                normals.push(values);
            }
        } else if (keyword === "f") {
            const points = args.map((token) => token.split("/").map((part, i) => {
                if (part === "") return -1;
                const index = Number(part);
                const count = [positions.length, uv.length, normals.length][i];
                return index < 0 ? count + index : index - 1;
            }));
            if (points.length < 3) throw new Error(`Invalid face at OBJ line ${lineIndex + 1}.`);
            for (let i = 1; i + 1 < points.length; i++) faces.push([points[0], points[i], points[i + 1]]);
        }
    }
    if (!positions.length || !faces.length || faces.some((face) => face.some(([v]) => !positions[v]))) {
        throw new Error("The OBJ needs vertices and valid faces.");
    }
    return { lines, positions, normals, uv, faces, vertexLines };
}

export function meshVertices(obj) {
    const result = [];
    for (const face of obj.faces) {
        const [a, b, c] = face.map(([position]) => obj.positions[position]);
        const normal = unit(cross(sub(b, a), sub(c, a)));
        for (const [v, uv, n] of face) {
            result.push({ position: obj.positions[v], normal: obj.normals[n] ?? normal, uv: obj.uv[uv] ?? [0, 0] });
        }
    }
    return result;
}

export function exportObj(obj) {
    const lines = obj.lines.slice();
    for (const i of obj.editedVertices ?? []) lines[obj.vertexLines[i]] = `v ${obj.positions[i].join(" ")}`;
    return lines.join("\n");
}

export function cubeVertices() {
    const points = [[-.5,-.5,-.5],[.5,-.5,-.5],[.5,.5,-.5],[-.5,.5,-.5],[-.5,-.5,.5],[.5,-.5,.5],[.5,.5,.5],[-.5,.5,.5]];
    const quads = [[0,3,2,1],[4,5,6,7],[0,4,7,3],[1,2,6,5],[3,7,6,2],[0,1,5,4]];
    return quads.flatMap(([a,b,c,d]) => {
        const normal = unit(cross(sub(points[b], points[a]), sub(points[c], points[a])));
        return [a,b,c,a,c,d].map((id) => ({ position: points[id], normal, uv: [0,0] }));
    });
}
