#[cfg(target_os = "android")]
use audir::opensles::Instance;
#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::{Device, Instance as InstanceTrait};

#[cfg(target_os = "android")]
use std::path::Path;

#[cfg(target_os = "android")]
pub fn load<P: AsRef<Path>>(path: P) -> Vec<u8> {
    use android_glue;

    let filename = path.as_ref().to_str().expect("Can`t convert Path to &str");
    match android_glue::load_asset(filename) {
        Ok(buf) => buf,
        Err(_) => panic!("Can`t load asset '{}'", filename),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_min_level(log::Level::Trace) // limit log level
                .with_tag("audir-music"), // logs will show under mytag tag
        );
    }

    #[cfg(not(target_os = "android"))]
    let mut audio_stream = {
        let file_path = std::env::args()
            .nth(1)
            .expect("No arg found. Please specify a file to open.");
        audrey::open(file_path)?
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

        instance.enumerate_physical_devices();

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
                input: 0,
                output: 2,
            },
        )?;

        let properties = device.stream_properties();
        let num_channels = properties.num_channels;

        let mut sample = 0;
        let mut callback = move |buffers: audir::StreamBuffers| {
            let buffer = std::slice::from_raw_parts_mut(
                buffers.output as *mut f32,
                buffers.frames as usize * num_channels,
            );

            for dt in 0..buffers.frames as usize {
                let frame = samples[sample];
                buffer[num_channels * dt as usize] = frame[0];
                buffer[num_channels * dt as usize + 1] = frame[1];
                sample = (sample + 1) % samples.len();
            }
        };

        match instance_properties.stream_mode {
            audir::StreamMode::Callback => {
                device.set_callback(Box::new(callback))?;
                device.start();
                loop {}
            }
            audir::StreamMode::Polling => {
                device.start();
                loop {
                    let buffers = device.acquire_buffers(!0)?;
                    callback(buffers);
                    device.release_buffers(buffers.frames)?;
                }
            }
        }
    }

    Ok(())
}
