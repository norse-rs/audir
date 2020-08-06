use crate::{api, api::Result, handle::Handle};
use libpulse_sys as pulse;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ptr;

struct PhysicalDevice {
    device_name: String,
    dev: *const i8,
    streams: api::StreamFlags,
    sample_spec: pulse::pa_sample_spec,
    channels: api::ChannelMask,
}

type PhysicalDeviceMap = HashMap<String, Handle<PhysicalDevice>>;

impl PhysicalDevice {
    // TOOD: as extension?
    fn default_format(&self) -> Result<api::FrameDesc> {
        let format = match self.sample_spec.format {
            pulse::pa_sample_format_t::F32le => api::Format::F32,
            pulse::pa_sample_format_t::S16le => api::Format::I16,
            format => {
                return Err(api::Error::Internal {
                    cause: format!("unhandled format: {:?}", format),
                })
            }
        };

        Ok(api::FrameDesc {
            format,
            channels: self.channels,
            sample_rate: self.sample_spec.rate as _,
        })
    }
}

fn map_channels(channel_map: &pulse::pa_channel_map) -> api::ChannelMask {
    let mut channels = api::ChannelMask::empty();
    for i in 0..channel_map.channels {
        channels |= match channel_map.map[i as usize] {
            pulse::PA_CHANNEL_POSITION_FRONT_LEFT => api::ChannelMask::FRONT_LEFT,
            pulse::PA_CHANNEL_POSITION_FRONT_RIGHT => api::ChannelMask::FRONT_RIGHT,
            pos => panic!("unsupported {:?}", pos),
        };
    }
    channels
}

extern "C" fn sink_info_cb(
    context: *mut pulse::pa_context,
    info: *const pulse::pa_sink_info,
    _: i32,
    user: *mut c_void,
) {
    if info.is_null() {
        return;
    }

    let info = unsafe { &*info };
    let physical_devices = unsafe { &mut *(user as *mut PhysicalDeviceMap) };

    let name = unsafe { CStr::from_ptr(info.name).to_string_lossy().into_owned() };
    let device_name = unsafe {
        CStr::from_ptr(info.description)
            .to_string_lossy()
            .into_owned()
    };
    physical_devices
        .entry(name)
        .and_modify(|device| {
            assert_eq!(device.sample_spec, info.sample_spec); // TODO: is this right?

            device.streams |= api::StreamFlags::OUTPUT;
        })
        .or_insert_with(|| {
            Handle::new(PhysicalDevice {
                device_name,
                dev: info.name,
                streams: api::StreamFlags::OUTPUT,
                sample_spec: info.sample_spec,
                channels: map_channels(&info.channel_map),
            })
        });
}

extern "C" fn source_info_cb(
    context: *mut pulse::pa_context,
    info: *const pulse::pa_source_info,
    _: i32,
    user: *mut c_void,
) {
    if info.is_null() {
        return;
    }

    let info = unsafe { &*info };
    let physical_devices = unsafe { &mut *(user as *mut PhysicalDeviceMap) };

    let name = unsafe {
        CStr::from_ptr(info.description)
            .to_string_lossy()
            .into_owned()
    };
    let device_name = unsafe {
        CStr::from_ptr(info.description)
            .to_string_lossy()
            .into_owned()
    };
    physical_devices
        .entry(name)
        .and_modify(|device| {
            assert_eq!(device.sample_spec, info.sample_spec); // TODO: is this right?

            device.streams |= api::StreamFlags::INPUT;
        })
        .or_insert_with(|| {
            Handle::new(PhysicalDevice {
                device_name,
                dev: info.name,
                streams: api::StreamFlags::INPUT,
                sample_spec: info.sample_spec,
                channels: map_channels(&info.channel_map),
            })
        });
}

fn map_format(format: api::Format) -> pulse::pa_sample_format_t {
    match format {
        api::Format::I16 => pulse::pa_sample_format_t::S16le,
        api::Format::F32 => pulse::pa_sample_format_t::F32le,
        _ => unimplemented!(),
    }
}

pub struct Instance {
    mainloop: *mut pulse::pa_mainloop,
    context: *mut pulse::pa_context,
    physical_devices: PhysicalDeviceMap,
}

impl api::Instance for Instance {
    type Device = Device;
    type Stream = Stream;
    type Session = (); // TODO

    unsafe fn properties() -> api::InstanceProperties {
        api::InstanceProperties {
            driver_id: api::DriverId::PulseAudio,
            stream_mode: api::StreamMode::Polling,
            sharing: api::SharingModeFlags::CONCURRENT,
        }
    }

