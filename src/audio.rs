use crate::steam_audio::utils::interleave;
use crate::steam_audio_bindgen::{IPLAmbisonicsBinauralEffectParams, IPLAmbisonicsBinauralEffectSettings, IPLAmbisonicsDecodeEffectSettings, IPLAmbisonicsEncodeEffectParams, IPLAmbisonicsEncodeEffectSettings, IPLAudioEffectState, IPLDirectEffectFlags, IPLDirectEffectParams, IPLDirectEffectSettings, IPLPanningEffectParams, IPLPanningEffectSettings, IPLSpeakerLayout};

use super::math_3d::*;
use super::steam_audio::*;
use super::transform::Transform3;
use itertools::Itertools;
use rodio::dynamic_mixer::{DynamicMixer, DynamicMixerController};
use rodio::{buffer::SamplesBuffer, Sink, Source};
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::time::Duration;
use std::vec::IntoIter as VecIntoIter;

pub const MAXSINKS: usize = 32;
pub const FRAMES_TO_BUFFER: usize = 2;
pub const FRAME_SIZE: i32 = 1024;
pub const SAMPLE_RATE: u32 = 44100;

pub struct AudioSystem {
    pub audio_processor: AudioProcessor,
    stream: rodio::OutputStream,
    stream_handle: rodio::OutputStreamHandle,
    emitters: HashMap<u32, Emitter>,
    listeners: HashMap<u32, Listener>,
    library: SoundLibrary,
    sink: Sink,
}

pub struct Emitter {
    pub transform: Transform3,
    sounds: Vec<Sound>,
    sound_playback: Option<SoundPlayback>,
    sink: Sink,
    ambisonics_encode: AmbisonicsEncode,
    ambisonics_decode: AmbisonicsDecode,
    ambisonics_binaural: AmbisonicsBinaural,
    panning: Panning,
    direct: Direct,
}

/*
Effect Stack:
  Ambisonics Rotation
  Ambisonics Binaural
  Path
*/
pub struct Listener {
    pub transform: Transform3,
    frame_count: usize,
    audio_queue: VecDeque<SoundFrame>,
    mono_buffer: Buffer,
    stereo_buffer: Buffer,
    ambisonics2_buffer: Buffer,
    ambisonics1_buffer: Buffer,
}

#[derive(Clone)]
pub struct Sound {
    frames: VecIntoIter<SoundFrame>,
    channels: u16,
    sample_rate: u32,
    duration: Duration,
}

#[derive(Debug, Clone)]
pub struct SoundFrame {
    data: VecIntoIter<f32>,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    size: usize,
}

#[derive(Default)]
pub struct SoundLibrary {
    sounds: HashMap<&'static str, Sound>,
}

pub enum SoundPlayback {
    Once,
    Loop,
    PingPong,
    Partial(Range<f32>),
}

impl Sound {
    #[inline]
    pub fn new(mut data: Vec<f32>, channels: u16, sample_rate: u32) -> Self {
        assert!(!data.is_empty(), "Empty Data Vector");

        let duration_ms = 1_000u64
            .checked_mul(data.len() as u64 * FRAME_SIZE as u64)
            .unwrap()
            / sample_rate as u64
            / channels as u64;

        let duration = Duration::from_millis(duration_ms);

        let duration_ns =
            (1_000_000_000u64 * FRAME_SIZE as u64) / sample_rate as u64 / channels as u64;
        let frame_duration = Duration::new(
            duration_ns / 1_000_000_000,
            (duration_ns % 1_000_000_000) as u32,
        );
        let frames: VecIntoIter<SoundFrame> = data
            .chunks(FRAME_SIZE.max(1) as usize)
            .map(|c| {
                let mut pad: Vec<f32> = vec![0.0; FRAME_SIZE as usize - c.len()];
                let mut v = c.to_vec();
                v.append(&mut pad);
                SoundFrame {
                    size: v.len(),
                    data: v.into_iter(),
                    channels,
                    sample_rate,
                    duration: frame_duration,
                }
            })
            .collect::<Vec<SoundFrame>>()
            .into_iter();
        Self {
            frames,
            channels,
            sample_rate,
            duration,
        }
    }
}

