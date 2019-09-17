use lewton::inside_ogg::OggStreamReader;
use norse_audir as audir;
use std::env;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = env::args()
        .nth(1)
        .expect("No arg found. Please specify a file to open.");
    let file = File::open(file_path).expect("Can't open file");
    let mut ogg_stream = OggStreamReader::new(file)?;

    let mut samples = Vec::new();
    loop {
        let data: Option<lewton::samples::InterleavedSamples<f32>> =
            ogg_stream.read_dec_packet_generic()?;
        match data {
            Some(data) => {
                samples.extend(data.samples);
            }
            None => break,
        }
    }

    unsafe {
        #[cfg(windows)]
        let instance = audir::wasapi::Instance::create("audir - sine");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - sine");

        let output_devices = instance.enumerate_physical_output_devices();
        let device = instance.create_device(
            &output_devices[0],
            audir::SampleDesc {
                format: audir::Format::F32,
                channels: 2,
                sample_rate: 48_000,
            },
        );
        let mut stream = device.output_stream();
        let properties = dbg!(device.properties());

        let sample_rate = properties.sample_rate;
        let num_channels = properties.num_channels;

        assert_eq!(sample_rate, ogg_stream.ident_hdr.audio_sample_rate as _);
        assert_eq!(num_channels, ogg_stream.ident_hdr.audio_channels as _);

        device.start();

        let mut sample = 0;
        loop {
            let (raw_buffer, num_frames) = stream.acquire_buffer(!0);
            let buffer = std::slice::from_raw_parts_mut(
                raw_buffer as *mut f32,
                num_frames as usize * num_channels,
            );

            for dt in 0..num_channels * num_frames as usize {
                buffer[dt as usize] = samples[sample];
                sample = (sample + 1) % samples.len();
            }

            stream.submit_buffer(num_frames);
        }
    }

    Ok(())
}
