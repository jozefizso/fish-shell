//! Native psub consumers and real CPython/argcomplete acceptance families.
use super::fixture::{Fixture, Captured, exact, quote, run, LIMIT};
use std::ffi::{OsStr, c_void};
use std::fs;
use std::io::{Read, Write};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

pub const FAMILIES: &[super::Family] = &[
    ("psub-binary-literal-suffix", psub_binary),
    ("psub-consumer-lifetime", psub_lifetime),
    ("psub-launch-failure-cleanup", psub_failure),
    ("psub-fifo-and-invalid-suffix", psub_rejections),
    ("psub-owned-testing-cleanup", psub_testing),
    ("psub-disown-normal-shell-exit", psub_disown),
    ("psub-abrupt-shell-exit", psub_abrupt),
    ("psub-cancel-and-drain", psub_cancel),
    ("psub-private-acl-and-pinned-name", psub_security),
    ("psub-nested-disowned-native-consumer", psub_nested),
    ("argcomplete-real-fd89-literal-argv", completion_literals),
    ("argcomplete-real-filesystem", completion_files),
    ("argcomplete-public-function-consumer", completion_public),
    ("argcomplete-context-byte-offset-and-framing", completion_context),
    ("argcomplete-required-dependency-absent", completion_missing),
    ("argcomplete-real-unsupported-version", completion_version),
    ("argcomplete-uncooperative-prelaunch", completion_variant),
    ("argcomplete-malformed-framing", completion_malformed),
    ("argcomplete-oversize-and-owner-reset", completion_oversize),
    ("argcomplete-cancel-and-drain", completion_cancel),
    ("argcomplete-unrelated-and-grandchild-handles", completion_isolation),
    ("argcomplete-native-status-policy", completion_status),
    ("argcomplete-real-occupied-crt-startup", completion_occupied_crt),
    ("argcomplete-real-startup-stderr", completion_startup_stderr),
    ("argcomplete-startup-failure-native-status", completion_startup_status),
    ("resource-lease-malformed-and-partial", keeper_bad_frames),
    ("resource-lease-pending-parent-death", keeper_pending_death),
    ("resource-lease-accepted-commit-message-gap", keeper_accept_gap),
    ("resource-lease-partial-job-registration-parent-death", keeper_partial_job),
    ("resource-lease-rejects-enclosing-job", keeper_host_rejection),
    ("resource-lease-release-deadline-and-eof", keeper_release_deadline),
];

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(sa: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, status: u32) -> i32;
    fn GetProcessId(process: *mut c_void) -> u32;
    fn FindFirstChangeNotificationW(path: *const u16, recursive: i32, filter: u32) -> *mut c_void;
    fn FindNextChangeNotification(handle: *mut c_void) -> i32;
    fn FindCloseChangeNotification(handle: *mut c_void) -> i32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry) -> i32;
    fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, image: *mut u16, length: *mut u32) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSuspendProcess(process: *mut c_void) -> i32;
    fn NtResumeProcess(process: *mut c_void) -> i32;
    fn NtQueryInformationProcess(process: *mut c_void, class: u32, data: *mut c_void,
        size: u32, returned: *mut u32) -> i32;
}
// Fault injection only: freeze an image/creator-validated owned keeper, never
// implement public stop/continue or target a user-selected PID.
struct PausedKeeper<'a>(&'a Keeper);
impl<'a> PausedKeeper<'a> {
    fn new(keeper: &'a Keeper) -> Self {
        assert!(unsafe { NtSuspendProcess(keeper.0.as_raw_handle()) } >= 0);
        Self(keeper)
    }
}
impl Drop for PausedKeeper<'_> {
    fn drop(&mut self) { assert!(unsafe { NtResumeProcess(self.0.0.as_raw_handle()) } >= 0); }
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
}
#[repr(C)]
struct ProcessCommandLine { length: u16, maximum: u16, buffer: *const u16 }
fn is_resource_keeper(process: &OwnedHandle) -> bool {
    // A nested .fish provider is also an image/creator-matching fish child.
    // Identify the non-evaluator keeper by its complete two-argument command,
    // not by assuming every matching executable is a resource owner.
    let mut storage = vec![0usize; 8196];
    let mut returned = 0;
    assert!(unsafe { NtQueryInformationProcess(process.as_raw_handle(), 60,
        storage.as_mut_ptr().cast(), std::mem::size_of_val(storage.as_slice()) as u32,
        &mut returned) } >= 0, "query owned child command line");
    let start = storage.as_ptr() as usize;
    let end = start + returned as usize;
    assert!(returned as usize >= std::mem::size_of::<ProcessCommandLine>()
        && returned as usize <= std::mem::size_of_val(storage.as_slice()));
    let command = unsafe { &*storage.as_ptr().cast::<ProcessCommandLine>() };
    let pointer = command.buffer as usize;
    let length = command.length as usize;
    assert!(length % 2 == 0 && pointer % 2 == 0
        && pointer >= start + std::mem::size_of::<ProcessCommandLine>()
        && pointer <= end && length <= end - pointer);
    let text: Vec<_> = unsafe { std::slice::from_raw_parts(command.buffer, length / 2) }
        .iter().copied().chain(Some(0)).collect();
    let mut count = 0;
    let arguments = unsafe { CommandLineToArgvW(text.as_ptr(), &mut count) };
    assert!(!arguments.is_null(), "parse owned child command line");
    let keeper = if count == 2 {
        let argument = unsafe { *arguments.add(1) };
        let expected: Vec<_> = fish::native_adapters::RESOURCE_LEASE_OPTION.encode_utf16().chain(Some(0)).collect();
        expected.iter().enumerate().all(|(index, unit)| unsafe { *argument.add(index) } == *unit)
    } else { false };
    unsafe { LocalFree(arguments.cast()); }
    keeper
}

