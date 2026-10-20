//! Native helper services. These are explicit Windows protocols, not Unix utility emulation.
pub mod accounts;
pub mod network;
pub mod processes;
pub mod desktop;

use std::ffi::c_void;
use std::io;

pub(crate) type Handle = *mut c_void;

pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn wide(text: &str) -> io::Result<Vec<u16>> {
    if text.contains('\0') {
        return Err(invalid("native service argument contains NUL"));
    }
    Ok(text.encode_utf16().chain(Some(0)).collect())
}

pub(crate) fn decode(units: &[u16]) -> io::Result<String> {
    String::from_utf16(units).map_err(|_| invalid("native service returned invalid UTF-16"))
}

/// Only for NUL-terminated strings inside an API-owned allocation with a documented lifetime.
pub(crate) unsafe fn text(pointer: *const u16) -> io::Result<String> {
    if pointer.is_null() {
        return Ok(String::new());
    }
    let mut len = 0;
    unsafe {
        while *pointer.add(len) != 0 {
            len += 1;
        }
        decode(std::slice::from_raw_parts(pointer, len))
    }
}

pub(crate) fn unsupported(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message)
}
