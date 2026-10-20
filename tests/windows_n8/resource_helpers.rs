//! Real temporary-resource ownership exercised inside one live native fish.
//! Called by the existing N8 native-helper family; no additional gate/family.
use super::{
    cases_probe::Event,
    console::Session,
    editor::{capture, finish, init},
    fixture::{Fixture, quote},
};
use std::ffi::{OsStr, c_void};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(30);
#[link(name = "kernel32")]
unsafe extern "system" {
    fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32) -> *mut c_void;
    fn FindNextChangeNotification(handle: *mut c_void) -> i32;
    fn FindCloseChangeNotification(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
}
fn qp(path: &Path) -> String { quote(path.to_str().expect("reversible resource fixture path")) }
fn wide(path: &Path) -> Vec<u16> { path.as_os_str().encode_wide().chain(Some(0)).collect() }

// Arm before the operation, then rearm before observing each pathname. Removal
// waits are bounded native notifications, not scheduler sleeps or busy polling.
struct Changes(*mut c_void);
impl Changes {
    fn new(root: &Path) -> Self {
        let handle = unsafe { FindFirstChangeNotificationW(wide(root).as_ptr(), 1, 3) };
        assert!(!handle.is_null() && handle as usize != usize::MAX,
            "required temporary-directory notification: {}", std::io::Error::last_os_error());
        Self(handle)
    }
    fn absent(&self, path: &Path) {
        let deadline = Instant::now() + LIMIT;
        loop {
            assert_ne!(unsafe { FindNextChangeNotification(self.0) }, 0, "rearm owned temporary-directory notification");
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                Err(error) => panic!("observe owned resource {}: {error}", path.display()),
                Ok(_) => {}
            }
            let remaining = deadline.checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("resource cleanup exceeded bound: {}", path.display()));
            assert_eq!(unsafe { WaitForSingleObject(self.0, remaining.as_millis().max(1) as u32) }, 0,
                "bounded resource removal notification: {}", path.display());
        }
    }
}
impl Drop for Changes {
    fn drop(&mut self) {
        let closed = unsafe { FindCloseChangeNotification(self.0) };
        if !std::thread::panicking() { assert_ne!(closed, 0, "close owned resource notification"); }
    }
}

struct Consumer(OwnedHandle);
impl Consumer {
    fn acknowledged(pid_file: &Path) -> Self {
        let pid: u32 = fs::read_to_string(pid_file).expect("ready consumer publishes its PID").parse().unwrap();
        // Our consumer has signalled its private ready event and remains blocked
        // at a release event/input read. This cannot reopen a recycled stranger.
        let raw = unsafe { OpenProcess(0x0010_1000, 0, pid) };
        assert!(!raw.is_null(), "retain acknowledged resource consumer: {}", std::io::Error::last_os_error());
        let result = Self(unsafe { OwnedHandle::from_raw_handle(raw) });
        result.live();
        result
    }
    fn live(&self) {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) }, 258,
            "acknowledged resource consumer exited before controlled release");
    }
    fn wait(&self, expected: u32) {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT.as_millis() as u32) }, 0,
            "bounded owned resource consumer exit");
        let mut code = 0;
        assert_ne!(unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &mut code) }, 0);
        assert_eq!(code, expected, "real resource consumer exit status");
    }
}

