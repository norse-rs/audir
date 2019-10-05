use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_min_level(log::Level::Trace) // limit log level
                .with_tag("audir"), // logs will show under mytag tag
        );
    }

    unsafe {
        #[cfg(windows)]
        let instance = audir::wasapi::Instance::create("audir - sine");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - sine");
        #[cfg(target_os = "android")]
        let instance = audir::opensles::Instance::create("audir - sine");

        let physical_devices = instance.enumerate_physical_devices();

        for device in &physical_devices {
            println!("{:#?}", instance.get_physical_device_properties(*device)?);
        }

        let output_device = physical_devices
            .into_iter()
            .find(|device| {
                let properties = instance.get_physical_device_properties(*device);
                match properties {
                    Ok(properties) => properties.streams.contains(audir::StreamFlags::OUTPUT),
                    Err(_) => false,
                }
            })
            .unwrap();

        let device = instance.create_device(
            output_device,
            audir::SampleDesc {
                format: audir::Format::F32,
                channels: 2,
                sample_rate: 48_000,
            },
        );

        let properties = dbg!(device.properties());

        let frequency = 440.0;
        let sample_rate = properties.sample_rate as f32;
        let num_channels = properties.num_channels;
        let cycle_step = frequency / sample_rate;
        let mut cycle = 0.0;

        let mut stream = device.output_stream();
        device.start();

        loop {
            let (raw_buffer, num_frames) = stream.acquire_buffer(!0);
            let buffer = std::slice::from_raw_parts_mut(
                raw_buffer as *mut f32,
                num_frames as usize * num_channels,
            );

            for dt in 0..num_frames {
                let phase = 2.0 * std::f32::consts::PI * cycle;
                let sample = phase.sin() * 0.5; // ((phase.sin() * 0.5 + 0.5) * std::u32::MAX as f32) as u32;

                buffer[num_channels * dt as usize] = sample;
                buffer[num_channels * dt as usize + 1] = sample;

                cycle = (cycle + cycle_step) % 1.0;
            }

            stream.submit_buffer(num_frames);
        }
    }

    Ok(())
}
