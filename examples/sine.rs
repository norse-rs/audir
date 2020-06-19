use audir::{Device, Instance, Stream};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        #[cfg(all(windows, not(feature = "asio")))]
        let instance = audir::wasapi::Instance::create("audir - sine");
        #[cfg(feature = "asio")]
        let instance = audir::asio::Instance::create("audir - sine");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - sine");
        #[cfg(target_os = "android")]
        let instance = audir::opensles::Instance::create("audir - sine");

        let physical_devices = instance.enumerate_physical_devices();

        for device in &physical_devices {
            println!(
                "{:X}: {:#?}",
                device,
                instance.physical_device_properties(*device)?
            );
        }

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
                .unwrap(),
        };

        println!(
            "{:X}: {:#?}",
            output_device,
            instance.physical_device_properties(output_device)?
        );

        let device = instance.create_device(
            audir::DeviceDesc {
                physical_device: output_device,
                sharing: audir::SharingMode::Concurrent,
                sample_desc: audir::SampleDesc {
                    format: audir::Format::F32,
                    sample_rate: 48_000,
                },
            },
            audir::Channels {
                input: 0,
                output: 2,
            },
        )?;

        let mut stream = device.get_stream()?;
        let properties = stream.properties();

        let frequency = 440.0;
        let sample_rate = properties.sample_rate as f32;
        let num_channels = properties.num_channels;
        let cycle_step = frequency / sample_rate;
        let mut cycle = 0.0;

        device.start();

        loop {
            let audir::StreamBuffers { output, frames, .. } = stream.acquire_buffers(!0)?;
            let buffer =
                std::slice::from_raw_parts_mut(output as *mut f32, frames as usize * num_channels);

            for dt in 0..frames {
                let phase = 2.0 * std::f32::consts::PI * cycle;
                let sample = phase.sin() * 0.5;

                buffer[num_channels * dt as usize] = sample;
                buffer[num_channels * dt as usize + 1] = sample;

                cycle = (cycle + cycle_step) % 1.0;
            }

            stream.release_buffers(frames)?;
        }
    }
}
