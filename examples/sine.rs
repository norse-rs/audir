use norse_audir::wasapi;

fn main() {
    unsafe {
        let instance = wasapi::Instance::create("audir - sine");
        let input_devices = instance.enumerate_physical_input_devices();
        let output_devices = instance.enumerate_physical_output_devices();

        println!(
            "I: {:?} / O: {:?}",
            input_devices.len(),
            output_devices.len()
        );

        for device in &output_devices {
            println!("{:#?}", device.get_properties());
        }

        let device = instance.create_device(&output_devices[0]);
        let buffer_frames = dbg!(device.get_buffer_frames());
        let stream = device.get_output_stream();
        device.start();

        let mut total_frames = 0;

        loop {
            let padding = device.get_current_padding();

            let num_frames = buffer_frames - padding;
            let raw_buffer = stream.acquire_buffer(num_frames, !0);
            let buffer =
                std::slice::from_raw_parts_mut(raw_buffer as *mut f32, num_frames as usize / 4);

            for dt in 0..buffer.len() / 2 {
                let frame_time = total_frames + dt;
                let time = frame_time as f32 / 48_000.0;
                let value = (100.0 * time).sin() * 0.2;

                buffer[2 * dt as usize + 1] = value;
                buffer[2 * dt as usize] = value;
            }
            stream.submit_buffer(num_frames);
        }
    }
}
