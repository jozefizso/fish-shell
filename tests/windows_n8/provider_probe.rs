//! Real production-provider launches and synchronized sharing/lifetime consumers.
use std::ffi::{OsStr, OsString, c_void};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use std::sync::Arc;
use fish_widestring::WString;
#[path = "provider_sync.rs"]
mod sync;

#[repr(C)]
struct Startup {
    size: u32, reserved: *mut u16, desktop: *mut u16, title: *mut u16,
    x: u32, y: u32, width: u32, height: u32, columns: u32, rows: u32, fill: u32,
    flags: u32, show: u16, reserved_size: u16, reserved_bytes: *mut u8,
    stdin: *mut c_void, stdout: *mut c_void, stderr: *mut c_void,
}
#[repr(C)]
struct Information { process: *mut c_void, thread: *mut c_void, pid: u32, tid: u32 }
#[repr(C)]
struct BasicInformation {
    exit_status: isize, peb: *mut c_void, affinity: usize, priority: isize,
    pid: usize, parent: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn GetCurrentProcess() -> *mut c_void;
    fn CreateProcessW(application: *const u16, line: *mut u16,
        process_security: *const c_void, thread_security: *const c_void, inherit: i32,
        flags: u32, environment: *const c_void, cwd: *const u16,
        startup: *const Startup, information: *mut Information) -> i32;
    fn GetStdHandle(kind: u32) -> *mut c_void;
    fn SetHandleInformation(handle: *mut c_void, mask: u32, flags: u32) -> i32;
    fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(process: *mut c_void, class: u32, info: *mut c_void,
        size: u32, returned: *mut u32) -> i32;
}
fn parent_pid() -> u32 {
    let mut info: BasicInformation = unsafe { std::mem::zeroed() };
    assert!(unsafe { NtQueryInformationProcess(GetCurrentProcess(), 0,
        (&mut info as *mut BasicInformation).cast(), std::mem::size_of::<BasicInformation>() as u32,
        std::ptr::null_mut()) } >= 0);
    u32::try_from(info.parent).unwrap()
}
fn wide(value: &OsStr) -> Vec<u16> { value.encode_wide().chain(Some(0)).collect() }
fn raw_provider(args: &[OsString]) {
    assert_eq!(args.len(), 2);
    fish::threads::init();
    let path = PathBuf::from(&args[0]);
    let expected: u32 = args[1].to_str().unwrap().parse().unwrap();
    let text: WString = path.to_str().unwrap().into();
    let command = fish::path::native_classify_command(&text).expect("required installed provider");
    let invocation = fish::path::native_plan_command(command,
        vec![path.into_os_string()], std::env::current_dir().unwrap(), std::env::vars_os().collect()).unwrap();
    let runtime = fish::threads::NativeRuntime::new().unwrap();
    let mut group = fish::exec::NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let plan = group.prepare_provider(invocation, &fish::io::IoChain::new()).unwrap();
    let child = group.spawn_prepared(plan).unwrap();
    let wait = unsafe { WaitForSingleObject(child.handle().as_raw_handle(), 25_000) };
    if wait != 0 {
        group.terminate_and_wait().expect("owned provider rollback");
        panic!("provider did not complete inside the bound: {wait:#x}");
    }
    assert_eq!(child.wait(None).unwrap(), expected, "provider raw DWORD changed");
    group.finish().unwrap();
}
fn ancestor(args: &[OsString]) {
    assert_eq!(args.len(), 4);
    fish::threads::init();
    let path = PathBuf::from(&args[0]);
    let text: WString = path.to_str().unwrap().into();
    let command = fish::path::native_classify_command(&text).expect("classify and lock actual provider");
    // Signal only after classification acquired the real directory and script leases.
    sync::Event::open(&args[1]).signal();
    sync::Event::open(&args[2]).wait();
    let mut invocation = fish::path::native_plan_command(command,
        vec![path.into_os_string()], std::env::current_dir().unwrap(), std::env::vars_os().collect()).unwrap();
    // An invocation that attempts to redirect script consumption to another real object must
    // fail even though that object's extension/interpreter is otherwise supported.
    invocation.script = Some(PathBuf::from(&args[3]));
    let runtime = fish::threads::NativeRuntime::new().unwrap();
    let group = fish::exec::NativeLaunchGroup::new(runtime).unwrap();
    let error = match group.prepare_provider(invocation, &fish::io::IoChain::new()) {
        Ok(_) => panic!("provider accepted a different script than its retained pin"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("script identity"));
}
fn barrier(args: &[OsString]) {
    assert_eq!(args.len(), 3);
    // Root provider and this helper are both alive behind release, making initial retained
    // process opens unambiguous. No process enumeration or unrelated process termination.
    std::fs::write(&args[2], format!("{}\n{}", parent_pid(), std::process::id())).unwrap();
    sync::Event::open(&args[0]).signal();
    sync::Event::open(&args[1]).wait();
}
fn survivor(args: &[OsString]) {
    assert_eq!(args.len(), 3);
    std::fs::write(&args[2], std::process::id().to_string()).unwrap();
    sync::Event::open(&args[0]).signal();
    sync::Event::open(&args[1]).wait();
}
fn spawn_survivor(args: &[OsString]) {
    assert_eq!(args.len(), 3);
    // PowerShell's explicit foreground fixture has a stdout capture pipe.
    // It is not a provider lease and must not make its EOF depend on our
    // deliberately surviving grandchild. Keep generic inheritance enabled for
    // every OTHER inheritable handle, including any erroneous provider lease.
    for kind in [-10i32, -11, -12] {
        let handle = unsafe { GetStdHandle(kind as u32) };
        if !handle.is_null() && handle != (-1isize) as *mut c_void {
            assert_ne!(unsafe { SetHandleInformation(handle, 1, 0) }, 0);
        }
    }
    let image = std::env::current_exe().unwrap();
    // Intentionally generic bInheritHandles=TRUE, no attribute-list restriction: any provider
    // lease erroneously inherited by this native helper would propagate into the survivor.
    let mut words = vec![image.as_os_str().to_owned(), "n8-provider-survivor".into()];
    words.extend_from_slice(args);
    let mut line = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let word = word.to_str().unwrap();
        assert!(!word.contains('"') && !word.ends_with('\\'));
        if index != 0 { line.push(32); }
        line.push(34);
        line.extend(word.encode_utf16());
        line.push(34);
    }
    line.push(0);
    let mut startup: Startup = unsafe { std::mem::zeroed() };
    startup.size = std::mem::size_of::<Startup>() as u32;
    let mut info: Information = unsafe { std::mem::zeroed() };
    assert_ne!(unsafe { CreateProcessW(wide(image.as_os_str()).as_ptr(), line.as_mut_ptr(),
        std::ptr::null(), std::ptr::null(), 1, 0x0800_0000, std::ptr::null(),
        std::ptr::null(), &startup, &mut info) }, 0, "{}", std::io::Error::last_os_error());
    let _process = unsafe { OwnedHandle::from_raw_handle(info.process) };
    let _thread = unsafe { OwnedHandle::from_raw_handle(info.thread) };
    sync::Event::open(&args[0]).wait();
}

