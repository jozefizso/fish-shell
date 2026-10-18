#[cfg(windows)]
pub(crate) mod ansi;
mod file_tester;
#[allow(clippy::module_inception)]
mod highlight;
pub use highlight::*;
