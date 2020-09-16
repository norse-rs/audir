#![allow(non_upper_case_globals)]

pub mod com;
mod fence;

use self::fence::*;

pub use winapi::shared::winerror::HRESULT;
pub type WasapiResult<T> = (T, HRESULT);

use com::{Guid, WeakPtr};
use std::{
    collections::HashMap, ffi::OsString, mem, os::windows::ffi::OsStringExt, ptr, slice,
    sync::Mutex,
};
use winapi::shared::{
    devpkey::*, ksmedia, minwindef::DWORD, mmreg::*, winerror, wtypes::PROPERTYKEY,
};
use winapi::um::{
    audioclient::*, audiosessiontypes::*, combaseapi::*, coml2api::STGM_READ, mmdeviceapi::*,
    objbase::COINIT_MULTITHREADED, propsys::*, winnt::*,
};
use winapi::Interface;

use crate::{
    api::{self, Result},
    handle::Handle,
};

unsafe fn string_from_wstr(os_str: *const WCHAR) -> String {
    let mut len = 0;
    while *os_str.offset(len) != 0 {
        len += 1;
    }
    let string: OsString = OsStringExt::from_wide(slice::from_raw_parts(os_str, len as _));
    string.into_string().unwrap()
}

#[repr(C)]
#[derive(com_impl::ComImpl)]
#[interfaces(IMMNotificationClient)]
pub struct NotificationClient {
    vtbl: com_impl::VTable<IMMNotificationClientVtbl>,
    refcount: com_impl::Refcount,
    cb: Box<dyn FnMut(api::Event)>,
}

#[com_impl::com_impl]
unsafe impl IMMNotificationClient for NotificationClient {
    unsafe fn on_device_state_changed(&self, pwstrDeviceId: LPCWSTR, state: DWORD) -> HRESULT {
        println!("changed {} to {}", string_from_wstr(pwstrDeviceId), state);
        winerror::S_OK
    }

    unsafe fn on_device_added(&self, pwstrDeviceId: LPCWSTR) -> HRESULT {
        println!("added {}", string_from_wstr(pwstrDeviceId));
        winerror::S_OK
    }

    unsafe fn on_device_removed(&self, pwstrDeviceId: LPCWSTR) -> HRESULT {
        println!("removed {}", string_from_wstr(pwstrDeviceId));
        winerror::S_OK
    }

    unsafe fn on_default_device_changed(
        &self,
        _flow: EDataFlow,
        role: ERole,
        pwstrDefaultDeviceId: LPCWSTR,
    ) -> HRESULT {
        if role == eConsole {
            println!("default {:?} ({})", pwstrDefaultDeviceId, role);
        }

        winerror::S_OK
    }

    unsafe fn on_property_value_changed(
        &self,
        _pwstrDeviceId: LPCWSTR,
        _key: PROPERTYKEY,
    ) -> HRESULT {
        winerror::S_OK
    }
}

fn map_frame_desc(frame_desc: &api::FrameDesc) -> Option<WAVEFORMATEXTENSIBLE> {
    let (format_tag, sub_format, bytes_per_sample) = match frame_desc.format {
        api::Format::F32 => (
            WAVE_FORMAT_EXTENSIBLE,
            ksmedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
            4,
        ),
        api::Format::U32 => return None,
        _ => unimplemented!(),
    };

    let mut channel_mask = 0;
    {
        let channels = frame_desc.channels;
        if channels.contains(api::ChannelMask::FRONT_LEFT) {
            channel_mask |= SPEAKER_FRONT_LEFT;
        }
        if channels.contains(api::ChannelMask::FRONT_RIGHT) {
            channel_mask |= SPEAKER_FRONT_RIGHT;
        }
        if channels.contains(api::ChannelMask::FRONT_CENTER) {
            channel_mask |= SPEAKER_FRONT_CENTER;
        }
    }

    let num_channels = frame_desc.num_channels();
    let bits_per_sample = 8 * bytes_per_sample;

    let format = WAVEFORMATEX {
        wFormatTag: format_tag,
        nChannels: num_channels as _,
        nSamplesPerSec: frame_desc.sample_rate as _,
        nAvgBytesPerSec: (num_channels * frame_desc.sample_rate * bytes_per_sample) as _,
        nBlockAlign: (num_channels * bytes_per_sample) as _,
        wBitsPerSample: bits_per_sample as _,
        cbSize: (mem::size_of::<WAVEFORMATEXTENSIBLE>() - mem::size_of::<WAVEFORMATEX>()) as _,
    };

    Some(WAVEFORMATEXTENSIBLE {
        Format: format,
        Samples: bits_per_sample as _,
        dwChannelMask: channel_mask,
        SubFormat: sub_format,
    })
}

