//! Required native N3 contracts. Each gate executes both real executable profiles.
#![cfg(windows)]
use std::path::Path;
#[path = "windows_n3/builtins.rs"]
mod builtins;
#[path = "windows_n3/capture.rs"]
mod capture;
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n3/formatter.rs"]
mod formatter;
#[path = "windows_n3/language.rs"]
mod language;
#[path = "windows_n3/metadata.rs"]
mod metadata;
#[path = "windows_n2/native.rs"]
mod native;
#[path = "windows_n3/read.rs"]
mod read;
#[path = "windows_n3/scripts.rs"]
mod scripts;
#[path = "windows_n3/strings.rs"]
mod strings;
use fixture::Fixture;

fn gate(id: &str, cases: &[(&str, fn(&Fixture))]) {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut failures = Vec::new();
    for (profile, fish) in fixture::artifacts() {
        let fixture = Fixture::new(
            &fish,
            Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
            &format!("{profile}-{id}"),
        );
        let formatter = fish
            .parent()
            .expect("artifact profile directory")
            .join("fish_indent.exe");
        std::fs::copy(&formatter, fixture.root.join("fish_indent.exe")).unwrap_or_else(|error| {
            panic!(
                "required {profile} formatter {}: {error}",
                formatter.display()
            )
        });
        eprintln!("RUN {profile} {id}");
        let previous_failures = failures.len();
        for &(name, exercise) in cases {
            eprintln!("RUN-CASE {profile} {id}/{name}");
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(&fixture))).is_ok()
            {
                eprintln!("PASS-CASE {profile} {id}/{name}");
            } else {
                failures.push(format!("{profile} {id}/{name}"));
            }
        }
        if failures.len() == previous_failures {
            eprintln!("PASS {profile} {id}");
        }
    }
    assert!(
        failures.is_empty(),
        "native behavioral cases failed: {failures:?}"
    );
}

#[test]
#[allow(non_snake_case)]
fn N3_LANGUAGE() {
    gate(
        "N3-LANGUAGE",
        &[
            ("control-functions-events", language::exercise),
            ("portable-builtins", builtins::exercise),
            ("string-regex", strings::exercise),
            ("declarative-metadata", metadata::exercise),
        ],
    );
}
#[test]
#[allow(non_snake_case)]
fn N3_CAPTURE() {
    gate(
        "N3-CAPTURE",
        &[
            ("capture-pipelines", capture::exercise),
            ("buffered-read", read::exercise),
        ],
    );
}
#[test]
#[allow(non_snake_case)]
fn N3_SCRIPTS() {
    gate(
        "N3-SCRIPTS",
        &[
            ("invocation-source-profile", scripts::exercise),
            ("formatter", formatter::exercise),
        ],
    );
}
