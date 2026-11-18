#[cfg(not(target_os = "android"))]
use std::ffi::CString;
use std::ffi::{c_char, c_void, CStr};
// Link the SDL build selected by Cargo, rather than an arbitrary system SDL.
use sdl3_sys as _;
pub type Ptr = *mut c_void;
#[derive(Clone, Copy, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
#[repr(C, align(8))]
pub struct Event(pub [u8; 128]);
impl Event {
    pub fn kind(&self) -> u32 {
        u32::from_ne_bytes(self.0[..4].try_into().unwrap())
    }
    pub fn key(&self) -> u32 {
        u32::from_ne_bytes(self.0[28..32].try_into().unwrap())
    }
    pub fn mouse(&self) -> (f32, f32) {
        (
            f32::from_ne_bytes(self.0[28..32].try_into().unwrap()),
            f32::from_ne_bytes(self.0[32..36].try_into().unwrap()),
        )
    }
}
extern "C" {
    pub fn SDL_Init(flags: u32) -> bool;
    pub fn SDL_Quit();
    pub fn SDL_GetError() -> *const c_char;
    fn SDL_GetCurrentVideoDriver() -> *const c_char;
    fn SDL_GetVersion() -> i32;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_SetAttribute(attribute: i32, value: i32) -> bool;
    fn SDL_CreateWindow(title: *const c_char, w: i32, h: i32, flags: u64) -> Ptr;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_CreateContext(win: Ptr) -> Ptr;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_DestroyContext(ctx: Ptr) -> bool;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_SetSwapInterval(interval: i32) -> bool;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_GetProcAddress(name: *const c_char) -> Ptr;
    #[cfg(not(target_os = "android"))]
    fn SDL_GL_SwapWindow(win: Ptr) -> bool;
    fn SDL_DestroyWindow(win: Ptr);
    fn SDL_GetWindowSize(win: Ptr, w: *mut i32, h: *mut i32) -> bool;
    fn SDL_GetWindowSizeInPixels(win: Ptr, w: *mut i32, h: *mut i32) -> bool;
    pub fn SDL_PollEvent(event: *mut Event) -> bool;
    fn SDL_PushEvent(event: *mut Event) -> bool;
    fn SDL_GetWindowID(window: Ptr) -> u32;
    pub fn SDL_GetTicks() -> u64;
    pub fn SDL_Delay(ms: u32);
    #[cfg(target_os = "android")]
    fn SDL_GetWindowProperties(window: Ptr) -> u32;
    #[cfg(target_os = "android")]
    fn SDL_GetPointerProperty(properties: u32, name: *const c_char, default: Ptr) -> Ptr;
    #[cfg(target_os = "android")]
    fn SDL_LockProperties(properties: u32) -> bool;
    #[cfg(target_os = "android")]
    fn SDL_UnlockProperties(properties: u32);
    #[cfg(target_os = "android")]
    fn SDL_LogMessage(category: i32, priority: i32, fmt: *const c_char, ...);
    #[cfg(target_os = "android")]
    fn ANativeWindow_acquire(window: Ptr);

}
pub fn error() -> String {
    unsafe {
        CStr::from_ptr(SDL_GetError())
            .to_string_lossy()
            .into_owned()
    }
}
pub struct Display {
    win: Ptr,
    #[cfg(not(target_os = "android"))]
    context: Ptr,
}
impl Display {
    pub fn push_click(&self, x: f32, y: f32) -> Result<(), String> {
        let (_, (w, h)) = self.sizes();
        let mut event = Event([0; 128]);
        event.0[..4].copy_from_slice(&0x401u32.to_ne_bytes());
        unsafe {
            event.0[16..20].copy_from_slice(&SDL_GetWindowID(self.win).to_ne_bytes());
        }
        event.0[24] = 1;
        event.0[25] = 1;
        event.0[26] = 1;
        event.0[28..32].copy_from_slice(&(x * w as f32 / self.view_width()).to_ne_bytes());
        event.0[32..36].copy_from_slice(&(y * h as f32 / 800.).to_ne_bytes());
        unsafe {
            if SDL_PushEvent(&mut event) {
                Ok(())
            } else {
                Err(error())
            }
        }
    }
    pub fn new(gles: bool, hidden: bool) -> Result<Self, String> {
        unsafe {
            if !SDL_Init(0x20) {
                return Err(error());
            }
            let version = SDL_GetVersion();
            eprintln!(
                "SDL {}.{}.{} video driver: {}",
                version / 1_000_000,
                version / 1000 % 1000,
                version % 1000,
                CStr::from_ptr(SDL_GetCurrentVideoDriver()).to_string_lossy()
            );
            #[cfg(target_os = "android")]
            {
                let _ = gles;
                sdl3_sys::hints::SDL_SetHint(c"SDL_ORIENTATIONS".as_ptr(), c"Portrait".as_ptr());
            }
            #[cfg(not(target_os = "android"))]
            for (a, v) in [
                (20, if gles { 4 } else { 1 }),
                (17, 3),
                (18, if gles { 0 } else { 3 }),
                (6, 24),
                (5, 1),
            ] {
                if !SDL_GL_SetAttribute(a, v) {
                    SDL_Quit();
                    return Err(error());
                }
            }
            let (title, flags, w, h) = if cfg!(target_os = "android") {
                (c"Pirate Smash | Filament + Bullet", 0x2021, 450, 800)
            } else {
                (
                    c"Pirate Smash | OpenGL + Bullet",
                    0x2022,
                    if std::env::args().any(|a| a == "--portrait") {
                        450
                    } else {
                        1100
                    },
                    800,
                )
            };
            let win = SDL_CreateWindow(title.as_ptr(), w, h, flags | if hidden { 0x8 } else { 0 });
            if win.is_null() {
                let e = error();
                SDL_Quit();
                return Err(e);
            }
            #[cfg(target_os = "android")]
            return Ok(Self { win });
            #[cfg(not(target_os = "android"))]
            {
                let context = SDL_GL_CreateContext(win);
                if context.is_null() {
                    let e = error();
                    SDL_DestroyWindow(win);
                    SDL_Quit();
                    return Err(e);
                }
                SDL_GL_SetSwapInterval(1);
                Ok(Self { win, context })
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    pub fn gl(&self) -> glow::Context {
        unsafe {
            glow::Context::from_loader_function(|s| {
                let name = CString::new(s).unwrap();
                SDL_GL_GetProcAddress(name.as_ptr()) as *const _
            })
        }
    }
    #[cfg(target_os = "android")]
    pub fn acquire_native_window(&self) -> Ptr {
        unsafe {
            let props = SDL_GetWindowProperties(self.win);
            if !SDL_LockProperties(props) {
                return std::ptr::null_mut();
            }
            let window = SDL_GetPointerProperty(
                props,
                c"SDL.window.android.window".as_ptr(),
                std::ptr::null_mut(),
            );
            if !window.is_null() {
                ANativeWindow_acquire(window);
            }
            SDL_UnlockProperties(props);
            window
        }
    }
    pub fn sizes(&self) -> ((u32, u32), (i32, i32)) {
        unsafe {
            let (mut w, mut h, mut pw, mut ph) = (0, 0, 0, 0);
            SDL_GetWindowSize(self.win, &mut w, &mut h);
            SDL_GetWindowSizeInPixels(self.win, &mut pw, &mut ph);
            ((pw.max(0) as u32, ph.max(0) as u32), (w, h))
        }
    }
    pub fn view_width(&self) -> f32 {
        {
            let ((w, h), _) = self.sizes();
            if w > 0 && h > 0 {
                return crate::math::VIEW_HEIGHT * w as f32 / h as f32;
            }
        }
        crate::math::DESKTOP_WIDTH
    }
    pub fn mouse(&self, e: &Event, view_width: f32) -> (f32, f32) {
        let (x, y) = e.mouse();
        let (_, (w, h)) = self.sizes();
        (
            x * view_width / w.max(1) as f32,
            y * crate::math::VIEW_HEIGHT / h.max(1) as f32,
        )
    }
    #[cfg(not(target_os = "android"))]
    pub fn present(&self) -> Result<(), String> {
        unsafe {
            if SDL_GL_SwapWindow(self.win) {
                Ok(())
            } else {
                Err(error())
            }
        }
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            #[cfg(not(target_os = "android"))]
            SDL_GL_DestroyContext(self.context);
            SDL_DestroyWindow(self.win);
            SDL_Quit();
        }
    }
}

#[cfg(target_os = "android")]
pub fn log_info(message: &str) {
    if let Ok(c) = std::ffi::CString::new(message) {
        unsafe {
            SDL_LogMessage(0, 3, c"%s".as_ptr(), c.as_ptr());
        }
    }
}
