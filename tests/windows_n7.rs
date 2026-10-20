//! N7 real evaluator/editor/console acceptance against profile-matched native artifacts.
#![cfg(windows)]
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_native/editor.rs"]
mod editor;
#[path = "windows_native/console_gate.rs"]
mod console_gate;
#[path = "windows_n7/complete.rs"]
mod complete;
#[path = "windows_n7/edit.rs"]
mod edit;
#[path = "windows_n7/eval_input.rs"]
mod eval_input;
#[path = "windows_n7/render.rs"]
mod render;
use fixture::Fixture;
type Family = (&'static str, fn(&Fixture));

#[test]
fn n7_complete() {
    console_gate::gate("N7-COMPLETE", &[complete::FAMILIES]);
}
#[test]
fn n7_edit() {
    console_gate::gate("N7-EDIT", &[edit::FAMILIES, eval_input::FAMILIES]);
}
#[test]
fn n7_render() {
    console_gate::gate("N7-RENDER", &[render::FAMILIES]);
}
