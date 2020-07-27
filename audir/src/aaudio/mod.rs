use crate::{api, api::Result};
use ndk::aaudio;
use std::ptr;
use std::collections::HashMap;
use std::sync::Mutex;

const DEFAULT_PHYSICAL_DEVICE: api::PhysicalDevice = ndk_sys::AAUDIO_UNSPECIFIED as _;

struct PhysicalDevice {
    device_name: String,
    streams: api::StreamFlags,
}
type DeviceId = i32;
type PhysicalDeviceMap = HashMap<DeviceId, PhysicalDevice>;
pub struct Instance {
    vm: jni::JavaVM,
    devices: Mutex<PhysicalDeviceMap>,
}

impl Instance {
    unsafe fn devices(env: &jni::AttachGuard) -> jni::sys::jobject {
        let class_ctxt = env.find_class("android/content/Context").unwrap();
        let audio_service = env
            .get_static_field(class_ctxt, "AUDIO_SERVICE", "Ljava/lang/String;")
            .unwrap();

        let audio_manager = env.call_method(
            ndk_glue::native_activity().activity(),
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[audio_service]
        )
        .unwrap()
        .l()
        .unwrap();

        let devices = env.call_method(
            audio_manager,
            "getDevices",
            "(I)[Landroid/media/AudioDeviceInfo;",
            &[3.into()] // GET_DEVICES_ALL
        )
        .unwrap();

        devices.l().unwrap().into_inner()
    }
}

impl api::Instance for Instance {
    type Device = Device;
    type Stream = Stream;
    type Session = ();

    unsafe fn properties() -> api::InstanceProperties {
        api::InstanceProperties {
            driver_id: api::DriverId::AAudio,
            stream_mode: api::StreamMode::Callback,
            sharing: api::SharingModeFlags::CONCURRENT | api::SharingModeFlags::EXCLUSIVE,
        }
    }

