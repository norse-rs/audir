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

        let device = instance.create_device(
            &output_devices[0],
            audir::SampleDesc {
                format: audir::Format::F32,
                channels: 2,
                sample_rate: 44_100,
            },
        );

        let frequency = 100.0;
        let sample_rate = 44_100.0;
        let num_channels = 2;
        let cycle_step = frequency / sample_rate;
        let mut cycle = 0.0;

        let mut stream = device.polled_output_stream();
        let device_properties = device.properties();
        let size = device_properties.buffer_size;
        loop {
            let raw_buffer = stream.acquire_buffer(size, !0);

            let buffer = std::slice::from_raw_parts_mut(raw_buffer as *mut f32, size as usize / 4);

            for dt in 0..buffer.len() / 2 {
                let phase = 2.0 * std::f32::consts::PI * cycle;
                let sample = phase.sin() * 0.5;

                buffer[num_channels * dt as usize] = sample;
                buffer[num_channels * dt as usize + 1] = sample;

                cycle = (cycle + cycle_step) % 1.0;
            }

            stream.submit_buffer(size);
        }
    }
}
