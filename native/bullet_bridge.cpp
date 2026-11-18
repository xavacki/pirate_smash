#include "bullet_bridge.h"
#include <btBulletDynamicsCommon.h>
#include <vector>
#include <memory>
#include <mutex>
struct BpWorld {
 btDefaultCollisionConfiguration config;
 btCollisionDispatcher dispatcher{&config};
 btDbvtBroadphase broadphase;
 btSequentialImpulseConstraintSolver solver;
 btDiscreteDynamicsWorld world{&dispatcher,&broadphase,&solver,&config};
 std::vector<btRigidBody*> bodies;
 std::vector<BpContact> contacts;
};
static btRigidBody* body(BpWorld* w,int32_t id) { return w && id>=0 && size_t(id)<w->bodies.size()?w->bodies[id]:nullptr; }
static bool contact_material(btManifoldPoint& point,const btCollisionObjectWrapper* a,int,int,const btCollisionObjectWrapper* b,int,int) {
 // Unity's recovered materials use Average, while Bullet defaults to multiplication.
 point.m_combinedFriction=(a->getCollisionObject()->getFriction()+b->getCollisionObject()->getFriction())*.5f;
 point.m_combinedRestitution=(a->getCollisionObject()->getRestitution()+b->getCollisionObject()->getRestitution())*.5f;
 return true;
}
extern "C" {
BpWorld* bp_world_new(float gravity) {
 auto* w=new BpWorld; w->world.setGravity(btVector3(0,gravity,0));
 w->world.getSolverInfo().m_numIterations=30;
 static std::once_flag material_callback_once;
 std::call_once(material_callback_once,[]{gContactAddedCallback=contact_material;});
 return w;
}
void bp_remove(BpWorld* w,int32_t id) { auto* b=body(w,id); if(!b)return; w->world.removeRigidBody(b); delete b->getMotionState(); delete b->getCollisionShape(); delete b; w->bodies[id]=nullptr; }
void bp_world_free(BpWorld* w) { if(!w)return; for(size_t i=0;i<w->bodies.size();++i)bp_remove(w,int32_t(i)); delete w; }
static int32_t add(BpWorld* w,btCollisionShape* shape,float x,float y,float z,float mass) {
 btTransform t; t.setIdentity(); t.setOrigin(btVector3(x,y,z)); btVector3 inertia(0,0,0); if(mass>0)shape->calculateLocalInertia(mass,inertia);
 auto* motion=new btDefaultMotionState(t); btRigidBody::btRigidBodyConstructionInfo info(mass,motion,shape,inertia); auto* b=new btRigidBody(info);
 int32_t id=int32_t(w->bodies.size()); b->setUserIndex(id); b->setFriction(0.6f); b->setRestitution(0); b->setDamping(0,0);
 b->setCollisionFlags(b->getCollisionFlags() | btCollisionObject::CF_CUSTOM_MATERIAL_CALLBACK);
 if(mass>0){b->setCcdMotionThreshold(0.25f); b->setCcdSweptSphereRadius(0.1f);}
 w->bodies.push_back(b); w->world.addRigidBody(b); return id;
}
int32_t bp_box(BpWorld* w,float x,float y,float z,float hx,float hy,float hz,float mass) { return add(w,new btBoxShape(btVector3(hx,hy,hz)),x,y,z,mass); }
int32_t bp_sphere(BpWorld* w,float x,float y,float z,float radius,float mass) { return add(w,new btSphereShape(radius),x,y,z,mass); }
int32_t bp_cylinder(BpWorld* w,float x,float y,float z,float hx,float hy,float hz,float mass) { return add(w,new btCylinderShape(btVector3(hx,hy,hz)),x,y,z,mass); }
int32_t bp_convex(BpWorld* w,float x,float y,float z,const float* xyz,int32_t count,float mass) {
 if(!xyz||count<4)return -1;
 auto* shape=new btConvexHullShape();
 for(int32_t i=0;i<count;++i)shape->addPoint(btVector3(xyz[i*3],xyz[i*3+1],xyz[i*3+2]),false);
 shape->setMargin(0.001f);shape->recalcLocalAabb();shape->optimizeConvexHull();
 return add(w,shape,x,y,z,mass);
}
void bp_material(BpWorld* w,int32_t id,float friction,float restitution,float linear_drag,float angular_drag) {
 auto* b=body(w,id);if(!b)return;
 b->setFriction(friction);b->setRestitution(restitution);b->setDamping(linear_drag,angular_drag);
}
void bp_prepare_stack(BpWorld* w) {
 // Authored puzzle stacks represent an equilibrium, not a pile being dropped at load.
 // Generate contact manifolds first so impacts and kinematic supports can wake islands.
 w->world.performDiscreteCollisionDetection();
 for(auto* b:w->bodies)if(b&&!b->isStaticOrKinematicObject()) {
  b->setLinearVelocity(btVector3(0,0,0));b->setAngularVelocity(btVector3(0,0,0));
  b->clearForces();b->setActivationState(ISLAND_SLEEPING);
 }
}
void bp_sync_kinematics(BpWorld* w) {
 for(auto* b:w->bodies)if(b&&b->isKinematicObject()) {
  btTransform t;b->getMotionState()->getWorldTransform(t);
  b->setWorldTransform(t);b->setInterpolationWorldTransform(t);
  b->setLinearVelocity(btVector3(0,0,0));b->setAngularVelocity(btVector3(0,0,0));
  w->world.updateSingleAabb(b);
 }
}
void bp_set_kinematic(BpWorld* w,int32_t id) {
 auto* b=body(w,id);if(!b)return;
 w->world.removeRigidBody(b);b->setMassProps(0,btVector3(0,0,0));
 b->setCollisionFlags((b->getCollisionFlags() & ~btCollisionObject::CF_STATIC_OBJECT) | btCollisionObject::CF_KINEMATIC_OBJECT);
 b->setActivationState(DISABLE_DEACTIVATION);w->world.addRigidBody(b);
}
void bp_set_pose(BpWorld* w,int32_t id,float x,float y,float z,float qx,float qy,float qz,float qw) {
 auto* b=body(w,id);if(!b)return;btQuaternion q(qx,qy,qz,qw);
 if(q.length2()<SIMD_EPSILON)q=btQuaternion::getIdentity();else q.normalize();
 btTransform t(q,btVector3(x,y,z));
 if(b->isKinematicObject()) {
  // Bullet samples the motion state each step and derives platform velocity
  // from its previous world transform, so bodies receive platform motion.
  b->getMotionState()->setWorldTransform(t);
 } else {
  b->setWorldTransform(t);b->setInterpolationWorldTransform(t);b->getMotionState()->setWorldTransform(t);
  w->world.updateSingleAabb(b);
 }
 b->activate(true);
}
void bp_step(BpWorld* w,float seconds) {
 w->world.stepSimulation(seconds,8,1.f/120.f); w->contacts.clear();
 for(int i=0;i<w->dispatcher.getNumManifolds();++i){ auto* m=w->dispatcher.getManifoldByIndexInternal(i); float impulse=0; bool hit=false;
 for(int j=0;j<m->getNumContacts();++j) if(m->getContactPoint(j).getDistance()<=0){hit=true;impulse+=m->getContactPoint(j).getAppliedImpulse();}
 if(hit)w->contacts.push_back({m->getBody0()->getUserIndex(),m->getBody1()->getUserIndex(),impulse}); }
}
int32_t bp_transform(BpWorld* w,int32_t id,BpTransform* out) { auto* b=body(w,id);if(!b||!out)return 0;auto& t=b->getWorldTransform(); auto p=t.getOrigin();auto q=t.getRotation();*out={p.x(),p.y(),p.z(),q.x(),q.y(),q.z(),q.w()};return 1; }
void bp_set_velocity(BpWorld* w,int32_t id,float x,float y,float z){auto* b=body(w,id);if(b){b->activate(true);b->setLinearVelocity(btVector3(x,y,z));}}
void bp_impulse(BpWorld* w,int32_t id,float x,float y,float z){auto* b=body(w,id);if(b){b->activate(true);b->applyCentralImpulse(btVector3(x,y,z));}}
void bp_reset(BpWorld* w,int32_t id,float x,float y,float z){auto* b=body(w,id);if(!b)return;btTransform t;t.setIdentity();t.setOrigin(btVector3(x,y,z));b->setWorldTransform(t);b->getMotionState()->setWorldTransform(t);b->setLinearVelocity(btVector3(0,0,0));b->setAngularVelocity(btVector3(0,0,0));b->clearForces();b->activate(true);w->world.updateSingleAabb(b);}
int32_t bp_contact_count(BpWorld* w){return int32_t(w->contacts.size());}
int32_t bp_contact(BpWorld* w,int32_t i,BpContact* out){if(i<0||size_t(i)>=w->contacts.size()||!out)return 0;*out=w->contacts[i];return 1;}
}
