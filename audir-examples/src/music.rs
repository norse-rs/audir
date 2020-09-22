use crate::instance::Instance;
use audir::{Device, Instance as InstanceTrait};

#[cfg(target_os = "android")]
use std::path::Path;

#[cfg(target_os = "android")]
pub fn load<P: AsRef<Path>>(path: P) -> Vec<u8> {
    use std::ffi::CString;
    use std::io::Read;

    let filename = path.as_ref().to_str().expect("Can`t convert Path to &str");
    let native_activity = ndk_glue::native_activity();
    let asset_manager = native_activity.asset_manager();
    let mut asset = asset_manager
        .open(&CString::new(filename).unwrap())
        .expect("Could not open asset");

    let mut data = vec![];
    asset.read_to_end(&mut data).unwrap();
    data
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(target_os = "android"))]
    let mut audio_stream = {
        let file_path = std::env::args().nth(1);
        match file_path {
            Some(path) => audrey::open(path)?,
            None => {
                println!("Error: No arg found. Please specify a file to open.");
                return Ok(());
            }
        }
    };

    #[cfg(target_os = "android")]
    let mut audio_stream = {
        let file = std::io::Cursor::new(load("asmr_48000.ogg"));
        audrey::Reader::new(file).unwrap()
    };

    let samples = audio_stream
        .frames::<[f32; 2]>()
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    unsafe {
        let instance_properties = Instance::properties();
        let instance = Instance::create("audir-music");

        let output_device = match instance.default_physical_output_device() {
            Some(device) => device,
            None => instance
                .enumerate_physical_devices()
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

        let sample_rate = 48_000;
        let format = audir::Format::F32;
        let output_channels = audir::ChannelMask::FRONT_LEFT | audir::ChannelMask::FRONT_RIGHT;

        assert!(instance.physical_device_supports_format(
            output_device,
            audir::SharingMode::Concurrent,
            audir::FrameDesc {
                sample_rate,
                format,
                channels: output_channels,
            },
        ));

        let mut sample = 0;
        let mut device = instance.create_device(
            audir::DeviceDesc {
                physical_device: output_device,
                sharing: audir::SharingMode::Concurrent,
                sample_desc: audir::SampleDesc {
                    format,
                    sample_rate,
                },
            },
            audir::Channels {
                input: audir::ChannelMask::empty(),
                output: output_channels,
            },
            Box::new(move |stream| {
                let properties = stream.properties;
                let num_channels = properties.num_channels();

                let buffer = std::slice::from_raw_parts_mut(
                    stream.buffers.output as *mut f32,
                    stream.buffers.frames as usize * num_channels,
                );

                for dt in 0..stream.buffers.frames as usize {
                    let frame = samples[sample];
                    buffer[num_channels * dt as usize] = frame[0];
                    buffer[num_channels * dt as usize + 1] = frame[1];
                    sample = (sample + 1) % samples.len();
                }
            }),
        )?;

        device.start();

        loop {
            if instance_properties.stream_mode == audir::StreamMode::Polling {
                device.submit_buffers(!0)?;
            }
        }
    }
}
