//! Real binary file consumers and native protocol/cancellation owners.
use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
const LIMIT: u32 = 25000;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn CreateEventW(sa: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetHandleInformation(handle: *mut c_void, mask: u32, flags: u32) -> i32;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn GetCurrentProcessId() -> u32;
    fn GetProcessTimes(process: *mut c_void, created: *mut u64, exited: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn AllocConsole() -> i32;
    fn GenerateConsoleCtrlEvent(control: u32, group: u32) -> i32;
    fn DuplicateHandle(source_process: *mut c_void, source: *mut c_void, target_process: *mut c_void,
        target: *mut *mut c_void, access: u32, inherit: i32, options: u32) -> i32;
    fn CreateJobObjectW(sa: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, class: i32, data: *const c_void, size: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn TerminateJobObject(job: *mut c_void, code: u32) -> i32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn CreateProcessW(image: *const u16, command: *mut u16, process_sa: *const c_void,
        thread_sa: *const c_void, inherit: i32, flags: u32, environment: *const c_void,
        cwd: *const u16, startup: *const Startup, information: *mut ProcessInformation) -> i32;
}

fn creation(process: &impl AsRawHandle) -> u64 {
    let (mut created, mut exited, mut kernel, mut user) = (0, 0, 0, 0);
    assert_ne!(unsafe { GetProcessTimes(process.as_raw_handle(), &mut created, &mut exited, &mut kernel, &mut user) }, 0);
    created
}
fn event(name: &OsStr) -> OwnedHandle {
    let name: Vec<_> = name.encode_wide().chain(Some(0)).collect();
    let raw = unsafe { OpenEventW(0x0010_0002, 0, name.as_ptr()) };
    assert!(!raw.is_null(), "required native barrier: {}", io::Error::last_os_error());
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn signal(event: &impl AsRawHandle) { assert_ne!(unsafe { SetEvent(event.as_raw_handle()) }, 0); }
fn wait(event: &impl AsRawHandle) { assert_eq!(unsafe { WaitForSingleObject(event.as_raw_handle(), LIMIT) }, 0, "bounded adapter barrier wait"); }
fn handles() -> u32 {
    let mut count = 0;
    assert_ne!(unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) }, 0);
    count
}
fn consume(args: &[OsString]) -> i32 {
    assert_eq!(args.len(), 8);
    let mut input = File::open(&args[0]).expect("open actual psub native path");
    let mut output = File::create(&args[6]).unwrap();
    let ready = event(&args[1]);
    let release = event(&args[2]);
    let done = event(&args[3]);
    let path = args[0].to_str().expect("strict native Unicode psub path");
    std::fs::write(&args[4], format!("{path}\n{}\n", std::process::id())).unwrap();
    let mut slab = [0; 16384];
    let count = input.read(&mut slab).unwrap();
    output.write_all(&slab[..count]).unwrap();
    signal(&ready);
    wait(&release);
    io::copy(&mut input, &mut output).unwrap();
    output.sync_all().unwrap();
    drop((input, output));
    std::fs::write(&args[5], b"consumed").unwrap();
    signal(&done);
    args[7].to_str().unwrap().parse().unwrap()
}
fn generate(args: &[OsString]) -> i32 {
    let mut remaining: usize = args[0].to_str().unwrap().parse().unwrap();
    let mut slab = [0; 65536];
    for (index, byte) in slab.iter_mut().enumerate() { *byte = (index % 256) as u8; }
    let mut out = io::stdout().lock();
    while remaining != 0 {
        let count = remaining.min(slab.len());
        out.write_all(&slab[..count]).unwrap();
        remaining -= count;
    }
    0
}
fn direct_completion(args: &[OsString], cancel: bool) -> i32 {
    assert_eq!(args.len(), 5);
    fish::threads::init();
    let _console = if cancel {
        unsafe { AllocConsole(); }
        Some(fish::signal::initialize_console_controls().unwrap())
    } else { None };
    let control = if cancel {
        let ready = event(&args[4]);
        Some(std::thread::spawn(move || {
            wait(&ready);
            assert_ne!(unsafe { GenerateConsoleCtrlEvent(1, GetCurrentProcessId()) }, 0, "request actual owner cancellation");
        }))
    } else { None };
    let runtime = fish::threads::NativeRuntime::new().unwrap();
    fish::exec::initialize_native_image_identity().unwrap();
    let unrelated = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    assert!(!unrelated.is_null());
    let unrelated = unsafe { OwnedHandle::from_raw_handle(unrelated) };
    assert_ne!(unsafe { SetHandleInformation(unrelated.as_raw_handle(), 1, 1) }, 0);
    let mut environment: Vec<_> = std::env::vars_os().collect();
    environment.push(("FISH_N8_UNRELATED_HANDLE".into(), (unrelated.as_raw_handle() as usize).to_string().into()));
    environment.push(("FISH_N8_UNRELATED_PID".into(), std::process::id().to_string().into()));
    let run = || fish::native_adapters::argcomplete(
        Path::new(&args[0]), Path::new(&args[1]), &[], args[2].to_str().unwrap(), args[3].to_str().unwrap(),
        &std::env::current_dir().unwrap(), &environment, Arc::clone(&runtime), &fish::native_limits::NativeLimits::default());
    if cancel {
        let error = run().expect_err("actual cancellation must not return completions");
        assert_eq!(error.kind(), io::ErrorKind::Interrupted, "{error}");
        control.unwrap().join().unwrap();
        assert!(runtime.drain().is_empty(), "cancel-and-drain left completion records");
        return 0;
    }
    let repeats: usize = args[4].to_str().unwrap().parse().unwrap();
    assert!(repeats != 0);
    let first = run();
    let baseline = handles();
    let mut result = first;
    for _ in 1..repeats { result = run(); }
    assert!(handles() <= baseline + 2, "completion owner leaked retained handles across repeated real children");
    assert!(runtime.drain().is_empty(), "finished completion owner left callback records");
    match result {
        Ok(bytes) => { io::stdout().write_all(&bytes).unwrap(); 0 }
        Err(error) => { eprintln!("{error}"); 1 }
    }
}
fn cancel_fish(args: &[OsString]) -> i32 {
    assert_eq!(args.len(), 3);
    unsafe { AllocConsole(); }
    let ready = event(&args[2]);
    let mut child = Command::new(&args[0]).args([OsStr::new("-N"), OsStr::new("-c"), &args[1]])
        .creation_flags(0x200).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || { let mut bytes = Vec::new(); {out}.read_to_end(&mut bytes).unwrap(); bytes });
    let err = std::thread::spawn(move || { let mut bytes = Vec::new(); {err}.read_to_end(&mut bytes).unwrap(); bytes });
    wait(&ready);
    assert_ne!(unsafe { GenerateConsoleCtrlEvent(1, child.id()) }, 0);
    if unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT) } != 0 {
        child.kill().unwrap(); child.wait().unwrap(); panic!("canceled fish did not drain owned work");
    }
    let status = child.wait().unwrap();
    let out = out.join().unwrap();
    let err = err.join().unwrap();
    assert!(!status.success(), "canceled evaluator reported success");
    io::stdout().write_all(&out).unwrap();
    io::stderr().write_all(&err).unwrap();
    0
}

