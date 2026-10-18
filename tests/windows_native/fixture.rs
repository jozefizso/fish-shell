//! Source-independent owned native processes shared by acceptance suites.
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

pub(super) const LIMIT: Duration = Duration::from_secs(30);
#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32;
}
pub(super) struct Captured {
    pub code: u32,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
}
pub(super) fn run(command: &mut Command, input: &[u8]) -> Captured {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {command:?}: {e}"));
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    let (send, receive) = mpsc::channel();
    let sender = send.clone();
    std::thread::spawn(move || {
        let result = stdin.write_all(&input);
        drop(stdin);
        let _ = sender.send((0, result.map(|_| Vec::new())));
    });
    for (index, mut stream) in [
        (
            1,
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        ),
        (
            2,
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
        ),
    ] {
        let sender = send.clone();
        std::thread::spawn(move || {
            let mut data = Vec::new();
            let result = stream.read_to_end(&mut data).map(|_| data);
            let _ = sender.send((index, result));
        });
    }
    drop(send);
    let wait = unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) };
    if wait != 0 {
        let _ = child.kill();
        let _ = child.wait();
        panic!("bounded process wait failed ({wait:#x}): {command:?}");
    }
    let status = child.wait().unwrap();
    let mut streams = BTreeMap::new();
    for _ in 0..3 {
        let (index, result) = receive
            .recv_timeout(LIMIT)
            .expect("child-owned streams did not reach EOF within bound");
        streams.insert(index, result.expect("native stream I/O failed"));
    }
    Captured {
        code: status.code().expect("native exit code") as u32,
        out: streams.remove(&1).unwrap(),
        err: streams.remove(&2).unwrap(),
    }
}

pub(super) struct Fixture {
    pub root: PathBuf,
    pub fish: PathBuf,
    pub probe: PathBuf,
    pub state: PathBuf,
}
impl Fixture {
    pub fn new(fish: &Path, probe: &Path, profile: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fish native 雪 🐟 {} {} {profile}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fish_copy = root.join("fish.exe");
        let probe_copy = root.join("native probe 雪.exe");
        fs::copy(fish, &fish_copy)
            .unwrap_or_else(|e| panic!("required {profile} fish artifact {}: {e}", fish.display()));
        fs::copy(probe, &probe_copy).expect("required native probe artifact");
        let state = root.join("user state");
        fs::create_dir(&state).unwrap();
        for dir in ["home", "config", "data", "cache"] {
            fs::create_dir(state.join(dir)).unwrap();
        }
        Self {
            root,
            fish: fish_copy,
            probe: probe_copy,
            state,
        }
    }
    pub fn command(&self, args: &[&OsStr]) -> Command {
        let mut cmd = Command::new(&self.fish);
        cmd.args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", "")
            // Formatting cases declare their locale instead of pinning the
            // workstation's current numeric defaults. N4 explicitly unsets this
            // precondition when testing restoration of actual OS defaults.
            .env("LANG", "C");
        if let Some(value) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", value);
        }
        for (key, dir) in [
            ("HOME", "home"),
            ("USERPROFILE", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("APPDATA", "config"),
            ("XDG_DATA_HOME", "data"),
            ("LOCALAPPDATA", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ] {
            cmd.env(key, self.state.join(dir));
        }
        cmd
    }
    pub fn eval(&self, code: &str) -> Captured {
        self.input(code, b"")
    }
    pub fn input(&self, code: &str, bytes: &[u8]) -> Captured {
        run(
            &mut self.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]),
            bytes,
        )
    }
    pub fn probe(&self) -> String {
        quote(self.probe.to_str().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub(super) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
#[track_caller]
pub(super) fn exact(result: Captured, code: u32, out: &[u8], err: &[u8]) {
    assert_eq!(
        result.code,
        code,
        "stderr={}",
        String::from_utf8_lossy(&result.err)
    );
    assert_eq!(result.out, out);
    assert_eq!(result.err, err);
}
pub(super) fn artifacts() -> [(String, PathBuf); 2] {
    let selected = std::env::var_os("FISH_NATIVE_TEST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_fish")));
    let release = std::env::var_os("FISH_NATIVE_TEST_RELEASE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            selected
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("release/fish.exe")
        });
    for path in [&selected, &release] {
        assert!(
            path.is_file(),
            "required native artifact missing: {}",
            path.display()
        );
    }
    [("debug".into(), selected), ("release".into(), release)]
}
