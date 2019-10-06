use crate::handle;

use std::{error, fmt, result};

pub type PhysicalDevice = handle::RawHandle;

#[derive(Debug, Clone, Copy)]
pub enum DriverId {
    Wasapi,
    PulseAudio,
    OpenSLES,
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

#[derive(Debug, Clone, Copy)]
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