#[repr(C)]
struct ProcessEntry {
    size: u32, usage: u32, pid: u32, heap: usize, module: u32, threads: u32,
    parent: u32, priority: i32, flags: u32, executable: [u16; 260],
}
struct Keeper(OwnedHandle);
impl Keeper {
    fn wait(&self) {
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT.as_millis() as u32) }, 0,
            "owned keeper did not exit after releasing its last consumer");
    }
}
impl Drop for Keeper {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } == 258 {
            // Proven creator + fixture image identity, retained before the
            // controlled consumer is released. Never reopen a PID for cleanup.
            unsafe { TerminateProcess(self.0.as_raw_handle(), 98); }
            assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT.as_millis() as u32) }, 0);
        }
    }
}
fn retain_keeper(f: &Fixture, parent: u32) -> Keeper {
    retain_keeper_except(f, parent, 0)
}
fn retain_keeper_except(f: &Fixture, parent: u32, excluded: u32) -> Keeper {
    let snapshot = unsafe { CreateToolhelp32Snapshot(2, 0) };
    assert_ne!(snapshot as isize, -1);
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry: ProcessEntry = unsafe { std::mem::zeroed() };
    entry.size = std::mem::size_of::<ProcessEntry>() as u32;
    let expected = fs::canonicalize(&f.fish).unwrap();
    let mut found = None;
    let mut more = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    while more != 0 {
        let length = entry.executable.iter().position(|unit| *unit == 0).unwrap();
        if entry.parent == parent && entry.pid != excluded
            && String::from_utf16(&entry.executable[..length]).unwrap().eq_ignore_ascii_case("fish.exe") {
            let raw = unsafe { OpenProcess(0x0010_1801, 0, entry.pid) };
            assert!(!raw.is_null(), "retain live keeper while native consumer is blocked");
            let process = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut image = [0u16; 32768];
            let mut size = image.len() as u32;
            assert_ne!(unsafe { QueryFullProcessImageNameW(process.as_raw_handle(), 0, image.as_mut_ptr(), &mut size) }, 0);
            let image = std::ffi::OsString::from_wide(&image[..size as usize]);
            if fs::canonicalize(image).unwrap() == expected && is_resource_keeper(&process) {
                assert!(found.is_none(), "fixture unexpectedly has multiple keeper owners");
                found = Some(Keeper(process));
            }
        }
        more = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    found.expect("real resource keeper not found while its consumer is blocked")
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetNamedSecurityInfoW(path: *const u16, kind: u32, information: u32, owner: *mut *mut c_void,
        group: *mut *mut c_void, dacl: *mut *mut c_void, sacl: *mut *mut c_void, descriptor: *mut *mut c_void) -> u32;
    fn GetSecurityDescriptorControl(descriptor: *const c_void, control: *mut u16, revision: *mut u32) -> i32;
    fn GetAce(acl: *const c_void, index: u32, ace: *mut *mut c_void) -> i32;
    fn ConvertSidToStringSidW(sid: *const c_void, text: *mut *mut u16) -> i32;
}
fn wide(text: &OsStr) -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() }
struct Event { name: String, handle: OwnedHandle }
impl Event {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!("Local\\fish-n8-{}-{}-{label}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(OsStr::new(&name)).as_ptr()) };
        assert!(!raw.is_null());
        Self { name, handle: unsafe { OwnedHandle::from_raw_handle(raw) } }
    }
    fn wait(&self) { assert_eq!(unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT.as_millis() as u32) }, 0, "bounded {}", self.name); }
    fn signal(&self) { assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0); }
}
impl Drop for Event { fn drop(&mut self) { self.signal(); } }
struct Watch(*mut c_void);
impl Watch {
    fn new(path: &Path) -> Self {
        let raw = unsafe { FindFirstChangeNotificationW(wide(path.as_os_str()).as_ptr(), 1, 1 | 2) };
        assert_ne!(raw as isize, -1, "required native deletion notification");
        Self(raw)
    }
    fn deleted(&self, path: &Path) {
        let deadline = std::time::Instant::now() + LIMIT;
        while path.exists() {
            let remaining = deadline.checked_duration_since(std::time::Instant::now()).expect("owned file deletion exceeded bound");
            assert_eq!(unsafe { WaitForSingleObject(self.0, remaining.as_millis() as u32) }, 0, "owned file not deleted after real consumer completion: {}", path.display());
            assert_ne!(unsafe { FindNextChangeNotification(self.0) }, 0);
        }
    }
}
impl Drop for Watch { fn drop(&mut self) { unsafe { FindCloseChangeNotification(self.0); } } }
struct Shell { child: Child, out: Option<JoinHandle<Vec<u8>>>, err: Option<JoinHandle<Vec<u8>>> }
impl Shell {
    fn start(command: &mut Command) -> Self {
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let out = std::thread::spawn(move || { let mut data = Vec::new(); out.read_to_end(&mut data).unwrap(); data });
        let err = std::thread::spawn(move || { let mut data = Vec::new(); err.read_to_end(&mut data).unwrap(); data });
        Self { child, out: Some(out), err: Some(err) }
    }
    fn finish(mut self) -> Captured {
        assert_eq!(unsafe { WaitForSingleObject(self.child.as_raw_handle(), LIMIT.as_millis() as u32) }, 0, "bounded owned fish wait");
        let code = self.child.wait().unwrap().code().unwrap() as u32;
        Captured { code, out: self.out.take().unwrap().join().unwrap(), err: self.err.take().unwrap().join().unwrap() }
    }
    fn kill(&mut self) { self.child.kill().unwrap(); self.child.wait().unwrap(); }
    fn close_input(&mut self) { drop(self.child.stdin.take()); }
    fn send_input(&mut self, bytes: &[u8]) { self.child.stdin.as_mut().unwrap().write_all(bytes).unwrap(); }
}
impl Drop for Shell {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() { let _ = self.child.kill(); let _ = self.child.wait(); }
    }
}
fn prefix(f: &Fixture, label: &str) -> (String, PathBuf) {
    let temp = f.root.join(format!("adapter temp {label} 雪"));
    fs::create_dir(&temp).unwrap();
    let functions = Path::new(env!("CARGO_MANIFEST_DIR")).join("share/functions");
    (format!("set -lx TEMP {}; set -lx TMP {}; set -g fish_function_path {}; ",
        quote(temp.to_str().unwrap()), quote(temp.to_str().unwrap()), quote(functions.to_str().unwrap())), temp)
}
struct Consumer { ready: Event, release: Event, done: Event, report: PathBuf, marker: PathBuf, output: PathBuf, process: Option<OwnedHandle> }
impl Consumer {
    fn new(f: &Fixture, label: &str) -> Self {
        Self { ready: Event::new(&format!("{label}-ready")), release: Event::new(&format!("{label}-release")), done: Event::new(&format!("{label}-done")),
            report: f.root.join(format!("{label}.report")), marker: f.root.join(format!("{label}.done")), output: f.root.join(format!("{label}.binary")), process: None }
    }
    fn command(&self, f: &Fixture, suffix: &str, count: usize, status: u32) -> String {
        format!("{} n8-psub-consume ({} n8-psub-generate {count} | psub -s {}) {} {} {} {} {} {} {status} >NUL 2>NUL",
            f.probe(), f.probe(), quote(suffix), quote(&self.ready.name), quote(&self.release.name), quote(&self.done.name),
            quote(self.report.to_str().unwrap()), quote(self.marker.to_str().unwrap()), quote(self.output.to_str().unwrap()))
    }
    fn started(&mut self) -> PathBuf {
        self.ready.wait();
        let report = fs::read_to_string(&self.report).unwrap();
        let mut lines = report.lines();
        let path = PathBuf::from(lines.next().unwrap());
        let pid = lines.next().unwrap().parse().unwrap();
        let raw = unsafe { OpenProcess(0x0010_0001, 0, pid) };
        assert!(!raw.is_null());
        self.process = Some(unsafe { OwnedHandle::from_raw_handle(raw) });
        assert_eq!(unsafe { WaitForSingleObject(self.process.as_ref().unwrap().as_raw_handle(), 0) }, 258);
        assert!(path.is_file(), "psub path disappeared while actual native consumer was blocked");
        path
    }
    fn completed(&self) { self.done.wait(); assert_eq!(unsafe { WaitForSingleObject(self.process.as_ref().unwrap().as_raw_handle(), LIMIT.as_millis() as u32) }, 0); }
}
impl Drop for Consumer {
    fn drop(&mut self) {
        self.release.signal();
        if let Some(process) = &self.process {
            if unsafe { WaitForSingleObject(process.as_raw_handle(), LIMIT.as_millis() as u32) } == 258 {
                unsafe { TerminateProcess(process.as_raw_handle(), 98); }
                assert_eq!(unsafe { WaitForSingleObject(process.as_raw_handle(), LIMIT.as_millis() as u32) }, 0);
            }
        }
    }
}
fn payload(count: usize) -> Vec<u8> { (0..count).map(|index| (index % 256) as u8).collect() }
fn psub_case(f: &Fixture, label: &str, suffix: &str, bytes: usize, status: u32) {
    let (setup, temp) = prefix(f, label);
    let mut consumer = Consumer::new(f, label);
    let code = format!("{setup}{}", consumer.command(f, suffix, bytes, status));
    let shell = Shell::start(&mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]));
    let path = consumer.started();
    assert!(path.to_str().unwrap().ends_with(suffix));
    let watch = Watch::new(&temp);
    consumer.release.signal();
    consumer.completed();
    exact(shell.finish(), status, b"", b"");
    watch.deleted(&path);
    assert_eq!(fs::read(&consumer.output).unwrap(), payload(bytes));
    assert_eq!(fs::read_dir(temp).unwrap().count(), 0);
}
fn psub_binary(f: &Fixture) { psub_case(f, "binary", ".雪 🐟 \u{f600} & ; (literal)'", 8 * 1024 * 1024 + 513, 0); }
fn psub_lifetime(f: &Fixture) { psub_case(f, "lifetime", ".bin", 65537, 0); }
fn psub_failure(f: &Fixture) {
    psub_case(f, "consumer-error", ".bin", 131073, 23);
    let (setup, temp) = prefix(f, "launch-error");
    let result = f.eval(&format!("{setup}{} (printf data | psub); set -l failure $status; exit $failure", quote(f.root.join("missing-owned-consumer.exe").to_str().unwrap())));
    assert_ne!(result.code, 0);
    assert!(String::from_utf8_lossy(&result.err).contains("fish"));
    assert_eq!(fs::read_dir(temp).unwrap().count(), 0, "failed native launch leaked psub file");
}
fn psub_rejections(f: &Fixture) {
    for (label, flags, diagnostic) in [
        ("fifo", "-F", "FIFO mode is unsupported"),
        ("suffix-separator", "-s '../escape'", "literal Windows filename suffix"),
        ("suffix-ads", "-s ':stream'", "literal Windows filename suffix"),
        ("suffix-trailing", "-s '.bad.'", "literal Windows filename suffix"),
    ] {
        let (setup, temp) = prefix(f, label);
        let result = f.eval(&format!("{setup}set -l path (printf data | psub {flags}); exit $status"));
        assert_ne!(result.code, 0);
        assert!(String::from_utf8_lossy(&result.err).contains(diagnostic));
        assert_eq!(fs::read_dir(temp).unwrap().count(), 0);
    }
}
fn psub_testing(f: &Fixture) {
    let (setup, temp) = prefix(f, "testing");
    let output = f.root.join("testing.output");
    let code = format!("{setup}set -l path (printf 'persistent' | psub --testing -s .txt); test -f $path; or exit 91; {} n8-psub-generate 0; __fish_native_adapter psub-test-cleanup $path; or exit 92; test -e $path; and exit 93; printf cleaned", f.probe());
    exact(f.eval(&code), 0, b"cleaned", b"");
    assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
    fs::write(&output, b"user-owned-not-a-psub").unwrap();
    let result = f.eval(&format!("__fish_native_adapter psub-test-cleanup {}", quote(output.to_str().unwrap())));
    assert_ne!(result.code, 0);
    assert_eq!(fs::read(output).unwrap(), b"user-owned-not-a-psub");
}
fn shell_exit_case(f: &Fixture, label: &str, abrupt: bool, disowned: bool) {
    let (setup, temp) = prefix(f, label);
    let mut consumer = Consumer::new(f, label);
    let accepted = Event::new(&format!("{label}-accepted"));
    let command = consumer.command(f, ".live", 2 * 1024 * 1024 + 7, 0);
    let disown = if disowned { "disown $last_pid; " } else { "" };
    let code = if abrupt { format!("{setup}{command} &; {disown}{} n8-adapter-signal {}; read -l n8_owned_hold",
        f.probe(), quote(&accepted.name)) }
        else { format!("{setup}{command} &; {disown}exit 0") };
    let mut shell = Shell::start(&mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]));
    let path = consumer.started();
    let keeper = retain_keeper(f, shell.child.id());
    let watch = Watch::new(&temp);
    if abrupt { accepted.wait(); shell.kill(); }
    let result = shell.finish();
    if !abrupt {
        let expected: &[u8] = if disowned { b"" } else {
            b"fish: leaving 1 native job(s) running; use wait or disown to make ownership explicit\n"
        };
        exact(result, 0, b"", expected);
    }
    assert!(path.is_file(), "root cleanup revoked accepted native consumer resource");
    assert_eq!(unsafe { WaitForSingleObject(consumer.process.as_ref().unwrap().as_raw_handle(), 0) }, 258);
    consumer.release.signal();
    consumer.completed();
    watch.deleted(&path);
    keeper.wait();
    assert_eq!(fs::read(&consumer.output).unwrap(), payload(2 * 1024 * 1024 + 7));
}
fn psub_disown(f: &Fixture) {
    shell_exit_case(f, "disown", false, true);
    shell_exit_case(f, "accepted-normal", false, false);
}
fn psub_abrupt(f: &Fixture) {
    shell_exit_case(f, "abrupt", true, false);
    shell_exit_case(f, "disowned-abrupt", true, true);
}
fn probe_command(f: &Fixture, mode: &str, args: &[&OsStr]) -> Command {
    let mut command = Command::new(&f.probe);
    command.arg(mode).args(args).current_dir(&f.root).env_clear().env("PATH", "").env("LANG", "C");
    for key in ["SystemRoot", "WINDIR"] { if let Some(value) = std::env::var_os(key) { command.env(key, value); } }
    for (key, leaf) in [("USERPROFILE", "home"), ("HOME", "home"), ("APPDATA", "config"), ("LOCALAPPDATA", "data"), ("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")] {
        command.env(key, f.state.join(leaf));
    }
    command
}
fn psub_cancel(f: &Fixture) {
    let (setup, temp) = prefix(f, "cancel");
    let consumer = Consumer::new(f, "cancel");
    let code = format!("{setup}{}", consumer.command(f, ".cancel", 262145, 0));
    let result = run(&mut probe_command(f, "n8-psub-cancel", &[f.fish.as_os_str(), OsStr::new(&code), OsStr::new(&consumer.ready.name)]).creation_flags(0x200), b"");
    assert_eq!(result.code, 0, "actual cancellation probe: {}", String::from_utf8_lossy(&result.err));
    let report = fs::read_to_string(&consumer.report).unwrap();
    let path = PathBuf::from(report.lines().next().unwrap());
    Watch::new(&temp).deleted(&path);
    assert!(!consumer.marker.exists(), "canceled blocked consumer completed its file read");
}
#[repr(C)]
struct AclHeader { revision: u8, reserved: u8, size: u16, count: u16, reserved2: u16 }
fn assert_private_acl(path: &Path) {
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    assert_eq!(unsafe { GetNamedSecurityInfoW(wide(path.as_os_str()).as_ptr(), 1, 4, std::ptr::null_mut(), std::ptr::null_mut(), &mut dacl, std::ptr::null_mut(), &mut descriptor) }, 0);
    assert!(!dacl.is_null());
    let mut control = 0;
    let mut revision = 0;
    assert_ne!(unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) }, 0);
    assert_ne!(control & 0x1000, 0, "psub inherited a broader temporary-directory ACL");
    let count = unsafe { (*(dacl as *const AclHeader)).count };
    assert_eq!(count, 2);
    let mut identities = Vec::new();
    for index in 0..u32::from(count) {
        let mut ace = std::ptr::null_mut();
        assert_ne!(unsafe { GetAce(dacl, index, &mut ace) }, 0);
        assert_eq!(unsafe { *(ace as *const u8) }, 0, "only allow ACEs in the protected private descriptor");
        let mut sid = std::ptr::null_mut();
        assert_ne!(unsafe { ConvertSidToStringSidW((ace as *const u8).add(8).cast(), &mut sid) }, 0);
        let mut length = 0;
        while unsafe { *sid.add(length) } != 0 { length += 1; }
        identities.push(String::from_utf16(unsafe { std::slice::from_raw_parts(sid, length) }).unwrap());
        unsafe { LocalFree(sid.cast()); }
    }
    unsafe { LocalFree(descriptor); }
    identities.sort();
    assert_eq!(identities, ["S-1-3-4", "S-1-5-18"]);
}
fn psub_security(f: &Fixture) {
    let (setup, temp) = prefix(f, "security");
    let mut consumer = Consumer::new(f, "security");
    let code = format!("{setup}{}", consumer.command(f, ".private", 32769, 0));
    let shell = Shell::start(&mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]));
    let path = consumer.started();
    assert_private_acl(&path);
    assert!(fs::rename(&path, temp.join("replacement")).is_err(), "live substitution was not pinned against replacement");
    let watch = Watch::new(&temp);
    consumer.release.signal(); consumer.completed();
    exact(shell.finish(), 0, b"", b""); watch.deleted(&path);
}

