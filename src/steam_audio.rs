use super::steam_audio_bindgen::*;
use olc_pge_macros::*;
use std::ptr::slice_from_raw_parts;
use std::sync::atomic::AtomicPtr;
use std::sync::Mutex;

pub type Context = IPLContext;
pub type AudioSettings = IPLAudioSettings;
pub type DefaultSampleFormat = f32;
pub const SAMPLESIZE: usize = std::mem::size_of::<DefaultSampleFormat>();
pub const PTRWIDTH: usize = std::mem::size_of::<usize>();
type Hrtf = IPLHRTF;

const IPL_CONTEXT_SETTINGS: IPLContextSettings = IPLContextSettings {
    version: STEAMAUDIO_VERSION_MAJOR << 16_u32
        | STEAMAUDIO_VERSION_MINOR << 8_u32
        | STEAMAUDIO_VERSION_PATCH,
    logCallback: None,
    allocateCallback: None,
    freeCallback: None,
    simdLevel: IPLSIMDLevel::IPL_SIMDLEVEL_AVX512,
};

pub struct AudioProcessor {
    context: Context,
    pub audio_settings: AudioSettings,
    hrtf: Hrtf,
}

pub struct Buffer {
    frame_size: i32,
    sample_rate: i32,
    channels: i32,
    frame_position: i32,

    cursor: std::io::Cursor<Vec<DefaultSampleFormat>>,
    data_ptr: AtomicPtr<DefaultSampleFormat>,
    ipl: IPLAudioBuffer,
}

pub struct EffectDescriptor {
    ty: EffectType,
    settings: EffectSettings,
}

pub enum EffectType {
    AmbisonicsBinaural,
    AmbisonicsEncode,
    AmbisonicsDecode,
    AmbisonicsPanning,
    AmbisonicsRotation,
    Binaural,
    Direct,
    Panning,
    Path,
    Reflection,
    VirtualSurround,
}

pub enum EffectSettings {
    AmbisonicsBinauralSettings {
        max_order: i32,
    },
    AmbisonicsEncodeSettings {
        max_order: i32,
    },
    AmbisonicsDecodeSettings {
        speaker_layout: SpeakerLayout,
        max_order: i32,
    },
    AmbisonicsPanningSettings {
        speaker_layout: SpeakerLayout,
        max_order: i32,
    },
    AmbisonicsRotationSettings {
        max_order: i32,
    },
    BinauralSettings,
    DirectSettings {
        num_channels: i32,
    },
    PanningSettings {
        speaker_layer: SpeakerLayout,
    },
    PathSettings {
        max_order: i32,
    },
    ReflectionSettings {
        ty: ReflectionEffectType,
        ir_size: i32,
        num_channels: i32,
    },
    VirtualSurroundSettings {
        speaker_layout: SpeakerLayout,
    },
}

pub enum ReflectionEffectType {}
pub enum EffectParameters {}
pub struct SpeakerLayout;

#[ipl]
#[derive(Effect)]
pub struct AmbisonicsBinaural {}
#[ipl]
#[derive(Effect)]
pub struct AmbisonicsEncode {}
#[ipl]
#[derive(Effect)]
pub struct AmbisonicsDecode {}
#[ipl]
#[derive(Effect)]
pub struct AmbisonicsPanning {}
#[ipl]
#[derive(Effect)]
pub struct AmbisonicsRotation {}
#[ipl]
#[derive(Effect)]
pub struct Binaural {}
#[ipl]
#[derive(Effect)]
pub struct Direct {}
#[ipl]
#[derive(Effect)]
pub struct Panning {}
#[ipl]
#[derive(Effect)]
pub struct Path {}
#[ipl]
#[derive(Effect)]
pub struct VirtualSurround {}

//These take 5 parameters, so we'll manually implement this
#[ipl]
pub struct Reflection {}

pub trait Effect {
    type Settings;
    type Params;
    fn create(audio_processor: &mut AudioProcessor, settings: Self::Settings) -> Self;
    fn apply(&self, params: Self::Params, in_buffer: &mut Buffer, out_buffer: &mut Buffer) -> IPLAudioEffectState;
    fn drop_effect(&mut self);
    fn reset(&mut self);
}

impl AudioProcessor {
    pub fn new(sample_rate: i32, frame_size: i32) -> Self {
        AudioProcessor::create_context(sample_rate, frame_size)
    }

    pub fn hrtf(&self) -> IPLHRTF {
        self.hrtf
    }
    pub fn context(&self) -> IPLContext {
        self.context
    }

