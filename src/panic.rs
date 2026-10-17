use crate::threads::is_main_thread;
#[cfg(not(windows))]
use crate::{common::get_program_name, nix::isatty};
#[cfg(not(windows))]
use fish_common::read_blocked;
#[cfg(not(windows))]
use libc::STDIN_FILENO;
#[cfg(not(windows))]
use nix::unistd::getpid;
#[cfg(not(windows))]
use std::time::Duration;
use std::{
    panic::{UnwindSafe, set_hook, take_hook},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

pub static AT_EXIT: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

#[cfg(not(windows))]
pub fn panic_handler(main: impl FnOnce() -> i32 + UnwindSafe) -> ! {
    // The isatty() check will stop us from hanging in most fish tests, but not those
    // running in a simulated terminal emulator environment (such as the tmux or pexpect
    // tests). The FISH_FAST_FAIL environment variable is set in the test driver to
    // prevent the test suite from hanging on panic.
    let cleanup = || {
        if is_main_thread() {
            if let Some(at_exit) = AT_EXIT.get() {
                (at_exit)();
            }
        }
    };
    if isatty(STDIN_FILENO) && std::env::var_os("FISH_FAST_FAIL").is_none() {
        let standard_hook = take_hook();
        set_hook(Box::new(move |panic_info| {
            standard_hook(panic_info);
            static PANICKING: AtomicBool = AtomicBool::new(false);
            if PANICKING
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            cleanup();
            eprintf!("%s crashed, please report a bug.", get_program_name());
            if !is_main_thread() {
                eprintf!("\n");
                std::thread::sleep(Duration::from_secs(1));
            } else {
                eprintf!(" Debug PID %d or press Enter to exit\n", getpid().as_raw());
                let mut buf = [0_u8; 1];
                while let Ok(n) = read_blocked(STDIN_FILENO, &mut buf) {
                    if n == 0 || matches!(buf[0], b'q' | b'\n' | b'\r') {
                        eprintf!("\n");
                        break;
                    }
                }
            }
            std::process::abort();
        }));
    }
    let exit_status = main();
    cleanup();
    std::process::exit(exit_status)
}

/// Batch-mode panic handling never enters the reader, changes terminal modes or
/// waits for user input. The standard hook prints the real panic/backtrace.
#[cfg(windows)]
pub fn panic_handler(main: impl FnOnce() -> i32 + UnwindSafe) -> ! {
    let cleanup = || {
        if is_main_thread() {
            if let Some(at_exit) = AT_EXIT.get() {
                (at_exit)();
            }
        }
    };
    let standard_hook = take_hook();
    set_hook(Box::new(move |info| {
        standard_hook(info);
        static PANICKING: AtomicBool = AtomicBool::new(false);
        if PANICKING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            cleanup();
        }
    }));
    let status = main();
    cleanup();
    std::process::exit(status)
}
