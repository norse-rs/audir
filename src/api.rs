use crate::handle;

use std::{error, fmt, result};

pub type PhysicalDevice = handle::RawHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverId {
    Wasapi,
    PulseAudio,
    OpenSLES,
    CoreAudio,
    WebAudio,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharingMode {
    Exclusive,
    Concurrent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    Polling,
    Callback,
}

bitflags::bitflags! {
    pub struct ChannelMask: u32 {
        const FRONT_LEFT = 0b0001;
        const FRONT_RIGHT = 0b0010;
        const FRONT_CENTER = 0b0100;
    }
}

bitflags::bitflags! {
    pub struct StreamFlags: u32 {
        const INPUT = 0b01;
        const OUTPUT = 0b10;
    }
}

pub type Frames = usize;

#[derive(Debug, Clone)]
pub struct PhysicalDeviceProperties {
    pub device_name: String,
    pub streams: StreamFlags,
}

#[derive(Debug, Copy, Clone)]
pub enum Format {
    F32,
    I16,
    U32,
}

#[derive(Debug, Copy, Clone)]
pub struct SampleDesc {
    pub format: Format,
    pub sample_rate: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct FrameDesc {
    pub format: Format,
    pub channels: usize,
    pub sample_rate: usize,
}

pub struct InstanceProperties {
    pub driver_id: DriverId,
    pub stream_mode: StreamMode,
    pub sharing: SharingModeFlags,
}

#[derive(Debug, Clone)]
pub struct StreamProperties {
    pub num_channels: usize,
    pub channel_mask: ChannelMask,
    pub sample_rate: usize,
    pub buffer_size: Frames,
}

#[derive(Debug, Clone)]
pub enum Error {
    DeviceLost,
    Validation,
    Internal { cause: String },
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::DeviceLost => writeln!(fmt, "Device lost"),
            Error::Validation => writeln!(fmt, "Validation error"),
            Error::Internal { ref cause } => writeln!(fmt, "Internal: {}", cause),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {}

#[derive(Debug, Clone)]
pub struct StreamDesc {}

#[derive(Debug, Clone)]
pub struct DeviceDesc {
    pub physical_device: PhysicalDevice,
    pub sharing: SharingMode,
    pub sample_desc: SampleDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    pub input: usize,
    pub output: usize,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamBuffers {
    pub frames: usize,
    pub input: *const (),
    pub output: *mut (),
}
pub type StreamCallback = Box<dyn FnMut(StreamBuffers) + Send>;

pub trait Instance {
    type Device: Device;

    unsafe fn properties() -> InstanceProperties;

    unsafe fn create(name: &str) -> Self;

    unsafe fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice>;

    unsafe fn default_physical_input_device(&self) -> Option<PhysicalDevice>;

    unsafe fn default_physical_output_device(&self) -> Option<PhysicalDevice>;

    unsafe fn physical_device_properties(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<PhysicalDeviceProperties>;

    unsafe fn physical_device_default_input_format(
        &self,
        physical_device: PhysicalDevice,
        sharing: SharingMode,
    ) -> Result<FrameDesc>;

    unsafe fn physical_device_default_output_format(
        &self,
        physical_device: PhysicalDevice,
        sharing: SharingMode,
    ) -> Result<FrameDesc>;

    unsafe fn create_device(
        &self,
        desc: DeviceDesc,
        channels: Channels,
    ) -> Result<Self::Device>;

    unsafe fn destroy_device(&self, device: &mut Self::Device);

    unsafe fn poll_events<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(Event);
}

pub trait Stream {
    unsafe fn properties(&self) -> StreamProperties;
    unsafe fn set_callback(&mut self, callback: StreamCallback) -> Result<()>;
    unsafe fn acquire_buffers(&mut self, timeout_ms: u32) -> Result<StreamBuffers>;
    unsafe fn release_buffers(&mut self, num_frames: Frames) -> Result<()>;
}

pub trait Device {
    type Stream: Stream;
    unsafe fn get_stream(&self) -> Result<Self::Stream>;

    unsafe fn start(&self);
    unsafe fn stop(&self);
}