impl Emitter {
    pub fn new(
        transform: Transform3,
        sound_playback: Option<SoundPlayback>,
        audio_system: &mut AudioSystem,
    ) -> Self {
        let audio_processor = &mut audio_system.audio_processor;
        let ambisonics_encode = AmbisonicsEncode::create(
            audio_processor,
            IPLAmbisonicsEncodeEffectSettings { maxOrder: 2 },
        );
        let ambisonics_decode = AmbisonicsDecode::create(audio_processor, IPLAmbisonicsDecodeEffectSettings{
            maxOrder: 2,
            speakerLayout: IPLSpeakerLayout {
                type_: crate::steam_audio_bindgen::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_STEREO,
                numSpeakers: 2,
                speakers: std::ptr::null_mut()
            },
            hrtf: audio_processor.hrtf(),
        });
        let ambisonics_binaural = AmbisonicsBinaural::create(
            audio_processor,
            IPLAmbisonicsBinauralEffectSettings {
                hrtf: audio_processor.hrtf(),
                maxOrder: 2,
            },
        );
        let panning = Panning::create(
            audio_processor,
            IPLPanningEffectSettings {
                speakerLayout: IPLSpeakerLayout {
                    type_: crate::steam_audio_bindgen::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_STEREO,
                    numSpeakers: 2,
                    speakers: std::ptr::null_mut()
                },
            },
        );
        let direct = Direct::create(audio_processor, IPLDirectEffectSettings { numChannels: 9 });
        let sink = Sink::try_new(&audio_system.stream_handle).expect("Failed to create sink");
        Self {
            transform,
            sounds: vec![],
            sink,
            sound_playback,
            ambisonics_encode,
            ambisonics_decode,
            ambisonics_binaural,
            panning,
            direct,
        }
    }

    pub fn with_sound(mut self, sound_name: &'static str, library: &SoundLibrary) -> Self {
        if let Some(sound) = library.get_sound(sound_name) {
            self.sounds.insert(self.sounds.len(), sound.clone());
        }
        self
    }

    #[inline]
    pub fn get_frames(&mut self) -> Option<Vec<SoundFrame>> {
        let frames: Vec<SoundFrame> = self
            .sounds
            .iter_mut()
            .filter_map(|s| s.frames.next())
            .collect();

        if frames.is_empty() {
            None
        } else {
            Some(frames)
        }
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    #[inline]
    fn play_frame(&mut self, frame: SoundFrame) {
        self.sink.set_volume(0.2);
        self.sink.append(frame);
    }
}

impl Iterator for Sound {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        (&mut self.frames)
            .peekable()
            .peek_mut()
            .and_then(|mut sf| sf.data.next())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.frames.size_hint()
    }
}

impl Iterator for SoundFrame {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        self.data.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }
}

impl Source for SoundFrame {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(self.duration)
    }
}

impl Source for Sound {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(self.duration)
    }
}

impl SoundLibrary {
    pub fn create_library() -> Self {
        Self {
            sounds: HashMap::new(),
        }
    }

    pub fn with_raw_sound_file(mut self, name: &'static str) -> Self {
        if let Ok(input_audio) = std::fs::read(name) {
            let sound = Sound::new(
                bytemuck::cast_slice(input_audio.as_slice()).to_vec(),
                1,
                44100,
            );
            self.sounds.insert(name.split('\\').last().unwrap(), sound);
        }
        self
    }

    pub fn with_sound(mut self, name: &'static str, sound: Sound) -> Self {
        self.sounds.insert(name, sound);
        self
    }

    pub fn get_sound(&self, name: &'static str) -> Option<&Sound> {
        self.sounds.get(name)
    }
}

