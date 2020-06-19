#![allow(non_upper_case_globals)]

pub mod com;
mod fence;

use self::fence::*;

pub use winapi::shared::winerror::HRESULT;
pub type WasapiResult<T> = (T, HRESULT);

use com::{Guid, WeakPtr};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::{ffi::OsString, mem, os::windows::ffi::OsStringExt, ptr, slice};
use winapi::shared::devpkey::*;
use winapi::shared::ksmedia;
use winapi::shared::minwindef::DWORD;
use winapi::shared::mmreg::*;
use winapi::shared::winerror;
use winapi::shared::wtypes::PROPERTYKEY;
use winapi::um::audioclient::*;
use winapi::um::audiosessiontypes::*;
use winapi::um::combaseapi::*;
use winapi::um::coml2api::STGM_READ;
use winapi::um::mmdeviceapi::*;
use winapi::um::objbase::COINIT_MULTITHREADED;
use winapi::um::propsys::*;
use winapi::um::winnt::*;

use winapi::Interface;

use crate::{
    api::{self, Result},
    handle::Handle,
};

#[derive(Debug)]
enum Event {
    Added(PhysicalDeviceId),
    Removed(PhysicalDeviceId),
    Changed {
        device: PhysicalDeviceId,
        state: u32,
    },
    Default {
        device: PhysicalDeviceId,
        flow: EDataFlow,
    },
}

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
    tx: Sender<Event>,
}

#[com_impl::com_impl]
unsafe impl IMMNotificationClient for NotificationClient {
    unsafe fn on_device_state_changed(&self, pwstrDeviceId: LPCWSTR, state: DWORD) -> HRESULT {
        let _ = self.tx.send(Event::Changed {
            device: string_from_wstr(pwstrDeviceId),
            state,
        });
        winerror::S_OK
    }

    unsafe fn on_device_added(&self, pwstrDeviceId: LPCWSTR) -> HRESULT {
        let _ = self.tx.send(Event::Added(string_from_wstr(pwstrDeviceId)));
        winerror::S_OK
    }

    unsafe fn on_device_removed(&self, pwstrDeviceId: LPCWSTR) -> HRESULT {
        let _ = self
            .tx
            .send(Event::Removed(string_from_wstr(pwstrDeviceId)));
        winerror::S_OK
    }

    unsafe fn on_default_device_changed(
        &self,
        flow: EDataFlow,
        role: ERole,
        pwstrDefaultDeviceId: LPCWSTR,
    ) -> HRESULT {
        if role == eConsole {
            let _ = self.tx.send(Event::Default {
                device: string_from_wstr(pwstrDefaultDeviceId),
                flow,
            });
        }

        winerror::S_OK
    }

    unsafe fn on_property_value_changed(
        &self,
        pwstrDeviceId: LPCWSTR,
        key: PROPERTYKEY,
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

    let bits_per_sample = 8 * bytes_per_sample;

    let format = WAVEFORMATEX {
        wFormatTag: format_tag,
        nChannels: frame_desc.channels as _,
        nSamplesPerSec: frame_desc.sample_rate as _,
        nAvgBytesPerSec: (frame_desc.channels * frame_desc.sample_rate * bytes_per_sample) as _,
        nBlockAlign: (frame_desc.channels * bytes_per_sample) as _,
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
                    return Err(api::Error::Validation); // TODO
                };

            Ok(api::FrameDesc {
                format,
                channels: wave_format.nChannels as _,
                sample_rate: wave_format.nSamplesPerSec as _,
            })
        }
        _ => Err(api::Error::Validation), // TODO
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
    state: u32,
    audio_client: WeakPtr<IAudioClient>,
    streams: api::StreamFlags,
}

impl PhysicalDevice {
    unsafe fn default_format(&self, sharing: api::SharingMode) -> Result<api::FrameDesc> {
        match sharing {
            api::SharingMode::Concurrent => {
                let mut mix_format = ptr::null_mut();
                self.audio_client.GetMixFormat(&mut mix_format);
                map_waveformat(mix_format)
            }
            api::SharingMode::Exclusive => unimplemented!(),
        }
    }
}

