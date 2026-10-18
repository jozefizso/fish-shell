//! Owned ConPTY transport and a console-client bootstrap for the N6 acceptance gates.
//! The bootstrap never reads input: fish is the sole console input consumer.
use std::ffi::{OsStr, OsString, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

type Handle = *mut c_void;
const LIMIT: Duration = Duration::from_secs(30);
const MAX_TRACE: usize = 16 * 1024 * 1024;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord {
    x: i16,
    y: i16,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}
#[repr(C)]
#[derive(Default)]
struct BufferInfo {
    size: Coord,
    cursor: Coord,
    attributes: u16,
    window: Rect,
    maximum: Coord,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CharInfo {
    unicode: u16,
    attributes: u16,
}
#[repr(C)]
struct StartupInfo {
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
    reserved_size: u16,
    reserved_bytes: *mut u8,
    stdin: Handle,
    stdout: Handle,
    stderr: Handle,
}
#[repr(C)]
struct StartupInfoEx {
    info: StartupInfo,
    attributes: Handle,
}
#[repr(C)]
struct ProcessInfo {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}
#[repr(C)]
#[derive(Default)]
struct BasicJobLimits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    minimum: usize,
    maximum: usize,
    active: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct IoCounters {
    read_ops: u64,
    write_ops: u64,
    other_ops: u64,
    read: u64,
    write: u64,
    other: u64,
}
#[repr(C)]
#[derive(Default)]
struct JobLimits {
    basic: BasicJobLimits,
    io: IoCounters,
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct KeyEvent {
    down: i32,
    repeat: u16,
    vk: u16,
    scan: u16,
    unicode: u16,
    controls: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct InputRecord {
    kind: u16,
    padding: u16,
    key: KeyEvent,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: Handle, size: u32) -> i32;
    fn CreatePseudoConsole(
        size: Coord,
        input: Handle,
        output: Handle,
        flags: u32,
        console: *mut Handle,
    ) -> i32;
    fn ResizePseudoConsole(console: Handle, size: Coord) -> i32;
    fn ClosePseudoConsole(console: Handle);
    fn InitializeProcThreadAttributeList(
        list: Handle,
        count: u32,
        flags: u32,
        bytes: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: Handle,
        flags: u32,
        attribute: usize,
        value: Handle,
        size: usize,
        previous: Handle,
        returned: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: Handle);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: Handle,
        thread_attributes: Handle,
        inherit: i32,
        flags: u32,
        environment: Handle,
        cwd: *const u16,
        startup: *mut StartupInfo,
        process: *mut ProcessInfo,
    ) -> i32;
    fn ResumeThread(thread: Handle) -> u32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn WaitForMultipleObjects(count: u32, handles: *const Handle, all: i32, millis: u32) -> u32;
    fn GetExitCodeProcess(handle: Handle, code: *mut u32) -> i32;
    fn CreateJobObjectW(attributes: Handle, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, information: Handle, size: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, code: u32) -> i32;
    fn TerminateProcess(process: Handle, code: u32) -> i32;
    fn CreateEventW(attributes: Handle, manual: i32, initial: i32, name: *const u16) -> Handle;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
    fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
    fn GetNumberOfConsoleInputEvents(console: Handle, count: *mut u32) -> i32;
    fn GetConsoleScreenBufferInfo(console: Handle, info: *mut BufferInfo) -> i32;
    fn ReadConsoleOutputW(
        console: Handle,
        buffer: *mut CharInfo,
        size: Coord,
        position: Coord,
        region: *mut Rect,
    ) -> i32;
    fn WriteConsoleInputW(
        console: Handle,
        records: *const InputRecord,
        count: u32,
        written: *mut u32,
    ) -> i32;
    fn GenerateConsoleCtrlEvent(kind: u32, group: u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn SetLastError(code: u32);
    fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32) -> Handle;
    fn FindNextChangeNotification(change: Handle) -> i32;
    fn FindCloseChangeNotification(change: Handle) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        sharing: u32,
        security: Handle,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetProcessWindowStation() -> Handle;
    fn GetUserObjectInformationW(
        object: Handle,
        index: i32,
        information: Handle,
        length: u32,
        needed: *mut u32,
    ) -> i32;
}
fn checked(ok: i32, operation: &str) {
    assert_ne!(ok, 0, "{operation}: {}", std::io::Error::last_os_error());
}
fn owned(handle: Handle, operation: &str) -> OwnedHandle {
    assert!(
        !handle.is_null() && handle as usize != usize::MAX,
        "{operation}: {}",
        std::io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(handle) }
}
fn wide(text: &OsStr) -> Vec<u16> {
    let mut text: Vec<u16> = text.encode_wide().collect();
    assert!(!text.contains(&0), "embedded NUL in native argument");
    text.push(0);
    text
}
fn pipe() -> (OwnedHandle, OwnedHandle) {
    let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
    checked(
        unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 0) },
        "CreatePipe",
    );
    (owned(read, "pipe reader"), owned(write, "pipe writer"))
}
fn millis(deadline: Instant) -> u32 {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .expect("console fixture deadline expired");
    remaining.as_millis().max(1).min(u32::MAX as u128 - 1) as u32
}
fn event(name: &OsStr, create: bool) -> OwnedHandle {
    let name = wide(name);
    unsafe {
        SetLastError(0);
    }
    let handle = unsafe {
        if create {
            CreateEventW(std::ptr::null_mut(), 0, 0, name.as_ptr())
        } else {
            OpenEventW(0x0010_0002, 0, name.as_ptr())
        }
    };
    let error = std::io::Error::last_os_error();
    let event = owned(handle, "owned console IPC event");
    assert!(
        !create || error.raw_os_error() != Some(183),
        "console IPC event was not newly owned"
    );
    event
}
fn signal(event: &OwnedHandle) {
    checked(unsafe { SetEvent(event.as_raw_handle()) }, "SetEvent");
}

