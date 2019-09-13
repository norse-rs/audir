
use libpulse_sys as pulse;
use norse_audir as audir;

use std::ptr;

fn main() {
    unsafe {
        let instance = audir::pulse::Instance::create("audir - sine");
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

        let device = instance.create_device(&output_devices[0], audir::SampleDesc {
            format: audir::Format::F32,
            channels: 2,
            sample_rate: 44_100,
        });
        let device_properties = device.properties();

        let mut stream = device.get_output_stream();

        let mut total_frames = 0;
        let size = device_properties.buffer_size;
        loop {
            let raw_buffer = stream.acquire_buffer(size, !0);

            let buffer =
                std::slice::from_raw_parts_mut(raw_buffer as *mut f32, size as usize / 4);

            for dt in 0..buffer.len() / 2 {
                let frame_time = total_frames + dt;
                let time = frame_time as f32 / 44_100.0;
                let value = (100.0 * time).sin() * 100.0;

                buffer[2 * dt as usize] = value as _;
                buffer[2 * dt as usize + 1] = value as _;
            }
            total_frames += buffer.len() / 2;

            stream.submit_buffer(size);
        }
    }
}
