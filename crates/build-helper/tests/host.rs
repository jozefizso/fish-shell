//! W1-HOST executes the same production version path used by root build.rs.
use fish_build_helper::version::{acquire_version, VersionSource};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PACKAGE_VERSION: &str = "4.9.3";
const CHILD_FIXTURE: &str = "FISH_W1_HOST_FIXTURE";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "fish native archive 日本語 🐟 {} {nonce} {attempt}", std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("create archive fixture: {err}"),
            }
        }
        panic!("unable to create isolated archive fixture");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove archive fixture");
    }
}

#[test]
fn w1_host() {
    if let Some(source_dir) = std::env::var_os(CHILD_FIXTURE) {
        let source_dir = PathBuf::from(source_dir);
        // Verify both this fixture and the deliberately tool-free child environment.
        assert!(!source_dir.join(".git").exists());
        for tool in ["sh", "awk", "git"] {
            assert!(Command::new(tool).output().is_err(), "unexpected host tool: {tool}");
        }
        let version = acquire_version(&source_dir, PACKAGE_VERSION).unwrap();
        assert_eq!(version.source, VersionSource::Package);
        assert_eq!(version.value, PACKAGE_VERSION);
        fs::write(source_dir.join("actual-version"), version.value).unwrap();
        return;
    }

    let fixture = Fixture::new();
    let empty_path = fixture.0.join("empty tools");
    fs::create_dir(&empty_path).unwrap();
    fs::write(fixture.0.join("Cargo.toml"), format!(
        "[package]\nname = \"fish\"\nversion = \"{PACKAGE_VERSION}\"\n"
    )).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "w1_host", "--nocapture"])
        .current_dir(&fixture.0)
        .env_clear()
        .env("PATH", &empty_path)
        .env(CHILD_FIXTURE, &fixture.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Windows system DLL discovery still needs the OS directory, not a tool PATH.
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    let mut child = command.spawn().expect("start native build-support test child");
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate timed-out test child");
            child.wait().expect("reap timed-out test child");
            break None;
        }
        thread::yield_now();
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert!(status.is_some_and(|status| status.success()),
        "native helper failed or timed out: {status:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stdout), String::from_utf8_lossy(&stderr));
    assert_eq!(fs::read_to_string(fixture.0.join("actual-version")).unwrap(), PACKAGE_VERSION);
}
