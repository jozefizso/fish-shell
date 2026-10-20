//! Real native quota and predicate consumers, registered by the N8 parent gate.
use super::fixture::{Fixture, exact, quote};
use std::ffi::{OsStr, c_void};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use fish::wutil::{AccessFlags, waccess};
use fish_widestring::str2wcstring;

pub const FAMILIES: &[super::Family] = &[
    ("native-quota-configured-policy-inventory-atomic-reset", configured_policy),
    ("native-quota-real-committed-process-memory", process_memory),
    ("native-quota-real-aggregate-job-memory-barrier", job_memory),
    ("native-quota-active-process-suspended-rollback", active_processes),
    ("native-quota-user-cpu-raw-status-not-cancellation", process_cpu),
    ("native-quota-future-groups-ancestor-query-scope", quota_scope),
    ("native-access-denied-dacl-readonly-sharing-missing", access_boundaries),
    ("native-reparse-identity-and-device-predicate-boundaries", reparses_devices),
];
fn output(result: super::fixture::Captured) -> String {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn record(text: &str, key: &str) -> u64 {
    let prefix = format!("{key}=");
    let values: Vec<_> = text.lines().filter_map(|line| line.strip_prefix(&prefix)).collect();
    assert_eq!(values.len(), 1, "expected one {key}: {text}");
    values[0].parse().unwrap()
}
fn all_unconfigured() -> &'static [u8] {
    b"--native-process-memory-kib not-configured\n--native-job-memory-kib not-configured\n--native-process-cpu-seconds not-configured\n--native-active-processes not-configured\n"
}
pub fn configured_policy(f: &Fixture) {
    exact(f.eval("ulimit --all"), 0, all_unconfigured(), b"");
    exact(f.eval("ulimit -a"), 0, all_unconfigured(), b"");
    exact(f.eval(concat!(
        "ulimit --native-process-memory-kib 65536; ",
        "ulimit --native-job-memory-kib 131072; ",
        "ulimit --native-process-cpu-seconds 2; ",
        "ulimit --native-active-processes 3; ulimit --all; ",
        "ulimit --native-process-memory-kib not-configured; ",
        "ulimit --native-job-memory-kib not-configured; ",
        "ulimit --native-process-cpu-seconds not-configured; ",
        "ulimit --native-active-processes not-configured; ulimit --all"
    )), 0, concat!(
        "--native-process-memory-kib 65536\n--native-job-memory-kib 131072\n",
        "--native-process-cpu-seconds 2\n--native-active-processes 3\n",
        "--native-process-memory-kib not-configured\n--native-job-memory-kib not-configured\n",
        "--native-process-cpu-seconds not-configured\n--native-active-processes not-configured\n"
    ).as_bytes(), b"");
    let mut code = String::from("ulimit --native-process-memory-kib 65536;\n");
    let mut rejected = 0;
    // Every pre-existing Unix category, its long alias, query and setter form.
    for &(short, long) in fish::builtins::ulimit::POSIX_OPTIONS {
        for option in [short, long] {
            for value in ["", " 123"] {
                code.push_str(&format!("ulimit {option}{value}; printf 'reject=%s\\n' $status;\n"));
                rejected += 1;
            }
        }
    }
    for form in ["", "123", "unlimited", "soft", "hard", "--native-process-memory-kib 0",
        "--native-process-memory-kib -1", "--native-process-memory-kib unlimited",
        "--native-process-memory-kib soft", "--native-process-memory-kib hard",
        "--native-process-memory-kib 18446744073709551616",
        "--native-process-memory-kib ''", "--native-process-memory-kib +1",
        "--native-process-memory-kib 1.5", "--all 123",
        "--native-process-memory-kib 65536 --native-active-processes 1"] {
        code.push_str(&format!("ulimit {form}; printf 'reject=%s\\n' $status;\n"));
        rejected += 1;
    }
    // These parse as u64 but cannot be represented by the actual kernel layout.
    for option in ["--native-process-memory-kib", "--native-job-memory-kib",
        "--native-process-cpu-seconds", "--native-active-processes"] {
        code.push_str(&format!("ulimit {option} 18446744073709551615; printf 'overflow=%s\\n' $status;\n"));
    }
    code.push_str("ulimit --all; ulimit --native-process-memory-kib; ulimit --native-process-memory-kib not-configured; ulimit --native-process-memory-kib");
    let result = f.eval(&code);
    assert_eq!(result.code, 0);
    let text = String::from_utf8(result.out).unwrap();
    assert_eq!(text.lines().filter(|line| *line == "reject=2").count(), rejected, "{text}");
    assert_eq!(text.lines().filter(|line| *line == "overflow=1").count(), 4, "{text}");
    assert!(text.ends_with(concat!(
        "--native-process-memory-kib 65536\n--native-job-memory-kib not-configured\n",
        "--native-process-cpu-seconds not-configured\n--native-active-processes not-configured\n",
        "65536\nnot-configured\n"
    )), "failed setter mutated policy: {text}");
    let diagnostics = String::from_utf8(result.err).unwrap();
    assert!(diagnostics.contains("unsupported"), "{diagnostics}");
    assert!(diagnostics.contains("before changing configured policy"), "{diagnostics}");
}
pub fn process_memory(f: &Fixture) {
    let baseline = output(f.eval(&format!("{} n8-memory 192", f.probe())));
    assert_eq!(record(&baseline, "committed"), 192 * 1024 * 1024);
    assert!(baseline.contains("cap-reached=true"));
    let limited = output(f.eval(&format!("ulimit --native-process-memory-kib 65536; {} n8-memory 192", f.probe())));
    assert!(record(&limited, "denied") > 0);
    assert!(record(&limited, "committed") > 0);
    assert!(record(&limited, "committed") < 64 * 1024 * 1024, "{limited}");
    assert!(!limited.contains("cap-reached"));
    let reset = output(f.eval(&format!("ulimit --native-process-memory-kib 65536; ulimit --native-process-memory-kib not-configured; {} n8-memory 192", f.probe())));
    assert_eq!(record(&reset, "committed"), 192 * 1024 * 1024);
    assert!(reset.contains("cap-reached=true"));
}
pub fn job_memory(f: &Fixture) {
    // Prove the same children can exceed the advertised aggregate budget
    // without this parser's quota, rather than mistaking an ancestor/system
    // allocation failure for enforcement by the configured Job.
    let baseline_ready = Event::new("aggregate-baseline-ready");
    let baseline_release = Event::new("aggregate-baseline-release");
    let baseline_held = f.root.join("aggregate-baseline-committed.txt");
    let baseline = output(f.eval(&format!(
        "{} n8-job-hold {} {} {} | {} n8-job-fill {} {}; printf 'stages=%s,%s\\n' $pipestatus",
        f.probe(), quote(&baseline_ready.name), quote(&baseline_release.name), qpath(&baseline_held),
        f.probe(), quote(&baseline_ready.name), quote(&baseline_release.name))));
    assert_eq!(fs::read_to_string(&baseline_held).unwrap(), (48 * 1024 * 1024).to_string());
    assert_eq!(record(&baseline, "committed"), 192 * 1024 * 1024, "{baseline}");
    assert!(baseline.contains("cap-reached=true"), "{baseline}");
    assert!(baseline.contains("stages=0,0"), "{baseline}");
    let ready = Event::new("aggregate-ready");
    let release = Event::new("aggregate-release");
    let held = f.root.join("aggregate-committed.txt");
    let text = output(f.eval(&format!(
        "ulimit --native-job-memory-kib 131072; {} n8-job-hold {} {} {} | {} n8-job-fill {} {}; printf 'stages=%s,%s\\n' $pipestatus",
        f.probe(), quote(&ready.name), quote(&release.name), qpath(&held),
        f.probe(), quote(&ready.name), quote(&release.name))));
    assert_eq!(fs::read_to_string(&held).unwrap(), (48 * 1024 * 1024).to_string());
    assert!(record(&text, "denied") > 0);
    let committed = record(&text, "committed");
    assert!(committed > 0 && committed < 80 * 1024 * 1024, "aggregate limit did not charge holding child: {text}");
    assert!(text.contains("stages=0,0"), "{text}");
}
pub fn active_processes(f: &Fixture) {
    let release = Event::new("active-hold");
    let first = f.root.join("active-first-entered");
    let rejected = f.root.join("active-rejected-entered");
    let after = f.root.join("active-after-reset");
    let result = f.eval(&format!(
        "ulimit --native-active-processes 1; {} n8-quota-marker {} | {} n8-quota-marker {} {}; printf 'pipeline=%s\\n' $status; ulimit --native-active-processes not-configured; {} n8-quota-marker {}",
        f.probe(), qpath(&rejected), f.probe(), qpath(&first), quote(&release.name), f.probe(), qpath(&after)));
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let text = String::from_utf8(result.out).unwrap();
    assert_ne!(record(&text, "pipeline"), 0, "full Job accepted second root");
    assert!(!rejected.exists(), "rejected suspended child entered user code");
    assert_eq!(fs::read(&after).unwrap(), b"entered", "rollback damaged root/host or subsequent groups");
    let diagnostic = String::from_utf8(result.err).unwrap();
    assert!(!diagnostic.is_empty(), "native assignment failure must be diagnosed");
}
pub fn process_cpu(f: &Fixture) {
    let text = output(f.eval(&format!("ulimit --native-process-cpu-seconds 1; {} n8-cpu-parent", f.probe())));
    let raw = u32::try_from(record(&text, "raw")).unwrap();
    assert!(raw > 255, "not kernel quota termination: {text}");
    assert_ne!(raw, 0xc000013a, "quota fabricated control-C provenance");
    assert!(record(&text, "user-100ns") >= 10_000_000, "not one USER CPU second: {text}");
    let status = fish::proc::ProcStatus::native(raw);
    assert_eq!(status.native_raw(), Some(raw));
    assert_eq!(status.status_value(), 1);
    assert!(!status.signal_exited());
    let result = f.eval(&format!("ulimit --native-process-cpu-seconds 1; {} n8-cpu-busy; printf 'status=%s\\n' $status; printf 'evaluator-continued\\n'", f.probe()));
    assert_eq!(result.code, 0);
    assert_eq!(result.out, b"status=1\nevaluator-continued\n");
    let diagnostic = String::from_utf8(result.err).unwrap().to_ascii_lowercase();
    assert!(diagnostic.contains(&format!("0x{raw:08x}")), "quota exit lost its native DWORD: {diagnostic}");
}
pub fn quota_scope(f: &Fixture) {
    // An actual enforced ancestor exists, but this query is configured policy,
    // not an invented unlimited value or the effective ancestor's 256 MiB.
    exact(f.eval(&format!("{} n8-ancestor-query {}", f.probe(), qpath(&f.fish))),
        0, all_unconfigured(), b"");
    let ready = Event::new("existing-group-ready");
    let release = Event::new("existing-group-release");
    let held = f.root.join("existing-group-memory.txt");
    let text = output(f.eval(&format!(
        "{} n8-job-hold {} {} {} 96 &; ulimit --native-process-memory-kib 65536; {} n8-release-event {} {}; wait; printf 'existing=%s\\n' $status; ulimit --native-process-memory-kib",
        f.probe(), quote(&ready.name), quote(&release.name), qpath(&held),
        f.probe(), quote(&ready.name), quote(&release.name))));
    assert_eq!(fs::read_to_string(&held).unwrap(), (96 * 1024 * 1024).to_string());
    assert_eq!(text, "existing=0\n65536\n", "existing groups were retroactively limited");
}
fn qpath(path: &Path) -> String { quote(path.to_str().unwrap()) }
fn access(path: &Path, mode: AccessFlags) -> io::Result<()> {
    waccess(&str2wcstring(path.to_str().unwrap()), mode)
}
pub fn access_boundaries(f: &Fixture) {
    let denied = f.root.join("denied-data-rights");
    fs::write(&denied, b"ACL protected data").unwrap();
    let _dacl = DeniedDacl::new(&denied);
    assert_eq!(access(&denied, AccessFlags::R_OK).unwrap_err().raw_os_error(), Some(5));
    assert_eq!(access(&denied, AccessFlags::W_OK).unwrap_err().raw_os_error(), Some(5));
    assert_eq!(fs::read(&denied).unwrap_err().raw_os_error(), Some(5));
    access(&denied, AccessFlags::F_OK).unwrap();
    exact(f.eval(&format!("test -r {}; printf '%s\\n' $status; test -w {}; printf '%s\\n' $status", qpath(&denied), qpath(&denied))),
        0, b"1\n1\n", b"");
    let readonly = f.root.join("read-only-attribute");
    fs::write(&readonly, b"readable").unwrap();
    let mut permissions = fs::metadata(&readonly).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&readonly, permissions).unwrap();
    let _restore = ReadOnlyRestore(readonly.clone());
    access(&readonly, AccessFlags::R_OK).unwrap();
    assert_eq!(access(&readonly, AccessFlags::W_OK).unwrap_err().raw_os_error(), Some(5));
    assert_eq!(OpenOptions::new().write(true).open(&readonly).unwrap_err().raw_os_error(), Some(5));
    exact(f.eval(&format!("test -r {}; printf '%s\\n' $status; test -w {}; printf '%s\\n' $status", qpath(&readonly), qpath(&readonly))),
        0, b"0\n1\n", b"");
    let sharing = f.root.join("exclusive-sharing");
    fs::write(&sharing, b"exclusive").unwrap();
    let held = OpenOptions::new().read(true).write(true).share_mode(0).open(&sharing).unwrap();
    assert_eq!(access(&sharing, AccessFlags::R_OK).unwrap_err().raw_os_error(), Some(32));
    assert_eq!(fs::read(&sharing).unwrap_err().raw_os_error(), Some(32));
    drop(held);
    access(&sharing, AccessFlags::R_OK).unwrap();
    let missing = access(&f.root.join("absent-predicate-file"), AccessFlags::R_OK).unwrap_err();
    assert!(matches!(missing.raw_os_error(), Some(2 | 3)), "{missing}");
}
pub fn reparses_devices(f: &Fixture) {
    let target = f.root.join("reparse-target");
    let link = f.root.join("actual-symbolic-link");
    let hard = f.root.join("actual-hard-link");
    fs::write(&target, b"reparse payload").unwrap();
    std::os::windows::fs::symlink_file(&target, &link).unwrap_or_else(|error|
        panic!("required real symbolic-link fixture needs Developer Mode or SeCreateSymbolicLinkPrivilege: {error}"));
    fs::hard_link(&target, &hard).unwrap();
    exact(f.eval(&format!("test -L {}; printf 'link=%s\\n' $status; test -f {}; printf 'target=%s\\n' $status; test {} -ef {}; printf 'identity=%s\\n' $status; test -L {}; printf 'hard-link=%s\\n' $status",
        qpath(&link), qpath(&link), qpath(&link), qpath(&hard), qpath(&hard))),
        0, b"link=0\ntarget=0\nidentity=0\nhard-link=1\n", b"");
    assert_eq!(fs::read(&link).unwrap(), b"reparse payload");
    fs::remove_file(&target).unwrap();
    exact(f.eval(&format!("test -L {}; printf 'link=%s\\n' $status; test -e {}; printf 'target=%s\\n' $status", qpath(&link), qpath(&link))),
        0, b"link=0\ntarget=1\n", b"");
    // Retention rejects all reparse entries, including this deliberately
    // dangling, exclusively owned test link. Remove only that exact entry.
    fs::remove_file(&link).unwrap();
    let opaque_path = f.root.join("opaque-non-symlink-reparse");
    let _opaque = OpaqueReparse::new(&opaque_path);
    use std::os::windows::fs::MetadataExt;
    let metadata = fs::symlink_metadata(&opaque_path).unwrap();
    assert_ne!(metadata.file_attributes() & 0x400, 0, "fixture must be a real reparse point");
    assert!(!metadata.file_type().is_symlink(), "opaque reparse tag became a Unix symlink");
    exact(f.eval(&format!("test -L {}; printf 'opaque-link=%s\\n' $status", qpath(&opaque_path))),
        0, b"opaque-link=1\n", b"");
    // NUL is a deliberately supported stream endpoint, not a filesystem name,
    // and must never turn into fabricated Unix character-device/mode identity.
    exact(f.eval(&format!("{} n8-device <NUL >NUL; printf 'nul=%s\\n' $status", f.probe())),
        0, b"nul=0\n", b"");
    for name in ["NUL", r"\\.\NUL", "CONIN$", "CONOUT$"] {
        assert_eq!(waccess(&str2wcstring(name), AccessFlags::F_OK).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
    for predicate in ["-b", "-c", "-G", "-g", "-k", "-O", "-p", "-S", "-u"] {
        let result = f.eval(&format!("test {predicate} {}", qpath(&hard)));
        assert_eq!(result.code, 2, "{predicate}: {}", String::from_utf8_lossy(&result.err));
        assert!(String::from_utf8_lossy(&result.err).contains("unsupported"));
    }
}
struct ReadOnlyRestore(PathBuf);
impl Drop for ReadOnlyRestore {
    fn drop(&mut self) {
        let mut permissions = fs::metadata(&self.0).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&self.0, permissions).unwrap();
    }
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn DeviceIoControl(handle: *mut c_void, code: u32, input: *const c_void, input_size: u32,
        output: *mut c_void, output_size: u32, returned: *mut u32, overlapped: *mut c_void) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(text: *const u16, revision: u32,
        descriptor: *mut *mut c_void, size: *mut u32) -> i32;
    fn GetFileSecurityW(name: *const u16, info: u32, descriptor: *mut c_void, size: u32, needed: *mut u32) -> i32;
    fn SetFileSecurityW(name: *const u16, info: u32, descriptor: *const c_void) -> i32;
    fn GetSecurityDescriptorControl(descriptor: *const c_void, control: *mut u16, revision: *mut u32) -> i32;
}
fn wide(text: &OsStr) -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() }
struct Event { name: String, _handle: OwnedHandle }
impl Event {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!("Local\\fish-n8-policy-{}-{}-{label}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(OsStr::new(&name)).as_ptr()) };
        assert!(!raw.is_null(), "{}", io::Error::last_os_error());
        Self { name, _handle: unsafe { OwnedHandle::from_raw_handle(raw) } }
    }
}
struct DeniedDacl { name: Vec<u16>, original: Vec<u8>, restore_flags: u32 }
impl DeniedDacl {
    fn new(path: &Path) -> Self {
        let name = wide(path.as_os_str());
        let mut needed = 0;
        assert_eq!(unsafe { GetFileSecurityW(name.as_ptr(), 4, std::ptr::null_mut(), 0, &mut needed) }, 0);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(122));
        let mut original = vec![0u8; needed as usize];
        assert_ne!(unsafe { GetFileSecurityW(name.as_ptr(), 4, original.as_mut_ptr().cast(), needed, &mut needed) }, 0);
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(unsafe { GetSecurityDescriptorControl(original.as_ptr().cast(), &mut control, &mut revision) }, 0);
        let restore_flags = 4 | if control & 0x1000 != 0 { 0x8000_0000 } else { 0x2000_0000 };
        // Deny only READ_DATA/WRITE_DATA. Metadata/WRITE_DAC stay available, so
        // existence queries and unconditional restoration remain real operations.
        let sddl = wide(OsStr::new("D:P(D;;0x3;;;WD)(A;;FA;;;WD)"));
        let mut descriptor = std::ptr::null_mut();
        assert_ne!(unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut descriptor, std::ptr::null_mut()) }, 0);
        let result = unsafe { SetFileSecurityW(name.as_ptr(), 0x8000_0004, descriptor) };
        let error = io::Error::last_os_error();
        unsafe { LocalFree(descriptor); }
        assert_ne!(result, 0, "required denied-DACL fixture: {error}");
        Self { name, original, restore_flags }
    }
}
impl Drop for DeniedDacl {
    fn drop(&mut self) {
        assert_ne!(unsafe { SetFileSecurityW(self.name.as_ptr(), self.restore_flags, self.original.as_ptr().cast()) }, 0,
            "restore fixture DACL: {}", io::Error::last_os_error());
    }
}

