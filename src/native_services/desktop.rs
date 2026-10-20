//! Windows desktop/volume helpers; targets are data, never shell command strings.
use super::{decode, invalid, wide};
use std::ffi::c_void;
use std::io;
use std::path::Path;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLogicalDriveStringsW(size: u32, buffer: *mut u16) -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(window: *mut c_void, operation: *const u16, file: *const u16,
        parameters: *const u16, directory: *const u16, show: i32) -> isize;
}
pub fn drives() -> io::Result<Vec<(String, &'static str)>> {
    let needed = unsafe { GetLogicalDriveStringsW(0, std::ptr::null_mut()) };
    if needed == 0 { return Err(io::Error::last_os_error()); }
    let mut buffer = vec![0; needed as usize + 1];
    let length = unsafe { GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr()) };
    if length == 0 { return Err(io::Error::last_os_error()); }
    if length as usize >= buffer.len() { return Err(invalid("logical drive list changed during query")); }
    let mut result = Vec::new();
    for root in buffer[..length as usize].split(|unit| *unit == 0).filter(|root| !root.is_empty()) {
        let name = decode(root)?;
        let root = wide(&name)?;
        let kind = match unsafe { GetDriveTypeW(root.as_ptr()) } {
            0 => "unknown", 1 => "unavailable", 2 => "removable", 3 => "fixed",
            4 => "network", 5 => "optical", 6 => "RAM disk", _ => return Err(invalid("unknown native drive type")),
        };
        result.push((name, kind));
    }
    Ok(result)
}
/// Deliberately request the registered desktop open verb. Each operand is one
/// filename/URI; no arguments, shell switches, or command-language fallback.
pub fn open(target: &str, cwd: &Path) -> io::Result<()> {
    if target.is_empty() { return Err(invalid("open requires a nonempty filename or URI")); }
    let target = wide(target)?;
    let cwd = wide(cwd.to_str().ok_or_else(|| invalid("open cwd is not strict Unicode"))?)?;
    let verb = wide("open")?;
    let status = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), target.as_ptr(),
        std::ptr::null(), cwd.as_ptr(), 1) };
    if status <= 32 { return Err(io::Error::other(format!("Windows desktop open failed (ShellExecute code {status})"))); }
    Ok(())
}