type PhysicalDeviceId = String;
type PhysialDeviceMap = HashMap<PhysicalDeviceId, Handle<PhysicalDevice>>;

pub struct Instance {
    raw: InstanceRaw,
    physical_devices: PhysialDeviceMap,
    notifier: WeakPtr<NotificationClient>,
    event_rx: Receiver<Event>,
}

impl api::Instance for Instance {
    type Device = Device;

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

        let (tx, event_rx) = channel();
        let notification_client = NotificationClient::create_raw(tx);

        let mut physical_devices = HashMap::new();
        Self::enumerate_physical_devices_by_flow(&mut physical_devices, instance, eCapture);
        Self::enumerate_physical_devices_by_flow(&mut physical_devices, instance, eRender);

        Instance {
            raw: instance,
            physical_devices,
            notifier: WeakPtr::from_raw(notification_client),
            event_rx,
        }
    }

    unsafe fn enumerate_physical_devices(&self) -> Vec<api::PhysicalDevice> {
        self.physical_devices
            .values()
            .filter_map(|device| {
                if device.state & DEVICE_STATE_ACTIVE != 0 {
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
            Some(self.physical_devices[&id].raw())
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
            Some(self.physical_devices[&id].raw())
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
            let mut value = mem::uninitialized();
            store.GetValue(
                &DEVPKEY_Device_FriendlyName as *const _ as *const _,
                &mut value,
            );
            let os_str = *value.data.pwszVal();
            string_from_wstr(os_str)
        };

        Ok(api::PhysicalDeviceProperties {
            device_name,
            streams: physical_device.streams,
        })
    }

    unsafe fn physical_device_default_input_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);
        physical_device.default_format(sharing)
    }

    unsafe fn physical_device_default_output_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
    ) -> Result<api::FrameDesc> {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);
        physical_device.default_format(sharing)
    }

    unsafe fn create_device(
        &self,
        desc: api::DeviceDesc,
        channels: api::Channels,
    ) -> Result<Device> {
        if channels.input != 0 && channels.output != 0 {
            return Err(api::Error::Validation);
        }

        let physical_device = Handle::<PhysicalDevice>::from_raw(desc.physical_device);
        let sharing = map_sharing_mode(desc.sharing);

        let fence = Fence::create(false, false);

        let frame_desc = api::FrameDesc {
            format: desc.sample_desc.format,
            channels: channels.input.max(channels.output),
            sample_rate: desc.sample_desc.sample_rate,
        };
        let mix_format = map_frame_desc(&frame_desc).unwrap(); // todo
        dbg!(physical_device.audio_client.Initialize(
            sharing,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            &mix_format as *const _ as _,
            ptr::null(),
        ));

        physical_device.audio_client.SetEventHandle(fence.0);

        Ok(Device {
            client: physical_device.audio_client,
            fence,
            stream: if channels.input > 0 {
                StreamTy::Input
            } else {
                StreamTy::Output
            },
        })
    }

    unsafe fn destroy_device(&self, device: &mut Device) {
        device.client.Release();
        device.fence.destory();
    }

    unsafe fn poll_events<F>(&self, _callback: F) -> Result<()>
    where
        F: FnMut(api::Event),
    {
        while let Ok(event) = self.event_rx.try_recv() {
            // TODO
            dbg!(event);
        }

        Ok(())
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
                DEVICE_STATE_ACTIVE
                    | DEVICE_STATE_DISABLED
                    | DEVICE_STATE_NOTPRESENT
                    | DEVICE_STATE_UNPLUGGED,
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
                        state,
                        audio_client,
                        streams: stream_flags,
                    })
                });
        }

        collection.Release();
    }

    pub unsafe fn physical_device_supports_format(
        &self,
        physical_device: api::PhysicalDevice,
        sharing: api::SharingMode,
        frame_desc: api::FrameDesc,
    ) {
        let physical_device = Handle::<PhysicalDevice>::from_raw(physical_device);

        let wave_format = map_frame_desc(&frame_desc).unwrap(); // todo
        let sharing = map_sharing_mode(sharing);

        let mut closest_format = ptr::null_mut();
        let hr = dbg!(physical_device.audio_client.IsFormatSupported(
            sharing,
            &wave_format as *const _ as _,
            &mut closest_format
        ));
    }
}

