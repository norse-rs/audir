#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::{Device, Instance as InstanceTrait, Stream};

fn main() -> anyhow::Result<()> {
    unsafe {
        let instance_properties = Instance::properties();
        let mut instance = Instance::create("audir - sine");
        instance.set_event_callback(Some(|event| {
            dbg!(event);
        }))?;

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

        let format = instance.physical_device_default_concurrent_format(output_device)?;

        println!(
            "{:X}: {:#?} @ {:#?}",
            output_device,
            instance.physical_device_properties(output_device)?,
            format,
        );

        let sample_rate = format.sample_rate;
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

                for i in 0..num_channels {
                    buffer[num_channels * dt as usize + i] = sample;
                }

                cycle = (cycle + cycle_step) % 1.0;
            }
        };

        let mut device = instance.create_device(
            audir::DeviceDesc {
                physical_device: output_device,
                sharing: audir::SharingMode::Concurrent,
                sample_desc: format.sample_desc(),
            },
            audir::Channels {
                input: audir::ChannelMask::empty(),
                output: format.channels,
            },
            Box::new(callback),
        )?;

        let _session = instance.create_session(sample_rate)?;

        device.start();

        loop {
            if instance_properties.stream_mode == audir::StreamMode::Polling {
                device.submit_buffers(!0)?;
            }
        }
    }
}
