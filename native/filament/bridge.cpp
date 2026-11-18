#include "bridge.h"
#include <android/native_window.h>
#include <android/log.h>
#include <jni.h>
#include <filament/Camera.h>
#include <filament/Engine.h>
#include <filament/IndexBuffer.h>
#include <filament/IndirectLight.h>
#include <filament/LightManager.h>
#include <filament/Material.h>
#include <filament/MaterialInstance.h>
#include <filament/RenderableManager.h>
#include <filament/Renderer.h>
#include <filament/Scene.h>
#include <filament/Skybox.h>
#include <filament/SwapChain.h>
#include <filament/Texture.h>
#include <filament/TextureSampler.h>
#include <filament/TransformManager.h>
#include <filament/VertexBuffer.h>
#include <filament/View.h>
#include <filament/Viewport.h>
#include <ktxreader/Ktx1Reader.h>
#include <math/mat3.h>
#include <math/mat4.h>
#include <private/backend/VirtualMachineEnv.h>
#include <utils/EntityManager.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

using namespace filament;
using namespace filament::math;
namespace {
constexpr uint32_t Transparent=1, Unlit=2, DoubleSided=4, Textured=8, Backdrop=16, Water=32;
using Entity = utils::Entity;
using Rm = RenderableManager;
float3 xyz(const float* a) { return {a[0],a[1],a[2]}; }
float4 rgba(const float* a) { return {a[0],a[1],a[2],a[3]}; }

std::vector<uint8_t> read(const std::string& path) {
    std::ifstream in(path,std::ios::binary);
    if (!in) throw std::runtime_error("Missing Filament asset: "+path);
    return {std::istreambuf_iterator<char>(in),std::istreambuf_iterator<char>()};
}

template<typename T>
backend::BufferDescriptor upload(std::vector<T> data) {
    auto* copy = new std::vector<T>(std::move(data));
    return {copy->data(),copy->size()*sizeof(T),
        [](void*,size_t,void* u) { delete static_cast<std::vector<T>*>(u); },copy};
}

struct GpuVertex { float3 pos; float4 frame, color; float2 uv; };
struct Mesh { VertexBuffer* vb; IndexBuffer* ib; Box bounds; uint32_t count, tex; };
struct Slot {
    Entity id;
    std::array<MaterialInstance*,5> material{};
    uint32_t mesh = UINT32_MAX;
    bool visible = false;
};

class GameRenderer {
    Engine* engine = nullptr;
    Renderer* renderer = nullptr;
    Scene *scene=nullptr,*overlay=nullptr;
    View *view=nullptr,*ui=nullptr;
    Camera *camera=nullptr,*uiCamera=nullptr;
    Entity cameraId,uiCameraId,key,fill,hudId;
    Skybox* sky = nullptr;
    IndirectLight* studio = nullptr;
    Texture* reflections = nullptr;
    SwapChain* chain = nullptr;
    ANativeWindow* window = nullptr;
    std::array<Material*,6> programs{};
    std::vector<Texture*> textures;
    std::vector<Mesh> meshes;
    std::vector<Slot> slots;
    VertexBuffer* hudV = nullptr;
    IndexBuffer* hudI = nullptr;
    uint32_t hudCapacity = 0;
    bool hudVisible = false;
    bool first = true;
    TextureSampler surfaceSampler{TextureSampler::MinFilter::LINEAR_MIPMAP_LINEAR,
        TextureSampler::MagFilter::LINEAR,TextureSampler::WrapMode::REPEAT};
    TextureSampler flatSampler{TextureSampler::MinFilter::LINEAR,TextureSampler::MagFilter::LINEAR};

public:
    void init(const char* assets, int samples) {
        engine=Engine::create(Engine::Backend::OPENGL);
        if (!engine) throw std::runtime_error("Filament could not initialize GLES");
        renderer=engine->createRenderer();
        renderer->setClearOptions({.clearColor={.018f,.027f,.046f,1},.clear=true});
        scene=engine->createScene(); overlay=engine->createScene();
        view=engine->createView(); ui=engine->createView();
        auto& em=utils::EntityManager::get();
        cameraId=em.create(); uiCameraId=em.create(); key=em.create(); fill=em.create(); hudId=em.create();
        camera=engine->createCamera(cameraId); uiCamera=engine->createCamera(uiCameraId);
        view->setCamera(camera); view->setScene(scene);
        view->setAntiAliasing(AntiAliasing::FXAA);
        if (samples>1) view->setMultiSampleAntiAliasingOptions({.enabled=true,.sampleCount=uint8_t(std::min(samples,4))});
        ui->setCamera(uiCamera); ui->setScene(overlay);
        ui->setPostProcessingEnabled(false);
        ui->setBlendMode(View::BlendMode::TRANSLUCENT);
        ui->setShadowingEnabled(false);
        sky=Skybox::Builder().color({.027f,.045f,.08f,1}).build(*engine);
        scene->setSkybox(sky);
        const char* variants[]={"lit","glass","emissive","particle","hud","water"};
        for (size_t i=0;i<programs.size();++i) {
            auto bytes=read(std::string(assets)+"/"+variants[i]+".filamat");
            programs[i]=Material::Builder().package(bytes.data(),bytes.size()).build(*engine);
            if (!programs[i]) throw std::runtime_error("Incompatible Filament material");
        }
        uint8_t white[]{255,255,255,255};
        texture(white,1,1,false);
        auto environment=read(std::string(assets)+"/studio.ktx");
        auto bundle=std::make_unique<image::Ktx1Bundle>(environment.data(),uint32_t(environment.size()));
        float3 sh[9]{};
        if (!bundle->getSphericalHarmonics(sh)) throw std::runtime_error("Studio light is missing SH coefficients");
        reflections=ktxreader::Ktx1Reader::createTexture(engine,bundle.release(),false);
        studio=IndirectLight::Builder().reflections(reflections).irradiance(3,sh).intensity(15000).build(*engine);
        scene->setIndirectLight(studio);
        LightManager::ShadowOptions shadow;
        shadow.mapSize=1024; shadow.shadowFar=85.f;
        LightManager::Builder(LightManager::Type::SUN).direction({.5f,-1.f,.45f})
            .color({1,.76f,.48f}).intensity(52000).castShadows(true).shadowOptions(shadow).build(*engine,key);
        LightManager::Builder(LightManager::Type::DIRECTIONAL).direction({-.65f,-.4f,-.55f})
            .color({.52f,.73f,1.f}).intensity(21000).build(*engine,fill);
        scene->addEntity(key); scene->addEntity(fill);
    }

