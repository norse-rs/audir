use crate::{
    DeviceProperties, DriverId, Format, PhysicalDeviceProperties, SampleDesc, SharingModeFlags,
};
use libpulse_sys as pulse;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ptr;

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
    let physical_devices = unsafe { &mut *(user as *mut Vec<PhysicalDevice>) };

    physical_devices.push(PhysicalDevice {
        name: unsafe {
            CStr::from_ptr(info.description)
                .to_string_lossy()
                .into_owned()
        },
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
    let physical_devices = unsafe { &mut *(user as *mut Vec<PhysicalDevice>) };
    physical_devices.push(PhysicalDevice {
        name: unsafe {
            CStr::from_ptr(info.description)
                .to_string_lossy()
                .into_owned()
        },
    });
}

pub struct PhysicalDevice {
    name: String,
}

impl PhysicalDevice {
    pub unsafe fn get_properties(&self) -> PhysicalDeviceProperties {
        PhysicalDeviceProperties {
            device_name: self.name.clone(),
            driver_id: DriverId::PulseAudio,
            sharing: SharingModeFlags::CONCURRENT,
        }
    }
}

fn map_format(format: Format) -> pulse::pa_sample_format_t {
    match format {
        Format::I16 => pulse::pa_sample_format_t::S16le,
        Format::F32 => pulse::pa_sample_format_t::F32le,
    }
}

pub struct Instance {
    pub mainloop: *mut pulse::pa_mainloop,
    pub context: *mut pulse::pa_context,
}

impl Instance {
    pub unsafe fn create(name: &str) -> Self {
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

        Instance { mainloop, context }
    }

    unsafe fn await_operation(&self, operation: *mut pulse::pa_operation) {
        loop {
            let state = pulse::pa_operation_get_state(operation);
            if state != pulse::PA_OPERATION_RUNNING {
                pulse::pa_operation_unref(operation);
                break;
            }
            pulse::pa_mainloop_iterate(self.mainloop, true as _, ptr::null_mut());
        }
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        let mut physical_devices = Vec::new();
        let operation = pulse::pa_context_get_sink_info_list(
            self.context,
            Some(sink_info_cb),
            &mut physical_devices as *mut _ as _,
        );
        self.await_operation(operation);
        physical_devices
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        let mut physical_devices = Vec::new();
        let operation = pulse::pa_context_get_source_info_list(
            self.context,
            Some(source_info_cb),
            &mut physical_devices as *mut _ as _,
        );
        self.await_operation(operation);
        physical_devices
    }

    pub unsafe fn create_device(
        &self,
        physical_device: &PhysicalDevice,
        sample_desc: SampleDesc,
    ) -> Device {
        let spec = pulse::pa_sample_spec {
            format: map_format(sample_desc.format),
            channels: sample_desc.channels as _,
            rate: sample_desc.sample_rate as _,
        };
        let stream = dbg!(pulse::pa_stream_new(
            self.context,
            b"audir\0".as_ptr() as _,
            &spec,
            ptr::null()
        )); // TODO: name, channel map

        let attribs = pulse::pa_buffer_attr {
            maxlength: !0,
            tlength: !0,
            prebuf: !0,
            minreq: !0,
            fragsize: !0,
        };

        pulse::pa_stream_connect_playback(
            stream,
            ptr::null(),
            &attribs,
            0,
            ptr::null(),
            ptr::null_mut(),
        );
        loop {
            let state = pulse::pa_stream_get_state(stream);
            if state == pulse::PA_STREAM_READY {
                break;
            }
            pulse::pa_mainloop_iterate(self.mainloop, true as _, ptr::null_mut());
        }

        Device {
            mainloop: self.mainloop,
            stream,
        }
    }
}

pub struct Device {
    pub mainloop: *mut pulse::pa_mainloop,
    pub stream: *mut pulse::pa_stream,
}

impl Device {
    pub unsafe fn get_output_stream(&self) -> OutputStream {
        OutputStream {
            mainloop: self.mainloop,
            stream: self.stream,
            cur_buffer: ptr::null_mut(),
        }
    }

    pub unsafe fn properties(&self) -> DeviceProperties {
        let buffer_attrs = unsafe { &*pulse::pa_stream_get_buffer_attr(self.stream) };
        dbg!((
            buffer_attrs.minreq,
            buffer_attrs.maxlength,
            buffer_attrs.tlength
        ));

        DeviceProperties {
            channels: Vec::new(),
            buffer_size: buffer_attrs.minreq,
        }
    }
}

pub struct OutputStream {
    pub mainloop: *mut pulse::pa_mainloop,
    pub stream: *mut pulse::pa_stream,
    pub cur_buffer: *mut c_void,
}

impl OutputStream {
    pub unsafe fn get_writeable_size(&self) -> u32 {
        unimplemented!()
    }

    pub unsafe fn acquire_buffer(&mut self, len: u32, timeout_ms: u32) -> *mut u8 {
        loop {
            // TODO: timeout
            let size = pulse::pa_stream_writable_size(self.stream);
            if size >= len as usize {
                break;
            }
            pulse::pa_mainloop_iterate(self.mainloop, false as _, ptr::null_mut());
        }

        let mut data = ptr::null_mut();
        let mut size = len as usize;
        pulse::pa_stream_begin_write(self.stream, &mut data, &mut size);
        assert_eq!(size, len as usize);
        self.cur_buffer = data;

        data as _
    }

    pub unsafe fn submit_buffer(&self, len: u32) {
        pulse::pa_stream_write(
            self.stream,
            self.cur_buffer,
            len as _,
            None,
            0,
            pulse::PA_SEEK_RELATIVE,
        );
        pulse::pa_mainloop_iterate(self.mainloop, false as _, ptr::null_mut());
    }
}
