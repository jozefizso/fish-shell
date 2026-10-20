//! Real child lifecycle and CPU consumers for N8 service acceptance.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    match mode {
        "n8-services-event" => {
            use std::os::windows::ffi::OsStrExt;
            use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut std::ffi::c_void;
                fn SetEvent(event: *mut std::ffi::c_void) -> i32;
                fn WaitForSingleObject(event: *mut std::ffi::c_void, millis: u32) -> u32;
            }
            assert_eq!(args.len(), 2);
            let open = |name: &OsString| {
                let name: Vec<_> = name.encode_wide().chain(Some(0)).collect();
                let event = unsafe { OpenEventW(0x100002, 0, name.as_ptr()) };
                assert!(!event.is_null(), "{}", std::io::Error::last_os_error());
                unsafe { OwnedHandle::from_raw_handle(event) }
            };
            let ready = open(&args[0]);
            let release = open(&args[1]);
            assert_ne!(unsafe { SetEvent(ready.as_raw_handle()) }, 0);
            assert_eq!(unsafe { WaitForSingleObject(release.as_raw_handle(), 30000) }, 0);
            Some(0)
        }
        "n8-services-hold" => {
            assert!(args.is_empty());
            println!("{}", std::process::id());
            std::io::stdout().flush().unwrap();
            let mut byte = [0];
            std::io::stdin().read_exact(&mut byte).unwrap();
            assert_eq!(byte, [b'!']);
            Some(0)
        }
        "n8-services-cpu" => {
            assert!(args.is_empty());
            let before = fish::native_accounting::current_process_times().unwrap().cpu;
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut value = 17u64;
            loop {
                for _ in 0..100000 { value = value.wrapping_mul(6364136223846793005).wrapping_add(1); }
                std::hint::black_box(value);
                let cpu = fish::native_accounting::current_process_times().unwrap().cpu.checked_sub(before).unwrap();
                if cpu.user + cpu.kernel >= Duration::from_millis(40) {
                    println!("{}", (cpu.user + cpu.kernel).as_nanos());
                    return Some(0);
                }
                assert!(Instant::now() < deadline, "native accounting did not observe bounded CPU work");
            }
        }
        _ => None,
    }
}
