//! Native acceptance entry point. One registry drives every required case in both build profiles.
#![cfg(windows)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "windows_native/provenance.rs"]
mod provenance;
#[path = "windows_native/fixture.rs"]
mod fixture;
use fixture::{exact, quote, run, Captured, Fixture};

const CASES: &[&str] = &[
    "W2-RUNTIME",
    "W3-BYTES",
    "W3-ERRORS",
    "W3-LIFETIME",
    "W3-CONSOLE",
    "W4-PATHS",
    "W5-LANGUAGE",
    "W5-NO-CONFIG",
    "W5-REJECT",
    "W6-CHILD",
    "W6-STREAMS",
    "W6-STATUS",
    "W6-FAILURE",
    "W6-BOUNDARY",
    "W6-CLEAN-RUN",
];
fn rejected(result: Captured) {
    assert_ne!(result.code, 0, "unsupported operation succeeded");
    assert!(
        !result.err.is_empty(),
        "missing useful rejection diagnostic"
    );
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for item in fs::read_dir(dir).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                entries.insert(path.strip_prefix(root).unwrap().to_owned(), Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn language(f: &Fixture) {
    exact(f.eval("echo 'quoted value'; printf '<%s>\\n' '' '雪🐟'; set -g xs one 'two words'; for x in $xs; echo $x; end; set -g scope global; begin; set -l scope local; echo $scope; end; echo $scope; if true; echo yes; else; echo bad; end; if false; echo bad; else; echo no; end; false; or echo recovered; true; and echo chained; math 6 / 2; exit 7; echo unreachable"), 7, "quoted value\n<>\n<雪🐟>\none\ntwo words\nlocal\nglobal\nyes\nno\nrecovered\nchained\n3\n".as_bytes(), b"");
    let mut bytes = "\u{f600}|\u{f600}|".as_bytes().to_vec();
    bytes.extend_from_slice(b"\xff\0\n");
    exact(
        f.eval("printf '%s|\\uF600|\\xff\\000\\n' '\u{f600}'"),
        0,
        &bytes,
        b"",
    );
    exact(f.eval("echo -en '\\xff\\000\\n'"), 0, b"\xff\0\n", b"");
    rejected(f.eval("if true; echo malformed"));
    rejected(f.eval("math 1 / 0"));
    exact(
        f.eval("set -g empty; true $empty; echo $status; false $empty; echo $status"),
        0,
        b"0\n1\n",
        b"",
    );
}
fn no_config(f: &Fixture) {
    let marker = f.root.join("config-ran");
    let script = format!("{} marker {}\n", f.probe(), quote(marker.to_str().unwrap()));
    for path in [
        f.state.join("config/fish/config.fish"),
        f.state.join("home/.config/fish/config.fish"),
    ] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &script).unwrap();
    }
    let before = snapshot(&f.state);
    exact(f.eval("echo no-config"), 0, b"no-config\n", b"");
    exact(
        run(
            &mut f.command(&[
                OsStr::new("--no-config"),
                OsStr::new("-c"),
                OsStr::new("echo long-option"),
            ]),
            b"",
        ),
        0,
        b"long-option\n",
        b"",
    );
    for args in [
        vec![],
        vec![OsStr::new("-N")],
        vec![OsStr::new("-N"), OsStr::new("-i")],
    ] {
        let result = run(&mut f.command(&args), b"");
        assert!(
            result.out.is_empty(),
            "unsupported mode emitted prompt/output"
        );
        rejected(result);
    }
    assert!(!marker.exists());
    assert_eq!(
        snapshot(&f.state),
        before,
        "initialization persisted user state"
    );
}
fn reject(f: &Fixture) {
    let jobs = [
        "echo forbidden | echo discarded",
        "echo forbidden > ignored",
        "echo forbidden 2> ignored",
        "echo forbidden &",
        "echo (set -g mutated yes; echo forbidden)",
        "exec echo forbidden",
        "fg",
        "bg",
        "set -U forbidden yes",
        "source missing.fish",
    ];
    for job in jobs {
        let result = f.eval(job);
        assert!(
            result.out.is_empty(),
            "unsupported job produced output: {job}"
        );
        rejected(result);
    }
    for job in [
        "echo (set -g mutated yes; echo forbidden) | echo discarded",
        "exec echo (set -g mutated yes; echo forbidden)",
        "echo (set -g mutated yes; echo forbidden) > ignored",
        "echo (set -g mutated yes; echo forbidden) &",
    ] {
        let result = f.eval(&format!("set -g mutated no; {job}; echo $mutated"));
        assert_eq!(
            result.out, b"no\n",
            "rejected job expanded a substitution: {job}"
        );
        assert!(!result.err.is_empty());
    }
    assert!(!f.root.join("ignored").exists());
}
fn paths(f: &Fixture) {
    let pwd = f.eval("printf '%s\\n' $PWD");
    assert_eq!(pwd.code, 0);
    assert!(pwd.err.is_empty());
    let pwd = String::from_utf8(pwd.out).unwrap();
    assert_cwd(pwd.strip_suffix('\n').unwrap(), &f.root);
    let result = f.eval(&format!("{} report", f.probe()));
    assert_eq!(result.code, 0);
    assert!(result.err.is_empty());
    assert_eq!(check_report_cwd(&String::from_utf8(result.out).unwrap(), &f.root), "");
    // N1 namespace resolution, drive-relative execution and UNC fixtures are now
    // positive/failure coverage in the mandatory windows_n1 acceptance gates.
    let root = f.root.ancestors().last().unwrap();
    let mut command = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("printf '%s\\n' $PWD"),
    ]);
    command.current_dir(root);
    let pwd = run(&mut command, b"");
    assert_eq!(pwd.code, 0);
    assert!(pwd.err.is_empty());
    let pwd = String::from_utf8(pwd.out).unwrap();
    assert_cwd(pwd.strip_suffix('\n').unwrap(), root);
}
fn assert_cwd(observed: &str, expected: &Path) {
    assert!(
        Path::new(observed).is_absolute(),
        "PWD is not absolute: {observed}"
    );
    assert_eq!(
        fs::canonicalize(observed).unwrap(),
        fs::canonicalize(expected).unwrap()
    );
}
fn hex(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn check_report_cwd<'a>(report: &'a str, expected: &Path) -> &'a str {
    let (record, remaining) = report.split_once('\n').unwrap();
    let encoded = record.strip_prefix("cwd=").unwrap();
    let mut units = encoded.as_bytes().chunks_exact(2);
    let decoded = units
        .by_ref()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert!(units.remainder().is_empty());
    assert_cwd(&String::from_utf8(decoded).unwrap(), expected);
    remaining
}

