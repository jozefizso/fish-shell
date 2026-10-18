//! Real process/Job Object accounting fixtures. No enclosing host job is modified.
use fish::native_accounting::{self as account, CpuTimes};
use fish::timer::TimerSnapshot;
use std::ffi::{OsStr, OsString, c_void};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Handle = *mut c_void;
const LIMIT: Duration = Duration::from_secs(25);
#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}
#[repr(C)]
struct StartupInfo {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    stdin: Handle,
    stdout: Handle,
    stderr: Handle,
}
#[repr(C)]
struct ProcessInfo {
    process: Handle,
    thread: Handle,
    pid: u32,
    tid: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, code: u32) -> i32;
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: Handle,
        thread_attributes: Handle,
        inherit: i32,
        flags: u32,
        environment: Handle,
        directory: *const u16,
        startup: *mut StartupInfo,
        info: *mut ProcessInfo,
    ) -> i32;
    fn ResumeThread(thread: Handle) -> u32;
    fn TerminateProcess(process: Handle, code: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn GetProcessTimes(
        process: Handle,
        created: *mut FileTime,
        exited: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> Handle;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: Handle, size: u32) -> i32;
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn owned(raw: Handle) -> OwnedHandle {
    assert!(
        !raw.is_null(),
        "required Win32 fixture: {}",
        std::io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn wait(handle: &impl AsRawHandle) {
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "bounded owned-handle wait: {}",
        std::io::Error::last_os_error()
    );
}
struct Event {
    handle: OwnedHandle,
    name: OsString,
}
impl Event {
    fn new(label: &str) -> Self {
        let name = OsString::from(format!(
            "Local\\fish-n5-account-{}-{label}",
            std::process::id()
        ));
        let handle = owned(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(&name).as_ptr()) });
        Self { handle, name }
    }
    fn open(name: &OsStr) -> Self {
        let handle = owned(unsafe { OpenEventW(0x00100002, 0, wide(name).as_ptr()) });
        Self {
            handle,
            name: name.to_owned(),
        }
    }
    fn signal(&self) {
        assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0);
    }
    fn wait(&self) {
        wait(&self.handle);
    }
}
struct Job(Arc<OwnedHandle>);
impl Job {
    fn new() -> Self {
        Self(Arc::new(owned(unsafe {
            CreateJobObjectW(std::ptr::null(), std::ptr::null())
        })))
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        // Only the fresh, unnamed job created above. Never the host/CI containment.
        assert_ne!(unsafe { TerminateJobObject(self.0.as_raw_handle(), 91) }, 0);
    }
}
struct Process {
    handle: OwnedHandle,
}
impl Process {
    fn spawn(job: &Job, args: &[&OsStr]) -> Self {
        let exe = std::env::current_exe().unwrap();
        let mut command = OsString::from("\"");
        command.push(&exe);
        command.push("\"");
        for arg in args {
            // Fixture arguments are modes or generated event identifiers, not user paths.
            assert!(!arg.to_string_lossy().contains([' ', '"']));
            command.push(" ");
            command.push(arg);
        }
        let mut command = wide(&command);
        let mut startup: StartupInfo = unsafe { std::mem::zeroed() };
        startup.size = std::mem::size_of::<StartupInfo>() as u32;
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                CreateProcessW(
                    wide(exe.as_os_str()).as_ptr(),
                    command.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    4, /* CREATE_SUSPENDED */
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &mut startup,
                    &mut info,
                )
            },
            0,
            "CreateProcessW: {}",
            std::io::Error::last_os_error()
        );
        let process = Self {
            handle: owned(info.process),
        };
        let thread = owned(info.thread);
        // The process guard reaps the still-suspended root if assignment fails.
        assert_ne!(
            unsafe {
                AssignProcessToJobObject(job.0.as_raw_handle(), process.handle.as_raw_handle())
            },
            0,
            "required owned Job Object assignment: {}",
            std::io::Error::last_os_error()
        );
        assert_ne!(unsafe { ResumeThread(thread.as_raw_handle()) }, u32::MAX);
        process
    }
    fn wait(&self) {
        wait(&self.handle);
        let mut code = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(self.handle.as_raw_handle(), &mut code) },
            0
        );
        assert_eq!(code, 0);
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        unsafe { TerminateProcess(self.handle.as_raw_handle(), 92) };
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT.as_millis() as u32) },
            0
        );
    }
}
fn total(cpu: CpuTimes) -> Duration {
    cpu.user.checked_add(cpu.kernel).unwrap()
}
fn burn() {
    // Stop on measured CPU, not wall-clock sleep (which proves no accounting work).
    let start = account::current_process_times().unwrap().cpu;
    let deadline = Instant::now() + LIMIT;
    let mut value = 0x123456789abcdefu64;
    loop {
        for _ in 0..100_000 {
            value = std::hint::black_box(
                value
                    .wrapping_mul(6364136223846793005)
                    .rotate_left(13)
                    .wrapping_add(1),
            );
        }
        if total(
            account::current_process_times()
                .unwrap()
                .cpu
                .checked_sub(start)
                .unwrap(),
        ) >= Duration::from_millis(150)
        {
            break;
        }
        assert!(Instant::now() < deadline, "real CPU work deadline");
    }
    std::hint::black_box(value);
}
fn raw_cpu(process: &Process) -> CpuTimes {
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    assert_ne!(
        unsafe {
            GetProcessTimes(
                process.handle.as_raw_handle(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        },
        0
    );
    let duration = |time: FileTime| {
        Duration::from_nanos(((u64::from(time.high) << 32) | u64::from(time.low)) * 100)
    };
    CpuTimes {
        user: duration(user),
        kernel: duration(kernel),
    }
}
fn owner() {
    let initial = account::tracked_children_cpu().unwrap();
    let self_before = account::current_process_snapshot().unwrap();
    let start = TimerSnapshot::take().unwrap();
    burn();
    let self_after = account::current_process_snapshot().unwrap();
    assert!(
        total(
            self_after
                .times
                .cpu
                .checked_sub(self_before.times.cpu)
                .unwrap()
        ) >= Duration::from_millis(150)
    );
    assert!(self_after.times.created <= self_after.times.observed);
    assert_eq!(self_after.times.exited, None);
    assert_eq!(
        self_after.times.elapsed,
        self_after.times.observed - self_after.times.created
    );
    assert!(self_after.peak_working_set_bytes >= self_after.working_set_bytes);

    let first_job = Job::new();
    let second_job = Job::new();
    let mut first = account::register_job(first_job.0.clone()).unwrap();
    assert!(
        account::register_job(first_job.0.clone()).is_err(),
        "duplicate registration"
    );
    let mut second = account::register_job(second_job.0.clone()).unwrap();
    let a = Process::spawn(&first_job, &[OsStr::new("n5-account-quiet")]);
    let b = Process::spawn(&second_job, &[OsStr::new("n5-account-quiet")]);
    a.wait();
    b.wait();
    let a_process = account::process_snapshot(a.handle.as_handle()).unwrap();
    let b_process = account::process_snapshot(b.handle.as_handle()).unwrap();
    for (process, snapshot) in [(&a, a_process), (&b, b_process)] {
        assert_eq!(
            snapshot.times.cpu,
            raw_cpu(process),
            "FILETIME 100ns conversion"
        );
        assert!(snapshot.times.exited.is_some());
        assert_eq!(
            snapshot.times.elapsed,
            snapshot.times.exited.unwrap() - snapshot.times.created
        );
        assert!(total(snapshot.times.cpu) >= Duration::from_millis(150));
        assert!(total(snapshot.times.cpu) <= snapshot.times.elapsed + Duration::from_millis(50));
        assert_eq!(
            account::process_times(process.handle.as_handle())
                .unwrap()
                .elapsed,
            snapshot.times.elapsed,
            "completed elapsed is frozen"
        );
    }
    let a_job = account::job_snapshot(first_job.0.as_handle()).unwrap();
    let b_job = account::job_snapshot(second_job.0.as_handle()).unwrap();
    for (snapshot, process) in [(a_job, a_process), (b_job, b_process)] {
        assert_eq!(snapshot.total_processes, 1);
        assert_eq!(snapshot.active_processes, 0);
        assert_eq!(
            snapshot.cpu, process.times.cpu,
            "single root contributes exactly once"
        );
    }
    let expected = initial
        .checked_add(a_job.cpu)
        .unwrap()
        .checked_add(b_job.cpu)
        .unwrap();
    assert_eq!(
        account::tracked_children_cpu().unwrap(),
        expected,
        "both disjoint concurrent jobs included"
    );
    assert_eq!(first.finish().unwrap().cpu, a_job.cpu);
    assert_eq!(
        account::tracked_children_cpu().unwrap(),
        expected,
        "live-to-finished replacement, not addition"
    );
    assert_eq!(first.finish().unwrap().cpu, a_job.cpu);
    assert_eq!(second.finish().unwrap().cpu, b_job.cpu);
    drop(first);
    drop(second);
    assert_eq!(
        account::tracked_children_cpu().unwrap(),
        expected,
        "finish/drop/cached completion are idempotent"
    );
    let end = TimerSnapshot::take().unwrap();
    assert!(
        TimerSnapshot::get_delta(&end, &start, true).is_err(),
        "reversed real timer interval"
    );
    let report = TimerSnapshot::get_delta(&start, &end, true).unwrap();
    assert!(report.contains("external") && report.contains("fish"));

    // A real descendant remains alive when the root exits; its later CPU is out of scope.
    let nested_job = Job::new();
    let mut nested = account::register_job(nested_job.0.clone()).unwrap();
    let ready = Event::new("nested-ready");
    let release = Event::new("nested-release");
    let done = Event::new("nested-done");
    let root = Process::spawn(
        &nested_job,
        &[
            OsStr::new("n5-account-root"),
            &ready.name,
            &release.name,
            &done.name,
        ],
    );
    ready.wait();
    root.wait();
    let cutoff = nested.finish().unwrap();
    assert_eq!(cutoff.total_processes, 2);
    assert_eq!(cutoff.active_processes, 1);
    let frozen = account::tracked_children_cpu().unwrap();
    assert_eq!(frozen, expected.checked_add(cutoff.cpu).unwrap());
    release.signal();
    done.wait();
    let deadline = Instant::now() + LIMIT;
    let completed = loop {
        let snapshot = account::job_snapshot(nested_job.0.as_handle()).unwrap();
        if snapshot.active_processes == 0 {
            break snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "nested child completion deadline"
        );
        std::thread::yield_now();
    };
    assert!(total(completed.cpu.checked_sub(cutoff.cpu).unwrap()) >= Duration::from_millis(150));
    assert_eq!(
        nested.finish().unwrap().cpu,
        cutoff.cpu,
        "root cutoff is retained"
    );
    assert_eq!(
        account::tracked_children_cpu().unwrap(),
        frozen,
        "post-root descendant CPU excluded"
    );
    println!("account-owner-ok");
}
fn faults() {
    let event = Event::new("wrong-kind");
    assert!(account::process_times(event.handle.as_handle()).is_err());
    assert!(account::job_snapshot(event.handle.as_handle()).is_err());
    assert!(account::register_job(Arc::new(event.handle)).is_err());
    assert_eq!(
        account::tracked_children_cpu().unwrap(),
        CpuTimes::default(),
        "failed registration must not poison registry"
    );
    burn();
    let measured = account::current_process_times().unwrap().cpu;
    assert!(
        CpuTimes::default().checked_sub(measured).is_err(),
        "counter regression is not zero-clamped"
    );
    let maximum = CpuTimes {
        user: Duration::MAX,
        kernel: Duration::MAX,
    };
    assert!(
        maximum.checked_add(measured).is_err(),
        "overflow is checked"
    );
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    assert_ne!(
        unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 0) },
        0
    );
    let read = owned(read);
    let write = owned(write);
    drop(read);
    let mut pipe = std::fs::File::from(write);
    assert!(
        account::write_self_usage(&mut pipe).is_err(),
        "real disconnected output propagates failure"
    );
    println!("account-faults-ok");
}
fn nested() {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("n5-account-cpu")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut data = Vec::new();
        let mut stream = stdout;
        stream.read_to_end(&mut data).unwrap();
        data
    });
    let err = std::thread::spawn(move || {
        let mut data = Vec::new();
        let mut stream = stderr;
        stream.read_to_end(&mut data).unwrap();
        data
    });
    if unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) } != 0 {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("owned nested CPU child deadline");
    }
    assert!(child.wait().unwrap().success());
    std::io::stdout().write_all(&out.join().unwrap()).unwrap();
    std::io::stderr().write_all(&err.join().unwrap()).unwrap();
}

