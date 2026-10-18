//! Native immediate-data Unicode clipboard support.
//!
//! Copy rejects NUL, expands bare LF to CRLF and preserves CRLF/lone CR. Paste
//! requires terminated, well-formed CF_UNICODETEXT and folds CRLF to LF, preserving
//! lone CR and all trailing newlines. Empty text is valid; no Unicode format is
//! NotFound, never an empty success. Busy/denied/unavailable services are errors.
use std::ffi::c_void;
use std::io;
use std::ptr::{null, null_mut};

type Handle = *mut c_void;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 2;
// This also bounds the foreign-memory NUL scan. Refuse unreasonable allocations
// rather than constructing slices or searching beyond the advertised allocation.
const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
// Three UTF-8 bytes per UTF-16 unit is the largest valid expansion. Reserve
// the format's terminating NUL before accepting a pipe-backed clipboard copy.
pub(crate) const MAX_INPUT_UTF8_BYTES: usize = (MAX_TEXT_BYTES / 2 - 1) * 3;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, memory: Handle) -> Handle;
    fn GetClipboardData(format: u32) -> Handle;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        parameter: Handle,
    ) -> Handle;
    fn DestroyWindow(window: Handle) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GlobalAlloc(flags: u32, size: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> Handle;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalSize(memory: Handle) -> usize;
    fn SetLastError(error: u32);
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Clipboard;
impl Clipboard {
    fn open(owner: Handle) -> io::Result<Self> {
        if unsafe { OpenClipboard(owner) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self)
    }
    fn close(self) -> io::Result<()> {
        // CloseClipboard ends access even on errors; do not retry or run a second
        // close against a different clipboard acquisition from Drop.
        std::mem::forget(self);
        if unsafe { CloseClipboard() } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
struct OwnerWindow(Handle);
impl OwnerWindow {
    fn new() -> io::Result<Self> {
        let class = [
            b'S' as u16,
            b'T' as u16,
            b'A' as u16,
            b'T' as u16,
            b'I' as u16,
            b'C' as u16,
            0,
        ];
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null(),
                0,
                0,
                0,
                0,
                0,
                -3isize as Handle,
                null_mut(),
                GetModuleHandleW(null()),
                null_mut(),
            )
        };
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(window))
    }
    fn destroy(self) -> io::Result<()> {
        if unsafe { DestroyWindow(self.0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        std::mem::forget(self);
        Ok(())
    }
}
impl Drop for OwnerWindow {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}
struct Memory(Handle);
impl Memory {
    fn allocate(bytes: usize) -> io::Result<Self> {
        let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
        if memory.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(memory))
    }
    fn transfer(mut self, format: u32) -> io::Result<()> {
        if unsafe { SetClipboardData(format, self.0) }.is_null() {
            return Err(io::Error::last_os_error());
        }
        // Only a successful SetClipboardData transfers ownership to Windows.
        self.0 = null_mut();
        Ok(())
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}
struct LockedMemory {
    memory: Handle,
    pointer: Handle,
}
impl LockedMemory {
    fn lock(memory: Handle) -> io::Result<Self> {
        let pointer = unsafe { GlobalLock(memory) };
        if pointer.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { memory, pointer })
    }
    fn unlock(self) -> io::Result<()> {
        unsafe {
            SetLastError(0);
        }
        let result = unsafe { GlobalUnlock(self.memory) };
        let error = io::Error::last_os_error();
        std::mem::forget(self);
        // Zero is also the normal result when the final lock was released.
        if result == 0 && error.raw_os_error() != Some(0) {
            return Err(error);
        }
        Ok(())
    }
}
impl Drop for LockedMemory {
    fn drop(&mut self) {
        unsafe {
            GlobalUnlock(self.memory);
        }
    }
}

pub fn copy_text(text: &str) -> io::Result<()> {
    if text.contains('\0') {
        return Err(invalid("clipboard text contains NUL"));
    }
    let mut previous_cr = false;
    let mut units = 1usize;
    for ch in text.chars() {
        units = units
            .checked_add(ch.len_utf16() + usize::from(ch == '\n' && !previous_cr))
            .ok_or_else(|| invalid("clipboard text is too large"))?;
        previous_cr = ch == '\r';
    }
    let bytes = units
        .checked_mul(2)
        .filter(|&n| n <= MAX_TEXT_BYTES)
        .ok_or_else(|| invalid("clipboard text exceeds 64 MiB"))?;
    // Allocate and encode before EmptyClipboard: invalid input/allocation failure
    // must not discard the existing clipboard contents.
    let memory = Memory::allocate(bytes)?;
    let locked = LockedMemory::lock(memory.0)?;
    let destination =
        unsafe { std::slice::from_raw_parts_mut(locked.pointer.cast::<u16>(), units) };
    let mut offset = 0;
    previous_cr = false;
    for ch in text.chars() {
        if ch == '\n' && !previous_cr {
            destination[offset] = 13;
            offset += 1;
        }
        offset += ch.encode_utf16(&mut destination[offset..]).len();
        previous_cr = ch == '\r';
    }
    destination[offset] = 0;
    locked.unlock()?;
    let owner = OwnerWindow::new()?;
    let clipboard = Clipboard::open(owner.0)?;
    if unsafe { EmptyClipboard() } == 0 {
        return Err(io::Error::last_os_error());
    }
    memory.transfer(CF_UNICODETEXT)?;
    clipboard.close()?;
    // Immediate data is independent of the owner lifetime. Do not retain an
    // unpumped window or use delayed rendering callbacks on the shell thread.
    owner.destroy()
}

pub fn paste_text() -> io::Result<String> {
    let clipboard = Clipboard::open(null_mut())?;
    let result = (|| {
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "clipboard has no Unicode text",
            ));
        }
        let memory = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if memory.is_null() {
            return Err(io::Error::last_os_error());
        }
        decode_text_memory(memory)
    })();
    let closed = clipboard.close();
    match result {
        Ok(text) => {
            closed?;
            Ok(text)
        }
        Err(error) => Err(error),
    }
}

