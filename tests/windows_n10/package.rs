use super::fixture::{Captured, run};
use std::fs::{self, OpenOptions};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct PackageFixture {
    root: PathBuf,
    archive: PathBuf,
    state: PathBuf,
}
impl PackageFixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let archive = required("FISH_N10_ARCHIVE");
        assert!(archive.is_file(), "N10 requires the real packaged ZIP");
        let root = std::env::temp_dir().join(format!(
            "fish N10 package 雪 🐟 {} {}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let state = root.join("independent user state");
        for directory in ["home", "config/fish", "data/fish", "cache/fish"] {
            fs::create_dir_all(state.join(directory)).unwrap();
        }
        for (name, contents) in [
            ("config/fish/config.fish", "set -g n10_preserved preserved\n"),
            ("config/fish/fish_variables", "# VERSION: 3.0\nSETUVAR n10_universal:preserved\n"),
            ("data/fish/fish_history", "- cmd: echo N10-preserved\n  when: 1\n"),
            ("home/unrelated.txt", "unrelated profile and terminal entries are not package files\n"),
        ] {
            fs::write(state.join(name), contents).unwrap();
        }
        Self { root, archive, state }
    }
    fn manage(&self, action: &str, archive: Option<&Path>, destination: Option<&Path>) -> Captured {
        let mut command = Command::new(required("FISH_N10_PWSH"));
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("build_tools/native.ps1"))
            .args(["-Action", action]);
        if let Some(path) = archive { command.arg("-Archive").arg(path); }
        if let Some(path) = destination { command.arg("-Destination").arg(path); }
        command.current_dir(&self.root);
        run(&mut command, b"")
    }
    fn install(&self, archive: &Path, destination: &Path) {
        success(self.manage("Install", Some(archive), Some(destination)));
    }
    fn executable(&self, directory: &Path, name: &str) -> Command {
        let mut command = Command::new(directory.join("bin").join(format!("{name}.exe")));
        let system = PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"));
        command.env_clear().current_dir(&self.root)
            .env("SystemRoot", &system)
            .env("PATH", std::env::join_paths([self.root.join("conflicting PATH"), system.join("System32")]).unwrap())
            .env("LANG", "C")
            .env("HOME", self.state.join("home"))
            .env("USERPROFILE", self.state.join("home"))
            .env("XDG_CONFIG_HOME", self.state.join("config"))
            .env("XDG_DATA_HOME", self.state.join("data"))
            .env("XDG_CACHE_HOME", self.state.join("cache"))
            .env("APPDATA", self.state.join("config"))
            .env("LOCALAPPDATA", self.state.join("data"));
        command
    }
    fn state_snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, result: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() { visit(root, &path, result); }
                else { result.push((path.strip_prefix(root).unwrap().to_owned(), fs::read(&path).unwrap())); }
            }
        }
        let mut result = Vec::new();
        visit(&self.state, &self.state, &mut result);
        result.sort();
        result
    }
}
impl Drop for PackageFixture {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
fn required(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(|| panic!("required N10 fixture is absent: {name}"))
}
fn success(result: Captured) -> Captured {
    assert_eq!(result.code, 0, "stdout={} stderr={}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    result
}
fn failure(result: Captured) {
    assert_ne!(result.code, 0, "failure was reported as success");
    assert!(!result.err.is_empty(), "failure must have a diagnostic");
}

pub fn artifact() {
    let fixture = PackageFixture::new();
    let inspect = success(fixture.manage("Inspect", Some(&fixture.archive), None));
    let manifest = String::from_utf8(inspect.out).unwrap();
    for required in ["unsigned", "x86_64-pc-windows-msvc", "bin/fish.exe", "bin/fish_indent.exe", "bin/fish_key_reader.exe", "license.rst", "third-party-notices.json", "build-inputs.json", "source-inputs.json"] {
        assert!(manifest.contains(required), "archive manifest missing {required}");
    }
    let install = fixture.root.join("installed version 雪");
    fixture.install(&fixture.archive, &install);
    let rebuilt_archive = required("FISH_N10_REBUILT_ARCHIVE");
    assert!(rebuilt_archive.is_file() && rebuilt_archive != fixture.archive, "required independent same-input package build");
    let rebuilt = fixture.root.join("rebuilt same inputs");
    fixture.install(&rebuilt_archive, &rebuilt);
    assert_eq!(fs::read(install.join("build-inputs.json")).unwrap(), fs::read(rebuilt.join("build-inputs.json")).unwrap(), "declared reproducible inputs differ between builds");
    success(fixture.manage("Remove", None, Some(&rebuilt)));
    let poison = fixture.root.join("conflicting PATH");
    fs::create_dir(&poison).unwrap();
    // Another real executable named fish earlier on PATH must not select helpers.
    fs::copy(install.join("bin/fish_key_reader.exe"), poison.join("fish.exe")).unwrap();
    fs::copy(install.join("bin/fish_key_reader.exe"), poison.join("fish_indent.exe")).unwrap();
    let mut shell = fixture.executable(&install, "fish");
    shell.args(["-N", "-c", "string match -rq '^(雪|fish)[0-9]+$' fish42; or exit 41; printf '%06.2f\\n' 1.25; string join / $__fish_data_dir $__fish_sysconf_dir"]);
    let result = success(run(&mut shell, b""));
    let output = String::from_utf8(result.out).unwrap();
    assert!(output.starts_with("001.25\n"), "real PCRE2/printf consumers: {output}");
    assert!(output.contains(install.to_str().unwrap()), "resources must be executable-relative: {output}");
    assert!(!output.contains(env!("CARGO_MANIFEST_DIR")), "developer path captured resources");
    for executable in ["fish", "fish_indent", "fish_key_reader"] {
        let mut version = fixture.executable(&install, executable);
        version.arg("--version");
        assert!(!success(run(&mut version, b"")).out.is_empty());
        let mut help = fixture.executable(&install, executable);
        help.arg("--help");
        let output = success(run(&mut help, b""));
        assert!(output.out.len() > 40, "working {executable} help");
    }
    let mut formatter = fixture.executable(&install, "fish_indent");
    let result = success(run(&mut formatter, b"if true;echo packaged;end\n"));
    assert_eq!(result.out, b"if true\n    echo packaged\nend\n");
    let key_evidence = fixture.root.join("installed key reader console");
    fs::create_dir(&key_evidence).unwrap();
    let mut keys = super::console::Session::start(
        &fixture.executable(&install, "fish_key_reader"),
        Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
        &key_evidence,
    );
    keys.wait_text(b"Press a key:");
    keys.send("雪".as_bytes());
    let transcript = keys.finish(0);
    assert_eq!(transcript.original_modes, transcript.final_modes);
    assert!(String::from_utf8_lossy(&transcript.bytes).contains("bind 雪 "));
    let mut dependencies = Command::new(required("FISH_N10_PWSH"));
    dependencies.args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/windows_n10/dependencies.ps1"))
        .arg("-InstallRoot").arg(&install).arg("-StateRoot").arg(&fixture.state);
    let loaded = success(run(&mut dependencies, b""));
    eprintln!("N10-ARTIFACT loaded module paths: {}", String::from_utf8_lossy(&loaded.out));
    let hidden = install.join("share/doc/fish/private hidden user note.txt");
    fs::write(&hidden, b"hidden user contents must survive refusal").unwrap();
    let mut attributes = Command::new(PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot")).join("System32/attrib.exe"));
    attributes.arg("+H").arg(&hidden);
    success(run(&mut attributes, b""));
    assert_ne!(fs::metadata(&hidden).unwrap().file_attributes() & 2, 0, "fixture must really set FILE_ATTRIBUTE_HIDDEN");
    failure(fixture.manage("Remove", None, Some(&install)));
    assert_eq!(fs::read(&hidden).unwrap(), b"hidden user contents must survive refusal");
    fs::remove_file(&hidden).unwrap();
    let before = fixture.state_snapshot();
    let relocated = fixture.root.join("relocated version 🐟");
    fs::rename(&install, &relocated).unwrap();
    let mut relocated_shell = fixture.executable(&relocated, "fish");
    relocated_shell.args(["-N", "-c", "echo relocated"]);
    assert_eq!(success(run(&mut relocated_shell, b"")).out, b"relocated\n");
    success(fixture.manage("Remove", None, Some(&relocated)));
    assert_eq!(before, fixture.state_snapshot());
    eprintln!("N10-ARTIFACT: real ZIP/hash/three entrypoints/regex/printf/help/formatter/relocation/state cases executed; console/catalog evidence belongs to required N9/N7 suites");
}

pub fn install() {
    assert_eq!(std::env::var("FISH_N10_CLEAN_MACHINE").as_deref(), Ok("1"), "N10-INSTALL requires an externally provisioned clean restricted-user Windows fixture, not a developer-host claim");
    let fixture = PackageFixture::new();
    let prior = required("FISH_N10_PRIOR_ARCHIVE");
    assert!(prior.is_file() && prior != fixture.archive, "required prior supported package");
    let denied = required("FISH_N10_DENIED_PARENT");
    assert!(denied.is_dir(), "fixture must supply an existing denied-write parent");
    let before = fixture.state_snapshot();
    let old = fixture.root.join("prior version");
    fixture.install(&prior, &old);
    let current = fixture.root.join("current version 雪");
    fixture.install(&fixture.archive, &current);
    let old_manifest = fs::read_to_string(old.join("package-manifest.json")).unwrap();
    let current_manifest = fs::read_to_string(current.join("package-manifest.json")).unwrap();
    let old_version = old_manifest.lines().find(|line| line.trim_start().starts_with("\"version\":")).expect("prior version field");
    let current_version = current_manifest.lines().find(|line| line.trim_start().starts_with("\"version\":")).expect("current version field");
    assert_ne!(old_version, current_version, "an upgrade requires a different prior supported version, not a renamed current ZIP");
    // Upgrades are side-by-side, and selecting an existing directory is an error.
    failure(fixture.manage("Install", Some(&fixture.archive), Some(&old)));
    failure(fixture.manage("Install", Some(&fixture.archive), Some(&denied.join(format!("fish-n10-{}", std::process::id())))));
    let lock = OpenOptions::new().read(true).share_mode(0).open(current.join("bin/fish.exe")).unwrap();
    failure(fixture.manage("Remove", None, Some(&current)));
    assert!(current.join("bin/fish.exe").exists());
    drop(lock);
    let corrupt = fixture.root.join("corrupt.zip");
    fs::write(&corrupt, b"PK corrupt partial archive").unwrap();
    let failed = fixture.root.join("failed version");
    failure(fixture.manage("Install", Some(&corrupt), Some(&failed)));
    assert!(!failed.exists());
    let formatter = current.join("bin/fish_indent.exe");
    let original = fs::read(&formatter).unwrap();
    fs::write(&formatter, b"corrupted installed executable").unwrap();
    failure(fixture.manage("Remove", None, Some(&current)));
    assert!(current.exists(), "corruption must not delete the package or user data");
    fs::write(&formatter, original).unwrap();
    let partial = fixture.root.join("partial extraction");
    fs::create_dir(&partial).unwrap();
    fs::write(partial.join("unrelated-user-data"), b"must survive").unwrap();
    failure(fixture.manage("Remove", None, Some(&partial)));
    assert_eq!(fs::read(partial.join("unrelated-user-data")).unwrap(), b"must survive");
    // Modified/unrelated files make removal fail rather than deleting user additions.
    fs::write(current.join("unrelated-user-file"), b"preserve me").unwrap();
    failure(fixture.manage("Remove", None, Some(&current)));
    assert_eq!(fs::read(current.join("unrelated-user-file")).unwrap(), b"preserve me");
    fs::remove_file(current.join("unrelated-user-file")).unwrap();
    success(fixture.manage("Remove", None, Some(&old)));
    success(fixture.manage("Remove", None, Some(&current)));
    assert_eq!(before, fixture.state_snapshot());
    fixture.install(&fixture.archive, &current);
    let mut command = fixture.executable(&current, "fish");
    command.args(["-c", "printf '%s/%s\\n' $n10_preserved $n10_universal"]);
    assert_eq!(success(run(&mut command, b"")).out, b"preserved/preserved\n");
    success(fixture.manage("Remove", None, Some(&current)));
    eprintln!("N10-INSTALL: side-by-side upgrade/removal/locked/denied/corrupt/untracked/reinstall-state cases executed on declared clean fixture");
}

pub fn matrix() {
    let fixture = PackageFixture::new();
    let result = success(fixture.manage("Inspect", Some(&fixture.archive), None));
    assert!(!String::from_utf8(result.out).unwrap().contains("package-manifest.json"), "manifest must not hash itself");
    // A real failed archive-validation stage must propagate out of the native entrypoint.
    let invalid = fixture.root.join("invalid input.zip");
    fs::write(&invalid, b"not a ZIP").unwrap();
    failure(fixture.manage("Inspect", Some(&invalid), None));
    let mut outside = Command::new(required("FISH_N10_PWSH"));
    outside.args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/windows_n10/orchestration.ps1"))
        .arg("-SourceRoot").arg(env!("CARGO_MANIFEST_DIR"))
        .arg("-OutsideDirectory").arg(&fixture.root)
        .current_dir(&fixture.root);
    let outside = success(run(&mut outside, b""));
    assert!(String::from_utf8_lossy(&outside.out).contains("N10 outside-workspace native orchestration passed"));
    // Cargo must actually fail and native orchestration must propagate that failure.
    // An unavailable RUSTC_WRAPPER is an ordinary real build-tool launch failure,
    // not a substitute executable or a mocked successful build stage.
    let mut build = Command::new(required("FISH_N10_PWSH"));
    build.args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("build_tools/native.ps1"))
        .args(["-Action", "Build", "-Features", "minimal"])
        .env("RUSTC_WRAPPER", fixture.root.join("absent compiler wrapper.exe"))
        .env("CARGO_TARGET_DIR", fixture.root.join("failing native build"))
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS");
    let failed = run(&mut build, b"");
    assert!(String::from_utf8_lossy(&failed.err).contains("cargo exited with code"), "failure must reach the real Cargo stage: {}", String::from_utf8_lossy(&failed.err));
    failure(failed);
    eprintln!("N10-MATRIX: orchestration failure propagation case; full profile/toolchain/OS/console matrix is release CI evidence, not inferred from this one process");
}