// Length-prefixed UTF-16 preserves the exact selected executable/argv/environment.
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn get_u32(input: &mut &[u8]) -> u32 {
    assert!(input.len() >= 4, "truncated console IPC");
    let value = u32::from_le_bytes(input[..4].try_into().unwrap());
    *input = &input[4..];
    value
}
fn put_os(out: &mut Vec<u8>, value: &OsStr) {
    let value: Vec<u16> = value.encode_wide().collect();
    put_u32(out, value.len().try_into().unwrap());
    for unit in value {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}
fn get_os(input: &mut &[u8]) -> OsString {
    let count = get_u32(input) as usize;
    assert!(count <= input.len() / 2, "truncated UTF-16 console IPC");
    let value: Vec<u16> = input[..count * 2]
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
        .collect();
    *input = &input[count * 2..];
    OsString::from_wide(&value)
}
fn put_modes(out: &mut Vec<u8>, modes: (u32, u32)) {
    put_u32(out, modes.0);
    put_u32(out, modes.1);
}
fn get_modes(input: &mut &[u8]) -> (u32, u32) {
    (get_u32(input), get_u32(input))
}
fn read_modes(input: &File, output: &File) -> (u32, u32) {
    let (mut i, mut o) = (0, 0);
    checked(
        unsafe { GetConsoleMode(input.as_raw_handle(), &mut i) },
        "GetConsoleMode(input)",
    );
    checked(
        unsafe { GetConsoleMode(output.as_raw_handle(), &mut o) },
        "GetConsoleMode(output)",
    );
    (i, o)
}

#[derive(Debug)]
pub struct Snapshot {
    pub input_mode: u32,
    pub output_mode: u32,
    pub buffer_width: i16,
    pub buffer_height: i16,
    pub viewport: [i16; 4],
    pub cursor: [i16; 2],
    /// Entire native screen buffer, row-major; not reconstructed from the VT trace.
    pub cells: Vec<u16>,
}
impl Snapshot {
    fn capture(input: &File, output: &File) -> Self {
        let (input_mode, output_mode) = read_modes(input, output);
        let mut info = BufferInfo::default();
        checked(
            unsafe { GetConsoleScreenBufferInfo(output.as_raw_handle(), &mut info) },
            "GetConsoleScreenBufferInfo",
        );
        assert!(info.size.x > 0 && info.size.y > 0);
        let count = info.size.x as usize * info.size.y as usize;
        assert!(count <= 4 * 1024 * 1024, "unbounded console screen buffer");
        // Character counts are not cell counts when wide glyphs occupy two
        // cells. Read CHAR_INFO rectangles to retain the actual native grid.
        let mut native_cells = vec![CharInfo::default(); count];
        let mut region = Rect {
            left: 0,
            top: 0,
            right: info.size.x - 1,
            bottom: info.size.y - 1,
        };
        checked(
            unsafe {
                ReadConsoleOutputW(
                    output.as_raw_handle(),
                    native_cells.as_mut_ptr(),
                    info.size,
                    Coord::default(),
                    &mut region,
                )
            },
            "ReadConsoleOutputW",
        );
        assert_eq!(
            [region.left, region.top, region.right, region.bottom],
            [0, 0, info.size.x - 1, info.size.y - 1],
            "incomplete native screen rectangle"
        );
        let cells = native_cells.into_iter().map(|cell| cell.unicode).collect();
        Self {
            input_mode,
            output_mode,
            buffer_width: info.size.x,
            buffer_height: info.size.y,
            viewport: [
                info.window.left,
                info.window.top,
                info.window.right,
                info.window.bottom,
            ],
            cursor: [info.cursor.x, info.cursor.y],
            cells,
        }
    }
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_modes(&mut out, (self.input_mode, self.output_mode));
        for value in [self.buffer_width, self.buffer_height]
            .into_iter()
            .chain(self.viewport)
            .chain(self.cursor)
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        put_u32(&mut out, self.cells.len() as u32);
        for unit in &self.cells {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }
    fn decode(mut input: &[u8]) -> Self {
        let (input_mode, output_mode) = get_modes(&mut input);
        let mut values = [0i16; 8];
        assert!(input.len() >= 16);
        for (value, bytes) in values.iter_mut().zip(input[..16].chunks_exact(2)) {
            *value = i16::from_le_bytes(bytes.try_into().unwrap());
        }
        input = &input[16..];
        let count = get_u32(&mut input) as usize;
        assert_eq!(input.len(), count * 2);
        let cells = input
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
            .collect();
        Self {
            input_mode,
            output_mode,
            buffer_width: values[0],
            buffer_height: values[1],
            viewport: values[2..6].try_into().unwrap(),
            cursor: values[6..8].try_into().unwrap(),
            cells,
        }
    }
}
#[derive(Debug)]
pub struct Transcript {
    pub bytes: Vec<u8>,
    pub original_modes: (u32, u32),
    pub final_modes: (u32, u32),
}

#[derive(Default)]
struct Trace {
    bytes: Vec<u8>,
    protocol: Option<bool>,
    parser: u8,
    csi: Vec<u8>,
    eof: bool,
    error: Option<String>,
}
impl Trace {
    // Parse the actual stream across arbitrary pipe fragmentation. Ignore OSC payloads.
    fn append(&mut self, bytes: &[u8]) {
        if self.bytes.len() + bytes.len() > MAX_TRACE {
            self.error = Some("console trace exceeded 16 MiB bound".into());
            return;
        }
        self.bytes.extend_from_slice(bytes);
        for &byte in bytes {
            match self.parser {
                0 if byte == 0x1b => self.parser = 1,
                1 => match byte {
                    b'[' => {
                        self.csi.clear();
                        self.parser = 2;
                    }
                    b']' | b'P' | b'_' | b'^' => self.parser = 3,
                    0x1b => (),
                    _ => self.parser = 0,
                },
                2 if (0x40..=0x7e).contains(&byte) => {
                    if matches!(byte, b'h' | b'l')
                        && self.csi.first() == Some(&b'?')
                        && self.csi[1..].split(|b| *b == b';').any(|p| p == b"2004")
                    {
                        self.protocol = Some(byte == b'h');
                    }
                    self.parser = 0;
                }
                2 if byte == 0x1b => self.parser = 1,
                2 => {
                    if self.csi.len() < 256 {
                        self.csi.push(byte);
                    } else {
                        self.parser = 0;
                    }
                }
                3 if byte == 7 => self.parser = 0,
                3 if byte == 0x1b => self.parser = 4,
                4 => self.parser = if byte == b'\\' { 0 } else { 3 },
                _ => (),
            }
        }
    }
}
type SharedTrace = Arc<(Mutex<Trace>, Condvar)>;
struct Attributes(Vec<usize>);
impl Attributes {
    fn pointer(&mut self) -> Handle {
        self.0.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.pointer()) };
    }
}
struct Console(Option<Handle>);
impl Console {
    fn handle(&self) -> Handle {
        self.0.expect("closed ConPTY")
    }
    fn close(&mut self, strict: bool) {
        if let Some(handle) = self.0.take() {
            close_console(handle, strict);
        }
    }
}
impl Drop for Console {
    fn drop(&mut self) {
        self.close(false);
    }
}

