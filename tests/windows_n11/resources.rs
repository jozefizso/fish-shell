//! Retained process identity and native quiescent resource counters (not host totals).
use std::{ffi::c_void, mem::{size_of, zeroed}, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle}};
#[repr(C)]
struct Memory {
    size: u32, faults: u32, peak_working: usize, working: usize,
    peak_paged: usize, paged: usize, peak_nonpaged: usize, nonpaged: usize,
    pagefile: usize, peak_pagefile: usize, private_bytes: usize,
}
#[repr(C)]
struct ThreadEntry { size: u32, usage: u32, id: u32, owner: u32, base: i32, delta: i32, flags: u32 }
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
    fn GetProcessId(process: *mut c_void) -> u32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn TerminateProcess(process: *mut c_void, code: u32) -> i32;
}
#[link(name = "psapi")]
unsafe extern "system" { fn GetProcessMemoryInfo(process: *mut c_void, memory: *mut Memory, bytes: u32) -> i32; }
pub struct Process(OwnedHandle);
#[derive(Clone, Copy, Debug)]
pub struct Sample { pub handles: u32, pub threads: u32, pub private_bytes: usize, pub working_bytes: usize }
impl Process {
    // Caller obtains this PID while its owned live console session is at a file barrier.
    pub fn retain(pid: u32) -> Self {
        let raw = unsafe { OpenProcess(0x0400 | 0x0010 | 0x0001, 0, pid) };
        assert!(!raw.is_null(), "retain owned fish identity: {}", std::io::Error::last_os_error());
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        assert_eq!(unsafe { GetProcessId(handle.as_raw_handle()) }, pid);
        Self(handle)
    }
    /// Only a process pinned while acknowledging our private owned-child barrier.
    pub fn terminate_owned(&self, code: u32) {
        assert_ne!(unsafe { TerminateProcess(self.0.as_raw_handle(), code) }, 0);
    }
    pub fn sample(&self) -> Sample {
        let mut handles = 0;
        assert_ne!(unsafe { GetProcessHandleCount(self.0.as_raw_handle(), &mut handles) }, 0);
        let mut memory: Memory = unsafe { zeroed() };
        memory.size = size_of::<Memory>() as u32;
        assert_ne!(unsafe { GetProcessMemoryInfo(self.0.as_raw_handle(), &mut memory, size_of::<Memory>() as u32) }, 0);
        let raw = unsafe { CreateToolhelp32Snapshot(4, 0) };
        assert!(raw as isize != -1 && !raw.is_null());
        let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut entry: ThreadEntry = unsafe { zeroed() };
        entry.size = size_of::<ThreadEntry>() as u32;
        let pid = unsafe { GetProcessId(self.0.as_raw_handle()) };
        let mut threads = 0;
        let mut next = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
        while next != 0 {
            if entry.owner == pid { threads += 1; }
            entry.size = size_of::<ThreadEntry>() as u32;
            next = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
        }
        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(18), "thread snapshot must reach ERROR_NO_MORE_FILES, not an observation error");
        assert!(threads > 0, "retained fish must remain alive at quiescence");
        Sample { handles, threads, private_bytes: memory.private_bytes, working_bytes: memory.working }
    }
}
