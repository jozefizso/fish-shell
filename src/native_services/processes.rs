//! A Toolhelp observation is not a process owner. PIDs can be reused after observation.
use super::{Handle, decode};
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    image: [u16; 260],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
}

#[derive(Debug)]
pub struct ProcessEntrySnapshot {
    pub pid: u32,
    pub parent_pid: u32,
    pub image: String,
    /// Protected/system/exited processes can be listed without permission to query an image.
    /// This is a per-entry capability error, never a fabricated path.
    pub full_image: io::Result<String>,
}

pub fn image(pid: u32) -> io::Result<String> {
    query_image(pid, &mut vec![0u16; 32768])
}

fn query_image(pid: u32, buffer: &mut [u16]) -> io::Result<String> {
    let handle = unsafe { OpenProcess(0x1000, 0, pid) }; // QUERY_LIMITED_INFORMATION
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut length = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.as_raw_handle(), 0, buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error());
    }
    decode(&buffer[..length as usize])
}

/// Enumerates all visible processes, including this process and its owned children.
/// No debug privilege is requested and an inaccessible image is retained as an explicit error.
pub fn snapshot() -> io::Result<Vec<ProcessEntrySnapshot>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(2, 0) };
    if snapshot as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry: ProcessEntry = unsafe { std::mem::zeroed() };
    entry.size = size_of::<ProcessEntry>() as u32;
    let mut present = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    let mut entries = Vec::new();
    let mut image_buffer = vec![0u16; 32768];
    while present != 0 {
        let length = entry.image.iter().position(|&c| c == 0).unwrap_or(entry.image.len());
        entries.push(ProcessEntrySnapshot {
            pid: entry.pid,
            parent_pid: entry.parent,
            image: decode(&entry.image[..length])?,
            full_image: query_image(entry.pid, &mut image_buffer),
        });
        present = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(18) { // ERROR_NO_MORE_FILES, including an empty observation
        return Err(error);
    }
    entries.sort_unstable_by_key(|entry| entry.pid);
    Ok(entries)
}
