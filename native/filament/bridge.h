#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif
struct ANativeWindow;
struct SmashVertex { float position[3], normal[3], color[4], uv[2]; };
struct SmashHudVertex { float position[2], color[4], uv[2]; };
struct SmashDraw {
    float transform[16], color[4], emissive[3], metallic, roughness, coat;
    uint32_t mesh, flags;
};
void* smash_fila_create(const char* assets, int samples, char* error, size_t capacity);
void smash_fila_destroy(void*);
void smash_fila_window(void*, struct ANativeWindow* acquired);
uint32_t smash_fila_texture(void*, const uint8_t*, uint32_t width, uint32_t height, bool hud);
uint32_t smash_fila_mesh(void*, const struct SmashVertex*, uint32_t, const uint32_t*, uint32_t, uint32_t);
bool smash_fila_frame(void*, const struct SmashDraw*, uint32_t, const struct SmashHudVertex*, uint32_t,
    uint32_t width, uint32_t height, const float* eye, const float* forward, const float* up,
    float fovy, float viewWidth, float bloom, float threshold, float exposure);
#ifdef __cplusplus
}
#endif
