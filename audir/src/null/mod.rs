use crate::{api, api::Result};

const NULL_DEVICE: api::PhysicalDevice = 0;
pub struct Instance;

impl api::Instance for Instance {
    type Device = Device;
    type Session = ();

    unsafe fn properties() -> api::InstanceProperties {
        api::InstanceProperties {
            driver_id: api::DriverId::Null,
            stream_mode: api::StreamMode::Callback,
            sharing: api::SharingModeFlags::all(),
        }
    }

    unsafe fn create(_: &str) -> Self {
        Instance
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        vec![NULL_DEVICE]
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        Some(NULL_DEVICE)
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        Some(NULL_DEVICE)
    }

    unsafe fn physical_device_properties(
        &self,
        _: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        Ok(api::PhysicalDeviceProperties {
            device_name: "null".into(),
            streams: api::StreamFlags::all(),
            form_factor: api::FormFactor::Unknown,
        })
    }

    unsafe fn physical_device_supports_format(
        &self,
        _: api::PhysicalDevice,
        _: api::SharingMode,
        _: api::FrameDesc,
    ) -> bool {
        true
    }

    unsafe fn physical_device_default_concurrent_format(
        &self,
        _: api::PhysicalDevice,
    ) -> Result<api::FrameDesc> {
        Ok(api::FrameDesc {
            format: api::Format::F32,
            sample_rate: 0,
            channels: api::ChannelMask::empty(),
        })
    }

    unsafe fn create_device(
        &self,
        _: api::DeviceDesc,
        _: api::Channels,
        _: api::StreamCallback,
    ) -> Result<Self::Device> {
        Ok(Device)
    }

    unsafe fn create_session(&self, _sample_rate: usize) -> Result<Self::Session> {
        Ok(())
    }

    unsafe fn set_event_callback<F>(&mut self, _callback: Option<F>) -> Result<()>
    where
        F: FnMut(api::Event) + Send + 'static,
    {
        Ok(())
    }
}

pub struct Device;

impl api::Device for Device {
    unsafe fn start(&self) { }

    unsafe fn stop(&self) { }

    unsafe fn stream_properties(&self) -> api::StreamProperties {
        api::StreamProperties {
            channels: api::ChannelMask::empty(),
            sample_rate: 0,
            buffer_size: 0,
        }
    }

    unsafe fn submit_buffers(&mut self, _: u32) -> api::Result<()> {
        Ok(())
    }
}