unsafe fn map_waveformat(format: *const WAVEFORMATEX) -> Result<api::FrameDesc> {
    let wave_format = &*format;
    match wave_format.wFormatTag {
        WAVE_FORMAT_EXTENSIBLE => {
            let wave_format_ex = &*(format as *const WAVEFORMATEXTENSIBLE);
            let subformat = Guid(wave_format_ex.SubFormat);
            let samples = wave_format_ex.Samples;
            let format =
                if subformat == Guid(ksmedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT) && samples == 32 {
                    api::Format::F32
                } else {
                    return Err(api::Error::Internal { cause: "unsupported format".into() }); // TODO
                };

            let mut channels = api::ChannelMask::empty();
            if wave_format_ex.dwChannelMask & SPEAKER_FRONT_LEFT != 0 {
                channels |= api::ChannelMask::FRONT_LEFT;
            }
            if wave_format_ex.dwChannelMask & SPEAKER_FRONT_RIGHT != 0 {
                channels |= api::ChannelMask::FRONT_RIGHT;
            }
            if wave_format_ex.dwChannelMask & SPEAKER_FRONT_CENTER != 0 {
                channels |= api::ChannelMask::FRONT_CENTER;
            }

            Ok(api::FrameDesc {
                format,
                channels,
                sample_rate: wave_format.nSamplesPerSec as _,
            })
        }
        _ => Err(api::Error::Internal { cause: "unsupported wave format".into() }), // TODO
    }
}

fn map_sharing_mode(sharing: api::SharingMode) -> AUDCLNT_SHAREMODE {
    match sharing {
        api::SharingMode::Exclusive => AUDCLNT_SHAREMODE_EXCLUSIVE,
        api::SharingMode::Concurrent => AUDCLNT_SHAREMODE_SHARED,
    }
}

type InstanceRaw = WeakPtr<IMMDeviceEnumerator>;
type PhysicalDeviceRaw = WeakPtr<IMMDevice>;
struct PhysicalDevice {
    device: PhysicalDeviceRaw,
    audio_client: WeakPtr<IAudioClient>,
    streams: api::StreamFlags,
}

impl PhysicalDevice {
    unsafe fn state(&self) -> u32 {
        let mut state = 0;
        self.device.GetState(&mut state);
        state
    }
}

type PhysicalDeviceId = String;
type PhysialDeviceMap = HashMap<PhysicalDeviceId, Handle<PhysicalDevice>>;

pub struct Session(Option<audio_thread_priority::RtPriorityHandle>);

impl std::ops::Drop for Session {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            audio_thread_priority::demote_current_thread_from_real_time(handle).unwrap();
        }
    }
}

pub struct Instance {
    raw: InstanceRaw,
    physical_devices: Mutex<PhysialDeviceMap>,
    notifier: WeakPtr<NotificationClient>,
}

impl api::Instance for Instance {
    type Device = Device;
    type Session = Session;

    unsafe fn properties() -> api::InstanceProperties {
        api::InstanceProperties {
            driver_id: api::DriverId::Wasapi,
            stream_mode: api::StreamMode::Polling,
            sharing: api::SharingModeFlags::CONCURRENT | api::SharingModeFlags::EXCLUSIVE,
        }
    }

