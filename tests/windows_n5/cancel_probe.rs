//! Real console controls and retained native owners; no mock notifier or PID kill.
use fish::exec::{NativeBreakOutcome, NativeChild, NativeLaunchGroup};
use fish::io::{IoChain, NativeCapture};
use fish::signal::{self, RawSignal, SigChecker};
use fish::threads::{NativeCompletionPayload, NativeRuntime};
use std::ffi::{OsStr, OsString, c_void};
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, LazyLock, mpsc};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(10);
const TIMEOUT: u32 = 258;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(sa: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(handle: *mut c_void) -> i32;
    fn ResetEvent(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const *mut c_void,
        all: i32,
        millis: u32,
    ) -> u32;
    fn GenerateConsoleCtrlEvent(control: u32, group: u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GetConsoleProcessList(processes: *mut u32, count: u32) -> u32;
    fn GetConsoleWindow() -> *mut c_void;
    fn GetConsoleCP() -> u32;
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn GetStdHandle(which: u32) -> *mut c_void;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
    fn TerminateProcess(process: *mut c_void, status: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn DuplicateHandle(
        source_process: *mut c_void,
        source: *mut c_void,
        target_process: *mut c_void,
        target: *mut *mut c_void,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> i32;
    fn GetProcessId(process: *mut c_void) -> u32;
    fn CreatePipe(
        read: *mut *mut c_void,
        write: *mut *mut c_void,
        sa: *mut c_void,
        size: u32,
    ) -> i32;
    fn GetNamedPipeInfo(
        pipe: *mut c_void,
        flags: *mut u32,
        output_size: *mut u32,
        input_size: *mut u32,
        instances: *mut u32,
    ) -> i32;
    fn PeekNamedPipe(
        pipe: *mut c_void,
        buffer: *mut u8,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        left: *mut u32,
    ) -> i32;
    fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
    fn ExitProcess(status: u32) -> !;
}
fn checked(value: i32) {
    assert_ne!(value, 0, "Win32: {}", io::Error::last_os_error());
}
fn own(raw: *mut c_void) -> OwnedHandle {
    assert!(
        !raw.is_null() && raw as isize != -1,
        "handle: {}",
        io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn wait(handle: &impl AsRawHandle) {
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "bounded native wait: {}",
        io::Error::last_os_error()
    );
}
fn live(handle: &impl AsRawHandle) {
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
        TIMEOUT,
        "a request is not an observed process exit"
    );
}
struct Event {
    name: OsString,
    handle: OwnedHandle,
}
impl Event {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = OsString::from(format!(
            "Local\\fish-n5-cancel-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let wide: Vec<_> = name.encode_wide().chain(Some(0)).collect();
        let handle = own(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) });
        Self { name, handle }
    }
    fn open(name: &OsStr) -> Self {
        let wide: Vec<_> = name.encode_wide().chain(Some(0)).collect();
        Self {
            name: name.to_owned(),
            handle: own(unsafe { OpenEventW(0x0010_0002, 0, wide.as_ptr()) }),
        }
    }
    fn signal(&self) {
        checked(unsafe { SetEvent(self.handle.as_raw_handle()) });
    }
    fn wait(&self) {
        wait(&self.handle);
    }
}
// Callback storage and event are process-lifetime objects. Even an already queued
// callback after unregistration cannot refer to released Rust or kernel storage.
static CALLBACK_EVENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static CALLBACK_FAILED: AtomicU64 = AtomicU64::new(0);
static HANDLER_EVENT: LazyLock<OwnedHandle> = LazyLock::new(|| {
    let event = own(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) });
    CALLBACK_EVENT.store(event.as_raw_handle(), Ordering::Release);
    event
});
fn callback_event() -> &'static OwnedHandle {
    &HANDLER_EVENT
}
unsafe extern "system" fn handoff(control: u32) -> i32 {
    if control > 1 {
        return 0;
    }
    COUNTS[control as usize].fetch_add(1, Ordering::Release);
    if unsafe { SetEvent(CALLBACK_EVENT.load(Ordering::Acquire)) } == 0 {
        CALLBACK_FAILED.fetch_add(1, Ordering::Release);
    }
    1
}
struct Handler;
impl Handler {
    fn install() -> Self {
        callback_event();
        checked(unsafe { SetConsoleCtrlHandler(Some(handoff), 1) });
        Self
    }
}
impl Drop for Handler {
    fn drop(&mut self) {
        checked(unsafe { SetConsoleCtrlHandler(Some(handoff), 0) });
    }
}
fn isolated_console() {
    checked(unsafe { SetConsoleCtrlHandler(None, 0) }); // This isolated controller receives C.
    assert_ne!(
        unsafe { GetConsoleCP() },
        0,
        "required real console is absent"
    );
    assert!(
        !unsafe { GetConsoleWindow() }.is_null(),
        "required isolated console window"
    );
    let mut pids = [0u32; 16];
    assert_eq!(
        unsafe { GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) },
        1,
        "controller must start alone in its own console"
    );
    assert_eq!(pids[0], std::process::id());
}
fn generate(control: u32) {
    checked(unsafe { GenerateConsoleCtrlEvent(control, 0) });
}
fn handle_count() -> u32 {
    let mut count = 0;
    checked(unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) });
    count
}
fn status(raw: u32) {
    let value = fish::proc::ProcStatus::native(raw);
    assert_eq!(value.native_raw(), Some(raw));
    assert_eq!(
        value.status_value(),
        if raw <= 255 { raw as i32 } else { 1 }
    );
    assert!(value.normal_exited());
    assert!(!value.signal_exited());
    assert!(!value.stopped());
}