const APPLICATION: &str = r#"# PYTHON_ARGCOMPLETE_OK
import argparse, argcomplete, os, sys, msvcrt, ctypes, subprocess
assert not os.get_inheritable(8) and not os.get_inheritable(9)
if 'FISH_N8_POINT' in os.environ:
    assert os.environ['COMP_POINT'] == os.environ['FISH_N8_POINT']
if 'FISH_N8_ARGV' in os.environ:
    import json
    assert sys.argv[1:] == json.loads(os.environ['FISH_N8_ARGV'])
if 'FISH_N8_UNRELATED_HANDLE' in os.environ:
    ownership_probe = r"""
import ctypes, msvcrt, os, subprocess, sys
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
# CompareObjectHandles is exported by KernelBase, not necessarily Kernel32.
kernelbase = ctypes.WinDLL('kernelbase', use_last_error=True)
handle = ctypes.c_void_p
kernel.GetCurrentProcess.restype = handle
kernel.GetHandleInformation.argtypes = [handle, ctypes.POINTER(ctypes.c_uint)]
kernel.OpenProcess.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.c_uint]
kernel.OpenProcess.restype = handle
kernel.DuplicateHandle.argtypes = [handle, handle, handle, ctypes.POINTER(handle), ctypes.c_uint, ctypes.c_int, ctypes.c_uint]
kernelbase.CompareObjectHandles.argtypes = [handle, handle]
kernel.CloseHandle.argtypes = [handle]

def check_isolation(owner_pid, sources, candidates, grandchild=False):
    owned = []
    try:
        # Freeze validity before allocating ANY observation handles. A missing
        # candidate's numeric slot can otherwise be reused by our own probe.
        present = []
        for candidate in dict.fromkeys(candidates):
            flags = ctypes.c_uint()
            if kernel.GetHandleInformation(candidate, ctypes.byref(flags)):
                present.append(candidate)
            else:
                assert ctypes.get_last_error() == 6, ctypes.WinError(ctypes.get_last_error())
        current = kernel.GetCurrentProcess()

        def duplicate(process, source):
            result = handle()
            assert kernel.DuplicateHandle(process, source, current, ctypes.byref(result), 0, False, 2), ctypes.WinError(ctypes.get_last_error())
            owned.append(result.value)
            return result.value

        # Borrow the already-live candidate slots: this single-threaded probe
        # never closes them, so observation allocations cannot reuse them.
        # GetHandleInformation-valid slots need not support DuplicateHandle
        # (ERROR_NOT_SUPPORTED); that says nothing about private-object identity.
        owner = kernel.OpenProcess(0x40, False, owner_pid)
        assert owner, ctypes.WinError(ctypes.get_last_error())
        owned.append(owner)
        baselines = [duplicate(owner, source) for source in sources]
        for baseline in baselines:
            assert kernelbase.CompareObjectHandles(baseline, baseline), 'private baseline is not comparable'
        for candidate in present:
            for baseline in baselines:
                assert not kernelbase.CompareObjectHandles(candidate, baseline), 'owned private capability reached descendant'
        if grandchild:
            # These are actual application fd8/fd9 objects, not assumed numeric
            # identities. Explicit observation duplicates are non-inheritable.
            sources = baselines + [msvcrt.get_osfhandle(8), msvcrt.get_osfhandle(9)]
            candidates = list(dict.fromkeys(candidates + sources))
            child = ownership_probe + '\ncheck_isolation(int(sys.argv[1]), list(map(int, sys.argv[2].split(","))), list(map(int, sys.argv[3].split(","))))\n'
            subprocess.run([sys.executable, '-I', '-c', child, str(os.getpid()),
                ','.join(map(str, sources)), ','.join(map(str, candidates))],
                close_fds=False, check=True)
    finally:
        for duplicate in reversed(owned):
            assert kernel.CloseHandle(duplicate), ctypes.WinError(ctypes.get_last_error())
"""
    exec(ownership_probe)
    unrelated = int(os.environ['FISH_N8_UNRELATED_HANDLE'])
    check_isolation(int(os.environ['FISH_N8_UNRELATED_PID']), [unrelated], [unrelated], grandchild=True)
