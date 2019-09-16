use crate::{DeviceProperties, SampleDesc};
use coreaudio_sys as ca;
use std::ptr;

unsafe fn get_property_data(
    id: ca::AudioObjectID,
    address: &ca::AudioObjectPropertyAddress,
    data: *mut (),
    len: u32,
) -> u32 {
    let mut size = len;
    let status = ca::AudioObjectGetPropertyData(
        id,
        address,
        0,
        ptr::null_mut(),
        &mut size,
        data as _,
    );
    assert_eq!(status, 0);

    size
}

unsafe fn get_property_data_size(
    id: ca::AudioObjectID,
    address: &ca::AudioObjectPropertyAddress,
) -> u32 {
    let mut size = 0;
    let status = ca::AudioObjectGetPropertyDataSize(
        id,
        address,
        0,
        ptr::null_mut(),
        &mut size
    );
    assert_eq!(status, 0);

    size
}

pub struct Instance {}

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        Instance { }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<ca::AudioObjectID> {
        let address = ca::AudioObjectPropertyAddress {
            mElement: ca::kAudioObjectPropertyElementMaster,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mSelector: ca::kAudioHardwarePropertyDevices,
        };

        let id = ca::kAudioObjectSystemObject;
        let size = get_property_data_size(id, &address);
        let num_elements = size / mem::size_of::<ca::AudioObjectID>();
        let mut data = Vec::with_capacity(num_elements);
        data.set_len(num_elements);
        get_property_data(id, &address, data.as_mut_ptr() as _, size);

        data
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        let audio_objects = self.enumerate_physical_devices();

        unimplemented!()
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        let audio_objects = self.enumerate_physical_devices();

        unimplemented!()
    }

    pub unsafe fn create_device(
        &self,
        physical_device: &PhysicalDevice,
        sample_desc: SampleDesc,
    ) -> Device {
        Device
    }

    pub unsafe fn properties(&self) -> DeviceProperties {
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
