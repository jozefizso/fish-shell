use super::{Family, fixture::{Fixture, run}};
use std::{fs, path::PathBuf, process::Command};

pub(super) const FAMILIES: &[Family] = &[
    ("native-python-floor-discovery-and-absence", interpreter),
    ("real-manpage-foreground-plain-compressed-corrupt-and-cache", manpages),
    ("native-background-manpage-generation-and-cleanup", background),
    ("live-browser-authenticated-theme-function-edits-and-loopback", web_edits),
    ("public-web-tool-normal-eof-failed-and-cancelled-lifetime", web_lifetime),
];
fn probe(f: &Fixture, mode: &str) {
    let python = PathBuf::from(std::env::var_os("FISH_N9_PYTHON").expect("required native Python 3.8+ fixture FISH_N9_PYTHON"));
    assert!(python.is_file(), "required native Python {}", python.display());
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = source.join("tests/windows_n9/tools_probe.py");
    let mut command = Command::new(python);
    command.arg("-B").arg(script).arg(mode).arg(&f.root).arg(&f.fish).arg(&source)
        .current_dir(&f.root).env_clear();
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(key, value); }
    }
    for key in ["FISH_N9_OLD_PYTHON", "FISH_N9_BROWSER"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
    let temporary = f.root.join("optional tool private temp 雪");
    fs::create_dir_all(&temporary).unwrap();
    command.env("TEMP", &temporary).env("TMP", &temporary).env("PYTHONUTF8", "1");
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "N9 {mode}:\n{}\n{}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    assert!(!fs::read_dir(&temporary).unwrap().any(|entry| entry.unwrap().file_name().to_string_lossy().starts_with("fish-webconfig-")), "web redirect directory leaked");
}
fn interpreter(f: &Fixture) { probe(f, "interpreter"); }
fn manpages(f: &Fixture) { probe(f, "manpages"); }
fn background(f: &Fixture) { probe(f, "background"); }
fn web_edits(f: &Fixture) { probe(f, "web-edits"); }
fn web_lifetime(f: &Fixture) { probe(f, "web-lifetime"); }
