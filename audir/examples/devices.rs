#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

use audir::Instance as InstanceTrait;

fn main() -> anyhow::Result<()> {
    unsafe {
        #[cfg(windows)]
        let instance = Instance::create("audir - devices");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - devices");

        let physical_devices = instance.enumerate_physical_devices();

        for device in &physical_devices {
            let properties = instance.physical_device_properties(*device)?;
            println!("{:#?}", properties);
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
