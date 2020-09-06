#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::{Device, Instance as InstanceTrait, Stream};

use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    unsafe {
        let instance_properties = Instance::properties();
        let instance = Instance::create("audir - capture");
        let physical_devices = instance.enumerate_physical_devices();

        let input_device = match instance.default_physical_input_device() {
            Some(device) => device,
            None => physical_devices
                .into_iter()
                .find(|device| {
                    let properties = instance.physical_device_properties(*device);
                    match properties {
                        Ok(properties) => properties.streams.contains(audir::StreamFlags::INPUT),
                        Err(_) => false,
                    }
                })
                .expect("no input device found"),
        };

        println!(
            "{:X}: {:#?}",
            input_device,
            instance.physical_device_properties(input_device)?
        );

        let sample_rate = 48_000;

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: sample_rate as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = Arc::new(Mutex::new(
            hound::WavWriter::create("capture.wav", spec).unwrap(),
        ));

        let wav = writer.clone();
        let callback = move |stream: &<Instance as InstanceTrait>::Stream, buffers| {
            let properties = stream.properties();
            let num_channels = properties.num_channels();

            let audir::StreamBuffers { input, frames, .. } = buffers;
            let buffer =
                std::slice::from_raw_parts(input as *const f32, frames as usize * num_channels);

            let mut writer = wav.lock().unwrap();
            for sample in buffer {
                writer.write_sample(*sample).unwrap();
            }
        };

        {
            let mut device = instance.create_device(
                audir::DeviceDesc {
                    physical_device: input_device,
                    sharing: audir::SharingMode::Concurrent,
                    sample_desc: audir::SampleDesc {
                        format: audir::Format::F32,
                        sample_rate,
                    },
                },
                audir::Channels {
                    input: audir::ChannelMask::FRONT_LEFT | audir::ChannelMask::FRONT_RIGHT,
                    output: audir::ChannelMask::empty(),
                },
                Box::new(callback),
            )?;

            let _session = instance.create_session(sample_rate)?;

            device.start();

            let mut start = std::time::Instant::now();
            loop {
                if start.elapsed() > std::time::Duration::from_secs(4) {
                    break;
                }

                if instance_properties.stream_mode == audir::StreamMode::Polling {
                    device.submit_buffers(!0)?;
                }
            }

            device.stop();
        }

        Arc::try_unwrap(writer)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap()
            .finalize()?;

        Ok(())
    }
}