fn controls() {
    isolated_console();
    let _fallback = Handler::install();
    let baseline = handle_count();
    for _ in 0..16 {
        let mut owner = signal::initialize_console_controls().expect("required console handler");
        let mut checker = SigChecker::new_sighupintterm();
        assert!(!checker.check());
        let mut retained = Vec::new();
        for (control, expected) in [(0, RawSignal::CTRL_C), (1, RawSignal::CTRL_BREAK)] {
            let before = signal::console_control_epoch();
            let old = owner.cancellation();
            let waiting = SigChecker::new_sighupintterm();
            let (send, done) = mpsc::channel();
            let waiter = std::thread::spawn(move || {
                send.send(waiting.wait()).unwrap();
            });
            generate(control);
            wait(&old.handle());
            done.recv_timeout(LIMIT)
                .expect("SigChecker native wake deadline")
                .unwrap();
            waiter.join().unwrap();
            assert_eq!(signal::signal_check_cancel(), expected.code());
            let after = signal::console_control_epoch();
            assert_eq!(after.ctrl_c - before.ctrl_c, u64::from(control == 0));
            assert_eq!(
                after.ctrl_break - before.ctrl_break,
                u64::from(control == 1)
            );
            assert_eq!(after.close, before.close);
            assert!(checker.check());
            assert!(!checker.check());
            owner.rearm().unwrap();
            assert_eq!(signal::signal_check_cancel(), 0);
            assert!(!owner.cancellation().is_cancelled());
            assert!(old.is_cancelled(), "rearm must not reset a retained token");
            retained.push(old);
        }
        // A genuine receipt races a normal-thread rearm. A receipt acknowledged
        // by that rearm lives in the old permanent token; a later one in the new.
        for _ in 0..8 {
            let before = signal::console_control_epoch();
            let old = owner.cancellation();
            let barrier = Arc::new(Barrier::new(2));
            let other = Arc::clone(&barrier);
            let sender = std::thread::spawn(move || {
                other.wait();
                generate(1);
            });
            barrier.wait();
            owner.rearm().unwrap();
            sender.join().unwrap();
            let current = owner.cancellation();
            let handles = [
                old.handle().as_raw_handle(),
                current.handle().as_raw_handle(),
            ];
            let observed =
                unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, LIMIT.as_millis() as u32) };
            assert!(observed < 2, "racing receipt was lost: {observed:#x}");
            assert_eq!(
                signal::console_control_epoch().ctrl_break,
                before.ctrl_break + 1
            );
            assert!(checker.check());
            assert!(!checker.check());
            owner.rearm().unwrap();
            assert!(!owner.cancellation().is_cancelled());
            retained.extend([old, current]);
        }
        let pending = owner.cancellation();
        let barrier = Arc::new(Barrier::new(2));
        let other = Arc::clone(&barrier);
        checked(unsafe { ResetEvent(callback_event().as_raw_handle()) });
        let before_shutdown = signal::console_control_epoch().ctrl_break;
        let sender = std::thread::spawn(move || {
            other.wait();
            generate(1);
        });
        barrier.wait();
        owner.shutdown().unwrap(); // Must unregister and drain admitted callbacks.
        sender.join().unwrap();
        if signal::console_control_epoch().ctrl_break == before_shutdown {
            wait(callback_event()); // Drain a receipt routed to the fallback after unregister.
        }
        owner.shutdown().unwrap();
        assert_eq!(
            owner.rearm().unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        assert!(signal::console_control_cancellation().is_none());
        assert_eq!(
            checker.wait().unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        // A retained token remains a valid real kernel object after its owner's
        // callback registration and all other epochs have been released.
        pending.cancel();
        wait(&pending.handle());
        drop((owner, retained, pending));
    }
    // Process-lifetime fallback storage was installed before the baseline.
    assert!(
        handle_count() <= baseline + 2,
        "console owner leaked handles across shutdowns"
    );
    assert_eq!(CALLBACK_FAILED.load(Ordering::Acquire), 0);
}