const CONSUMER: &str = r#"import ctypes,os,pathlib,sys,time
from ctypes import wintypes
assert sys.platform == 'win32', 'N8 resource consumers require native Windows Python'
mode,kind,label,pid_name,ready_name,release_name,receipt_name,argument = sys.argv[1:]
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.OpenEventW.argtypes = [wintypes.DWORD,wintypes.BOOL,wintypes.LPCWSTR]
kernel.OpenEventW.restype = wintypes.HANDLE
kernel.SetEvent.argtypes = [wintypes.HANDLE]
kernel.SetEvent.restype = wintypes.BOOL
kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE,wintypes.DWORD]
kernel.WaitForSingleObject.restype = wintypes.DWORD
kernel.CloseHandle.argtypes = [wintypes.HANDLE]
kernel.CloseHandle.restype = wintypes.BOOL
ready = kernel.OpenEventW(2,False,ready_name)
release = kernel.OpenEventW(0x00100000,False,release_name)
assert ready and release, ctypes.WinError(ctypes.get_last_error())
cancelled = None
handler_installed = False
try:
    other = os.environ['FISH_N8_RESOURCE_OTHER']
    file_path,nested_path = (argument,other) if kind == 'argv-file' else (other,argument)
    file_path,nested_path = pathlib.Path(file_path),pathlib.Path(nested_path)
    def consume():
        # read_bytes opens and closes each file. No lingering Python file handle
        # can accidentally prevent an early owner deletion and mask a lease bug.
        assert file_path.read_bytes() == 'resource-file:雪'.encode('utf-8')
        assert nested_path.read_bytes() == 'resource-directory:雪'.encode('utf-8')
    consume()
    def acknowledge():
        pathlib.Path(pid_name).write_text(str(os.getpid()),encoding='ascii')
        assert kernel.SetEvent(ready), ctypes.WinError(ctypes.get_last_error())
    if mode == 'hold':
        acknowledge()
        assert kernel.WaitForSingleObject(release,30000) == 0, 'bounded consumer release event'
        consume()
        pathlib.Path(receipt_name).write_bytes(b'argv-env-consumed-after-parent-return')
    elif mode == 'console':
        # CPython input() waits on a SIGINT-only wakeup after ReadConsoleW is
        # aborted. A Python SIGBREAK handler alone cannot wake that read. Own
        # native console input and a native cancellation event instead; only an
        # actual console handler invocation may release the cancellation wait.
        class Key(ctypes.Structure):
            _fields_ = [('down',wintypes.BOOL),('repeat',wintypes.WORD),
                        ('virtual_key',wintypes.WORD),('scan',wintypes.WORD),
                        ('character',wintypes.WCHAR),('controls',wintypes.DWORD)]
        class Payload(ctypes.Union):
            _fields_ = [('key',Key),('reserved',wintypes.BYTE*16)]
        class Record(ctypes.Structure):
            _fields_ = [('kind',wintypes.WORD),('payload',Payload)]
        assert ctypes.sizeof(Record) == 20
        Handler = ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.DWORD)
        kernel.CreateEventW.argtypes = [ctypes.c_void_p,wintypes.BOOL,wintypes.BOOL,wintypes.LPCWSTR]
        kernel.CreateEventW.restype = wintypes.HANDLE
        kernel.SetConsoleCtrlHandler.argtypes = [Handler,wintypes.BOOL]
        kernel.SetConsoleCtrlHandler.restype = wintypes.BOOL
        kernel.GetStdHandle.argtypes = [wintypes.DWORD]
        kernel.GetStdHandle.restype = wintypes.HANDLE
        kernel.GetConsoleMode.argtypes = [wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]
        kernel.GetConsoleMode.restype = wintypes.BOOL
        kernel.WaitForMultipleObjects.argtypes = [wintypes.DWORD,ctypes.POINTER(wintypes.HANDLE),wintypes.BOOL,wintypes.DWORD]
        kernel.WaitForMultipleObjects.restype = wintypes.DWORD
        kernel.ReadConsoleInputW.argtypes = [wintypes.HANDLE,ctypes.POINTER(Record),wintypes.DWORD,ctypes.POINTER(wintypes.DWORD)]
        kernel.ReadConsoleInputW.restype = wintypes.BOOL
        cancelled = kernel.CreateEventW(None,True,False,None)
        assert cancelled, ctypes.WinError(ctypes.get_last_error())
        control_kind = None
        @Handler
        def control_received(kind):
            global control_kind
            if kind not in (0,1):
                return False
            control_kind = kind
            return kernel.SetEvent(cancelled)
        assert kernel.SetConsoleCtrlHandler(control_received,True), ctypes.WinError(ctypes.get_last_error())
        handler_installed = True
        console_input = kernel.GetStdHandle(-10 & 0xffffffff)
        console_mode = wintypes.DWORD()
        assert kernel.GetConsoleMode(console_input,ctypes.byref(console_mode)), ctypes.WinError(ctypes.get_last_error())
        handles = (wintypes.HANDLE*2)(cancelled,console_input)
        acknowledge()
        print('N8-RESOURCE-CHILD-READ:'+label+':',end='',flush=True)
        deadline = time.monotonic()+30
        while True:
            remaining = deadline-time.monotonic()
            assert remaining > 0, 'bounded native consumer console cancellation'
            result = kernel.WaitForMultipleObjects(2,handles,False,max(1,int(remaining*1000)))
            if result == 0:
                name = {0:'CTRL_C',1:'CTRL_BREAK'}[control_kind]
                pathlib.Path(receipt_name).write_text('actual-console-'+name,encoding='ascii')
                raise SystemExit(130)
            assert result == 1, 'native console input/cancellation wait: '+hex(result)
            record,count = Record(),wintypes.DWORD()
            assert kernel.ReadConsoleInputW(console_input,ctypes.byref(record),1,ctypes.byref(count)), ctypes.WinError(ctypes.get_last_error())
            assert count.value == 1
            if record.kind == 1 and record.payload.key.down and record.payload.key.character != '\0':
                raise AssertionError('console consumer must be cancelled, not given ordinary input')
    else:
        raise AssertionError('unknown resource consumer mode')