struct InputWriter {
    sender: Option<mpsc::Sender<Vec<u8>>>,
    result: mpsc::Receiver<std::io::Result<()>>,
    done: mpsc::Receiver<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl InputWriter {
    fn new(handle: OwnedHandle) -> Self {
        let (sender, receive) = mpsc::channel::<Vec<u8>>();
        let (result_send, result) = mpsc::channel();
        let (done_send, done) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut file = File::from(handle);
            for bytes in receive {
                let result = file.write_all(&bytes);
                let failed = result.is_err();
                if result_send.send(result).is_err() || failed {
                    break;
                }
            }
            let _ = done_send.send(());
        });
        Self {
            sender: Some(sender),
            result,
            done,
            thread: Some(thread),
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        // Ownership across the writer thread makes the potentially blocking native
        // pipe write bounded for the test thread, including fragmented UTF-8.
        self.sender
            .as_ref()
            .expect("closed console input")
            .send(bytes.to_vec())
            .expect("console input writer stopped");
        self.result
            .recv_timeout(LIMIT)
            .expect("ConPTY input write deadline")
            .expect("ConPTY input write");
    }
    fn close(&mut self, strict: bool) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            if self.done.recv_timeout(LIMIT).is_ok() {
                let _ = thread.join();
            } else if strict {
                panic!("ConPTY input writer exceeded cleanup deadline");
            }
        }
    }
}
impl Drop for InputWriter {
    fn drop(&mut self) {
        self.close(false);
    }
}