if 'FISH_N8_READY' in os.environ:
    kernel = ctypes.WinDLL('kernel32')
    kernel.OpenEventW.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.c_wchar_p]
    kernel.OpenEventW.restype = ctypes.c_void_p
    kernel.SetEvent.argtypes = [ctypes.c_void_p]
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint]
    ready = kernel.OpenEventW(2, False, os.environ['FISH_N8_READY'])
    release = kernel.OpenEventW(0x100000, False, os.environ['FISH_N8_RELEASE'])
    assert ready and release and kernel.SetEvent(ready)
    assert kernel.WaitForSingleObject(release, 25000) == 0
os.write(1, b'ordinary-stdout-must-not-be-completions\n')
os.write(9, b'actual-fd9-diagnostic\n')
parser = argparse.ArgumentParser()
parser.add_argument('--choice', choices=['literal雪', 'literal;$(notrun)', 'two words', "quote'fish"])
parser.add_argument('file', nargs='?')
argcomplete.autocomplete(parser)
raise SystemExit('argcomplete did not complete')
"#;
fn python(key: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("required real native Python fixture {key} is not provisioned")));
    assert!(path.is_file(), "required native interpreter missing: {}", path.display());
    assert!(path.is_absolute());
    path
}
fn application(f: &Fixture, label: &str, source: &str) -> PathBuf {
    let path = f.root.join(format!("application {label} 雪.py"));
    fs::write(&path, source).unwrap(); path
}
fn completion_command(interpreter: &Path, app: &Path, line: &str, prefix: &str, args: &[&str]) -> String {
    let mut command = format!("__fish_native_adapter argcomplete {} {} -- {} {}", quote(line), quote(prefix), quote(interpreter.to_str().unwrap()), quote(app.to_str().unwrap()));
    for argument in args { command.push(' '); command.push_str(&quote(argument)); }
    command
}
fn completion_literals(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "literal", APPLICATION);
    let shadow_marker = f.root.join("shadow-argcomplete-ran");
    fs::write(f.root.join("argcomplete.py"), format!(
        "from pathlib import Path\nPath({:?}).write_text('untrusted dependency executed')\nraise RuntimeError('cwd dependency shadow')\n",
        shadow_marker.to_str().unwrap())).unwrap();
    let line = "application.py --choice literal";
    let arguments = ["", "two words", "雪🐟", "quote'\"\\", "%!^&|;$(marker)"];
    let json = "[\"\",\"two words\",\"雪🐟\",\"quote'\\\"\\\\\",\"%!^&|;$(marker)\"]";
    let code = format!("set -lx FISH_N8_POINT {}; set -lx FISH_N8_ARGV {}; {}", line.len(), quote(json), completion_command(&python, &app, line, line, &arguments));
    let result = f.eval(&code);
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty());
    let mut records: Vec<_> = String::from_utf8(result.out).unwrap().split('\n').map(str::to_owned).collect();
    records.sort();
    assert_eq!(records, ["literal;$(notrun)", "literal雪"]);
    assert!(!f.root.join("marker").exists());
    assert!(!shadow_marker.exists(), "adapter imported a cwd/application-directory dependency shadow");
}
fn completion_public(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "public", APPLICATION);
    let functions = Path::new(env!("CARGO_MANIFEST_DIR")).join("share/functions");
    let callback = format!("__fish_argcomplete_complete {} {}", quote(interpreter.to_str().unwrap()), quote(app.to_str().unwrap()));
    let code = format!("set -g fish_function_path {}; complete -c n8-actual-app -f -a {}; complete -C 'n8-actual-app --choice literal'",
        quote(functions.to_str().unwrap()), quote(&format!("({callback})")));
    let result = f.eval(&code);
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty());
    let text = String::from_utf8(result.out).unwrap();
    let mut values: Vec<_> = text.lines().map(|line| line.split('\t').next().unwrap()).collect();
    values.sort();
    assert_eq!(values, ["literal;$(notrun)", "literal雪"]);
}
fn completion_context(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "context", APPLICATION);
    let prefix = "application.py 雪🐟 --choice literal";
    let line = format!("{prefix} unused\n");
    let mut input = line.as_bytes().to_vec();
    input.extend_from_slice(b"\n\0");
    input.extend_from_slice(prefix.as_bytes());
    input.push(b'\n');
    let code = format!("set -lx FISH_N8_POINT {}; __fish_native_adapter argcomplete-current -- {} {}",
        prefix.len(), quote(interpreter.to_str().unwrap()), quote(app.to_str().unwrap()));
    let result = f.input(&code, &input);
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let mut values: Vec<_> = String::from_utf8(result.out).unwrap().split('\n').map(str::to_owned).collect();
    values.sort();
    assert_eq!(values, ["literal;$(notrun)", "literal雪"]);
    let result = f.input(&code, b"partial context");
    assert_ne!(result.code, 0);
    assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("partial or malformed completion context framing"));
}
fn completion_files(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "filesystem", APPLICATION);
    for name in ["n8literal-one.txt", "n8literal-雪.txt", "n8literal & ;.txt"] { fs::write(f.root.join(name), b"real filesystem completion").unwrap(); }
    let line = "application.py n8literal";
    let result = f.eval(&completion_command(&python, &app, line, line, &[]));
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let mut records: Vec<_> = String::from_utf8(result.out).unwrap().split('\n').map(str::to_owned).collect(); records.sort();
    assert_eq!(records, ["n8literal & ;.txt", "n8literal-one.txt", "n8literal-雪.txt"]);
}
fn completion_missing(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON_NO_ARGCOMPLETE");
    let marker = f.root.join("missing-dependency-app-ran");
    let app = application(f, "missing", &format!("# PYTHON_ARGCOMPLETE_OK\nfrom pathlib import Path\nPath({:?}).write_text('bad')\n", marker.to_str().unwrap()));
    let result = f.eval(&completion_command(&interpreter, &app, "app.py x", "app.py x", &[]));
    assert_ne!(result.code, 0); assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("installed argcomplete 3.x")); assert!(!marker.exists());
}
fn completion_version(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON_UNSUPPORTED");
    let marker = f.root.join("unsupported-version-app-ran");
    let app = application(f, "version", &format!("# PYTHON_ARGCOMPLETE_OK\nfrom pathlib import Path\nPath({:?}).write_text('bad')\n", marker.to_str().unwrap()));
    let result = f.eval(&completion_command(&interpreter, &app, "app.py x", "app.py x", &[]));
    assert_ne!(result.code, 0); assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("native CPython 3.10 through 3.14")); assert!(!marker.exists());
}
fn completion_variant(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON");
    let marker = f.root.join("uncooperative-app-ran");
    let app = application(f, "uncooperative", &format!("from pathlib import Path\nPath({:?}).write_text('bad')\n", marker.to_str().unwrap()));
    let result = f.eval(&completion_command(&interpreter, &app, "app.py x", "app.py x", &[]));
    assert_ne!(result.code, 0); assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("not opted into argcomplete")); assert!(!marker.exists());
    let result = f.eval("__fish_native_adapter socket /tmp/unix.sock");
    assert_ne!(result.code, 0); assert!(String::from_utf8_lossy(&result.err).contains("unsupported"));
    let result = f.eval(&completion_command(&f.probe, &app, "app.py", "not-a-prefix", &[]));
    assert_ne!(result.code, 0); assert!(String::from_utf8_lossy(&result.err).contains("cursor prefix"));
    let marker = f.root.join("generic-fd-child-ran");
    let result = f.eval(&format!("{} n8-adapter-touch {} 8>&1 9>&2", f.probe(), quote(marker.to_str().unwrap())));
    assert_ne!(result.code, 0);
    assert!(!marker.exists(), "generic fd 8/9 rejection executed a partially configured native child");
}
fn malformed_app(f: &Fixture, label: &str, expression: &str) -> PathBuf {
    application(f, label, &format!("# PYTHON_ARGCOMPLETE_OK\nimport os, argparse, argcomplete\np=argparse.ArgumentParser()\nos.write(8, {expression})\nos._exit(0)\n"))
}
fn direct(f: &Fixture, python: &Path, app: &Path, count: &str) -> Captured {
    run(&mut probe_command(f, "n8-argcomplete-direct", &[python.as_os_str(), app.as_os_str(), OsStr::new("app.py --choice literal"), OsStr::new("app.py --choice literal"), OsStr::new(count)]), b"")
}
fn completion_malformed(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    for (label, value, diagnostic) in [("nul", "b'a\\0b'", "malformed newline framing"), ("cr", "b'a\\rb'", "malformed newline framing"),
        ("utf8", "b'\\xff'", "not UTF-8"), ("record", "b'x' * 65537", "record over 64 KiB")] {
        let app = malformed_app(f, label, value);
        let result = direct(f, &python, &app, "8");
        assert_ne!(result.code, 0); assert!(result.out.is_empty());
        assert!(String::from_utf8_lossy(&result.err).contains(diagnostic), "{}", String::from_utf8_lossy(&result.err));
    }
    let app = application(f, "newline-choice",
        "# PYTHON_ARGCOMPLETE_OK\nimport argparse, argcomplete\np=argparse.ArgumentParser()\np.add_argument('--choice', choices=['bad\\nvalue'])\nargcomplete.autocomplete(p)\n");
    let result = f.eval(&completion_command(&python, &app, "app.py --choice bad", "app.py --choice bad", &[]));
    assert_ne!(result.code, 0);
    assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("unsupported newline/NUL framing"));
    // A final record without a trailing separator is the actual argcomplete
    // protocol, not a truncated length-prefixed record.
    let app = malformed_app(f, "final-record", "b'last-record'");
    exact(direct(f, &python, &app, "4"), 0, b"last-record", b"");
}
fn completion_oversize(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    let app = malformed_app(f, "oversize", "b'x' * (2 * 1024 * 1024)");
    let result = direct(f, &python, &app, "10");
    assert_ne!(result.code, 0); assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err).contains("exceeds 1 MiB"));
}
fn completion_cancel(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "cancel", APPLICATION);
    let ready = Event::new("completion-cancel-ready"); let release = Event::new("completion-cancel-release");
    let mut command = probe_command(f, "n8-argcomplete-cancel", &[python.as_os_str(), app.as_os_str(), OsStr::new("app.py --choice literal"), OsStr::new("app.py --choice literal"), OsStr::new(&ready.name)]);
    command.creation_flags(0x200).env("FISH_N8_READY", &ready.name).env("FISH_N8_RELEASE", &release.name);
    exact(run(&mut command, b""), 0, b"", b"");
    release.signal();
}
fn completion_isolation(f: &Fixture) {
    let python = python("FISH_NATIVE_TEST_PYTHON");
    let app = application(f, "isolation", APPLICATION);
    let result = direct(f, &python, &app, "8");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty());
    let mut records: Vec<_> = String::from_utf8(result.out).unwrap().split('\n').map(str::to_owned).collect(); records.sort();
    assert_eq!(records, ["literal;$(notrun)", "literal雪"]);
}
fn keeper_bad_frames(f: &Fixture) {
    for label in ["initial", "partial", "oversized", "unknown", "bad-job"] {
        let (_, temp) = prefix(f, label);
        let result = run(&mut probe_command(f, "n8-keeper-bad-frame", &[OsStr::new(label)])
            .env("TEMP", &temp).env("TMP", &temp), b"");
        assert_ne!(result.code, 0, "malformed keeper input reported success: {label}");
        assert_eq!(fs::read_dir(temp).unwrap().count(), 0, "malformed/partial keeper request leaked owned file");
    }
}

