use norse_audir::wasapi;
use std::fs::File;
use std::env;
use lewton::inside_ogg::OggStreamReader;

fn main() -> Result<(), Box<std::error::Error>> {
    let file_path = env::args().nth(1).expect("No arg found. Please specify a file to open.");
    let file = File::open(file_path).expect("Can't open file");
    let mut ogg_stream = OggStreamReader::new(file)?;

    let mut samples = Vec::new();
    loop {
        let data: Option<lewton::samples::InterleavedSamples<f32>> = ogg_stream.read_dec_packet_generic()?;
        match data {
            Some(data) => {
                samples.extend(data.samples);
            }
            None => break,
        }
    }

    unsafe {
        let instance = wasapi::Instance::create("audir - music");
        let output_devices = instance.enumerate_physical_output_devices();
        let device = instance.create_device(&output_devices[0]);
        let properties = dbg!(device.properties());
        let buffer_frames = properties.buffer_size;
        let stream = device.get_output_stream();

        let sample_rate = 48_000;
        let num_channels = properties.num_channels;

        assert_eq!(sample_rate, ogg_stream.ident_hdr.audio_sample_rate);
        assert_eq!(num_channels, ogg_stream.ident_hdr.audio_channels as _);

        device.start();

        let mut sample = 0;
        loop {
            let padding = device.get_current_padding();

            let num_frames = buffer_frames - padding;
            let raw_buffer = stream.acquire_buffer(num_frames, !0);
            let buffer = std::slice::from_raw_parts_mut(
                raw_buffer as *mut f32,
                num_frames as usize * num_channels,
            );

            for dt in 0..num_channels*num_frames as usize {
                sample = (sample + 1) % samples.len();
                buffer[dt as usize] = samples[sample];
            }

            stream.submit_buffer(num_frames);
        }
    }

    Ok(())
}