pub struct Session {
    process: OwnedHandle,
    job: OwnedHandle,
    console: Option<Console>,
    input: Option<InputWriter>,
    request: OwnedHandle,
    response: OwnedHandle,
    root: PathBuf,
    trace: SharedTrace,
    reader: Option<std::thread::JoinHandle<()>>,
    reader_done: mpsc::Receiver<()>,
}
impl Session {
    /// Launch the actual selected executable with the command's explicit environment.
    /// Callers supply a curated environment (normally Fixture::command / env_clear).
    pub fn start(command: &Command, probe: &Path, evidence_root: &Path) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let root = evidence_root.join(format!("console-{id}"));
        fs::create_dir_all(evidence_root).unwrap();
        fs::create_dir(&root).expect("unique console evidence directory");
        let request_name = OsString::from(format!("Local\\fish-n6-request-{id}"));
        let response_name = OsString::from(format!("Local\\fish-n6-response-{id}"));
        let request = event(&request_name, true);
        let response = event(&response_name, true);
        let mut config = Vec::new();
        put_os(&mut config, command.get_program());
        let args: Vec<_> = command.get_args().collect();
        put_u32(&mut config, args.len() as u32);
        for arg in args {
            put_os(&mut config, arg);
        }
        let cwd = command
            .get_current_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        put_os(&mut config, cwd.as_os_str());
        let envs: Vec<_> = command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect();
        put_u32(&mut config, envs.len() as u32);
        for (key, value) in envs {
            put_os(&mut config, key);
            put_os(&mut config, value);
        }
        fs::write(root.join("launch.bin"), config).unwrap();
        let (input_read, input_write) = pipe();
        let (output_read, output_write) = pipe();
        let mut console = std::ptr::null_mut();
        let result = unsafe {
            CreatePseudoConsole(
                Coord { x: 100, y: 30 },
                input_read.as_raw_handle(),
                output_write.as_raw_handle(),
                0,
                &mut console,
            )
        };
        assert_eq!(
            result, 0,
            "required CreatePseudoConsole failed: {result:#x}"
        );
        let console = Console(Some(console));
        // Drain from creation through ClosePseudoConsole, including failure cleanup.
        let trace: SharedTrace = Arc::new((Mutex::new(Trace::default()), Condvar::new()));
        let reader_trace = trace.clone();
        let (done_send, reader_done) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut output = File::from(output_read);
            let mut buffer = [0; 8192];
            loop {
                let result = output.read(&mut buffer);
                let (lock, changed) = &*reader_trace;
                let mut state = lock.lock().unwrap();
                match result {
                    Ok(0) => state.eof = true,
                    Ok(count) => state.append(&buffer[..count]),
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                        state.eof = true
                    }
                    Err(error) => {
                        state.error = Some(error.to_string());
                        state.eof = true;
                    }
                }
                changed.notify_all();
                if state.eof {
                    break;
                }
            }
            let _ = done_send.send(());
        });
        let mut required = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut required);
        }
        assert!(required > 0);
        let mut storage = vec![0; required.div_ceil(std::mem::size_of::<usize>())];
        checked(
            unsafe {
                InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut required)
            },
            "InitializeProcThreadAttributeList",
        );
        let mut attributes = Attributes(storage);
        checked(
            unsafe {
                UpdateProcThreadAttribute(
                    attributes.pointer(),
                    0,
                    0x0002_0016,
                    console.handle(),
                    std::mem::size_of::<Handle>(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            "ConPTY process attribute",
        );
        let application = wide(probe.as_os_str());
        let bootstrap_args = [
            probe.as_os_str(),
            OsStr::new("n6-console-bootstrap"),
            root.as_os_str(),
            &request_name,
            &response_name,
        ];
        let mut native_command = quote_command(&bootstrap_args);
        let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
        let mut desktop = private_desktop(command);
        startup.info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
        startup.attributes = attributes.pointer();
        if let Some(desktop) = &mut desktop {
            startup.info.desktop = desktop.as_mut_ptr();
        }
        let mut process: ProcessInfo = unsafe { std::mem::zeroed() };
        let job = owned(
            unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) },
            "CreateJobObjectW",
        );
        let mut limits = JobLimits::default();
        limits.basic.flags = 0x2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE; only this fixture family.
        checked(
            unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    9,
                    (&mut limits as *mut JobLimits).cast(),
                    std::mem::size_of::<JobLimits>() as u32,
                )
            },
            "owned job limits",
        );
        let launched = unsafe {
            CreateProcessW(
                application.as_ptr(),
                native_command.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                0x0008_0004,
                std::ptr::null_mut(),
                std::ptr::null(),
                &mut startup.info,
                &mut process,
            )
        };
        if launched == 0 {
            let error = std::io::Error::last_os_error();
            panic!("CreateProcessW(console bootstrap): {error}");
        }
        let thread = owned(process.thread, "bootstrap thread");
        let process = owned(process.process, "bootstrap process");
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) };
        if assigned == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                TerminateProcess(process.as_raw_handle(), 1);
            }
            panic!("required owned job assignment failed: {error}");
        }
        let resumed = unsafe { ResumeThread(thread.as_raw_handle()) };
        assert_ne!(resumed, u32::MAX, "ResumeThread");
        drop(thread);
        drop(attributes);
        drop(input_read);
        drop(output_write);
        let mut session = Self {
            process,
            job,
            console: Some(console),
            input: Some(InputWriter::new(input_write)),
            request,
            response,
            root,
            trace,
            reader: Some(reader),
            reader_done,
        };
        session.wait_response(Instant::now() + LIMIT);
        assert_eq!(
            fs::read(session.root.join("response.bin")).unwrap(),
            b"ready"
        );
        session
    }
    fn wait_response(&mut self, deadline: Instant) {
        let handles = [self.response.as_raw_handle(), self.process.as_raw_handle()];
        let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, millis(deadline)) };
        assert_eq!(
            wait,
            0,
            "console bootstrap IPC failed/timed out ({wait:#x}); trace={:?}",
            String::from_utf8_lossy(&self.trace())
        );
    }
    fn request(&mut self, bytes: &[u8]) -> Vec<u8> {
        fs::write(self.root.join("request.bin"), bytes).unwrap();
        signal(&self.request);
        self.wait_response(Instant::now() + LIMIT);
        fs::read(self.root.join("response.bin")).unwrap()
    }
    pub fn send(&mut self, bytes: &[u8]) {
        self.input
            .as_mut()
            .expect("closed console input")
            .send(bytes);
    }
    pub fn paste(&mut self, text: &str) {
        let enabled = self
            .trace
            .0
            .lock()
            .unwrap()
            .protocol
            .expect("paste protocol state has not been observed");
        if enabled {
            self.send(b"\x1b[200~");
        }
        self.send(text.as_bytes());
        if enabled {
            self.send(b"\x1b[201~");
        }
    }
    /// Raw observed VT bytes, including queries, for deterministic fragmented responses.
    pub fn trace(&self) -> Vec<u8> {
        self.trace.0.lock().unwrap().bytes.clone()
    }
    fn wait_trace(&mut self, predicate: impl Fn(&Trace) -> bool, description: &str) {
        let deadline = Instant::now() + LIMIT;
        let (lock, changed) = &*self.trace;
        let mut state = lock.lock().unwrap();
        loop {
            assert!(
                state.error.is_none(),
                "console drain failed: {:?}",
                state.error
            );
            if predicate(&state) {
                return;
            }
            assert!(
                !state.eof,
                "console EOF waiting for {description}; trace={:?}",
                String::from_utf8_lossy(&state.bytes)
            );
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| {
                    panic!(
                        "console deadline waiting for {description}; trace={:?}",
                        String::from_utf8_lossy(&state.bytes)
                    )
                });
            let (next, _) = changed.wait_timeout(state, remaining).unwrap();
            state = next;
        }
    }
    pub fn wait_text(&mut self, needle: &[u8]) {
        assert!(!needle.is_empty());
        self.wait_trace(
            |state| state.bytes.windows(needle.len()).any(|part| part == needle),
            &format!("VT bytes {needle:?}"),
        );
    }
    pub fn wait_protocol(&mut self, enabled: bool) {
        self.wait_trace(
            |state| state.protocol == Some(enabled),
            &format!("bracketed paste enabled={enabled}"),
        );
    }
    pub fn snapshot(&mut self) -> Snapshot {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let bytes = self.request(&[1]);
        fs::write(
            self.root.join(format!(
                "snapshot-{}.bin",
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            &bytes,
        )
        .unwrap();
        Snapshot::decode(&bytes)
    }
    /// Observe native record consumption without reading input or sleeping.
    pub fn wait_input_drained(&mut self) {
        let deadline = Instant::now() + LIMIT;
        loop {
            let reply = self.request(&[4]);
            assert_eq!(reply.len(), 4);
            if u32::from_le_bytes(reply.try_into().unwrap()) == 0 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "native console input did not drain"
            );
            std::thread::yield_now();
        }
    }
    pub fn resize(&mut self, cols: i16, rows: i16) {
        assert!(cols > 0 && rows > 0);
        let result = unsafe {
            ResizePseudoConsole(
                self.console.as_ref().expect("closed ConPTY").handle(),
                Coord { x: cols, y: rows },
            )
        };
        assert_eq!(result, 0, "ResizePseudoConsole: {result:#x}");
    }
    /// Sends a real control event to this bootstrap's owned console, never the host console.
    pub fn control(&mut self, kind: u32) {
        assert!(
            matches!(kind, 0 | 1),
            "only CTRL_C_EVENT/CTRL_BREAK_EVENT are fixture inputs"
        );
        let mut payload = vec![2];
        put_u32(&mut payload, kind);
        assert_eq!(self.request(&payload), b"ok");
    }
    pub fn key(
        &mut self,
        down: bool,
        repeat: u16,
        vk: u16,
        scan: u16,
        unicode: u16,
        controls: u32,
    ) {
        self.key_records(&[(down, repeat, vk, scan, unicode, controls)]);
    }
    /// Submit a complete native sequence without IPC latency becoming an Escape
    /// timeout. Console/ConPTY still performs its real KEY_EVENT translation.
    pub fn key_records(&mut self, records: &[(bool, u16, u16, u16, u16, u32)]) {
        let mut payload = vec![3];
        for &(down, repeat, vk, scan, unicode, controls) in records {
            for value in [
                u32::from(down),
                repeat as u32,
                vk as u32,
                scan as u32,
                unicode as u32,
                controls,
            ] {
                put_u32(&mut payload, value);
            }
        }
        assert_eq!(self.request(&payload), b"ok");
    }
    /// Wait for a closed evidence file; directory change notification is armed before checking.
    pub fn wait_file(&mut self, path: &Path) -> Vec<u8> {
        let parent = path.parent().expect("evidence file parent");
        let notification = unsafe {
            FindFirstChangeNotificationW(wide(parent.as_os_str()).as_ptr(), 0, 0x1 | 0x8 | 0x10)
        };
        assert!(
            notification as usize != usize::MAX && !notification.is_null(),
            "FindFirstChangeNotificationW: {}",
            std::io::Error::last_os_error()
        );
        struct Notification(Handle);
        impl Drop for Notification {
            fn drop(&mut self) {
                unsafe {
                    FindCloseChangeNotification(self.0);
                }
            }
        }
        let notification = Notification(notification);
        let deadline = Instant::now() + LIMIT;
        let mut exited = false;
        loop {
            // Disallow a concurrent writer, so existence alone cannot accept a partial file.
            let handle = unsafe {
                CreateFileW(
                    wide(path.as_os_str()).as_ptr(),
                    0x8000_0000,
                    1,
                    std::ptr::null_mut(),
                    3,
                    0x80,
                    std::ptr::null_mut(),
                )
            };
            if handle as usize != usize::MAX {
                let mut bytes = Vec::new();
                File::from(owned(handle, "closed evidence file"))
                    .read_to_end(&mut bytes)
                    .unwrap();
                return bytes;
            }
            let error = std::io::Error::last_os_error();
            assert!(
                matches!(error.raw_os_error(), Some(2 | 3 | 32 | 33)),
                "read evidence {}: {error}",
                path.display()
            );
            assert!(
                !exited,
                "console target exited before publishing {}",
                path.display()
            );
            let handles = [notification.0, self.process.as_raw_handle()];
            let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, millis(deadline)) };
            assert!(
                wait <= 1,
                "evidence wait {} failed/timed out ({wait:#x}); trace={:?}",
                path.display(),
                String::from_utf8_lossy(&self.trace())
            );
            if wait == 1 {
                exited = true;
            } else {
                checked(
                    unsafe { FindNextChangeNotification(notification.0) },
                    "FindNextChangeNotification",
                );
            }
        }
    }
    pub fn finish(mut self, expected_status: u32) -> Transcript {
        let wait =
            unsafe { WaitForSingleObject(self.process.as_raw_handle(), LIMIT.as_millis() as u32) };
        assert_eq!(
            wait,
            0,
            "console target did not exit ({wait:#x}); trace={:?}",
            String::from_utf8_lossy(&self.trace())
        );
        let mut code = 0;
        checked(
            unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) },
            "bootstrap exit code",
        );
        assert_eq!(code, 0, "console bootstrap failed");
        let final_bytes =
            fs::read(self.root.join("final.bin")).expect("post-exit console mode evidence");
        let mut final_data = final_bytes.as_slice();
        assert_eq!(
            get_u32(&mut final_data),
            expected_status,
            "actual selected executable exit status"
        );
        let final_modes = get_modes(&mut final_data);
        assert!(final_data.is_empty());
        let original_bytes = fs::read(self.root.join("original.bin")).unwrap();
        let mut original_data = original_bytes.as_slice();
        let original_modes = get_modes(&mut original_data);
        assert_eq!(
            final_modes, original_modes,
            "fish failed to restore the actual original native console modes"
        );
        // No descendant may retain output after the target/observer exit.
        checked(
            unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) },
            "close owned completed fixture family",
        );
        self.close(true);
        let state = self.trace.0.lock().unwrap();
        assert!(
            state.error.is_none(),
            "console drain failed: {:?}",
            state.error
        );
        assert!(state.eof, "console output did not reach EOF");
        let bytes = state.bytes.clone();
        fs::write(self.root.join("trace.vt"), &bytes).unwrap();
        Transcript {
            bytes,
            original_modes,
            final_modes,
        }
    }
    fn close(&mut self, strict: bool) {
        let mut input = self.input.take();
        if let Some(writer) = input.as_mut() {
            writer.sender.take();
        }
        if let Some(mut console) = self.console.take() {
            console.close(strict);
        }
        if let Some(mut writer) = input {
            writer.close(strict);
        }
        if let Some(reader) = self.reader.take() {
            let result = self.reader_done.recv_timeout(LIMIT);
            if result.is_ok() {
                let _ = reader.join();
            } else if strict {
                panic!("ConPTY output reader exceeded cleanup deadline");
            }
            // A failed cleanup detaches rather than hanging the test runner.
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            TerminateJobObject(self.job.as_raw_handle(), 1);
        }
        self.close(false);
        if let Ok(state) = self.trace.0.lock() {
            let _ = fs::write(self.root.join("trace.vt"), &state.bytes);
        }
    }
}
fn close_console(console: Handle, strict: bool) {
    // ClosePseudoConsole can synchronously write output. The independent drain is
    // kept alive, and even a broken host cannot indefinitely block harness Drop.
    let value = console as usize;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        unsafe {
            ClosePseudoConsole(value as Handle);
        }
        let _ = send.send(());
    });
    let result = receive.recv_timeout(LIMIT);
    if strict {
        result.expect("ClosePseudoConsole exceeded bounded cleanup");
    }
}
fn quote_command(args: &[&OsStr]) -> Vec<u16> {
    let mut out = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        if index != 0 {
            out.push(b' ' as u16);
        }
        out.push(b'"' as u16);
        let mut slashes = 0;
        for unit in arg.encode_wide() {
            assert_ne!(unit, 0);
            if unit == b'\\' as u16 {
                slashes += 1;
                continue;
            }
            if unit == b'"' as u16 {
                out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2 + 1));
            } else {
                out.extend(std::iter::repeat_n(b'\\' as u16, slashes));
            }
            slashes = 0;
            out.push(unit);
        }
        out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
        out.push(b'"' as u16);
    }
    out.push(0);
    out
}
unsafe extern "system" fn ignore_control(kind: u32) -> i32 {
    i32::from(matches!(kind, 0 | 1))
}