fn child(f: &Fixture) {
    let code = "printf 'export=<%s>\\n' $FISH_ACCEPT_EXPORTED; set -q FISH_ACCEPT_LOCAL; and echo leaked-local; set -q FISH_ACCEPT_ERASED; and echo leaked-erased; printf 'cwd=<%s>\\n' $PWD; printf 'arg=<%s>\\n' $argv; exit 7";
    let args = ["", "two words", "a\"b", "tail\\", "雪🐟", "'quoted'"];
    let launch = format!(
        "set -gx FISH_ACCEPT_EXPORTED 'visible 雪'; set -g FISH_ACCEPT_LOCAL hidden; set -gx FISH_ACCEPT_ERASED gone; set -e FISH_ACCEPT_ERASED; {} -N -c {} {}; printf 'status=<%s> pipe=<%s>\\n' $status $pipestatus; exit 7",
        quote(f.fish.to_str().unwrap()),
        quote(code),
        args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
    );
    let result = f.eval(&launch);
    assert_eq!(result.code, 7);
    assert!(result.err.is_empty());
    let text = String::from_utf8(result.out).unwrap();
    let mut records = text.splitn(3, '\n');
    assert_eq!(records.next(), Some("export=<visible 雪>"));
    let cwd = records
        .next()
        .unwrap()
        .strip_prefix("cwd=<")
        .unwrap()
        .strip_suffix('>')
        .unwrap();
    assert_cwd(cwd, &f.root);
    let expected = format!(
        "{}status=<7> pipe=<7>\n",
        args.iter()
            .map(|s| format!("arg=<{s}>\n"))
            .collect::<String>()
    );
    assert_eq!(records.next().unwrap(), expected);
    let result = f.eval(&format!("set -gx FISH_ACCEPT_EXPORTED 'visible 雪'; set -g FISH_ACCEPT_LOCAL hidden; set -gx FISH_ACCEPT_ERASED gone; set -e FISH_ACCEPT_ERASED; {} report {}", f.probe(), args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")));
    assert_eq!(result.code, 0);
    assert!(result.err.is_empty());
    let text = String::from_utf8(result.out).unwrap();
    let remaining = check_report_cwd(&text, &f.root);
    let expected = format!(
        "{}FISH_ACCEPT_EXPORTED={}\n",
        args.iter()
            .enumerate()
            .map(|(i, s)| format!("arg{i}={}\n", hex(s)))
            .collect::<String>(),
        hex("visible 雪")
    );
    assert_eq!(remaining, expected);
}
fn streams(f: &Fixture) {
    let bytes = b"\0\n\r\n\xff\x80native\0";
    let mut expected = b"before\nprobe-out:".to_vec();
    expected.extend_from_slice(bytes);
    expected.extend_from_slice(b"after\n");
    exact(
        f.input(
            &format!("echo before; {} stream; echo after", f.probe()),
            bytes,
        ),
        0,
        &expected,
        b"probe-err:\0\n\r\n\xff\x80",
    );
    // Larger than ordinary pipe buffers: concurrent draining is required, not optional.
    let bytes = vec![b'x'; 1024 * 1024];
    let mut expected = b"probe-out:".to_vec();
    expected.extend_from_slice(&bytes);
    exact(
        f.input(&format!("{} stream", f.probe()), &bytes),
        0,
        &expected,
        b"probe-err:\0\n\r\n\xff\x80",
    );
}
fn status(f: &Fixture) {
    for raw in [0u32, 7, 255, 256, 0xC0000005, 0xFFFFFFFF] {
        let mapped = if raw <= 255 { raw } else { 1 };
        let mut direct = Command::new(&f.probe);
        direct.arg("exit").arg(raw.to_string());
        exact(run(&mut direct, b""), raw, b"", b"");
        let result = f.eval(&format!(
            "{} exit {raw}; printf 'status=%s pipe=%s\\n' $status $pipestatus; exit {mapped}",
            f.probe()
        ));
        assert_eq!(result.code, mapped);
        assert_eq!(
            result.out,
            format!("status={mapped} pipe={mapped}\n").as_bytes()
        );
        if raw <= 255 {
            assert!(result.err.is_empty());
        } else {
            let err = String::from_utf8(result.err).unwrap();
            assert!(
                err.contains(&raw.to_string())
                    || err.to_ascii_lowercase().contains(&format!("{raw:08x}")),
                "raw native status lost: {err}"
            );
            assert!(
                !err.to_ascii_lowercase().contains("signal"),
                "fabricated Unix signal: {err}"
            );
        }
        let result = f.eval(&format!("{} exit {raw}", f.probe()));
        assert_eq!(
            result.code, mapped,
            "implicit host exit policy for {raw:#x}"
        );
        assert!(result.out.is_empty());
    }
}
fn failures(f: &Fixture) {
    let invalid = f.root.join("not executable.exe");
    fs::write(&invalid, b"not a PE executable\0").unwrap();
    for path in [f.root.join("missing.exe"), invalid] {
        for _ in 0..8 {
            let result = f.eval(&format!(
                "{}; set -l failed $status; echo continued; exit $failed",
                quote(path.to_str().unwrap())
            ));
            assert_ne!(result.code, 0);
            assert_eq!(result.out, b"continued\n");
            assert!(!result.err.is_empty());
        }
        exact(f.eval("echo usable"), 0, b"usable\n", b"");
    }
}
fn boundary(f: &Fixture) {
    let marker = f.root.join("must not exist");
    let command = format!("{} marker {}", f.probe(), quote(marker.to_str().unwrap()));
    for job in [
        format!("exec {command}"),
        format!("fg {command}"),
        format!("bg {command}"),
        format!("{command} | echo forbidden"),
        format!("{command} > ignored"),
        format!("echo ({command})"),
        format!("{command} &"),
    ] {
        let result = f.eval(&job);
        rejected(result);
        assert!(
            !marker.exists(),
            "unsupported job launched native probe: {job}"
        );
    }
    for extension in ["bat", "cmd", "ps1"] {
        let path = f.root.join(format!("forbidden script.{extension}"));
        let body = if extension == "ps1" {
            format!(
                "[IO.File]::WriteAllText('{}', 'launched')",
                marker.display()
            )
        } else {
            format!("@echo launched>\"{}\"\r\n", marker.display())
        };
        fs::write(&path, body).unwrap();
        rejected(f.eval(&quote(path.to_str().unwrap())));
        assert!(!marker.exists(), "script fallback ran {extension}");
    }
    let mut code = OsString::from("echo '");
    use std::os::windows::ffi::OsStringExt;
    code.push(OsString::from_wide(&[0xd800]));
    code.push("'");
    let invalid = run(
        &mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), &code]),
        b"",
    );
    assert!(
        invalid.out.is_empty(),
        "invalid native text was evaluated after lossy conversion"
    );
    rejected(invalid);
    provenance::exercise(f);
}

