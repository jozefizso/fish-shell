//! Real native N8 provider/platform acceptance against debug and release artifacts.
#![cfg(windows)]
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_native/console_gate.rs"]
mod console_gate;
#[path = "windows_native/editor.rs"]
mod editor;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_n6/cases_probe.rs"]
mod cases_probe;
#[path = "windows_n6/child.rs"]
mod child;
#[path = "windows_n8/providers.rs"]
mod providers;
#[path = "windows_n8/tools.rs"]
mod tools;
#[path = "windows_n8/provider_console.rs"]
mod provider_console;
#[path = "windows_n8/policy.rs"]
mod policy;
#[path = "windows_n8/adapters.rs"]
mod adapters;
#[path = "windows_n8/services.rs"]
mod services;
#[path = "windows_n8/resource_helpers.rs"]
mod resource_helpers;
use fixture::Fixture;
type Family = (&'static str, fn(&Fixture));

#[test]
fn n8_argv() {
    console_gate::gate("N8-ARGV", &[providers::CASES]);
}
#[test]
fn n8_tools() {
    console_gate::gate("N8-TOOLS", &[tools::CASES, provider_console::FAMILIES, child::FAMILIES]);
}
#[test]
fn n8_policy() {
    console_gate::gate("N8-POLICY", &[policy::FAMILIES, adapters::FAMILIES, services::FAMILIES]);
}