fn assert_private_station(expected: &OsStr, root: &Path) {
    let station = unsafe { GetProcessWindowStation() };
    assert!(!station.is_null(), "GetProcessWindowStation");
    let mut needed = 0;
    unsafe {
        GetUserObjectInformationW(station, 2, std::ptr::null_mut(), 0, &mut needed);
    }
    assert!(
        needed > 0 && needed <= 64 * 1024,
        "window station name unavailable"
    );
    let mut name = vec![0u16; (needed as usize).div_ceil(2)];
    checked(
        unsafe {
            GetUserObjectInformationW(station, 2, name.as_mut_ptr().cast(), needed, &mut needed)
        },
        "private window station identity",
    );
    let length = name
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(name.len());
    let actual = OsString::from_wide(&name[..length]);
    assert!(
        !actual.to_string_lossy().eq_ignore_ascii_case("WinSta0"),
        "clipboard acceptance cannot access the user's station"
    );
    assert_eq!(
        actual, expected,
        "console bootstrap did not inherit the isolated private clipboard station"
    );
    fs::write(
        root.join("station.txt"),
        actual.to_string_lossy().as_bytes(),
    )
    .unwrap();
}

fn private_desktop(command: &Command) -> Option<Vec<u16>> {
    command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("FISH_N6_PRIVATE_DESKTOP"))
        .and_then(|(_, value)| value)
        .map(wide)
}

