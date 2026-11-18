use crate::sdl;
use sdl3_sys::{
    audio::{
        SDL_AudioDeviceID, SDL_AudioSpec, SDL_AudioStream, SDL_BindAudioStream,
        SDL_ClearAudioStream, SDL_CloseAudioDevice, SDL_CreateAudioStream, SDL_DestroyAudioStream,
        SDL_FlushAudioStream, SDL_GetAudioStreamQueued, SDL_LoadWAV, SDL_OpenAudioDevice,
        SDL_PutAudioStreamData, SDL_SetAudioStreamGain, SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,
        SDL_AUDIO_S16LE,
    },
    init::{SDL_InitSubSystem, SDL_QuitSubSystem, SDL_INIT_AUDIO},
    stdinc::{SDL_free, SDL_rand_r},
    timer::SDL_GetPerformanceCounter,
};
use std::{
    ffi::CStr,
    ptr::{self, NonNull},
    time::{SystemTime, UNIX_EPOCH},
};

const CANNON_CLIPS: [&CStr; 5] = [
    c"assets/audio/fire.wav",
    c"assets/audio/fire2.wav",
    c"assets/audio/fire3.wav",
    c"assets/audio/fire4.wav",
    c"assets/audio/fire5.wav",
];
const CANNON_FORMAT: SDL_AudioSpec = SDL_AudioSpec {
    format: SDL_AUDIO_S16LE,
    channels: 1,
    freq: 44_100,
};

pub struct CannonAudio {
    clips: Vec<Vec<u8>>,
    device: SDL_AudioDeviceID,
    voices: Vec<NonNull<SDL_AudioStream>>,
    next_voice: usize,
    random: u64,
}

impl CannonAudio {
    pub fn new() -> Result<Self, String> {
        let clips = CANNON_CLIPS
            .iter()
            .map(|&path| load_clip(path))
            .collect::<Result<_, _>>()?;
        let clock_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);

        unsafe {
            if !SDL_InitSubSystem(SDL_INIT_AUDIO) {
                return Err(sdl::error());
            }

            // Request roughly 11 ms per callback on Android. Keep explicit SDL hints
            // from the environment and let the backend negotiate the actual size.
            #[cfg(target_os = "android")]
            {
                use sdl3_sys::hints::{
                    SDL_SetHintWithPriority, SDL_HINT_AUDIO_DEVICE_SAMPLE_FRAMES, SDL_HINT_DEFAULT,
                };
                SDL_SetHintWithPriority(
                    SDL_HINT_AUDIO_DEVICE_SAMPLE_FRAMES,
                    c"512".as_ptr(),
                    SDL_HINT_DEFAULT,
                );
            }
            let device = SDL_OpenAudioDevice(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, ptr::null());
            let mut audio = Self {
                clips,
                device,
                voices: Vec::with_capacity(4),
                next_voice: 0,
                random: clock_seed ^ SDL_GetPerformanceCounter(),
            };
            if device.0 == 0 {
                return Err(sdl::error());
            }

            // SDL mixes these streams concurrently so rapid cannon shots do not queue up.
            for _ in 0..4 {
                let stream = SDL_CreateAudioStream(&CANNON_FORMAT, ptr::null());
                let voice = NonNull::new(stream).ok_or_else(sdl::error)?;
                audio.voices.push(voice);
                if !SDL_BindAudioStream(device, stream) || !SDL_SetAudioStreamGain(stream, 0.5) {
                    return Err(sdl::error());
                }
            }
            Ok(audio)
        }
    }

    pub fn play(&mut self) -> Result<(), String> {
        unsafe {
            let index = SDL_rand_r(&mut self.random, self.clips.len() as i32) as usize;
            let clip = &self.clips[index];
            let voice = self.voices[self.next_voice].as_ptr();
            self.next_voice = (self.next_voice + 1) % self.voices.len();
            // If all voices are busy, reuse the oldest so this shot starts immediately.
            let pending = SDL_GetAudioStreamQueued(voice);
            if pending < 0
                || (pending > 0 && !SDL_ClearAudioStream(voice))
                || !SDL_PutAudioStreamData(voice, clip.as_ptr().cast(), clip.len() as i32)
                || !SDL_FlushAudioStream(voice)
            {
                return Err(sdl::error());
            }
        }
        Ok(())
    }
}

impl Drop for CannonAudio {
    fn drop(&mut self) {
        unsafe {
            for voice in &self.voices {
                SDL_DestroyAudioStream(voice.as_ptr());
            }
            if self.device.0 != 0 {
                SDL_CloseAudioDevice(self.device);
            }
            SDL_QuitSubSystem(SDL_INIT_AUDIO);
        }
    }
}

fn load_clip(path: &CStr) -> Result<Vec<u8>, String> {
    let mut spec = SDL_AudioSpec::default();
    let mut samples = ptr::null_mut();
    let mut length = 0;
    unsafe {
        if !SDL_LoadWAV(path.as_ptr(), &mut spec, &mut samples, &mut length) {
            return Err(format!("{}: {}", path.to_string_lossy(), sdl::error()));
        }
        let valid =
            spec == CANNON_FORMAT && !samples.is_null() && length > 0 && length <= i32::MAX as u32;
        let clip = valid.then(|| std::slice::from_raw_parts(samples, length as usize).to_vec());
        SDL_free(samples.cast());
        clip.ok_or_else(|| {
            format!(
                "{} must contain 44.1 kHz mono PCM16 audio",
                path.to_string_lossy()
            )
        })
    }
}