    ~GameRenderer() {
        if (!engine) return;
        attach(nullptr);
        engine->flushAndWait();
        engine->destroy(view); engine->destroy(ui); engine->destroy(scene); engine->destroy(overlay);
        auto& em=utils::EntityManager::get();
        for (auto& s : slots) {
            engine->destroy(s.id); em.destroy(s.id);
            for (auto* m : s.material) if(m) engine->destroy(m);
        }
        for (auto e : {key,fill,cameraId,uiCameraId,hudId}) if(e) { engine->destroy(e); em.destroy(e); }
        for (auto& m : meshes) { engine->destroy(m.vb); engine->destroy(m.ib); }
        if (hudV) engine->destroy(hudV);
        if (hudI) engine->destroy(hudI);
        for(auto* m : programs) if(m) engine->destroy(m);
        for(auto* t : textures) engine->destroy(t);
        if(sky) engine->destroy(sky);
        if(studio) engine->destroy(studio);
        if(reflections) engine->destroy(reflections);
        if(renderer) engine->destroy(renderer);
        Engine::destroy(&engine);
    }

    void attach(ANativeWindow* acquired) {
        if (window==acquired) {
            if (acquired) ANativeWindow_release(acquired);
            return;
        }
        if (chain) { engine->destroy(chain); engine->flushAndWait(); chain=nullptr; }
        if (window) ANativeWindow_release(window);
        window=acquired;
        if (window) chain=engine->createSwapChain(window);
    }