    unsafe fn create(_: &str) -> Self {
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
            physical_devices: Mutex::new(physical_devices),
            notifier: WeakPtr::null(),
        }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        let mut physical_devices = self.physical_devices.lock().unwrap();

        Self::enumerate_physical_devices_by_flow(&mut physical_devices, self.raw, eCapture);
        Self::enumerate_physical_devices_by_flow(&mut physical_devices, self.raw, eRender);

        physical_devices
            .values()
            .filter_map(|device| {
                if device.state() & DEVICE_STATE_ACTIVE != 0 {
                    Some(device.raw())
                } else {
                    None
                }
            })
            .collect()
    }

    unsafe fn default_physical_input_device(&self) -> Option<api::PhysicalDevice> {
        let mut device = PhysicalDeviceRaw::null();
        let _hr = self
            .raw
            .GetDefaultAudioEndpoint(eCapture, eConsole, device.mut_void() as *mut _);
        if device.is_null() {
            None
        } else {
            let id = Self::get_physical_device_id(device);
            Some(self.physical_devices.lock().unwrap()[&id].raw())
        }
    }

    unsafe fn default_physical_output_device(&self) -> Option<api::PhysicalDevice> {
        let mut device = PhysicalDeviceRaw::null();
        let _hr = self
            .raw
            .GetDefaultAudioEndpoint(eRender, eConsole, device.mut_void() as *mut _);
        if device.is_null() {
            None
        } else {
            let id = Self::get_physical_device_id(device);
            Some(self.physical_devices.lock().unwrap()[&id].raw())
        }
    }

    unsafe fn physical_device_properties(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::PhysicalDeviceProperties> {
        type PropertyStore = WeakPtr<IPropertyStore>;

        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);

        let mut store = PropertyStore::null();
        physical_device
            .device
            .OpenPropertyStore(STGM_READ, store.mut_void() as *mut _);

        let device_name = {
            let mut value = mem::MaybeUninit::uninit();
            store.GetValue(
                &DEVPKEY_Device_FriendlyName as *const _ as *const _,
                value.as_mut_ptr(),
            );
            let os_str = *value.assume_init().data.pwszVal();
            string_from_wstr(os_str)
        };

        let _form_factor = {
            let mut value = mem::MaybeUninit::uninit();
            store.GetValue(
                &PKEY_AudioEndpoint_FormFactor as *const _ as *const _,
                value.as_mut_ptr(),
            );
            *value.assume_init().data.uintVal()
        };

        Ok(api::PhysicalDeviceProperties {
            device_name,
            form_factor: api::FormFactor::Unknown, // todo
            streams: physical_device.streams,
        })
    }

    unsafe fn physical_device_default_concurrent_format(
        &self,
        physical_device: api::PhysicalDevice,
    ) -> Result<api::FrameDesc> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);

        let mut mix_format = ptr::null_mut();
        physical_device.audio_client.GetMixFormat(&mut mix_format);
        map_waveformat(mix_format)
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
        callback: api::StreamCallback,
    ) -> Result<Device> {
        if !channels.input.is_empty() && !channels.output.is_empty() {
            // no duplex
            return api::Error::validation("Duplex not supported");
        }

        let use_default_sample_rate = desc.sample_desc.sample_rate == api::DEFAULT_SAMPLE_RATE;
        if use_default_sample_rate && desc.sharing == api::SharingMode::Exclusive {
            return api::Error::validation("Default sample rate can't be used with exclusive sharing mode");
        }

        let physical_device = Handle::<PhysicalDevice>::from_raw(desc.physical_device);
        let sharing = map_sharing_mode(desc.sharing);

        let fence = Fence::create(false, false);

        let sample_rate = if use_default_sample_rate {
            self.physical_device_default_concurrent_format(desc.physical_device)?.sample_rate
        } else {
            desc.sample_desc.sample_rate
        };

        let frame_desc = api::FrameDesc {
            format: desc.sample_desc.format,
            channels: if !channels.input.is_empty() {
                channels.input
            } else {
                channels.output
            },
            sample_rate,
        };
        let mix_format = map_frame_desc(&frame_desc).unwrap(); // todo
        let _hr = physical_device.audio_client.Initialize(
            sharing,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            &mix_format as *const _ as _,
            ptr::null(),
        );

        physical_device.audio_client.SetEventHandle(fence.0);

        let mut mix_format = ptr::null_mut();
        physical_device.audio_client.GetMixFormat(&mut mix_format);
        let frame_desc = map_waveformat(mix_format).unwrap();

        let (properties, device_stream) = if !channels.input.is_empty() {
            let mut capture_client = WeakPtr::<IAudioCaptureClient>::null();
            physical_device.audio_client.GetService(
                &IAudioCaptureClient::uuidof(),
                capture_client.mut_void() as _,
            );
            let buffer_size = {
                let mut size = 0;
                physical_device.audio_client.GetBufferSize(&mut size);
                size
            };

            let properties = api::StreamProperties {
                channels: frame_desc.channels,
                sample_rate: frame_desc.sample_rate,
                buffer_size: buffer_size as _,
            };
            let device_stream = DeviceStream::Input {
                client: capture_client,
            };

            (properties, device_stream)
        } else {
            let mut render_client = WeakPtr::<IAudioRenderClient>::null();
            physical_device
                .audio_client
                .GetService(&IAudioRenderClient::uuidof(), render_client.mut_void() as _);
            let buffer_size = {
                let mut size = 0;
                physical_device.audio_client.GetBufferSize(&mut size);
                size
            };

            let properties = api::StreamProperties {
                channels: frame_desc.channels,
                sample_rate: frame_desc.sample_rate,
                buffer_size: buffer_size as _,
            };
            let device_stream = DeviceStream::Output {
                client: render_client,
                buffer_size,
            };

            (properties, device_stream)
        };

        Ok(Device {
            client: physical_device.audio_client,
            fence,
            device_stream,
            callback,
            properties,
        })
    }

    unsafe fn create_session(&self, sample_rate: usize) -> Result<Session> {
        if sample_rate == api::DEFAULT_SAMPLE_RATE {
            return api::Error::validation("Default sample rate can't be used for session creation");
        }

        let rt_handle =
            audio_thread_priority::promote_current_thread_to_real_time(0, sample_rate as _)
                .unwrap();
        Ok(Session(Some(rt_handle)))
    }

    unsafe fn set_event_callback<F>(&mut self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(api::Event) + Send + 'static,
    {
        if !self.notifier.is_null() {
            self.raw
                .UnregisterEndpointNotificationCallback(self.notifier.as_mut_ptr() as *mut _);
            self.notifier.as_unknown().Release();
        }

        if let Some(callback) = callback {
            self.notifier = WeakPtr::from_raw(NotificationClient::create_raw(Box::new(callback)));
            self.raw
                .RegisterEndpointNotificationCallback(self.notifier.as_mut_ptr() as *mut _);
        }

        Ok(())
    }

    unsafe fn physical_device_supports_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
        frame_desc: api::FrameDesc,
    ) -> bool {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);

        let wave_format = map_frame_desc(&frame_desc).unwrap(); // todo
        let sharing = map_sharing_mode(sharing);

        let mut closest_format = ptr::null_mut();
        let hr = dbg!(physical_device.audio_client.IsFormatSupported(
            sharing,
            &wave_format as *const _ as _,
            &mut closest_format
        ));

        hr == winerror::S_OK
    }
}

