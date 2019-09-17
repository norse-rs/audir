pub mod com;

pub use winapi::shared::winerror::HRESULT;
pub type WasapiResult<T> = (T, HRESULT);

use com::WeakPtr;
use std::{ffi::OsString, mem, os::windows::ffi::OsStringExt, ptr, slice};
use winapi::shared::devpkey::*;
use winapi::shared::mmreg::*;
use winapi::um::audioclient::*;
use winapi::um::audiosessiontypes::*;
use winapi::um::combaseapi::*;
use winapi::um::coml2api::STGM_READ;
use winapi::um::mmdeviceapi::*;
use winapi::um::objbase::COINIT_MULTITHREADED;
use winapi::um::propsys::*;
use winapi::um::synchapi;
use winapi::um::winnt;
use winapi::Interface;

use crate::{
    ChannelMask, DeviceProperties, DriverId, Frames, PhysicalDeviceProperties, SampleDesc,
    SharingModeFlags,
};

pub type Instance = WeakPtr<IMMDeviceEnumerator>;

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);

        let mut instance = Instance::null();
        let _hr = CoCreateInstance(
            &CLSID_MMDeviceEnumerator,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IMMDeviceEnumerator::uuidof(),
            instance.mut_void(),
        );
        instance
    }

    unsafe fn enumerate_physical_devices(&self, ty: EDataFlow) -> Vec<PhysicalDevice> {
        type DeviceCollection = WeakPtr<IMMDeviceCollection>;

        let collection = {
            let mut collection = DeviceCollection::null();
            let _hr =
                self.EnumAudioEndpoints(ty, DEVICE_STATE_ACTIVE, collection.mut_void() as *mut _);
            collection
        };

        let num_items = {
            let mut num = 0;
            collection.GetCount(&mut num);
            num
        };

        (0..num_items)
            .map(|i| {
                let mut device = PhysicalDevice::null();
                collection.Item(i, device.mut_void() as *mut _);
                device
            })
            .collect()
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        self.enumerate_physical_devices(eCapture)
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        self.enumerate_physical_devices(eRender)
    }

    pub unsafe fn create_device(
        &self,
        physical_device: &PhysicalDevice,
        sample_desc: SampleDesc,
    ) -> Device {
        let mut audio_client = WeakPtr::<IAudioClient>::null();
        physical_device.Activate(
            &IAudioClient::uuidof(),
            CLSCTX_ALL,
            ptr::null_mut(),
            audio_client.mut_void() as *mut _,
        );
        let mut mix_format = ptr::null_mut();
        audio_client.GetMixFormat(&mut mix_format);
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            mix_format,
            ptr::null(),
        );

        let fence = Fence::create(false, false);
        audio_client.SetEventHandle(fence.0);

        Device {
            client: audio_client,
            fence,
        }
    }
}

pub type PhysicalDevice = WeakPtr<IMMDevice>;

impl PhysicalDevice {
    pub unsafe fn get_properties(&self) -> PhysicalDeviceProperties {
        type PropertyStore = WeakPtr<IPropertyStore>;

        let mut store = PropertyStore::null();
        self.OpenPropertyStore(STGM_READ, store.mut_void() as *mut _);

        let device_name = {
            let mut value = mem::uninitialized();
            store.GetValue(
                &DEVPKEY_Device_FriendlyName as *const _ as *const _,
                &mut value,
            );
            let os_str = *value.data.pwszVal();
            let mut len = 0;
            while *os_str.offset(len) != 0 {
                len += 1;
            }
            let name: OsString = OsStringExt::from_wide(slice::from_raw_parts(os_str, len as _));
            name.into_string().unwrap()
        };

        PhysicalDeviceProperties {
            device_name,
            driver_id: DriverId::Wasapi,
            sharing: SharingModeFlags::CONCURRENT | SharingModeFlags::EXCLUSIVE,
        }
    }
}

pub struct Device {
    client: WeakPtr<IAudioClient>,
    fence: Fence,
}

impl Device {
    pub unsafe fn output_stream(&self) -> OutputStream {
        let mut client = WeakPtr::<IAudioRenderClient>::null();
        self.client
            .GetService(&IAudioRenderClient::uuidof(), client.mut_void() as *mut _);

        let buffer_size = {
            let mut size = 0;
            self.client.GetBufferSize(&mut size);
            size
        };

        OutputStream {
            device: self.client,
            client,
            buffer_size,
            fence: self.fence,
        }
    }

    pub unsafe fn properties(&self) -> DeviceProperties {
        let buffer_size = {
            let mut size = 0;
            self.client.GetBufferSize(&mut size);
            size as _
        };

        let mut mix_format = ptr::null_mut();
        self.client.GetMixFormat(&mut mix_format);

        match (*mix_format).wFormatTag {
            WAVE_FORMAT_EXTENSIBLE => {
                let format = &*(mix_format as *const WAVEFORMATEXTENSIBLE);

                let mut channel_mask = ChannelMask::empty();
                if format.dwChannelMask & SPEAKER_FRONT_LEFT != 0 {
                    channel_mask |= ChannelMask::FRONT_LEFT;
                }
                if format.dwChannelMask & SPEAKER_FRONT_RIGHT != 0 {
                    channel_mask |= ChannelMask::FRONT_RIGHT;
                }
                if format.dwChannelMask & SPEAKER_FRONT_CENTER != 0 {
                    channel_mask |= ChannelMask::FRONT_CENTER;
                }
                // TODO: more channels

                DeviceProperties {
                    num_channels: format.Format.nChannels as _,
                    channel_mask,
                    sample_rate: format.Format.nSamplesPerSec as _,
                    buffer_size,
                }
            }
            _ => unimplemented!(),
        }
    }

    pub unsafe fn start(&self) {
        self.client.Start();
    }

    pub unsafe fn stop(&self) {
        self.client.Stop();
    }
}

pub struct OutputStream {
    device: WeakPtr<IAudioClient>,
    client: WeakPtr<IAudioRenderClient>,
    buffer_size: u32,
    fence: Fence,
}

impl OutputStream {
    pub unsafe fn acquire_buffer(&self, timeout_ms: u32) -> (*mut u8, Frames) {
        self.fence.wait(timeout_ms);

        let mut data = ptr::null_mut();
        let mut padding = 0;

        self.device.GetCurrentPadding(&mut padding);

        let len = self.buffer_size - padding;
        self.client.GetBuffer(len, &mut data);
        (data, len as _)
    }

    pub unsafe fn submit_buffer(&self, num_frames: Frames) {
        self.client.ReleaseBuffer(num_frames as _, 0);
    }
}

#[derive(Copy, Clone)]
struct Fence(pub winnt::HANDLE);
impl Fence {
    unsafe fn create(manual_reset: bool, initial_state: bool) -> Self {
        Fence(synchapi::CreateEventA(
            ptr::null_mut(),
            manual_reset as _,
            initial_state as _,
            ptr::null(),
        ))
    }

    unsafe fn wait(&self, timeout_ms: u32) -> u32 {
        synchapi::WaitForSingleObject(self.0, timeout_ms)
    }
}