    uint32_t texture(const uint8_t* data,uint32_t w,uint32_t h,bool hud) {
        const uint32_t levels=hud ? 1 : 1+uint32_t(std::log2(std::max(w,h)));
        auto usage=Texture::Usage::DEFAULT;
        if (levels>1) usage=usage|Texture::Usage::GEN_MIPMAPPABLE;
        auto* tex=Texture::Builder().width(w).height(h).levels(uint8_t(levels)).usage(usage)
            .format(hud ? Texture::InternalFormat::RGBA8 : Texture::InternalFormat::SRGB8_A8).build(*engine);
        auto* copy=new std::vector<uint8_t>(data,data+size_t(w)*h*4);
        tex->setImage(*engine,0,Texture::PixelBufferDescriptor(copy->data(),copy->size(),
            Texture::Format::RGBA,Texture::Type::UBYTE,[](void*,size_t,void* p) {
                delete static_cast<std::vector<uint8_t>*>(p);
            },copy));
        if (levels>1) tex->generateMipmaps(*engine);
        textures.push_back(tex);
        if(hud) programs[4]->setDefaultParameter("tex",tex,flatSampler);
        return uint32_t(textures.size()-1);
    }

    uint32_t mesh(const SmashVertex* source,uint32_t vertices,const uint32_t* indices,uint32_t count,uint32_t tex) {
        std::vector<GpuVertex> v; v.reserve(vertices);
        float3 minimum{1e20f}, maximum{-1e20f};
        for (uint32_t i=0;i<vertices;i++) {
            const auto& s=source[i];
            auto normal=xyz(s.normal);
            normal=dot(normal,normal)>1e-9f ? normalize(normal) : float3{0,1,0};
            auto t=normalize(cross(std::abs(normal.y)>.9f ? float3{1,0,0} : float3{0,1,0},normal));
            auto q=mat3f::packTangentFrame(mat3f{t,cross(normal,t),normal});
            auto pos=xyz(s.position);
            minimum=min(minimum,pos); maximum=max(maximum,pos);
            v.push_back({pos,{q.x,q.y,q.z,q.w},rgba(s.color),{s.uv[0],s.uv[1]}});
        }
        auto* vb=VertexBuffer::Builder().vertexCount(vertices).bufferCount(1)
            .attribute(VertexAttribute::POSITION,0,VertexBuffer::AttributeType::FLOAT3,offsetof(GpuVertex,pos),sizeof(GpuVertex))
            .attribute(VertexAttribute::TANGENTS,0,VertexBuffer::AttributeType::FLOAT4,offsetof(GpuVertex,frame),sizeof(GpuVertex))
            .attribute(VertexAttribute::COLOR,0,VertexBuffer::AttributeType::FLOAT4,offsetof(GpuVertex,color),sizeof(GpuVertex))
            .attribute(VertexAttribute::UV0,0,VertexBuffer::AttributeType::FLOAT2,offsetof(GpuVertex,uv),sizeof(GpuVertex)).build(*engine);
        auto* ib=IndexBuffer::Builder().indexCount(count).bufferType(IndexBuffer::IndexType::UINT).build(*engine);
        vb->setBufferAt(*engine,0,upload(std::move(v)));
        ib->setBuffer(*engine,upload(std::vector<uint32_t>(indices,indices+count)));
        meshes.push_back({vb,ib,{(minimum+maximum)*.5f,max((maximum-minimum)*.5f,float3{.001f})},count,tex});
        return uint32_t(meshes.size()-1);
    }

