use crate::{DeviceProperties, SampleDesc};

pub struct Instance {}

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        Instance { }
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        unimplemented!()
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        unimplemented!()
    }

    pub unsafe fn create_device(
        &self,
        physical_device: &PhysicalDevice,
        sample_desc: SampleDesc,
    ) -> Device {
        Device
    }

    pub unsafe properties(&self) -> DeviceProperties {
        unimplemented!()
    }
}

pub struct PhysicalDevice;

pub struct Device;

impl Device {
    pub unsafe fn output_stream<F>(&self, callback: Option<F>)
    where
        F: FnMut(*mut (), usize),
    {
        unimplemented!()
    }
}
