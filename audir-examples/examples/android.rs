#[cfg_attr(target_os = "android", ndk_glue::main(backtrace))]
pub fn main() {
    audir_examples::run().unwrap();
}