impl Listener {
    pub fn new(transform: Transform3, audio_processor: &mut AudioProcessor) -> Self {
        let mut mono_buffer = Buffer::empty(audio_processor, 1);
        let mut ambisonics1_buffer = Buffer::empty(audio_processor, 9);
        let mut ambisonics2_buffer = Buffer::empty(audio_processor, 9);
        let mut stereo_buffer = Buffer::empty(audio_processor, 2);
        Self {
            transform,
            frame_count: 0,
            audio_queue: VecDeque::new(),
            mono_buffer,
            stereo_buffer,
            ambisonics1_buffer,
            ambisonics2_buffer,
        }
    }
    #[inline]
    pub fn get_frames(&mut self) -> Vec<SoundFrame> {
        //get all the sounds for the current frame
        let frame_sounds = self
            .audio_queue
            .drain(0..self.frame_count)
            .collect::<Vec<SoundFrame>>();
        //reset the counter
        self.frame_count = 0;

        frame_sounds
    }

    #[inline]
    fn add_frames(&mut self, frames: Vec<SoundFrame>) {
        self.frame_count += frames.len();
        self.audio_queue.append(&mut frames.into_iter().collect());
    }

    #[inline]
    pub fn apply_convolutions(
        &mut self,
        mut frame: SoundFrame,
        emitter: &Emitter,
        audio_processor: &mut AudioProcessor,
    ) -> SoundFrame {
        let emitter_position = self.transform.pos - emitter.transform.pos;
        let emitter_position2 = emitter.transform.pos - self.transform.pos;
        let rotation =
            Rotor3::from_vectors(self.transform.rot.forward(), emitter_position2.normal());
        let distance = 1.0 / emitter_position.length();
        //apply the effects and return the new frame
        let mut output: Vec<f32> = Vec::with_capacity(2 * frame.size as usize);
        let mut mono_buffer = Buffer::new_with_data(audio_processor, 1, frame.data.collect());
        // emitter.panning.apply(
        //     IPLPanningEffectParams{ direction: emitter_position2.normal().into() },
        //     &mut mono_buffer,
        //     &mut self.stereo_buffer,
        // );

        emitter.ambisonics_encode.apply(
            IPLAmbisonicsEncodeEffectParams {
                direction: rotation.forward().normal().into(),
                order: 2,
            },
            &mut mono_buffer,
            &mut self.ambisonics1_buffer,
        );

        emitter.direct.apply(
            IPLDirectEffectParams {
                flags: IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYAIRABSORPTION|
                IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDIRECTIVITY|
                IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDISTANCEATTENUATION|
                IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYTRANSMISSION,
                transmissionType: crate::steam_audio_bindgen::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQDEPENDENT,
                distanceAttenuation: distance,
                airAbsorption: [distance * 3.0, distance * 2.0, distance * 2.5],
                directivity: 1.0,
                occlusion: 0.0,
                transmission: [0.3, 0.2, 0.1],
            },
            &mut self.ambisonics1_buffer,
            &mut self.ambisonics2_buffer,
        );

        emitter.ambisonics_binaural.apply(
            IPLAmbisonicsBinauralEffectParams {
                hrtf: audio_processor.hrtf(),
                order: 2,
            },
            &mut self.ambisonics2_buffer,
            &mut self.stereo_buffer,
        );
        let o = interleave(audio_processor, output, &mut self.stereo_buffer);
        frame.data = o.into_iter();
        frame.channels = 2;
        frame
    }

}

impl AudioSystem {
    pub fn create_system() -> Self {
        let (stream, stream_handle) =
            rodio::OutputStream::try_default().expect("No handle to output stream");
        let audio_processor = AudioProcessor::new(44100, FRAME_SIZE);
        let sink = Sink::try_new(&stream_handle).expect("Failed to create sink");
        Self {
            stream,
            stream_handle,
            listeners: HashMap::new(),
            emitters: HashMap::new(),
            library: SoundLibrary::default(),
            audio_processor,
            sink,
        }
    }

