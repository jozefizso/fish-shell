#[cfg(unix)]
use crate::common::BUILD_DIR;
use crate::env::EnvStack;
use crate::env::{EnvMode, EnvVar, EnvVarFlags, Environment};
#[cfg(unix)]
use crate::locale::set_libc_locales;
use crate::parser::{CancelBehavior, Parser};
use crate::prelude::*;
#[cfg(unix)]
use crate::proc::proc_init;
#[cfg(unix)]
use crate::signal::signal_reset_handlers;
#[cfg(unix)]
use crate::wutil::wgetcwd;
use std::collections::HashMap;
#[cfg(unix)]
use std::env::set_current_dir;
#[cfg(unix)]
use std::path::PathBuf;

pub use serial_test::serial;

#[cfg(unix)]
pub fn test_init() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        // If we are building with `cargo build` and have build w/ `cmake`, this might not
        // yet exist.
        let mut test_dir = PathBuf::from(BUILD_DIR);
        test_dir.push("fish-test");
        std::fs::create_dir_all(&test_dir).unwrap();
        set_current_dir(&test_dir).unwrap();
        // Safety: all tests that access locale should go through the enclosing function.
        unsafe {
            set_libc_locales(/*log_ok=*/ false)
        };
        crate::topic_monitor::init();
        crate::threads::init();
        proc_init();

        // Set default signal handlers, so we can ctrl-C out of this.
        signal_reset_handlers();

        // Set PWD from getcwd - fixes #5599
        EnvStack::globals().set_pwd_from_getcwd();
    });
}

#[cfg(windows)]
pub fn run_native_test_in_child(test_name: &str) -> bool {
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};
    // Own the real registered main thread in an isolated exact-test process,
    // without changing another library test's thread ID or process globals.
    if std::env::var_os("FISH_NATIVE_TEST_CHILD").as_deref()
        != Some(std::ffi::OsStr::new(test_name))
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env("FISH_NATIVE_TEST_CHILD", test_name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let wait = unsafe { WaitForSingleObject(child.as_raw_handle(), 30_000) };
        if wait != 0 {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native owning test exceeded its process bound: {wait}");
        }
        assert!(child.wait().unwrap().success());
        return false;
    }
    true
}

/// An environment built around an std::map.
#[derive(Clone, Default)]
pub struct TestEnvironment {
    pub vars: HashMap<WString, WString>,
}
impl TestEnvironment {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }
}
impl Environment for TestEnvironment {
    fn getf(&self, name: &wstr, _mode: EnvMode) -> Option<EnvVar> {
        self.vars
            .get(name)
            .map(|value| EnvVar::new(value.clone(), EnvVarFlags::default()))
    }
    fn get_names(&self, _flags: EnvMode) -> Vec<WString> {
        self.vars.keys().cloned().collect()
    }
}

/// A test environment that knows about PWD.
#[derive(Default)]
pub struct PwdEnvironment {
    pub parent: TestEnvironment,
}
impl PwdEnvironment {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }
}
impl Environment for PwdEnvironment {
    fn getf(&self, name: &wstr, mode: EnvMode) -> Option<EnvVar> {
        if name == "PWD" {
            #[cfg(unix)]
            let cwd = wgetcwd();
            #[cfg(windows)]
            let cwd = fish_widestring::native_to_shell_text_owned(
                fish_widestring::osstr2wcstring(std::env::current_dir().expect("test cwd"))
                    .expect("valid native test cwd"),
            );
            return Some(EnvVar::new(cwd, EnvVarFlags::default()));
        }
        self.parent.getf(name, mode)
    }

    fn get_names(&self, flags: EnvMode) -> Vec<WString> {
        let mut res = self.parent.get_names(flags);
        if !res.iter().any(|n| n == "PWD") {
            res.push(L!("PWD").to_owned());
        }
        res
    }
}

/// A wrapper around a Parser with some test helpers.
pub struct TestParser {
    pub parser: Parser,
    #[cfg(unix)]
    pub pushed_dirs: Vec<String>,
}

impl TestParser {
    pub fn new() -> TestParser {
        let variables = EnvStack::new();
        #[cfg(windows)]
        variables
            .initialize_native_from(
                std::iter::empty(),
                &std::env::current_dir().expect("native model-test cwd"),
            )
            .expect("native model-test environment");
        TestParser {
            parser: Parser::new(variables, CancelBehavior::default())
                .expect("test parser initialization"),
            #[cfg(unix)]
            pushed_dirs: Vec::new(),
        }
    }

    // Mark that we expect noisy errors to be printed and that these should be suppressed.
    pub fn suppressing_stderr(mut self) -> TestParser {
        self.parser.test_only_suppress_stderr = true;
        self
    }
}

#[cfg(unix)]
pub trait ParserExt {
    /// Helper to chdir and then update $PWD.
    fn pushd(&self, pushed_dirs: &mut Vec<String>, path: &str);
    fn popd(&self, pushed_dirs: &mut Vec<String>);
}

#[cfg(unix)]
impl ParserExt for Parser {
    fn pushd(&self, pushed_dirs: &mut Vec<String>, path: &str) {
        let cwd = wgetcwd();
        pushed_dirs.push(cwd.to_string());

        // We might need to create the directory. We don't care if this fails due to the directory
        // already being present.
        std::fs::create_dir_all(path).unwrap();

        std::env::set_current_dir(path).unwrap();
        self.vars().set_pwd_from_getcwd();
    }

    fn popd(&self, pushed_dirs: &mut Vec<String>) {
        let old_cwd = pushed_dirs.pop().unwrap();
        std::env::set_current_dir(old_cwd).unwrap();
        self.vars().set_pwd_from_getcwd();
    }
}

impl std::ops::Deref for TestParser {
    type Target = Parser;
    fn deref(&self) -> &Self::Target {
        &self.parser
    }
}

impl std::ops::DerefMut for TestParser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parser
    }
}