    unsafe fn create(name: &str) -> Self {
        let native_activity = ndk_glue::native_activity();
        let vm_ptr = native_activity.vm();
        let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) }.unwrap();

        let mut instance = Instance {
            vm,
            devices: Mutex::new(PhysicalDeviceMap::new()),
        };
        instance.enumerate_physical_devices(); // populate physical devices
        instance
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        let env = self.vm.attach_current_thread().unwrap();
        let mut physical_devices = Vec::new();

        let mut devices = self.devices.lock().unwrap();
        devices.clear();

        let device_array = Self::devices(&env);
        let len = env.get_array_length(device_array).unwrap();
        for i in 0..len {
            let device = env.get_object_array_element(device_array, i).unwrap();

            let ty = env.call_method(device, "getType", "()I", &[]).unwrap();
            let ty_desc = match ty.i().unwrap() {
                1 => "TYPE_BUILTIN_EARPIECE",
                2 => "TYPE_BUILTIN_SPEAKER",
                3 => "TYPE_WIRED_HEADSET",
                4 => "TYPE_WIRED_HEADPHONES",
                5 => "TYPE_LINE_ANALOG",
                6 => "TYPE_LINE_DIGITAL",
                7 => "TYPE_BLUETOOTH_SCO",
                8 => "TYPE_BLUETOOTH_A2DP",
                9 => "TYPE_HDMI",
                10 => "TYPE_HDMI_ARC",
                11 => "TYPE_USB_DEVICE",
                12 => "TYPE_USB_ACCESSORY",
                13 => "TYPE_DOCK",
                14 => "TYPE_FM",
                15 => "TYPE_BUILTIN_MIC",
                16 => "TYPE_FM_TUNER",
                17 => "TYPE_TV_TUNER",
                18 => "TYPE_TELEPHONY",
                19 => "TYPE_AUX_LINE",
                20 => "TYPE_IP",
                21 => "TYPE_BUS",
                22 => "TYPE_USB_HEADSET",
                23 => "TYPE_HEARING_AID",
                24 => "TYPE_BUILTIN_SPEAKER_SAFE",
                _ => "-",
            };

            // Device Name
            let name = env
                .call_method(device, "getProductName", "()Ljava/lang/CharSequence;", &[])
                .unwrap();
            let name = env
                .call_method(name.l().unwrap(), "toString", "()Ljava/lang/String;", &[])
                .unwrap();
            let device_name: String = env.get_string(name.l().unwrap().into()).unwrap().into();

            // Stream Flags
            let mut streams = api::StreamFlags::empty();
            if env.call_method(device, "isSink", "()Z", &[]).unwrap().z().unwrap() {
                streams |= api::StreamFlags::OUTPUT;
            }
            if env.call_method(device, "isSource", "()Z", &[]).unwrap().z().unwrap() {
                streams |= api::StreamFlags::INPUT;
            }

            let id = env.call_method(device, "getId", "()I", &[]).unwrap().i().unwrap();
            physical_devices.push(id as _);
            devices.insert(id, PhysicalDevice {
                device_name,
                streams,
            });
        }

        physical_devices
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        let mut builder = ndk::aaudio::AAudioStreamBuilder::new().unwrap();
        builder = builder.direction(ndk::aaudio::AAudioDirection::Input);
        match builder.open_stream() {
            Ok(stream) => {
                let device_id = stream.get_device_id();
                Some(device_id as _)
            }
            Err(_) => None,
        }
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        let mut builder = ndk::aaudio::AAudioStreamBuilder::new().unwrap();
        builder = builder.direction(ndk::aaudio::AAudioDirection::Output);
        match builder.open_stream() {
            Ok(stream) => {
                let device_id = dbg!(stream.get_device_id());
                Some(device_id as _)
            }
            Err(_) => None,
        }
    }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        let devices = self.devices.lock().unwrap();
        let device = &devices[&(physical_device as i32)]; // TODO: check

        Ok(
            api::PhysicalDeviceProperties {
                device_name: device.device_name.clone(),
                streams: device.streams,
            }
        )
    }

    unsafe fn physical_device_supports_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
        frame_desc: api::FrameDesc,
    ) -> bool {
        // TODO: channels, sample rate?
        let format = frame_desc.format;
        format == api::Format::F32 || format == api::Format::I16
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
        mut callback: api::StreamCallback<Stream>,
    ) -> Result<Device> {
        let builder = aaudio::AAudioStreamBuilder::new().unwrap()
            .device_id(desc.physical_device as _)
            .data_callback(Box::new(move |astream, data, frames| {
                let num_channels = astream.get_channel_count();
                let channels = if num_channels == 2 {
                    api::ChannelMask::FRONT_LEFT | api::ChannelMask::FRONT_RIGHT
                } else {
                    unimplemented!()
                };
                let stream = Stream {
                    properties: api::StreamProperties {
                        channels,
                        sample_rate: astream.get_sample_rate() as _,
                        buffer_size: astream.get_buffer_size_in_frames() as _,
                    },
                };

                callback(&stream, api::StreamBuffers { frames: frames as _, input: ptr::null(), output: data as *mut _ });
                aaudio::AAudioCallbackResult::Continue
            }));
        let stream = builder.open_stream().unwrap();
        Ok(Device {
            stream,
        })
    }

    unsafe fn create_session(&self, _: usize) -> Result<()> {
        Ok(())
    }

    unsafe fn poll_events<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(api::Event) { todo!() }
}

pub struct Device {
    stream: aaudio::AAudioStream,
}

impl api::Device for Device {
    unsafe fn start(&self) {
        self.stream.request_start().unwrap();
    }
    unsafe fn stop(&self) {
        self.stream.request_stop().unwrap();
    }

    unsafe fn submit_buffers(&mut self, _timeout_ms: u32) -> Result<()> {
        Err(api::Error::Validation)
    }
}

pub struct Stream {
    properties: api::StreamProperties,
}

impl api::Stream for Stream {
    unsafe fn properties(&self) -> api::StreamProperties {
        self.properties.clone()
    }
}