    pub fn with_emitter(mut self, emitter: Emitter) -> Self {
        self.emitters.insert(self.emitters.len() as u32, emitter);
        self
    }

    pub fn with_library(mut self, library: SoundLibrary) -> Self {
        self.library = library;
        self
    }

    pub fn with_listener(mut self, listener: Listener) -> Self {
        self.listeners.insert(self.listeners.len() as u32, listener);
        self
    }

    pub fn register_emitter(&mut self, emitter: Emitter) -> u32 {
        let uid = self.emitters.len() as u32;
        self.emitters.insert(uid, emitter);
        uid
    }

    pub fn register_listener(&mut self, listener: Listener) -> u32 {
        let uid = self.listeners.len() as u32;
        self.listeners.insert(uid, listener);
        uid
    }

    pub fn get_emitter(&mut self, uid: u32) -> Option<&mut Emitter> {
        self.emitters.get_mut(&uid)
    }

    pub fn get_listener(&mut self, uid: u32) -> Option<&mut Listener> {
        self.listeners.get_mut(&uid)
    }

    fn frame_to_buffer(frame: SoundFrame) -> SamplesBuffer<f32> {
        SamplesBuffer::new(
            frame.channels as u16,
            frame.sample_rate as u32,
            frame.data.collect::<Vec<f32>>(),
        )
    }

    fn play_listener(listener: &Listener) {
        //For all the frames in get_frames, create buffers and append them to a sink
    }

    #[inline]
    pub fn update(&mut self) {
        if self.sink.len() < FRAMES_TO_BUFFER {
            let (mixer_controller, mixer) = rodio::dynamic_mixer::mixer::<f32>(2, 44100);
            //for each Listener, apply_convolutions.
            for (_, listener) in self.listeners.iter_mut() {
                //for each Emitter, get the frames and send them to all Listeners
                for (emitter_id, emitter) in self.emitters.iter_mut() {
                    if let Some(frames) = emitter.get_frames() {
                        for (i, frame) in frames.into_iter().enumerate() {
                            let out =
                                listener.apply_convolutions(
                                    frame,
                                    &*emitter,
                                    &mut self.audio_processor,
                                );
                            mixer_controller.add(out);
                        }
                    }
                }
            }
            self.sink.append(mixer);
        }
    }
}

impl SoundFrame {
    pub fn empty() -> Self {
        Self {
            data: vec![].into_iter(),
            duration: std::time::Duration::from_secs(0),
            channels: 0,
            sample_rate: 0,
            size: 0,
        }
    }

    fn interleave(mut self) -> Self {
        let chunk_size = self.data.len() / 2;
        let frame_data = self.data.collect::<Vec<_>>();
        let mut chunks = frame_data.chunks(chunk_size);
        self.data = chunks
            .next()
            .unwrap()
            .iter()
            .copied()
            .interleave(chunks.next().unwrap().iter().copied())
            .collect::<Vec<_>>()
            .into_iter();
        self
    }

    fn mono_to_stereo(mut self) -> Self {
        self.channels = 2;
        let c2 = self.data.clone();
        self.data = self
            .data
            .interleave(c2)
            .collect::<Vec<f32>>()
            .into_iter();
        self
    }
}

// struct SinkPool {
//     sinks: HashMap<u32, rodio::Sink>,
// }

// impl SinkPool {
//     fn new(stream_handle: &rodio::OutputStreamHandle) -> Self {
//         Self {
//             sinks: (0..MAXSINKS)
//                 .into_iter()
//                 .map(|_| Sink::try_new(stream_handle).expect("Failed to create sink"))
//                 .collect(),
//         }
//     }

//     fn get_sink(&mut self, emitter_id: u32) -> Option<&mut Sink> {

//         self.sinks.iter_mut().find_map(|(id, s)| {
//             if (emitter_id == *id) && (s.len() < FRAMES_TO_BUFFER) {
//                 Some(s)
//             } else {
//                 None
//             }
//         })
//     }
// }