fn retain_process(pid: u32) -> OwnedHandle {
    let raw = unsafe { OpenProcess(0x0010_0001, 0, pid) };
    assert!(!raw.is_null(), "retain controlled owned fixture {pid}");
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn keeper_death_case(f: &Fixture, phase: &str) {
    let (_, temp) = prefix(f, phase);
    let ready = Event::new(&format!("{phase}-parent-ready"));
    let hold = Event::new(&format!("{phase}-parent-hold"));
    let report = f.root.join(format!("{phase}-parent.report"));
    let mut consumer = Consumer::new(f, phase);
    let mut command = probe_command(f, "n8-keeper-parent-death", &[
        OsStr::new(phase), OsStr::new(&ready.name), OsStr::new(&hold.name), report.as_os_str(),
        OsStr::new(&consumer.ready.name), OsStr::new(&consumer.release.name), OsStr::new(&consumer.done.name),
        consumer.report.as_os_str(), consumer.marker.as_os_str(), consumer.output.as_os_str(),
    ]);
    command.env("TEMP", &temp).env("TMP", &temp);
    let mut parent = Shell::start(&mut command);
    ready.wait();
    let report = fs::read_to_string(&report).unwrap();
    let records: Vec<_> = report.lines().collect();
    assert_eq!(records.len(), 4);
    let path = PathBuf::from(records[0]);
    assert!(path.is_file());
    let root = retain_process(records[1].parse().unwrap());
    let keeper = retain_process(records[2].parse().unwrap());
    let second: u32 = records[3].parse().unwrap();
    let second = (second != 0).then(|| retain_process(second));
    let watch = Watch::new(&temp);
    parent.kill();
    let accepted = phase == "accept-gap" || phase == "partial-next";
    if accepted {
        assert_eq!(consumer.started(), path);
        assert_eq!(unsafe { WaitForSingleObject(keeper.as_raw_handle(), 0) }, 258,
            "keeper abandoned an accepted consumer after parent death");
        if let Some(second) = second.as_ref() {
            assert_eq!(unsafe { WaitForSingleObject(second.as_raw_handle(), LIMIT.as_millis() as u32) }, 0,
                "partial Job packet left an unaccepted suspended root alive behind an accepted consumer");
            assert!(path.is_file());
            let mut marker = consumer.marker.as_os_str().to_owned(); marker.push(".pending");
            assert!(!Path::new(&marker).exists());
        }
        consumer.release.signal(); consumer.completed();
        assert_eq!(fs::read(&consumer.output).unwrap(), payload(65537));
    } else {
        assert_eq!(unsafe { WaitForSingleObject(root.as_raw_handle(), LIMIT.as_millis() as u32) }, 0,
            "preaccept parent death stranded a suspended owned root");
        if phase != "resumed-pending" {
            assert!(!consumer.report.exists(), "suspended unaccepted consumer was resumed");
        } else {
            assert!(consumer.report.exists(), "unaccepted resumed consumer did not reach its read barrier");
        }
        assert!(!consumer.marker.exists());
    }
    watch.deleted(&path);
    assert_eq!(unsafe { WaitForSingleObject(keeper.as_raw_handle(), LIMIT.as_millis() as u32) }, 0);
    // The parent process has already been killed and waited above. Drain its
    // capture pipes only after the keeper/consumer lifetime assertions: EOF is
    // not evidence that the parent is alive or that its descendants exited.
    parent.finish();
    assert_eq!(fs::read_dir(temp).unwrap().count(), 0);
    hold.signal();
}
fn keeper_pending_death(f: &Fixture) {
    keeper_death_case(f, "before-ack");
    keeper_death_case(f, "after-ack");
    keeper_death_case(f, "resumed-pending");
}
fn keeper_accept_gap(f: &Fixture) { keeper_death_case(f, "accept-gap"); }
fn keeper_partial_job(f: &Fixture) { keeper_death_case(f, "partial-next"); }
fn keeper_host_rejection(f: &Fixture) {
    for (label, mode, expected) in [
        ("host-rejection", "n8-keeper-reject-host", b"parent-and-pending-root-survived-rejection".as_slice()),
        ("identity-rejection", "n8-keeper-reject-identity", b"stale-proof-rejected-without-resume-or-termination".as_slice()),
    ] {
        let (_, temp) = prefix(f, label);
        let marker = f.root.join(format!("{label}-survived"));
        exact(run(&mut probe_command(f, mode, &[marker.as_os_str()])
            .env("TEMP", &temp).env("TMP", &temp), b""), 0, b"", b"");
        assert_eq!(fs::read(marker).unwrap(), expected);
        assert_eq!(fs::read_dir(temp).unwrap().count(), 0);
    }
}
fn psub_nested(f: &Fixture) {
    psub_nested_case(f, "nested-argv", false, None);
    psub_nested_case(f, "nested-environment", true, None);
    for provider in ["cmd", "ps1", "fish"] {
        for environment in [false, true] {
            psub_nested_case(f, &format!("nested-{provider}-{}", if environment { "env" } else { "argv" }),
                environment, Some(provider));
        }
    }
}
fn psub_nested_case(f: &Fixture, label: &str, environment: bool, provider: Option<&str>) {
    let (setup, temp) = prefix(f, label);
    let mut consumer = Consumer::new(f, label);
    let continued = Event::new(&format!("{label}-caller-returned"));
    let invocation = if environment { "set -lx FISH_N8_INPUT $argv[1]; " } else { "" };
    let mode = if environment { "n8-psub-consume-env" } else { "n8-psub-consume $argv[1]" };
    let command = if let Some(provider) = provider {
        let script = f.root.join(format!("{label}.{provider}"));
        let events = [&consumer.ready.name, &consumer.release.name, &consumer.done.name];
        let paths = [&consumer.report, &consumer.marker, &consumer.output];
        let source = match provider {
            "cmd" => {
                let alias = format!("{label}-probe.exe");
                fs::copy(&f.probe, f.root.join(&alias)).unwrap();
                let mut words = vec![format!("\"%~dp0{alias}\""),
                    if environment { "n8-psub-consume-env".to_owned() }
                    else { "n8-psub-consume \"%~1\"".to_owned() }];
                words.extend(events.iter().map(|event| format!("\"{event}\"")));
                words.extend(paths.iter().map(|path| format!("\"{}\"", path.file_name().unwrap().to_str().unwrap())));
                format!("@echo off\r\n{} 0\r\nexit /b %errorlevel%\r\n", words.join(" "))
            }
            "ps1" => {
                let ps = |value: &str| format!("'{}'", value.replace('\'', "''"));
                let mut words = vec![format!("& {}", ps(f.probe.to_str().unwrap())),
                    if environment { "n8-psub-consume-env".to_owned() }
                    else { "n8-psub-consume $args[0]".to_owned() }];
                words.extend(events.iter().map(|event| ps(event)));
                words.extend(paths.iter().map(|path| ps(path.to_str().unwrap())));
                format!("[IO.File]::WriteAllText($PSCommandPath+'.pathext',$env:PATHEXT)\nif(($env:PATHEXT -split ';') -notcontains '.EXE'){{throw 'nested native consumer fixture requires declared .EXE PATHEXT capability'}}\n{} 0\nexit $LASTEXITCODE\n", words.join(" "))
            }
            "fish" => {
                let mut words = vec![f.probe(), mode.to_owned()];
                words.extend(events.iter().map(|event| quote(event)));
                words.extend(paths.iter().map(|path| quote(path.to_str().unwrap())));
                format!("{} 0\n", words.join(" "))
            }
            _ => unreachable!(),
        };
        super::providers::write_provider_source(&script, &source);
        format!("{} {} >NUL 2>NUL", quote(script.to_str().unwrap()),
            if environment { "" } else { "$argv[1]" })
    } else {
        format!("{} {mode} {} {} {} {} {} {} 0 >NUL 2>NUL",
            f.probe(), quote(&consumer.ready.name), quote(&consumer.release.name), quote(&consumer.done.name),
            quote(consumer.report.to_str().unwrap()), quote(consumer.marker.to_str().unwrap()), quote(consumer.output.to_str().unwrap()))
    };
    let code = format!("{setup}function n8_actual_consumer; {invocation}{command} &; disown $last_pid; end; n8_actual_consumer ({} n8-psub-generate 65537 | psub); {} n8-adapter-signal {}; read -l n8_owned_hold; exit 0",
        f.probe(), f.probe(), quote(&continued.name));
    let mut native_command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    if provider == Some("ps1") { native_command.env("PATHEXT", ".EXE"); }
    let mut shell = Shell::start(&mut native_command);
    let path = consumer.started();
    if provider == Some("ps1") {
        let value = fs::read_to_string(f.root.join(format!("{label}.ps1.pathext"))).unwrap();
        assert!(value.split(';').any(|extension| extension.eq_ignore_ascii_case(".EXE")),
            "PowerShell must retain the declared native consumer capability: {value}");
    }
    let keeper = retain_keeper(f, shell.child.id());
    continued.wait();
    assert!(path.is_file(), "caller callback deleted a file still leased by its disowned nested native consumer");
    let watch = Watch::new(&temp);
    consumer.release.signal(); consumer.completed(); watch.deleted(&path);
    keeper.wait();
    shell.close_input();
    exact(shell.finish(), 0, b"", b"");
    assert_eq!(fs::read(&consumer.output).unwrap(), payload(65537));
}

fn completion_status(f: &Fixture) {
    let interpreter = python("FISH_NATIVE_TEST_PYTHON");
    for (label, statement, status, raw) in [
        ("ordinary-status", "os._exit(27)", 27, "0x0000001B"),
        ("high-status", "ctypes.windll.kernel32.ExitProcess(ctypes.c_uint32(0xCAFE1234))", 1, "0xCAFE1234"),
    ] {
        let app = application(f, label, &format!(
            "# PYTHON_ARGCOMPLETE_OK\nimport os, ctypes, argparse, argcomplete\nparser=argparse.ArgumentParser()\n{statement}\n"));
        let result = f.eval(&completion_command(&interpreter, &app, "app.py --choice literal", "app.py --choice literal", &[]));
        assert_eq!(result.code, status, "{}", String::from_utf8_lossy(&result.err));
        assert!(result.out.is_empty());
        assert!(String::from_utf8_lossy(&result.err).contains(raw));
    }
}

fn keeper_release_deadline(f: &Fixture) {
    let (setup, temp) = prefix(f, "release-deadline");
    let mut first = Consumer::new(f, "release-deadline-a");
    let mut second = Consumer::new(f, "release-deadline-b");
    let only_first = Event::new("release-only-first-ready");
    let both = Event::new("release-both-ready");
    let returned = Event::new("release-root-returned");
    let first_command = first.command(f, ".deadline-a", 65537, 0);
    let second_command = second.command(f, ".deadline-b", 65537, 0);
    let code = format!(
        "{setup}{first_command} &; set -l a $last_pid; {} n8-adapter-signal {}; read -l spawn_second; \
         {second_command} &; set -l b $last_pid; {} n8-adapter-signal {}; read -l finish_first; \
         wait $a; {} n8-adapter-signal {}; disown $b; exit 0",
        f.probe(), quote(&only_first.name), f.probe(), quote(&both.name), f.probe(), quote(&returned.name));
    let mut shell = Shell::start(&mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]));
    let first_path = first.started();
    only_first.wait();
    let first_keeper = retain_keeper(f, shell.child.id());
    let first_keeper_pid = unsafe { GetProcessId(first_keeper.0.as_raw_handle()) };
    assert_ne!(first_keeper_pid, 0);
    let paused = PausedKeeper::new(&first_keeper);
    shell.send_input(b"next\n");
    let second_path = second.started();
    both.wait();
    let second_keeper = retain_keeper_except(f, shell.child.id(), first_keeper_pid);
    let watch = Watch::new(&temp);
    first.release.signal();
    first.completed();
    shell.send_input(b"finish\n");
    // A's R is sent but its real keeper cannot acknowledge it. The private
    // cleanup deadline must return the root without canceling B or its lease.
    returned.wait();
    let result = shell.finish();
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.out.is_empty());
    assert!(String::from_utf8_lossy(&result.err)
        .contains("psub release acknowledgement deadline exceeded after release command was sent"));
    assert!(first_path.is_file());
    assert!(second_path.is_file());
    assert_eq!(unsafe { WaitForSingleObject(second.process.as_ref().unwrap().as_raw_handle(), 0) }, 258);
    // After root EOF, A performs real handle-relative cleanup and then exits
    // on the broken ACK pipe. B's accepted consumer remains independently live.
    drop(paused);
    watch.deleted(&first_path);
    first_keeper.wait();
    assert!(second_path.is_file());
    assert_eq!(fs::read(&first.output).unwrap(), payload(65537));
    second.release.signal(); second.completed();
    watch.deleted(&second_path); second_keeper.wait();
    assert_eq!(fs::read(&second.output).unwrap(), payload(65537));
    assert_eq!(fs::read_dir(temp).unwrap().count(), 0);
}

