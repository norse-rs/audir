#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod sles;

use crate::{api, api::Result};
use std::os::raw::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};

const BUFFER_NUM_FRAMES: usize = 1024; // TODO: random
const BUFFER_CHAIN_SIZE: usize = 2;

const DEFAULT_PHYSICAL_DEVICE: api::PhysicalDevice = 0;

pub struct Instance {
    instance: sles::SLObjectItf,
    engine: sles::SLEngineItf,
}

impl api::Instance for Instance {
    type Device = Device;

    unsafe fn create(_name: &str) -> Self {
        let mut instance = ptr::null();
        sles::slCreateEngine(
            &mut instance,
            0,
            ptr::null(),
            0,
            ptr::null(),
            ptr::null_mut(),
        );
        ((**instance).Realize).unwrap()(instance, sles::SL_BOOLEAN_FALSE as _);

        let mut engine = ptr::null();
        ((**instance).GetInterface).unwrap()(
            instance,
            sles::SL_IID_ENGINE,
            &mut engine as *mut _ as _,
        );

        Instance { instance, engine }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        vec![DEFAULT_PHYSICAL_DEVICE]
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        Some(DEFAULT_PHYSICAL_DEVICE)
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        Some(DEFAULT_PHYSICAL_DEVICE)
    }

    unsafe fn get_physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        assert_eq!(physical_device, DEFAULT_PHYSICAL_DEVICE);

        Ok(api::PhysicalDeviceProperties {
            device_name: "default".into(),
            driver_id: api::DriverId::OpenSLES,
            sharing: api::SharingModeFlags::CONCURRENT, // TODO
            streams: api::StreamFlags::INPUT | api::StreamFlags::OUTPUT,
        })
    }

    unsafe fn create_device(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
        sample_desc: api::SampleDesc,
    ) -> Device {
        assert_eq!(physical_device, DEFAULT_PHYSICAL_DEVICE);
        assert_eq!(sharing, api::SharingMode::Concurrent);

        Device {
            engine: self.engine,
            sample_desc,
            output_state: None,
            input_state: None,
        }
    }

    unsafe fn destroy_device(&self, device: &mut Device) {
        unimplemented!()
    }
}

pub struct Device {
    engine: sles::SLEngineItf,
    sample_desc: api::SampleDesc,
    output_state: Option<sles::SLPlayItf>,
    input_state: Option<sles::SLPlayItf>,
}

impl api::Device for Device {}

