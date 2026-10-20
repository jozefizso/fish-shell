//! Main-thread editor observation through actual commandline state and closed-file barriers.
use super::{console::Session, fixture::Fixture};
use std::{ffi::OsStr, fs};
// Capture the actual shared reader buffer, not the host's echo. A separate ready
// file is closed after all capture streams, making directory notification a barrier.
pub(super) fn init(extra: &str) -> String {
    format!(
        "set -g n6_capture 0; function fish_prompt; printf 'N6> '; end; function n6_capture; set -g n6_capture (math $n6_capture + 1); set -l n6_buffer; string match -rq '(?s)^(?<n6_buffer>.*)\\n$' -- (commandline | string collect -N); printf '%s' \"$n6_buffer\" > capture-$n6_capture; commandline --cursor > cursor-$n6_capture; printf '%s' $fish_bind_mode > mode-$n6_capture; printf ready > ready-$n6_capture; end; bind f12 n6_capture; {extra}"
    )
}
pub(super) fn start(f: &Fixture, extra: &str) -> Session {
    let init = init(extra);
    let cmd = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-i"),
        OsStr::new("-C"),
        OsStr::new(&init),
    ]);
    let mut s = Session::start(&cmd, &f.probe, &f.root);
    s.wait_protocol(true);
    s
}
pub(super) fn observe(s: &mut Session, f: &Fixture, n: usize) -> (String, usize) {
    s.send(b"\x1b[24~");
    assert_eq!(s.wait_file(&f.root.join(format!("ready-{n}"))), b"ready");
    let text = fs::read_to_string(f.root.join(format!("capture-{n}"))).unwrap();
    let cursor = fs::read_to_string(f.root.join(format!("cursor-{n}")))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (text, cursor)
}
#[track_caller]
pub(super) fn capture(s: &mut Session, f: &Fixture, n: usize, expected: &str) -> usize {
    let (text, cursor) = observe(s, f, n);
    assert_eq!(text, expected);
    cursor
}
pub(super) fn finish(mut s: Session) -> super::console::Transcript {
    s.send(b"\x03exit 0\r");
    let t = s.finish(0);
    assert_eq!(
        t.original_modes, t.final_modes,
        "normal exit must restore every mode bit"
    );
    assert!(
        t.bytes.windows(8).any(|x| x == b"\x1b[?2004l"),
        "actual paste protocol teardown missing"
    );
    t
}
