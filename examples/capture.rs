use norse_audir as audir;

fn main() {
    unsafe {
        #[cfg(windows)]
        let instance = audir::wasapi::Instance::create("audir - capture");

        let input_devices = instance.enumerate_physical_input_devices();

        println!(
            "I: {:?}",
            input_devices.len(),
        );

        for device in &input_devices {
            println!("{:#?}", device.get_properties());
        }

        let device = instance.create_device(
            &input_devices[0],
            audir::SampleDesc {
                format: audir::Format::F32,
                channels: 2,
                sample_rate: 44_100,
            },
        );

        let mut stream = device.input_stream();
        let properties = dbg!(device.properties());

        device.start();

        loop {
            let (raw_buffer, num_frames) = stream.acquire_buffer(!0);

            // TODO

            stream.release_buffer(num_frames);
        }
    }
}