    pub fn create_context(sample_rate: i32, frame_size: i32) -> Self {
        let context = unsafe {
            let mut ipl_context = std::mem::MaybeUninit::uninit();
            assert_eq!(
                IPLerror::IPL_STATUS_SUCCESS,
                iplContextCreate(&IPL_CONTEXT_SETTINGS, ipl_context.as_mut_ptr())
            );
            ipl_context.assume_init()
        };

        let mut audio_settings = IPLAudioSettings {
            samplingRate: sample_rate,
            frameSize: frame_size,
        };

        let hrtf = unsafe {
            let mut hrtf = std::mem::MaybeUninit::uninit();
            assert_eq!(
                IPLerror::IPL_STATUS_SUCCESS,
                iplHRTFCreate(
                    context,
                    &mut audio_settings,
                    &mut IPLHRTFSettings {
                        type_: IPLHRTFType::IPL_HRTFTYPE_DEFAULT,
                        sofaFileName: std::ptr::null(),
                    },
                    hrtf.as_mut_ptr(),
                )
            );
            hrtf.assume_init()
        };
        Self {
            context,
            audio_settings,
            hrtf,
        }
    }

    pub fn update_settings(&mut self, sample_rate: i32, frame_size: i32) {
        self.audio_settings.samplingRate = sample_rate;
        self.audio_settings.frameSize = frame_size;
    }
}

