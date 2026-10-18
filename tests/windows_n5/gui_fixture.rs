//! The same real barrier child, with a GUI PE subsystem and no console attachment.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
#[path = "cancel_probe.rs"]
mod cancel_probe;
#[cfg(windows)]
#[path = "../windows_native/conpty.rs"]
mod conpty;
#[cfg(windows)]
fn main() {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().expect("required GUI probe mode");
    cancel_probe::run(mode.to_str().expect("Unicode mode"), args.collect());
}
#[cfg(not(windows))]
fn main() {
    panic!("windows_n5_gui_probe requires native Windows");
}
