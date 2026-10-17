//! Target-specific startup; the native batch path never initializes the Unix reader.
#[cfg(unix)]
#[path = "fish/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "fish/windows.rs"]
mod platform;

fn main() {
    platform::main();
}
