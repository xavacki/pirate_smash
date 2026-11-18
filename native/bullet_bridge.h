#ifndef SMASH_BULLET_BRIDGE_H
#define SMASH_BULLET_BRIDGE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct BpWorld BpWorld;
typedef struct { float x,y,z,qx,qy,qz,qw; } BpTransform;
typedef struct { int32_t a,b; float impulse; } BpContact;
BpWorld* bp_world_new(float gravity);
void bp_world_free(BpWorld*);
int32_t bp_box(BpWorld*, float x,float y,float z,float hx,float hy,float hz,float mass);
int32_t bp_sphere(BpWorld*,float x,float y,float z,float radius,float mass);
int32_t bp_cylinder(BpWorld*,float x,float y,float z,float hx,float hy,float hz,float mass);
int32_t bp_convex(BpWorld*,float x,float y,float z,const float* xyz,int32_t count,float mass);
void bp_material(BpWorld*,int32_t id,float friction,float restitution,float linear_drag,float angular_drag);
void bp_prepare_stack(BpWorld*);
void bp_sync_kinematics(BpWorld*);
void bp_set_pose(BpWorld*,int32_t id,float x,float y,float z,float qx,float qy,float qz,float qw);
void bp_set_kinematic(BpWorld*,int32_t id);
void bp_step(BpWorld*,float seconds);
int32_t bp_transform(BpWorld*,int32_t id,BpTransform* out);
void bp_set_velocity(BpWorld*,int32_t id,float x,float y,float z);
void bp_impulse(BpWorld*,int32_t id,float x,float y,float z);
void bp_reset(BpWorld*,int32_t id,float x,float y,float z);
void bp_remove(BpWorld*,int32_t id);
int32_t bp_contact_count(BpWorld*);
int32_t bp_contact(BpWorld*,int32_t index,BpContact* out);
#ifdef __cplusplus
}
#endif
#endif
