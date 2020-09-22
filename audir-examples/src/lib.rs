pub mod instance;

#[cfg(feature = "music")]
mod music;
#[cfg(feature = "music")]
pub use crate::music::run;