finally:
    if handler_installed:
        assert kernel.SetConsoleCtrlHandler(control_received,False)
    if cancelled:
        assert kernel.CloseHandle(cancelled)
    assert kernel.CloseHandle(ready)
    assert kernel.CloseHandle(release)
"#;

struct Paths { file: PathBuf, directory: PathBuf }
impl Paths {
    fn record(f: &Fixture, label: &str) -> Self {
        let record = fs::read_to_string(f.root.join(format!("resource-{label}.paths"))).unwrap();
        let paths: Vec<_> = record.lines().map(PathBuf::from).collect();
        assert_eq!(paths.len(), 2, "closed resource identity record");
        Self { file: paths[0].clone(), directory: paths[1].clone() }
    }
    fn live(&self) {
        assert_eq!(fs::read(&self.file).unwrap(), "resource-file:雪".as_bytes());
        assert_eq!(fs::read(self.directory.join("child.txt")).unwrap(), "resource-directory:雪".as_bytes());
    }
    fn removed(&self, changes: &Changes) {
        changes.absent(&self.file);
        changes.absent(&self.directory);
    }
}
fn allocation(f: &Fixture, label: &str) -> String {
    // Allocation in this inner helper must belong to the outer active function,
    // not disappear as soon as the command substitution or helper returns.
    format!("function n8_resource_allocate; \
        set -g n8_owned_file (__fish_native_fs make-temp-file); or return 91; \
        set -g n8_owned_directory (__fish_native_fs make-temp-directory); or return 92; \
        printf '%s' 'resource-file:雪' >$n8_owned_file; or return 93; \
        printf '%s' 'resource-directory:雪' >\"$n8_owned_directory/child.txt\"; or return 94; \
        printf '%s\\n%s\\n' $n8_owned_file $n8_owned_directory >{}; \
        end", qp(&f.root.join(format!("resource-{label}.paths"))))
}
fn scope(f: &Fixture, label: &str, body: &str) -> String {
    format!("{}; function n8_resource_scope; n8_resource_allocate; or return $status; \
        test -f $n8_owned_file; or return 95; \
        test -f \"$n8_owned_directory/child.txt\"; or return 96; \
        {body}; end; n8_resource_scope; set -l n8_result $status; printf '%s' $n8_result >{}\r",
        allocation(f, label), qp(&f.root.join(format!("resource-{label}.status"))))
}
fn status(s: &mut Session, f: &Fixture, label: &str) -> u32 {
    let bytes = s.wait_file(&f.root.join(format!("resource-{label}.status")));
    std::str::from_utf8(&bytes).unwrap().parse().expect("real function status")
}
fn recover(s: &mut Session, f: &Fixture, original: (u32, u32), next: &mut usize) {
    s.wait_protocol(true);
    let state = s.snapshot();
    assert_eq!((state.input_mode, state.output_mode), original, "resource cleanup/cancellation must restore fish input ownership and modes");
    *next += 1;
    capture(s, f, *next, "");
}
fn run_scope(s: &mut Session, f: &Fixture, changes: &Changes, original: (u32, u32), next: &mut usize, label: &str, body: &str, expected: Option<u32>) {
    s.send(scope(f, label, body).as_bytes());
    let actual = status(s, f, label);
    if let Some(expected) = expected { assert_eq!(actual, expected, "resource {label} scope"); }
    else { assert_ne!(actual, 0, "resource {label} must fail usefully"); }
    recover(s, f, original, next);
    Paths::record(f, label).removed(changes);
}
fn consumer_command(f: &Fixture, python: &Path, script: &Path, label: &str, ready: &Event, release: &Event, mode: &str, kind: &str) -> String {
    let argument = if kind == "argv-file" { "$n8_owned_file" } else { "\"$n8_owned_directory/child.txt\"" };
    format!("{} -I -X utf8 {} {} {} {} {} {} {} {} {argument}",
        qp(python), qp(script), quote(mode), quote(kind), quote(label),
        qp(&f.root.join(format!("resource-{label}.pid"))), quote(&ready.name), quote(&release.name),
        qp(&f.root.join(format!("resource-{label}.receipt"))))
}
fn environment(kind: &str) -> &'static str {
    if kind == "argv-file" { "set -lx FISH_N8_RESOURCE_OTHER \"$n8_owned_directory/child.txt\"" }
    else { "set -lx FISH_N8_RESOURCE_OTHER $n8_owned_file" }
}

