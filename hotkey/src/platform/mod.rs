#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(windows)]
pub use windows::*;
#[cfg(target_os = "macos")]
pub use macos::*;
