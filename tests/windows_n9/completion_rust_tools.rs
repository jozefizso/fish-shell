use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{ffi::{OsStr, c_void}, fs, io, os::windows::ffi::OsStrExt, path::{Path, PathBuf}, time::{Duration, UNIX_EPOCH}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-ruff-generated-rule-diagnostics-fixes-cache-and-fallback", ruff),
    ("real-native-ruff-same-path-executable-upgrade-ignore-consumer", ruff_upgrade),
    ("real-native-ruff-present-generator-failure", ruff_generator_failure),
    ("real-native-rumdl-generated-diagnostics-and-unicode-fixes", rumdl),
];

fn required(name: &str) -> PathBuf {
    let tool = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("required genuine native tool: {name}")));
    assert!(tool.is_absolute() && tool.is_file(), "required absolute native tool: {}", tool.display());
    tool
}
fn evaluate(f: &Fixture, tool: &Path, cache: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, tool);
    command.env("XDG_CACHE_HOME", cache);
    run(&mut command, b"")
}
fn capture(f: &Fixture, name: &str, result: &Captured) {
    fs::write(f.root.join(format!("{name}.stdout")), &result.out).unwrap();
    fs::write(f.root.join(format!("{name}.stderr")), &result.err).unwrap();
    fs::write(f.root.join(format!("{name}.status")), result.code.to_string()).unwrap();
}
fn status(result: &Captured, expected: u32) {
    assert_eq!(result.code, expected, "{}", String::from_utf8_lossy(&result.err));
}
fn source(tool: &str) -> String {
    format!("status get-file completions/{tool}.fish | source; or exit\n")
}
const PICK: &str = r#"
function __n9_pick --argument-names context expected
    set -l values (complete -C "$context" | string split -f 1 \t | string match -- "$expected")
    test (count $values) -eq 1; or exit 81
    printf '%s\n' $values
end
"#;
const RUFF_SELECT: &str = r#"
set -l check (__n9_pick 'ruff ch' check)
set -l isolated (__n9_pick 'ruff check --isol' --isolated)
set -l no_cache (__n9_pick 'ruff check --no-cach' --no-cache)
set -l select (__n9_pick 'ruff check --sel' --select)
set -l rule (__n9_pick 'ruff check --select F401' F401)
set -l output (__n9_pick 'ruff check --output-f' --output-format)
set -l format (__n9_pick 'ruff check --output-format conc' concise)
"#;
const RUMDL_SELECT: &str = r#"
set -l check (__n9_pick 'rumdl ch' check)
set -l isolated (__n9_pick 'rumdl check --no-conf' --no-config)
set -l no_cache (__n9_pick 'rumdl check --no-cach' --no-cache)
set -l select (__n9_pick 'rumdl check --ena' --enable)
set -l output (__n9_pick 'rumdl check --output-f' --output-format)
set -l format (__n9_pick 'rumdl check --output-format conc' concise)
"#;