struct Work {
    group: Option<NativeLaunchGroup>,
    child: Arc<NativeChild>,
    capture: Arc<NativeCapture>,
    ready: Event,
    release: Event,
    ack: Event,
}
impl Work {
    fn new(runtime: &Arc<NativeRuntime>, image: &Path, kind: &str, raw: u32) -> Self {
        let ready = Event::new("ready");
        let release = Event::new("release");
        let ack = Event::new("ack");
        let capture = NativeCapture::new(65536, Arc::clone(runtime)).unwrap();
        let mut group = NativeLaunchGroup::new(Arc::clone(runtime)).unwrap();
        let child = group
            .spawn(
                image,
                &[
                    image.as_os_str().to_owned(),
                    "n5-cancel-child".into(),
                    ready.name.clone(),
                    release.name.clone(),
                    ack.name.clone(),
                    kind.into(),
                    raw.to_string().into(),
                ],
                &std::env::current_dir().unwrap(),
                &std::env::vars_os().collect::<Vec<_>>(),
                &capture.io_chain(),
                false,
            )
            .unwrap();
        let work = Self {
            group: Some(group),
            child,
            capture,
            ready,
            release,
            ack,
        };
        work.ready.wait();
        live(&work.child.handle());
        work
    }
    fn complete(mut self, raw: u32) -> Vec<u8> {
        self.release.signal();
        wait(&self.child.handle());
        assert_eq!(self.child.wait(None).unwrap(), raw);
        status(raw);
        let output = self.capture.finish().unwrap();
        assert!(output.error.is_none(), "{:?}", output.error);
        assert!(!output.interrupted);
        assert!(!output.buffer.discarded());
        let retained = Arc::clone(&self.child);
        self.group.take().unwrap().finish().unwrap();
        assert_eq!(retained.wait(None).unwrap(), raw);
        assert!(
            matches!(retained.request_ctrl_break().unwrap(), NativeBreakOutcome::AlreadyExited(code) if code == raw)
        );
        output.buffer.newline_serialized()
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        if self.group.is_some() {
            // Only the retained handle created by our own launch can be killed.
            // No PID lookup and no enclosing/CI Job Object is ever terminated.
            if unsafe { WaitForSingleObject(self.child.handle().as_raw_handle(), 0) } == TIMEOUT {
                checked(unsafe { TerminateProcess(self.child.handle().as_raw_handle(), 1) });
                wait(&self.child.handle());
            }
        }
    }
}
fn no_exit(runtime: &NativeRuntime, target: u32) {
    assert!(!runtime.drain().into_iter().any(|record|
        matches!(record.payload, NativeCompletionPayload::ProcessExit { pid, .. } if pid == target)),
        "a cooperative request fabricated process completion");
}
fn exit_event(runtime: &NativeRuntime, target: u32, expected: u32) {
    let start = Instant::now();
    let mut count = 0;
    while count == 0 {
        let remaining = LIMIT
            .checked_sub(start.elapsed())
            .expect("process-exit event deadline");
        for record in runtime.wait(Some(remaining), None).unwrap() {
            if let NativeCompletionPayload::ProcessExit { pid, raw_status } = record.payload {
                if pid == target {
                    assert_eq!(raw_status, expected);
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 1);
    no_exit(runtime, target);
}
fn requests(fish: &Path, gui: &Path) {
    isolated_console();
    let _fallback = Handler::install();
    fish::threads::init();
    let mut owner = signal::initialize_console_controls().unwrap();
    let runtime = NativeRuntime::new().unwrap();
    let image = std::env::current_exe().unwrap();
    let unrelated = Work::new(&runtime, &image, "ignore", 17);
    let receipt = owner.cancellation();
    generate(0);
    wait(&receipt.handle());
    assert_eq!(
        unrelated.child.wait(Some(&receipt)).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    live(&unrelated.child.handle());
    no_exit(&runtime, unrelated.child.pid());
    owner.rearm().unwrap();
    let before_targeted = signal::console_control_epoch();
    let cooperative = Work::new(&runtime, &image, "ack", 130);
    assert!(matches!(
        cooperative.child.request_ctrl_break().unwrap(),
        NativeBreakOutcome::Requested
    ));
    cooperative.ack.wait(); // Written/signalled by normal child context, not its handler.
    live(&cooperative.child.handle());
    live(&unrelated.child.handle());
    no_exit(&runtime, cooperative.child.pid());
    assert_eq!(
        signal::console_control_epoch(),
        before_targeted,
        "targeted child Break must not fabricate a shell control receipt"
    );
    assert!(!owner.cancellation().is_cancelled());
    let pid = cooperative.child.pid();
    cooperative.release.signal();
    wait(&cooperative.child.handle());
    exit_event(&runtime, pid, 130);
    let output = cooperative.complete(130);
    assert!(
        output
            .windows(b"ack=break\n".len())
            .any(|part| part == b"ack=break\n")
    );

    let mut ignored = Work::new(&runtime, &image, "ignore", 23);
    assert!(matches!(
        ignored.child.request_ctrl_break().unwrap(),
        NativeBreakOutcome::Requested
    ));
    ignored.ack.wait();
    live(&ignored.child.handle());
    live(&unrelated.child.handle());
    no_exit(&runtime, ignored.child.pid());

    assert!(
        gui.is_file(),
        "required in-repo GUI fixture is absent: {}",
        gui.display()
    );
    for (image, kind) in [
        (image.as_path(), "detached"),
        (image.as_path(), "foreign"),
        (gui, "gui"),
    ] {
        let unsupported = Work::new(&runtime, image, kind, 31);
        assert!(matches!(
            unsupported.child.request_ctrl_break().unwrap(),
            NativeBreakOutcome::UnsupportedConsole
        ));
        live(&unsupported.child.handle());
        no_exit(&runtime, unsupported.child.pid());
        let output = unsupported.complete(31);
        assert!(output.ends_with(b"controls=0,0\n"), "{:?}", output);
        live(&unrelated.child.handle());
        live(&ignored.child.handle());
    }
    // High native exit codes retain their full DWORD, including STILL_ACTIVE
    // as a completed value and STATUS_CONTROL_C_EXIT without a control receipt.
    for raw in [259, 0x8000_0001, 0xc000_013a, u32::MAX] {
        let normal = Work::new(&runtime, &image, "ignore", raw);
        let output = normal.complete(raw);
        assert!(output.ends_with(b"controls=0,0\n"));
        live(&unrelated.child.handle());
        live(&ignored.child.handle());
    }
    shell_native_wait(fish, &runtime, &image);
    live(&unrelated.child.handle());
    live(&ignored.child.handle());
    checked(unsafe { ResetEvent(ignored.ack.handle.as_raw_handle()) });
    assert!(matches!(
        ignored.child.request_ctrl_break().unwrap(),
        NativeBreakOutcome::Requested
    ));
    ignored.ack.wait();
    live(&ignored.child.handle());
    no_exit(&runtime, ignored.child.pid());
    // Normal-context receipt is a deterministic barrier after the ignored
    // request. No implicit force is allowed, even though the child has not exited.
    ignored
        .group
        .as_mut()
        .unwrap()
        .terminate_and_wait()
        .unwrap();
    wait(&ignored.child.handle());
    assert_eq!(ignored.child.wait(None).unwrap(), 1);
    status(1);
    assert!(matches!(
        ignored.child.request_ctrl_break().unwrap(),
        NativeBreakOutcome::AlreadyExited(1)
    ));
    ignored.group.take().unwrap().finish().unwrap();
    let forced = ignored.capture.finish().unwrap();
    assert!(forced.error.is_none());
    live(&unrelated.child.handle());
    assert_eq!(signal::console_control_epoch(), before_targeted);
    assert!(unrelated.complete(17).ends_with(b"controls=0,0\n"));
    assert_eq!(CALLBACK_FAILED.load(Ordering::Acquire), 0);
    owner.shutdown().unwrap();
}

struct TransferredProcess(OwnedHandle);
impl TransferredProcess {
    fn read(path: &Path) -> Self {
        Self(own(std::fs::read_to_string(path)
            .unwrap()
            .parse::<usize>()
            .unwrap() as *mut c_void))
    }
    fn completed(&self, expected: u32) {
        wait(&self.0);
        let mut raw = 0;
        checked(unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &mut raw) });
        assert_eq!(raw, expected);
        status(raw);
    }
}
impl Drop for TransferredProcess {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } == TIMEOUT {
            checked(unsafe { TerminateProcess(self.0.as_raw_handle(), 1) });
            wait(&self.0);
        }
    }
}
fn shell_native_wait(fish: &Path, runtime: &Arc<NativeRuntime>, image: &Path) {
    let bg_ready = Event::new("shell-background-ready");
    let bg_release = Event::new("shell-background-release");
    let fg_ready = Event::new("shell-foreground-ready");
    let fg_release = Event::new("shell-foreground-release");
    let ack = Event::new("shell-child-ack");
    let cwd = std::env::current_dir().unwrap();
    let bg_handle = cwd.join("n5-background-owned-handle");
    let fg_handle = cwd.join("n5-foreground-owned-handle");
    let command = |ready: &Event, release: &Event, raw: u32, handle: &Path| {
        format!(
            "{} n5-cancel-owned-child {} {} {} ignore {raw} {} {}",
            quote(image.to_str().unwrap()),
            quote(ready.name.to_str().unwrap()),
            quote(release.name.to_str().unwrap()),
            quote(ack.name.to_str().unwrap()),
            std::process::id(),
            quote(handle.to_str().unwrap())
        )
    };
    let code = format!(
        "{} &; set -l bg $last_pid; function n5_bg_exit --on-process-exit $bg; echo unrelated-exit; end; {}",
        command(&bg_ready, &bg_release, 17, &bg_handle),
        command(&fg_ready, &fg_release, 41, &fg_handle)
    );
    let capture = NativeCapture::new(65536, Arc::clone(runtime)).unwrap();
    let mut group = NativeLaunchGroup::new(Arc::clone(runtime)).unwrap();
    let shell = group
        .spawn(
            fish,
            &[
                fish.as_os_str().to_owned(),
                "-N".into(),
                "-c".into(),
                code.into(),
            ],
            &cwd,
            &std::env::vars_os().collect::<Vec<_>>(),
            &capture.io_chain(),
            false,
        )
        .unwrap();
    bg_ready.wait();
    let background = TransferredProcess::read(&bg_handle);
    fg_ready.wait();
    let foreground = TransferredProcess::read(&fg_handle);
    live(&background.0);
    live(&foreground.0);
    assert!(matches!(
        shell.request_ctrl_break().unwrap(),
        NativeBreakOutcome::Requested
    ));
    wait(&shell.handle());
    assert_eq!(
        shell.wait(None).unwrap(),
        130,
        "the shell interrupt is not a fabricated child cancellation"
    );
    ack.wait(); // Normal evaluator context requests Break only for foreground work.
    live(&background.0);
    live(&foreground.0);
    exit_event(runtime, shell.pid(), 130);
    bg_release.signal();
    fg_release.signal();
    background.completed(17);
    foreground.completed(41);
    let result = capture.finish().unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    let bytes = result.buffer.newline_serialized();
    let output = String::from_utf8(bytes).unwrap();
    assert!(
        !output.contains("unrelated-exit"),
        "live background job received a fabricated exit event"
    );
    assert_eq!(
        output
            .lines()
            .filter(|line| *line == "controls=0,0")
            .count(),
        1
    );
    assert_eq!(
        output
            .lines()
            .filter(|line| *line == "controls=0,1")
            .count(),
        1
    );
    group.finish().unwrap();
    assert_eq!(shell.wait(None).unwrap(), 130);
}

