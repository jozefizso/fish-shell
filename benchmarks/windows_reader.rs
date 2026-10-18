//! Native Windows reader baseline; compile with rustc --edition=2024.
//! Arguments: absolute fish/probe binary directory, new owned evidence directory.
#![allow(dead_code)]
#[path = "../tests/windows_n6/console.rs"]
mod console;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
fn quote(path: &Path) -> String {
    format!(
        "'{}'",
        path.to_str()
            .unwrap()
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    )
}
fn capture(session: &mut console::Session, root: &Path, number: &mut usize, expected: &str) {
    *number += 1;
    session.send(b"\x1b[24~");
    assert_eq!(
        session.wait_file(&root.join(format!("ready-{number}"))),
        b"ready"
    );
    assert_eq!(
        fs::read(root.join(format!("capture-{number}"))).unwrap(),
        expected.as_bytes()
    );
    // File acknowledgement establishes model completion; mode observation also
    // establishes input readiness after any evaluator/reader transition.
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.snapshot().input_mode & 7 != 0 {
        assert!(
            Instant::now() < deadline,
            "editor input modes not reclaimed"
        );
    }
}
fn command(fish: &Path, root: &Path) -> Command {
    let mut command = Command::new(fish);
    command.args(["-N", "-i", "-C", "set -g bench_n 0; function fish_prompt; printf 'BENCH> '; end; function bench_capture; set -g bench_n (math $bench_n + 1); printf '%s' (commandline | string collect) > capture-$bench_n; printf ready > ready-$bench_n; end; bind f12 bench_capture"]);
    command
        .current_dir(root)
        .env_clear()
        .env("PATH", "")
        .env("LANG", "C");
    for (key, directory) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("TEMP", "temp"),
        ("TMP", "temp"),
    ] {
        let path = root.join(directory);
        fs::create_dir_all(&path).unwrap();
        command.env(key, path);
    }
    command.env("SystemRoot", std::env::var_os("SystemRoot").unwrap());
    command
}
fn main() {
    let binaries = PathBuf::from(std::env::args_os().nth(1).expect("binary directory"));
    let output = PathBuf::from(std::env::args_os().nth(2).expect("owned evidence root"));
    fs::create_dir_all(&output).unwrap();
    let fish = binaries.join("fish.exe");
    let probe = binaries.join("windows_native_probe.exe");
    for sample in 0..19 {
        let root = output.join(format!("startup-{sample}"));
        fs::create_dir(&root).unwrap();
        let command = command(&fish, &root);
        let started = Instant::now();
        let mut session = console::Session::start(&command, &probe, &root);
        session.wait_text(b"BENCH> ");
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        session.send(b"exit 0\r");
        let result = session.finish(0);
        assert_eq!(result.original_modes, result.final_modes);
        if sample >= 3 {
            println!("owned-console-first-prompt-ms,{},{}", sample - 3, elapsed);
        }
    }
    let root = output.join("interaction");
    fs::create_dir(&root).unwrap();
    let command = command(&fish, &root);
    let mut session = console::Session::start(&command, &probe, &root);
    session.wait_text(b"BENCH> ");
    let mut number = 0;
    for (metric, text) in [
        ("ascii-edit-capture-ack-ms", "a"),
        ("unicode-edit-capture-ack-ms", "雪🐟\u{f600}e\u{301}"),
    ] {
        for sample in 0..35 {
            capture(&mut session, &root, &mut number, "");
            let started = Instant::now();
            session.send(text.as_bytes());
            capture(&mut session, &root, &mut number, text);
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            session.send(b"\x03");
            if sample >= 3 {
                println!("{metric},{},{}", sample - 3, elapsed);
            }
        }
    }
    capture(&mut session, &root, &mut number, "");
    for sample in 0..19 {
        let child = root.join(format!("child-{sample}"));
        let ready = root.join(format!("child-done-{sample}"));
        let script = format!(
            "{} marker {}; test $status -eq 0; and printf done > {}\r",
            quote(&probe),
            quote(&child),
            quote(&ready)
        );
        let started = Instant::now();
        session.send(script.as_bytes());
        assert_eq!(session.wait_file(&ready), b"done");
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(fs::read(&child).unwrap(), b"launched");
        if sample >= 3 {
            println!("native-child-submit-complete-ms,{},{}", sample - 3, elapsed);
        }
    }
    session.send(b"exit 0\r");
    let result = session.finish(0);
    assert_eq!(result.original_modes, result.final_modes);
}
