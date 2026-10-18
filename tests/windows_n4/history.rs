use super::fixture::{Captured, Fixture, LIMIT, exact, quote, run};
use std::ffi::{OsStr, c_void};
use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

type Handle = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn WaitForSingleObject(event: Handle, timeout: u32) -> u32;
    fn CloseHandle(event: Handle) -> i32;
}
struct Event {
    handle: Handle,
    name: String,
}
impl Event {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "Local\\fish-n4-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(
            !handle.is_null(),
            "CreateEventW: {}",
            std::io::Error::last_os_error()
        );
        Self { handle, name }
    }
    fn wait(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle, LIMIT.as_millis() as u32) },
            0,
            "history barrier {}",
            self.name
        );
    }
    fn signal(&self) {
        assert_ne!(unsafe { SetEvent(self.handle) }, 0);
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
// Declared inside each scoped orchestration closure so unwinding releases children before
// `thread::scope` joins them. The child and process runner retain their independent deadlines.
struct ReleaseOnDrop<'a>(&'a Event);
impl Drop for ReleaseOnDrop<'_> {
    fn drop(&mut self) {
        unsafe {
            SetEvent(self.0.handle);
        }
    }
}
struct InterruptedWriter(std::process::Child);
impl Drop for InterruptedWriter {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            if unsafe { WaitForSingleObject(self.0.as_raw_handle(), LIMIT.as_millis() as u32) } == 0
            {
                let _ = self.0.wait();
            }
        }
    }
}
fn barrier(f: &Fixture, ready: &Event, release: &Event) -> String {
    format!(
        "{} n4-barrier {} {}; or exit 93",
        f.probe(),
        quote(&ready.name),
        quote(&release.name)
    )
}
fn evaluate(f: &Fixture, session: &str, code: &str) -> Captured {
    run(
        &mut f
            .command(&[OsStr::new("-c"), OsStr::new(code)])
            .env("fish_history", session),
        b"",
    )
}
fn path(f: &Fixture, session: &str) -> PathBuf {
    f.state
        .join("data")
        .join("fish")
        .join(format!("{session}_history"))
}
fn entries(result: Captured) -> Vec<String> {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(
        result.err.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.err)
    );
    let mut lines: Vec<_> = String::from_utf8(result.out)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

