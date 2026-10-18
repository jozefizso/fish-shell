#![cfg_attr(
    all(nightly, feature = "benchmark"),
    feature(test),
    allow(unused_features)
)]

pub const BUILD_VERSION: &str = env!("FISH_BUILD_VERSION");

#[macro_use]
pub mod common;

#[macro_use]
pub mod wutil;

pub mod abbrs;
pub mod ast;
pub mod autoload;
pub mod builtins;
#[cfg(windows)]
pub mod clipboard_windows;
pub mod complete;
#[cfg(windows)]
pub mod console_windows;
pub mod editable_line;
pub mod env;
#[cfg_attr(windows, path = "env_dispatch/windows.rs")]
pub mod env_dispatch;
pub mod env_universal_common;
pub mod event;
#[cfg_attr(windows, path = "exec_windows.rs")]
pub mod exec;
pub mod expand;
#[cfg(unix)]
pub mod fd_monitor;
pub mod fd_readable_set;
#[cfg_attr(windows, path = "fds_windows.rs")]
pub mod fds;
pub mod flog;
#[cfg(unix)]
pub mod fork_exec;
#[cfg_attr(windows, path = "fs/windows.rs")]
pub mod fs;
pub mod function;
pub mod global_safety;
pub mod highlight;
pub mod history;
#[cfg(windows)]
pub mod identity_windows;
pub mod input;
#[cfg_attr(windows, path = "io_windows.rs")]
pub mod io;
pub mod job_group;
pub mod key;
pub mod kill;
pub mod locale;
pub mod localization;
#[cfg(windows)]
pub mod native_accounting;
#[cfg(windows)]
pub mod native_fd_bridge;
#[cfg(windows)]
pub(crate) mod native_text;
#[cfg(unix)]
pub mod nix;
pub mod null_terminated_array;
pub mod operation_context;
pub mod pager;
pub mod panic;
pub mod parse_constants;
pub mod parse_execution;
pub mod parse_tree;
pub mod parse_util;
pub mod parser;
pub mod parser_keywords;
#[cfg_attr(windows, path = "path/windows.rs")]
pub mod path;
pub mod portable_atomic;
pub mod prelude;
#[cfg(unix)]
pub mod print_help;
pub mod proc;
pub mod re;
pub mod reader;
pub mod redirection;
pub mod screen;
#[cfg_attr(windows, path = "signal_windows.rs")]
pub mod signal;
pub mod stdx;
pub mod terminal;
pub mod termsize;
pub mod text_face;
pub mod text_helpers;
#[cfg_attr(windows, path = "threads/windows.rs")]
pub mod threads;
#[cfg_attr(windows, path = "timer_windows.rs")]
pub mod timer;
pub mod tinyexpr;
pub mod tokenizer;
#[cfg(unix)]
pub mod topic_monitor;
pub mod trace;
pub mod tty_handoff;
pub mod universal_notifier;
pub mod wait_handle;
pub mod wildcard;

#[cfg(feature = "gettext-extract")]
pub extern crate fish_gettext_extraction;

#[cfg(test)]
mod tests;
