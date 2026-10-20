//! Native shipped-asset, optional-tool and help/catalog acceptance.
#![cfg(windows)]
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_native/console_gate.rs"]
mod console_gate;
#[path = "windows_native/editor.rs"]
mod editor;
#[path = "windows_n6/console.rs"]
mod console;
#[path = "windows_n9/assets.rs"]
mod assets;
#[path = "windows_n9/tools.rs"]
mod tools;
#[path = "windows_n9/functions_core.rs"]
mod functions_core;
#[path = "windows_n9/functions_tools.rs"]
mod functions_tools;
#[path = "windows_n9/completion_contexts.rs"]
mod completion_contexts;
#[path = "windows_n9/completion_clang.rs"]
mod completion_clang;
#[path = "windows_n9/completion_mysql.rs"]
mod completion_mysql;
#[path = "windows_n9/completion_builtins.rs"]
mod completion_builtins;
#[path = "windows_n9/completion_convert.rs"]
mod completion_convert;
#[path = "windows_n9/completion_age.rs"]
mod completion_age;
#[path = "windows_n9/completion_rust_tools.rs"]
mod completion_rust_tools;
#[path = "windows_n9/completion_typst.rs"]
mod completion_typst;
#[path = "windows_n9/completion_netcat.rs"]
mod completion_netcat;
#[path = "windows_n9/completion_gpg_legacy.rs"]
mod completion_gpg_legacy;
#[path = "windows_n9/git_action.rs"]
mod git_action;
#[path = "windows_n9/theme_hook.rs"]
mod theme_hook;
#[path = "windows_n9/completion_7zip.rs"]
mod completion_7zip;
#[path = "windows_n9/completion_7zip_standalone.rs"]
mod completion_7zip_standalone;
#[path = "windows_n9/completion_gettext.rs"]
mod completion_gettext;
#[path = "windows_n9/locale.rs"]
mod locale;
use fixture::Fixture;
type Family = (&'static str, fn(&Fixture));

#[test]
fn n9_assets() {
    console_gate::gate("N9-ASSETS", &[assets::FAMILIES, functions_core::FAMILIES, functions_tools::FAMILIES, completion_contexts::FAMILIES, completion_clang::FAMILIES, completion_mysql::FAMILIES, completion_builtins::FAMILIES, completion_convert::FAMILIES, completion_age::FAMILIES, completion_rust_tools::FAMILIES, completion_typst::FAMILIES, completion_netcat::FAMILIES, completion_gpg_legacy::FAMILIES, git_action::FAMILIES, theme_hook::FAMILIES, completion_7zip::FAMILIES, completion_7zip_standalone::FAMILIES, completion_gettext::FAMILIES]);
}
#[test]
fn n9_tools() {
    console_gate::gate("N9-TOOLS", &[tools::FAMILIES]);
}
#[test]
fn n9_locale() {
    locale::n9_locale();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!("PASS {profile} N9-LOCALE");
}