pub(super) fn exercise_startup(f: &Fixture) {
    exact(
        evaluate(
            f,
            "n4_session_a",
            "builtin history append 'first 雪' ''; builtin history save; set -g fish_history n4_session_b; builtin history append second; builtin history save; set -g fish_history n4_session_a; builtin history search",
        ),
        0,
        "first 雪\n".as_bytes(),
        b"",
    );
    exact(
        evaluate(f, "n4_session_b", "builtin history search"),
        0,
        b"second\n",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_session_a",
            "set -q history[1]; or exit 87; printf '<%s>\\n' $history[1]",
        ),
        0,
        "<first 雪>\n".as_bytes(),
        b"",
    );
    // Corrupt old disk history has an observable read diagnostic. Private
    // startup must select memory history before any such decoding takes place.
    let old_private = path(f, "n4_private_existing");
    let old_bytes = b"- cmd: old-secret\n  when: 123\ngarbage\n\0bad\n- cmd: recovered\n  when: 124\n- cmd: interrupted";
    fs::write(&old_private, old_bytes).unwrap();
    for flag in ["-P", "-N"] {
        let mut command = f.command(&[OsStr::new(flag), OsStr::new("-c"), OsStr::new("set -g fish_history n4_private; builtin history append secret; builtin history save; builtin history search")]);
        command.env("fish_history", "n4_private_existing");
        let result = run(&mut command, b"");
        exact(result, 0, b"secret\n", b"");
        assert!(
            !path(f, "n4_private").exists(),
            "private session persisted history"
        );
    }
    assert_eq!(fs::read(old_private).unwrap(), old_bytes);
    let escaped = "builtin history append '雪 🐟' 'back\\slash' 'line\none'; builtin history save; builtin history search --null";
    let written = evaluate(f, "n4_escaped", escaped);
    assert_eq!(written.code, 0, "{}", String::from_utf8_lossy(&written.err));
    assert!(written.err.is_empty());
    let fresh = evaluate(f, "n4_escaped", "builtin history search --null");
    exact(fresh, 0, &written.out, b"");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let disk = fs::read_to_string(path(f, "n4_escaped")).unwrap();
    let timestamps: Vec<u64> = disk
        .lines()
        .filter_map(|line| line.strip_prefix("  when: "))
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(timestamps.len(), 3);
    assert!(
        timestamps.into_iter().all(|timestamp| timestamp <= now),
        "history invented future timestamps"
    );
    exact(
        evaluate(
            f,
            "n4_delete_at_exit",
            "builtin history append retained deleted; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_delete_at_exit",
            "builtin history delete --exact --case-sensitive deleted",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(f, "n4_delete_at_exit", "builtin history search"),
        0,
        b"retained\n",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_clear_session",
            "builtin history append baseline; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_clear_session",
            "builtin history append temporary; builtin history clear-session; builtin history search",
        ),
        0,
        b"baseline\n",
        b"",
    );
    exact(
        evaluate(f, "n4_clear_session", "builtin history search"),
        0,
        b"baseline\n",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_clear_session",
            "builtin history append baseline temporary; builtin history save; builtin history clear-session; builtin history search",
        ),
        0,
        b"baseline\n",
        b"",
    );
    exact(
        evaluate(f, "n4_clear_session", "builtin history search"),
        0,
        b"baseline\n",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_clear_session",
            "builtin history clear; builtin history search",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(f, "n4_clear_session", "builtin history search"),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_delete_clear",
            "builtin history append original; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_delete_clear",
            "builtin history delete --exact --case-sensitive original; builtin history append original; builtin history clear-session; builtin history search",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(f, "n4_delete_clear", "builtin history search"),
        0,
        b"",
        b"",
    );
}

