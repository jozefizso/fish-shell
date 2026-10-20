//! Real console regressions for editor-owned evaluator input and nested consumers.
use super::{Family, console::Session, editor::{capture, finish, start}, fixture::{Fixture, quote}};
use std::ffi::{OsStr, c_void};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const FAMILIES: &[Family] = &[
    ("typed-cancel-cpu-binding-prefix-suffix-recovery", cpu_binding),
    ("typed-cancel-initial-paused-editor-prompt", initial_prompt),
    ("typed-cancel-evaluated-completion-condition", completion_condition),
    ("typed-cancel-evaluated-completion-provider", completion_provider),
    ("typed-cancel-evaluated-event-handler", event_callback),
    ("typed-cancel-native-universal-notification-callback", universal_callback),
    ("typed-cancel-startup-universal-notification-callback", startup_universal_callback),
    ("typed-cancel-blocked-native-pipe-read", blocked_io),
    ("typed-cancel-blocked-native-job-wait", blocked_wait),
    ("nonconsuming-typeahead-nested-read", nested_read),
    ("nonconsuming-typeahead-nested-raw-null-read", nested_raw_read),
    ("nonconsuming-typeahead-native-child-donation", child_typeahead),
    ("pasted-physical-etx-does-not-cancel-or-execute", paste_etx),
    ("partial-paste-framing-survives-event-evaluation", partial_frame),
    ("bounded-backlog-abort-reaches-typed-cancel", backlog_normal),
    ("bounded-backlog-preserves-consumed-paste-begin", backlog_paste),
    ("ambiguous-three-escape-prefix-reaches-typed-cancel", three_escapes),
    ("ambiguous-native-escape-repeat-reaches-typed-cancel", repeated_escapes),
    ("typed-cancel-native-control-race-real-receipts", native_race),
];

const RECEIPTS: &str = "set -g n7_native_count 0; function n7_control --on-signal CTRL_C; set -g n7_native_count (math $n7_native_count + 1); printf '%s' $n7_native_count > native-count; end";
const SPIN: &str = "function n7_spin; printf entered > entered; while true; end; printf bad > callback-tail; end; bind f2 n7_spin";

fn init(extra: &str) -> String {
    format!("{RECEIPTS}; {extra}")
}
fn f2(s: &mut Session) {
    s.key(true, 1, 0x71, 0, 0, 0);
}
fn records(text: &str) -> Vec<(bool, u16, u16, u16, u16, u32)> {
    text.encode_utf16().map(|unit| (true, 1, 0, 0, unit, 0)).collect()
}
fn typed_cancel(s: &mut Session, prefix: &str, suffix: &str) {
    let mut keys = records(prefix);
    keys.push((true, 1, 0x43, 0x2e, 3, 8));
    keys.extend(records(suffix));
    // One WriteConsoleInputW call establishes an exact prefix/cancel/suffix FIFO.
    s.key_records(&keys);
}
fn editor_owned(s: &mut Session) {
    assert_eq!(s.snapshot().input_mode & 1, 0, "callbacks must retain raw input");
    s.wait_protocol(true);
}
fn recovered(s: &mut Session, f: &Fixture, receipt_count: usize) {
    editor_owned(s);
    s.send(b"\x03printf '%s' $n7_native_count > receipts; printf recovered > recovered\r");
    assert_eq!(s.wait_file(&f.root.join("recovered")), b"recovered");
    assert_eq!(fs::read(f.root.join("receipts")).unwrap(), receipt_count.to_string().as_bytes());
    assert!(!f.root.join("callback-tail").exists(), "cancelled callback continued");
}

