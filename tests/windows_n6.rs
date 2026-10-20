//! Real native N6 acceptance. All families run against both profile-matched artifacts.
#![cfg(windows)]
#[path = "windows_n6/cases_probe.rs"]
mod cases_probe;
#[path = "windows_n6/child.rs"]
mod child;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_n6/editor.rs"]
mod editor_cases;
#[path = "windows_native/editor.rs"]
mod editor;
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n6/tools.rs"]
mod tools;
#[path = "windows_native/console_gate.rs"]
mod console_gate;
use console_gate::gate;
use fixture::Fixture;
type Family = (&'static str, fn(&Fixture));

#[test]
fn n6_editor() {
    gate("N6-EDITOR", &[editor_cases::FAMILIES]);
}
#[test]
fn n6_child() {
    gate("N6-CHILD", &[child::FAMILIES]);
}
#[test]
fn n6_tools() {
    gate("N6-TOOLS", &[tools::FAMILIES]);
}