pub(super) fn exercise_sharing(f: &Fixture) {
    exact(
        evaluate(
            f,
            "n4_shared",
            "builtin history append seed; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    let ready = Event::new();
    let release = Event::new();
    let code = format!(
        "set -l snapshot (builtin history search); {}; builtin history append reader; builtin history save; test (builtin history search --exact reader | count) -eq 1; or exit 90; test (builtin history search --exact writer | count) -eq 0; or exit 91; test (builtin history search --exact seed | count) -eq 1; or exit 92; builtin history merge; or exit 93; builtin history search",
        barrier(f, &ready, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let reader = scope.spawn(|| evaluate(f, "n4_shared", &code));
        ready.wait();
        exact(
            evaluate(
                f,
                "n4_shared",
                "builtin history append writer; builtin history delete --exact --case-sensitive seed; builtin history save",
            ),
            0,
            b"",
            b"",
        );
        release.signal();
        assert_eq!(entries(reader.join().unwrap()), vec!["reader", "writer"]);
    });
    assert_eq!(
        entries(evaluate(f, "n4_shared", "builtin history search")),
        vec!["reader", "writer"]
    );

    let ready_a = Event::new();
    let ready_b = Event::new();
    let release = Event::new();
    let a = format!(
        "{}; for value in alpha1 alpha2 alpha3; builtin history append $value; builtin history save; or exit 94; end",
        barrier(f, &ready_a, &release)
    );
    let b = format!(
        "{}; for value in beta1 beta2 beta3; builtin history append $value; builtin history save; or exit 95; end",
        barrier(f, &ready_b, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let a = scope.spawn(|| evaluate(f, "n4_concurrent", &a));
        let b = scope.spawn(|| evaluate(f, "n4_concurrent", &b));
        ready_a.wait();
        ready_b.wait();
        release.signal();
        exact(a.join().unwrap(), 0, b"", b"");
        exact(b.join().unwrap(), 0, b"", b"");
    });
    assert_eq!(
        entries(evaluate(f, "n4_concurrent", "builtin history search")),
        vec!["alpha1", "alpha2", "alpha3", "beta1", "beta2", "beta3"]
    );

    exact(
        evaluate(
            f,
            "n4_session_foreign",
            "builtin history append baseline; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    let ready = Event::new();
    let release = Event::new();
    let code = format!(
        "set -l snapshot (builtin history search); builtin history append temporary baseline; builtin history save; {}; builtin history clear-session; or exit 94; builtin history merge; or exit 95; builtin history search",
        barrier(f, &ready, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let reader = scope.spawn(|| evaluate(f, "n4_session_foreign", &code));
        ready.wait();
        exact(
            evaluate(
                f,
                "n4_session_foreign",
                "builtin history append foreign; builtin history save",
            ),
            0,
            b"",
            b"",
        );
        release.signal();
        assert_eq!(entries(reader.join().unwrap()), vec!["baseline", "foreign"]);
    });
    assert_eq!(
        entries(evaluate(f, "n4_session_foreign", "builtin history search")),
        vec!["baseline", "foreign"]
    );
}

pub(super) fn exercise_recovery(f: &Fixture) {
    fs::create_dir_all(f.state.join("data/fish")).unwrap();
    let rejected = b"# 1339519901\nrejected-old-one\n";
    fs::write(path(f, "n4_rejected"), rejected).unwrap();
    let failure = evaluate(
        f,
        "n4_rejected",
        "builtin history append pending; builtin history save; set -l saved $status; builtin history search --exact pending; exit $saved",
    );
    assert_ne!(failure.code, 0);
    assert_eq!(failure.out, b"pending\n");
    assert!(
        !failure.err.iter().all(u8::is_ascii_whitespace),
        "failed save lacked a diagnostic"
    );
    assert_eq!(fs::read(path(f, "n4_rejected")).unwrap(), rejected);
    fs::write(path(f, "n4_old"), b"- cmd: legacy\n  when: 1339519901\n- cmd: line\\ncontinued\n  when: 1339519903\n  paths:\n    - /old/path\n- cmd: #command\n  when: 1339519904\n").unwrap();
    exact(
        evaluate(f, "n4_old", "builtin history search --null"),
        0,
        b"#command\0line\ncontinued\0legacy\0",
        b"",
    );
    exact(
        evaluate(
            f,
            "n4_old",
            "builtin history append migrated; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    exact(
        evaluate(f, "n4_old", "builtin history search --null"),
        0,
        b"migrated\0#command\0line\ncontinued\0legacy\0",
        b"",
    );
    fs::write(path(f, "n4_corrupt"), b"- cmd: valid\n  when: 123\ngarbage\n\0bad\n- cmd: recovered\n  when: 124\n- cmd: interrupted").unwrap();
    let recovered = evaluate(f, "n4_corrupt", "builtin history search");
    assert_eq!(recovered.code, 0);
    assert_eq!(recovered.out, b"recovered\nvalid\n");
    assert!(
        !recovered.err.iter().all(u8::is_ascii_whitespace),
        "corruption lacked a diagnostic"
    );
    let repaired = evaluate(
        f,
        "n4_corrupt",
        "builtin history append new; builtin history save",
    );
    assert_eq!(
        repaired.code,
        0,
        "{}",
        String::from_utf8_lossy(&repaired.err)
    );
    assert!(repaired.out.is_empty());
    assert_eq!(
        entries(evaluate(f, "n4_corrupt", "builtin history search")),
        vec!["new", "recovered", "valid"]
    );

    exact(
        evaluate(
            f,
            "n4_denied",
            "builtin history append preserved; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    let target = path(f, "n4_denied");
    let before = fs::read(&target).unwrap();
    let deny_replace = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&target)
        .unwrap();
    let ready = Event::new();
    let release = Event::new();
    let code = format!(
        "builtin history append pending; builtin history save; set -l saved $status; builtin history merge; set -l merged $status; builtin history clear; set -l cleared $status; builtin history search --exact pending; {}; test $saved -ne 0; or exit 96; test $merged -ne 0; or exit 97; test $cleared -ne 0; or exit 98; builtin history merge; or exit 99; builtin history save",
        barrier(f, &ready, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let writer = scope.spawn(|| evaluate(f, "n4_denied", &code));
        ready.wait();
        assert_eq!(fs::read(&target).unwrap(), before);
        drop(deny_replace);
        release.signal();
        let result = writer.join().unwrap();
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        assert_eq!(result.out, b"pending\n");
        assert!(
            !result.err.iter().all(u8::is_ascii_whitespace),
            "failed operations lacked diagnostics"
        );
    });
    assert_eq!(
        entries(evaluate(f, "n4_denied", "builtin history search")),
        vec!["pending", "preserved"]
    );

    exact(
        evaluate(
            f,
            "n4_exit_retry",
            "builtin history append preserved deleted; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    let exit_target = path(f, "n4_exit_retry");
    let exit_before = fs::read(&exit_target).unwrap();
    let deny_exit_replace = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&exit_target)
        .unwrap();
    let ready = Event::new();
    let release = Event::new();
    let code = format!(
        "builtin history delete --exact --case-sensitive deleted; builtin history append pending; builtin history save; set -l saved $status; builtin history search --exact pending; {}; test $saved -ne 0",
        barrier(f, &ready, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let writer = scope.spawn(|| evaluate(f, "n4_exit_retry", &code));
        ready.wait();
        assert_eq!(fs::read(&exit_target).unwrap(), exit_before);
        drop(deny_exit_replace);
        release.signal();
        let result = writer.join().unwrap();
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        assert_eq!(result.out, b"pending\n");
        assert!(
            !result.err.iter().all(u8::is_ascii_whitespace),
            "failed explicit save lacked a diagnostic"
        );
    });
    assert_eq!(
        entries(evaluate(f, "n4_exit_retry", "builtin history search")),
        vec!["pending", "preserved"]
    );

    exact(
        evaluate(
            f,
            "n4_clear_retry",
            "builtin history append baseline; builtin history save",
        ),
        0,
        b"",
        b"",
    );
    let clear_target = path(f, "n4_clear_retry");
    let clear_before = fs::read(&clear_target).unwrap();
    let deny_clear = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&clear_target)
        .unwrap();
    let ready = Event::new();
    let release = Event::new();
    let code = format!(
        "builtin history append pending; builtin history clear-session; set -l cleared $status; builtin history search --exact pending; {}; test $cleared -ne 0; or exit 96; builtin history save; or exit 97; builtin history search",
        barrier(f, &ready, &release)
    );
    std::thread::scope(|scope| {
        let _release_on_panic = ReleaseOnDrop(&release);
        let writer = scope.spawn(|| evaluate(f, "n4_clear_retry", &code));
        ready.wait();
        assert_eq!(fs::read(&clear_target).unwrap(), clear_before);
        drop(deny_clear);
        release.signal();
        let result = writer.join().unwrap();
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        assert_eq!(result.out, b"pending\nbaseline\n");
        assert!(
            !result.err.iter().all(u8::is_ascii_whitespace),
            "failed session clear lacked diagnostics"
        );
    });
    assert_eq!(
        entries(evaluate(f, "n4_clear_retry", "builtin history search")),
        vec!["baseline"]
    );

    for stage in ["partial", "complete"] {
        let payload = f.root.join(format!("history-payload-{stage}"));
        fs::write(&payload, b"- cmd: unacknowledged\n  when: 456\n").unwrap();
        let ready = Event::new();
        let release = Event::new();
        // The transaction helper, not a simulated history store, owns the interrupted save.
        let mut command = std::process::Command::new(&f.probe);
        command.args([
            OsStr::new("n4-rewrite"),
            target.as_os_str(),
            payload.as_os_str(),
            OsStr::new(&ready.name),
            OsStr::new(&release.name),
            OsStr::new(stage),
        ]);
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut writer = InterruptedWriter(command.spawn().unwrap());
        ready.wait();
        let prior = fs::read(&target).unwrap();
        writer.0.kill().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(writer.0.as_raw_handle(), LIMIT.as_millis() as u32) },
            0,
            "interrupted writer did not exit"
        );
        writer.0.wait().unwrap();
        assert_eq!(fs::read(&target).unwrap(), prior);
        assert_eq!(
            entries(evaluate(f, "n4_denied", "builtin history search")),
            vec!["pending", "preserved"]
        );
    }
}
