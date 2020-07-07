use audir::Instance;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        #[cfg(windows)]
        let instance = audir::wasapi::Instance::create("audir - devices");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - devices");

        let physical_devices = instance.enumerate_physical_devices();

        for device in &physical_devices {
            let properties = instance.physical_device_properties(*device)?;
            println!("{:#?}", instance.physical_device_properties(*device)?);
            if properties.streams.contains(audir::StreamFlags::INPUT) {
                println!(
                    " - Input Format: {:#?}",
                    instance.physical_device_default_input_format(
                        *device,
                        audir::SharingMode::Concurrent
                    )
                );
            }
            if properties.streams.contains(audir::StreamFlags::OUTPUT) {
                println!(
                    " - Ouput Format: {:#?}",
                    instance.physical_device_default_output_format(
                        *device,
                        audir::SharingMode::Concurrent
                    )
                );
            }
        }

        if let Some(output_device) = instance.default_physical_output_device() {
            println!(
                "default output: {:#?}",
                instance.physical_device_properties(output_device)?
            );
        }

        if let Some(input_device) = instance.default_physical_input_device() {
            println!(
                "default input: {:#?}",
                instance.physical_device_properties(input_device)?
            );
        }
    }

    Ok(())
}
