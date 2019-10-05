use crate::{
    ChannelMask, DeviceProperties, DriverId, Format, Frames, PhysicalDeviceProperties, SampleDesc,
    SharingModeFlags,
};
use opensles as sles;
use std::os::raw::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};

const BUFFER_NUM_FRAMES: usize = 1024; // TODO: random
const BUFFER_CHAIN_SIZE: usize = 3; // TODO: random

pub struct Instance {
    instance: sles::bindings::SLObjectItf,
    engine: sles::bindings::SLEngineItf,
}

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        let mut instance = ptr::null();
        sles::bindings::slCreateEngine(
            &mut instance,
            0,
            ptr::null(),
            0,
            ptr::null(),
            ptr::null_mut(),
        );
        ((**instance).Realize).unwrap()(instance, sles::SL_BOOLEAN_FALSE);

        let mut engine = ptr::null();
        ((**instance).GetInterface).unwrap()(
            instance,
            sles::bindings::SL_IID_ENGINE,
            &mut engine as *mut _ as _,
        );

        Instance { instance, engine }
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        vec![PhysicalDevice::Input]
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        vec![PhysicalDevice::Output]
    }

    pub unsafe fn create_device(
        &self,
        _physical_device: &PhysicalDevice,
        sample_desc: SampleDesc,
    ) -> Device {
        Device {
            engine: self.engine,
            sample_desc,
        }
    }
}

pub enum PhysicalDevice {
    Input,
    Output,
}

impl PhysicalDevice {
    pub unsafe fn properties(&self) -> PhysicalDeviceProperties {
        let device_name = match *self {
            PhysicalDevice::Input => "Audio Input",
            PhysicalDevice::Output => "Audio Output",
        };

        PhysicalDeviceProperties {
            device_name: device_name.to_string(),
            driver_id: DriverId::OpenSLES,
            sharing: SharingModeFlags::CONCURRENT, // TODO
        }
    }
}

pub struct Device {
    engine: sles::bindings::SLEngineItf,
    sample_desc: SampleDesc,
    output_state: Option<sles::bindings::SLPlayItf>,
    input_state: Option<sles::bindings::SLPlayItf>,
}

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
        ((**mix).Realize).unwrap()(mix, sles::SL_BOOLEAN_FALSE);

        let mut audio_player = ptr::null();
        let mut locator_source = sles::bindings::SLDataLocator_AndroidSimpleBufferQueue {
            locatorType: 0x800007BD, // sles::bindings::SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE,
            numBuffers: BUFFER_CHAIN_SIZE as _,
        };

        let mut create_player = |format| {
            let mut source = sles::bindings::SLDataSource {
                pLocator: &mut locator_source as *mut _ as _,
                pFormat: format,
            };
            let mut locator_sink = sles::bindings::SLDataLocator_OutputMix {
                locatorType: sles::SL_DATALOCATOR_OUTPUTMIX,
                outputMix: mix,
            };
            let mut sink = sles::bindings::SLDataSink {
                pLocator: &mut locator_sink as *mut _ as _,
                pFormat: ptr::null_mut(),
            };
            let ids = [sles::bindings::SL_IID_BUFFERQUEUE];
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
                    requirements.as_ptr()
                )
            );
        };

        match self.sample_desc.format {
            Format::F32 => {
                let mut format_source = sles::bindings::SLAndroidDataFormat_PCM_EX {
                    formatType: 0x4, // SL_ANDROID_DATAFORMAT_PCM_EX
                    numChannels: self.sample_desc.channels as _,
                    sampleRate: self.sample_desc.sample_rate as u32 * 1000,
                    bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    channelMask: sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT, // TODO
                    endianness: sles::SL_BYTEORDER_LITTLEENDIAN,                             // TODO
                    representation: 0x3, // SL_ANDROID_PCM_REPRESENTATION_FLOAT
                };

                create_player(&mut format_source as *mut _ as _);
            }
            Format::U32 => {
                let mut format_source = sles::bindings::SLDataFormat_PCM {
                    formatType: sles::SL_DATAFORMAT_PCM,
                    numChannels: self.sample_desc.channels as _,
                    samplesPerSec: self.sample_desc.sample_rate as u32 * 1000, // TODO
                    bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                    channelMask: sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT, // TODO
                    endianness: sles::SL_BYTEORDER_LITTLEENDIAN,                             // TODO
                };

                create_player(&mut format_source as *mut _ as _);
            }

            _ => unimplemented!(),
        }

        log::warn!(
            "realize: {}",
            ((**audio_player).Realize).unwrap()(audio_player, sles::SL_BOOLEAN_FALSE)
        );

        let mut queue: sles::bindings::SLAndroidSimpleBufferQueueItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(
            audio_player,
            sles::bindings::SL_IID_BUFFERQUEUE,
            &mut queue as *mut _ as _,
        );

        let mut state: sles::bindings::SLPlayItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(
            audio_player,
            sles::bindings::SL_IID_PLAY,
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

        extern "C" fn write_cb(
            _: sles::bindings::SLAndroidSimpleBufferQueueItf,
            user: *mut c_void,
        ) {
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

    pub unsafe fn properties(&self) -> DeviceProperties {
        DeviceProperties {
            num_channels: self.sample_desc.channels,
            channel_mask: ChannelMask::empty(), // TODO
            sample_rate: self.sample_desc.sample_rate,
            buffer_size: BUFFER_NUM_FRAMES,
        }
    }

    pub unsafe fn start(&mut self) {
        if let Some(ref state) = self.output_state {
            ((**state).SetPlayState).unwrap()(state, sles::SL_PLAYSTATE_PLAYING);
        }
        if let Some(ref state) = self.input_state {
            ((**state).SetPlayState).unwrap()(state, sles::SL_PLAYSTATE_PLAYING);
        }
    }

    pub unsafe fn stop(&mut self) {
        unimplemented!()
    }
}

pub struct OutputStream {
    mix: sles::bindings::SLObjectItf,
    audio_player: sles::bindings::SLObjectItf,
    queue: sles::bindings::SLAndroidSimpleBufferQueueItf,

    pair: Arc<(Mutex<bool>, Condvar)>,
    buffers: Vec<Vec<u32>>, // TODO: alignment
    cur_buffer: usize,
}

impl OutputStream {
    pub unsafe fn acquire_buffer(&mut self, timeout_ms: u32) -> (*mut u8, Frames) {
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

    pub unsafe fn submit_buffer(&mut self, num_frames: usize) {
        let buffer = &mut self.buffers[self.cur_buffer];
        ((**self.queue).Enqueue).unwrap()(
            self.queue,
            buffer.as_mut_ptr() as _,
            buffer.len() as u32 * 4,
        ); // TODO: sizeof u32, num_frames
    }
}
