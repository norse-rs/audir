use crate::{api, api::Result};
use std::os::raw::c_void;
use std::ptr;
use audir_sles as sles;

const BUFFER_NUM_FRAMES: usize = 1024; // TODO: random
const BUFFER_CHAIN_SIZE: usize = 3; // TOdo

const DEFAULT_PHYSICAL_DEVICE: api::PhysicalDevice = 0;

struct CallbackData {
    buffers: Vec<Vec<u32>>,
    cur_buffer: usize,
    callback: api::OutputCallback,
}

pub struct Instance {
    instance: sles::SLObjectItf,
    engine: sles::SLEngineItf,
}

impl api::Instance for Instance {
    type Device = Device;

    unsafe fn properties() -> api::InstanceProperties {
        api::InstanceProperties {
            driver_id: api::DriverId::OpenSLES,
            stream_mode: api::StreamMode::Callback,
            sharing: api::SharingModeFlags::CONCURRENT,
        }
    }

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

    unsafe fn physical_device_default_input_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::SampleDesc> {
        unimplemented!()
    }

    unsafe fn physical_device_default_output_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::SampleDesc> {
        unimplemented!()
    }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        assert_eq!(physical_device, DEFAULT_PHYSICAL_DEVICE);

        Ok(api::PhysicalDeviceProperties {
            device_name: "default".into(),
            streams: api::StreamFlags::INPUT | api::StreamFlags::OUTPUT,
        })
    }

    unsafe fn destroy_device(&self, device: &mut Device) {
        unimplemented!()
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        input_desc: Option<api::SampleDesc>,
        output_desc: Option<api::SampleDesc>,
    ) -> Result<Self::Device> {
        assert_eq!(desc.physical_device, DEFAULT_PHYSICAL_DEVICE);
        assert_eq!(desc.sharing, api::SharingMode::Concurrent);

        let output_stream = output_desc.map(|sample_desc| {
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
                log::warn!("{}", ((**self.engine).CreateAudioPlayer).unwrap()(
                    self.engine,
                    &mut audio_player,
                    &mut source,
                    &mut sink,
                    1,
                    ids.as_ptr(),
                    requirements.as_ptr() as _,
                ));
            };

            match sample_desc.format {
                api::Format::F32 => {
                    let mut format_source = sles::SLAndroidDataFormat_PCM_EX {
                        formatType: sles::SL_ANDROID_DATAFORMAT_PCM_EX as _,
                        numChannels: sample_desc.channels as _,
                        sampleRate: (sample_desc.sample_rate * 1000) as _,
                        bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                        containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                        channelMask: (sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT)
                            as _, // TODO
                        endianness: sles::SL_BYTEORDER_LITTLEENDIAN as _, // TODO
                        representation: sles::SL_ANDROID_PCM_REPRESENTATION_FLOAT as _,
                    };

                    create_player(&mut format_source as *mut _ as _);
                }
                api::Format::U32 => {
                    let mut format_source = sles::SLDataFormat_PCM {
                        formatType: sles::SL_DATAFORMAT_PCM as _,
                        numChannels: sample_desc.channels as _,
                        samplesPerSec: (sample_desc.sample_rate * 1000) as _,
                        bitsPerSample: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                        containerSize: sles::SL_PCMSAMPLEFORMAT_FIXED_32 as _,
                        channelMask: (sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT)
                            as _, // TODO
                        endianness: sles::SL_BYTEORDER_LITTLEENDIAN as _, // TODO
                    };

                    create_player(&mut format_source as *mut _ as _);
                }

                _ => unimplemented!(),
            }

            ((**audio_player).Realize).unwrap()(audio_player, sles::SL_BOOLEAN_FALSE as _);

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

            (OutputStream {
                queue,
                sample_desc,
            }, state)
        });

        Ok(Device {
            engine: self.engine,
            output_stream,
            input_stream: None,
            input_state: None,
        })
    }

    unsafe fn poll_events<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(api::Event),
    {
        Ok(())
    }
}

pub struct Device {
    engine: sles::SLEngineItf,
    output_stream: Option<(OutputStream, sles::SLPlayItf)>,
    input_stream: Option<InputStream>,
    input_state: Option<sles::SLPlayItf>,
}

impl api::Device for Device {
    type OutputStream = OutputStream;
    type InputStream = InputStream;

    unsafe fn get_output_stream(&self) -> Result<Self::OutputStream> {
        match self.output_stream {
            Some((stream, _)) => Ok(stream),
            None => Err(api::Error::Validation),
        }
    }

    unsafe fn get_input_stream(&self) -> Result<Self::InputStream> {
        unimplemented!()
    }

    unsafe fn start(&self) {
        if let Some((_, ref state)) = self.output_stream {
            log::warn!("start {}", ((***state).SetPlayState).unwrap()(*state, sles::SL_PLAYSTATE_PLAYING as _));
        }
        if let Some(ref state) = self.input_state {
            ((***state).SetPlayState).unwrap()(*state, sles::SL_PLAYSTATE_PLAYING as _);
        }
    }

    unsafe fn stop(&self) {
        unimplemented!()
    }
}

#[derive(Copy, Clone)]
pub struct OutputStream {
    queue: sles::SLAndroidSimpleBufferQueueItf,
    sample_desc: api::SampleDesc,
}

impl api::Stream for OutputStream {
    unsafe fn properties(&self) -> api::StreamProperties {
        api::StreamProperties {
            num_channels: self.sample_desc.channels,
            channel_mask: api::ChannelMask::empty(), // TODO
            sample_rate: self.sample_desc.sample_rate,
            buffer_size: BUFFER_NUM_FRAMES,
        }
    }
}

impl api::OutputStream for OutputStream {
    unsafe fn set_callback(&mut self, callback: api::OutputCallback) -> Result<()> {
        let buffers = (0..BUFFER_CHAIN_SIZE).map(|_| {
            let buffer_size = self.sample_desc.channels * BUFFER_NUM_FRAMES;
            let mut buffer = Vec::<u32>::with_capacity(buffer_size);
            buffer.set_len(buffer_size);
            buffer
        }).collect();

        let data = Box::new(CallbackData {
            buffers,
            cur_buffer: 0,
            callback,
        });
        let data = Box::into_raw(data); // TODO: destroy

        extern "C" fn write_cb(queue: sles::SLAndroidSimpleBufferQueueItf, user: *mut c_void) {
            unsafe {
                let data = unsafe { &mut *(user as *mut CallbackData) };
                data.cur_buffer = (data.cur_buffer + 1) % data.buffers.len();
                let buffer = &mut data.buffers[data.cur_buffer];

                (data.callback)(buffer.as_mut_ptr() as _, buffer.len() / 2); // TODO: channels + sizeof u32
                ((**queue).Enqueue).unwrap()(
                    queue,
                    buffer.as_mut_ptr() as _,
                    (buffer.len() * 4) as _,
                );
            }
        }

        (**self.queue).RegisterCallback.unwrap()(self.queue, Some(write_cb), data as _);

        // Enqueue one frame to get the ball rolling
        write_cb(self.queue, data as _);

        Ok(())
    }

    unsafe fn acquire_buffer(&mut self, timeout_ms: u32) -> Result<(*mut (), api::Frames)> {
        Err(api::Error::Validation)
    }

    unsafe fn release_buffer(&mut self, num_frames: api::Frames) -> Result<()> {
        Err(api::Error::Validation)
    }
}

pub struct InputStream {}

impl api::Stream for InputStream {
    unsafe fn properties(&self) -> api::StreamProperties {
        unimplemented!()
    }
}

impl api::InputStream for InputStream {}
