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
    pub driver_id: DriverId,
    pub sharing: SharingModeFlags,
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
    pub channels: usize,
    pub sample_rate: usize,
}

#[derive(Debug, Clone)]
pub struct DeviceProperties {
    pub num_channels: usize,
    pub channel_mask: ChannelMask,
    pub sample_rate: usize,
    pub buffer_size: Frames,
}

#[derive(Debug, Clone)]
pub enum Error {
    DeviceLost,
    Validation,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::DeviceLost => writeln!(fmt, "Device lost"),
            Error::Validation => writeln!(fmt, "Validation error"),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

pub trait Instance {
    type Device: Device;

    unsafe fn create(name: &str) -> Self;

    unsafe fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice>;

    unsafe fn default_physical_input_device(&self) -> Option<PhysicalDevice>;

    unsafe fn default_physical_output_device(&self) -> Option<PhysicalDevice>;

    unsafe fn get_physical_device_properties(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<PhysicalDeviceProperties>;

    unsafe fn create_device(
        &self,
        physical_device: PhysicalDevice,
        sharing: SharingMode,
        input_sample_desc: Option<SampleDesc>,
        output_sample_desc: Option<SampleDesc>,
    ) -> Self::Device;

    unsafe fn destroy_device(&self, device: &mut Self::Device);
}

pub trait Device {
    type OutputStream: OutputStream;
    type InputStream: InputStream;

    unsafe fn get_output_stream(&self) -> Result<Self::OutputStream>;
    unsafe fn get_input_stream(&self) -> Result<Self::InputStream>;

    unsafe fn start(&self);
    unsafe fn stop(&self);
}

pub trait OutputStream {}

pub trait InputStream {}
