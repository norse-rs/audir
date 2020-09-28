use crate::handle;

use std::{error, fmt, result};

/// Opaque physical device handle.
pub type PhysicalDevice = handle::RawHandle;

pub const DEFAULT_SAMPLE_RATE: usize = 0;
/// Driver Implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverId {
    Wasapi,
    PulseAudio,
    OpenSLES,
    AAudio,

    Null,
}

bitflags::bitflags! {
    /// Physical Devices may support different resource access modi.
    ///
    /// Other applications and instance may access the same physical device
    /// concurrently or the application requires exclusive access to the certain device.
    pub struct SharingModeFlags: u32 {
        /// Exclusive device access flag.
        const EXCLUSIVE = 0b01;
        /// Concurrent devices access shared by multiple processes flag.
        const CONCURRENT = 0b10;
    }
}

/// Physical device access.
///
/// Sharing mode specifies system-wide access to a physical device resource.
/// Access is not isolated to the current process or instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharingMode {
    /// Exclusive device access.
    Exclusive,
    /// Concurrent devices access shared by multiple processes.
    Concurrent,
}

/// Device stream operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    /// Explicit polling.
    ///
    /// Users need to manually execute `submit_buffers` to poll the stream buffers.
    /// The users are also in control of the audio session in which the stream will be processed.
    Polling,

    /// Callback based stream.
    ///
    /// The device internally poll the stream buffers. Audio sessions are automatically created and maintained.
    /// The execution context of the stream callback is hidden from the users.
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

/// Sample description.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SampleDesc {
    /// Sample Format.
    pub format: Format,
    /// Sample Rate.
    pub sample_rate: usize,
}

/// Frame description.
///
/// Consists of a channel mask and a sample description.
/// A frame is composed of one samples per channel.
#[derive(Debug, Copy, Clone)]
pub struct FrameDesc {
    /// Sample Format.
    pub format: Format,
    /// Sample Rate.
    pub sample_rate: usize,
    /// Channel Mask.
    pub channels: ChannelMask,
}

impl FrameDesc {
    /// Number of channels for the channel mask.
    pub fn num_channels(&self) -> usize {
        self.channels.bits().count_ones() as _
    }

    /// Sample descriptor.
    pub fn sample_desc(&self) -> SampleDesc {
        SampleDesc {
            format: self.format,
            sample_rate: self.sample_rate,
        }
    }
}

/// Properties of the instance implementation.
#[derive(Debug, Clone, Copy)]
pub struct InstanceProperties {
    /// Driver identifier.
    pub driver_id: DriverId,

    /// Operation mode of the device stream.
    pub stream_mode: StreamMode,

    /// Device sharing modes.
    pub sharing: SharingModeFlags,
}
#[derive(Debug, Clone)]
pub enum Error {
    /// Device Lost
    DeviceLost,

    /// Validation error.
    ///
    /// Denote errors caused by incorrect API usage.
    Validation { description: String },

    /// Internal implementation errors.
    Internal { cause: String },
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::DeviceLost => writeln!(fmt, "Device lost"),
            Error::Validation { ref description } => {
                writeln!(fmt, "Validation error: {}", description)
            }
            Error::Internal { ref cause } => writeln!(fmt, "Internal: {}", cause),
        }
    }
}

impl Error {
    pub(crate) fn validation<O, T: ToString>(description: T) -> Result<O> {
        Err(Error::Validation {
            description: description.to_string(),
        })
    }
}

pub type Result<T> = result::Result<T, Error>;

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

/// Device Stream properties.
#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamBuffers {
    /// Number of frames per buffer.
    pub frames: usize,

    /// Input frame buffer.
    ///
    /// For streams with empty input channels the pointer will be null.
    /// The buffer pointer is aligned according to the stream format requirements.
    pub input: *const (),

    /// Input frame buffer.
    ///
    /// For streams with empty output channels the pointer will be null.
    /// The buffer pointer is aligned according to the stream format requirements.
    pub output: *mut (),
}

pub struct Stream {
    pub properties: StreamProperties,
    pub buffers: StreamBuffers,
}

