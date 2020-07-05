#[cfg(windows)]
pub mod wasapi;

#[cfg(target_os = "linux")]
pub mod pulse;

#[cfg(target_os = "android")]
pub mod opensles;

// #[cfg(target_os = "android")]
// pub mod aaudio;

#[cfg(target_os = "macos")]
pub mod coreaudio;

#[cfg(target_os = "wasm32")]
pub mod webaudio;

pub(crate) mod api;
mod handle;

pub use crate::api::*;
