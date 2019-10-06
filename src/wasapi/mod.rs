#![allow(non_upper_case_globals)]

pub mod com;

pub use winapi::shared::winerror::HRESULT;
pub type WasapiResult<T> = (T, HRESULT);

use com::WeakPtr;
use std::collections::HashMap;
use std::{ffi::OsString, mem, os::windows::ffi::OsStringExt, ptr, slice};
use winapi::shared::devpkey::*;
use winapi::shared::ksmedia;
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

use crate::api::{self, Result};

fn map_sample_desc(sample_desc: &api::SampleDesc) -> Option<WAVEFORMATEXTENSIBLE> {
    let (format_tag, sub_format, bytes_per_sample) = match sample_desc.format {
        api::Format::F32 => (
            WAVE_FORMAT_EXTENSIBLE,
            ksmedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
            4,
        ),
        api::Format::U32 => return None,
        _ => unimplemented!(),
    };

    let bits_per_sample = 8 * bytes_per_sample;

    let format = WAVEFORMATEX {
        wFormatTag: format_tag,
        nChannels: sample_desc.channels as _,
        nSamplesPerSec: sample_desc.sample_rate as _,
        nAvgBytesPerSec: (sample_desc.channels * sample_desc.sample_rate * bytes_per_sample) as _,
        nBlockAlign: (sample_desc.channels * bytes_per_sample) as _,
        wBitsPerSample: bits_per_sample as _,
        cbSize: (mem::size_of::<WAVEFORMATEXTENSIBLE>() - mem::size_of::<WAVEFORMATEX>()) as _,
    };

    Some(WAVEFORMATEXTENSIBLE {
        Format: format,
        Samples: bits_per_sample as _,
        dwChannelMask: 0, // TODO
        SubFormat: sub_format,
    })
}

type InstanceRaw = WeakPtr<IMMDeviceEnumerator>;
type PhysicalDeviceRaw = WeakPtr<IMMDevice>;
type PhysialDeviceMap = HashMap<PhysicalDeviceRaw, api::StreamFlags>;

pub struct Instance {
    raw: InstanceRaw,
    physical_devices: PhysialDeviceMap,
}

impl Instance {
    pub unsafe fn create(_name: &str) -> Self {
        CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);

        let mut instance = InstanceRaw::null();
        let _hr = CoCreateInstance(
            &CLSID_MMDeviceEnumerator,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IMMDeviceEnumerator::uuidof(),
            instance.mut_void(),
        );

        let mut physical_devices = HashMap::new();
        Self::enumerate_physical_devices_by_flow(&mut physical_devices, instance, eCapture);
        Self::enumerate_physical_devices_by_flow(&mut physical_devices, instance, eRender);