impl Buffer {
    pub fn empty(audio_processor: &AudioProcessor, num_channels: i32) -> Self {
        let ipl = unsafe {
            let mut buffer = std::mem::MaybeUninit::uninit();
            iplAudioBufferAllocate(
                audio_processor.context(),
                num_channels,
                audio_processor.audio_settings.frameSize,
                buffer.as_mut_ptr(),
            );
            buffer.assume_init()
        };
        let mut empty_buf = vec![];
        Self {
            cursor: std::io::Cursor::new(empty_buf),
            ipl,
            channels: num_channels,
            sample_rate: audio_processor.audio_settings.samplingRate,
            frame_size: audio_processor.audio_settings.frameSize,
            frame_position: 0,
            data_ptr: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn new_with_data(
        audio_processor: &AudioProcessor,
        num_channels: i32,
        data: Vec<DefaultSampleFormat>,
    ) -> Self {
        let mut buf = Self {
            cursor: std::io::Cursor::new(data),
            ipl: IPLAudioBuffer {
                numChannels: num_channels,
                numSamples: audio_processor.audio_settings.frameSize,
                data: AtomicPtr::new(std::ptr::null_mut()),
            },
            channels: num_channels,
            sample_rate: audio_processor.audio_settings.samplingRate,
            frame_size: audio_processor.audio_settings.frameSize,
            data_ptr: AtomicPtr::new(std::ptr::null_mut()),
            frame_position: 0,
        };
        buf.update_ipl_data();
        buf
    }

    #[inline]
    pub fn advance_frame(&mut self) {
        self.advance_cursor(self.frame_size as u64);
        self.frame_position = 0;
        self.update_ipl_data();
    }

    #[inline]
    pub fn sample(&mut self) -> Option<DefaultSampleFormat> {
        if self.cursor.get_ref().is_empty() {
            self.try_read_value_from_ipl()
        } else {
            let mut sample_buffer = [0_u8; SAMPLESIZE];
            if let Ok(bytes) = self.read(&mut sample_buffer) {
                if bytes == SAMPLESIZE {
                    Some(bytemuck::cast(sample_buffer))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    #[inline]
    fn advance_cursor(&mut self, adv: u64) {
        let p: u64 = self.cursor.position() as u64;
        self.cursor.set_position(p + adv);
    }

    #[inline]
    fn update_ipl_data(&mut self) {
        self.update_data_pointer();
        self.ipl.data = AtomicPtr::new(&mut self.data_ptr);
    }

    #[inline]
    fn update_data_pointer(&mut self) {
        let start_data = self.cursor.get_mut().as_mut_ptr();
        let pos = self.cursor.position() as usize * SAMPLESIZE;
        self.data_ptr =
            AtomicPtr::new(unsafe { (start_data as usize + pos) as *mut DefaultSampleFormat });
    }

    #[inline]
    pub fn get_data_pointer(&mut self) -> *mut DefaultSampleFormat {
        *self.data_ptr.get_mut()
    }

    #[inline]
    pub fn try_read_value_from_ipl(&mut self) -> Option<DefaultSampleFormat> {
        self.update_ipl_data();
        let start_data = unsafe { (*(*self.ipl.data.get_mut())).get_mut() };
        let pos = self.cursor.position() + self.frame_position as u64;
        let data_ptr =
            unsafe { (*start_data as usize + (pos as usize)) as *mut DefaultSampleFormat };
        if !data_ptr.is_null() {
            let v = unsafe { *data_ptr };
            if (-1.0..=1.0).contains(&v) {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    fn bytes_left(&self) -> i32 {
        (self.cursor.get_ref().len() as i32
            - (self.cursor.position() + self.frame_position as u64) as i32)
            * SAMPLESIZE as i32
    }

    #[inline]
    fn advance_frame_position(&mut self, pos: i32) {
        self.frame_position += 1;
        if self.frame_position > self.frame_size {
            self.advance_frame();
            self.frame_position = 0;
        }
    }

    pub fn clear_ipl(&mut self){

    }

    pub fn all_samples(&mut self) -> Vec<DefaultSampleFormat> {
        let vec = self.cursor.get_ref();
        if !vec.is_empty() {
            vec.clone()
        } else {
            let mut vec = Vec::with_capacity((self.channels * self.frame_size) as usize);
            for c in 0..self.channels as usize {
                let d_ptr = self.ipl.data.load(std::sync::atomic::Ordering::Relaxed);
                let mut channel_ptr: *mut DefaultSampleFormat = unsafe {
                    let d_ptr = d_ptr as *mut *mut f32;
                    *((d_ptr as usize + (c * PTRWIDTH))
                        as *mut *mut DefaultSampleFormat)
                };
                unsafe {
                    let mut c_v = (*slice_from_raw_parts(
                        channel_ptr,
                        self.frame_size as usize,
                    )).to_vec();
                    vec.append(&mut c_v);
                }
            }
            vec
        }
    }

    pub fn channel_samples(&mut self, channel: usize) -> Vec<DefaultSampleFormat> {
        let vec = self.cursor.get_ref();
        if !vec.is_empty() {
            vec.clone()
        } else {
            let d_ptr = self.ipl.data.load(std::sync::atomic::Ordering::Relaxed);
            let mut channel_ptr: *mut DefaultSampleFormat = unsafe {
                let d_ptr = d_ptr as *mut *mut f32;
                *((d_ptr as usize + (channel * SAMPLESIZE))
                  as *mut *mut DefaultSampleFormat)
            };
            unsafe {
                (*slice_from_raw_parts(
                    channel_ptr,
                    self.frame_size as usize,
                )).to_vec()
            }
        }
    }
}

impl std::io::Read for Buffer {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_left = self.bytes_left();
        let buf_len = buf.len();
        let n: i32 = bytes_left - buf_len as i32;
        let p: usize = self.cursor.position() as usize;
        let c: usize = (n as usize).min(buf_len) / SAMPLESIZE;
        let o: usize = (p + c);
        if n > SAMPLESIZE as i32 {
            bytemuck::cast_slice::<DefaultSampleFormat, u8>(
                &self.cursor.get_ref().as_slice()[p..o],
            )
            .iter()
            .enumerate()
            .for_each(|(i, b)| {
                buf[i] = *b;
            });
            self.advance_frame_position(c as i32);
            Ok(c * SAMPLESIZE)
        } else {
            Ok(0)
        }
    }
}

impl std::io::Write for Buffer {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.cursor
            .get_mut()
            .extend_from_slice(bytemuck::cast_slice(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unimplemented!()
    }
}

use std::io::Read;
impl std::iter::Iterator for Buffer {
    type Item = DefaultSampleFormat;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.advance_frame_position(1);
        self.sample()
    }
}

pub mod utils {
    #[inline]
    pub fn interleave(
        audio_processor: &super::AudioProcessor,
        mut output_audio_frame: Vec<f32>,
        in_buffer: &mut super::Buffer,
    ) -> Vec<f32> {
        unsafe {
            let (frame_ptr, frame_len, frame_capacity) = output_audio_frame.into_raw_parts();
            super::iplAudioBufferInterleave(
                audio_processor.context(),
                &mut in_buffer.ipl,
                frame_ptr,
            );
            (*std::slice::from_raw_parts(frame_ptr, frame_capacity)).to_vec()
        }
    }
}

impl Effect for Reflection {
    type Settings = IPLReflectionEffectSettings;
    type Params = IPLReflectionEffectParams;
    fn create(audio_processor: &mut AudioProcessor, mut settings: Self::Settings) -> Self {
        let effect = unsafe {
            let mut effect = std::mem::MaybeUninit::uninit();
            assert_eq!(
                IPLerror::IPL_STATUS_SUCCESS,
                iplReflectionEffectCreate(
                    audio_processor.context,
                    &mut audio_processor.audio_settings,
                    &mut settings,
                    effect.as_mut_ptr(),
                )
            );
            effect.assume_init()
        };
        Self { settings, effect }
    }

    fn apply(&self, mut params: Self::Params, in_buffer: &mut Buffer, out_buffer: &mut Buffer) -> IPLAudioEffectState {
        unsafe {
            let state = iplReflectionEffectApply(
                self.effect,
                &mut params,
                &mut in_buffer.ipl,
                &mut out_buffer.ipl,
                std::ptr::null_mut(),
            );
            out_buffer.data_ptr = AtomicPtr::new(*(*(*out_buffer.ipl.data.get_mut())).get_mut());
            state
        }
    }

    fn reset(&mut self){
        unsafe{
            iplReflectionEffectReset(self.effect);
        }
    }

    fn drop_effect(&mut self) {
        unsafe {
            iplReflectionEffectRelease(&mut self.effect);
        }
    }
}

impl Drop for Reflection {
    fn drop(&mut self) {
        self.drop_effect();
    }
}

impl Drop for AudioProcessor {
    fn drop(&mut self) {
        unsafe {
            iplContextRelease(&mut self.context);
            iplHRTFRelease(&mut self.hrtf);
        }
    }
}
impl From<crate::math_3d::Vector3> for IPLVector3 {
    fn from(v: crate::math_3d::Vector3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}