pub type StreamCallback = Box<dyn FnMut(Stream) + Send>;

pub trait Instance {
    type Device: Device;

    /// Audio Session
    ///
    /// See more details on `create_session`.
    type Session;

    /// Get instance properties.
    unsafe fn properties() -> InstanceProperties;

    /// Create an instance object.
    ///
    /// ## Validation
    ///
    /// - The instance **must** outlive all its child objects.
    unsafe fn create(name: &str) -> Self;

    /// Retrieve a list of physical devices of the current instance.
    ///
    /// The list may vary over time when devices get added or removed.
    /// Users may track changes manually by registering an event handler.
    unsafe fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice>;

    /// Get the default physical input device.
    unsafe fn default_physical_input_device(&self) -> Option<PhysicalDevice>;

    /// Get the default physical output device.
    unsafe fn default_physical_output_device(&self) -> Option<PhysicalDevice>;

    /// Get physical device properties.
    ///
    /// ## Validation
    ///
    /// - `physical_device` **must** be a valid handle.
    unsafe fn physical_device_properties(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<PhysicalDeviceProperties>;

    /// Check format support for a physical device.
    ///
    /// ## Validation
    ///
    /// - `physical_device` **must** be a valid handle.
    unsafe fn physical_device_supports_format(
        &self,
        physical_device: PhysicalDevice,
        sharing: SharingMode,
        frame_desc: FrameDesc,
    ) -> bool;

    /// Get default concurrent mode format.
    ///
    /// Returns the default format used for physical devices when
    /// used with concurrent sharing mode.
    ///
    /// ## Validation
    ///
    /// - `physical_device` **must** be a valid handle.
    unsafe fn physical_device_default_concurrent_format(
        &self,
        physical_device: PhysicalDevice,
    ) -> Result<FrameDesc>;

    /// Create a new logical device.
    ///
    /// A logical device with an associated stream will be created
    /// from a physical device.
    ///
    /// ## Validation
    ///
    /// - `physical_device` **must** be a valid handle.
    /// - If the device properties does not include `StreamFlags::INPUT`, the input channel mask must be empty.
    /// - If the device properties does not include `StreamFlags::OUTPUT`, the output channel mask must be empty.
    /// - If output channel mask is not empty, the format consisting of sample desc and output channel mask
    ///   **must** be supported by this physical device.
    /// - If input channel mask is not empty, the format consisting of sample desc and input channel mask
    ///   **must** be supported by this physical device.
    unsafe fn create_device(
        &self,
        desc: DeviceDesc,
        channels: Channels,
        callback: StreamCallback,
    ) -> Result<Self::Device>;

    /// Create an audio session.
    ///
    /// Audio sessions are needed for ensuring realtime properties for audio streaming.
    /// Callback based instances have an internal executor with the a properly configured audio session.
    /// After creating a session the current executor thread will have realtime properties for the lifetime of the session.
    ///
    /// All polling instances will expose a concurrent default format with a `sample_rate`,
    /// which is not equal to `DEFAULT_SAMPLE_RATE`.
    ///
    /// ## Validation
    ///
    /// - `sample_rate` **must** not be `DEFAULT_SAMPLE_RATE`.
    unsafe fn create_session(&self, sample_rate: usize) -> Result<Self::Session>;

    unsafe fn set_event_callback<F>(&mut self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(Event) + Send + 'static;
}

pub trait Device {
    unsafe fn start(&self);
    unsafe fn stop(&self);

    unsafe fn stream_properties(&self) -> StreamProperties;

    /// Submit stream buffers.
    ///
    /// This function **must** be called only for devices of a polling instance.
    /// It will internally wait for acquiring the streaming buffers, call the stream callback
    /// for reading/writing the buffers and submit these to the audio engine.
    ///
    /// ## Validation
    ///
    /// - **Must** only be called for devices, which corresponding instance streaming properties are `Polling`.
    unsafe fn submit_buffers(&mut self, _timeout_ms: u32) -> Result<()> {
        Error::validation("`submit_buffers` not allowed for callback based instances")
    }
}