    bool frame(const SmashDraw* draws,uint32_t drawCount,const SmashHudVertex* hud,uint32_t vertexCount,
            uint32_t w,uint32_t h,const float* eye,const float* forward,const float* up,
            float fovy,float viewWidth,float bloom,float threshold,float exposure) {
        if (!chain || !w || !h) return false;
        if (!renderer->beginFrame(chain)) return false;
        view->setViewport({0,0,w,h}); ui->setViewport({0,0,w,h});
        camera->setProjection(fovy,viewWidth/800.,.3,1000.);
        uiCamera->setProjection(Camera::Projection::ORTHO,0,viewWidth,800,0,0,1);
        camera->lookAt(xyz(eye),xyz(eye)+xyz(forward),xyz(up));
        camera->setExposure(16.f,1.f/125,std::clamp(100*exposure,10.f,1000.f));
        BloomOptions opts;
        opts.enabled=bloom>0;
        opts.quality=QualityLevel::MEDIUM;
        opts.highlight=24;
        opts.strength=std::clamp(bloom,0.f,1.f);
        opts.threshold=threshold>0;
        view->setBloomOptions(opts);
        if (slots.size()<drawCount) slots.resize(drawCount);
        Texture* harbor = textures[0];
        for (uint32_t i=0; i<drawCount; ++i) {
            if (draws[i].flags & Backdrop) harbor = textures[meshes[draws[i].mesh].tex];
        }
        const float halfFovY = std::tan(fovy * 0.00872664626f);
        const float2 halfFov{halfFovY * viewWidth / 800.f, halfFovY};
        for (uint32_t i=0; i<drawCount; ++i) {
            draw(slots[i], draws[i]);
            if (draws[i].flags & Water) {
                auto* water = slots[i].material[4];
                water->setParameter("harbor", harbor, flatSampler);
                water->setParameter("mask", textures[meshes[draws[i].mesh].tex], flatSampler);
                water->setParameter("eye", xyz(eye));
                water->setParameter("forward", xyz(forward));
                water->setParameter("up", xyz(up));
                water->setParameter("halfFov", halfFov);
            }
        }
        for(size_t i=drawCount;i<slots.size();++i) {
            auto& s=slots[i];
            if(s.visible) { scene->remove(s.id); s.visible=false; }
        }
        uploadHud(hud,vertexCount);
        renderer->render(view);
        if (vertexCount) renderer->render(ui);
        renderer->endFrame();
        if (first) {
            first=false;
            __android_log_print(ANDROID_LOG_INFO,"SmashFilament","Presented first Filament frame");
        }
        return true;
    }

private:
    void draw(Slot& s,const SmashDraw& d) {
        const auto& mesh=meshes[d.mesh];
        const bool transparent=d.flags&Transparent, unlit=d.flags&Unlit;
        const bool water = d.flags & Water;
        const int variant = water ? 4 : (unlit?2:0)+(transparent?1:0);
        auto*& mat=s.material[variant];
        if(!mat) mat=programs[water ? 5 : variant]->createInstance();
        if (!water) {
            mat->setParameter("tint",RgbaType::sRGB,rgba(d.color));
            // Keep alpha-blended fire and smoke in display range, even when many puffs overlap.
            float emissionScale=12000.f;
            if (d.flags&Backdrop) emissionScale=1.f;
            else if (unlit && transparent) emissionScale=2.2f;
            mat->setParameter("emit",xyz(d.emissive)*emissionScale);
            mat->setParameter("tex",textures[d.flags&Textured ? mesh.tex : 0],surfaceSampler);
            mat->setCullingMode(d.flags&DoubleSided ? backend::CullingMode::NONE : backend::CullingMode::BACK);
            if(!unlit) {
                mat->setParameter("metal",d.metallic);
                mat->setParameter("rough",std::max(.045f,d.roughness));
                mat->setParameter("coat",d.coat);
            }
        }
        auto& rm=engine->getRenderableManager();
        if(!s.id) {
            s.id=utils::EntityManager::get().create();
            Rm::Builder(1).boundingBox(mesh.bounds).geometry(0,Rm::PrimitiveType::TRIANGLES,mesh.vb,mesh.ib,0,mesh.count)
                .material(0,mat).receiveShadows(true).build(*engine,s.id);
            s.mesh=d.mesh;
        }
        if(!s.visible) { scene->addEntity(s.id); s.visible=true; }
        auto instance=rm.getInstance(s.id);
        if(s.mesh!=d.mesh) {
            rm.setGeometryAt(instance,0,Rm::PrimitiveType::TRIANGLES,mesh.vb,mesh.ib,0,mesh.count);
            rm.setAxisAlignedBoundingBox(instance,mesh.bounds);
            s.mesh=d.mesh;
        }
        rm.setMaterialInstanceAt(instance,0,mat);
        rm.setCastShadows(instance,!water&&!transparent&&!unlit&&std::abs(d.transform[0])<50);
        auto& tm=engine->getTransformManager();
        tm.setTransform(tm.getInstance(s.id), mat4f{rgba(d.transform),rgba(d.transform+4),rgba(d.transform+8),rgba(d.transform+12)});
    }

