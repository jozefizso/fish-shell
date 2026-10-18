//! Test-only isolation adapted from clipboard_windows' proven NETONLY fixture.
//! No clipboard API is called here, and failure never falls back to WinSta0.
use std::{
    ffi::{OsStr, c_void},
    io,
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
};
type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Luid {
    low: u32,
    high: i32,
}
#[repr(C)]
#[derive(Default)]
struct Statistics {
    token: Luid,
    auth: Luid,
    expiration: i64,
    kind: u32,
    impersonation: u32,
    charged: u32,
    available: u32,
    groups: u32,
    privileges: u32,
    modified: Luid,
}
#[repr(C)]
struct Security {
    length: u32,
    descriptor: Handle,
    inherit: i32,
}
#[repr(C)]
struct Startup {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    chars_x: u32,
    chars_y: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_data: *mut u8,
    input: Handle,
    output: Handle,
    error: Handle,
}
#[repr(C)]
struct Process {
    process: Handle,
    thread: Handle,
    pid: u32,
    tid: u32,
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        buffer: Handle,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: Handle, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut Handle,
        size: *mut u32,
    ) -> i32;
    fn CreateProcessWithLogonW(
        user: *const u16,
        domain: *const u16,
        password: *const u16,
        logon: u32,
        application: *const u16,
        command: *mut u16,
        flags: u32,
        environment: Handle,
        directory: *const u16,
        startup: *const Startup,
        process: *mut Process,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn GetCurrentThreadId() -> u32;
    fn CloseHandle(handle: Handle) -> i32;
    fn LocalFree(memory: Handle) -> Handle;
    fn CreateDirectoryW(path: *const u16, security: *const Security) -> i32;
    fn ResumeThread(thread: Handle) -> u32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn TerminateProcess(process: Handle, status: u32) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetProcessWindowStation() -> Handle;
    fn GetThreadDesktop(thread: u32) -> Handle;
    fn CreateWindowStationW(
        name: *const u16,
        flags: u32,
        access: u32,
        security: *const Security,
    ) -> Handle;
    fn SetProcessWindowStation(station: Handle) -> i32;
    fn CloseWindowStation(station: Handle) -> i32;
    fn CreateDesktopW(
        name: *const u16,
        device: *const u16,
        mode: Handle,
        flags: u32,
        access: u32,
        security: *const Security,
    ) -> Handle;
    fn SetThreadDesktop(desktop: Handle) -> i32;
    fn CloseDesktop(desktop: Handle) -> i32;
    fn GetUserObjectInformationW(
        object: Handle,
        index: i32,
        data: Handle,
        bytes: u32,
        needed: *mut u32,
    ) -> i32;
}
fn check(v: i32, what: &str) {
    assert_ne!(v, 0, "{what}: {}", io::Error::last_os_error());
}
fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    let v: Vec<_> = s.as_ref().encode_wide().collect();
    assert!(!v.contains(&0));
    v.into_iter().chain([0]).collect()
}
struct Kernel(Handle);
impl Drop for Kernel {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Local(Handle);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn token(process: Handle) -> Kernel {
    let mut t = null_mut();
    check(
        unsafe { OpenProcessToken(process, 8, &mut t) },
        "OpenProcessToken",
    );
    Kernel(t)
}
fn authentication(process: Handle) -> Luid {
    let t = token(process);
    let mut data = Statistics::default();
    let mut needed = 0;
    check(
        unsafe {
            GetTokenInformation(
                t.0,
                10,
                (&mut data as *mut Statistics).cast(),
                std::mem::size_of::<Statistics>() as u32,
                &mut needed,
            )
        },
        "TokenStatistics",
    );
    assert_eq!(needed as usize, std::mem::size_of::<Statistics>());
    data.auth
}
pub fn authentication_text() -> String {
    let a = authentication(unsafe { GetCurrentProcess() });
    format!("{}:{}", a.high, a.low)
}
fn security(inheritance: &str) -> Local {
    let t = token(unsafe { GetCurrentProcess() });
    let mut needed = 0;
    unsafe {
        GetTokenInformation(t.0, 1, null_mut(), 0, &mut needed);
    }
    assert!(needed > 0 && needed <= 65536);
    let mut data = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    check(
        unsafe { GetTokenInformation(t.0, 1, data.as_mut_ptr().cast(), needed, &mut needed) },
        "TokenUser",
    );
    let sid = unsafe { *data.as_ptr().cast::<Handle>() };
    let mut text = null_mut();
    check(
        unsafe { ConvertSidToStringSidW(sid, &mut text) },
        "SID text",
    );
    let owned = Local(text.cast());
    let mut n = 0;
    while n < 256 && unsafe { *text.add(n) } != 0 {
        n += 1;
    }
    assert!(n < 256);
    let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(text, n) }).unwrap();
    drop(owned);
    let mut descriptor = null_mut();
    check(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(format!(
                    "D:P(A;{inheritance};GA;;;SY)(A;{inheritance};GA;;;{sid})"
                ))
                .as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        },
        "protected fixture DACL",
    );
    Local(descriptor)
}
fn object_name(object: Handle) -> String {
    let mut text = [0u16; 512];
    let mut needed = 0;
    check(
        unsafe {
            GetUserObjectInformationW(
                object,
                2,
                text.as_mut_ptr().cast(),
                std::mem::size_of_val(&text) as u32,
                &mut needed,
            )
        },
        "UOI_NAME",
    );
    assert!(needed >= 2 && needed as usize <= std::mem::size_of_val(&text));
    let end = text.iter().position(|&c| c == 0).unwrap();
    String::from_utf16(&text[..end]).unwrap()
}
pub struct PrivateStation {
    old: Handle,
    old_desktop: Handle,
    station: Handle,
    desktop: Handle,
    switched: bool,
    thread_switched: bool,
    pub name: String,
}
impl PrivateStation {
    pub fn create(parent_auth: &str) -> Self {
        assert_ne!(
            authentication_text(),
            parent_auth,
            "fresh authentication identity required before any station or clipboard operation"
        );
        let mut s = Self {
            old: unsafe { GetProcessWindowStation() },
            old_desktop: unsafe { GetThreadDesktop(GetCurrentThreadId()) },
            station: null_mut(),
            desktop: null_mut(),
            switched: false,
            thread_switched: false,
            name: String::new(),
        };
        assert!(!s.old.is_null() && !s.old_desktop.is_null());
        let old_name = object_name(s.old);
        let d = security("");
        let a = Security {
            length: std::mem::size_of::<Security>() as u32,
            descriptor: d.0,
            inherit: 0,
        };
        s.station = unsafe {
            CreateWindowStationW(null(), 1, 0x0001 | 0x0002 | 0x0004 | 0x0008 | 0x0020, &a)
        };
        assert!(
            !s.station.is_null(),
            "anonymous CWF_CREATE_ONLY station: {}",
            io::Error::last_os_error()
        );
        check(
            unsafe { SetProcessWindowStation(s.station) },
            "private process station",
        );
        s.switched = true;
        s.desktop = unsafe {
            CreateDesktopW(
                wide("fish-private-clipboard").as_ptr(),
                null(),
                null_mut(),
                0,
                0x83,
                &a,
            )
        };
        assert!(
            !s.desktop.is_null(),
            "private desktop: {}",
            io::Error::last_os_error()
        );
        check(
            unsafe { SetThreadDesktop(s.desktop) },
            "private thread desktop",
        );
        s.thread_switched = true;
        s.name = object_name(unsafe { GetProcessWindowStation() });
        assert_ne!(s.name, old_name);
        assert!(!s.name.eq_ignore_ascii_case("WinSta0"));
        assert_eq!(s.name, object_name(s.station));
        s
    }
    pub fn desktop_path(&self) -> String {
        format!("{}\\{}", self.name, object_name(self.desktop))
    }
}
impl Drop for PrivateStation {
    fn drop(&mut self) {
        unsafe {
            if self.thread_switched {
                SetThreadDesktop(self.old_desktop);
            }
            if self.switched {
                SetProcessWindowStation(self.old);
            }
            if !self.desktop.is_null() {
                CloseDesktop(self.desktop);
            }
            if !self.station.is_null() {
                CloseWindowStation(self.station);
            }
        }
    }
}
struct Child(Handle);
impl Drop for Child {
    fn drop(&mut self) {
        unsafe {
            if WaitForSingleObject(self.0, 0) != 0 {
                TerminateProcess(self.0, 99);
                WaitForSingleObject(self.0, 5000);
            }
            CloseHandle(self.0);
        }
    }
}
fn quoted(s: &OsStr) -> Vec<u16> {
    let mut out = vec![34];
    let mut slashes = 0;
    for c in s.encode_wide() {
        if c == 92 {
            slashes += 1;
            continue;
        }
        if c == 34 {
            out.extend(std::iter::repeat_n(92, slashes * 2 + 1));
        } else {
            out.extend(std::iter::repeat_n(92, slashes));
        }
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n(92, slashes * 2));
    out.push(34);
    out
}
pub fn launch(root: &Path, fish: &Path, probe: &Path) {
    let evidence = root.join("private-clipboard");
    let d = security("OICI");
    let a = Security {
        length: std::mem::size_of::<Security>() as u32,
        descriptor: d.0,
        inherit: 0,
    };
    check(
        unsafe { CreateDirectoryW(wide(&evidence).as_ptr(), &a) },
        "create-new protected clipboard evidence",
    );
    let parent = authentication_text();
    let arguments = [
        probe.as_os_str(),
        OsStr::new("n6-clipboard-private"),
        root.as_os_str(),
        fish.as_os_str(),
        probe.as_os_str(),
        OsStr::new(&parent),
    ];
    let mut command = Vec::new();
    for (n, arg) in arguments.iter().enumerate() {
        if n != 0 {
            command.push(32);
        }
        command.extend(quoted(arg));
    }
    command.push(0);
    let system = std::env::var_os("SystemRoot").expect("required SystemRoot");
    let mut entries = vec![
        ("SystemRoot", system.as_os_str()),
        ("WINDIR", system.as_os_str()),
        ("TEMP", evidence.as_os_str()),
        ("TMP", evidence.as_os_str()),
    ];
    entries.sort_by_key(|(k, _)| k.to_ascii_uppercase());
    let mut environment = Vec::new();
    for (k, v) in entries {
        environment.extend(format!("{k}=").encode_utf16());
        environment.extend(wide(v));
    }
    environment.push(0);
    let mut startup: Startup = unsafe { std::mem::zeroed() };
    startup.size = std::mem::size_of::<Startup>() as u32;
    let mut process: Process = unsafe { std::mem::zeroed() };
    check(
        unsafe {
            CreateProcessWithLogonW(
                wide("fish-local-fixture").as_ptr(),
                wide(".").as_ptr(),
                wide("fixture-not-network-credentials").as_ptr(),
                2,
                wide(probe).as_ptr(),
                command.as_mut_ptr(),
                0x00000004 | 0x08000000 | 0x00000400,
                environment.as_mut_ptr().cast(),
                wide(&evidence).as_ptr(),
                &startup,
                &mut process,
            )
        },
        "required fresh NETONLY logon",
    );
    let child = Child(process.process);
    let thread = Kernel(process.thread);
    assert_ne!(
        authentication(child.0),
        authentication(unsafe { GetCurrentProcess() }),
        "fresh auth before resume"
    );
    assert_ne!(
        unsafe { ResumeThread(thread.0) },
        u32::MAX,
        "resume private helper"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(child.0, 20_000) },
        0,
        "bounded private editor clipboard scenario"
    );
    let mut code = 0;
    check(
        unsafe { GetExitCodeProcess(child.0, &mut code) },
        "private helper exit",
    );
    assert_eq!(
        code,
        0,
        "private editor clipboard helper failed; see {}",
        evidence.display()
    );
}
