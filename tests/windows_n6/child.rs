use super::{
    Family,
    cases_probe::{Event, RetainedProcess},
    editor::{capture, finish, start},
    fixture::{Fixture, quote},
};
use std::fs;
pub(super) const FAMILIES: &[Family] = &[
    ("tui-exclusive-input-protocol-mode-reclaim", tui),
    ("failed-launch-editor-recovery", failed_launch),
    ("ignored-break-keeps-donation-until-release", cancellation),
    ("control-handler-explicit-exit-seven", handler_exit),
    ("interactive-background-nul-file-pipe", background),
];
fn parse_modes(bytes: Vec<u8>) -> (u32, u32) {
    let text = String::from_utf8(bytes).unwrap();
    let (i, o) = text.split_once(':').unwrap();
    (i.parse().unwrap(), o.parse().unwrap())
}
fn tui(f: &Fixture) {
    let release = Event::new("tui-release");
    let controlled = Event::new("tui-control");
    let mut s = start(f, "");
    let editor = s.snapshot();
    s.send(
        format!(
            "{} n6-case-tui {} {} {}; printf '%s' $status > child-status\r",
            f.probe(),
            quote(f.root.to_str().unwrap()),
            quote(&release.name),
            quote(&controlled.name)
        )
        .as_bytes(),
    );
    s.wait_file(&f.root.join("child-ready"));
    s.wait_protocol(false);
    s.wait_text(b"N6-CHILD-FIRST-READ");
    let trace = s.trace();
    let first = trace
        .windows(b"N6-CHILD-FIRST-READ".len())
        .position(|x| x == b"N6-CHILD-FIRST-READ")
        .unwrap();
    let off = trace[..first]
        .windows(8)
        .rposition(|x| x == b"\x1b[?2004l")
        .expect("paste protocol disabled before child first read");
    assert!(
        !trace[off + 8..first]
            .windows(8)
            .any(|x| x == b"\x1b[?2004h")
    );
    let donated = parse_modes(fs::read(f.root.join("child-original-modes")).unwrap());
    let changed = parse_modes(fs::read(f.root.join("child-changed-modes")).unwrap());
    assert_ne!(changed, donated, "child must actually change native modes");
    let native = s.snapshot();
    assert_eq!((native.input_mode, native.output_mode), changed);
    // Host sends raw paste because fish has disabled the envelope before child input.
    s.paste("TUI-雪🐟");
    s.send(b"\r");
    s.wait_file(&f.root.join("child-read-complete"));
    assert_eq!(
        fs::read(f.root.join("child-input")).unwrap(),
        "TUI-雪🐟".as_bytes()
    );
    assert!(!f.root.join("child-status").exists());
    release.set();
    assert_eq!(s.wait_file(&f.root.join("child-status")), b"0");
    s.wait_protocol(true);
    let reclaimed = s.snapshot();
    assert_eq!(
        (reclaimed.input_mode, reclaimed.output_mode),
        (editor.input_mode, editor.output_mode)
    );
    s.send(b"restored-reader");
    capture(&mut s, f, 1, "restored-reader");
    let t = finish(s);
    assert_eq!(
        donated, t.original_modes,
        "first child instructions must observe original donated modes"
    );
}
fn failed_launch(f: &Fixture) {
    let mut s = start(f, "");
    let before = s.snapshot();
    let invalid = f.root.join("invalid-child.exe");
    fs::write(&invalid, b"owned invalid Windows executable\r\n").unwrap();
    // Missing resolution never donates; an existing invalid executable reaches
    // CreateProcessW after donation and must unwind the same native ownership.
    for (n, executable) in [f.root.join("missing-child.exe"), invalid]
        .into_iter()
        .enumerate()
    {
        s.send(
            format!(
                "{}; printf '%s' $status > failed-status-{n}\r",
                quote(executable.to_str().unwrap())
            )
            .as_bytes(),
        );
        assert_ne!(
            s.wait_file(&f.root.join(format!("failed-status-{n}"))),
            b"0"
        );
        s.wait_protocol(true);
        let after = s.snapshot();
        assert_eq!(
            (before.input_mode, before.output_mode),
            (after.input_mode, after.output_mode)
        );
        s.send(b"still-editable");
        capture(&mut s, f, n * 2 + 1, "still-editable");
        s.send(b"\x03");
        capture(&mut s, f, n * 2 + 2, "");
    }
    finish(s);
}
fn cancellation(f: &Fixture) {
    let release = Event::new("ignore-release");
    let controlled = Event::new("ignore-control");
    let mut s = start(f, "");
    let editor = s.snapshot();
    s.send(
        format!(
            "{} n6-case-ignore-control {} {} {}; printf released > ignored-status\r",
            f.probe(),
            quote(f.root.to_str().unwrap()),
            quote(&release.name),
            quote(&controlled.name)
        )
        .as_bytes(),
    );
    s.wait_file(&f.root.join("child-ready"));
    let pid = fs::read_to_string(f.root.join("child-pid")).unwrap().parse().unwrap();
    let child = RetainedProcess::acknowledged(pid);
    child.running("ignored-control child must be alive before receipt");
    s.wait_protocol(false);
    let before = s.snapshot();
    s.control(1);
    controlled.wait();
    child.running("real receipt must not finish the ignoring child");
    // The real handler acknowledgement is the barrier; no grace period or sleep.
    let held = s.snapshot();
    assert_eq!(
        (held.input_mode, held.output_mode),
        (before.input_mode, before.output_mode)
    );
    assert!(
        !f.root.join("ignored-status").exists(),
        "ignored cancellation must not release shell ownership"
    );
    let trace = s.trace();
    let off = trace
        .windows(8)
        .rposition(|x| x == b"\x1b[?2004l")
        .expect("protocol disabled for child");
    assert!(!trace[off + 8..].windows(8).any(|x| x == b"\x1b[?2004h"));
    release.set();
    let raw_status = child.wait_status("released ignored-control child");
    fs::write(f.root.join("ignored-native-status"), format!("0x{raw_status:08X}")).unwrap();
    assert_eq!(raw_status, 0, "receipt provenance must not rewrite an ordinary native exit");
    s.wait_protocol(true);
    let after = s.snapshot();
    assert_eq!(
        (after.input_mode, after.output_mode),
        (editor.input_mode, editor.output_mode)
    );
    assert!(!f.root.join("ignored-status").exists(),
        "callback rearm and native exit zero must not resume the cancelled command tail");
    s.send(b"printf '%s' $status > ignored-reader-status\r");
    s.wait_file(&f.root.join("ignored-reader-status"));
    assert_eq!(fs::read(f.root.join("ignored-reader-status")).unwrap(), b"130",
        "fresh reader observes the cancelled evaluator, independently of native DWORD zero");
    s.send(b"after-break");
    capture(&mut s, f, 1, "after-break");
    finish(s);
}
fn handler_exit(f: &Fixture) {
    let mut s = start(
        f,
        "function n6_break --on-signal CTRL_C; exit 7; end; or exit 91",
    );
    capture(&mut s, f, 1, "");
    s.control(0);
    let t = s.finish(7);
    assert_eq!(t.original_modes, t.final_modes);
    let mut s = start(
        f,
        "set -g n6_capture 1; function n6_break --on-signal CTRL_C; exit 7; end; or exit 91; fish_vi_key_bindings; bind -M insert f12 n6_capture; bind -M default f12 n6_capture; bind -M default f forward-jump; function n6_vi_default --on-variable fish_bind_mode; if test $fish_bind_mode = default; printf ready > vi-default-ready; end; end",
    );
    capture(&mut s, f, 2, "");
    assert_eq!(fs::read(f.root.join("mode-2")).unwrap(), b"insert");
    s.send(b"find argument");
    capture(&mut s, f, 3, "find argument");
    s.key(true, 1, 0x1b, 0, 0x1b, 0);
    s.wait_file(&f.root.join("vi-default-ready"));
    capture(&mut s, f, 4, "find argument");
    assert_eq!(fs::read(f.root.join("mode-4")).unwrap(), b"default");
    s.key(true, 1, 0x46, 0, b'f' as u16, 0);
    // Observe native record consumption before receipt. The installed single-key
    // find binding has no longer-sequence ambiguity or sleep-based scheduling.
    s.wait_input_drained();
    s.control(0);
    let t = s.finish(7);
    assert_eq!(t.original_modes, t.final_modes);
}
fn background(f: &Fixture) {
    let mut s = start(f, "");
    let root = |name: &str| quote(f.root.join(name).to_str().unwrap());
    s.send(
        format!(
            "{} n6-case-stdin {} {} &\r",
            f.probe(),
            root("nul-data"),
            root("nul-ready")
        )
        .as_bytes(),
    );
    s.wait_file(&f.root.join("nul-ready"));
    assert_eq!(fs::read(f.root.join("nul-data")).unwrap(), b"");
    s.send(b"printf typed > bg-typed\r");
    assert_eq!(s.wait_file(&f.root.join("bg-typed")), b"typed");
    fs::write(f.root.join("stdin-source"), "explicit-雪").unwrap();
    s.send(
        format!(
            "{} n6-case-stdin {} {} < {} &\r",
            f.probe(),
            root("file-data"),
            root("file-ready"),
            root("stdin-source")
        )
        .as_bytes(),
    );
    s.wait_file(&f.root.join("file-ready"));
    assert_eq!(
        fs::read(f.root.join("file-data")).unwrap(),
        "explicit-雪".as_bytes()
    );
    s.send(
        format!(
            "printf pipe-data | {} n6-case-stdin {} {} &\r",
            f.probe(),
            root("pipe-data"),
            root("pipe-ready")
        )
        .as_bytes(),
    );
    s.wait_file(&f.root.join("pipe-ready"));
    assert_eq!(fs::read(f.root.join("pipe-data")).unwrap(), b"pipe-data");
    s.send(b"wait; printf drained > bg-complete\r");
    assert_eq!(s.wait_file(&f.root.join("bg-complete")), b"drained");
    finish(s);
}