// Parse the selected concise format's filename, line, column and rule fields;
// summaries and diagnostic prose are deliberately not acceptance criteria.
fn diagnostics(result: &Captured, filename: &str) -> Vec<(u32, u32, String)> {
    let text = std::str::from_utf8(&result.out).unwrap();
    let prefix = format!("{filename}:");
    text.lines().filter_map(|line| {
        let rest = line.strip_prefix(&prefix)?;
        let mut fields = rest.splitn(3, ':');
        let row = fields.next().unwrap().parse().unwrap();
        let column = fields.next().expect("diagnostic column").parse().unwrap();
        let rule = fields.next().expect("diagnostic rule").split_whitespace().next().unwrap();
        Some((row, column, rule.trim_matches(['[', ']']).to_owned()))
    }).collect()
}
fn negatives(f: &Fixture, tool: &Path, cache: &Path, name: &str) {
    for (context, forbidden) in [
        (format!("{name} --fi"), "--fix"),
        (format!("{name} rule --fi"), "--fix"),
        (format!("{name} rule --output-format conc"), "concise"),
    ] {
        let result = evaluate(f, tool, cache, &format!("{}complete -C {}", source(name), quote(&context)));
        status(&result, 0);
        assert!(!std::str::from_utf8(&result.out).unwrap().lines()
            .any(|line| line.split('\t').next() == Some(forbidden)), "wrong subcommand context: {context}");
    }
    // Fixture::eval has an empty PATH, not a renamed producer or a fake CLI.
    let absent = f.eval(&format!("{}complete -C '{name} check --fi'", source(name)));
    status(&absent, 0);
    assert!(absent.err.is_empty(), "missing producer must be quiet: {}", String::from_utf8_lossy(&absent.err));
    // Unknown-command filesystem fallback is valid; only generated semantics
    // must disappear when the real producer cannot be resolved.
    assert!(!std::str::from_utf8(&absent.out).unwrap().lines().any(|line|
        matches!(line.split('\t').next(), Some("--fix" | "--fixable" | "--fix-only"))),
        "absent producer must not register generated options");
    let absent_command = f.eval(&format!("command {name} --version"));
    assert_eq!(absent_command.code, 127);
    assert!(absent_command.out.is_empty());
}
fn ruff_consumers(f: &Fixture, tool: &Path, cache: &Path, label: &str) {
    let filename = "ruff lint 雪 🐟.py";
    let original = "import os\r\nmessage = \"雪 🐟\"\r\nprint(message)\r\n";
    let fixed = "message = \"雪 🐟\"\r\nprint(message)\r\n";
    fs::write(f.root.join(filename), original).unwrap();
    let setup = format!("{}{PICK}{RUFF_SELECT}", source("ruff"));
    let invocation = format!("command ruff $check $isolated $no_cache $select $rule $output $format {}", quote(filename));
    let lint = evaluate(f, tool, cache, &format!("{setup}{invocation}"));
    capture(f, &format!("ruff-{label}-lint"), &lint);
    status(&lint, 1);
    assert_eq!(diagnostics(&lint, filename), [(1, 8, "F401".to_owned())]);
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), original.as_bytes());

    // A different completion-selected rule must not diagnose the unused import.
    let other_rule = evaluate(f, tool, cache, &format!("{setup}set rule (__n9_pick 'ruff check --select F841' F841)\n{invocation}"));
    status(&other_rule, 0);
    assert!(diagnostics(&other_rule, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), original.as_bytes());

    let fix = evaluate(f, tool, cache, &format!("{setup}set -l fix (__n9_pick 'ruff check --fi' --fix)\n{invocation} $fix"));
    capture(f, &format!("ruff-{label}-fix"), &fix);
    status(&fix, 0);
    assert!(diagnostics(&fix, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), fixed.as_bytes(), "only the unused import is removed; Unicode and CRLF survive");
    let clean = evaluate(f, tool, cache, &format!("{setup}{invocation}"));
    status(&clean, 0);
    assert!(diagnostics(&clean, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), fixed.as_bytes());
}
fn ruff(f: &Fixture) {
    let tool = required("FISH_N9_RUFF");
    let cache = f.state.join("ruff writable cache 雪");
    fs::create_dir(&cache).unwrap();
    ruff_consumers(f, &tool, &cache, "writable-cache");
    negatives(f, &tool, &cache, "ruff");

    // Restrict only the owned completion-cache leaf. Denying the entire XDG
    // cache root prevents native fish state initialization before Ruff can run.
    let unavailable = f.state.join("ruff unavailable cache 雪");
    let leaf = unavailable.join("fish/completions");
    fs::create_dir_all(&leaf).unwrap();
    let denied = DeniedCache::new(&leaf);
    assert_eq!(fs::write(leaf.join("must-not-write"), b"denied").unwrap_err().raw_os_error(), Some(5));
    ruff_consumers(f, &tool, &unavailable, "unavailable-cache-fallback");
    assert!(!leaf.join("ruff.fish").exists() && !leaf.join("ruff.stamp").exists(), "real generator/consumer works without publishing a cache");
    drop(denied);
    fs::write(leaf.join("restored-write"), b"restored").unwrap();
}

