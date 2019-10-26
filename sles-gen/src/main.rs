fn main() {
    let bindings = bindgen::Builder::default()
        .raw_line("#![allow(non_camel_case_types)]")
        .raw_line("#![allow(non_snake_case)]")
        .layout_tests(false)
        .header("headers/OpenSLES.h")
        .header("headers/OpenSLES_Android.h")
        // TODO: not sure about these types but appears not valid
        .blacklist_type("sl_uint32_t")
        .blacklist_type("sl_int32_t")
        .raw_line("pub type sl_int32_t = i32;")
        .raw_line("pub type sl_uint32_t = u32;")
        .generate()
        .expect("Unable to generate bindings");

    // output
    bindings
        .write_to_file("../sles/src/lib.rs")
        .expect("Couldn't write bindings!");
}
