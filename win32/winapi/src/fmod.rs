//! A small implementation of the FMOD 3 sample API, on top of the host audio stream.
//!
//! The program loads raw PCM from memory and plays it on software channels.
//! FMUSIC_* is unused in practice: nothing imports a song loader.

use std::sync::{Arc, Mutex};

use runtime::Context;

use crate::Ptr;

const FSOUND_FREE: i32 = -1;
const FSOUND_ALL: i32 = -3;

const FSOUND_LOOP_NORMAL: u32 = 0x2;
const FSOUND_16BITS: u32 = 0x10;
const FSOUND_STEREO: u32 = 0x40;
const FSOUND_SIGNED: u32 = 0x100;
const FSOUND_LOADMEMORY: u32 = 0x8000;
const FSOUND_LOADRAW: u32 = 0x10000;

const DEFAULT_RATE: u32 = 22050;
const CHANNEL_COUNT: usize = 32;
/// Audio kept queued in the host. Larger survives slower frames, smaller
/// starts sounds sooner.
#[cfg(not(target_family = "wasm"))]
const TARGET_QUEUE_MS: u32 = 100;
#[cfg(not(target_family = "wasm"))]
const CHUNK_FRAMES: usize = 256;
// Each web chunk costs a round trip to the page, and must survive its jank.
#[cfg(target_family = "wasm")]
const TARGET_QUEUE_MS: u32 = 200;
#[cfg(target_family = "wasm")]
const CHUNK_FRAMES: usize = 1024;
/// Headroom so several simultaneous channels rarely clip.
const MASTER_GAIN: f32 = 0.7;
/// A channel handle holds the slot here and a reuse count above it, so a
/// handle kept after its sound ended cannot touch the slot's next sound.
const CHANNEL_INDEX_BITS: u32 = 12;

struct Sample {
    /// Mono samples, in the range -1..1.
    data: Vec<f32>,
    looping: bool,
}

#[derive(Default)]
struct Channel {
    sample: Option<Arc<Sample>>,
    position: f64,
    frequency: u32,
    volume: u8,
    paused: bool,
    muted: bool,
    looping: bool,
    generation: u32,
}

impl Channel {
    fn handle(&self, index: usize) -> i32 {
        (index as u32 | (self.generation << CHANNEL_INDEX_BITS)) as i32
    }

    /// Advance this channel, adding its output to `out`.
    fn mix(&mut self, out: &mut [f32], rate: u32) {
        let Some(sample) = self.sample.clone() else {
            return;
        };
        if self.paused {
            return;
        }
        let step = self.frequency as f64 / rate as f64;
        let gain = if self.muted {
            0.0
        } else {
            self.volume as f32 / 255.0
        };
        let len = sample.data.len();
        for value in out.iter_mut() {
            if self.position >= len as f64 {
                if !self.looping || len == 0 {
                    self.sample = None;
                    return;
                }
                self.position %= len as f64;
            }
            let index = self.position as usize;
            let next = if index + 1 < len {
                sample.data[index + 1]
            } else if self.looping {
                sample.data[0]
            } else {
                sample.data[index]
            };
            let fraction = (self.position - index as f64) as f32;
            *value += (sample.data[index] + (next - sample.data[index]) * fraction) * gain;
            self.position += step;
        }
    }
}

struct State {
    rate: u32,
    samples: Vec<Arc<Sample>>,
    channels: Vec<Channel>,
    stream: Option<host::AudioStream>,
    #[cfg(not(target_family = "wasm"))]
    dump: Option<std::fs::File>,
}

impl State {
    fn new() -> Self {
        Self {
            rate: DEFAULT_RATE,
            samples: Vec::new(),
            channels: (0..CHANNEL_COUNT).map(|_| Channel::default()).collect(),
            stream: None,
            #[cfg(not(target_family = "wasm"))]
            dump: std::env::var_os("THESEUS_AUDIO_DUMP")
                .and_then(|path| std::fs::File::create(path).ok()),
        }
    }

    fn sample(&self, handle: u32) -> Option<Arc<Sample>> {
        self.samples.get((handle as usize).checked_sub(1)?).cloned()
    }

