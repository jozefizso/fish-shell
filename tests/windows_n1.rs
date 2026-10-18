//! Required N1 gates. Fixtures are owned, source-independent and never silently skipped.
#![cfg(windows)]
use fish_widestring::{WString, native_to_shell_text, osstr2wcstring};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
#[path = "windows_native/fixture.rs"]
mod fixture;
use fixture::{Captured, Fixture, exact, quote, run};
#[path = "windows_n1/discovery.rs"]
mod discovery;
#[path = "windows_n1/glob.rs"]
mod glob;
#[path = "windows_n1/native.rs"]
mod native;
#[path = "windows_n1/paths.rs"]
mod paths;

const REQUIRED: &[&str] = &["N1-PATHS", "N1-DISCOVERY", "N1-GLOB"];
fn path_text(path: &Path) -> WString {
    native_to_shell_text(&osstr2wcstring(path).expect("fixture Unicode"))
}
fn qpath(path: &Path) -> String {
    quote(path.to_str().expect("fixture Unicode"))
}
fn success(result: Captured) -> Vec<u8> {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(
        result.err.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.err)
    );
    result.out
}
fn failed(result: Captured, diagnostic: bool) {
    assert_ne!(result.code, 0, "operation unexpectedly succeeded");
    assert!(
        result.out.is_empty(),
        "failed operation emitted a value: {:?}",
        result.out
    );
    if diagnostic {
        assert!(!result.err.is_empty(), "missing native diagnostic");
    }
}
fn same_path_output(result: Captured, expected: &Path) {
    let bytes = success(result);
    let text = std::str::from_utf8(&bytes)
        .unwrap()
        .strip_suffix('\n')
        .unwrap();
    let actual = fish::wutil::native_file_identity(&path_text(Path::new(text)), true).unwrap();
    let expected = fish::wutil::native_file_identity(&path_text(expected), true).unwrap();
    assert_eq!(actual, expected, "reported path identifies another object");
}
fn gate(id: &str, case: fn(&Fixture)) {
    assert!(REQUIRED.contains(&id));
    let artifacts = fixture::artifacts();
    let mut executed = 0;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (profile, fish) in artifacts {
        let f = Fixture::new(
            &fish,
            Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
            &format!("{profile}-{id}"),
        );
        eprintln!("RUN {profile} {id}");
        case(&f);
        executed += 1;
        eprintln!("PASS {profile} {id}");
    }
    assert_eq!(executed, 2, "both native profiles are mandatory");
}
// Stable Rust identifiers encode the exact required IDs; RUN/PASS records retain hyphens.
#[test]
#[allow(non_snake_case)]
fn N1_PATHS() {
    gate("N1-PATHS", paths::exercise);
}
#[test]
#[allow(non_snake_case)]
fn N1_DISCOVERY() {
    gate("N1-DISCOVERY", discovery::exercise);
}
#[test]
#[allow(non_snake_case)]
fn N1_GLOB() {
    gate("N1-GLOB", glob::exercise);
}
