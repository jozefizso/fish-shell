//! Mandatory native console tests. Every controller owns a fresh actual console;
//! no control broadcast can reach the test runner or its enclosing host job.
use super::fixture::{Fixture, run};
use std::os::windows::process::CommandExt;
use std::process::Command;

pub const FAMILIES: &[(&str, fn(&Fixture))] = &[
    (
        "console-receipt-rearm-shutdown",
        console_receipt_rearm_and_shutdown,
    ),
    (
        "cooperative-ignored-forced-unsupported",
        cooperative_ignored_forced_and_unsupported,
    ),
    (
        "blocked-read-pipe-evaluator",
        blocked_read_pipe_and_evaluator,
    ),
    (
        "close-and-native-status-provenance",
        console_close_and_native_status_provenance,
    ),
];

fn controller(f: &Fixture, mode: &str) {
    let mut command = Command::new(&f.probe);
    command
        .arg(mode)
        .arg(&f.fish)
        .arg(f.root.join("windows_n5_gui_probe.exe"))
        .current_dir(&f.root)
        .creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE, never the runner's console.
    command.env_clear().env("PATH", "").env("LANG", "C");
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    for (key, directory) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("APPDATA", "config"),
        ("XDG_DATA_HOME", "data"),
        ("LOCALAPPDATA", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        command.env(key, f.state.join(directory));
    }
    // run retains the actual child handle, bounds the process wait and drains
    // stdout/stderr concurrently, including assertion diagnostics on failure.
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.out.is_empty(), "{:?}", result.out);
    // Unsupported/live-job outcomes may truthfully diagnose retained ordinary
    // jobs at shutdown. Their control, ownership and status assertions belong
    // to the probe; diagnostic wording is not part of this family contract.
    if mode != "n5-cancel-requests" {
        assert!(
            result.err.is_empty(),
            "{}",
            String::from_utf8_lossy(&result.err)
        );
    }
}

pub fn console_receipt_rearm_and_shutdown(f: &Fixture) {
    controller(f, "n5-cancel-controls");
}

pub fn cooperative_ignored_forced_and_unsupported(f: &Fixture) {
    controller(f, "n5-cancel-requests");
}

pub fn blocked_read_pipe_and_evaluator(f: &Fixture) {
    // Held input/capture endpoints distinguish cancellation from EOF. The
    // physical CLI reader has no -c command, and builtin output must unwind
    // while its full pipe remains completely unconsumed, then work after rearm.
    // Parser ownership never moves off its main thread; worker supervisors own
    // only their retained children, never the enclosing host job.
    for mode in [
        "n5-cancel-read",
        "n5-cancel-stdin",
        "n5-cancel-evaluator",
        "n5-cancel-output",
        "n5-cancel-pipe",
        "n5-cancel-native-wait",
    ] {
        controller(f, mode);
    }
}

pub fn console_close_and_native_status_provenance(f: &Fixture) {
    controller(f, "n5-cancel-close");
    let p = f.probe();
    // These are ordinary high DWORD exits, including the Windows Ctrl-C exit
    // value. Neither the high bit nor this particular number is a receipt.
    for raw in [259u32, 0x8000_0001, 0xc000_013a, u32::MAX] {
        let result = f.eval(&format!(
            "function n5_process --on-process-exit 0; printf 'process=%s\\n' $argv[3]; end; or exit 91; \
             function n5_control --on-signal CTRL_C; echo unexpected-control; end; or exit 92; \
             {p} exit {raw} | {p} exit 7; \
             set -l saved $status $pipestatus; printf 'status=%s\\n' $saved"
        ));
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        let output = String::from_utf8(result.out).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines.iter().filter(|line| **line == "process=1").count(), 1);
        assert_eq!(lines.iter().filter(|line| **line == "process=7").count(), 1);
        assert!(!lines.contains(&"unexpected-control"));
        let statuses: Vec<_> = lines
            .iter()
            .copied()
            .filter(|line| line.starts_with("status="))
            .collect();
        assert_eq!(statuses, ["status=7", "status=1", "status=7"]);
    }
}