fn cpu_binding(f: &Fixture) {
    let mut s = start(f, &init(SPIN));
    s.key_records(&records("old-edit"));
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    typed_cancel(&mut s, "discard-this-prefix", "suffix-雪🐟");
    capture(&mut s, f, 1, "suffix-雪🐟");
    recovered(&mut s, f, 0);
    finish(s);
}
fn initial_prompt(f: &Fixture) {
    let code = "function fish_prompt; if not set -q n7_prompt_once; set -g n7_prompt_once 1; printf entered > entered; while true; end; printf bad > prompt-tail; end; printf 'N7> '; end";
    let mut s = start(f, &init(code));
    s.wait_file(&f.root.join("entered"));
    editor_owned(&mut s);
    typed_cancel(&mut s, "initial-prefix", "after-initial-雪");
    capture(&mut s, f, 1, "after-initial-雪");
    assert!(!f.root.join("prompt-tail").exists());
    recovered(&mut s, f, 0);
    finish(s);
}
fn completion_condition(f: &Fixture) {
    let mut s = start(f, &init("function n7_condition; printf entered > entered; while true; end; printf bad > callback-tail; end; complete -c n7_candidate -f -n n7_condition -a result"));
    s.send(b"n7_candidate \t");
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    typed_cancel(&mut s, "", "");
    capture(&mut s, f, 1, "");
    recovered(&mut s, f, 0);
    finish(s);
}
fn completion_provider(f: &Fixture) {
    let mut s = start(f, &init("function n7_provider; printf entered > entered; while true; end; printf bad > callback-tail; printf result; end; complete -c n7_candidate -f -a '(n7_provider)'"));
    s.send(b"n7_candidate \t");
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    typed_cancel(&mut s, "", "");
    capture(&mut s, f, 1, "");
    recovered(&mut s, f, 0);
    finish(s);
}
fn event_callback(f: &Fixture) {
    let mut s = start(f, &init("function n7_event --on-event n7-spin; printf entered > entered; while true; end; printf bad > callback-tail; end; bind f2 'emit n7-spin'"));
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    typed_cancel(&mut s, "", "");
    capture(&mut s, f, 1, "");
    recovered(&mut s, f, 0);
    finish(s);
}
fn universal_callback(f: &Fixture) {
    universal_callback_at(f, false);
}
fn startup_universal_callback(f: &Fixture) {
    universal_callback_at(f, true);
}
fn universal_callback_at(f: &Fixture, startup: bool) {
    let setup = super::editor::init(&init(
        "function n7_uvar_spin --on-variable n7_uvar_trigger; printf entered > entered; while true; end; printf bad > callback-tail; end",
    ));
    let command = f.command(&[OsStr::new("-i"), OsStr::new("-C"), OsStr::new(&setup)]);
    let mut s = Session::start(&command, &f.probe, &f.root);
    s.wait_protocol(true);
    if !startup {
        s.send(b"old-uvar-edit");
        capture(&mut s, f, 1, "old-uvar-edit");
    }
    let result = super::fixture::run(
        &mut f.command(&[OsStr::new("-c"), OsStr::new("set -U n7_uvar_trigger go")]),
        b"",
    );
    assert_eq!(result.code, 0, "universal notification writer: {}", String::from_utf8_lossy(&result.err));
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    typed_cancel(&mut s, "notification-prefix", "after-uvar-雪");
    capture(&mut s, f, if startup { 1 } else { 2 }, "after-uvar-雪");
    recovered(&mut s, f, 0);
    finish(s);
}


fn blocked_io(f: &Fixture) {
    let release = Release::new();
    let available = Release::new();
    let pipe = format!("\\\\.\\pipe\\fish-n7-eval-{}", release.id);
    let mut server = OwnedProbe::start(f, &[
        OsStr::new("n7-eval-pipe-server"), OsStr::new(&pipe),
        OsStr::new("pipe-ready"), OsStr::new("pipe-consumed"),
        OsStr::new(&release.name), OsStr::new(&available.name),
    ]);
    // Device namespace redirections are intentionally rejected by N1. Import a
    // real retained pipe before evaluator root-descriptor initialization in a
    // console probe instead of bypassing that product filesystem policy.
    let command = probe_command(f, &[
        OsStr::new("n7-eval-io-client"), OsStr::new(&pipe),
        OsStr::new(&available.name), OsStr::new("io-result"),
    ]);
    let mut s = Session::start(&command, &f.probe, &f.root);
    s.wait_protocol(true);
    assert_eq!(s.wait_file(&f.root.join("pipe-consumed")), b"consumed");
    editor_owned(&mut s);
    typed_cancel(&mut s, "", "");
    assert_eq!(s.wait_file(&f.root.join("io-result")), b"cancelled:recovered:no-receipt");
    release.set();
    server.finish(0);
    let transcript = s.finish(0);
    assert_eq!(transcript.original_modes, transcript.final_modes);
}
fn blocked_wait(f: &Fixture) {
    let release = Release::new();
    let code = format!("{} n7-eval-hold child-ready {} 37 &; set -g n7_child $last_pid; function n7_child_exit --on-process-exit $n7_child; printf '%s' $argv[3] > child-status; end; function n7_wait; printf entered > entered; wait $n7_child; printf bad > callback-tail; end; bind f2 n7_wait", f.probe(), quote(&release.name));
    let mut s = start(f, &init(&code));
    assert_eq!(s.wait_file(&f.root.join("child-ready")), b"ready");
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    typed_cancel(&mut s, "ordinary-queued-input", "");
    capture(&mut s, f, 1, "");
    release.set();
    s.send(b"wait $n7_child; printf drained > child-drained\r");
    assert_eq!(s.wait_file(&f.root.join("child-drained")), b"drained");
    assert_eq!(s.wait_file(&f.root.join("child-status")), b"37", "local cancel must not invent child exit status");
    recovered(&mut s, f, 0);
    finish(s);
}