fn python_command(f: &Fixture, interpreter: &Path, arguments: &[&OsStr]) -> Command {
    let mut command = Command::new(interpreter);
    command.args(arguments).current_dir(&f.root).env_clear()
        .env("PATH", "").env("LANG", "C").env("TEMP", &f.root).env("TMP", &f.root);
    for key in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
    command
}

// These are actual private CPython installations, not a replacement interpreter
// or argcomplete module. The .pth selects the real provisioned dependency; its
// ASCII-only executable line also works with 3.10's locale-decoded .pth files.
fn startup_python(f: &Fixture, label: &str) -> PathBuf {
    let base = python("FISH_NATIVE_TEST_PYTHON");
    let discover = "import argcomplete,os; print(os.path.dirname(os.path.dirname(argcomplete.__file__)))";
    let result = run(&mut python_command(f, &base,
        &[OsStr::new("-I"), OsStr::new("-X"), OsStr::new("utf8"), OsStr::new("-c"), OsStr::new(discover)]), b"");
    assert_eq!(result.code, 0, "required real argcomplete dependency: {}", String::from_utf8_lossy(&result.err));
    let installed = String::from_utf8(result.out).unwrap();
    let installed = installed.trim_end_matches(['\r', '\n']);
    assert!(Path::new(installed).is_absolute() && Path::new(installed).is_dir());
    let root = f.root.join(format!("real CPython startup {label} 雪"));
    let result = run(&mut python_command(f, &base, &[
        OsStr::new("-I"), OsStr::new("-m"), OsStr::new("venv"),
        OsStr::new("--without-pip"), root.as_os_str(),
    ]), b"");
    assert_eq!(result.code, 0, "required native venv creation: {}", String::from_utf8_lossy(&result.err));
    let site = root.join("Lib/site-packages");
    assert!(site.is_dir(), "native CPython did not create its real site-packages directory");
    let hexadecimal: String = installed.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect();
    fs::write(site.join("fish_n8_real_argcomplete.pth"),
        format!("import sys; sys.path.append(bytes.fromhex('{hexadecimal}').decode('utf-8'))\n")).unwrap();
    fs::write(site.join("sitecustomize.py"), STARTUP_CUSTOMIZE).unwrap();
    let interpreter = root.join("Scripts/python.exe");
    assert!(interpreter.is_file());
    interpreter
}

