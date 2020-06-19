use crate::{api, api::Result, handle::Handle};
use libpulse_sys as pulse;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ptr;

pub struct PhysicalDevice {
    device_name: String,
    dev: *const i8,
    streams: api::StreamFlags,
    sample_spec: pulse::pa_sample_spec,
}

type PhysialDeviceMap = HashMap<String, Handle<PhysicalDevice>>;

impl PhysicalDevice {
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
            channels: self.sample_spec.channels as _,
            sample_rate: self.sample_spec.rate as _,
        })
    }
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
    let physical_devices = unsafe { &mut *(user as *mut PhysialDeviceMap) };

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
    let physical_devices = unsafe { &mut *(user as *mut PhysialDeviceMap) };

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
    physical_devices: PhysialDeviceMap,
}

impl api::Instance for Instance {
    type Device = Device;

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

        let mut physical_devices = PhysialDeviceMap::new();

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

    unsafe fn physical_device_default_input_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);
        physical_device.default_format()
    }

    unsafe fn physical_device_default_output_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);
        physical_device.default_format()
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
    ) -> Result<Self::Device> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(desc.physical_device);

        let input_stream = ptr::null_mut(); // TODO
        let output_stream = if channels.output > 0 {
            let spec = pulse::pa_sample_spec {
                format: map_format(desc.sample_desc.format),
                channels: channels.output as _,
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
            ptr::null_mut()
        };

        Ok(Device {
            mainloop: self.mainloop,
            input_stream,
            output_stream,
        })
    }

    unsafe fn destroy_device(&self, device: &mut Self::Device) {
        unimplemented!()
    }

    unsafe fn poll_events<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(api::Event),
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
    pub mainloop: *mut pulse::pa_mainloop,
    pub input_stream: *mut pulse::pa_stream,
    pub output_stream: *mut pulse::pa_stream,
}

impl api::Device for Device {
    type Stream = Stream;

    unsafe fn get_stream(&self) -> Result<Stream> {
        let stream = self.output_stream;
        if stream.is_null() {
            return Err(api::Error::Validation);
        }

        let sample_spec = &*pulse::pa_stream_get_sample_spec(stream);
        let frame_size = pulse::pa_frame_size(sample_spec);

        Ok(Stream {
            mainloop: self.mainloop,
            stream,
            cur_buffer: ptr::null_mut(),
            frame_size,
        })
    }

    unsafe fn start(&self) {
        println!("Device::start unimplemented");
    }

    unsafe fn stop(&self) {
        println!("Device::stop unimplemented");
    }
}

pub struct Stream {
    mainloop: *mut pulse::pa_mainloop,
    stream: *mut pulse::pa_stream,
    cur_buffer: *mut c_void,
    frame_size: usize,
}

impl api::Stream for Stream {
    unsafe fn properties(&self) -> api::StreamProperties {
        let buffer_attrs = &*pulse::pa_stream_get_buffer_attr(self.stream);
        dbg!((
            buffer_attrs.minreq,
            buffer_attrs.maxlength,
            buffer_attrs.tlength
        ));
        let sample_spec = &*pulse::pa_stream_get_sample_spec(self.stream);

        api::StreamProperties {
            num_channels: sample_spec.channels as _,
            channel_mask: api::ChannelMask::empty(), // TODO
            sample_rate: sample_spec.rate as _,
            buffer_size: buffer_attrs.minreq as _,
        }
    }

    unsafe fn set_callback(&mut self, _: api::StreamCallback) -> Result<()> {
        Err(api::Error::Validation)
    }

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