fn supervise_console_worker(mode: &str) {
    isolated_console();
    let _fallback = Handler::install();
    fish::threads::init();
    let runtime = NativeRuntime::new().unwrap();
    let capture = NativeCapture::new(65536, Arc::clone(&runtime)).unwrap();
    let image = std::env::current_exe().unwrap();
    // The supervisor is outside this private owned group. Even a worker panic
    // or timeout cannot orphan accepted/disowned descendants or touch a host job.
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let worker = group
        .spawn(
            &image,
            &[image.as_os_str().to_owned(), mode.into()],
            &std::env::current_dir().unwrap(),
            &std::env::vars_os().collect::<Vec<_>>(),
            &capture.io_chain(),
            false,
        )
        .unwrap();
    wait(&worker.handle());
    assert_eq!(
        worker.wait(None).unwrap(),
        0,
        "native control worker failed"
    );
    let output = capture.finish().unwrap();
    assert!(output.error.is_none(), "{:?}", output.error);
    assert!(output.buffer.newline_serialized().is_empty());
    group.finish().unwrap();
}

fn native_wait() {
    assert_ne!(
        unsafe { GetConsoleCP() },
        0,
        "supervisor's actual isolated console"
    );
    checked(unsafe { SetConsoleCtrlHandler(None, 0) }); // Runtime group opts into C.
    let _fallback = Handler::install();
    fish::threads::init();
    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
    let mut owner = signal::initialize_console_controls().unwrap();
    let runtime = NativeRuntime::new().unwrap();
    let capture = NativeCapture::new(65536, Arc::clone(&runtime)).unwrap();
    let bg_ready = Event::new("parser-background-ready");
    let bg_release = Event::new("parser-background-release");
    let fg_ready = Event::new("parser-foreground-ready");
    let fg_release = Event::new("parser-foreground-release");
    let ack = Event::new("parser-foreground-receipt");
    let cwd = std::env::current_dir().unwrap();
    let bg_path = cwd.join("n5-parser-background-handle");
    let fg_path = cwd.join("n5-parser-foreground-handle");
    let image = std::env::current_exe().unwrap();
    let command = |ready: &Event, release: &Event, raw: u32, path: &Path| {
        format!(
            "{} n5-cancel-owned-child {} {} {} ignore {raw} {} {}",
            quote(image.to_str().unwrap()),
            quote(ready.name.to_str().unwrap()),
            quote(release.name.to_str().unwrap()),
            quote(ack.name.to_str().unwrap()),
            std::process::id(),
            quote(path.to_str().unwrap())
        )
    };
    let ready_wait = Event::open(&fg_ready.name);
    let sender = std::thread::spawn(move || {
        ready_wait.wait();
        generate(0);
    });
    let code: fish::prelude::WString = format!(
        "{} &; {}",
        command(&bg_ready, &bg_release, 17, &bg_path),
        command(&fg_ready, &fg_release, 41, &fg_path)
    )
    .into();
    let result = parser.eval(&code, &capture.io_chain());
    sender.join().unwrap();
    assert_eq!(result.status.status_value(), 130);
    assert_eq!(result.status.native_raw(), None);
    assert!(!result.status.signal_exited());
    let shell_status = parser.last_statuses();
    assert_eq!(shell_status.status, 130);
    assert_eq!(shell_status.pipestatus, [130]);
    bg_ready.wait();
    let background = TransferredProcess::read(&bg_path);
    let foreground = TransferredProcess::read(&fg_path);
    ack.wait();
    live(&background.0);
    live(&foreground.0);
    let bg_pid = unsafe { GetProcessId(background.0.as_raw_handle()) };
    let fg_pid = unsafe { GetProcessId(foreground.0.as_raw_handle()) };
    assert_ne!(bg_pid, 0);
    assert_ne!(fg_pid, 0);
    let jobs = parser.jobs().to_vec();
    let mut live_members = 0;
    for process in jobs.iter().flat_map(|job| job.external_procs()) {
        if [bg_pid, fg_pid].contains(&process.pid().unwrap().get()) {
            assert!(
                process.status().is_empty(),
                "receipt fabricated a completed native stage"
            );
            live_members += 1;
        }
    }
    assert_eq!(live_members, 2);
    owner.rearm().unwrap();
    let observers: fish::prelude::WString = format!(
        "set -g n5_bg_exits 0; set -g n5_fg_exits 0; \
         function n5_bg --on-process-exit {bg_pid}; set -g n5_bg_exits (math $n5_bg_exits + 1); end; \
         function n5_fg --on-process-exit {fg_pid}; set -g n5_fg_exits (math $n5_fg_exits + 1); end; \
         test $n5_bg_exits -eq 0; and test $n5_fg_exits -eq 0").into();
    assert_eq!(
        parser
            .eval(&observers, &IoChain::new())
            .status
            .status_value(),
        0
    );
    bg_release.signal();
    fg_release.signal();
    background.completed(17);
    foreground.completed(41);
    let check: fish::prelude::WString =
        "test $n5_bg_exits -eq 1; and test $n5_fg_exits -eq 1".into();
    for _ in 0..2 {
        fish::proc::job_reap(&mut parser, false, None);
        fish::event::fire_delayed(&mut parser);
        assert_eq!(
            parser.eval(&check, &IoChain::new()).status.status_value(),
            0
        );
    }
    for process in jobs.iter().flat_map(|job| job.external_procs()) {
        let expected = if process.pid().unwrap().get() == bg_pid {
            17
        } else {
            41
        };
        assert_eq!(process.status().native_raw(), Some(expected));
        assert_eq!(process.status().status_value(), expected as i32);
        assert!(!process.status().signal_exited());
    }
    let output = capture.finish().unwrap();
    assert!(output.error.is_none(), "{:?}", output.error);
    assert!(!output.interrupted);
    let text = String::from_utf8(output.buffer.newline_serialized()).unwrap();
    assert_eq!(
        text.lines().filter(|line| *line == "controls=0,0").count(),
        1
    );
    assert_eq!(
        text.lines().filter(|line| *line == "controls=0,1").count(),
        1
    );
    // Reaping/removing jobs and dispatching their handlers cannot invalidate the
    // separately retained exact process capabilities or mutate their raw DWORDs.
    background.completed(17);
    foreground.completed(41);
    owner.shutdown().unwrap();
}