const STARTUP_CUSTOMIZE: &str = r#"import os
_root = os.environ['FISH_N8_STARTUP_ROOT']
_held = []
_slots = int(os.environ.get('FISH_N8_CRT_SLOTS', '0'))
for _slot in range(3, 3 + _slots):
    _fd = os.open(os.path.join(_root, 'startup-slot-' + str(_slot) + '.bin'),
                  os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_BINARY, 0o600)
    if _fd != _slot:
        os.close(_fd)
        raise RuntimeError('required real CRT fixture slot is already occupied: ' + str(_slot))
    os.set_inheritable(_slot, False)
    _held.append(_slot)
with open(os.path.join(_root, 'startup.log'), 'a', encoding='utf-8') as _log:
    _log.write(str(os.getpid()) + ':' + str(_slots) + '\n')
if os.environ.get('FISH_N8_STARTUP_STDERR') == '1':
    os.write(2, b'installed-site-startup-diagnostic\n')
if '_ARGCOMPLETE' in os.environ and 'FISH_N8_STARTUP_EXIT' in os.environ:
    if os.environ.get('FISH_N8_BAD_STARTUP_ACK') == '1':
        import ctypes
        _kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        _kernel.WriteFile.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint,
                                     ctypes.POINTER(ctypes.c_uint), ctypes.c_void_p]
        _payload = ctypes.create_string_buffer(b'INVALID_INSTALLED_STARTUP_ACK\n')
        _count = ctypes.c_uint()
        _handle = int(os.environ['FISH_INTERNAL_ARGCOMPLETE_FDS'].split(';')[2])
        assert _kernel.WriteFile(_handle, _payload, len(_payload.value), ctypes.byref(_count), None)
    _status = int(os.environ['FISH_N8_STARTUP_EXIT'])
    if _status > 255:
        import ctypes
        ctypes.windll.kernel32.ExitProcess(ctypes.c_uint32(_status))
    os._exit(_status)
