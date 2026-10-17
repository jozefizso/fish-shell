use crate::prelude::*;
#[cfg(not(windows))]
use crate::wildcard::wildcard_match;
#[cfg(not(windows))]
use crate::{parse_util::unescape_wildcards, wutil::unescape_bytes_and_write_to_fd};
#[cfg(not(windows))]
use fish_widestring::wcs2bytes;
#[cfg(not(windows))]
use libc::c_int;
#[cfg(not(windows))]
use nix::unistd;
#[cfg(not(windows))]
use std::os::fd::BorrowedFd;
#[cfg(not(windows))]
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

#[rustfmt::skip::macros(category)]
pub mod categories {
    use super::wstr;
    use crate::prelude::*;
    use std::sync::atomic::AtomicBool;

    pub struct Category {
        pub name: &'static wstr,
        pub description: LocalizableString,
        pub enabled: AtomicBool,
    }

    /// Macro to declare a static variable identified by $var,
    /// with the given name and description, and optionally enabled by default.
    macro_rules! declare_category {
        (
            ($var:ident, $name:literal, $description:literal, $enabled:expr)
        ) => {
            #[allow(non_upper_case_globals)]
            pub static $var: Category = Category {
                name: L!($name),
                description: localizable_string!($description),
                enabled: AtomicBool::new($enabled),
            };
        };
        (
            ($var:ident, $name:expr, $description:expr)
        ) => {
            declare_category!(($var, $name, $description, false));
        };
    }

    /// Macro to extract the variable name for a category.
    macro_rules! category_name {
        (($var:ident, $name:literal, $description:literal, $enabled:expr)) => {
            $var
        };
        (($var:ident, $name:literal, $description:literal)) => {
            $var
        };
    }

    macro_rules! categories {
        (
            // A repetition of categories, separated by semicolons.
            $($cats:tt);*

            // Allow trailing semicolon.
            $(;)?
        ) => {
            // Declare each category.
            $(
                declare_category!($cats);
            )*

            // Define a function which gives you a Vector of all categories.
            pub fn all_categories() -> Vec<&'static Category> {
                vec![
                    $(
                        & category_name!($cats),
                    )*
                ]
            }
        };
    }

    categories!(
        (error, "error", "Serious unexpected errors (on by default)", true);

        (debug, "debug", "Debugging aid (on by default)", true);

        (warning, "warning", "Warnings (on by default)", true);

        (warning_path, "warning-path", "Warnings about unusable paths for config/history (on by default)", true);

        (deprecated_test, "deprecated-test", "Warning about using test's zero- or one-argument modes (`test -d $foo`), which will be changed in future.");

        (config, "config", "Finding and reading configuration");

        (event, "event", "Firing events");

        (exec, "exec", "Errors reported by exec (on by default)", true);

        (exec_job_status, "exec-job-status", "Jobs changing status");

        (exec_job_exec, "exec-job-exec", "Jobs being executed");

        (exec_fork, "exec-fork", "Calls to fork()");

        (output_invalid, "output-invalid", "Trying to print invalid output");
        (ast_construction, "ast-construction", "Parsing fish AST");

        (proc_job_run, "proc-job-run", "Jobs getting started or continued");

        (proc_termowner, "proc-termowner", "Terminal ownership events");

        (proc_internal_proc, "proc-internal-proc", "Internal (non-forked) process events");

        (proc_reap_internal, "proc-reap-internal", "Reaping internal (non-forked) processes");

        (proc_reap_external, "proc-reap-external", "Reaping external (forked) processes");
        (proc_pgroup, "proc-pgroup", "Process groups");

        (env_locale, "env-locale", "Changes to locale variables");

        (env_export, "env-export", "Changes to exported variables");

        (env_dispatch, "env-dispatch", "Reacting to variables");

        (uvar_file, "uvar-file", "Writing/reading the universal variable store");
        (uvar_notifier, "uvar-notifier", "Notifications about universal variable changes");

        (topic_monitor, "topic-monitor", "Internal details of the topic monitor");
        (char_encoding, "char-encoding", "Character encoding issues");

        (history, "history", "Command history events");
        (history_file, "history-file", "Reading/Writing the history file");

        (profile_history, "profile-history", "History performance measurements");

        (synced_file_access, "synced-file-access", "Synchronized file access");

        (iothread, "iothread", "Background IO thread events");
        (fd_monitor, "fd-monitor", "FD monitor events");

        (term_support, "term-support", "Terminal feature detection");

        (reader, "reader", "The interactive reader/input system");
        (reader_render, "reader-render", "Rendering the command line");
        (complete, "complete", "The completion system");
        (path, "path", "Searching/using paths");

        (screen, "screen", "Screen repaints");

        (abbrs, "abbrs", "Abbreviation expansion");

        (refcell, "refcell", "Refcell dynamic borrowing");
        (autoload, "autoload", "autoloading");
    );
}