fn child(args: &[OsString]) {
    let ready = Event::open(&args[0]);
    let release = Event::open(&args[1]);
    let ack = Event::open(&args[2]);
    let kind = args[3].to_str().unwrap();
    let raw = args[4].to_str().unwrap().parse::<u32>().unwrap();
    if matches!(kind, "detached" | "foreign") {
        let stdout = unsafe { GetStdHandle(-11i32 as u32) };
        let stderr = unsafe { GetStdHandle(-12i32 as u32) };
        checked(unsafe { FreeConsole() });
        if kind == "foreign" {
            checked(unsafe { AllocConsole() });
        }
        // Detaching/allocating a console must not replace our retained redirected
        // capture endpoints with CONOUT$ or lose the child's observable receipts.
        checked(unsafe { SetStdHandle(-11i32 as u32, stdout) });
        checked(unsafe { SetStdHandle(-12i32 as u32, stderr) });
    }
    let handler = (unsafe { GetConsoleCP() } != 0).then(Handler::install);
    if kind == "gui" {
        assert_eq!(
            unsafe { GetConsoleCP() },
            0,
            "GUI fixture unexpectedly inherited a console"
        );
    }
    if args.len() == 7 {
        // This PID is used only to open the controller for DUP_HANDLE. The
        // controller receives our exact current-process capability, never a PID
        // selected later as a termination target.
        let controller =
            own(unsafe { OpenProcess(0x0040, 0, args[5].to_str().unwrap().parse().unwrap()) });
        let mut transferred = std::ptr::null_mut();
        checked(unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                GetCurrentProcess(),
                controller.as_raw_handle(),
                &mut transferred,
                0x0010_1001,
                0,
                0,
            )
        });
        std::fs::write(&args[6], (transferred as usize).to_string()).unwrap();
    }
    ready.signal();
    let handles = [
        release.handle.as_raw_handle(),
        callback_event().as_raw_handle(),
    ];
    loop {
        // This is an owned lifetime barrier, not an independently expiring
        // watchdog. The outside controller bounds waits and retains the exact
        // child/group for cleanup, including while a console close is pending.
        let result = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, u32::MAX) };
        match result {
            0 => break,
            1 => {
                checked(unsafe { ResetEvent(callback_event().as_raw_handle()) });
                // All acknowledgement, I/O and exit decisions happen here.
                let c = COUNTS[0].load(Ordering::Acquire);
                let b = COUNTS[1].load(Ordering::Acquire);
                assert_eq!(c, 0, "targeted Break must not become Ctrl-C");
                assert!(b > 0);
                if kind == "ack" {
                    println!("ack=break");
                }
                io::stdout().flush().unwrap();
                ack.signal();
            }
            other => panic!("child barrier wait {other:#x}"),
        }
    }
    println!(
        "controls={},{}",
        COUNTS[0].load(Ordering::Acquire),
        COUNTS[1].load(Ordering::Acquire)
    );
    io::stdout().flush().unwrap();
    drop(handler);
    assert_eq!(CALLBACK_FAILED.load(Ordering::Acquire), 0);
    unsafe { ExitProcess(raw) }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn blocked(mode: &str, fish: &Path) {
    isolated_console();
    let _fallback = Handler::install();
    // Retain both ends: EOF can never masquerade as successful cancellation.
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    checked(unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 4096) });
    let read = own(read);
    let _write = own(write);
    checked(unsafe { SetStdHandle(-10i32 as u32, read.as_raw_handle()) });
    fish::threads::init();
    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
    let mut owner = signal::initialize_console_controls().unwrap();
    let runtime = NativeRuntime::new().unwrap();
    let image = std::env::current_exe().unwrap();
    let unrelated = Work::new(&runtime, &image, "ignore", 19);
    let ready = Event::new("evaluation-entered");
    let marker = quote(image.to_str().unwrap());
    let event = quote(ready.name.to_str().unwrap());
    let definitions: fish::prelude::WString =
        "set -g n5_c 0; set -g n5_b 0; function n5_c_handler --on-signal CTRL_C; set -g n5_c (math $n5_c + 1); end; function n5_b_handler --on-signal CTRL_BREAK; set -g n5_b (math $n5_b + 1); end".into();
    assert_eq!(
        parser
            .eval(&definitions, &IoChain::new())
            .status
            .status_value(),
        0
    );
    let ready_wait = ready.handle;
    let sender = std::thread::spawn(move || {
        wait(&ready_wait);
        generate(0);
    });
    let code = if mode == "n5-cancel-read" {
        format!("{marker} n5-cancel-ready {event}; read -l n5_result")
    } else {
        format!("{marker} n5-cancel-ready {event}; while true; end")
    };
    let code: fish::prelude::WString = code.into();
    let result = parser.eval(&code, &IoChain::new());
    sender.join().unwrap();
    assert_eq!(result.status.status_value(), 130);
    assert_eq!(
        result.status.native_raw(),
        None,
        "evaluator interrupt is not a child DWORD"
    );
    assert!(!result.status.signal_exited());
    live(&unrelated.child.handle());
    owner.rearm().unwrap();
    fish::event::fire_delayed(&mut parser);
    // Event dispatch is a separate normal-thread action from receipt/rearm.
    let check: fish::prelude::WString = "test $n5_c -eq 1; and test $n5_b -eq 0".into();
    assert_eq!(
        parser.eval(&check, &IoChain::new()).status.status_value(),
        0
    );
    fish::event::fire_delayed(&mut parser);
    assert_eq!(
        parser.eval(&check, &IoChain::new()).status.status_value(),
        0
    );
    // Also exercise the actual selected debug/release shell image, rather than
    // claiming an owner API test alone is a shell executable cancellation test.
    let shell_ready = Event::new("actual-shell-entered");
    let body = if mode == "n5-cancel-read" {
        "read -l n5_result"
    } else {
        "while true; end"
    };
    let mut command = Command::new(fish);
    command
        .args(["-N", "-c", &format!("printf 'n5-entered\\n'; {body}")])
        .stdin(Stdio::piped());
    let mut shell = Process::spawn(&mut command, Some(shell_ready.name.clone()));
    shell_ready.wait();
    live(&shell.child);
    generate(0);
    let (raw, output) = shell.finish();
    assert_eq!(raw, 130, "{}", String::from_utf8_lossy(&output[1]));
    assert_eq!(output[0], b"n5-entered\n");
    assert!(
        output[1].is_empty(),
        "{}",
        String::from_utf8_lossy(&output[1])
    );
    live(&unrelated.child.handle());
    owner.rearm().unwrap();
    assert!(unrelated.complete(19).ends_with(b"controls=0,0\n"));
    owner.shutdown().unwrap();
}