struct KeeperClient {
    child: Child,
    input: Option<ChildStdin>,
    output: Option<ChildStdout>,
}
impl KeeperClient {
    fn new(partial_initial: bool) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg(fish::native_adapters::RESOURCE_LEASE_OPTION)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let mut parent = std::ptr::null_mut();
        assert_ne!(unsafe { DuplicateHandle(GetCurrentProcess(), GetCurrentProcess(), child.as_raw_handle(),
            &mut parent, 0x0010_1040, 0, 0) }, 0);
        input.write_all(b"PSUB2").unwrap();
        let bytes = (parent as usize as u64).to_le_bytes();
        input.write_all(if partial_initial { &bytes[..3] } else { &bytes }).unwrap();
        Self { child, input: Some(input), output: Some(output) }
    }
    fn response(&mut self) -> Vec<u8> {
        let mut header = [0; 5];
        self.output.as_mut().unwrap().read_exact(&mut header).unwrap();
        let size = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
        assert!(size <= 131072);
        let mut bytes = vec![0; size];
        self.output.as_mut().unwrap().read_exact(&mut bytes).unwrap();
        assert_eq!(header[0], 0, "real keeper rejected fixture: {}", String::from_utf8_lossy(&bytes));
        bytes
    }
    fn create(&mut self, suffix: &str) -> OsString {
        let input = self.input.as_mut().unwrap();
        input.write_all(&(suffix.len() as u32).to_le_bytes()).unwrap();
        input.write_all(suffix.as_bytes()).unwrap();
        let bytes: Vec<_> = (0..65537).map(|index| (index % 256) as u8).collect();
        for slab in bytes.chunks(65536) {
            input.write_all(&(slab.len() as u32).to_le_bytes()).unwrap();
            input.write_all(slab).unwrap();
        }
        input.write_all(&0u32.to_le_bytes()).unwrap();
        String::from_utf8(self.response()).unwrap().into()
    }
    fn register(&mut self, root: &SuspendedRoot, partial: bool) -> Option<Vec<u8>> {
        let input = self.input.as_mut().unwrap();
        input.write_all(b"J").unwrap();
        input.write_all(&(root.job.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
        if partial { return None; }
        input.write_all(&(root.process.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
        input.write_all(&root.pid.to_le_bytes()).unwrap();
        input.write_all(&creation(&root.process).to_le_bytes()).unwrap();
        let cookie = self.response();
        assert_eq!(cookie.len(), 8);
        Some(cookie)
    }
    fn accept(&mut self, cookie: &[u8]) {
        self.input.as_mut().unwrap().write_all(b"A").unwrap();
        self.input.as_mut().unwrap().write_all(cookie).unwrap();
        assert!(self.response().is_empty());
    }
    fn finish(mut self) -> (u32, Vec<u8>) {
        drop(self.input.take());
        let mut output = self.output.take().unwrap();
        let drain = std::thread::spawn(move || { let mut bytes = Vec::new(); output.read_to_end(&mut bytes).unwrap(); bytes });
        if unsafe { WaitForSingleObject(self.child.as_raw_handle(), LIMIT) } != 0 {
            self.child.kill().unwrap(); self.child.wait().unwrap(); panic!("bounded real keeper shutdown");
        }
        let status = self.child.wait().unwrap().code().unwrap() as u32;
        (status, drain.join().unwrap())
    }
}
impl Drop for KeeperClient {
    fn drop(&mut self) {
        drop(self.input.take());
        if self.child.try_wait().unwrap().is_none() {
            if unsafe { WaitForSingleObject(self.child.as_raw_handle(), LIMIT) } != 0 {
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
    }
}
#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    process_time: i64, job_time: i64, flags: u32, minimum: usize, maximum: usize,
    active: u32, affinity: usize, priority: u32, scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct ExtendedLimits { basic: BasicLimits, io: [u64; 6], memory: [usize; 4] }
#[repr(C)]
struct Startup {
    cb: u32, reserved: *mut u16, desktop: *mut u16, title: *mut u16,
    x: u32, y: u32, width: u32, height: u32, columns: u32, rows: u32,
    fill: u32, flags: u32, show: u16, reserved_bytes: u16, reserved_data: *mut u8,
    stdin: *mut c_void, stdout: *mut c_void, stderr: *mut c_void,
}
#[repr(C)]
struct ProcessInformation { process: *mut c_void, thread: *mut c_void, pid: u32, thread_id: u32 }
struct SuspendedRoot { job: OwnedHandle, process: OwnedHandle, thread: OwnedHandle, pid: u32 }
impl SuspendedRoot {
    fn new(arguments: &[OsString]) -> Self {
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        assert!(!job.is_null());
        let job = unsafe { OwnedHandle::from_raw_handle(job) };
        let mut limits = ExtendedLimits::default();
        limits.basic.flags = 0x2000;
        assert_ne!(unsafe { SetInformationJobObject(job.as_raw_handle(), 9,
            (&limits as *const ExtendedLimits).cast(), std::mem::size_of::<ExtendedLimits>() as u32) }, 0);
        let mut command = Vec::new();
        for argument in arguments {
            if !command.is_empty() { command.push(32); }
            command.push(34);
            let mut slashes = 0;
            for unit in argument.encode_wide() {
                assert_ne!(unit, 0);
                if unit == 92 { slashes += 1; continue; }
                command.extend(std::iter::repeat_n(92, if unit == 34 { slashes * 2 + 1 } else { slashes }));
                command.push(unit);
                slashes = 0;
            }
            command.extend(std::iter::repeat_n(92, slashes * 2));
            command.push(34);
        }
        command.push(0);
        let image: Vec<_> = arguments[0].encode_wide().chain(Some(0)).collect();
        let mut startup: Startup = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<Startup>() as u32;
        let mut info: ProcessInformation = unsafe { std::mem::zeroed() };
        assert_ne!(unsafe { CreateProcessW(image.as_ptr(), command.as_mut_ptr(), std::ptr::null(),
            std::ptr::null(), 0, 0x204, std::ptr::null(), std::ptr::null(), &startup, &mut info) }, 0);
        let process = unsafe { OwnedHandle::from_raw_handle(info.process) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.thread) };
        assert_ne!(unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) }, 0);
        Self { job, process, thread, pid: info.pid }
    }
    fn commit(&self) {
        let limits = ExtendedLimits::default();
        assert_ne!(unsafe { SetInformationJobObject(self.job.as_raw_handle(), 9,
            (&limits as *const ExtendedLimits).cast(), std::mem::size_of::<ExtendedLimits>() as u32) }, 0);
    }
    fn resume(&self) { assert_ne!(unsafe { ResumeThread(self.thread.as_raw_handle()) }, u32::MAX); }
}
impl Drop for SuspendedRoot {
    fn drop(&mut self) {
        assert_ne!(unsafe { TerminateJobObject(self.job.as_raw_handle(), 98) }, 0);
        assert_eq!(unsafe { WaitForSingleObject(self.process.as_raw_handle(), LIMIT) }, 0);
    }
}
fn keeper_bad_frame(args: &[OsString]) -> i32 {
    let label = args[0].to_str().unwrap();
    let mut keeper = KeeperClient::new(label == "initial");
    if label != "initial" {
        let suffix = format!(".{label}");
        let input = keeper.input.as_mut().unwrap();
        input.write_all(&(suffix.len() as u32).to_le_bytes()).unwrap();
        input.write_all(suffix.as_bytes()).unwrap();
        let packet: &[u8] = match label {
            "partial" => &[100,0,0,0,1,2,3],
            "oversized" => &[1,0,1,0],
            "unknown" => &[0,0,0,0,b'?'],
            "bad-job" => &[0,0,0,0,b'J',1,0,0],
            _ => panic!("unknown malformed fixture"),
        };
        input.write_all(packet).unwrap();
    }
    let (code, bytes) = keeper.finish();
    io::stdout().write_all(&bytes).unwrap();
    code as i32
}
fn keeper_parent_death(args: &[OsString]) -> i32 {
    assert_eq!(args.len(), 10);
    let phase = args[0].to_str().unwrap();
    let mut keeper = KeeperClient::new(false);
    let path = keeper.create(".parent-death");
    let image = std::env::current_exe().unwrap().into_os_string();
    let mut argv = vec![image.clone(), "n8-psub-consume".into(), path.clone()];
    argv.extend_from_slice(&args[4..10]);
    argv.push("0".into());
    let root = SuspendedRoot::new(&argv);
    let cookie = keeper.register(&root, phase == "before-ack");
    let mut second = None;
    if phase == "accept-gap" || phase == "partial-next" {
        root.commit();
        if phase == "partial-next" { keeper.accept(cookie.as_ref().unwrap()); }
        root.resume();
        wait(&event(&args[4]));
        if phase == "partial-next" {
            let mut marker = args[8].clone(); marker.push(".pending");
            let pending = SuspendedRoot::new(&[image, "n8-adapter-touch".into(), marker]);
            keeper.register(&pending, true);
            second = Some(pending);
        }
    } else if phase == "resumed-pending" {
        root.resume();
        wait(&event(&args[4]));
    } else { assert!(phase == "before-ack" || phase == "after-ack"); }
    std::fs::write(&args[3], format!("{}\n{}\n{}\n{}\n", path.to_str().unwrap(),
        root.pid, keeper.child.id(), second.as_ref().map_or(0, |root| root.pid))).unwrap();
    signal(&event(&args[1]));
    wait(&event(&args[2]));
    drop(second);
    drop(root);
    let (code, _) = keeper.finish();
    code as i32
}

fn keeper_reject_host(args: &[OsString]) -> i32 {
    assert_eq!(args.len(), 1);
    let host = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    assert!(!host.is_null());
    let host = unsafe { OwnedHandle::from_raw_handle(host) };
    assert_ne!(unsafe { AssignProcessToJobObject(host.as_raw_handle(), GetCurrentProcess()) }, 0);
    let mut keeper = KeeperClient::new(false);
    keeper.create(".reject-host");
    let image = std::env::current_exe().unwrap().into_os_string();
    let root = SuspendedRoot::new(&[image, "n8-adapter-touch".into(), args[0].clone()]);
    let input = keeper.input.as_mut().unwrap();
    input.write_all(b"J").unwrap();
    input.write_all(&(host.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
    input.write_all(&(root.process.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
    input.write_all(&root.pid.to_le_bytes()).unwrap();
    input.write_all(&creation(&root.process).to_le_bytes()).unwrap();
    let (status, _) = keeper.finish();
    assert_eq!(status, 1, "keeper accepted an enclosing/parent-containing Job");
    assert_eq!(unsafe { WaitForSingleObject(root.process.as_raw_handle(), 0) }, 258,
        "rejected registration terminated an unrelated pending fixture");
    std::fs::write(&args[0], b"parent-and-pending-root-survived-rejection").unwrap();
    drop(root);
    0
}

fn keeper_reject_identity(args: &[OsString]) -> i32 {
    assert_eq!(args.len(), 1);
    let mut keeper = KeeperClient::new(false);
    keeper.create(".reject-identity");
    let image = std::env::current_exe().unwrap().into_os_string();
    let root = SuspendedRoot::new(&[image, "n8-adapter-touch".into(), args[0].clone()]);
    let input = keeper.input.as_mut().unwrap();
    input.write_all(b"J").unwrap();
    input.write_all(&(root.job.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
    input.write_all(&(root.process.as_raw_handle() as usize as u64).to_le_bytes()).unwrap();
    input.write_all(&root.pid.to_le_bytes()).unwrap();
    input.write_all(&creation(&root.process).wrapping_add(1).to_le_bytes()).unwrap();
    let (status, _) = keeper.finish();
    assert_eq!(status, 1, "keeper accepted stale process-object identity proof");
    assert_eq!(unsafe { WaitForSingleObject(root.process.as_raw_handle(), 0) }, 258);
    assert!(!Path::new(&args[0]).exists(), "rejected suspended fixture executed");
    std::fs::write(&args[0], b"stale-proof-rejected-without-resume-or-termination").unwrap();
    drop(root);
    0
}

pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    Some(match mode {
        "--fish-native-resource-lease" => {
            assert!(args.is_empty());
            fish::native_adapters::resource_lease_entrypoint()
        }
        "n8-keeper-bad-frame" => keeper_bad_frame(args),
        "n8-psub-consume-env" => {
            let mut argv = vec![std::env::var_os("FISH_N8_INPUT").expect("literal exported substitution path")];
            argv.extend_from_slice(args);
            consume(&argv)
        }
        "n8-keeper-parent-death" => keeper_parent_death(args),
        "n8-keeper-reject-host" => keeper_reject_host(args),
        "n8-keeper-reject-identity" => keeper_reject_identity(args),
        "n8-psub-consume" => consume(args),
        "n8-psub-generate" => generate(args),
        "n8-adapter-signal" => { signal(&event(&args[0])); 0 }
        "n8-adapter-wait" => { wait(&event(&args[0])); 0 }
        "n8-adapter-touch" => { std::fs::write(&args[0], b"unexpected child effect").unwrap(); 0 }
        "n8-argcomplete-direct" => direct_completion(args, false),
        "n8-argcomplete-cancel" => direct_completion(args, true),
        "n8-psub-cancel" => cancel_fish(args),
        _ => return None,
    })
}