    unsafe fn create(name: &str) -> Self {
        let name = std::ffi::CString::new(name).unwrap();
        let mainloop = pulse::pa_mainloop_new();
        let api = pulse::pa_mainloop_get_api(mainloop);
        let context = pulse::pa_context_new(api, name.as_ptr() as *const _);
        pulse::pa_context_connect(context, ptr::null(), 0, ptr::null());

        loop {
            pulse::pa_mainloop_iterate(mainloop, true as _, ptr::null_mut());
            let state = pulse::pa_context_get_state(context);
            if state == pulse::PA_CONTEXT_READY {
                break;
            }
        }

        let mut physical_devices = PhysicalDeviceMap::new();

        // input devices
        let operation = pulse::pa_context_get_sink_info_list(
            context,
            Some(sink_info_cb),
            &mut physical_devices as *mut _ as _,
        );
        Self::await_operation(mainloop, operation);

        // output devices
        let operation = pulse::pa_context_get_source_info_list(
            context,
            Some(source_info_cb),
            &mut physical_devices as *mut _ as _,
        );
        Self::await_operation(mainloop, operation);

        Instance {
            mainloop,
            context,
            physical_devices,
        }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        self.physical_devices
            .values()
            .map(|device| device.raw())
            .collect()
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        self.physical_devices
            .get("default")
            .filter(|device| device.streams.contains(api::StreamFlags::INPUT))
            .map(|device| device.raw())
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        self.physical_devices
            .get("default")
            .filter(|device| device.streams.contains(api::StreamFlags::OUTPUT))
            .map(|device| device.raw())
    }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);

        Ok(api::PhysicalDeviceProperties {
            device_name: physical_device.device_name.clone(),
            streams: physical_device.streams,
        })
    }

    unsafe fn physical_device_supports_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
        frame_desc: api::FrameDesc,
    ) -> bool {
        if sharing == api::SharingMode::Exclusive {
            // concurrent only
            return false;
        }

        // TODO: supporting everything?
        return true;
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
        callback: api::StreamCallback<Stream>,
    ) -> Result<Self::Device> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(desc.physical_device);

        let stream = if !channels.output.is_empty() {
            let spec = pulse::pa_sample_spec {
                format: map_format(desc.sample_desc.format),
                channels: channels.output.bits().count_ones() as _,
                rate: desc.sample_desc.sample_rate as _,
            };

            let stream = dbg!(pulse::pa_stream_new(
                self.context,
                b"audir\0".as_ptr() as _,
                &spec,
                ptr::null()
            )); // TODO: name, channel map

            // TODO
            let attribs = pulse::pa_buffer_attr {
                maxlength: !0,
                tlength: !0,
                prebuf: !0,
                minreq: !0,
                fragsize: !0,
            };

            dbg!(pulse::pa_stream_connect_playback(
                stream,
                ptr::null(),
                &attribs,
                0,
                ptr::null(),
                ptr::null_mut(),
            ));

            loop {
                let state = dbg!(pulse::pa_stream_get_state(stream));
                if state == pulse::PA_STREAM_READY {
                    break;
                }
                pulse::pa_mainloop_iterate(self.mainloop, true as _, ptr::null_mut());
            }

            stream
        } else {
            todo!()
        };

        let sample_spec = &*pulse::pa_stream_get_sample_spec(stream);
        let frame_size = pulse::pa_frame_size(sample_spec);

        Ok(Device {
            mainloop: self.mainloop,
            stream,
            cur_buffer: ptr::null_mut(),
            frame_size,
            callback,
        })
    }

    unsafe fn create_session(&self, sample_rate: usize) -> Result<Self::Session> {
        Ok(())
    }

    unsafe fn set_event_callback<F>(&mut self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(api::Event) + Send + 'static
    {
        unimplemented!()
    }
}

impl Instance {
    unsafe fn await_operation(
        mainloop: *mut pulse::pa_mainloop,
        operation: *mut pulse::pa_operation,
    ) {
        loop {
            let state = pulse::pa_operation_get_state(operation);
            if state != pulse::PA_OPERATION_RUNNING {
                pulse::pa_operation_unref(operation);
                break;
            }
            pulse::pa_mainloop_iterate(mainloop, true as _, ptr::null_mut());
        }
    }
}

pub struct Device {
    mainloop: *mut pulse::pa_mainloop,
    stream: *mut pulse::pa_stream,
    cur_buffer: *mut c_void,
    frame_size: usize,
    callback: api::StreamCallback<Stream>,
}

impl Device {
    unsafe fn acquire_buffers(&mut self, timeout_ms: u32) -> Result<api::StreamBuffers> {
        let mut size = loop {
            let size = pulse::pa_stream_writable_size(self.stream);
            if size > 0 {
                break size;
            }

            pulse::pa_mainloop_prepare(self.mainloop, timeout_ms as _); // TODO: timeout
            pulse::pa_mainloop_poll(self.mainloop);
            pulse::pa_mainloop_dispatch(self.mainloop);
        };

        let mut data = ptr::null_mut();
        pulse::pa_stream_begin_write(self.stream, &mut data, &mut size);
        self.cur_buffer = data;
        Ok(api::StreamBuffers {
            input: ptr::null(),
            output: data as _,
            frames: (size / self.frame_size) as _,
        })
    }

    unsafe fn release_buffers(&mut self, num_frames: api::Frames) -> Result<()> {
        pulse::pa_stream_write(
            self.stream,
            self.cur_buffer,
            num_frames * self.frame_size,
            None,
            0,
            pulse::PA_SEEK_RELATIVE,
        );
        Ok(())
    }
}

impl api::Device for Device {
    unsafe fn start(&self) {
        println!("Device::start unimplemented");
    }

    unsafe fn stop(&self) {
        println!("Device::stop unimplemented");
    }

    unsafe fn submit_buffers(&mut self, timeout_ms: u32) -> Result<()> {
        let buffers = self.acquire_buffers(timeout_ms)?;
        (self.callback)(&Stream(self.stream), buffers);
        self.release_buffers(buffers.frames)
    }
}

pub struct Stream(*mut pulse::pa_stream);

impl api::Stream for Stream {
    unsafe fn properties(&self) -> api::StreamProperties {
        let stream = self.0;

        let buffer_attrs = &*pulse::pa_stream_get_buffer_attr(stream);
        let sample_spec = &*pulse::pa_stream_get_sample_spec(stream);
        let channel_map = &*pulse::pa_stream_get_channel_map(stream);

        api::StreamProperties {
            channels: map_channels(channel_map),
            sample_rate: sample_spec.rate as _,
            buffer_size: buffer_attrs.minreq as _,
        }
    }
}