impl std::ops::Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.raw.Release();
            WeakPtr::from_raw(self.notifier.as_mut_ptr() as *mut IMMNotificationClient).Release();
            // TODO: drop audio clients
        }
    }
}

enum StreamTy {
    Input,
    Output,
}

pub struct Device {
    client: WeakPtr<IAudioClient>,
    fence: Fence,
    stream: StreamTy,
}

impl api::Device for Device {
    type Stream = Stream;

    unsafe fn get_stream(&self) -> Result<Stream> {
        match self.stream {
            StreamTy::Input => {
                let mut capture_client = WeakPtr::<IAudioCaptureClient>::null();
                self.client.GetService(
                    &IAudioCaptureClient::uuidof(),
                    capture_client.mut_void() as _,
                );

                Ok(Stream::Input {
                    client: capture_client,
                    fence: self.fence,
                })
            }
            StreamTy::Output => {
                let mut render_client = WeakPtr::<IAudioRenderClient>::null();

                self.client
                    .GetService(&IAudioRenderClient::uuidof(), render_client.mut_void() as _);

                let buffer_size = {
                    let mut size = 0;
                    self.client.GetBufferSize(&mut size);
                    size
                };

                Ok(Stream::Output {
                    client: render_client,
                    device: self.client,
                    buffer_size,
                    fence: self.fence,
                })
            }
        }
    }

    unsafe fn start(&self) {
        self.client.Start();
    }

    unsafe fn stop(&self) {
        self.client.Stop();
    }
}

pub enum Stream {
    Input {
        client: WeakPtr<IAudioCaptureClient>,
        fence: Fence,
    },
    Output {
        device: WeakPtr<IAudioClient>,
        client: WeakPtr<IAudioRenderClient>,
        buffer_size: u32,
        fence: Fence,
    },
}

impl api::Stream for Stream {
    unsafe fn properties(&self) -> api::StreamProperties {
        match *self {
            Stream::Input { .. } => unimplemented!(),
            Stream::Output { device, .. } => {
                let buffer_size = {
                    let mut size = 0;
                    device.GetBufferSize(&mut size);
                    size as _
                };

                let mut mix_format = ptr::null_mut();
                device.GetMixFormat(&mut mix_format);

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

                        api::StreamProperties {
                            num_channels: format.Format.nChannels as _,
                            channel_mask,
                            sample_rate: format.Format.nSamplesPerSec as _,
                            buffer_size,
                        }
                    }
                    _ => unimplemented!(),
                }
            }
        }
    }

    unsafe fn acquire_buffers(&mut self, timeout_ms: u32) -> Result<api::StreamBuffers> {
        match self {
            Stream::Input { client, fence } => {
                fence.wait(timeout_ms);

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
            Stream::Output {
                device,
                client,
                buffer_size,
                fence,
            } => {
                fence.wait(timeout_ms);

                let mut data = ptr::null_mut();
                let mut padding = 0;

                device.GetCurrentPadding(&mut padding);

                let len = *buffer_size - padding;
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
        match self {
            Stream::Input { client, .. } => {
                client.ReleaseBuffer(num_frames as _);
            }
            Stream::Output { client, .. } => {
                client.ReleaseBuffer(num_frames as _, 0);
            }
        }
        Ok(())
    }

    unsafe fn set_callback(&mut self, _: api::StreamCallback) -> Result<()> {
        Err(api::Error::Validation)
    }
}
