//! Real Win32 child/barrier and retained-owner probes. No shell behavior is mocked.
use std::ffi::{OsStr, OsString, c_void};
use std::io::{self, BufRead, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
const LIMIT: Duration = Duration::from_secs(20);
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn CreateEventW(sa: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn ExitProcess(code: u32) -> !;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn GetProcessId(process: *mut c_void) -> u32;
    fn GetExitCodeProcess(process: *mut c_void, status: *mut u32) -> i32;
    fn SetHandleInformation(handle: *mut c_void, mask: u32, flags: u32) -> i32;
    fn GetHandleInformation(handle: *mut c_void, flags: *mut u32) -> i32;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn CreateJobObjectW(sa: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, class: i32, data: *const c_void, size: u32)
    -> i32;
    fn QueryInformationJobObject(
        job: *mut c_void,
        class: i32,
        data: *mut c_void,
        size: u32,
        returned: *mut u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn TerminateJobObject(job: *mut c_void, status: u32) -> i32;
    fn TerminateProcess(process: *mut c_void, status: u32) -> i32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn CreateProcessW(
        exe: *const u16,
        line: *mut u16,
        process_sa: *mut c_void,
        thread_sa: *mut c_void,
        inherit: i32,
        flags: u32,
        env: *mut c_void,
        cwd: *const u16,
        startup: *mut Startup,
        process: *mut ProcessInfo,
    ) -> i32;
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn own(raw: *mut c_void) -> OwnedHandle {
    assert!(
        !raw.is_null() && raw as isize != -1,
        "required native fixture handle: {}",
        io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn event(name: &OsStr) -> OwnedHandle {
    own(unsafe { OpenEventW(0x0010_0002, 0, wide(name).as_ptr()) })
}
fn signal(handle: &impl AsRawHandle) {
    assert_ne!(
        unsafe { SetEvent(handle.as_raw_handle()) },
        0,
        "SetEvent: {}",
        io::Error::last_os_error()
    );
}
fn wait(handle: &impl AsRawHandle) {
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "bounded real-native fixture wait failed"
    );
}
struct Named {
    name: OsString,
    handle: OwnedHandle,
}
impl Named {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = OsString::from(format!(
            "Local\\fish-n5-probe-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let handle = own(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(&name).as_ptr()) });
        Self { name, handle }
    }
}
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn barrier(args: &[OsString], copy: bool) {
    let ready = event(&args[0]);
    let release = event(&args[1]);
    std::fs::write(&args[2], std::process::id().to_string()).unwrap();
    signal(&ready);
    wait(&release);
    if copy {
        io::copy(&mut io::stdin().lock(), &mut io::stdout().lock()).unwrap();
        io::stdout().flush().unwrap();
    }
    let code: u32 = args[3].to_str().unwrap().parse().unwrap();
    unsafe { ExitProcess(code) }
}

fn retained() {
    use fish::exec::NativeLaunchGroup;
    use fish::io::IoChain;
    use fish::proc::{InternalJobId, Pid};
    use fish::threads::{NativeCompletionPayload, NativeRuntime};
    use fish::wait_handle::{WaitHandle, WaitHandleStore};
    use fish_widestring::L;
    fish::threads::init();
    let runtime = NativeRuntime::new().unwrap();
    let probe = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let env: Vec<_> = std::env::vars_os().collect();
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let child = group
        .spawn(
            &probe,
            &[probe.as_os_str().to_owned(), "exit".into(), "259".into()],
            &cwd,
            &env,
            &IoChain::new(),
            false,
        )
        .unwrap();
    assert_eq!(child.wait(None).unwrap(), 259);
    let pid = Pid::from_native(child.pid());
    let wh = WaitHandle::new(
        pid,
        InternalJobId::default(),
        L!("native probe").to_owned(),
        Arc::clone(&child),
    );
    let mut cache = WaitHandleStore::new_with_capacity(2);
    cache.add(wh.clone());
    // A real process callback must be observed, not a test-side fake notifier.
    let mut observed = 0;
    for record in runtime.wait(Some(LIMIT), None).unwrap() {
        if let NativeCompletionPayload::ProcessExit {
            pid: actual,
            raw_status,
        } = record.payload
        {
            assert_eq!(actual, child.pid());
            assert_eq!(raw_status, 259);
            observed += 1;
            wh.set_status_and_complete(fish::proc::ProcStatus::native(raw_status).status_value());
        }
    }
    assert_eq!(
        observed, 1,
        "real process completion callback missing/duplicated"
    );
    group.finish().unwrap();
    assert!(
        runtime.drain().is_empty(),
        "finish did not drain owned callbacks"
    );
    // Cached completion retains its exact native child across actual OS churn.
    // Keeping that object alive prevents Windows from recycling this PID.
    for _ in 0..128 {
        let mut churn = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
        let next = churn
            .spawn(
                &probe,
                &[probe.as_os_str().to_owned(), "exit".into(), "0".into()],
                &cwd,
                &env,
                &IoChain::new(),
                false,
            )
            .unwrap();
        assert_ne!(next.pid(), child.pid(), "retained process PID was recycled");
        assert_eq!(next.wait(None).unwrap(), 0);
        churn.finish().unwrap();
    }
    assert_eq!(
        unsafe { GetProcessId(child.handle().as_raw_handle()) },
        child.pid()
    );
    assert_eq!(
        child.wait(None).unwrap(),
        259,
        "retained native status changed during process churn"
    );
    assert_eq!(cache.get_by_pid(pid).unwrap().status(), Some(1));
    assert!(
        wh.native_job_completion().is_none(),
        "a process-only completion claimed a whole job"
    );
    cache.remove(&wh);
    assert_eq!(cache.size(), 0);
    let retained_raw = child.handle().as_raw_handle();
    let mut flags = 0;
    assert_ne!(
        unsafe { GetHandleInformation(retained_raw, &mut flags) },
        0,
        "cached child handle closed before its final owner"
    );
    drop(wh);
    drop(child);
    assert_eq!(
        unsafe { GetHandleInformation(retained_raw, &mut flags) },
        0,
        "drained child's last retained process handle leaked"
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(6),
        "expected invalid closed owned handle"
    );
    assert!(runtime.drain().is_empty());

    let ready = Named::new("rollback-ready");
    let release = Named::new("rollback-release");
    let marker = cwd.join("n5-rollback-child.pid");
    let corrupt = cwd.join("n5-invalid-image.exe");
    std::fs::write(&corrupt, b"MZ-invalid-native-image").unwrap();
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let root = group
        .spawn(
            &probe,
            &[
                probe.as_os_str().to_owned(),
                "n5-jobs-barrier".into(),
                ready.name.clone(),
                release.name.clone(),
                marker.as_os_str().to_owned(),
                "0".into(),
            ],
            &cwd,
            &env,
            &IoChain::new(),
            false,
        )
        .unwrap();
    wait(&ready.handle);
    assert!(
        group
            .spawn(
                &corrupt,
                &[corrupt.as_os_str().to_owned()],
                &cwd,
                &env,
                &IoChain::new(),
                false
            )
            .is_err(),
        "real invalid second stage did not fail"
    );
    drop(group);
    assert_eq!(
        unsafe { WaitForSingleObject(root.handle().as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "partial native launch orphaned first root"
    );
    drop(root);
    assert!(
        runtime.drain().is_empty(),
        "rollback callback/handle ownership not drained"
    );
    std::fs::remove_file(corrupt).unwrap();
    retained_event_lease();
    println!("retained-identity=stable;cache;callbacks-drained;partial-rollback;event-lease");
}

fn retained_event_lease() {
    use fish::io::IoChain;
    use fish::proc::Pid;
    use fish_widestring::WString;

    fn handle_count() -> u32 {
        let mut count = 0;
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
            0,
            "GetProcessHandleCount: {}",
            io::Error::last_os_error()
        );
        count
    }

    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
    let probe = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let io = IoChain::new();
    let eval = |parser: &mut fish::parser::Parser, code: &str| {
        let code: WString = code.into();
        assert_eq!(
            parser.eval(&code, &io).status.status_value(),
            0,
            "event lease fixture evaluation failed: {code}"
        );
    };
    // Initialize the parser's native runtime, stdio and completion machinery
    // before measuring. The fixture is single-threaded at each count cutoff;
    // no evaluator command or child launch runs concurrently with a sample.
    eval(
        &mut parser,
        &format!("{} exit 0", quote(probe.to_str().unwrap())),
    );
    fish::proc::job_reap(&mut parser, false, None);
    fish::event::fire_delayed(&mut parser);
    eval(
        &mut parser,
        r#"
function n5_lease_catch_process --on-process-exit 0
    set -ga n5_lease_catch $argv
end
function n5_lease_catch_job --on-job-exit 0
    set -ga n5_lease_catch $argv
end
"#,
    );

    let mut settled_handles = None;
    for (option, event_name) in [
        ("--on-process-exit", "PROCESS_EXIT"),
        ("--on-job-exit", "JOB_EXIT"),
    ] {
        for remove_handler in [false, true] {
            let ready = Named::new("event-lease-ready");
            let release = Named::new("event-lease-release");
            let marker = cwd.join("n5-event-lease.pid");
            eval(
                &mut parser,
                "set -g n5_lease_deliveries; set -g n5_lease_catch; set -g n5_lease_replacement 0; test (count $n5_lease_deliveries) -eq 0",
            );
            assert!(parser.jobs().is_empty());
            assert_eq!(parser.wait_handles().size(), 0);
            eval(
                &mut parser,
                &format!(
                    "{} n5-jobs-barrier {} {} {} 37 &",
                    quote(probe.to_str().unwrap()),
                    quote(ready.name.to_str().unwrap()),
                    quote(release.name.to_str().unwrap()),
                    quote(marker.to_str().unwrap())
                ),
            );
            wait(&ready.handle);
            let pid: u32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
            let observer = own(unsafe { OpenProcess(0x0010_1000, 0, pid) });
            signal(&release.handle);
            wait(&observer);
            fish::proc::job_reap(&mut parser, false, None);
            fish::event::fire_delayed(&mut parser);
            // Native observation joins its registered waits and publishes the
            // real process/job transitions. Their catch-all callbacks run
            // under the reaper's removal guard and may leave completed internal
            // jobs behind. Reap once after that synchronous callback boundary;
            // this is cleanup of known-completed owners, not readiness polling.
            fish::proc::job_reap(&mut parser, false, None);
            assert!(
                parser.jobs().is_empty(),
                "completed native job remained a semantic lease owner"
            );
            let pid = Pid::from_native(pid);
            assert_eq!(
                parser.wait_handles().get_by_pid(pid).unwrap().status(),
                Some(37)
            );
            assert_eq!(
                parser
                    .wait_handles()
                    .get_by_pid(pid)
                    .unwrap()
                    .native_job_completion()
                    .unwrap()
                    .status,
                37,
                "whole-job replay lost the actual final status"
            );
            // Drain the original real lifecycle transitions before registering
            // a late handler. Only function's targeted cached replay is queued
            // below, not a manually constructed Event or a second transition.
            eval(&mut parser, "set -g n5_lease_catch");
            parser.global_event_blocks = 1;
            eval(
                &mut parser,
                &format!(
                    "function n5_lease_late {option} {pid}; set -ga n5_lease_deliveries $argv; end; wait {pid}"
                ),
            );
            assert_eq!(
                parser.wait_handles().size(),
                0,
                "wait did not consume the last cached process/job completion"
            );
            assert!(parser.jobs().is_empty());
            // This observer is only a bounded synchronization/identity aid.
            // Close it before the ownership sample: keeping it alive would
            // itself pin the process object and invalidate this regression.
            drop(observer);
            {
                let query = own(unsafe { OpenProcess(0x0010_1000, 0, pid.get()) });
                assert_eq!(unsafe { GetProcessId(query.as_raw_handle()) }, pid.get());
                let mut status = 0;
                assert_ne!(
                    unsafe { GetExitCodeProcess(query.as_raw_handle(), &mut status) },
                    0
                );
                assert_eq!(status, 37);
                assert_eq!(unsafe { WaitForSingleObject(query.as_raw_handle(), 0) }, 0);
            }
            // Compare immediately across dispatch, excluding unrelated launch-
            // time runtime initialization. The query above owns no sampled lease.
            let queued_handles = handle_count();
            if remove_handler {
                eval(
                    &mut parser,
                    "functions --erase n5_lease_late; function n5_lease_late; set -g n5_lease_replacement 1; end",
                );
            }
            fish::event::fire_delayed(&mut parser);
            eval(
                &mut parser,
                "test (count $n5_lease_deliveries) -eq 0; and test (count $n5_lease_catch) -eq 0",
            );
            if !remove_handler {
                assert_eq!(
                    handle_count(),
                    queued_handles,
                    "blocked delivery released its process-object lease"
                );
            }
            // Erasure must not retarget a replacement handler. Once dispatch
            // has delivered or removed the event, its last lease must be gone.
            parser.global_event_blocks = 0;
            for _ in 0..2 {
                fish::event::fire_delayed(&mut parser);
                if remove_handler {
                    eval(
                        &mut parser,
                        "test (count $n5_lease_deliveries) -eq 0; and test $n5_lease_replacement -eq 0",
                    );
                } else {
                    eval(
                        &mut parser,
                        &format!("test \"$n5_lease_deliveries\" = '{event_name} {pid} 37'"),
                    );
                }
                eval(&mut parser, "test (count $n5_lease_catch) -eq 0");
                assert_eq!(
                    handle_count(),
                    queued_handles - 1,
                    "{option}: dispatch/removal leaked the last native process-object lease"
                );
            }
            eval(&mut parser, "functions --erase n5_lease_late");
            std::fs::remove_file(marker).unwrap();
            let settled = handle_count();
            if let Some(previous) = settled_handles {
                assert_eq!(
                    settled, previous,
                    "successive native exit deliveries/removals leaked handles"
                );
            }
            settled_handles = Some(settled);
        }
    }
    eval(
        &mut parser,
        "functions --erase n5_lease_catch_process n5_lease_catch_job",
    );
}

#[repr(C)]
struct Startup {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_chars: u32,
    y_chars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_count: u16,
    reserved_bytes: *mut u8,
    stdin: *mut c_void,
    stdout: *mut c_void,
    stderr: *mut c_void,
}
#[repr(C)]
struct ProcessInfo {
    process: *mut c_void,
    thread: *mut c_void,
    pid: u32,
    tid: u32,
}
#[repr(C)]
#[derive(Default)]
struct Accounting {
    user: i64,
    kernel: i64,
    period_user: i64,
    period_kernel: i64,
    faults: u32,
    total: u32,
    active: u32,
    terminated: u32,
}
#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    process_cpu: i64,
    job_cpu: i64,
    flags: u32,
    min_working: usize,
    max_working: usize,
    active_limit: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct ExtendedLimits {
    basic: BasicLimits,
    io: fish::native_accounting::IoCounters,
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
}
struct PolicyJob(OwnedHandle);
impl PolicyJob {
    fn new() -> Self {
        let job = Self(own(unsafe {
            CreateJobObjectW(std::ptr::null(), std::ptr::null())
        }));
        let mut limits = ExtendedLimits::default();
        // All members belong to this fixture, never the enclosing runner.
        // Survival checks keep this outer owner alive until deliberate cleanup.
        limits.basic.flags = 0x2000;
        assert_ne!(
            unsafe {
                SetInformationJobObject(
                    job.0.as_raw_handle(),
                    9,
                    (&limits as *const ExtendedLimits).cast(),
                    std::mem::size_of::<ExtendedLimits>() as u32,
                )
            },
            0,
            "mandatory owned kill-on-close cleanup fixture: {}",
            io::Error::last_os_error()
        );
        job
    }
    fn accounting(&self) -> Accounting {
        let mut a = Accounting::default();
        assert_ne!(
            unsafe {
                QueryInformationJobObject(
                    self.0.as_raw_handle(),
                    1,
                    (&mut a as *mut Accounting).cast(),
                    std::mem::size_of::<Accounting>() as u32,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        a
    }
}
impl Drop for PolicyJob {
    fn drop(&mut self) {
        // This is the fixture-created job only, never an enclosing CI job.
        assert_ne!(unsafe { TerminateJobObject(self.0.as_raw_handle(), 99) }, 0);
    }
}
struct Process(OwnedHandle);
impl Drop for Process {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } == 258 {
            assert_ne!(unsafe { TerminateProcess(self.0.as_raw_handle(), 99) }, 0);
            wait(&self.0);
        }
    }
}
fn spawn_in_policy(job: &PolicyJob, exe: &Path, args: &[OsString], cwd: &Path) -> Process {
    // CreateProcessW requires one UTF-16 command line; keep the public fixture
    // interface as an argument array and apply the documented CRT quote rules.
    let mut line = Vec::<u16>::new();
    for arg in std::iter::once(exe.as_os_str()).chain(args.iter().map(OsString::as_os_str)) {
        if !line.is_empty() {
            line.push(32);
        }
        line.push(34);
        let mut slashes = 0;
        for unit in arg.encode_wide() {
            assert_ne!(unit, 0, "NUL in fixture argument");
            if unit == 92 {
                slashes += 1;
                continue;
            }
            line.extend(std::iter::repeat_n(
                92,
                slashes * if unit == 34 { 2 } else { 1 },
            ));
            if unit == 34 {
                line.push(92);
            }
            line.push(unit);
            slashes = 0;
        }
        line.extend(std::iter::repeat_n(92, slashes * 2));
        line.push(34);
    }
    line.push(0);
    let mut startup: Startup = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<Startup>() as u32;
    let null = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("NUL")
        .expect("mandatory native NUL fixture");
    let errors = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cwd.join("n5-policy-stderr"))
        .unwrap();
    for handle in [null.as_raw_handle(), errors.as_raw_handle()] {
        assert_ne!(unsafe { SetHandleInformation(handle, 1, 1) }, 0);
    }
    startup.flags = 0x100;
    startup.stdin = null.as_raw_handle();
    startup.stdout = null.as_raw_handle();
    startup.stderr = errors.as_raw_handle();
    let mut pi: ProcessInfo = unsafe { std::mem::zeroed() };
    // Ordinary rollback uses supported nested jobs without asking an enclosing
    // runner/CI job for breakaway rights it may intentionally withhold.
    assert_ne!(
        unsafe {
            CreateProcessW(
                wide(exe.as_os_str()).as_ptr(),
                line.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                0x0000_0004 | 0x0000_0200,
                std::ptr::null_mut(),
                wide(cwd.as_os_str()).as_ptr(),
                &mut startup,
                &mut pi,
            )
        },
        0,
        "mandatory nested policy fixture CreateProcessW: {}",
        io::Error::last_os_error()
    );
    let process = Process(own(pi.process));
    let thread = own(pi.thread);
    assert_ne!(
        unsafe { AssignProcessToJobObject(job.0.as_raw_handle(), process.0.as_raw_handle()) },
        0,
        "mandatory owned policy job membership: {}",
        io::Error::last_os_error()
    );
    assert_ne!(unsafe { ResumeThread(thread.as_raw_handle()) }, u32::MAX);
    process
}

fn abrupt_shell_survival(fish: &Path, cwd: &Path) {
    let probe = std::env::current_exe().unwrap();
    for disowned in [false, true] {
        let job = PolicyJob::new();
        let ready = Named::new("abrupt-background-ready");
        let release = Named::new("abrupt-background-release");
        let fence_ready = Named::new("abrupt-shell-fence-ready");
        let fence_release = Named::new("abrupt-shell-fence-release");
        let child_pid = cwd.join("n5-abrupt-background.pid");
        let fence_pid = cwd.join("n5-abrupt-fence.pid");
        let command = format!(
            "{} n5-jobs-barrier {} {} {} 0 >/dev/null 2>/dev/null &; {}; {} n5-jobs-barrier {} {} {} 0 >/dev/null 2>/dev/null",
            quote(probe.to_str().unwrap()),
            quote(ready.name.to_str().unwrap()),
            quote(release.name.to_str().unwrap()),
            quote(child_pid.to_str().unwrap()),
            if disowned { "disown $last_pid" } else { "true" },
            quote(probe.to_str().unwrap()),
            quote(fence_ready.name.to_str().unwrap()),
            quote(fence_release.name.to_str().unwrap()),
            quote(fence_pid.to_str().unwrap()),
        );
        let shell = spawn_in_policy(&job, fish, &["-N".into(), "-c".into(), command.into()], cwd);
        wait(&ready.handle);
        wait(&fence_ready.handle);
        // Both children are held at private barriers. Open once while their
        // live identities are established, then keep capabilities, not PIDs.
        let retain = |path: &Path| {
            let pid = std::fs::read_to_string(path).unwrap().parse().unwrap();
            Process(own(unsafe { OpenProcess(0x0010_1001, 0, pid) }))
        };
        let child = retain(&child_pid);
        let fence = retain(&fence_pid);
        assert_eq!(
            unsafe { WaitForSingleObject(child.0.as_raw_handle(), 0) },
            258
        );
        // Reaching the next foreground command proves the background launch
        // was accepted (and disown, when requested, has returned).
        assert_ne!(unsafe { TerminateProcess(shell.0.as_raw_handle(), 73) }, 0);
        wait(&shell.0);
        let mut status = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(shell.0.as_raw_handle(), &mut status) },
            0
        );
        assert_eq!(status, 73);
        assert_eq!(
            unsafe { WaitForSingleObject(child.0.as_raw_handle(), 0) },
            258,
            "accepted background work died with an abruptly terminated shell; disowned={disowned}"
        );
        signal(&release.handle);
        signal(&fence_release.handle);
        wait(&child.0);
        wait(&fence.0);
        assert_ne!(
            unsafe { GetExitCodeProcess(child.0.as_raw_handle(), &mut status) },
            0
        );
        assert_eq!(
            status, 0,
            "survivor did not reach its controlled normal exit"
        );
        assert_eq!(
            job.accounting().active,
            0,
            "abrupt-exit fixture left an orphan"
        );
    }
}

fn shell_rollback_host(fish: &Path, cwd: &Path) {
    let job = PolicyJob::new();
    let probe = std::env::current_exe().unwrap();
    let ready = Named::new("rollback-host-sentinel-ready");
    let release = Named::new("rollback-host-sentinel-release");
    let sentinel = spawn_in_policy(
        &job,
        &probe,
        &[
            "n5-jobs-barrier".into(),
            ready.name.clone(),
            release.name.clone(),
            cwd.join("n5-rollback-sentinel.pid").into_os_string(),
            "0".into(),
        ],
        cwd,
    );
    wait(&ready.handle);
    let first_ready = Named::new("shell-partial-first-ready");
    let first_release = Named::new("shell-partial-first-release");
    let invalid = cwd.join("n5-shell-invalid.exe");
    std::fs::write(&invalid, b"MZ-invalid-native-image").unwrap();
    // Native stages launch right-to-left after planning pins both images.
    // Put the valid barrier on the right so its real root exists before
    // CreateProcessW rejects the invalid left image and rollback must reap it.
    let code = format!(
        "{} | {} n5-jobs-barrier {} {} {} 0 &",
        quote(invalid.to_str().unwrap()),
        quote(probe.to_str().unwrap()),
        quote(first_ready.name.to_str().unwrap()),
        quote(first_release.name.to_str().unwrap()),
        quote(cwd.join("n5-partial-first.pid").to_str().unwrap()),
    );
    let shell = spawn_in_policy(&job, fish, &["-N".into(), "-c".into(), code.into()], cwd);
    wait(&shell.0);
    let mut status = 0;
    assert_ne!(
        unsafe { GetExitCodeProcess(shell.0.as_raw_handle(), &mut status) },
        0
    );
    assert_ne!(status, 0, "partial pipeline launch returned success");
    assert!(
        !std::fs::read(cwd.join("n5-policy-stderr"))
            .unwrap()
            .is_empty(),
        "partial launch omitted diagnostic"
    );
    let a = job.accounting();
    assert!(
        a.total >= 3,
        "first native stage never reached real process creation"
    );
    assert_eq!(
        a.active, 1,
        "actual fish partial launch orphaned a root/descendant"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(sentinel.0.as_raw_handle(), 0) },
        258,
        "partial rollback killed unrelated owned sentinel"
    );
    signal(&release.handle);
    wait(&sentinel.0);
    assert_eq!(job.accounting().active, 0);
    println!("shell-partial=failed;roots-reaped;sentinel-live;host-live");
}

fn active_wait_disown() {
    use fish::builtins::builtin_run;
    use fish::io::{IoChain, IoStreams, OutputStream, StringOutputStream};
    use fish_widestring::{L, WString};
    fish::threads::init();
    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
    let probe = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let ar = Named::new("active-wait-a-ready");
    let ax = Named::new("active-wait-a-release");
    let br = Named::new("active-wait-b-ready");
    let bx = Named::new("active-wait-b-release");
    let callback = Named::new("active-wait-disowned");
    let ap = cwd.join("n5-active-wait-a.pid");
    let bp = cwd.join("n5-active-wait-b.pid");
    let command = |ready: &Named, release: &Named, file: &Path| {
        format!(
            "{} n5-jobs-barrier {} {} {} 0",
            quote(probe.to_str().unwrap()),
            quote(ready.name.to_str().unwrap()),
            quote(release.name.to_str().unwrap()),
            quote(file.to_str().unwrap())
        )
    };
    let code: WString = format!(
        r#"
set -g n5_callback_count 0; set -g n5_forbidden 0
{} &; set -g n5_a $last_pid
{} &; set -g n5_b $last_pid
function disown_during_wait --on-process-exit $n5_a
    set -g n5_callback_count (math $n5_callback_count + 1)
    disown $n5_b
    set -g n5_disown_status $status
    {} n5-jobs-signal {}
end
function forbidden_process --on-process-exit $n5_b; set -g n5_forbidden 1; end
function forbidden_job --on-job-exit $n5_b; set -g n5_forbidden 1; end
"#,
        command(&ar, &ax, &ap),
        command(&br, &bx, &bp),
        quote(probe.to_str().unwrap()),
        quote(callback.name.to_str().unwrap())
    )
    .into();
    let io = IoChain::new();
    assert_eq!(parser.eval(&code, &io).status.status_value(), 0);
    wait(&ar.handle);
    wait(&br.handle);
    let open_ready = |file: &Path| {
        let pid: u32 = std::fs::read_to_string(file).unwrap().parse().unwrap();
        (
            pid,
            Process(own(unsafe { OpenProcess(0x0010_1001, 0, pid) })),
        )
    };
    let (apid, a) = open_ready(&ap);
    let (bpid, b) = open_ready(&bp);
    let retained: Vec<_> = parser
        .jobs()
        .iter()
        .flat_map(|job| {
            job.processes()
                .iter()
                .filter_map(|process| process.make_wait_handle(job.internal_job_id))
        })
        .collect();
    assert_eq!(retained.len(), 2);
    // No evaluator/reaper is entered after A is released until builtin_run:
    // wait captures A+B first, then its actual native completion dispatches the
    // callback which disowns B. This avoids a root-marker/next-command race.
    signal(&ax.handle);
    wait(&a.0);
    let callback_wait = event(&callback.name);
    let release_b = event(&bx.name);
    let releaser = std::thread::spawn(move || {
        wait(&callback_wait);
        signal(&release_b);
    });
    let mut out = OutputStream::String(StringOutputStream::new());
    let mut err = OutputStream::String(StringOutputStream::new());
    let mut streams = IoStreams::new(&mut out, &mut err, &io);
    let atext: WString = apid.to_string().into();
    let btext: WString = bpid.to_string().into();
    let result = builtin_run(&mut parser, &mut [L!("wait"), &atext, &btext], &mut streams);
    releaser.join().unwrap();
    wait(&b.0);
    assert_eq!(
        result.status_value(),
        0,
        "disown broke already captured wait handles"
    );
    assert!(
        out.contents().is_empty() && err.contents().is_empty(),
        "active wait emitted unexpected output/diagnostics"
    );
    for handle in &retained {
        assert!(
            handle.is_completed(),
            "disown lost retained process completion"
        );
    }
    let check: WString = "test $n5_callback_count -eq 1; and test $n5_disown_status -eq 0; and test $n5_forbidden -eq 0".into();
    for _ in 0..2 {
        fish::proc::job_reap(&mut parser, false, None);
        fish::event::fire_delayed(&mut parser);
        assert_eq!(parser.eval(&check, &io).status.status_value(), 0);
    }
    assert!(
        !parser.jobs().iter().any(|job| job
            .processes()
            .iter()
            .any(|process| process.pid().is_some_and(|pid| pid.get() == bpid))),
        "disowned job remained selectable after retained wait completed"
    );
    println!("active-wait=retained;callback-once;disowned;no-future-events");
}

pub fn run(mode: &str, args: Vec<OsString>) {
    match mode {
        "n5-jobs-barrier" => barrier(&args, false),
        "n5-jobs-copy-barrier" => barrier(&args, true),
        "n5-jobs-signal" => signal(&event(&args[0])),
        "n5-jobs-wait" => wait(&event(&args[0])),
        "n5-jobs-input" => {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes).unwrap();
            std::fs::write(&args[0], bytes).unwrap();
        }
        "n5-jobs-deferred-consumer" => {
            let ready = event(&args[0]);
            let release = event(&args[1]);
            signal(&ready);
            wait(&release);
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes).unwrap();
            assert_eq!(
                bytes.len(),
                args[2].to_str().unwrap().parse::<usize>().unwrap()
            );
            assert!(
                bytes
                    .chunks_exact(32)
                    .all(|b| b == b"abcdefghijklmnopqrstuvwxyz012345"),
                "deferred pump corrupted stream"
            );
        }
        "n5-jobs-streaming-consumer" => {
            let mut input = io::BufReader::new(io::stdin().lock());
            let mut line = String::new();
            input.read_line(&mut line).unwrap();
            assert_eq!(line, "readiness\n");
            signal(&event(&args[0]));
            let mut rest = String::new();
            input.read_to_string(&mut rest).unwrap();
            assert_eq!(rest, "tail\n");
        }
        "n5-jobs-retained" => retained(),
        "n5-jobs-rollback-host" => shell_rollback_host(Path::new(&args[0]), Path::new(&args[1])),
        "n5-jobs-abrupt-host" => abrupt_shell_survival(Path::new(&args[0]), Path::new(&args[1])),
        "n5-jobs-active-wait" => active_wait_disown(),
        _ => panic!("unknown N5 jobs probe mode {mode}"),
    }
}
