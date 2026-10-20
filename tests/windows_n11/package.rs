use super::fixture::{Fixture, Captured, run};
use std::{ffi::OsStr, fs, os::windows::io::AsRawHandle, path::{Path, PathBuf}, process::{Command, Stdio}, time::{SystemTime, UNIX_EPOCH}};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32;
}

pub fn required(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(|| panic!("required N11 fixture missing: {name}"))
}
pub fn success(result: Captured) -> Captured {
    assert_eq!(result.code, 0, "stdout={} stderr={}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    result
}
pub struct Installed {
    pub fixture: Fixture,
    pub evidence: PathBuf,
}
impl Installed {
    pub fn new(label: &str) -> Self {
        let archive = required("FISH_N10_ARCHIVE");
        assert!(archive.is_absolute() && archive.is_file(), "N11 requires an actual N10 ZIP");
        let probe = std::env::var_os("FISH_NATIVE_TEST_RELEASE_PROBE").map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_BIN_EXE_windows_native_probe")).parent().unwrap().parent().unwrap().join("release/windows_native_probe.exe")
            });
        let mut fixture = Fixture::new(Path::new(env!("CARGO_BIN_EXE_fish")), &probe, label);
        // Fixture owns state/cleanup; only the installed release image is executed below.
        fs::remove_file(&fixture.fish).unwrap();
        let destination = fixture.root.join("installed release 雪 🐟");
        let evidence = required("FISH_N11_EVIDENCE_DIR").join(format!("{label}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(evidence.parent().unwrap()).unwrap();
        fs::create_dir(&evidence).unwrap();
        let manage = |action: &str| {
            let mut command = Command::new(required("FISH_N10_PWSH"));
            command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("build_tools/native.ps1"))
                .args(["-Action", action, "-Archive"]).arg(&archive);
            if action == "Install" { command.arg("-Destination").arg(&destination); }
            success(run(&mut command, b""))
        };
        fs::write(evidence.join("n10-manifest.json"), manage("Inspect").out).unwrap();
        fs::write(evidence.join("archive-path.txt"), archive.as_os_str().to_str().unwrap()).unwrap();
        let result = manage("Install");
        fs::write(evidence.join("install.stdout"), result.out).unwrap();
        fs::write(evidence.join("install.stderr"), result.err).unwrap();
        fixture.fish = destination.join("bin/fish.exe");
        assert!(fixture.fish.is_file());
        fs::copy(destination.join("build-inputs.json"), evidence.join("build-inputs.json"))
            .expect("N10 installed build-inputs.json is required");
        let installed = Self { fixture, evidence };
        installed.python("tests/windows_n11/evidence.py", &[]);
        installed
    }
    pub fn python(&self, relative: &str, args: &[&OsStr]) {
        let mut command = Command::new(required("FISH_N11_PYTHON"));
        command.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
            .arg("--evidence").arg(&self.evidence).args(args)
            .env("FISH_N11_INSTALLED_BIN", &self.fixture.fish)
            .env("FISH_N11_PROBE_BIN", &self.fixture.probe);
        let stem = Path::new(relative).file_stem().unwrap().to_string_lossy();
        let stdout = fs::File::create(self.evidence.join(format!("{stem}.stdout"))).unwrap();
        let stderr = fs::File::create(self.evidence.join(format!("{stem}.stderr"))).unwrap();
        // The Python consumers establish their own kill-on-close nested Job Object
        // before launching any work; a timeout cannot leave their descendants behind.
        let mut child = command.stdin(Stdio::null()).stdout(stdout).stderr(stderr).spawn().unwrap();
        let waited = unsafe { WaitForSingleObject(child.as_raw_handle(), 1_800_000) };
        if waited != 0 { let _ = child.kill(); let _ = child.wait(); panic!("bounded N11 evidence consumer failed: {relative} ({waited:#x})"); }
        let status = child.wait().unwrap();
        assert!(status.success(), "{relative} failed ({status}); retained stdout/stderr: {}", self.evidence.display());
    }
    pub fn command(&self, binary: &str, args: &[&str]) -> Command {
        let command = self.fixture.command(&[]);
        // Preserve the fixture's argument-array/environment policy for sibling tools.
        let mut selected = Command::new(self.fixture.fish.parent().unwrap().join(format!("{binary}.exe")));
        selected.current_dir(&self.fixture.root).env_clear().args(args);
        for (key, value) in command.get_envs() {
            if let Some(value) = value { selected.env(key, value); }
        }
        // Avoid inheriting the developer TEMP root through native library fallbacks.
        let temp = self.fixture.state.join("temp");
        fs::create_dir_all(&temp).unwrap();
        selected.env("TEMP", &temp).env("TMP", &temp);
        selected
    }
    pub fn record(&self, case: &str) {
        use std::io::Write;
        let mut file = fs::OpenOptions::new().create(true).append(true).open(self.evidence.join("executed-cases.txt")).unwrap();
        writeln!(file, "{case}").unwrap();
        eprintln!("PASS-CASE N11 {case}");
    }
}