pub(super) fn exercise(f: &Fixture) {
    let python = std::env::var_os("FISH_NATIVE_TEST_PYTHON").map(PathBuf::from)
        .expect("N8 resource cleanup requires the mandatory real FISH_NATIVE_TEST_PYTHON fixture; never skipped");
    assert!(python.is_absolute() && python.is_file(), "required absolute native Python: {}", python.display());
    let script = f.root.join("resource consumer 雪.py");
    fs::write(&script, CONSUMER).unwrap();
    let temporary = f.root.join("resource helper temporary files");
    fs::create_dir(&temporary).unwrap();
    let changes = Changes::new(&temporary);
    let init = init("");
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-i"), OsStr::new("-C"), OsStr::new(&init)]);
    command.env("TEMP", &temporary).env("TMP", &temporary);
    let mut s = Session::start(&command, &f.probe, &f.root);
    s.wait_protocol(true);
    let state = s.snapshot();
    let original = (state.input_mode, state.output_mode);
    let mut next = 0;

    run_scope(&mut s, f, &changes, original, &mut next, "normal", "return 0", Some(0));
    let error_file = f.root.join("resource-error.diagnostic");
    let body = format!("__fish_native_fs read \"$n8_owned_directory/absent\" 2>{}; set -l failed $status; test $failed -ne 0; or return 97; return $failed", qp(&error_file));
    run_scope(&mut s, f, &changes, original, &mut next, "error", &body, None);

    run_scope(&mut s, f, &changes, original, &mut next, "explicit-release",
        "__fish_native_fs remove $n8_owned_file; or return 97; \
         __fish_native_fs remove-temp-directory $n8_owned_directory; or return 98; \
         test -e $n8_owned_file; and return 99; test -e $n8_owned_directory; and return 100; return 0", Some(0));
    let transfer_error = f.root.join("resource-transfer-rejected.diagnostic");
    let body = format!("__fish_native_fs transfer-temp-file $n8_owned_file 2>{}; set -l failed $status; \
        test $failed -ne 0; or return 97; test -f $n8_owned_file; or return 98; \
        test -f \"$n8_owned_directory/child.txt\"; or return 99; return 23", qp(&transfer_error));
    run_scope(&mut s, f, &changes, original, &mut next, "unaccepted-transfer", &body, Some(23));

    let invalid = f.root.join("invalid resource consumer.exe");
    fs::write(&invalid, b"owned invalid native Windows image\r\n").unwrap();
    let launch_error = f.root.join("resource-failed-launch.diagnostic");
    let body = format!("{} $n8_owned_file \"$n8_owned_directory/child.txt\" 2>{}; \
        set -l failed $status; test $failed -ne 0; or return 97; return $failed", qp(&invalid), qp(&launch_error));
    run_scope(&mut s, f, &changes, original, &mut next, "failed-launch", &body, None);

    // The complete prompt must not already occur in the typed command's echo.
    let body = "read --prompt-str (string join '' N8-RESOURCE- READ-CANCEL:) answer; set -l failed $status; \
        test $failed -ne 0; or return 97; return $failed";
    s.send(scope(f, "read-cancel", body).as_bytes());
    s.wait_text(b"N8-RESOURCE-READ-CANCEL:");
    let paths = Paths::record(f, "read-cancel");
    paths.live();
    s.control(0);
    recover(&mut s, f, original, &mut next);
    assert!(!f.root.join("resource-read-cancel.status").exists(), "cancelled read resumed its command tail");
    paths.removed(&changes);

    {
        let label = "child-cancel";
        let ready = Event::new("resource-child-cancel-ready");
        let release = Event::new("resource-child-cancel-release");
        let child = consumer_command(f, &python, &script, label, &ready, &release, "console", "argv-file");
        let body = format!("{}; {child}; set -l failed $status; return $failed", environment("argv-file"));
        s.send(scope(f, label, &body).as_bytes());
        ready.wait();
        let consumer = Consumer::acknowledged(&f.root.join(format!("resource-{label}.pid")));
        s.wait_text(b"N8-RESOURCE-CHILD-READ:child-cancel:");
        s.wait_protocol(false);
        let paths = Paths::record(f, label);
        paths.live();
        s.control(0);
        consumer.wait(130);
        recover(&mut s, f, original, &mut next);
        assert_eq!(fs::read(f.root.join(format!("resource-{label}.receipt"))).unwrap(), b"actual-console-CTRL_BREAK");
        assert!(!f.root.join(format!("resource-{label}.status")).exists(), "cancelled child resumed its command tail");
        paths.removed(&changes);
    }

    for kind in ["argv-file", "argv-directory-child"] {
        let label = format!("disowned-{kind}");
        let ready = Event::new(&format!("resource-{kind}-ready"));
        let release = Event::new(&format!("resource-{kind}-release"));
        let child = consumer_command(f, &python, &script, &label, &ready, &release, "hold", kind);
        let body = format!("{}; {child} >{} 2>{} &; set -l consumer $last_pid; \
            read --prompt-str N8-RESOURCE-TRANSFER-FENCE-{kind}: accepted; or return $status; \
            disown $consumer; or return $status; \
            __fish_native_fs transfer-temp-file $n8_owned_file; or return $status; return 0",
            environment(kind), qp(&f.root.join(format!("resource-{label}.stdout"))), qp(&f.root.join(format!("resource-{label}.stderr"))));
        s.send(scope(f, &label, &body).as_bytes());
        ready.wait();
        let consumer = Consumer::acknowledged(&f.root.join(format!("resource-{label}.pid")));
        s.wait_text(format!("N8-RESOURCE-TRANSFER-FENCE-{kind}:").as_bytes());
        let paths = Paths::record(f, &label);
        paths.live();
        // The real consumer has acknowledged both native paths and closed its
        // file handles. Only now may fish disown/transfer and return its owner.
        s.send(b"\r");
        assert_eq!(status(&mut s, f, &label), 0, "accepted disowned transfer must succeed");
        recover(&mut s, f, original, &mut next);
        consumer.live();
        paths.live();
        release.set();
        consumer.wait(0);
        assert_eq!(fs::read(f.root.join(format!("resource-{label}.receipt"))).unwrap(), b"argv-env-consumed-after-parent-return");
        // Drive the existing main-thread dispatch after actual process exit.
        // The consumer never removes either resource itself.
        recover(&mut s, f, original, &mut next);
        paths.removed(&changes);
        assert!(fs::read(f.root.join(format!("resource-{label}.stderr"))).unwrap().is_empty());
    }
    s.send(b"resource-cleanup-left-editor-usable");
    next += 1;
    capture(&mut s, f, next, "resource-cleanup-left-editor-usable");
    finish(s);
    assert_eq!(fs::read_dir(&temporary).unwrap().count(), 0, "no helper-owned temporary files/directories may leak");
}
