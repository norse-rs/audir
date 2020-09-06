use crate::handle;

use std::{error, fmt, result};

pub type PhysicalDevice = handle::RawHandle;

pub const DEFAULT_SAMPLE_RATE: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverId {
    Wasapi,
    PulseAudio,
    OpenSLES,
    AAudio,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFactor {
    ///
    Unknown,
    /// Remote Network
    Remote,
    ///
    LineLevel,
    ///
    Headphones,
    ///
    Headset,
    ///
    Microphone,
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
    pub form_factor: FormFactor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Format {
    F32,
    I16,
    U32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SampleDesc {
    pub format: Format,
    pub sample_rate: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct FrameDesc {
    pub format: Format,
    pub channels: ChannelMask,
    pub sample_rate: usize,
}

impl FrameDesc {
    pub fn num_channels(&self) -> usize {
        self.channels.bits().count_ones() as _
    }

    pub fn sample_desc(&self) -> SampleDesc {
        SampleDesc {
            format: self.format,
            sample_rate: self.sample_rate,
        }
    }
}

pub struct InstanceProperties {
    pub driver_id: DriverId,
    pub stream_mode: StreamMode,
    pub sharing: SharingModeFlags,
}

#[derive(Debug, Clone)]
pub struct StreamProperties {
    pub channels: ChannelMask,
    pub sample_rate: usize,
    pub buffer_size: Frames,
}

impl StreamProperties {
    pub fn num_channels(&self) -> usize {
        self.channels.bits().count_ones() as _
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    DeviceLost,
    Validation { description: String },
    Internal { cause: String },
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::DeviceLost => writeln!(fmt, "Device lost"),
            Error::Validation { ref description } => writeln!(fmt, "Validation error: {}", description),
            Error::Internal { ref cause } => writeln!(fmt, "Internal: {}", cause),
        }
    }
}

impl Error {
    pub(crate) fn validation<O, T: ToString>(description: T) -> Result<O> {
        Err(Error::Validation { description: description.to_string() })
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Added(PhysicalDevice),
    Removed(PhysicalDevice),
    DefaultInputDevice(Option<PhysicalDevice>),
    DefaultOutputDevice(Option<PhysicalDevice>),
}

#[derive(Debug, Clone)]
pub struct DeviceDesc {
    pub physical_device: PhysicalDevice,
    pub sharing: SharingMode,
    pub sample_desc: SampleDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    pub input: ChannelMask,
    pub output: ChannelMask,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamBuffers {
    pub frames: usize,
    pub input: *const (),
    pub output: *mut (),
}
pub type StreamCallback<S> = Box<dyn FnMut(&S, StreamBuffers) + Send>;

pub trait Instance {
    type Device: Device;
    type Stream: Stream;
    type Session;

    unsafe fn properties() -> InstanceProperties;

    unsafe fn create(name: &str) -> Self;

    unsafe fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice>;

    unsafe fn default_physical_input_device(&self) -> Option<PhysicalDevice>;

    unsafe fn default_physical_output_device(&self) -> Option<PhysicalDevice>;

    unsafe fn physical_device_properties(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<PhysicalDeviceProperties>;

    unsafe fn physical_device_supports_format(
        &self,
        physical_device: PhysicalDevice,
        sharing: SharingMode,
        frame_desc: FrameDesc,
    ) -> bool;

    unsafe fn physical_device_default_concurrent_format(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<FrameDesc>;

    unsafe fn create_device(
        &self,
        desc: DeviceDesc,
        channels: Channels,
        callback: StreamCallback<Self::Stream>,
    ) -> Result<Self::Device>;

    unsafe fn create_session(&self, sample_rate: usize) -> Result<Self::Session>;

    unsafe fn set_event_callback<F>(&mut self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(Event) + Send + 'static;
}

pub trait Device {
    unsafe fn start(&self);
    unsafe fn stop(&self);

    unsafe fn submit_buffers(&mut self, _timeout_ms: u32) -> Result<()> {
        Error::validation("`submit_buffers` not allowed for callback based instances")
    }
}

pub trait Stream {
    unsafe fn properties(&self) -> StreamProperties;
}
