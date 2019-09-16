#[cfg(windows)]
pub mod wasapi;

#[cfg(target_os = "linux")]
pub mod pulse;

#[cfg(target_os = "macos")]
pub mod coreaudio;

#[derive(Debug, Clone, Copy)]
pub enum DriverId {
    Wasapi,
    PulseAudio,
}

bitflags::bitflags! {
    /// Physical Devices may support different resource access modi.
    ///
    /// Other applications and instance may access the same physical device
    /// concurrently or the application requires exclusive access to the certain device.
    pub struct SharingModeFlags: u32 {
        const EXCLUSIVE = 0b01;
        const CONCURRENT = 0b10;
    }
}

bitflags::bitflags! {
    pub struct ChannelMask: u32 {
        const FRONT_LEFT = 0b0001;
        const FRONT_RIGHT = 0b0010;
        const FRONT_CENTER = 0b0100;
    }
}

pub type Frames = u32;

#[derive(Debug, Clone)]
pub struct PhysicalDeviceProperties {
    pub device_name: String,
    pub driver_id: DriverId,
    pub sharing: SharingModeFlags,
}

#[derive(Debug, Copy, Clone)]
pub enum Format {
    F32,
    I16,
}

#[derive(Debug, Copy, Clone)]
pub struct SampleDesc {
    pub format: Format,
    pub channels: usize,
    pub sample_rate: usize,
}

#[derive(Debug, Clone)]
pub struct DeviceProperties {
    pub num_channels: usize,
    pub channel_mask: ChannelMask,
    pub buffer_size: Frames,
}