fn ruff_upgrade(f: &Fixture) {
    let previous = required("FISH_N9_RUFF_PREVIOUS");
    let current = required("FISH_N9_RUFF");
    let bin = f.state.join("ruff upgrade executable 雪");
    let cache = f.state.join("ruff upgrade shared cache 雪");
    fs::create_dir(&bin).unwrap();
    fs::create_dir(&cache).unwrap();
    let tool = bin.join("ruff.exe");
    fs::copy(&previous, &tool).unwrap();
    // Model two installs with distinct executable modification times, without
    // sleeping or inspecting cache contents, timestamps or identity stamps.
    fs::OpenOptions::new().write(true).open(&tool).unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000)).unwrap();
    let filename = "ruff upgrade ignore 雪 🐟.py";
    let original = "import os\r\nmessage = \"雪 🐟\"\r\nprint(message)\r\n";
    fs::write(f.root.join(filename), original).unwrap();
    let setup = format!("{}{PICK}{RUFF_SELECT}", source("ruff"));
    let invocation = format!("command ruff $check $isolated $no_cache $select $rule $output $format {}", quote(filename));
    let before = evaluate(f, &tool, &cache, &format!("{setup}{invocation}"));
    capture(f, "ruff-upgrade-previous-lint", &before);
    status(&before, 1);
    assert_eq!(diagnostics(&before, filename), [(1, 8, "F401".to_owned())]);
    let unavailable = evaluate(f, &tool, &cache, &format!("{}complete -C 'ruff check --add-ig'", source("ruff")));
    status(&unavailable, 0);
    assert!(!std::str::from_utf8(&unavailable.out).unwrap().lines()
        .any(|line| line.split('\t').next() == Some("--add-ignore")));
    let unsupported = evaluate(f, &tool, &cache, &format!("{setup}{invocation} --add-ignore"));
    capture(f, "ruff-upgrade-previous-unsupported-ignore", &unsupported);
    status(&unsupported, 2);
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), original.as_bytes());

    // Replace bytes at the SAME executable path; keep the SAME completion
    // cache. A stale generated program cannot select the new --add-ignore.
    fs::copy(&current, &tool).unwrap();
    fs::OpenOptions::new().write(true).open(&tool).unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_060)).unwrap();
    let updated = evaluate(f, &tool, &cache, &format!("{setup}set -l add_ignore (__n9_pick 'ruff check --add-ig' --add-ignore)\n{invocation} $add_ignore"));
    capture(f, "ruff-upgrade-selected-ignore", &updated);
    status(&updated, 0);
    let ignored = fs::read(f.root.join(filename)).unwrap();
    let lines: Vec<_> = std::str::from_utf8(&ignored).unwrap().split("\r\n").collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("import os"));
    assert_eq!(&lines[1..], ["message = \"雪 🐟\"", "print(message)", ""]);
    let clean = evaluate(f, &tool, &cache, &format!("{setup}{invocation}"));
    status(&clean, 0);
    assert!(diagnostics(&clean, filename).is_empty());
    let ignore_noqa = evaluate(f, &tool, &cache, &format!("{setup}set -l ignore_noqa (__n9_pick 'ruff check --ignore-noq' --ignore-noqa)\n{invocation} $ignore_noqa"));
    status(&ignore_noqa, 1);
    assert_eq!(diagnostics(&ignore_noqa, filename), [(1, 8, "F401".to_owned())],
        "the clean result is the new suppression's effect, not deletion of the import");
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), ignored);
}

