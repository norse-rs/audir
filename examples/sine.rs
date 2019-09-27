use norse_audir as audir;
use opensles as sles;
use std::ptr;
use std::os::raw::c_void;

const NUM_SAMPLES: usize = 4096;

fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Trace) // limit log level
            .with_tag("audir") // logs will show under mytag tag
    );

    unsafe {
        #[cfg(windows)]
        let instance = audir::wasapi::Instance::create("audir - sine");
        #[cfg(target_os = "linux")]
        let instance = audir::pulse::Instance::create("audir - sine");
        #[cfg(target_os = "android")]
        let instance = audir::opensles::Instance::create("audir - sine");

        let input_devices = instance.enumerate_physical_input_devices();
        let output_devices = instance.enumerate_physical_output_devices();

        log::warn!(
            "I: {:?} / O: {:?}",
            input_devices.len(),
            output_devices.len()
        );

        let mut engine = ptr::null();
        log::info!("{:?}", sles::bindings::slCreateEngine(&mut engine, 0, ptr::null(), 0, ptr::null(), ptr::null_mut()));
        log::info!("{:?}", ((**engine).Realize).unwrap()(engine, sles::SL_BOOLEAN_FALSE));

        let mut interface: sles::bindings::SLEngineItf = ptr::null();
        log::info!("{:?}", ((**engine).GetInterface).unwrap()(engine, sles::bindings::SL_IID_ENGINE, &mut interface as *mut _ as _));

        let mut output_mix: sles::bindings::SLObjectItf = ptr::null();
        log::info!("{:?}", ((**interface).CreateOutputMix).unwrap()(interface, &mut output_mix, 0, ptr::null(), ptr::null()));

        log::info!("{:?}", ((**output_mix).Realize).unwrap()(output_mix, sles::SL_BOOLEAN_FALSE));

        let mut audio_player = ptr::null();
        let mut locator_source = sles::bindings::SLDataLocator_AndroidSimpleBufferQueue {
            locatorType: 0x800007BD, // sles::bindings::SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE,
            numBuffers: 2, // ?
        };
        // TODO: u32
        let mut format_source = sles::bindings::SLDataFormat_PCM {
            formatType: sles::SL_DATAFORMAT_PCM,
            numChannels: 2, // TODO
            samplesPerSec: sles::SL_SAMPLINGRATE_44_1, // ZODO
            bitsPerSample: 32,
            containerSize: 32,
            channelMask: sles::SL_SPEAKER_FRONT_LEFT | sles::SL_SPEAKER_FRONT_RIGHT,
            endianness: sles::SL_BYTEORDER_LITTLEENDIAN,
        };
        let mut source = sles::bindings::SLDataSource {
            pLocator: &mut locator_source as *mut _ as _,
            pFormat: &mut format_source as *mut _ as _,
        };
        let mut locator_sink = sles::bindings::SLDataLocator_OutputMix {
            locatorType: sles::SL_DATALOCATOR_OUTPUTMIX,
            outputMix: output_mix,
        };
        let mut sink = sles::bindings::SLDataSink {
            pLocator: &mut locator_sink as *mut _ as _,
            pFormat: ptr::null_mut(),
        };
        let ids = [sles::bindings::SL_IID_BUFFERQUEUE];
        let requirements = [sles::SL_BOOLEAN_TRUE];
        log::info!("player: {:?}", ((**interface).CreateAudioPlayer).unwrap()(interface, &mut audio_player, &mut source, &mut sink, 1, ids.as_ptr(), requirements.as_ptr()));

        log::info!("{:?}", ((**audio_player).Realize).unwrap()(audio_player, sles::SL_BOOLEAN_FALSE));

        let mut queue: sles::bindings::SLAndroidSimpleBufferQueueItf = ptr::null();
        log::info!("queue: {:?}", ((**audio_player).GetInterface).unwrap()(audio_player, sles::bindings::SL_IID_BUFFERQUEUE, &mut queue as *mut _ as _));

        let mut state: sles::bindings::SLPlayItf = ptr::null();
        log::info!("state: {:?}", ((**audio_player).GetInterface).unwrap()(audio_player, sles::bindings::SL_IID_PLAY, &mut state as *mut _ as _));

        let mut buffers = [[0u32; 2 * NUM_SAMPLES]; 2];
        let frequency = 100.0;
        let sample_rate = 44_100.0 as f32;
        let num_channels = 2;
        let cycle_step = frequency / sample_rate;
        let mut cycle = 0.0;
        let mut cur_buffer = 0;

        unsafe fn output_stream<F>(queue: sles::bindings::SLAndroidSimpleBufferQueueItf, callback: F)
        where
            F: FnMut(sles::bindings::SLAndroidSimpleBufferQueueItf),
        {
            let callback = Box::new(callback);
            extern "C" fn write_cb<F>(queue: sles::bindings::SLAndroidSimpleBufferQueueItf, user: *mut c_void)
            where
                F: FnMut(sles::bindings::SLAndroidSimpleBufferQueueItf),
            {
                unsafe {
                    (&mut *(user as *mut F))(queue);
                }
            }

            log::info!("queue cb: {:?}", ((**queue).RegisterCallback).unwrap()(queue, Some(write_cb::<F>), Box::into_raw(callback) as *mut _));
        }

        fn do_shit(cycle: &mut f32, cycle_step: f32, buffer: &mut [u32; NUM_SAMPLES * 2]) {
            for dt in 0..NUM_SAMPLES {
                let phase = 2.0 * std::f32::consts::PI * *cycle;
                let sample = ((phase.sin() * 0.5 + 0.5) * std::u32::MAX as f32) as u32;

                buffer[2 * dt as usize] = sample;
                buffer[2 * dt as usize + 1] = sample;

                *cycle = (*cycle + cycle_step) % 1.0;
            }
        }

        output_stream(queue, |_| {
            cur_buffer = (cur_buffer + 1) % 2;
            // log::warn!("render rust cb {:?}", cur_buffer);
            let buffer = &mut buffers[cur_buffer];
            do_shit(&mut cycle, cycle_step, buffer);
            ((**queue).Enqueue).unwrap()(queue, buffers[cur_buffer].as_ptr() as _, (8 * NUM_SAMPLES) as _);
        });

        log::info!("state change: {:?}", ((**state).SetPlayState).unwrap()(state, sles::SL_PLAYSTATE_PLAYING));
        log::info!("enqueue: {:?}", ((**queue).Enqueue).unwrap()(queue, buffers[cur_buffer].as_ptr() as _, (8 * NUM_SAMPLES) as _));

        loop {}
        /*
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

        let mut stream = device.output_stream();
        let properties = dbg!(device.properties());

        let frequency = 100.0;
        let sample_rate = properties.sample_rate as f32;
        let num_channels = properties.num_channels;
        let cycle_step = frequency / sample_rate;
        let mut cycle = 0.0;

        device.start();

        loop {
            let (raw_buffer, num_frames) = stream.acquire_buffer(!0);
            let buffer = std::slice::from_raw_parts_mut(
                raw_buffer as *mut f32,
                num_frames as usize * num_channels,
            );

            for dt in 0..num_frames {
                let phase = 2.0 * std::f32::consts::PI * cycle;
                let sample = phase.sin() * 0.5;

                buffer[num_channels * dt as usize] = sample;
                buffer[num_channels * dt as usize + 1] = sample;

                cycle = (cycle + cycle_step) % 1.0;
            }

            stream.submit_buffer(num_frames);
        }
        */
    }
}
