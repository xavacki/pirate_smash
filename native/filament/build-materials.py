#!/usr/bin/env python3
"""Build the renderer's five material variants with the same matc as the Android SDK."""
import pathlib
import subprocess
import sys

here = pathlib.Path(__file__).resolve().parent
dest = here.parent.parent / "assets/filament"
dest.mkdir(parents=True, exist_ok=True)
for name, lit, transparent in [
    ("lit", True, False), ("glass", True, True), ("emissive", False, False),
    ("particle", False, True), ("hud", False, True)
]:
    is_hud = name == "hud"
    fragment = """prepareMaterial(m);
    vec2 uv = getUV0();
    uv.y = 1.0 - uv.y;
    vec4 c = getColor() * texture(materialParams_tex, uv);
    """ + ("c.rgb *= c.a; m.baseColor = c;" if is_hud else
    """vec4 base = materialParams.tint * c;
    m.baseColor = vec4(base.rgb * base.a, base.a);
    m.emissive = vec4(materialParams.emit * c.rgb * base.a, 0.0);
    """ + ("""m.metallic = materialParams.metal;
    m.roughness = materialParams.rough;
    m.clearCoat = materialParams.coat;
    m.clearCoatRoughness = 0.14;
    """ if lit else "m.baseColor.rgb = vec3(0.0);"))
    params = ["{ type: sampler2d, name: tex }"]
    if not is_hud:
        params += ["{ type: float4, name: tint }", "{ type: float3, name: emit }"]
    if lit:
        params += [f"{{ type: float, name: {p} }}" for p in ("metal", "rough", "coat")]
    material = f"""material {{
    name: Smash_{name},
    shadingModel: {"lit" if lit else "unlit"},
    requires: [uv0, color],
    parameters: [{", ".join(params)}],
    blending: {"transparent" if transparent else "opaque"},
    doubleSided: true,
    depthWrite: {"false" if transparent else "true"},
    depthCulling: {"false" if is_hud else "true"},
    culling: {"none" if is_hud else "back"}
}}
fragment {{ void material(inout MaterialInputs m) {{ {fragment} }} }}
"""
    source = dest / f"{name}.mat"
    source.write_text(material)
    subprocess.run([sys.argv[1], "-p", "mobile", "-a", "opengl", "-o",
                    dest / f"{name}.filamat", source], check=True)
    source.unlink()