        Instance {
            raw: instance,
            physical_devices,
        }
    }

    unsafe fn enumerate_physical_devices_by_flow(
        physical_devices: &mut PhysialDeviceMap,
        instance: InstanceRaw,
        ty: EDataFlow,
    ) {
        type DeviceCollection = WeakPtr<IMMDeviceCollection>;

        let stream_flags = match ty {
            eCapture => api::StreamFlags::INPUT,
            eRender => api::StreamFlags::OUTPUT,
            _ => unreachable!(),
        };

        let collection = {
            let mut collection = DeviceCollection::null();
            let _hr = instance.EnumAudioEndpoints(
                ty,
                DEVICE_STATE_ACTIVE,
                collection.mut_void() as *mut _,
            );
            collection
        };

        let num_items = {
            let mut num = 0;
            collection.GetCount(&mut num);
            num
        };

        for i in 0..num_items {
            let mut device = PhysicalDeviceRaw::null();
            collection.Item(i, device.mut_void() as *mut _);
            physical_devices
                .entry(device)
                .and_modify(|flags| {
                    *flags |= stream_flags;
                })
                .or_insert(stream_flags);
        }

        collection.Release();
    }

    pub unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        self.physical_devices
            .keys()
            .map(|device| device.as_mut_ptr() as _)
            .collect()
    }

    pub unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        let mut device = PhysicalDeviceRaw::null();
        let _hr = self
            .raw
            .GetDefaultAudioEndpoint(eCapture, eConsole, device.mut_void() as *mut _);
        if device.is_null() {
            None
        } else {
            Some(device.as_mut_ptr() as _)
        }
    }

    pub unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        let mut device = PhysicalDeviceRaw::null();
        let _hr = self
            .raw
            .GetDefaultAudioEndpoint(eRender, eConsole, device.mut_void() as *mut _);
        if device.is_null() {
            None
        } else {
            Some(device.as_mut_ptr() as _)
        }
    }

    pub unsafe fn get_physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        type PropertyStore = WeakPtr<IPropertyStore>;

        let device = PhysicalDeviceRaw::from_raw(physical_device as _);
        let streams = *self
            .physical_devices
            .get(&device)
            .ok_or(api::Error::DeviceLost)?;

        let mut store = PropertyStore::null();
        device.OpenPropertyStore(STGM_READ, store.mut_void() as *mut _);

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

        Ok(api::PhysicalDeviceProperties {
            device_name,
            driver_id: api::DriverId::Wasapi,
            sharing: api::SharingModeFlags::CONCURRENT | api::SharingModeFlags::EXCLUSIVE,
            streams,
        })
    }

    pub unsafe fn create_device(
        &self,
        physical_device: api::PhysicalDevice,
        sample_desc: api::SampleDesc,
    ) -> Device {
        let physical_device = PhysicalDeviceRaw::from_raw(physical_device as _);

        let mut audio_client = WeakPtr::<IAudioClient>::null();
        physical_device.Activate(
            &IAudioClient::uuidof(),
            CLSCTX_ALL,
            ptr::null_mut(),
            audio_client.mut_void() as *mut _,
        );
        let mut original_fmt = ptr::null_mut();
        audio_client.GetMixFormat(&mut original_fmt);
        let mix_format = map_sample_desc(&sample_desc).unwrap(); // todo
        dbg!(audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            &mix_format as *const _ as _,
            ptr::null(),
        ));

        let fence = Fence::create(false, false);
        audio_client.SetEventHandle(fence.0);

        Device {
            client: audio_client,
            fence,
        }
    }
}

impl std::ops::Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.raw.Release();
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

    pub unsafe fn input_stream(&self) -> InputStream {
        let mut client = WeakPtr::<IAudioCaptureClient>::null();
        self.client
            .GetService(&IAudioCaptureClient::uuidof(), client.mut_void() as _);

        InputStream {
            client,
            fence: self.fence,
        }
    }

    pub unsafe fn properties(&self) -> api::DeviceProperties {
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

                let mut channel_mask = api::ChannelMask::empty();
                if format.dwChannelMask & SPEAKER_FRONT_LEFT != 0 {
                    channel_mask |= api::ChannelMask::FRONT_LEFT;
                }
                if format.dwChannelMask & SPEAKER_FRONT_RIGHT != 0 {
                    channel_mask |= api::ChannelMask::FRONT_RIGHT;
                }
                if format.dwChannelMask & SPEAKER_FRONT_CENTER != 0 {
                    channel_mask |= api::ChannelMask::FRONT_CENTER;
                }
                // TODO: more channels

                api::DeviceProperties {
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

pub struct InputStream {
    client: WeakPtr<IAudioCaptureClient>,
    fence: Fence,
}

impl InputStream {
    pub unsafe fn acquire_buffer(&self, timeout_ms: u32) -> (*const u8, api::Frames) {
        self.fence.wait(timeout_ms);

        let mut len = 0;
        self.client.GetNextPacketSize(&mut len);

        let mut data = ptr::null_mut();
        let mut num_frames = 0;
        let mut flags = 0;

        self.client.GetBuffer(
            &mut data,
            &mut num_frames,
            &mut flags,
            ptr::null_mut(),
            ptr::null_mut(),
        );

        if flags != 0 {
            dbg!(flags);
        }

        (data, num_frames as _)
    }

    pub unsafe fn release_buffer(&self, num_frames: api::Frames) {
        self.client.ReleaseBuffer(num_frames as _);
    }
}

pub struct OutputStream {
    device: WeakPtr<IAudioClient>,
    client: WeakPtr<IAudioRenderClient>,
    buffer_size: u32,
    fence: Fence,
}

impl OutputStream {
    pub unsafe fn acquire_buffer(&self, timeout_ms: u32) -> (*mut u8, api::Frames) {
        self.fence.wait(timeout_ms);

        let mut data = ptr::null_mut();
        let mut padding = 0;

        self.device.GetCurrentPadding(&mut padding);

        let len = self.buffer_size - padding;
        self.client.GetBuffer(len, &mut data);
        (data, len as _)
    }

    pub unsafe fn release_buffer(&self, num_frames: api::Frames) {
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