    fn channel(&mut self, handle: i32) -> Option<&mut Channel> {
        let raw = handle as u32;
        let index = (raw & ((1 << CHANNEL_INDEX_BITS) - 1)) as usize;
        let channel = self.channels.get_mut(index)?;
        (channel.generation == raw >> CHANNEL_INDEX_BITS).then_some(channel)
    }

    /// Apply a change to one channel, or to all of them for FSOUND_ALL.
    fn each(&mut self, handle: i32, mut f: impl FnMut(&mut Channel)) -> u32 {
        if handle == FSOUND_ALL {
            self.channels.iter_mut().for_each(f);
            return 1;
        }
        match self.channel(handle) {
            Some(channel) => {
                f(channel);
                1
            }
            None => 0,
        }
    }

    fn play(&mut self, sample: Arc<Sample>, paused: bool) -> i32 {
        let Some(index) = self.channels.iter().position(|c| c.sample.is_none()) else {
            return FSOUND_FREE;
        };
        let channel = &mut self.channels[index];
        *channel = Channel {
            looping: sample.looping,
            sample: Some(sample),
            position: 0.0,
            frequency: self.rate,
            volume: 255,
            paused,
            muted: false,
            generation: (channel.generation + 1) & 0x7FFFF,
        };
        channel.handle(index)
    }