impl Device {
    pub unsafe fn output_stream(&mut self) -> OutputStream {
        let mut mix = ptr::null();
        ((**self.engine).CreateOutputMix).unwrap()(
            self.engine,
            &mut mix,
            0,
            ptr::null(),
            ptr::null(),
        );
        ((**mix).Realize).unwrap()(mix, sles::SL_BOOLEAN_FALSE as _);

        let mut audio_player = ptr::null();
        let mut locator_source = sles::SLDataLocator_AndroidSimpleBufferQueue {
            locatorType: sles::SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE as _,
            numBuffers: BUFFER_CHAIN_SIZE as _,
        };

        let mut create_player = |format| {
            let mut source = sles::SLDataSource {
                pLocator: &mut locator_source as *mut _ as _,
                pFormat: format,
            };
            let mut locator_sink = sles::SLDataLocator_OutputMix {
                locatorType: sles::SL_DATALOCATOR_OUTPUTMIX as _,
                outputMix: mix,
            };
            let mut sink = sles::SLDataSink {
                pLocator: &mut locator_sink as *mut _ as _,
                pFormat: ptr::null_mut(),
            };
            let ids = [sles::SL_IID_BUFFERQUEUE];
            let requirements = [sles::SL_BOOLEAN_TRUE];
            log::warn!(
                "{}",
                ((**self.engine).CreateAudioPlayer).unwrap()(
                    self.engine,
                    &mut audio_player,
                    &mut source,
                    &mut sink,
                    1,
                    ids.as_ptr(),
                    requirements.as_ptr() as _,
                )
            );
        };

        match self.sample_desc.format {
            api::Format::F32 => {
                let mut format_source = sles::SLAndroidDataFormat_PCM_EX {
                    formatType: sles::SL_ANDROID_DATAFORMAT_PCM_EX as _,
                    numChannels: self.sample_desc.channels as _,
                    sampleRate: (self.sample_desc.sample_rate * 1000) as _,
                    bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    channelMask: (sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT) as _, // TODO
                    endianness: sles::SL_BYTEORDER_LITTLEENDIAN as _, // TODO
                    representation: sles::SL_ANDROID_PCM_REPRESENTATION_FLOAT as _,
                };

                create_player(&mut format_source as *mut _ as _);
            }
            api::Format::U32 => {
                let mut format_source = sles::SLDataFormat_PCM {
                    formatType: sles::SL_DATAFORMAT_PCM as _,
                    numChannels: self.sample_desc.channels as _,
                    samplesPerSec: (self.sample_desc.sample_rate * 1000) as _,
                    bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    channelMask: (sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT) as _, // TODO
                    endianness: sles::SL_BYTEORDER_LITTLEENDIAN as _, // TODO
                };

                create_player(&mut format_source as *mut _ as _);
            }

            _ => unimplemented!(),
        }

        log::warn!(
            "realize: {}",
            ((**audio_player).Realize).unwrap()(audio_player, sles::SL_BOOLEAN_FALSE as _)
        );

        let mut queue: sles::SLAndroidSimpleBufferQueueItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(
            audio_player,
            sles::SL_IID_BUFFERQUEUE,
            &mut queue as *mut _ as _,
        );

        let mut state: sles::SLPlayItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(
            audio_player,
            sles::SL_IID_PLAY,
            &mut state as *mut _ as _,
        );

        let buffers = (0..BUFFER_CHAIN_SIZE)
            .map(|_| {
                let buffer_size = self.sample_desc.channels * BUFFER_NUM_FRAMES;
                let mut buffer = Vec::<u32>::with_capacity(buffer_size);
                buffer.set_len(buffer_size);
                buffer
            })
            .collect::<Vec<_>>();

        let pair = Arc::new((Mutex::new(true), Condvar::new()));

        extern "C" fn write_cb(_: sles::SLAndroidSimpleBufferQueueItf, user: *mut c_void) {
            unsafe {
                let pair = Arc::from_raw(user as *mut (Mutex<bool>, Condvar));
                {
                    let &(ref lock, ref cvar) = &*pair;
                    let mut signal = lock.lock().unwrap();
                    assert!(!*signal);
                    *signal = true;
                    cvar.notify_all();
                }
                Arc::into_raw(pair);
            }
        }

        let cb_pair = Arc::into_raw(Arc::clone(&pair)); // TODO: destroy
        ((**queue).RegisterCallback).unwrap()(queue, Some(write_cb), cb_pair as *mut _);

        OutputStream {
            mix,
            audio_player,
            pair,
            queue,

            buffers,
            cur_buffer: 0,
        }
    }

    pub unsafe fn properties(&self) -> api::DeviceProperties {
        api::DeviceProperties {
            num_channels: self.sample_desc.channels,
            channel_mask: api::ChannelMask::empty(), // TODO
            sample_rate: self.sample_desc.sample_rate,
            buffer_size: BUFFER_NUM_FRAMES,
        }
    }

    pub unsafe fn start(&mut self) {
        if let Some(ref state) = self.output_state {
            ((***state).SetPlayState).unwrap()(*state, sles::SL_PLAYSTATE_PLAYING as _);
        }
        if let Some(ref state) = self.input_state {
            ((***state).SetPlayState).unwrap()(*state, sles::SL_PLAYSTATE_PLAYING as _);
        }
    }

    pub unsafe fn stop(&mut self) {
        unimplemented!()
    }
}

pub struct OutputStream {
    mix: sles::SLObjectItf,
    audio_player: sles::SLObjectItf,
    queue: sles::SLAndroidSimpleBufferQueueItf,

    pair: Arc<(Mutex<bool>, Condvar)>,
    buffers: Vec<Vec<u32>>, // TODO: alignment
    cur_buffer: usize,
}

impl OutputStream {
    pub unsafe fn acquire_buffer(&mut self, timeout_ms: u32) -> (*mut u8, api::Frames) {
        {
            let &(ref lock, ref cvar) = &*self.pair;
            let mut signal = lock.lock().unwrap();
            while !*signal {
                signal = cvar.wait(signal).unwrap();
            }
            assert!(*signal);
            *signal = false;
        }

        // cycle active buffers
        self.cur_buffer = (self.cur_buffer + 1) % self.buffers.len();

        let buffer = &mut self.buffers[self.cur_buffer];

        (buffer.as_mut_ptr() as _, buffer.len() / 2) // TODO: channels + sizeof u32
    }

    pub unsafe fn release_buffer(&mut self, num_frames: api::Frames) {
        let buffer = &mut self.buffers[self.cur_buffer];
        ((**self.queue).Enqueue).unwrap()(
            self.queue,
            buffer.as_mut_ptr() as _,
            (buffer.len() * 4) as _,
        ); // TODO: sizeof u32, num_frames
    }
}
