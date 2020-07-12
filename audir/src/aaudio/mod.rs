use crate::{api, api::Result};
use ndk::aaudio;
use std::ptr;

const DEFAULT_PHYSICAL_DEVICE: api::PhysicalDevice = ndk_sys::AAUDIO_UNSPECIFIED as _;

pub struct Instance {
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
        Instance {

        }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        todo!()
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        Some(DEFAULT_PHYSICAL_DEVICE)
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        Some(DEFAULT_PHYSICAL_DEVICE)
    }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> { todo!() }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
        mut callback: api::StreamCallback<Stream>,
    ) -> Result<Device> {
        let builder = aaudio::AAudioStreamBuilder::new().unwrap()
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
        todo!()
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