fn physical_stdin(fish: &Path) {
    isolated_console();
    let _fallback = Handler::install();
    let ready = Event::new("physical-stdin-entered");
    let mut command = Command::new(fish);
    command
        // -C supplies only a startup barrier: there is deliberately no -c or
        // script operand. All source acquisition must use physical CLI stdin.
        .args(["-N", "-C", "printf 'n5-entered\\n'"])
        .stdin(Stdio::piped())
        .creation_flags(0x0000_0200); // Owned process group, same private console.
    let mut shell = Process::spawn(&mut command, Some(ready.name.clone()));
    // Never write a byte or close this endpoint until the shell has exited.
    // A raw synchronous stdin read cannot satisfy this wait through EOF.
    let writer = shell.child.stdin.take().unwrap();
    ready.wait();
    live(&shell.child);
    checked(unsafe { GenerateConsoleCtrlEvent(1, shell.child.id()) });
    let (raw, output) = shell.finish();
    assert_eq!(raw, 130, "{}", String::from_utf8_lossy(&output[1]));
    assert_eq!(output[0], b"n5-entered\n");
    // Batch input cancellation may report the interrupted physical read. The
    // contract is exit 130 without evaluating input, not diagnostic silence.
    drop(writer);
}

fn pipe_available(read: &impl AsRawHandle) -> u32 {
    let mut available = 0;
    checked(unsafe {
        PeekNamedPipe(
            read.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    });
    available
}

fn supervise_builtin_output() {
    isolated_console();
    let _fallback = Handler::install();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("n5-cancel-output-worker")
        .stdin(Stdio::null())
        .creation_flags(0x0000_0010); // Worker owns its own real console as well.
    // A regressed Interrupted retry loop must fail a bounded outside wait.
    // Process cleanup terminates only this retained worker, not a PID/host job.
    let mut worker = Process::spawn(&mut command, None);
    let (raw, output) = worker.finish();
    assert_eq!(raw, 0, "{}", String::from_utf8_lossy(&output[1]));
    assert!(output[0].is_empty());
    assert!(
        output[1].is_empty(),
        "{}",
        String::from_utf8_lossy(&output[1])
    );
}

fn builtin_output() {
    isolated_console();
    let _fallback = Handler::install();
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    checked(unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 4096) });
    let read = own(read);
    let write = own(write);
    let mut capacity = 0;
    checked(unsafe {
        GetNamedPipeInfo(
            read.as_raw_handle(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut capacity,
            std::ptr::null_mut(),
        )
    });
    assert!(capacity > 0 && capacity < 1_048_576);
    let stdout = unsafe { GetStdHandle(-11i32 as u32) };
    // Import this real synchronous pipe as the evaluator's root stdout before
    // root descriptors initialize. No synthetic OutputStream or mock builtin.
    checked(unsafe { SetStdHandle(-11i32 as u32, write.as_raw_handle()) });
    fish::threads::init();
    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    let mut parser = fish::parser::Parser::new(vars, fish::parser::CancelBehavior::Return).unwrap();
    let mut owner = signal::initialize_console_controls().unwrap();
    let io = IoChain::new();
    let observed = read.try_clone().unwrap();
    let full = Event::new("builtin-output-full");
    let full_wait = full.handle.try_clone().unwrap();
    let sender = std::thread::spawn(move || {
        let deadline = Instant::now() + LIMIT;
        // Peek is observational only: the consumer never reads/closes/releases
        // space until parser.eval has actually returned after the control.
        while pipe_available(&observed) < capacity {
            assert!(Instant::now() < deadline, "builtin never filled its pipe");
            std::thread::sleep(Duration::from_millis(1));
        }
        full.signal();
        generate(1);
    });
    // A single real builtin append exceeds kernel capacity. Once Peek sees the
    // pipe full, printf is still appending its field, not between parser jobs.
    // Interrupted must escape FdOutputStream rather than Write::write_all retry.
    let code: fish::prelude::WString = "printf '%1048576s' x".into();
    let result = parser.eval(&code, &io);
    sender.join().unwrap();
    wait(&full_wait);
    assert_eq!(result.status.status_value(), 130);
    assert_eq!(result.status.native_raw(), None);
    assert!(!result.status.signal_exited());
    assert_eq!(signal::signal_check_cancel(), RawSignal::CTRL_BREAK.code());
    assert_eq!(
        pipe_available(&read),
        capacity,
        "cancellation must unwind without draining or closing the consumer"
    );
    // Only after unwind is demonstrated may the same consumer release space.
    // The same IoChain/root endpoint and synchronous worker are reused below.
    owner.rearm().unwrap();
    assert_eq!(signal::signal_check_cancel(), 0);
    let mut consumer = std::fs::File::from(read);
    let mut prefix = vec![0; capacity as usize];
    consumer.read_exact(&mut prefix).unwrap();
    assert!(prefix.iter().all(|byte| *byte == b' '));
    assert_eq!(pipe_available(&consumer), 0);
    let code: fish::prelude::WString = "printf 'rearmed\\n'".into();
    assert_eq!(parser.eval(&code, &io).status.status_value(), 0);
    assert_eq!(pipe_available(&consumer), b"rearmed\n".len() as u32);
    let mut output = [0; b"rearmed\n".len()];
    consumer.read_exact(&mut output).unwrap();
    assert_eq!(&output, b"rearmed\n");
    assert_eq!(pipe_available(&consumer), 0);
    owner.shutdown().unwrap();
    checked(unsafe { SetStdHandle(-11i32 as u32, stdout) });
}

