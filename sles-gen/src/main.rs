fn main() {
    let bindings = bindgen::Builder::default()
        .header("headers/OpenSLES.h")
        .header("headers/OpenSLES_Platform.h")
        .header("headers/OpenSLES_Android.h")
        .generate()
        .expect("Unable to generate bindings");

    // output
    bindings
        .write_to_file("../src/opensles/sles.rs")
        .expect("Couldn't write bindings!");
}
