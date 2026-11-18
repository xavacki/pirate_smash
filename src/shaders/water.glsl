// World-space waves perturb both the surface normal and its planar reflection.
// Sample where the reflected ray hits the same harbor plate drawn behind the cargo.
vec2 harborUV(vec3 ray, vec3 forward, vec3 right, vec3 up, vec2 halfFov) {
    float depth = max(dot(ray, forward), 0.01);
    return vec2(0.5) + vec2(dot(ray, right), -dot(ray, up)) / (2.0 * depth * halfFov);
}

vec3 harborWater(vec3 point, vec3 eye, vec3 forward, vec3 up, vec2 halfFov,
                 float time, sampler2D harbor, sampler2D mask) {
    vec3 right = normalize(cross(forward, up));
    vec3 ray = point - eye;
    vec2 uv = harborUV(ray, forward, right, up, halfFov);
    float coverage = texture(mask, uv).a * (1.0 - smoothstep(270.0, 475.0, ray.z));
    if (coverage < 0.005) discard;

    vec2 p = point.xz;
    float depth = length(ray);
    // Separate wavelengths and drift directions keep the ripples from moving as a single pattern.
    vec2 wave = vec2(0.19, 0.74) * sin(dot(p, vec2(0.19, 0.74)) - 2.1 * time);
    wave += vec2(-0.68, 0.37) * sin(dot(p, vec2(-0.68, 0.37)) + 1.64 * time + 1.2);
    wave += vec2(0.45, 0.56) * sin(dot(p, vec2(1.57, 1.96)) - 3.25 * time + wave.x);
    wave += vec2(-0.27, 0.32) * sin(dot(p, vec2(-3.10, 3.66)) + 4.31 * time)
        * (1.0 - smoothstep(65.0, 175.0, depth));

    vec3 planeNormal = normalize(vec3(0.0, 1.0, -0.05));
    vec3 normal = normalize(planeNormal + vec3(wave.x, 0.0, wave.y) * 0.115);
    vec3 view = -ray / depth;
    vec3 reflected = reflect(-view, normal);
    vec3 plateHit = ray + reflected * ((500.0 - dot(ray, forward)) /
        max(dot(reflected, forward), 0.10));
    vec2 reflectionUV = harborUV(plateHit, forward, right, up, halfFov);
    reflectionUV = clamp(reflectionUV, vec2(0.002, 0.025), vec2(0.998, 0.325));

    vec3 original = texture(harbor, uv).rgb;
    vec2 distortion = wave * vec2(0.0070, 0.0038) * (1.0 - smoothstep(135.0, 230.0, depth));
    vec2 displaced = uv + distortion;
    // Keep ripple distortions inside the water, away from boats, dock and shore.
    float displacedCoverage = texture(mask, displaced).a;
    vec3 water = texture(harbor, mix(uv, displaced, displacedCoverage)).rgb;
    vec3 reflection = texture(harbor, reflectionUV).rgb;
    float fresnel = 0.035 + 0.54 * pow(1.0 - max(dot(normal, view), 0.0), 5.0);
    float sunOffset = (uv.x - 0.688 + wave.x * 0.025) * 11.0;
    float sunLane = exp(-sunOffset * sunOffset);
    float glint = pow(max(0.0, sin(dot(p, vec2(0.31, 3.9)) - time * 4.3 + wave.y * 1.9)), 9.0);
    vec3 surface = mix(water * (0.91 + 0.19 * wave.y), reflection, fresnel * 0.32)
        + vec3(0.27, 0.107, 0.021) * sunLane * glint;
    return mix(original, surface, coverage);
}