fn pipe() {
    assert_ne!(
        unsafe { GetConsoleCP() },
        0,
        "supervisor's isolated console"
    );
    let _fallback = Handler::install();
    fish::threads::init();
    let mut owner = signal::initialize_console_controls().unwrap();
    let runtime = NativeRuntime::new().unwrap();
    let image = std::env::current_exe().unwrap();
    let unrelated = Work::new(&runtime, &image, "ignore", 11);
    let producer = Work::new(&runtime, &image, "capture", 29);
    // The writer remains live and retains its pipe endpoint. finish must cancel
    // and drain the actual outstanding read, not wait for process exit or EOF.
    let cancellation = owner.cancellation();
    // Only the controller's console group is being interrupted. The producer
    // and unrelated fixture each own a separate NEW_PROCESS_GROUP.
    checked(unsafe { GenerateConsoleCtrlEvent(1, std::process::id()) });
    wait(&cancellation.handle());
    assert_eq!(signal::signal_check_cancel(), RawSignal::CTRL_BREAK.code());
    producer.capture.cancel();
    let result = producer.capture.finish().unwrap();
    assert!(result.interrupted);
    assert!(result.error.is_none(), "{:?}", result.error);
    assert!(!result.buffer.discarded());
    assert_eq!(result.buffer.newline_serialized(), b"available:\0\xff\r\n");
    live(&producer.child.handle());
    live(&unrelated.child.handle());
    no_exit(&runtime, producer.child.pid());
    owner.rearm().unwrap();
    producer.release.signal();
    wait(&producer.child.handle());
    assert_eq!(producer.child.wait(None).unwrap(), 29);
    // capture was consumed deliberately above, so finish only the retained group.
    let mut producer = producer;
    producer.group.take().unwrap().finish().unwrap();
    assert_eq!(producer.child.wait(None).unwrap(), 29);
    assert!(unrelated.complete(11).ends_with(b"controls=0,0\n"));
    owner.shutdown().unwrap();
}