fn nested_read(f: &Fixture) {
    nested(f, false);
}
fn nested_raw_read(f: &Fixture) {
    nested(f, true);
}
fn nested(f: &Fixture, raw: bool) {
    let read = if raw { "read --null --local value" } else { "read --local value" };
    let code = format!("function n7_nested; printf entered > entered; while not test -e release; end; {read}; printf '%s' \"$value\" > nested-result; end; bind f2 n7_nested");
    let mut s = start(f, &init(&code));
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    let expected = if raw { "fifo-雪🐟\nsecond-line" } else { "fifo-雪🐟" };
    let mut keys = records(expected);
    if raw {
        keys.push((true, 1, 0, 0, 0, 0));
    } else {
        keys.push((true, 1, 0x0d, 0, 13, 0));
    }
    s.key_records(&keys);
    fs::write(f.root.join("release"), b"release").unwrap();
    assert_eq!(s.wait_file(&f.root.join("nested-result")), expected.as_bytes());
    capture(&mut s, f, 1, "");
    recovered(&mut s, f, 0);
    finish(s);
}
fn child_typeahead(f: &Fixture) {
    let code = format!("function n7_child; printf entered > entered; while not test -e release; end; {} n7-eval-stdin child-ready child-result; end; bind f2 n7_child", f.probe());
    let mut s = start(f, &init(&code));
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    let mut keys = records("child-fifo-雪🐟");
    keys.push((true, 1, 0x0d, 0, 13, 0));
    s.key_records(&keys);
    fs::write(f.root.join("release"), b"release").unwrap();
    assert_eq!(s.wait_file(&f.root.join("child-result")), "child-fifo-雪🐟\r\n".as_bytes());
    capture(&mut s, f, 1, "");
    recovered(&mut s, f, 0);
    finish(s);
}