"#;

const STARTUP_APPLICATION: &str = r#"# PYTHON_ARGCOMPLETE_OK
import argparse, argcomplete, os
_root = os.environ['FISH_N8_STARTUP_ROOT']
for _slot in range(3, min(8, 3 + int(os.environ.get('FISH_N8_CRT_SLOTS', '0')))):
    assert os.path.samestat(os.fstat(_slot), os.stat(os.path.join(_root, 'startup-slot-' + str(_slot) + '.bin')))
assert not os.get_inheritable(8) and not os.get_inheritable(9)
with open(os.path.join(_root, 'application-ran'), 'wb') as _marker:
    _marker.write(b'real argparse application')
os.write(1, b'ordinary application stdout\n')
os.write(2, b'ordinary application stderr\n')
os.write(9, b'cooperating application fd9 diagnostic\n')
parser = argparse.ArgumentParser()
parser.add_argument('--choice', choices=['literal雪', 'literal;$(notrun)'])
argcomplete.autocomplete(parser)
raise SystemExit('real argcomplete did not complete')
"#;

fn startup_result(f: &Fixture, interpreter: &Path, app: &Path, root: &Path,
    slots: usize, stderr: bool, exit: Option<u32>, bad_ack: bool) -> Captured {
    let line = "app.py --choice literal";
    let code = completion_command(interpreter, app, line, line, &[]);
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    command.env("FISH_N8_STARTUP_ROOT", root).env("FISH_N8_CRT_SLOTS", slots.to_string())
        .env("FISH_N8_STARTUP_STDERR", if stderr { "1" } else { "0" })
        .env("FISH_N8_BAD_STARTUP_ACK", if bad_ack { "1" } else { "0" });
    if let Some(exit) = exit { command.env("FISH_N8_STARTUP_EXIT", exit.to_string()); }
    run(&mut command, b"")
}

fn assert_startup_completion(result: Captured, root: &Path, slots: usize) {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty());
    let mut values: Vec<_> = String::from_utf8(result.out).unwrap().split('\n').map(str::to_owned).collect();
    values.sort();
    assert_eq!(values, ["literal;$(notrun)", "literal雪"]);
    assert_eq!(fs::read(root.join("application-ran")).unwrap(), b"real argparse application");
    let log = fs::read_to_string(root.join("startup.log")).unwrap();
    let records: Vec<_> = log.lines().collect();
    assert_eq!(records.len(), 2, "real installed sitecustomize did not run in preflight and application");
    assert!(records.iter().all(|record| record.split_once(':').unwrap().1 == slots.to_string()));
}

fn completion_occupied_crt(f: &Fixture) {
    let interpreter = startup_python(f, "occupied");
    let app = application(f, "occupied-crt", STARTUP_APPLICATION);
    // Two held slots give opened=[5,6], and naive saved=[7,8]. Five held
    // slots exercise original endpoints at 8/9; seven also occupy both targets.
    for slots in [2, 5, 7] {
        let root = f.root.join(format!("occupied CRT {slots}"));
        fs::create_dir(&root).unwrap();
        assert_startup_completion(startup_result(f, &interpreter, &app, &root, slots, false, None, false), &root, slots);
    }
}

fn completion_startup_stderr(f: &Fixture) {
    let interpreter = startup_python(f, "stderr");
    let app = application(f, "startup-stderr", STARTUP_APPLICATION);
    let root = f.root.join("startup stderr ownership");
    fs::create_dir(&root).unwrap();
    assert_startup_completion(startup_result(f, &interpreter, &app, &root, 0, true, None, false), &root, 0);
}

fn completion_startup_status(f: &Fixture) {
    let interpreter = startup_python(f, "status");
    let app = application(f, "startup-status", STARTUP_APPLICATION);
    for (label, raw, mapped, bad_ack) in [
        ("missing-ack-27", 27u32, 27u32, false),
        ("invalid-ack-37", 37, 37, true),
        ("missing-ack-high", 0xCAFE1234, 1, false),
    ] {
        let root = f.root.join(label);
        fs::create_dir(&root).unwrap();
        let result = startup_result(f, &interpreter, &app, &root, 0, true, Some(raw), bad_ack);
        assert_eq!(result.code, mapped, "{}", String::from_utf8_lossy(&result.err));
        assert!(result.out.is_empty());
        let diagnostic = String::from_utf8(result.err).unwrap();
        assert!(diagnostic.contains(&format!("0x{raw:08X}")));
        assert!(diagnostic.contains("missing or invalid CRT fd 8/9 negotiation"));
        assert!(diagnostic.contains("installed-site-startup-diagnostic"));
        assert!(!root.join("application-ran").exists(), "startup failure ran the actual application");
        let log = fs::read_to_string(root.join("startup.log")).unwrap();
        assert_eq!(log.lines().count(), 2);
    }
}
