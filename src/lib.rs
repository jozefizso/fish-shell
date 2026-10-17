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

#[cfg(unix)]
pub mod abbrs;
pub mod ast;
#[cfg(unix)]
pub mod autoload;
pub mod builtins;
pub mod complete;
#[cfg(unix)]
pub mod editable_line;
pub mod env;
#[cfg_attr(windows, path = "env_dispatch/windows.rs")]
pub mod env_dispatch;
#[cfg(unix)]
pub mod env_universal_common;
pub mod event;
#[cfg_attr(windows, path = "exec_windows.rs")]
pub mod exec;
pub mod expand;
#[cfg(unix)]
pub mod fd_monitor;
#[cfg(unix)]
pub mod fd_readable_set;
#[cfg_attr(windows, path = "fds_windows.rs")]
pub mod fds;
pub mod flog;
#[cfg(unix)]
pub mod fork_exec;
#[cfg(unix)]
pub mod fs;
pub mod function;
pub mod global_safety;
#[cfg(unix)]
pub mod highlight;
#[cfg(unix)]
pub mod history;
#[cfg(unix)]
pub mod input;
#[cfg_attr(windows, path = "io_windows.rs")]
pub mod io;
pub mod job_group;
#[cfg(unix)]
pub mod key;
#[cfg(unix)]
pub mod kill;
pub mod locale;
pub mod localization;
#[cfg(unix)]
pub mod nix;
pub mod null_terminated_array;
pub mod operation_context;
#[cfg(unix)]
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
#[cfg(unix)]
pub mod re;
#[cfg(unix)]
pub mod reader;
pub mod redirection;
#[cfg(unix)]
pub mod screen;
#[cfg_attr(windows, path = "signal_windows.rs")]
pub mod signal;
pub mod stdx;
#[cfg(unix)]
pub mod terminal;
#[cfg(unix)]
pub mod termsize;
#[cfg(unix)]
pub mod text_face;
#[cfg_attr(windows, path = "threads/windows.rs")]
pub mod threads;
#[cfg(unix)]
pub mod timer;
pub mod tinyexpr;
pub mod tokenizer;
#[cfg(unix)]
pub mod topic_monitor;
pub mod trace;
#[cfg(unix)]
pub mod tty_handoff;
#[cfg(unix)]
pub mod universal_notifier;
pub mod wait_handle;
pub mod wildcard;

#[cfg(feature = "gettext-extract")]
pub extern crate fish_gettext_extraction;

#[cfg(all(test, unix))]
mod tests;