impl Instance {
    unsafe fn get_physical_device_id(device: PhysicalDeviceRaw) -> String {
        let mut str_id = ptr::null_mut();
        device.GetId(&mut str_id);
        let mut len = 0;
        while *str_id.offset(len) != 0 {
            len += 1;
        }
        let name: OsString = OsStringExt::from_wide(slice::from_raw_parts(str_id, len as _));
        name.into_string().unwrap()
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
                DEVICE_STATEMASK_ALL,
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
            let id = Self::get_physical_device_id(device);

            let state = {
                let mut state = 0;
                device.GetState(&mut state);
                state
            };

            physical_devices
                .entry(id)
                .and_modify(|device| {
                    device.streams |= stream_flags;
                })
                .or_insert_with(|| {
                    let mut audio_client = WeakPtr::<IAudioClient>::null();

                    if state & DEVICE_STATE_ACTIVE != 0 {
                        device.Activate(
                            &IAudioClient::uuidof(),
                            CLSCTX_ALL,
                            ptr::null_mut(),
                            audio_client.mut_void() as *mut _,
                        );
                    }

                    Handle::new(PhysicalDevice {
                        device,
                        audio_client,
                        streams: stream_flags,
                    })
                });
        }

        collection.Release();
    }
}

impl std::ops::Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.raw.Release();
            if !self.notifier.is_null() {
                WeakPtr::from_raw(self.notifier.as_mut_ptr() as *mut IMMNotificationClient)
                    .Release();
            }
            // TODO: drop audio clients
        }
    }
}