fn ignored_control(args: &[OsString]) {
    assert_eq!(args.len(), 3);
    // Publish only our own creator and current process while the real N6
    // handler/readiness/release protocol keeps both lifetimes unambiguous.
    let root = PathBuf::from(&args[0]);
    std::fs::write(root.join("provider-native-pids"),
        format!("{}\n{}\n", parent_pid(), std::process::id())).unwrap();
    super::windows_native_n6_cases::run("n6-case-ignore-control", args.to_vec());
    // The direct N6 case deliberately leaves its changed modes for fish. Here
    // cmd/PowerShell still owns a subsequent native confirmation/debugger read,
    // so this cooperating nested child returns the provider's own donated modes.
    // It does not restore the paused fish editor's modes or VT protocols.
    let original = std::fs::read_to_string(root.join("child-original-modes")).unwrap();
    let (input, output) = original.split_once(':').unwrap();
    let expected = [input.parse::<u32>().unwrap(), output.parse::<u32>().unwrap()];
    let mut restored = [0u32; 2];
    for (index, name) in ["CONIN$", "CONOUT$"].iter().enumerate() {
        let file = std::fs::OpenOptions::new().read(true).write(true).open(name).unwrap();
        assert_ne!(unsafe { SetConsoleMode(file.as_raw_handle(), expected[index]) }, 0);
        assert_ne!(unsafe { GetConsoleMode(file.as_raw_handle(), &mut restored[index]) }, 0);
        assert_eq!(restored[index], expected[index]);
    }
    std::fs::write(root.join("provider-native-restored-modes"),
        format!("{}:{}", restored[0], restored[1])).unwrap();
}

pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    match mode {
        "n8-provider-raw" => raw_provider(args),
        "n8-provider-ancestor" => ancestor(args),
        "n8-provider-ignore-control" => ignored_control(args),
        "n8-provider-barrier" => barrier(args),
        "n8-provider-survivor" => survivor(args),
        "n8-provider-spawn-survivor" => spawn_survivor(args),
        "n8-provider-wait" => { assert_eq!(args.len(), 1); sync::Event::open(&args[0]).wait(); }
        "n8-provider-signal" => { assert_eq!(args.len(), 1); sync::Event::open(&args[0]).signal(); }
        _ => return None,
    }
    Some(0)
}