/// Console handles and the desktop are explicit. A null lpDesktop does not
/// inherit an arbitrary caller station; never fall back to the user's clipboard.
fn spawn_console_target(command: &Command, input: &File, output: &File) -> OwnedHandle {
    let input = input.try_clone().unwrap();
    let output = output.try_clone().unwrap();
    for handle in [input.as_raw_handle(), output.as_raw_handle()] {
        checked(
            unsafe { SetHandleInformation(handle, 1, 1) },
            "inheritable console clone",
        );
    }
    let handles = [input.as_raw_handle(), output.as_raw_handle()];
    let mut required = 0;
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut required);
    }
    assert!(required > 0);
    let mut attributes = Attributes(vec![0; required.div_ceil(std::mem::size_of::<usize>())]);
    checked(
        unsafe { InitializeProcThreadAttributeList(attributes.pointer(), 1, 0, &mut required) },
        "target attribute list",
    );
    checked(
        unsafe {
            UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                0x0002_0002,
                handles.as_ptr().cast_mut().cast(),
                std::mem::size_of_val(&handles),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        "target console handle allowlist",
    );
    let application = wide(command.get_program());
    let arguments: Vec<&OsStr> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .collect();
    let mut line = quote_command(&arguments);
    let cwd = command.get_current_dir().map(|path| wide(path.as_os_str()));
    let mut environment = Vec::new();
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            environment.extend(key.encode_wide());
            environment.push(b'=' as u16);
            environment.extend(value.encode_wide());
            environment.push(0);
        }
    }
    if environment.is_empty() {
        environment.push(0);
    }
    environment.push(0);
    let mut desktop = private_desktop(command);
    let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
    startup.info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
    // A leaf may allocate its own classic console to exercise scrollback.
    // Keep that owned fixture hidden; ordinary ConPTY clients have no window.
    startup.info.flags = 0x100 | 0x1; // USESTDHANDLES | USESHOWWINDOW (SW_HIDE)
    startup.info.stdin = input.as_raw_handle();
    startup.info.stdout = output.as_raw_handle();
    startup.info.stderr = output.as_raw_handle();
    if let Some(desktop) = &mut desktop {
        startup.info.desktop = desktop.as_mut_ptr();
    }
    startup.attributes = attributes.pointer();
    let mut process: ProcessInfo = unsafe { std::mem::zeroed() };
    checked(
        unsafe {
            CreateProcessW(
                application.as_ptr(),
                line.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                0x0008_0400,
                environment.as_mut_ptr().cast(),
                cwd.as_ref().map_or(std::ptr::null(), |path| path.as_ptr()),
                &mut startup.info,
                &mut process,
            )
        },
        "launch actual selected console executable",
    );
    drop(owned(process.thread, "target primary thread"));
    owned(process.process, "target process")
}