pub enum DeviceStream {
    Input {
        client: WeakPtr<IAudioCaptureClient>,
    },
    Output {
        client: WeakPtr<IAudioRenderClient>,
        buffer_size: u32,
    },
}

pub struct Device {
    client: WeakPtr<IAudioClient>,
    fence: Fence,
    device_stream: DeviceStream,
    callback: api::StreamCallback,
    properties: api::StreamProperties,
}

impl std::ops::Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.client.Release();
            self.fence.destory();
        }
    }
}

impl Device {
    unsafe fn acquire_buffers(&mut self, timeout_ms: u32) -> Result<api::StreamBuffers> {
        self.fence.wait(timeout_ms);

        match self.device_stream {
            DeviceStream::Input { client } => {
                let mut len = 0;
                client.GetNextPacketSize(&mut len);

                let mut data = ptr::null_mut();
                let mut num_frames = 0;
                let mut flags = 0;

                client.GetBuffer(
                    &mut data,
                    &mut num_frames,
                    &mut flags,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );

                if flags != 0 {
                    dbg!(flags);
                }

                Ok(api::StreamBuffers {
                    frames: num_frames as _,
                    input: data as _,
                    output: ptr::null_mut(),
                })
            }
            DeviceStream::Output {
                client,
                buffer_size,
            } => {
                let mut data = ptr::null_mut();
                let mut padding = 0;

                self.client.GetCurrentPadding(&mut padding);

                let len = buffer_size - padding;
                client.GetBuffer(len, &mut data);
                Ok(api::StreamBuffers {
                    frames: len as _,
                    input: ptr::null(),
                    output: data as _,
                })
            }
        }
    }

    unsafe fn release_buffers(&mut self, num_frames: api::Frames) -> Result<()> {
        match self.device_stream {
            DeviceStream::Input { client } => {
                client.ReleaseBuffer(num_frames as _);
            }
            DeviceStream::Output { client, .. } => {
                client.ReleaseBuffer(num_frames as _, 0);
            }
        }
        Ok(())
    }
}

impl api::Device for Device {
    unsafe fn start(&self) {
        self.client.Start();
    }

    unsafe fn stop(&self) {
        self.client.Stop();
    }

    unsafe fn stream_properties(&self) -> api::StreamProperties {
        self.properties
    }

    unsafe fn submit_buffers(&mut self, timeout_ms: u32) -> Result<()> {
        let buffers = self.acquire_buffers(timeout_ms)?;
        (self.callback)(api::Stream {
            properties: self.properties,
            buffers,
        });
        self.release_buffers(buffers.frames)
    }
}
