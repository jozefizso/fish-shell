mod history_search;

#[cfg(windows)]
mod batch_windows;
#[cfg(windows)]
pub use batch_windows::{read_ni, reader_read};

mod input;
pub mod iothreads;
#[allow(clippy::module_inception)]
mod reader;

mod word_motion;

pub use reader::*;