fn transport(f: &Fixture, case: &str) {
    let mode = match case {
        "W3-BYTES" => "transport-bytes",
        "W3-ERRORS" => "transport-errors",
        "W3-LIFETIME" => "transport-lifetime",
        "W3-CONSOLE" => "transport-console",
        _ => unreachable!(),
    };
    let mut command = Command::new(&f.probe);
    command.arg(mode).current_dir(&f.root);
    let input: &[u8] = if case == "W3-BYTES" {
        b"\0\n\r\n\xff\x80"
    } else {
        b""
    };
    let expected: &[u8] = match case {
        "W3-BYTES" => input,
        "W3-LIFETIME" => b"standard-borrow:standard-owner-still-open",
        _ => b"",
    };
    exact(run(&mut command, input), 0, expected, b"");
    if case == "W3-BYTES" {
        let path = f.root.join("native transport bytes.bin");
        let mut command = Command::new(&f.probe);
        command.arg("transport-file").arg(&path);
        exact(run(&mut command, b""), 0, b"", b"");
        assert_eq!(fs::read(path).unwrap(), input);
    }
}

fn runtime_initialization(probe: &Path, cwd: &Path) {
    let mut command = Command::new(probe);
    command.arg("runtime-init").current_dir(cwd).env_clear();
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    // Neither installed locale availability nor inherited categories may change
    // the prototype's deterministic C initialization.
    command
        .env("LC_ALL", "fr_FR.UTF-8")
        .env("LC_NUMERIC", "de_DE.UTF-8");
    exact(run(&mut command, b""), 0, b"", b"");
}

