use crate::{api, api::Result};

pub struct Instance {
}

impl api::Instance for Instance {
    type Device = Device;

    unsafe fn properties() -> api::InstanceProperties { todo!() }

    unsafe fn create(name: &str) -> Self {
        Instance {

        }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        todo!()
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> { todo!() }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> { todo!() }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> { todo!() }

    unsafe fn physical_device_default_input_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> { todo!() }

    unsafe fn physical_device_default_output_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> { todo!() }

    unsafe fn create_device(&self, desc: api::DeviceDesc, channels: api::Channels) -> Result<Device> { todo!() }

    unsafe fn destroy_device(&self, device: &mut Device) { todo!() }

    unsafe fn poll_events<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(api::Event) { todo!() }
}

pub struct Device { }

impl api::Device for Device {
    unsafe fn start(&self) { todo!() }
    unsafe fn stop(&self) { todo!() }

    unsafe fn acquire_buffers(&mut self, _timeout_ms: u32) -> Result<api::StreamBuffers> {
        todo!()
    }
    unsafe fn release_buffers(&mut self, _num_frames: api::Frames) -> Result<()> {
        todo!()
    }
}
