#[cfg_attr(target_os = "android", ndk_glue::main(backtrace))]
fn run() {
    let native_activity = ndk_glue::native_activity();
    let vm_ptr = native_activity.vm();
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) }.unwrap();
    let env = vm.attach_current_thread().unwrap();
    let class_ctxt = dbg!(env.find_class("android/content/Context")).unwrap();
    let audio_service = env
        .get_static_field(class_ctxt, "AUDIO_SERVICE", "Ljava/lang/String;")
        .unwrap();

    let audio_manager = dbg!(env.call_method(
        ndk_glue::native_activity().activity(),
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[audio_service]
    ))
    .unwrap()
    .l()
    .unwrap();

    let devices = dbg!(env.call_method(
        audio_manager,
        "getDevices",
        "(I)[Landroid/media/AudioDeviceInfo;",
        &[dbg!(2.into())]
    ))
    .unwrap();

    let device_array = devices.l().unwrap().into_inner();
    let len = dbg!(env.get_array_length(device_array)).unwrap();
    for i in 0..len {
        let device = env.get_object_array_element(device_array, i).unwrap();
        let ty = env.call_method(device, "getType", "()I", &[]).unwrap();
        let ty_desc = match ty.i().unwrap() {
            19 => "TYPE_AUX_LINE",
            8 => "TYPE_BLUETOOTH_A2DP",
            7 => "TYPE_BLUETOOTH_SCO",
            1 => "TYPE_BUILTIN_EARPIECE",
            15 => "TYPE_BUILTIN_MIC",
            2 => "TYPE_BUILTIN_SPEAKER",
            24 => "TYPE_BUILTIN_SPEAKER_SAFE",
            21 => "TYPE_BUS",
            13 => "TYPE_DOCK",
            14 => "TYPE_FM",
            16 => "TYPE_FM_TUNER",
            9 => "TYPE_HDMI",
            10 => "TYPE_HDMI_ARC",
            23 => "TYPE_HEARING_AID",
            20 => "TYPE_IP",
            5 => "TYPE_LINE_ANALOG",
            6 => "TYPE_LINE_DIGITAL",
            18 => "TYPE_TELEPHONY",
            17 => "TYPE_TV_TUNER",
            12 => "TYPE_USB_ACCESSORY",
            11 => "TYPE_USB_DEVICE",
            22 => "TYPE_USB_HEADSET",
            4 => "TYPE_WIRED_HEADPHONES",
            3 => "TYPE_WIRED_HEADSET",

            _ => "-",
        };
        println!("{:?}", ty_desc);
        let name = env
            .call_method(device, "getProductName", "()Ljava/lang/CharSequence;", &[])
            .unwrap();
        let name = env
            .call_method(name.l().unwrap(), "toString", "()Ljava/lang/String;", &[])
            .unwrap();
        let product_name: String = env.get_string(name.l().unwrap().into()).unwrap().into();
        dbg!(product_name);
    }
}

fn main() {
    run();
}