#[repr(C)]
struct ReparseGuid {
    tag: u32,
    length: u16,
    reserved: u16,
    guid: [u8; 16],
}
struct OpaqueReparse {
    file: fs::File,
    data: ReparseGuid,
}
impl OpaqueReparse {
    fn new(path: &Path) -> Self {
        let file = OpenOptions::new().read(true).write(true).create_new(true)
            .share_mode(7).custom_flags(0x0020_0000).open(path).unwrap();
        // Non-Microsoft, non-name-surrogate reparse data with its owning GUID.
        // No symlink target or filesystem filter is fabricated.
        let data = ReparseGuid { tag: 0x42, length: 0, reserved: 0,
            guid: [0x98, 0x0c, 0x27, 0x3f, 0xf1, 0x3a, 0x40, 0x4d,
                0xb7, 0x52, 0xe4, 0xe2, 0xd1, 0x71, 0xd4, 0x08] };
        let mut returned = 0;
        assert_ne!(unsafe { DeviceIoControl(file.as_raw_handle(), 0x0009_00a4,
            (&data as *const ReparseGuid).cast(), std::mem::size_of::<ReparseGuid>() as u32,
            std::ptr::null_mut(), 0, &mut returned, std::ptr::null_mut()) }, 0,
            "required opaque-reparse filesystem fixture: {}", io::Error::last_os_error());
        Self { file, data }
    }
}
impl Drop for OpaqueReparse {
    fn drop(&mut self) {
        let mut returned = 0;
        assert_ne!(unsafe { DeviceIoControl(self.file.as_raw_handle(), 0x0009_00ac,
            (&self.data as *const ReparseGuid).cast(), std::mem::size_of::<ReparseGuid>() as u32,
            std::ptr::null_mut(), 0, &mut returned, std::ptr::null_mut()) }, 0,
            "remove owned opaque reparse data: {}", io::Error::last_os_error());
    }
}
