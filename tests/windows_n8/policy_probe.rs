//! Real kernel consumers for N8 quota and scope cases. Routed by the native probe.
use std::ffi::{OsStr, OsString, c_void};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::{Command, Stdio};
use std::ptr;
use std::time::{Duration, Instant};
type Handle = *mut c_void;
const MIB: usize = 1024 * 1024;
#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    process_time: i64, job_time: i64, flags: u32,
    min_working_set: usize, max_working_set: usize, active: u32,
    affinity: usize, priority: u32, scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct Limits {
    basic: BasicLimits, io: [u64; 6],
    process_memory: usize, job_memory: usize, peak_process: usize, peak_job: usize,
}
#[repr(C)]
#[derive(Default)]
struct FileTime { low: u32, high: u32 }
impl FileTime {
    fn ticks(&self) -> u64 { u64::from(self.low) | (u64::from(self.high) << 32) }
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn VirtualAlloc(address: *mut c_void, size: usize, flags: u32, protection: u32) -> *mut c_void;
    fn VirtualFree(address: *mut c_void, size: usize, flags: u32) -> i32;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn GetProcessTimes(process: Handle, creation: *mut FileTime, exit: *mut FileTime,
        kernel: *mut FileTime, user: *mut FileTime) -> i32;
    fn CreateJobObjectW(security: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, data: *const c_void, size: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn GetCurrentProcess() -> Handle;
}
fn own(raw: Handle) -> OwnedHandle {
    assert!(!raw.is_null(), "native quota probe: {}", io::Error::last_os_error());
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn event(name: &OsStr) -> OwnedHandle {
    let name: Vec<_> = name.encode_wide().chain(Some(0)).collect();
    own(unsafe { OpenEventW(0x0010_0002, 0, name.as_ptr()) })
}
fn wait(event: &OwnedHandle) {
    assert_eq!(unsafe { WaitForSingleObject(event.as_raw_handle(), 20_000) }, 0,
        "quota probe barrier did not complete");
}
struct Memory(Vec<*mut c_void>);
impl Memory {
    fn new() -> Self { Self(Vec::new()) }
    fn commit(&mut self) -> Result<(), i32> {
        let block = unsafe { VirtualAlloc(ptr::null_mut(), MIB, 0x3000, 4) };
        if block.is_null() {
            return Err(io::Error::last_os_error().raw_os_error().expect("Win32 allocation error"));
        }
        // Commit is the measured resource, not merely reservation or Vec capacity.
        unsafe { block.cast::<u8>().write_volatile(0x8a); }
        self.0.push(block);
        Ok(())
    }
    fn bytes(&self) -> usize { self.0.len() * MIB }
}
impl Drop for Memory {
    fn drop(&mut self) {
        for &block in &self.0 { assert_ne!(unsafe { VirtualFree(block, 0, 0x8000) }, 0); }
    }
}
fn allocate(cap_mib: usize) -> (Memory, Option<i32>) {
    let mut memory = Memory::new();
    for _ in 0..cap_mib {
        if let Err(error) = memory.commit() { return (memory, Some(error)); }
    }
    (memory, None)
}
fn report(memory: &Memory, error: Option<i32>) {
    println!("committed={}", memory.bytes());
    match error {
        Some(error) => println!("denied={error}"),
        None => println!("cap-reached=true"),
    }
}
fn cpu_busy() -> i32 {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut value = 0x9e37_79b9u64;
    while Instant::now() < deadline {
        for _ in 0..65_536 {
            value = std::hint::black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
    }
    std::hint::black_box(value);
    87 // A missing kernel CPU limit must not resemble quota termination.
}
fn cpu_parent() -> i32 {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("n8-cpu-busy").stdin(Stdio::null()).stdout(Stdio::null()).spawn().unwrap();
    let result = unsafe { WaitForSingleObject(child.as_raw_handle(), 20_000) };
    if result != 0 {
        let _ = child.kill();
        let _ = child.wait();
        panic!("bounded USER CPU child wait failed: {result}");
    }
    let mut creation = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    assert_ne!(unsafe { GetProcessTimes(child.as_raw_handle(), &mut creation, &mut exit, &mut kernel, &mut user) }, 0);
    let raw = child.wait().unwrap().code().unwrap() as u32;
    println!("raw={raw}");
    println!("user-100ns={}", user.ticks());
    0
}
pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    Some(match mode {
        "n8-memory" => {
            let (memory, error) = allocate(args[0].to_str().unwrap().parse().unwrap());
            report(&memory, error);
            0
        }
        "n8-job-hold" => {
            let ready = event(&args[0]);
            let release = event(&args[1]);
            let amount = args.get(3).map(|v| v.to_str().unwrap().parse().unwrap()).unwrap_or(48);
            let (memory, error) = allocate(amount);
            assert_eq!(error, None, "holding child must commit its requested MiB");
            std::fs::write(&args[2], memory.bytes().to_string()).unwrap();
            assert_ne!(unsafe { SetEvent(ready.as_raw_handle()) }, 0);
            wait(&release);
            0
        }
        "n8-job-fill" => {
            let ready = event(&args[0]);
            let release = event(&args[1]);
            wait(&ready);
            let (memory, error) = allocate(192);
            report(&memory, error);
            assert_ne!(unsafe { SetEvent(release.as_raw_handle()) }, 0);
            0
        }
        "n8-quota-marker" => {
            std::fs::write(&args[0], b"entered").unwrap();
            if args.len() == 2 { wait(&event(&args[1])); }
            0
        }
        "n8-cpu-busy" => cpu_busy(),
        "n8-cpu-parent" => cpu_parent(),
        "n8-ancestor-query" => {
            // Only this fixture process joins the deliberately constrained ancestor.
            // Fish inherits it; its own configured-default query must still say none.
            let job = own(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
            let mut limits = Limits::default();
            limits.basic.flags = 0x200; // JOB_OBJECT_LIMIT_JOB_MEMORY
            limits.job_memory = 256 * MIB;
            assert_ne!(unsafe { SetInformationJobObject(job.as_raw_handle(), 9,
                (&limits as *const Limits).cast(), size_of::<Limits>() as u32) }, 0);
            assert_ne!(unsafe { AssignProcessToJobObject(job.as_raw_handle(), GetCurrentProcess()) }, 0,
                "required nested Job fixture could not be provisioned: {}", io::Error::last_os_error());
            let mut child = Command::new(&args[0]).args(["-N", "-c", "ulimit --all"])
                .stdin(Stdio::null()).spawn().unwrap();
            let result = unsafe { WaitForSingleObject(child.as_raw_handle(), 20_000) };
            if result != 0 {
                let _ = child.kill(); let _ = child.wait();
                panic!("bounded ancestor fish wait failed: {result}");
            }
            child.wait().unwrap().code().unwrap()
        }
        "n8-release-event" => {
            wait(&event(&args[0]));
            assert_ne!(unsafe { SetEvent(event(&args[1]).as_raw_handle()) }, 0);
            0
        }
        "n8-device" => {
            use std::io::{Read, Write};
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes).unwrap();
            assert!(bytes.is_empty(), "NUL input must reach immediate EOF");
            std::io::stdout().write_all(b"native NUL consumer\\0\\xff").unwrap();
            0
        }
        _ => return None,
    })
}
