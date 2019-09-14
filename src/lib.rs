#[cfg(windows)]
pub mod wasapi;

#[cfg(unix)]
pub mod alsa;

#[cfg(unix)]
pub mod pulse;

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
        const Exclusive = 0b01;
        const Concurrent = 0b10;
    }
}

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
pub enum Channel {
    FrontLeft,
    FrontRight,
}

#[derive(Debug, Copy, Clone)]
pub struct SampleDesc {
    pub format: Format,
    pub channels: usize,
    pub sample_rate: usize,
}

#[derive(Debug, Clone)]
pub struct DeviceProperties {
    pub channels: Vec<Channel>,
    pub buffer_size: u32,
}
