
<h1 align="center">audir</h1>
<p align="center">
    <a href="https://github.com/norse-rs">
       <img src="https://img.shields.io/badge/project-norse-9cf.svg?style=flat-square" alt="NORSE">
    </a>
    <a href="https://github.com/norse-rs/audir/actions">
        <img src="https://github.com/norse-rs/audir/workflows/ci/badge.svg?style=flat" alt="ci">
    </a>
    <br>
    <a href="LICENSE-MIT">
      <img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License - MIT">
    </a>
    <a href="LICENSE-APACHE">
      <img src="https://img.shields.io/badge/license-APACHE2-green.svg?style=flat-square" alt="License - Apache2">
    </a>
    <br>
    <b>🚧 Under Construction 🚧</b>
</p>

Low level cross-platform audio library.

The library tries to be un-opionionated by closely tying the API to the exposed functionality of the backends. See the [design notes](audir/DESIGN.md) for more background about different aspects of the API.

## Backends

- Wasapi (Windows)
- Pulse (Linux)
- OpenSL|ES (Android)
- AAudio (Android)

## Usage

Basic audio rendering example:

```Rust
/// `Instance` is the main entry-point and refers to one backend implementation
let instance_properties = Instance::properties();
let instance = Instance::create("sine");

/// Select a physical device for audio rendering.
let output_device = match instance.default_physical_output_device() {
    Some(device) => device,
    None => physical_devices
        .into_iter()
        .find(|device| {
            let properties = instance.physical_device_properties(*device);
            match properties {
                Ok(properties) => properties.streams.contains(audir::StreamFlags::OUTPUT),
                Err(_) => false,
            }
        })
        .expect("No output device found"),
};

let sample_rate = 48_000;
let mut device = instance.create_device(
    // Concurrent access to selected output device
    audir::DeviceDesc {
        physical_device: output_device,
        sharing: audir::SharingMode::Concurrent,
        sample_desc: audir::SampleDesc {
            format: audir::Format::F32,
            sample_rate,
        },
    },
    // Stereo Output
    audir::Channels {
        input: audir::ChannelMask::empty(),
        output: audir::ChannelMask::FRONT_LEFT | audir::ChannelMask::FRONT_RIGHT,
    },
    // Callback which will be executed by the audio executor.
    Box::new(move |stream| {
        let properties = stream.properties();

        let sample_rate = properties.sample_rate as f32;
        let num_channels = properties.num_channels();

        let audir::StreamBuffers { output, frames, .. } = stream.buffers;
        let buffer =
            std::slice::from_raw_parts_mut(output as *mut f32, frames as usize * num_channels);

        for dt in 0..frames {
            // fill buffers..
        }
    }),
)?;

match instance_properties.stream_mode {
    audir::StreamMode::Polling => {
        // Configure the current thread for audio execution (for polling).
        let _session = instance.create_session(sample_rate)?;
        // Start playback
        device.start();
        loop {
            // Backends may support Polling or Callback streaming mode.
            // In case of polling we manually control the playback loop and the executor,
            // otherwise the device will automatically do the audio processing.
            device.submit_buffers(!0)?;
        }
    }
    audir::StreamMode::Callback => {
        // Start playback
        device.start();
        loop { }
    }
}
```