    /// Mix until the host has enough audio queued.
    fn pump(&mut self) {
        let rate = self.rate;
        let stream = self.stream.get_or_insert_with(|| {
            let stream = host::host().create_audio_stream(host::AudioSpec {
                sample_rate: rate,
                channels: 1,
            });
            stream.resume();
            stream
        });
        if !stream.is_open() {
            return;
        }
        let target = rate * TARGET_QUEUE_MS / 1000 * 2;
        while self.stream.as_ref().unwrap().queued_bytes() < target {
            let mut mixed = [0f32; CHUNK_FRAMES];
            for channel in &mut self.channels {
                channel.mix(&mut mixed, rate);
            }
            let bytes: Vec<u8> = mixed
                .iter()
                .flat_map(|value| {
                    let scaled = (value * MASTER_GAIN).clamp(-1.0, 1.0) * i16::MAX as f32;
                    (scaled as i16).to_le_bytes()
                })
                .collect();
            #[cfg(not(target_family = "wasm"))]
            if let Some(dump) = &mut self.dump {
                use std::io::Write;
                let _ = dump.write_all(&bytes);
            }
            self.stream.as_ref().unwrap().put_data(&bytes);
        }
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(STATE.lock().unwrap().get_or_insert_with(State::new))
}

/// Keep the host stream fed; called from the program's message loop.
pub fn pump() {
    if let Some(state) = STATE.lock().unwrap().as_mut() {
        state.pump();
    }
}

fn decode(data: &[u8], mode: u32) -> Vec<f32> {
    let frame: Vec<f32> = if mode & FSOUND_16BITS != 0 {
        data.chunks_exact(2)
            .map(|b| {
                let value = u16::from_le_bytes([b[0], b[1]]);
                if mode & FSOUND_SIGNED != 0 {
                    value as i16 as f32 / 32768.0
                } else {
                    (value as f32 - 32768.0) / 32768.0
                }
            })
            .collect()
    } else {
        data.iter()
            .map(|&b| {
                if mode & FSOUND_SIGNED != 0 {
                    b as i8 as f32 / 128.0
                } else {
                    (b as f32 - 128.0) / 128.0
                }
            })
            .collect()
    };
    if mode & FSOUND_STEREO != 0 {
        frame.chunks_exact(2).map(|s| (s[0] + s[1]) / 2.0).collect()
    } else {
        frame
    }
}

#[win32_derive::dllexport]
pub fn FSOUND_SetMemorySystem(
    _ctx: &mut Context,
    _pool: u32,
    _poollen: i32,
    _useralloc: u32,
    _userrealloc: u32,
    _userfree: u32,
) -> u32 {
    1
}

#[win32_derive::dllexport]
pub fn FSOUND_Init(_ctx: &mut Context, mixrate: i32, _maxsoftwarechannels: i32, _flags: u32) -> u32 {
    with(|state| {
        state.rate = mixrate as u32;
        state.pump();
    });
    1
}

#[win32_derive::dllexport]
pub fn FSOUND_Close(_ctx: &mut Context) -> u32 {
    with(|state| state.each(FSOUND_ALL, |channel| channel.sample = None));
    0
}

#[win32_derive::dllexport]
pub fn FSOUND_SetMute(_ctx: &mut Context, channel: i32, mute: u32) -> u32 {
    let mute = mute & 0xff != 0;
    with(|state| state.each(channel, |c| c.muted = mute))
}

#[win32_derive::dllexport]
pub fn FSOUND_SetPaused(_ctx: &mut Context, channel: i32, paused: u32) -> u32 {
    let paused = paused & 0xff != 0;
    with(|state| state.each(channel, |c| c.paused = paused))
}

#[win32_derive::dllexport]
pub fn FSOUND_SetLoopMode(_ctx: &mut Context, channel: i32, loopmode: u32) -> u32 {
    let looping = loopmode & FSOUND_LOOP_NORMAL != 0;
    with(|state| state.each(channel, |c| c.looping = looping))
}

#[win32_derive::dllexport]
pub fn FSOUND_SetVolume(_ctx: &mut Context, channel: i32, vol: i32) -> u32 {
    let volume = vol.clamp(0, 255) as u8;
    with(|state| state.each(channel, |c| c.volume = volume))
}

#[win32_derive::dllexport]
pub fn FSOUND_SetFrequency(_ctx: &mut Context, channel: i32, freq: i32) -> u32 {
    let frequency = freq.max(0) as u32;
    with(|state| state.each(channel, |c| c.frequency = frequency))
}

#[win32_derive::dllexport]
pub fn FSOUND_SetPriority(_ctx: &mut Context, _channel: i32, _priority: i32) -> u32 {
    1
}

#[win32_derive::dllexport]
pub fn FSOUND_IsPlaying(_ctx: &mut Context, channel: i32) -> u32 {
    with(|state| match state.channel(channel) {
        Some(c) if c.sample.is_some() => 1,
        _ => 0,
    })
}

#[win32_derive::dllexport]
pub fn FSOUND_StopSound(_ctx: &mut Context, channel: i32) -> u32 {
    with(|state| state.each(channel, |c| c.sample = None))
}

#[win32_derive::dllexport]
pub fn FSOUND_PlaySoundEx(
    _ctx: &mut Context,
    _channel: i32,
    sptr: u32,
    _dsp: u32,
    startpaused: u32,
) -> i32 {
    with(|state| {
        let Some(sample) = state.sample(sptr) else {
            log::warn!("FSOUND_PlaySoundEx: bad sample {sptr:x}");
            return FSOUND_FREE;
        };
        state.play(sample, startpaused & 0xff != 0)
    })
}

#[win32_derive::dllexport]
pub fn FSOUND_Sample_Load(
    ctx: &mut Context,
    _index: i32,
    name: Ptr<u8>,
    mode: u32,
    memlength: i32,
) -> u32 {
    if mode & FSOUND_LOADMEMORY == 0 || mode & FSOUND_LOADRAW == 0 {
        log::warn!("FSOUND_Sample_Load: unsupported mode {mode:x}");
        return 0;
    }
    let data = decode(&ctx.memory[name.addr..][..memlength as usize], mode);
    let sample = Arc::new(Sample {
        data,
        looping: mode & FSOUND_LOOP_NORMAL != 0,
    });
    with(|state| {
        state.samples.push(sample);
        state.samples.len() as u32
    })
}

#[win32_derive::dllexport]
pub fn FSOUND_Sample_Free(_ctx: &mut Context, _sptr: u32) {
    // Handles index `samples`, so the slot stays; the game loads few samples.
}

#[win32_derive::dllexport]
pub fn FMUSIC_PlaySong(_ctx: &mut Context, _module: u32) -> u32 {
    1
}

#[win32_derive::dllexport]
pub fn FMUSIC_StopSong(_ctx: &mut Context, _module: u32) -> u32 {
    1
}

#[win32_derive::dllexport]
pub fn FMUSIC_SetPaused(_ctx: &mut Context, _module: u32, _pause: u32) -> u32 {
    1
}