// Std Child ownership covers actual fish and the nested console-close victim.
// Streams drain concurrently; cleanup always uses the retained owned handle.
struct Process {
    child: Child,
    streams: mpsc::Receiver<(usize, Vec<u8>)>,
}
impl Process {
    fn spawn(command: &mut Command, ready: Option<OsString>) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (send, streams) = mpsc::channel();
        for (index, mut stream) in [
            (
                0,
                Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
            ),
            (
                1,
                Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
            ),
        ] {
            let send = send.clone();
            let mut ready = if index == 0 { ready.clone() } else { None };
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let mut block = [0u8; 8192];
                loop {
                    let count = stream.read(&mut block).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&block[..count]);
                    if ready.is_some()
                        && bytes
                            .windows(b"n5-entered\n".len())
                            .any(|part| part == b"n5-entered\n")
                    {
                        Event::open(&ready.take().unwrap()).signal();
                    }
                }
                let _ = send.send((index, bytes));
            });
        }
        Self { child, streams }
    }
    fn finish(&mut self) -> (u32, Vec<Vec<u8>>) {
        wait(&self.child);
        let code = self.child.wait().unwrap().code().unwrap() as u32;
        let mut result = vec![Vec::new(), Vec::new()];
        for _ in 0..2 {
            let (index, bytes) = self
                .streams
                .recv_timeout(LIMIT)
                .expect("bounded concurrent stream drain");
            result[index] = bytes;
        }
        (code, result)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.child.as_raw_handle(), 0) } == TIMEOUT {
            self.child.kill().unwrap();
            wait(&self.child);
            self.child.wait().unwrap();
        }
    }
}
fn close() {
    isolated_console();
    let _fallback = Handler::install();
    fish::threads::init();
    let runtime = NativeRuntime::new().unwrap();
    let image = std::env::current_exe().unwrap();
    let unrelated = Work::new(&runtime, &image, "ignore", 13);
    assert_eq!(unsafe { GenerateConsoleCtrlEvent(2, 0) }, 0);
    let ready = Event::new("close-ready");
    let metadata = std::env::current_dir().unwrap().join("n5-close-client");
    let arguments = format!(
        "n5-cancel-close-victim \"{}\" \"{}\" {}",
        ready.name.to_str().unwrap(),
        metadata.display(),
        std::process::id(),
    );
    let mut victim = super::conpty::Session::spawn(&image, &arguments);
    ready.wait();
    let handles: Vec<usize> = std::fs::read_to_string(&metadata)
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    assert_eq!(handles.len(), 2);
    assert_eq!(handles[0] as u32, unsafe {
        GetProcessId(victim.process.as_raw_handle())
    });
    let receipt = own(handles[1] as *mut c_void);
    live(&victim.process);
    live(&receipt);
    // This documented native operation sends real CTRL_CLOSE to connected
    // clients, independently of a legacy/proxy HWND's window-message policy.
    victim.close();
    wait(&victim.process);
    let mut raw = 0;
    checked(unsafe { GetExitCodeProcess(victim.process.as_raw_handle(), &mut raw) });
    // Receipt and actual DWORD remain independent observations. Abrupt close
    // may prevent the best-effort callback handoff; no forced cleanup is success.
    assert!(matches!(
        unsafe { WaitForSingleObject(receipt.as_raw_handle(), 0) },
        0 | TIMEOUT
    ));
    status(raw);
    victim.drain();
    live(&unrelated.child.handle());
    assert!(unrelated.complete(13).ends_with(b"controls=0,0\n"));
}
fn close_victim(args: &[OsString]) {
    isolated_console();
    let owner = signal::initialize_console_controls().unwrap();
    // The supervisor already owns this exact child process. Transfer only the
    // independent production cancellation capability before readiness.
    let controller =
        own(unsafe { OpenProcess(0x0040, 0, args[2].to_str().unwrap().parse().unwrap()) });
    let duplicate = |source, access| {
        let mut target = std::ptr::null_mut();
        checked(unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                source,
                controller.as_raw_handle(),
                &mut target,
                access,
                0,
                0,
            )
        });
        target as usize
    };
    let cancellation = owner.cancellation();
    let receipt = duplicate(cancellation.handle().as_raw_handle(), 0x0010_0000);
    std::fs::write(&args[1], format!("{}\n{receipt}\n", std::process::id())).unwrap();
    Event::open(&args[0]).signal();
    // No normal-context acknowledgment or cleanup is required by CTRL_CLOSE.
    // Keep the victim alive until Windows closes it; only the outside retained
    // process wait has a deadline. A local ten-second panic is not close proof.
    let forever = Event::new("close-no-graceful-guarantee");
    assert_eq!(
        unsafe { WaitForSingleObject(forever.handle.as_raw_handle(), u32::MAX) },
        0,
        "close lifetime barrier unexpectedly failed"
    );
    panic!("unreleased close lifetime barrier");
}

pub fn run(mode: &str, args: Vec<OsString>) {
    assert!(mode.starts_with("n5-cancel-"));
    match mode {
        "n5-cancel-controls" => controls(),
        "n5-cancel-requests" => requests(
            Path::new(args.first().expect("required fish artifact")),
            Path::new(args.get(1).expect("required GUI artifact")),
        ),
        "n5-cancel-read" | "n5-cancel-evaluator" => blocked(
            mode,
            Path::new(args.first().expect("required fish artifact")),
        ),
        "n5-cancel-stdin" => {
            physical_stdin(Path::new(args.first().expect("required fish artifact")))
        }
        "n5-cancel-output" => supervise_builtin_output(),
        "n5-cancel-output-worker" => builtin_output(),
        "n5-cancel-pipe" => supervise_console_worker("n5-cancel-pipe-worker"),
        "n5-cancel-pipe-worker" => pipe(),
        "n5-cancel-native-wait" => supervise_console_worker("n5-cancel-native-wait-worker"),
        "n5-cancel-native-wait-worker" => native_wait(),
        "n5-cancel-close" => close(),
        "n5-cancel-close-victim" => close_victim(&args),
        "n5-cancel-child" if args[3] == "capture" => {
            let ready = Event::open(&args[0]);
            let release = Event::open(&args[1]);
            io::stdout().write_all(b"available:\0\xff\r\n").unwrap();
            io::stdout().flush().unwrap();
            ready.signal();
            release.wait();
            unsafe { ExitProcess(args[4].to_str().unwrap().parse().unwrap()) }
        }
        "n5-cancel-child" | "n5-cancel-owned-child" => child(&args),
        "n5-cancel-ready" => Event::open(&args[0]).signal(),
        other => panic!("unknown cancellation probe mode {other}"),
    }
}
