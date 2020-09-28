#[cfg(windows)]
pub mod wasapi;

#[cfg(target_os = "linux")]
pub mod pulse;

#[cfg(target_os = "android")]
pub mod opensles;

#[cfg(target_os = "android")]
pub mod aaudio;

pub mod null;

pub(crate) mod api;
mod handle;

pub use crate::api::*;