/// flog formats values. By default we would like to use Display, and fall back to Debug.
/// However that would require specialization. So instead we make two "separate" traits, bring them both in scope,
/// and let Rust figure it out.
/// Clients can opt a Debug type into Floggable by implementing FloggableDebug:
///    impl FloggableDebug for MyType {}
pub trait FloggableDisplay: std::fmt::Display {
    /// Return a string representation of this thing.
    fn to_flog_str(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

// Special handling for `WString` to decode PUA codepoints back into the original bytes.
#[cfg(not(windows))]
impl FloggableDisplay for WString {
    fn to_flog_str(&self) -> Vec<u8> {
        wcs2bytes(self)
    }
}

#[cfg(not(windows))]
impl FloggableDisplay for &wstr {
    fn to_flog_str(&self) -> Vec<u8> {
        wcs2bytes(*self)
    }
}

#[cfg(windows)]
impl FloggableDisplay for WString {}
#[cfg(windows)]
impl FloggableDisplay for &wstr {}

impl FloggableDisplay for LocalizableString {
    fn to_flog_str(&self) -> Vec<u8> {
        self.localize().to_flog_str()
    }
}

macro_rules! default_flog_impls {
    ($( $t:ty ),* ) => {
        $(
            impl FloggableDisplay for $t {}
        )*
    };
}

macro_rules! default_flog_impls_lifetimes {
    ($( $t:ty ),* ) => {
        $(
            impl<'a> FloggableDisplay for $t {}
        )*
    };
}

default_flog_impls! {
    String, &str, u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, char, std::io::Error, std::backtrace::Backtrace
}
#[cfg(not(windows))]
default_flog_impls! { nix::errno::Errno, crate::key::Key }
default_flog_impls_lifetimes! {
    std::path::Display<'a>, std::borrow::Cow<'a, str>
}

pub trait FloggableDebug: std::fmt::Debug {
    fn to_flog_str(&self) -> Vec<u8> {
        format!("{:?}", self).into_bytes()
    }
}

/// Write to our flog file.
#[cfg(not(windows))]
pub fn flog_impl(s: &[u8]) {
    let fd = get_flog_file_fd();
    if fd < 0 {
        return;
    }
    _ = unistd::write(unsafe { BorrowedFd::borrow_raw(fd) }, s);
}

#[cfg(windows)]
enum NativeLogDestination {
    Stderr,
    File(std::os::windows::io::OwnedHandle),
    Disabled,
}

#[cfg(windows)]
static NATIVE_LOG: std::sync::Mutex<NativeLogDestination> =
    std::sync::Mutex::new(NativeLogDestination::Stderr);

/// Transfer ownership of a real native logging endpoint to the logger.
#[cfg(windows)]
pub fn set_flog_file(handle: std::os::windows::io::OwnedHandle) -> std::io::Result<()> {
    *NATIVE_LOG.lock().map_err(|_| std::io::Error::other("logging lock poisoned"))? =
        NativeLogDestination::File(handle);
    Ok(())
}

#[cfg(windows)]
pub fn disable_flog() -> std::io::Result<()> {
    *NATIVE_LOG.lock().map_err(|_| std::io::Error::other("logging lock poisoned"))? =
        NativeLogDestination::Disabled;
    Ok(())
}

/// Report native I/O errors to callers that need to observe logging delivery.
#[cfg(windows)]
pub fn flog_impl(bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::windows::io::AsHandle;
    let destination = NATIVE_LOG.lock().map_err(|_| std::io::Error::other("logging lock poisoned"))?;
    let mut endpoint = match &*destination {
        NativeLogDestination::Stderr => crate::io::NativeOutput::stderr()?,
        NativeLogDestination::File(handle) => crate::io::NativeOutput::borrowed(handle.as_handle()),
        NativeLogDestination::Disabled => return Ok(()),
    };
    endpoint.write_all(bytes)?;
    endpoint.flush()
}


/// The entry point for flogging.
#[macro_export]
macro_rules! flog {
    ($category:ident, $($elem:expr),+ $(,)*) => {
        if $crate::flog::categories::$category.enabled.load(std::sync::atomic::Ordering::Relaxed) {
            #[allow(unused_imports)]
            use $crate::{flog::{FloggableDisplay as _, FloggableDebug as _}};
            let mut output: Vec<u8> = Vec::new();
            output.extend($crate::flog::categories::$category.name.to_flog_str());
            output.push(b':');
            $(
                {
                    output.push(b' ');
                    output.extend($elem.to_flog_str());
                }
            )+
            // We don't use locking here so we have to append our own newline to avoid multiple writes.
            output.push(b'\n');
            let _ = $crate::flog::flog_impl(&output);
        }
    };
}

#[macro_export]
macro_rules! flogf {
    ($category:ident, $fmt: expr, $($elem:expr),+ $(,)*) => {
        $crate::flog::flog!($category, $crate::wutil::sprintf!($fmt, $($elem),*))
    }
}

#[macro_export]
macro_rules! should_flog {
    ($category:ident) => {
        $crate::flog::categories::$category
            .enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    };
}

pub use {flog, flogf, should_flog};

/// For each category, if its name matches the wildcard, set its enabled to the given sense.
#[cfg(not(windows))]
fn apply_one_wildcard(wc_esc: &wstr, sense: bool) {
    let wc = unescape_wildcards(wc_esc);
    let mut match_found = false;
    for cat in categories::all_categories() {
        if wildcard_match(cat.name, &wc, false) {
            cat.enabled.store(sense, Ordering::Relaxed);
            match_found = true;
        }
    }
    if !match_found {
        eprintf!("Failed to match debug category: %s\n", wc_esc);
    }
}

#[cfg(windows)]
fn apply_one_wildcard(pattern: &wstr, sense: bool) {
    // Category names are text, not filesystem paths; no traversal or completion
    // service is needed for '*' and '?' category selection.
    fn matches(text: &wstr, pattern: &wstr) -> bool {
        let (mut t, mut p, mut star, mut retry) = (0, 0, None, 0);
        while t < text.len() {
            if p < pattern.len() && (pattern.char_at(p) == '?' || pattern.char_at(p) == text.char_at(t)) {
                t += 1;
                p += 1;
            } else if p < pattern.len() && pattern.char_at(p) == '*' {
                star = Some(p);
                p += 1;
                retry = t;
            } else if let Some(s) = star {
                retry += 1;
                t = retry;
                p = s + 1;
            } else {
                return false;
            }
        }
        while p < pattern.len() && pattern.char_at(p) == '*' { p += 1; }
        p == pattern.len()
    }
    let mut found = false;
    for category in categories::all_categories() {
        if matches(category.name, pattern) {
            category.enabled.store(sense, Ordering::Relaxed);
            found = true;
        }
    }
    if !found {
        let _ = flog_impl(&crate::wutil::sprintf!("Failed to match debug category: %s\n", pattern).to_flog_str());
    }
}


/// Set the active flog categories according to the given wildcard `wc`.
pub fn activate_flog_categories_by_pattern(wc_ptr: &wstr) {
    let mut wc: WString = wc_ptr.into();
    // Normalize underscores to dashes, allowing the user to be sloppy.
    for c in wc.as_char_slice_mut() {
        if *c == '_' {
            *c = '-';
        }
    }
    for s in wc.split(',') {
        if s.starts_with('-') {
            apply_one_wildcard(s.slice_from(1), false);
        } else {
            apply_one_wildcard(s, true);
        }
    }
}

/// The flog output fd. Defaults to stderr. A value < 0 disables flog.
#[cfg(not(windows))]
static FLOG_FD: AtomicI32 = AtomicI32::new(libc::STDERR_FILENO);

#[cfg(not(windows))]
pub fn set_flog_file_fd(fd: c_int) {
    FLOG_FD.store(fd, Ordering::Relaxed);
}

#[cfg(not(windows))]
#[inline]
pub fn get_flog_file_fd() -> c_int {
    FLOG_FD.load(Ordering::Relaxed)
}

#[cfg(not(windows))]
pub fn log_extra_to_flog_file(s: &wstr) {
    unescape_bytes_and_write_to_fd(s, get_flog_file_fd());
}

#[cfg(windows)]
pub fn log_extra_to_flog_file(s: &wstr) -> std::io::Result<()> {
    flog_impl(&s.to_flog_str())
}
