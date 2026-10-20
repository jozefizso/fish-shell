//! N10 gate entrypoints. Required artifact/clean-machine fixtures are errors, never skips.
#![cfg(windows)]
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_n10/package.rs"]
mod package;

#[test]
fn n10_artifact() {
    package::artifact();
}

#[test]
fn n10_install() {
    package::install();
}

#[test]
fn n10_matrix() {
    package::matrix();
}