/// W2-RUNTIME can run without requiring completed fish debug/release artifacts.
#[test]
fn native_runtime_initialization() {
    let temporary = fish_tempfile::new_dir().unwrap();
    let cwd = temporary.path().join("runtime space 日本語 🦀");
    fs::create_dir(&cwd).unwrap();
    let probe = cwd.join("native runtime.exe");
    fs::copy(env!("CARGO_BIN_EXE_windows_native_probe"), &probe).unwrap();
    runtime_initialization(&probe, &cwd);
}

/// A focused consuming-evaluator regression entry point; the required registry also
/// runs these assertions for both debug and release under W6-BOUNDARY.
#[test]
fn native_text_provenance() {
    let fish = std::env::var_os("FISH_NATIVE_TEST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_fish")));
    let f = Fixture::new(
        &fish,
        Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
        "provenance",
    );
    provenance::exercise(&f);
}

#[test]
fn native_required_acceptance_cases() {
    let selected = std::env::var_os("FISH_NATIVE_TEST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_fish")));
    let release = std::env::var_os("FISH_NATIVE_TEST_RELEASE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let target = selected.parent().expect("fish artifact directory");
            target
                .parent()
                .expect("target profile directory")
                .join("release/fish.exe")
        });
    assert!(
        selected.is_file(),
        "required debug/selected fish artifact missing: {}",
        selected.display()
    );
    assert!(
        release.is_file(),
        "required release artifact missing; build --release or set FISH_NATIVE_TEST_RELEASE_BIN: {}",
        release.display()
    );
    let probe = PathBuf::from(env!("CARGO_BIN_EXE_windows_native_probe"));
    for (profile, artifact) in [("debug", selected), ("release", release)] {
        let mut executed = Vec::new();
        for id in CASES {
            let f = Fixture::new(&artifact, &probe, profile);
            eprintln!("RUN {profile} {id}");
            match *id {
                "W2-RUNTIME" => runtime_initialization(&f.probe, &f.root),
                "W3-BYTES" | "W3-ERRORS" | "W3-LIFETIME" | "W3-CONSOLE" => transport(&f, id),
                "W4-PATHS" => {
                    let before = snapshot(&f.state);
                    paths(&f);
                    assert_eq!(snapshot(&f.state), before);
                }
                "W5-LANGUAGE" => language(&f),
                "W5-NO-CONFIG" => no_config(&f),
                "W5-REJECT" => reject(&f),
                "W6-CHILD" => child(&f),
                "W6-STREAMS" => streams(&f),
                "W6-STATUS" => status(&f),
                "W6-FAILURE" => failures(&f),
                "W6-BOUNDARY" => boundary(&f),
                "W6-CLEAN-RUN" => {
                    let before = snapshot(&f.state);
                    language(&f);
                    child(&f);
                    streams(&f);
                    status(&f);
                    assert_eq!(snapshot(&f.state), before);
                }
                other => panic!("undiscovered required case {other}"),
            }
            executed.push(*id);
            eprintln!("PASS {profile} {id}");
        }
        assert_eq!(
            executed, CASES,
            "a mandatory native acceptance case was not executed"
        );
    }
}
