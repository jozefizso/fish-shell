//! Native identity for prompt initialization, without executable or environment guesses.
//!
//! These names describe the current Windows user and DNS host. Windows has no POSIX
//! uid 0: neither account spelling nor administrator-token elevation makes a user
//! Unix root. This provider deliberately does not manufacture USER ids or EUID.
use fish_widestring::{WString, native_to_shell_text_owned, utf16_to_wcstring};
use std::io;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetUserNameW(buffer: *mut u16, length: *mut u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetComputerNameExW(format: i32, buffer: *mut u16, length: *mut u32) -> i32;
}

fn decode_name(units: &[u16]) -> io::Result<WString> {
    if units.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native identity is empty",
        ));
    }
    let name = utf16_to_wcstring(units)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(native_to_shell_text_owned(name))
}

/// Query the effective Windows user's account name. Call when USER was not
/// explicitly inherited, preserving fish's existing exported USER override policy.
pub fn username() -> io::Result<WString> {
    // UNLEN is 256 UTF-16 code units, excluding the terminating NUL.
    let mut buffer = [0u16; 257];
    let mut length = buffer.len() as u32;
    if unsafe { GetUserNameW(buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // GetUserNameW includes its NUL in the returned length.
    let length = length as usize;
    if length == 0 || length > buffer.len() || buffer[length - 1] != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GetUserNameW returned an invalid name length",
        ));
    }
    decode_name(&buffer[..length - 1])
}

/// Query the local computer's DNS hostname, never an inherited hostname variable.
pub fn hostname() -> io::Result<WString> {
    const COMPUTER_NAME_DNS_HOSTNAME: i32 = 1;
    // A DNS name has at most 255 UTF-16 code units, plus a terminating NUL.
    let mut buffer = [0u16; 256];
    let mut length = buffer.len() as u32;
    if unsafe { GetComputerNameExW(COMPUTER_NAME_DNS_HOSTNAME, buffer.as_mut_ptr(), &mut length) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    // Unlike GetUserNameW, GetComputerNameExW excludes its NUL from the length.
    let length = length as usize;
    if length >= buffer.len() || buffer[length] != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GetComputerNameExW returned an invalid name length",
        ));
    }
    decode_name(&buffer[..length])
}
