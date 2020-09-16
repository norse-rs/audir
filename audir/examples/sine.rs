#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::{Device, Instance as InstanceTrait};

use dasp::signal::Signal;

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

        let mut source = None;
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
            Box::new(move |stream| {
                let sample_rate = stream.properties.sample_rate as f32;
                let num_channels = stream.properties.num_channels();

                source = Some(match source.take() {
                    Some(source) => source,
                    None => dasp::signal::rate(sample_rate as _).const_hz(frequency).sine()
                });
                let source = source.as_mut().unwrap();

                let audir::StreamBuffers { output, frames, .. } = stream.buffers;
                let buffer =
                    std::slice::from_raw_parts_mut(output as *mut f32, frames as usize * num_channels);

                for dt in 0..frames {
                    let sample = source.next() as f32 * 0.5;
                    for i in 0..num_channels {
                        buffer[num_channels * dt as usize + i] = sample;
                    }
                }
            }),
        )?;

        match instance_properties.stream_mode {
            audir::StreamMode::Polling => {
                let _session = instance.create_session(sample_rate)?;
                device.start();
                loop {
                    device.submit_buffers(!0)?;
                }
            }
            audir::StreamMode::Callback => {
                device.start();
                loop { }
            }
        }
    }
}