fn decode_text_memory(memory: Handle) -> io::Result<String> {
    let bytes = unsafe { GlobalSize(memory) };
    if bytes < 2 || bytes % 2 != 0 || bytes > MAX_TEXT_BYTES {
        return Err(invalid("invalid or oversized Unicode clipboard allocation"));
    }
    let locked = LockedMemory::lock(memory)?;
    let units = unsafe { std::slice::from_raw_parts(locked.pointer.cast::<u16>(), bytes / 2) };
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| invalid("Unicode clipboard allocation has no NUL terminator"))?;
    // CF_UNICODETEXT ends at its first NUL; padding is not text.
    let mut text = String::with_capacity(end);
    let mut chars = char::decode_utf16(units[..end].iter().copied()).peekable();
    while let Some(ch) = chars.next() {
        let ch = ch.map_err(|_| invalid("clipboard contains unpaired UTF-16 surrogates"))?;
        if ch == '\r' && matches!(chars.peek(), Some(Ok('\n'))) {
            continue;
        }
        text.push(ch);
    }
    locked.unlock()?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const EXACT_TEST: &str = "clipboard_windows::tests::private_station_provider_regression";
    const PROTOCOL: &str = "FISH_PRIVATE_CLIPBOARD_FIXTURE";
    const AUTH: &str = "FISH_PRIVATE_CLIPBOARD_AUTH";
    const EVIDENCE: &str = "FISH_PRIVATE_CLIPBOARD_EVIDENCE";
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 258;
    const STATION_RIGHTS: u32 = 0x0001 | 0x0002 | 0x0004 | 0x0008 | 0x0020;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Luid {
        low: u32,
        high: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct TokenStatistics {
        token: Luid,
        authentication: Luid,
        expiration: i64,
        token_type: u32,
        impersonation: u32,
        charged: u32,
        available: u32,
        groups: u32,
        privileges: u32,
        modified: Luid,
    }
    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        descriptor: Handle,
        inherit: i32,
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
    struct ProcessInfo {
        process: Handle,
        thread: Handle,
        process_id: u32,
        thread_id: u32,
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
            startup: *const StartupInfo,
            information: *mut ProcessInfo,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateDirectoryW(path: *const u16, security: *const SecurityAttributes) -> i32;
        fn GetCurrentProcess() -> Handle;
        fn GetCurrentThreadId() -> u32;
        fn CloseHandle(handle: Handle) -> i32;
        fn LocalFree(memory: Handle) -> Handle;
        fn ResumeThread(thread: Handle) -> u32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
        fn TerminateProcess(process: Handle, status: u32) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetProcessWindowStation() -> Handle;
        fn GetThreadDesktop(thread: u32) -> Handle;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
        fn CreateWindowStationW(
            name: *const u16,
            flags: u32,
            access: u32,
            security: *const SecurityAttributes,
        ) -> Handle;
        fn OpenWindowStationW(name: *const u16, inherit: i32, access: u32) -> Handle;
        fn SetProcessWindowStation(station: Handle) -> i32;
        fn CloseWindowStation(station: Handle) -> i32;
        fn CreateDesktopW(
            name: *const u16,
            device: *const u16,
            mode: Handle,
            flags: u32,
            access: u32,
            security: *const SecurityAttributes,
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
        fn GetClipboardOwner() -> Handle;
        fn IsWindow(window: Handle) -> i32;
    }

    fn check(result: i32, operation: &str) {
        assert_ne!(result, 0, "{operation}: {}", io::Error::last_os_error());
    }
    fn wide(text: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
        let text: Vec<_> = text.as_ref().encode_wide().collect();
        assert!(!text.contains(&0));
        text.into_iter().chain([0]).collect()
    }
    struct KernelHandle(Handle);
    impl Drop for KernelHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct LocalMemory(Handle);
    impl Drop for LocalMemory {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn token(process: Handle) -> KernelHandle {
        let mut handle = null_mut();
        check(
            unsafe { OpenProcessToken(process, 8, &mut handle) },
            "OpenProcessToken",
        );
        KernelHandle(handle)
    }
    fn authentication(process: Handle) -> Luid {
        let token = token(process);
        let mut statistics = TokenStatistics::default();
        let mut needed = 0;
        check(
            unsafe {
                GetTokenInformation(
                    token.0,
                    10,
                    (&mut statistics as *mut TokenStatistics).cast(),
                    std::mem::size_of::<TokenStatistics>() as u32,
                    &mut needed,
                )
            },
            "TokenStatistics",
        );
        assert_eq!(needed as usize, std::mem::size_of::<TokenStatistics>());
        statistics.authentication
    }
    fn auth_text(auth: Luid) -> String {
        format!("{}:{}", auth.high, auth.low)
    }
    fn restrictive_security(inheritance: &str) -> LocalMemory {
        let token = token(unsafe { GetCurrentProcess() });
        let mut needed = 0;
        unsafe {
            GetTokenInformation(token.0, 1, null_mut(), 0, &mut needed);
        }
        assert!(needed > 0 && needed <= 65536);
        // usize storage guarantees TOKEN_USER's pointer alignment.
        let mut user = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        check(
            unsafe {
                GetTokenInformation(token.0, 1, user.as_mut_ptr().cast(), needed, &mut needed)
            },
            "TokenUser",
        );
        let sid = unsafe { *(user.as_ptr().cast::<Handle>()) };
        let mut text = null_mut();
        check(
            unsafe { ConvertSidToStringSidW(sid, &mut text) },
            "ConvertSidToStringSidW",
        );
        let text_memory = LocalMemory(text.cast());
        let mut length = 0;
        // Windows SID strings are bounded by SECURITY_MAX_SID_STRING_CHARACTERS.
        while length < 256 && unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        assert!(length < 256);
        let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).unwrap();
        drop(text_memory);
        // Protected DACL: only this token's user and LocalSystem, no inherited
        // interactive/everyone ACE, and no real credentials in any channel.
        let sddl = wide(format!(
            "D:P(A;{inheritance};GA;;;SY)(A;{inheritance};GA;;;{sid})"
        ));
        let mut descriptor = null_mut();
        check(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut descriptor,
                    null_mut(),
                )
            },
            "explicit restrictive DACL",
        );
        LocalMemory(descriptor)
    }
    fn object_name(handle: Handle) -> String {
        let mut text = [0u16; 512];
        let mut needed = 0;
        check(
            unsafe {
                GetUserObjectInformationW(
                    handle,
                    2,
                    text.as_mut_ptr().cast(),
                    std::mem::size_of_val(&text) as u32,
                    &mut needed,
                )
            },
            "UOI_NAME",
        );
        assert!(needed >= 2 && needed as usize <= std::mem::size_of_val(&text));
        let end = text
            .iter()
            .position(|&ch| ch == 0)
            .expect("terminated object name");
        String::from_utf16(&text[..end]).unwrap()
    }
    struct PrivateStation {
        original: Handle,
        original_desktop: Handle,
        station: Handle,
        desktop: Handle,
        station_switched: bool,
        desktop_switched: bool,
    }
    impl PrivateStation {
        fn create() -> Self {
            let mut owned = Self {
                original: unsafe { GetProcessWindowStation() },
                original_desktop: unsafe { GetThreadDesktop(GetCurrentThreadId()) },
                station: null_mut(),
                desktop: null_mut(),
                station_switched: false,
                desktop_switched: false,
            };
            assert!(!owned.original.is_null() && !owned.original_desktop.is_null());
            let original_name = object_name(owned.original);
            let security = restrictive_security("");
            let attributes = SecurityAttributes {
                length: std::mem::size_of::<SecurityAttributes>() as u32,
                descriptor: security.0,
                inherit: 0,
            };
            // NULL + CWF_CREATE_ONLY is intentional: named stations require an
            // administrator, and opening/reusing an auto station is forbidden.
            owned.station = unsafe { CreateWindowStationW(null(), 1, STATION_RIGHTS, &attributes) };
            assert!(
                !owned.station.is_null(),
                "anonymous CWF_CREATE_ONLY: {}",
                io::Error::last_os_error()
            );
            check(
                unsafe { SetProcessWindowStation(owned.station) },
                "private process station",
            );
            owned.station_switched = true;
            let name = wide("fish-private-clipboard");
            // CREATEWINDOW | READOBJECTS | WRITEOBJECTS, not desktop GENERIC_ALL.
            owned.desktop =
                unsafe { CreateDesktopW(name.as_ptr(), null(), null_mut(), 0, 0x83, &attributes) };
            assert!(
                !owned.desktop.is_null(),
                "private desktop: {}",
                io::Error::last_os_error()
            );
            check(
                unsafe { SetThreadDesktop(owned.desktop) },
                "private thread desktop",
            );
            owned.desktop_switched = true;
            let actual = object_name(unsafe { GetProcessWindowStation() });
            assert_ne!(actual, original_name, "never access original clipboard");
            assert!(
                !actual.eq_ignore_ascii_case("WinSta0"),
                "never access interactive clipboard"
            );
            assert_eq!(actual, object_name(owned.station));
            owned
        }
        fn finish(mut self) {
            check(
                unsafe { SetThreadDesktop(self.original_desktop) },
                "restore thread desktop",
            );
            self.desktop_switched = false;
            check(
                unsafe { SetProcessWindowStation(self.original) },
                "restore process station",
            );
            self.station_switched = false;
            check(unsafe { CloseDesktop(self.desktop) }, "close owned desktop");
            self.desktop = null_mut();
            check(
                unsafe { CloseWindowStation(self.station) },
                "close owned station",
            );
            self.station = null_mut();
        }
    }
    impl Drop for PrivateStation {
        fn drop(&mut self) {
            unsafe {
                if self.desktop_switched {
                    SetThreadDesktop(self.original_desktop);
                }
                if self.station_switched {
                    SetProcessWindowStation(self.original);
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

    struct EvidenceDirectory(PathBuf);
    impl EvidenceDirectory {
        fn create() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fish-private-clipboard-{}-{unique}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let security = restrictive_security("OICI");
            let attributes = SecurityAttributes {
                length: std::mem::size_of::<SecurityAttributes>() as u32,
                descriptor: security.0,
                inherit: 0,
            };
            check(
                unsafe { CreateDirectoryW(wide(&path).as_ptr(), &attributes) },
                "create-new protected evidence directory",
            );
            Self(path)
        }
    }
    impl Drop for EvidenceDirectory {
        fn drop(&mut self) {
            // Only this unique, create-new directory is ever removed.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct ChildProcess(Handle);
    impl Drop for ChildProcess {
        fn drop(&mut self) {
            unsafe {
                if WaitForSingleObject(self.0, 0) != WAIT_OBJECT_0 {
                    TerminateProcess(self.0, 99);
                    WaitForSingleObject(self.0, 5000);
                }
                CloseHandle(self.0);
            }
        }
    }
    fn launch_private_child() {
        let directory = EvidenceDirectory::create();
        let evidence = directory.0.join("result.txt");
        let executable = std::env::current_exe().expect("current test executable");
        assert!(
            !executable
                .as_os_str()
                .encode_wide()
                .any(|ch| ch == b'\"' as u16)
        );
        let application = wide(&executable);
        let mut command = vec![b'"' as u16];
        command.extend(executable.as_os_str().encode_wide());
        command.extend(wide(format!(
            "\" --exact {EXACT_TEST} --test-threads=1 --nocapture"
        )));
        let root = std::env::var_os("SystemRoot").expect("required SystemRoot");
        let temp = directory.0.as_os_str();
        // No inherited user configuration, credentials, test filters, PATH,
        // injection variables or unrelated fixture markers.
        let mut entries = vec![
            ("SystemRoot", root.as_os_str()),
            ("WINDIR", root.as_os_str()),
            ("TEMP", temp),
            ("TMP", temp),
            (PROTOCOL, std::ffi::OsStr::new("fresh-logon-v1")),
            (EVIDENCE, evidence.as_os_str()),
        ];
        let parent_auth = authentication(unsafe { GetCurrentProcess() });
        let parent_identity = auth_text(parent_auth);
        entries.push((AUTH, std::ffi::OsStr::new(&parent_identity)));
        entries.sort_by_key(|(key, _)| key.to_ascii_uppercase());
        let mut environment = Vec::<u16>::new();
        for (key, value) in entries {
            environment.extend(wide(format!("{key}=")).into_iter().take(key.len() + 1));
            let value = wide(value);
            environment.extend(value);
        }
        environment.push(0);
        let mut startup: StartupInfo = unsafe { std::mem::zeroed() };
        startup.size = std::mem::size_of::<StartupInfo>() as u32;
        let mut information: ProcessInfo = unsafe { std::mem::zeroed() };
        // Deliberately invalid, fixture-only network credentials. This creates a
        // fresh local authentication identity without profile/elevation/network
        // operations, and never authenticates these strings against any service.
        check(
            unsafe {
                CreateProcessWithLogonW(
                    wide("fish-local-fixture").as_ptr(),
                    wide(".").as_ptr(),
                    wide("fixture-not-network-credentials").as_ptr(),
                    2,
                    application.as_ptr(),
                    command.as_mut_ptr(),
                    0x00000004 | 0x08000000 | 0x00000400,
                    environment.as_mut_ptr().cast(),
                    wide(&directory.0).as_ptr(),
                    &startup,
                    &mut information,
                )
            },
            "required fresh-logon clipboard fixture",
        );
        let child = ChildProcess(information.process);
        let thread = KernelHandle(information.thread);
        let child_auth = authentication(child.0);
        assert_ne!(
            parent_auth, child_auth,
            "authentication isolation before resume"
        );
        assert_ne!(
            unsafe { ResumeThread(thread.0) },
            u32::MAX,
            "resume isolated test"
        );
        let wait = unsafe { WaitForSingleObject(child.0, 45000) };
        assert_eq!(
            wait, WAIT_OBJECT_0,
            "bounded isolated provider test (timeout={WAIT_TIMEOUT})"
        );
        let mut status = 0;
        check(
            unsafe { GetExitCodeProcess(child.0, &mut status) },
            "isolated child status",
        );
        let report = std::fs::read_to_string(&evidence).expect("required private fixture evidence");
        assert_eq!(status, 0, "isolated clipboard regression failed:\n{report}");
        assert_eq!(
            report,
            "fresh-logon;private-station;provider-regressions=passed\n"
        );
    }

    fn install_foreign(bytes: &[u8], allocation: usize) {
        assert!(bytes.len() <= allocation);
        let memory = Memory::allocate(allocation).unwrap();
        let locked = LockedMemory::lock(memory.0).unwrap();
        // Fill allocator padding too: unterminated cases cannot accidentally
        // acquire a NUL from GlobalAlloc rounding.
        let actual = unsafe { GlobalSize(memory.0) };
        assert!(actual >= allocation && actual <= MAX_TEXT_BYTES);
        unsafe {
            std::ptr::write_bytes(locked.pointer, 0x41, actual);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), locked.pointer.cast::<u8>(), bytes.len());
        }
        locked.unlock().unwrap();
        let owner = OwnerWindow::new().unwrap();
        let clipboard = Clipboard::open(owner.0).unwrap();
        check(unsafe { EmptyClipboard() }, "empty private clipboard");
        memory.transfer(CF_UNICODETEXT).unwrap();
        clipboard.close().unwrap();
        owner.destroy().unwrap();
    }
    fn utf16_bytes(units: &[u16]) -> Vec<u8> {
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
    }
    fn provider_regressions(station: &PrivateStation) {
        let text = "fish 雪 🐟 e\u{301}\nsecond\r\nthird\rlast\n\n";
        copy_text(text).unwrap();
        assert_eq!(
            paste_text().unwrap(),
            "fish 雪 🐟 e\u{301}\nsecond\nthird\rlast\n\n"
        );
        // Assert actual Windows representation, not just symmetric normalization.
        {
            let clipboard = Clipboard::open(null_mut()).unwrap();
            let memory = unsafe { GetClipboardData(CF_UNICODETEXT) };
            assert!(!memory.is_null());
            let locked = LockedMemory::lock(memory).unwrap();
            let units = unsafe {
                std::slice::from_raw_parts(locked.pointer.cast::<u16>(), GlobalSize(memory) / 2)
            };
            let end = units.iter().position(|&unit| unit == 0).unwrap();
            assert_eq!(
                String::from_utf16(&units[..end]).unwrap(),
                "fish 雪 🐟 e\u{301}\r\nsecond\r\nthird\rlast\r\n\r\n"
            );
            locked.unlock().unwrap();
            clipboard.close().unwrap();
        }
        // The copy's owner window has already been destroyed, but immediate data
        // must survive that destruction and a second clipboard acquisition.
        let owner = unsafe { GetClipboardOwner() };
        assert!(owner.is_null() || unsafe { IsWindow(owner) } == 0);
        assert_eq!(
            paste_text().unwrap(),
            "fish 雪 🐟 e\u{301}\nsecond\nthird\rlast\n\n"
        );
        assert!(copy_text("bad\0suffix").is_err());
        assert_eq!(
            paste_text().unwrap(),
            "fish 雪 🐟 e\u{301}\nsecond\nthird\rlast\n\n"
        );
        copy_text("").unwrap();
        assert_eq!(paste_text().unwrap(), "");
        {
            let owner = OwnerWindow::new().unwrap();
            let clipboard = Clipboard::open(owner.0).unwrap();
            check(
                unsafe { EmptyClipboard() },
                "empty clipboard has no Unicode format",
            );
            clipboard.close().unwrap();
            owner.destroy().unwrap();
        }
        assert_eq!(paste_text().unwrap_err().kind(), io::ErrorKind::NotFound);
        {
            let format =
                unsafe { RegisterClipboardFormatW(wide("fish-private-unsupported").as_ptr()) };
            assert_ne!(format, 0);
            let memory = Memory::allocate(2).unwrap();
            let locked = LockedMemory::lock(memory.0).unwrap();
            unsafe {
                std::ptr::write_bytes(locked.pointer, 0, GlobalSize(memory.0));
            }
            locked.unlock().unwrap();
            let owner = OwnerWindow::new().unwrap();
            let clipboard = Clipboard::open(owner.0).unwrap();
            check(
                unsafe { EmptyClipboard() },
                "empty before unsupported private format",
            );
            memory.transfer(format).unwrap();
            clipboard.close().unwrap();
            owner.destroy().unwrap();
        }
        assert_eq!(paste_text().unwrap_err().kind(), io::ErrorKind::NotFound);
        for units in [&[0xd800, 0][..], &[0xdc00, 0][..], &[0xd800, 0x41, 0][..]] {
            let bytes = utf16_bytes(units);
            install_foreign(&bytes, bytes.len());
            assert_eq!(paste_text().unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
        // The clipboard broker terminates CF_UNICODETEXT before returning it.
        // Exercise the production bounded decoder on a genuinely unterminated
        // owned native allocation instead of asserting broker-internal behavior.
        let unterminated = Memory::allocate(32).unwrap();
        let locked = LockedMemory::lock(unterminated.0).unwrap();
        unsafe {
            std::ptr::write_bytes(locked.pointer, 0x41, GlobalSize(unterminated.0));
        }
        locked.unlock().unwrap();
        assert_eq!(
            decode_text_memory(unterminated.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        // First NUL is the format terminator, not an embedded-NUL transport.
        install_foreign(&utf16_bytes(&[0x41, 0, 0xd800, 0]), 8);
        assert_eq!(paste_text().unwrap(), "A");
        // The real provider must refuse a foreign allocation above its scan cap.
        let oversized = Memory::allocate(MAX_TEXT_BYTES + 2).unwrap();
        let locked = LockedMemory::lock(oversized.0).unwrap();
        unsafe {
            std::ptr::write_bytes(locked.pointer, 0, GlobalSize(oversized.0));
        }
        locked.unlock().unwrap();
        let owner = OwnerWindow::new().unwrap();
        let clipboard = Clipboard::open(owner.0).unwrap();
        check(
            unsafe { EmptyClipboard() },
            "empty before oversized allocation",
        );
        oversized.transfer(CF_UNICODETEXT).unwrap();
        clipboard.close().unwrap();
        owner.destroy().unwrap();
        assert_eq!(paste_text().unwrap_err().kind(), io::ErrorKind::InvalidData);
        copy_text("before busy").unwrap();
        // A separate thread holds the *private* clipboard until an explicit
        // release barrier. No sleeps, retries, or interaction with WinSta0.
        std::thread::scope(|scope| {
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
            let desktop = station.desktop as usize;
            scope.spawn(move || {
                let previous = unsafe { GetThreadDesktop(GetCurrentThreadId()) };
                check(
                    unsafe { SetThreadDesktop(desktop as Handle) },
                    "busy-holder private desktop",
                );
                let owner = OwnerWindow::new().unwrap();
                let clipboard = Clipboard::open(owner.0).unwrap();
                ready_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                clipboard.close().unwrap();
                owner.destroy().unwrap();
                check(
                    unsafe { SetThreadDesktop(previous) },
                    "restore busy-holder desktop",
                );
            });
            ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(copy_text("must not replace").is_err());
            assert!(paste_text().is_err());
            release_tx.send(()).unwrap();
        });
        assert_eq!(paste_text().unwrap(), "before busy");
        // Open only our already-verified private station, with clipboard access
        // deliberately absent. The deny case may never target a caller station.
        let name = object_name(station.station);
        let restricted =
            unsafe { OpenWindowStationW(wide(&name).as_ptr(), 0, STATION_RIGHTS & !0x0004) };
        assert!(!restricted.is_null(), "open restricted private station");
        struct RestrictedStation {
            handle: Handle,
            restore: Handle,
        }
        impl Drop for RestrictedStation {
            fn drop(&mut self) {
                unsafe {
                    SetProcessWindowStation(self.restore);
                    CloseWindowStation(self.handle);
                }
            }
        }
        let restricted = RestrictedStation {
            handle: restricted,
            restore: station.station,
        };
        check(
            unsafe { SetProcessWindowStation(restricted.handle) },
            "restrict private station rights",
        );
        assert_eq!(object_name(unsafe { GetProcessWindowStation() }), name);
        assert_eq!(
            paste_text().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(copy_text("denied").is_err());
        drop(restricted);
        assert_eq!(paste_text().unwrap(), "before busy");
        // Successful subsequent operations demonstrate every failure path released
        // its open/lock/window resources instead of poisoning the service.
        copy_text("after failures 🐟\n").unwrap();
        assert_eq!(paste_text().unwrap(), "after failures 🐟\n");
    }

    /// One discovered behavioral test, reexecuted by exact name in an owned,
    /// suspended fresh-logon child. Missing isolation is a failure, never a skip.
    #[test]
    fn private_station_provider_regression() {
        match std::env::var_os(PROTOCOL) {
            None => launch_private_child(),
            Some(protocol) => {
                assert_eq!(protocol, "fresh-logon-v1");
                let parent_auth = std::env::var(AUTH).expect("owned parent identity");
                assert_ne!(
                    auth_text(authentication(unsafe { GetCurrentProcess() })),
                    parent_auth,
                    "fresh authentication before creating station"
                );
                let evidence =
                    PathBuf::from(std::env::var_os(EVIDENCE).expect("private evidence path"));
                assert!(evidence.is_absolute());
                assert_eq!(evidence.file_name().unwrap(), "result.txt");
                let result = std::panic::catch_unwind(|| {
                    let station = PrivateStation::create();
                    provider_regressions(&station);
                    station.finish();
                });
                let report = match &result {
                    Ok(()) => {
                        "fresh-logon;private-station;provider-regressions=passed\n".to_owned()
                    }
                    Err(payload) => format!(
                        "private provider failed: {}\n",
                        payload
                            .downcast_ref::<String>()
                            .map(String::as_str)
                            .or_else(|| payload.downcast_ref::<&str>().copied())
                            .unwrap_or("non-string panic")
                    ),
                };
                use std::io::Write;
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(Path::new(&evidence))
                    .expect("create-new private evidence");
                output.write_all(report.as_bytes()).unwrap();
                output.sync_all().unwrap();
                if let Err(payload) = result {
                    std::panic::resume_unwind(payload);
                }
            }
        }
    }
}