/// Probe router entry point. No fixture paths or Cargo artifact constants are assumed.
pub fn run(mode: &str, args: Vec<OsString>) -> ! {
    assert_eq!(mode, "n6-console-bootstrap", "unknown console fixture mode");
    assert_eq!(args.len(), 3);
    let root = PathBuf::from(&args[0]);
    let request = event(&args[1], false);
    let response = event(&args[2], false);
    let input = OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .expect("required real client CONIN$");
    let output = OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
        .expect("required real client CONOUT$");
    let original = read_modes(&input, &output);
    let mut original_data = Vec::new();
    put_modes(&mut original_data, original);
    fs::write(root.join("original.bin"), original_data).unwrap();
    checked(
        unsafe { SetConsoleCtrlHandler(Some(ignore_control), 1) },
        "bootstrap console control observer",
    );
    let config = fs::read(root.join("launch.bin")).unwrap();
    let mut config = config.as_slice();
    let program = get_os(&mut config);
    let mut command = Command::new(program);
    for _ in 0..get_u32(&mut config) {
        command.arg(get_os(&mut config));
    }
    command.current_dir(get_os(&mut config)).env_clear();
    for _ in 0..get_u32(&mut config) {
        let key = get_os(&mut config);
        let value = get_os(&mut config);
        if key == OsStr::new("FISH_N6_PRIVATE_STATION") {
            assert_private_station(&value, &root);
        }
        command.env(key, value);
    }
    assert!(config.is_empty());
    // A ConPTY client's initial standard handles may be pipes/invalid handles.
    // Explicit console-device handles are mandatory; there is no pipe-as-tty fallback.
    let child = spawn_console_target(&command, &input, &output);
    fs::write(root.join("response.bin"), b"ready").unwrap();
    signal(&response);
    loop {
        let handles = [request.as_raw_handle(), child.as_raw_handle()];
        let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, 0xffff_ffff) };
        if wait == 1 {
            break;
        }
        assert_eq!(wait, 0, "console bootstrap request wait");
        let payload = fs::read(root.join("request.bin")).unwrap();
        let mut body = &payload[1..];
        let reply = match payload[0] {
            1 => {
                assert!(body.is_empty());
                Snapshot::capture(&input, &output).encode()
            }
            2 => {
                let kind = get_u32(&mut body);
                assert!(matches!(kind, 0 | 1));
                assert!(body.is_empty());
                checked(
                    unsafe { GenerateConsoleCtrlEvent(kind, 0) },
                    "owned-console GenerateConsoleCtrlEvent",
                );
                b"ok".to_vec()
            }
            3 => {
                let mut records = Vec::new();
                while !body.is_empty() {
                    let down = get_u32(&mut body);
                    let repeat = get_u32(&mut body) as u16;
                    let vk = get_u32(&mut body) as u16;
                    let scan = get_u32(&mut body) as u16;
                    let unicode = get_u32(&mut body) as u16;
                    let controls = get_u32(&mut body);
                    assert!(down <= 1 && repeat > 0);
                    assert!(records.len() + usize::from(repeat) <= 65535);
                    let record = InputRecord {
                        kind: 1,
                        padding: 0,
                        key: KeyEvent {
                            down: down as i32,
                            repeat: 1,
                            vk,
                            scan,
                            unicode,
                            controls,
                        },
                    };
                    // ConPTY normalizes RepeatCount to one while encoding VT.
                    // Emit every repetition, as a real frontend must. Native
                    // decoder units separately cover retained repeat metadata.
                    records.extend(std::iter::repeat_n(record, usize::from(repeat)));
                }
                assert!(!records.is_empty());
                let mut written = 0;
                checked(
                    unsafe {
                        WriteConsoleInputW(
                            input.as_raw_handle(),
                            records.as_ptr(),
                            records.len() as u32,
                            &mut written,
                        )
                    },
                    "WriteConsoleInputW(atomic native record sequence)",
                );
                assert_eq!(written as usize, records.len());
                b"ok".to_vec()
            }
            4 => {
                assert!(body.is_empty());
                let mut count = 0;
                checked(
                    unsafe { GetNumberOfConsoleInputEvents(input.as_raw_handle(), &mut count) },
                    "GetNumberOfConsoleInputEvents",
                );
                count.to_le_bytes().to_vec()
            }
            other => panic!("unknown owned console IPC request {other}"),
        };
        fs::write(root.join("response.bin"), reply).unwrap();
        signal(&response);
    }
    let mut status = 0;
    checked(
        unsafe { GetExitCodeProcess(child.as_raw_handle(), &mut status) },
        "actual target exit code",
    );
    let mut final_data = Vec::new();
    put_u32(&mut final_data, status);
    put_modes(&mut final_data, read_modes(&input, &output));
    fs::write(root.join("final.bin"), final_data).unwrap();
    std::process::exit(0)
}