    void uploadHud(const SmashHudVertex* hud,uint32_t n) {
        auto& rm=engine->getRenderableManager();
        if (!n) {
            if (hudVisible) { overlay->remove(hudId); hudVisible=false; }
            return;
        }
        if (hudCapacity<n) {
            rm.destroy(hudId);
            if (hudV) engine->destroy(hudV);
            if (hudI) engine->destroy(hudI);
            hudCapacity=std::max(n,hudCapacity*2);
            hudV=VertexBuffer::Builder().vertexCount(hudCapacity).bufferCount(1)
                .attribute(VertexAttribute::POSITION,0,VertexBuffer::AttributeType::FLOAT2,offsetof(SmashHudVertex,position),sizeof(SmashHudVertex))
                .attribute(VertexAttribute::COLOR,0,VertexBuffer::AttributeType::FLOAT4,offsetof(SmashHudVertex,color),sizeof(SmashHudVertex))
                .attribute(VertexAttribute::UV0,0,VertexBuffer::AttributeType::FLOAT2,offsetof(SmashHudVertex,uv),sizeof(SmashHudVertex)).build(*engine);
            hudI=IndexBuffer::Builder().indexCount(hudCapacity).bufferType(IndexBuffer::IndexType::UINT).build(*engine);
            std::vector<uint32_t> idx(hudCapacity);
            for(uint32_t i=0;i<hudCapacity;++i) idx[i]=i;
            hudI->setBuffer(*engine,upload(std::move(idx)));
            Rm::Builder(1).boundingBox({{550,400,0},{550,400,1}}).culling(false)
                .geometry(0,Rm::PrimitiveType::TRIANGLES,hudV,hudI,0,n).material(0,programs[4]->getDefaultInstance())
                .castShadows(false).receiveShadows(false).build(*engine,hudId);
            auto& tm=engine->getTransformManager();
            tm.setTransform(tm.getInstance(hudId),mat4f::translation(float3{0,0,-.5f}));
        } else rm.setGeometryAt(rm.getInstance(hudId),0,Rm::PrimitiveType::TRIANGLES,hudV,hudI,0,n);
        hudV->setBufferAt(*engine,0,upload(std::vector<SmashHudVertex>(hud,hud+n)));
        if(!hudVisible) { overlay->addEntity(hudId); hudVisible=true; }
    }
};
}

extern "C" JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM* vm,void*) {
    return filament::VirtualMachineEnv::JNI_OnLoad(vm);
}
extern "C" void* smash_fila_create(const char* path,int samples,char* error,size_t size) {
    try { auto renderer=std::make_unique<GameRenderer>(); renderer->init(path,samples); return renderer.release(); }
    catch(const std::exception& e) { std::snprintf(error,size,"%s",e.what()); return nullptr; }
}
extern "C" void smash_fila_destroy(void* r) { delete static_cast<GameRenderer*>(r); }
extern "C" void smash_fila_window(void* r,ANativeWindow* w) { static_cast<GameRenderer*>(r)->attach(w); }
extern "C" uint32_t smash_fila_texture(void* r,const uint8_t* d,uint32_t w,uint32_t h,bool ui) {
    return static_cast<GameRenderer*>(r)->texture(d,w,h,ui);
}
extern "C" uint32_t smash_fila_mesh(void* r,const SmashVertex* d,uint32_t n,const uint32_t* i,uint32_t c,uint32_t t) {
    return static_cast<GameRenderer*>(r)->mesh(d,n,i,c,t);
}
extern "C" bool smash_fila_frame(void* r,const SmashDraw* d,uint32_t n,const SmashHudVertex* hud,uint32_t hn,
    uint32_t w,uint32_t h,const float* eye,const float* forward,const float* up,float fovy,float viewWidth,float bloom,float threshold,float exposure) {
    return static_cast<GameRenderer*>(r)->frame(d,n,hud,hn,w,h,eye,forward,up,fovy,viewWidth,bloom,threshold,exposure);
}
