#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::{Device, Instance as InstanceTrait, Stream};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        let instance_properties = Instance::properties();
        let instance = Instance::create("audir - sine");

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

        let frequency = 440.0;
        let mut cycle = 0.0;

        let callback = move |stream: &<Instance as InstanceTrait>::Stream, buffers| {
            let properties = stream.properties();

            let sample_rate = properties.sample_rate as f32;
            let num_channels = properties.num_channels();
            let cycle_step = frequency / sample_rate;

            let audir::StreamBuffers { output, frames, .. } = buffers;
            let buffer =
                std::slice::from_raw_parts_mut(output as *mut f32, frames as usize * num_channels);

            for dt in 0..frames {
                let phase = 2.0 * std::f32::consts::PI * cycle;
                let sample = phase.sin() * 0.5;

                buffer[num_channels * dt as usize] = sample;
                buffer[num_channels * dt as usize + 1] = sample;

                cycle = (cycle + cycle_step) % 1.0;
            }
        };

        let mut device = instance.create_device(
            audir::DeviceDesc {
                physical_device: output_device,
                sharing: audir::SharingMode::Concurrent,
                sample_desc: audir::SampleDesc {
                    format: audir::Format::F32,
                    sample_rate: 48_000,
                },
            },
            audir::Channels {
                input: audir::ChannelMask::empty(),
                output: audir::ChannelMask::FRONT_LEFT | audir::ChannelMask::FRONT_RIGHT,
            },
            Box::new(callback),
        )?;

        device.start();

        loop {
            if instance_properties.stream_mode == audir::StreamMode::Polling {
                device.submit_buffers(!0)?;
            }
        }
    }
}
