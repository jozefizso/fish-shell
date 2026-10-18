//! Observe actual shell CPU while its main-thread function is blocked in wait.
use std::ffi::c_void;
use std::os::windows::io::{AsHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::time::{Duration, Instant};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
}

pub fn assert_wait_is_idle(pid_file: &Path) {
    let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
    // The shell published this identity immediately before its acknowledged
    // controlled wait, and the harness has not released that wait. Query only;
    // cleanup remains with the launcher, never a PID-only termination.
    let raw = unsafe { OpenProcess(0x1000, 0, pid) };
    assert!(
        !raw.is_null(),
        "query owned blocked shell: {}",
        std::io::Error::last_os_error()
    );
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    let before = fish::native_accounting::process_times(process.as_handle())
        .unwrap()
        .cpu;
    let start = fish::native_accounting::current_process_times()
        .unwrap()
        .cpu;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut work = 1u64;
    loop {
        for n in 0..65536u64 {
            work = work
                .rotate_left(7)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(n);
        }
        std::hint::black_box(work);
        let delta = fish::native_accounting::current_process_times()
            .unwrap()
            .cpu
            .checked_sub(start)
            .unwrap();
        if delta.user + delta.kernel >= Duration::from_millis(400) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "required real CPU observation could not finish within bound"
        );
    }
    let after = fish::native_accounting::process_times(process.as_handle())
        .unwrap()
        .cpu;
    let delta = after.checked_sub(before).unwrap();
    assert!(
        delta.user + delta.kernel < Duration::from_millis(100),
        "under-construction completion queue spun while internal wait was blocked: {delta:?}"
    );
}
