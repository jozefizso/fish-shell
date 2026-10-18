mod binding;
mod decode;
#[allow(clippy::module_inception)]
mod input;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{NativeDecoder, NativeInput, WaitSources};

pub(crate) use binding::*;
pub(crate) use decode::*;
pub(crate) use input::*;
