pub mod com;

pub use winapi::shared::winerror::HRESULT;
pub type WasapiResult<T> = (T, HRESULT);

use com::WeakPtr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use winapi::shared::devpkey::*;
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

use crate::{DriverId, PhysicalDeviceProperties, SharingModeFlags};

pub type Instance = WeakPtr<IMMDeviceEnumerator>;

impl Instance {
    pub unsafe fn create(name: &str) -> Self {
        CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);

        let mut instance = Instance::null();
        let hr = CoCreateInstance(
            &CLSID_MMDeviceEnumerator,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IMMDeviceEnumerator::uuidof(),
            instance.mut_void(),
        );

        dbg!(hr);
        instance
    }

    unsafe fn enumerate_physical_devices(&self, ty: EDataFlow) -> Vec<PhysicalDevice> {
        type DeviceCollection = WeakPtr<IMMDeviceCollection>;

        let collection = {
            let mut collection = DeviceCollection::null();
            let hr =
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
                dbg!(device)
            })
            .collect()
    }

    pub unsafe fn enumerate_physical_input_devices(&self) -> Vec<PhysicalDevice> {
        self.enumerate_physical_devices(eCapture)
    }

    pub unsafe fn enumerate_physical_output_devices(&self) -> Vec<PhysicalDevice> {
        self.enumerate_physical_devices(eRender)
    }

    pub unsafe fn create_device(&self, physical_device: &PhysicalDevice) -> Device {
        let mut audio_client = WeakPtr::<IAudioClient>::null();
        dbg!(physical_device.Activate(
            &IAudioClient::uuidof(),
            CLSCTX_ALL,
            ptr::null_mut(),
            audio_client.mut_void() as *mut _
        ));
        let mut mix_format = ptr::null_mut();
        audio_client.GetMixFormat(&mut mix_format);
        dbg!(audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            mix_format,
            ptr::null()
        ));

        let fence = Fence::create(false, false);
        dbg!(audio_client.SetEventHandle(fence.0));

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
            let mut value = std::mem::uninitialized();
            store.GetValue(
                &DEVPKEY_Device_FriendlyName as *const _ as *const _,
                &mut value,
            );
            let utf16 = *(value.data.as_ptr() as *const usize) as *const u16;
            let mut len = 0;
            while *utf16.offset(len) != 0 {
                len += 1;
            }
            let name_os: OsString =
                OsStringExt::from_wide(std::slice::from_raw_parts(utf16, len as _));
            name_os.into_string().unwrap()
        };

        PhysicalDeviceProperties {
            device_name,
            driver_id: DriverId::Wasapi,
            sharing: SharingModeFlags::Concurrent | SharingModeFlags::Exclusive,
        }
    }
}

pub struct Device {
    client: WeakPtr<IAudioClient>,
    fence: Fence,
}

impl Device {
    pub unsafe fn get_output_stream(&self) -> OutputStream {
        let mut client = WeakPtr::<IAudioRenderClient>::null();
        dbg!(self
            .client
            .GetService(&IAudioRenderClient::uuidof(), client.mut_void() as *mut _));

        OutputStream {
            client,
            fence: self.fence,
        }
    }

    pub unsafe fn get_buffer_frames(&self) -> u32 {
        let mut size = 0;
        self.client.GetBufferSize(&mut size);
        size as _
    }

    pub unsafe fn get_current_padding(&self) -> u32 {
        let mut size = 0;
        self.client.GetCurrentPadding(&mut size);
        size as _
    }

    pub unsafe fn start(&self) {
        dbg!(self.client.Start());
    }
}

pub struct OutputStream {
    client: WeakPtr<IAudioRenderClient>,
    fence: Fence,
}

impl OutputStream {
    pub unsafe fn acquire_buffer(&self, len: u32, timeout_ms: u32) -> *mut u8 {
        self.fence.wait(timeout_ms);
        let mut data = ptr::null_mut();
        self.client.GetBuffer(len, &mut data);
        data
    }

    pub unsafe fn submit_buffer(&self, len: u32) {
        self.client.ReleaseBuffer(len, 0);
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
