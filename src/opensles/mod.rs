use opensles as sles;
use std::ptr;
use crate::{SampleDesc, Frames};
use std::os::raw::c_void;
use std::sync::Arc;
use std::sync::{Mutex, Condvar};

pub struct Instance {
    instance: sles::bindings::SLObjectItf,
    engine: sles::bindings::SLEngineItf,
}

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        let mut instance = ptr::null();
        sles::bindings::slCreateEngine(&mut instance, 0, ptr::null(), 0, ptr::null(), ptr::null_mut());
        ((**instance).Realize).unwrap()(instance, sles::SL_BOOLEAN_FALSE);

        let mut engine = ptr::null();
        ((**instance).GetInterface).unwrap()(instance, sles::bindings::SL_IID_ENGINE, &mut engine as *mut _ as _);

        Instance {
            instance,
            engine,
        }
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
        _sample_desc: SampleDesc,
    ) -> Device {
        Device { engine: self.engine }
    }
}

pub enum PhysicalDevice {
    Input,
    Output,
}

pub struct Device {
    engine: sles::bindings::SLEngineItf,
}

impl Device {
    pub unsafe fn output_stream(&self) -> OutputStream {
        let mut mix = ptr::null();
        ((**self.engine).CreateOutputMix).unwrap()(self.engine, &mut mix, 0, ptr::null(), ptr::null());
        ((**mix).Realize).unwrap()(mix, sles::SL_BOOLEAN_FALSE);

        let mut audio_player = ptr::null();
        let mut locator_source = sles::bindings::SLDataLocator_AndroidSimpleBufferQueue {
            locatorType: 0x800007BD, // sles::bindings::SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE,
            numBuffers: 2, // TODO
        };
        // TODO: u32
        let mut format_source = sles::bindings::SLDataFormat_PCM {
            formatType: sles::SL_DATAFORMAT_PCM,
            numChannels: 2, // TODO
            samplesPerSec: sles::SL_SAMPLINGRATE_44_1, // TODO
            bitsPerSample: 32, // TODO
            containerSize: 32, // TODO
            channelMask: sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT, // TODO
            endianness: sles::SL_BYTEORDER_LITTLEENDIAN, // TODO
        };
        let mut source = sles::bindings::SLDataSource {
            pLocator: &mut locator_source as *mut _ as _,
            pFormat: &mut format_source as *mut _ as _,
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
        ((**self.engine).CreateAudioPlayer).unwrap()(self.engine, &mut audio_player, &mut source, &mut sink, 1, ids.as_ptr(), requirements.as_ptr());

        ((**audio_player).Realize).unwrap()(audio_player, sles::SL_BOOLEAN_FALSE);

        let mut queue: sles::bindings::SLAndroidSimpleBufferQueueItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(audio_player, sles::bindings::SL_IID_BUFFERQUEUE, &mut queue as *mut _ as _);

        let mut state: sles::bindings::SLPlayItf = ptr::null();
        ((**audio_player).GetInterface).unwrap()(audio_player, sles::bindings::SL_IID_PLAY, &mut state as *mut _ as _);


        let num_frames = 4096; // TODO: random
        let buffer_chain = 2; // TODO: ?

        let mut buffers = Vec::new();
        for i in 0..buffer_chain {
            let mut buffer = Vec::<u32>::with_capacity(2 * num_frames); // TODO: 2 == num channels
            buffer.set_len(2 * num_frames);
            buffers.push(buffer);
        }

        let pair = Arc::new((Mutex::new(true), Condvar::new()));

        extern "C" fn write_cb(queue: sles::bindings::SLAndroidSimpleBufferQueueItf, user: *mut c_void)
        {
            unsafe {
                let pair = Arc::from_raw(user as *mut (Mutex<bool>, Condvar));
                {
                    let &(ref lock, ref cvar) = &*pair;
                    let mut signal = lock.lock().unwrap();
                    *signal = true;
                    cvar.notify_all();
                }
                Arc::into_raw(pair);
            }
        }

        let cb_pair = Arc::into_raw(Arc::clone(&pair)); // TODO: clear
        ((**queue).RegisterCallback).unwrap()(queue, Some(write_cb), cb_pair as *mut _);

        {
            // TODO: remove out of here..
            ((**state).SetPlayState).unwrap()(state, sles::SL_PLAYSTATE_PLAYING);
        }

        OutputStream {
            mix,
            state,
            audio_player,
            pair,
            queue,

            buffers,
            cur_buffer: 0,
        }
    }
}

pub struct OutputStream {
    mix: sles::bindings::SLObjectItf,
    state: sles::bindings::SLPlayItf,
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
            *signal = false;
        }

        // cycle active buffers
        self.cur_buffer = (self.cur_buffer + 1) % self.buffers.len();

        let buffer = &mut self.buffers[self.cur_buffer];

        (buffer.as_mut_ptr() as _, buffer.len() / 2) // TODO: channels + sizeof u32
    }

    pub unsafe fn submit_buffer(&mut self, num_frames: usize) {
        let buffer = &mut self.buffers[self.cur_buffer];
        ((**self.queue).Enqueue).unwrap()(self.queue, buffer.as_mut_ptr() as _, buffer.len() as u32 * 4); // TODO: sizeof u32
    }
}