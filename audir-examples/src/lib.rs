#[cfg(all(target_os = "android", feature = "aaudio"))]
use audir::aaudio::Instance;
#[cfg(all(target_os = "android", feature = "opensles"))]
use audir::opensles::Instance;
#[cfg(target_os = "linux")]
use audir::pulse::Instance;
#[cfg(windows)]
use audir::wasapi::Instance;

#[cfg(feature = "music")]
mod music;
#[cfg(feature = "music")]
pub use crate::music::run;