fn ruff_generator_failure(f: &Fixture) {
    let tool = required("FISH_N9_RUFF_LEGACY");
    let cache = f.state.join("ruff unsupported generator cache 雪");
    fs::create_dir(&cache).unwrap();
    let present = evaluate(f, &tool, &cache, "command -sq ruff");
    status(&present, 0);
    // This official older CLI runs, but interprets the unsupported generator
    // request as missing inputs and emits diagnostics instead of fish source.
    let producer = evaluate(f, &tool, &cache, "command ruff generate-shell-completion fish");
    capture(f, "ruff-present-generator-error", &producer);
    assert_ne!(producer.code, 0);
    let failed = evaluate(f, &tool, &cache, &source("ruff"));
    capture(f, "ruff-present-generator-source", &failed);
    assert_eq!(failed.code, producer.code,
        "failed generator output must not execute as fish commands and replace its status");
    assert!(failed.out.is_empty(), "failed generator diagnostics are not successful source output");
    assert!(!failed.err.is_empty(), "a present producer failure must remain visible");
}
fn rumdl(f: &Fixture) {
    let tool = required("FISH_N9_RUMDL");
    let cache = f.state.join("rumdl cache 雪");
    fs::create_dir(&cache).unwrap();
    let filename = "rumdl lint 雪 🐟.md";
    let original = "#雪 🐟\r\n\r\ntext 雪 🐟   \r\n";
    let fixed = "# 雪 🐟\r\n\r\ntext 雪 🐟\r\n";
    fs::write(f.root.join(filename), original).unwrap();
    let setup = format!("{}{PICK}{RUMDL_SELECT}", source("rumdl"));
    // The generated --enable option takes free-form rule codes, not an enum.
    let invocation = format!("command rumdl $check $isolated $no_cache $select MD018,MD009 $output $format {}", quote(filename));
    let lint = evaluate(f, &tool, &cache, &format!("{setup}{invocation}"));
    capture(f, "rumdl-selected-lint", &lint);
    status(&lint, 1);
    assert_eq!(diagnostics(&lint, filename), [(1, 2, "MD018".to_owned()), (3, 9, "MD009".to_owned())]);
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), original.as_bytes());
    let ignored = evaluate(f, &tool, &cache, &format!("{setup}set -l disable (__n9_pick 'rumdl check --dis' --disable)\n{invocation} $disable MD018,MD009"));
    status(&ignored, 0);
    assert!(diagnostics(&ignored, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), original.as_bytes());
    let fix = evaluate(f, &tool, &cache, &format!("{setup}set -l fix (__n9_pick 'rumdl check --fi' --fix)\n{invocation} $fix"));
    capture(f, "rumdl-selected-fix", &fix);
    status(&fix, 0);
    assert!(diagnostics(&fix, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), fixed.as_bytes(), "real heading/whitespace edits preserve Unicode and CRLF");
    let clean = evaluate(f, &tool, &cache, &format!("{setup}{invocation}"));
    status(&clean, 0);
    assert!(diagnostics(&clean, filename).is_empty());
    assert_eq!(fs::read(f.root.join(filename)).unwrap(), fixed.as_bytes());
    negatives(f, &tool, &cache, "rumdl");
}

// Save/restore the owned leaf's DACL, retaining WRITE_DAC during denial.
// Deny creation only to the owner; never grant access to unrelated principals.
// Restore the original protected/unprotected DACL even on unwinding.
#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetFileSecurityW(name: *const u16, info: u32, descriptor: *mut c_void, size: u32, needed: *mut u32) -> i32;
    fn SetFileSecurityW(name: *const u16, info: u32, descriptor: *const c_void) -> i32;
    fn GetSecurityDescriptorControl(descriptor: *const c_void, control: *mut u16, revision: *mut u32) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(text: *const u16, revision: u32, descriptor: *mut *mut c_void, size: *mut u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" { fn LocalFree(memory: *mut c_void) -> *mut c_void; }
struct DeniedCache { name: Vec<u16>, original: Vec<u8>, restore_flags: u32 }
impl DeniedCache {
    fn new(path: &Path) -> Self {
        let sddl = "D:P(D;;0x6;;;OW)(A;;FA;;;OW)";
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut needed = 0;
        assert_eq!(unsafe { GetFileSecurityW(name.as_ptr(), 4, std::ptr::null_mut(), 0, &mut needed) }, 0);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(122));
        let mut original = vec![0u8; needed as usize];
        assert_ne!(unsafe { GetFileSecurityW(name.as_ptr(), 4, original.as_mut_ptr().cast(), needed, &mut needed) }, 0);
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(unsafe { GetSecurityDescriptorControl(original.as_ptr().cast(), &mut control, &mut revision) }, 0);
        let restore_flags = 4 | if control & 0x1000 != 0 { 0x8000_0000 } else { 0x2000_0000 };
        let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = std::ptr::null_mut();
        assert_ne!(unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut descriptor, std::ptr::null_mut()) }, 0);
        let result = unsafe { SetFileSecurityW(name.as_ptr(), 0x8000_0004, descriptor) };
        let error = io::Error::last_os_error();
        unsafe { LocalFree(descriptor); }
        assert_ne!(result, 0, "owned completion fixture DACL: {error}");
        Self { name, original, restore_flags }
    }
}
impl Drop for DeniedCache {
    fn drop(&mut self) {
        assert_ne!(unsafe { SetFileSecurityW(self.name.as_ptr(), self.restore_flags, self.original.as_ptr().cast()) }, 0,
            "restore owned completion fixture DACL: {}", io::Error::last_os_error());
    }
}
