//! N11 release-artifact consumers. Missing required evidence is a failure, never a skip.
#![cfg(windows)]
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_native/editor.rs"]
mod editor;
#[path = "windows_n11/package.rs"]
mod package;
#[path = "windows_n11/cli.rs"]
mod cli;
#[path = "windows_n11/daily.rs"]
mod daily;
#[path = "windows_n11/faults.rs"]
mod faults;
#[path = "windows_n11/resources.rs"]
mod resources;
#[path = "windows_n11/native.rs"]
mod native;
#[path = "windows_n5/support.rs"]
mod support;
#[path = "windows_n5/jobs.rs"]
mod jobs;
#[path = "windows_n2/lifetime.rs"]
mod lifetime;
#[path = "windows_n3/scripts.rs"]
mod scripts;
#[path = "windows_n4/notifications.rs"]
mod notifications;
#[path = "windows_n4/storage.rs"]
mod storage;
#[path = "windows_n5/cancel.rs"]
mod cancel;
#[path = "windows_n5/spin.rs"]
mod spin;

#[test]
#[serial_test::serial]
fn n11_coverage() {
    let installed = package::Installed::new("coverage");
    cli::exercise(&installed);
    installed.python("tests/windows_n11/reconcile.py", &[]);
}

#[test]
#[serial_test::serial]
fn n11_faults() {
    let installed = package::Installed::new("faults");
    faults::exercise(&installed);
}

#[test]
#[serial_test::serial]
fn n11_quality() {
    let installed = package::Installed::new("quality");
    daily::exercise(&installed);
    installed.python("benchmarks/windows_quality.py", &[installed.fixture.fish.as_os_str(), installed.fixture.probe.as_os_str()]);
}