fn cli_fault(fish: &OsStr) {
    for rusage in [false, true] {
        let mut read = std::ptr::null_mut();
        let mut write = std::ptr::null_mut();
        assert_ne!(
            unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 0) },
            0
        );
        let read = owned(read);
        let write = owned(write);
        drop(read);
        let mut command = Command::new(fish);
        command.arg("-N");
        if rusage {
            command.arg("--print-rusage-self");
        }
        command.arg("-c").arg(if rusage {
            "printf 'normal-output\\n'; exit 37"
        } else {
            "time begin; printf 'normal-output\\n'; end; exit 37"
        });
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(write))
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let drain = std::thread::spawn(move || {
            let mut data = Vec::new();
            stdout.read_to_end(&mut data).unwrap();
            data
        });
        if unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) } != 0 {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned accounting CLI deadline");
        }
        assert_eq!(
            child.wait().unwrap().code(),
            Some(37),
            "diagnostic write failure must preserve shell status"
        );
        assert_eq!(drain.join().unwrap(), b"normal-output\n");
    }
    println!("account-cli-fault-ok");
}

pub fn run(mode: &str, args: Vec<OsString>) {
    match mode {
        "n5-account-owner" => owner(),
        "n5-account-faults" => faults(),
        "n5-account-cli-fault" => {
            assert_eq!(args.len(), 1);
            cli_fault(&args[0]);
        }
        "n5-account-cpu" => {
            burn();
            println!("account-work");
        }
        "n5-account-quiet" => burn(),
        "n5-account-copy" => {
            std::io::copy(&mut std::io::stdin().lock(), &mut std::io::stdout().lock()).unwrap();
        }
        "n5-account-nested" => nested(),
        "n5-account-cached-child" => {
            assert_eq!(args.len(), 2);
            burn();
            println!("account-work");
            Event::open(&args[0]).signal();
            Event::open(&args[1]).wait();
        }
        "n5-account-observe" => {
            assert_eq!(args.len(), 3);
            Event::open(&args[1]).wait();
            let pid = args[0].to_str().unwrap().parse().unwrap();
            let process = owned(unsafe { OpenProcess(0x00101000, 0, pid) });
            Event::open(&args[2]).signal();
            wait(&process);
            let snapshot = account::process_times(process.as_handle()).unwrap();
            assert!(snapshot.exited.is_some());
            assert!(total(snapshot.cpu) >= Duration::from_millis(150));
        }
        "n5-account-root" => {
            assert_eq!(args.len(), 3);
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("n5-account-descendant")
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            // The controller's fresh Job Object owns the descendant through inheritance.
            Event::open(&args[0]).wait();
            drop(child);
        }
        "n5-account-descendant" => {
            assert_eq!(args.len(), 3);
            Event::open(&args[0]).signal();
            Event::open(&args[1]).wait();
            burn();
            Event::open(&args[2]).signal();
        }
        _ => panic!("unknown N5 accounting probe mode {mode}"),
    }
}