fn paste_etx(f: &Fixture) {
    let mut s = start(f, &init("function n7_paste; printf entered > entered; while not test -e release; end; printf finished > callback-finished; end; bind f2 n7_paste"));
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    editor_owned(&mut s);
    let mut keys = records("\x1b[200~printf unsafe > pasted-effect\n");
    keys.push((true, 1, 0x43, 0x2e, 3, 8));
    keys.extend(records("printf pending\x1b[201~"));
    s.key_records(&keys);
    fs::write(f.root.join("release"), b"release").unwrap();
    assert_eq!(s.wait_file(&f.root.join("callback-finished")), b"finished", "pasted ETX cancelled evaluator");
    capture(&mut s, f, 1, "printf unsafe > pasted-effect\nprintf pending");
    assert!(!f.root.join("pasted-effect").exists());
    recovered(&mut s, f, 0);
    finish(s);
}
fn event_setup(f: &Fixture, release: &Release, body: &str) -> String {
    format!("{} n7-eval-hold child-ready {} 0 &; set -g n7_trigger $last_pid; function n7_partial --on-process-exit $n7_trigger; printf entered > entered; {body}; end", f.probe(), quote(&release.name))
}
fn partial_frame(f: &Fixture) {
    let release = Release::new();
    let code = event_setup(f, &release, "while not test -e release; end; printf finished > callback-finished");
    let mut s = start(f, &init(&code));
    s.wait_file(&f.root.join("child-ready"));
    s.key_records(&records("\x1b[20"));
    s.wait_input_drained(); // The outer consumer really consumed a partial begin.
    release.set();
    s.wait_file(&f.root.join("entered"));
    let mut keys = records("0~printf unsafe > partial-effect\n");
    keys.push((true, 1, 0x43, 0x2e, 3, 8));
    keys.extend(records("printf pending\x1b[201"));
    s.key_records(&keys);
    let _ = s.snapshot();
    s.key_records(&records("~"));
    fs::write(f.root.join("release"), b"release").unwrap();
    assert_eq!(s.wait_file(&f.root.join("callback-finished")), b"finished");
    capture(&mut s, f, 1, "printf unsafe > partial-effect\nprintf pending");
    assert!(!f.root.join("partial-effect").exists());
    recovered(&mut s, f, 0);
    finish(s);
}
fn backlog_normal(f: &Fixture) {
    let mut s = start(f, &init(SPIN));
    f2(&mut s);
    s.wait_file(&f.root.join("entered"));
    typed_cancel(&mut s, &"x".repeat(5000), "after-backlog-雪");
    capture(&mut s, f, 1, "after-backlog-雪");
    recovered(&mut s, f, 0);
    finish(s);
}
fn backlog_paste(f: &Fixture) {
    let release = Release::new();
    let code = event_setup(f, &release, "while true; end; printf bad > callback-tail");
    let mut s = start(f, &init(&code));
    s.wait_file(&f.root.join("child-ready"));
    s.key_records(&records("\x1b[200~"));
    s.wait_input_drained(); // InputData already owns the paste begin/undo group.
    release.set();
    s.wait_file(&f.root.join("entered"));
    let text = format!("{}\nprintf unsafe > backlog-effect", "x".repeat(5000));
    let mut keys = records(&text);
    keys.push((true, 1, 0x43, 0x2e, 3, 8));
    keys.extend(records("\x1b[201~"));
    s.key_records(&keys);
    capture(&mut s, f, 1, &text);
    assert!(!f.root.join("backlog-effect").exists());
    typed_cancel(&mut s, "", "");
    capture(&mut s, f, 2, "");
    recovered(&mut s, f, 0);
    finish(s);
}
fn three_escapes(f: &Fixture) {
    ambiguous_escapes(f, false);
}
fn repeated_escapes(f: &Fixture) {
    ambiguous_escapes(f, true);
}
fn ambiguous_escapes(f: &Fixture, repeated_record: bool) {
    let mut s = start(f, &init(SPIN));
    f2(&mut s);
    s.wait_file(&f.root.join("entered"));
    editor_owned(&mut s);
    let mut keys = if repeated_record {
        vec![(true, 3, 0, 0, 0x1b, 0)]
    } else {
        records("\x1b\x1b\x1b")
    };
    keys.extend(records("[200~"));
    keys.push((true, 1, 0x43, 0x2e, 3, 8));
    keys.extend(records("after-ambiguous-雪"));
    // The monitor must return ownership to the actual decoder, not invent a
    // paste after three ESC bytes and leave this CPU binding uninterruptible.
    s.key_records(&keys);
    capture(&mut s, f, 1, "after-ambiguous-雪");
    recovered(&mut s, f, 0);
    finish(s);
}
fn native_race(f: &Fixture) {
    let mut s = start(f, &init(SPIN));
    f2(&mut s);
    s.wait_file(&f.root.join("entered"));
    typed_cancel(&mut s, "queued-prefix", "");
    // A genuine native callback may race either the peek or its exact-prefix
    // NOWAIT discard. It must survive local epoch teardown exactly once.
    s.control(0);
    assert_eq!(s.wait_file(&f.root.join("native-count")), b"1");
    capture(&mut s, f, 1, "");
    let code = format!("{} exit 3221225786; set -l child_status $status; printf '%s:%s' $child_status $n7_native_count > raw-status\r", f.probe());
    s.send(code.as_bytes());
    assert_eq!(s.wait_file(&f.root.join("raw-status")), b"1:1", "high native DWORD is not another control receipt");
    recovered(&mut s, f, 1);
    finish(s);
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
}
struct Release {
    id: String,
    name: String,
    handle: OwnedHandle,
}
impl Release {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let name = format!("Local\\fish-n7-eval-{id}");
        let wide: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(!handle.is_null(), "CreateEventW: {}", std::io::Error::last_os_error());
        Self { id, name, handle: unsafe { OwnedHandle::from_raw_handle(handle) } }
    }
    fn set(&self) {
        assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0);
    }
}
fn probe_command(f: &Fixture, args: &[&OsStr]) -> Command {
    let environment = f.command(&[]);
    let mut command = Command::new(&f.probe);
    command.args(args).current_dir(&f.root).env_clear();
    for (name, value) in environment.get_envs() {
        if let Some(value) = value {
            command.env(name, value);
        }
    }
    command
}
struct OwnedProbe(Option<Child>);
impl OwnedProbe {
    fn start(f: &Fixture, args: &[&OsStr]) -> Self {
        Self(Some(probe_command(f, args).spawn().unwrap()))
    }
    fn finish(&mut self, status: u32) {
        let child = self.0.as_mut().unwrap();
        assert_eq!(unsafe { WaitForSingleObject(child.as_raw_handle(), 30_000) }, 0, "owned pipe server did not finish");
        assert_eq!(child.wait().unwrap().code().unwrap() as u32, status);
        self.0.take();
    }
}
impl Drop for OwnedProbe {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
