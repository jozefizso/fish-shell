use super::{Family, console::Session, fixture::Fixture};
use std::{ffi::OsStr, fs};
pub(super) const FAMILIES: &[Family] = &[
    ("utility-free-default-first-prompt", first_prompt),
    ("interactive-startup-order-private-login-profiles", startup),
    ("interactive-noexecute-and-option-errors", noexecute),
    ("native-record-unicode-repeats-modifiers", unicode),
    ("navigation-delete-undo-selection-kill-yank-modes", editing),
    ("multiline-paste-no-execute-single-undo", paste),
    ("viewport-resize-overrides-screen", resize),
    ("typed-native-control-recovery-and-eof", controls),
];
// Capture the actual shared reader buffer, not the host's echo. A separate ready
// file is closed after all capture streams, making directory notification a barrier.
pub(super) fn init(extra: &str) -> String {
    format!(
        "set -g n6_capture 0; function fish_prompt; printf 'N6> '; end; function n6_capture; set -g n6_capture (math $n6_capture + 1); printf '%s' (commandline | string collect) > capture-$n6_capture; commandline --cursor > cursor-$n6_capture; printf '%s' $fish_bind_mode > mode-$n6_capture; printf ready > ready-$n6_capture; end; bind f12 n6_capture; {extra}"
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
pub(super) fn capture(s: &mut Session, f: &Fixture, n: usize, expected: &str) -> usize {
    s.send(b"\x1b[24~");
    assert_eq!(s.wait_file(&f.root.join(format!("ready-{n}"))), b"ready");
    assert_eq!(
        fs::read(f.root.join(format!("capture-{n}"))).unwrap(),
        expected.as_bytes()
    );
    fs::read_to_string(f.root.join(format!("cursor-{n}")))
        .unwrap()
        .trim()
        .parse()
        .unwrap()
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
fn first_prompt(f: &Fixture) {
    let cmd = f.command(&[OsStr::new("-N"), OsStr::new("-i")]);
    let mut s = Session::start(&cmd, &f.probe, &f.root);
    s.wait_protocol(true);
    s.send(b"printf '%s' utility-freX\x7fe > first-result\r");
    assert_eq!(s.wait_file(&f.root.join("first-result")), b"utility-free");
    // The isolated PATH is empty: default prompt and bindings cannot depend on a
    // workstation utility. The next command also demonstrates a second cycle.
    s.send(b"printf '%s' second-cycle > second-result\r");
    assert_eq!(s.wait_file(&f.root.join("second-result")), b"second-cycle");
    // Preserved command-substitution status must still be negated, and must not
    // retain the previous pipeline's status vector or overwrite its own vector.
    s.send(b"false | true; not set n6_false (false); printf '%s:%s' $status $pipestatus > negated-false\r");
    assert_eq!(s.wait_file(&f.root.join("negated-false")), b"0:1");
    s.send(b"true | false; not set n6_true (true); printf '%s:%s' $status $pipestatus > negated-true\r");
    assert_eq!(s.wait_file(&f.root.join("negated-true")), b"1:0");
    finish(s);
}
fn startup(f: &Fixture) {
    fs::create_dir_all(f.state.join("config/fish/conf.d")).unwrap();
    fs::write(
        f.state.join("config/fish/conf.d/10-order.fish"),
        "printf 'snippet\\n' >> order\n",
    )
    .unwrap();
    fs::write(
        f.state.join("config/fish/config.fish"),
        "printf 'config\\n' >> order\n",
    )
    .unwrap();
    for (n, flags) in [
        vec!["-i", "-P", "-l"],
        vec!["--interactive", "--private", "--login"],
        vec!["--private", "--login"],
    ]
    .into_iter()
    .enumerate()
    {
        let order = format!("order-{n}");
        let init1 = format!("printf 'init-one\\n' >> order; {}", init(""));
        let init2 = "printf 'init-two\\n' >> order";
        let profile = f.root.join(format!("startup-{n}.profile"));
        let execution_profile = f.root.join(format!("execution-{n}.profile"));
        let execution = "printf 'execution\\n' >> order; status is-interactive; set -l i $status; status is-login; printf '%s:%s:%s' $i $status $fish_private_mode > startup-state";
        let mut args: Vec<&OsStr> = flags.iter().map(OsStr::new).collect();
        args.extend([
            OsStr::new("-C"),
            OsStr::new(&init1),
            OsStr::new("--init-command"),
            OsStr::new(init2),
            OsStr::new("--profile-startup"),
            profile.as_os_str(),
        ]);
        args.extend([
            OsStr::new(if n == 0 { "-p" } else { "--profile" }),
            execution_profile.as_os_str(),
        ]);
        // Explicit -ic must execute the requested command after the init sequence.
        if n == 0 {
            args.extend([OsStr::new("-c"), OsStr::new(execution)]);
        }
        let cmd = f.command(&args);
        let mut s = Session::start(&cmd, &f.probe, &f.root);
        if n == 0 {
            assert_eq!(s.wait_file(&f.root.join("startup-state")), b"0:0:1");
            let t = s.finish(0);
            assert_eq!(t.original_modes, t.final_modes);
        } else {
            s.wait_protocol(true);
            s.send(format!("{execution}\r").as_bytes());
            assert_eq!(s.wait_file(&f.root.join("startup-state")), b"0:0:1");
            finish(s);
        }
        fs::rename(f.root.join("order"), f.root.join(&order)).unwrap();
        assert_eq!(
            fs::read(f.root.join(order)).unwrap(),
            b"snippet\nconfig\ninit-one\ninit-two\nexecution\n"
        );
        assert!(fs::read_to_string(profile).unwrap().contains("init-one"));
        assert!(
            fs::read_to_string(execution_profile)
                .unwrap()
                .contains("startup-state")
        );
        fs::remove_file(f.root.join("startup-state")).unwrap();
    }
    // -N suppresses configuration, not repeated init commands or real input.
    let mut s = start(f, "printf init-only > no-config");
    assert_eq!(s.wait_file(&f.root.join("no-config")), b"init-only");
    s.send(b"printf interactive > no-config-command\r");
    assert_eq!(
        s.wait_file(&f.root.join("no-config-command")),
        b"interactive"
    );
    finish(s);
    assert!(!f.root.join("order").exists());
    let history = f.state.join("data/fish/fish_history");
    if history.exists() {
        assert!(
            !fs::read_to_string(history)
                .unwrap()
                .contains("startup-state"),
            "private interactive commands leaked to history"
        );
    }
}
fn noexecute(f: &Fixture) {
    fs::create_dir_all(f.state.join("config/fish")).unwrap();
    fs::write(
        f.state.join("config/fish/config.fish"),
        "printf bad > config-effect",
    )
    .unwrap();
    for flag in ["-n", "--no-execute"] {
        let cmd = f.command(&[
            OsStr::new("-i"),
            OsStr::new(flag),
            OsStr::new("-C"),
            OsStr::new("printf bad > init-effect"),
            OsStr::new("-c"),
            OsStr::new("printf bad > eval-effect"),
        ]);
        let t = Session::start(&cmd, &f.probe, &f.root).finish(0);
        assert_eq!(t.original_modes, t.final_modes);
        for name in ["config-effect", "init-effect", "eval-effect"] {
            assert!(!f.root.join(name).exists());
        }
    }
    for args in [
        vec!["-i", "--private=bad"],
        vec!["-i", "--login=bad"],
        vec!["-i", "--init-command"],
        vec!["-i", "--profile-startup"],
    ] {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let r = super::fixture::run(&mut f.command(&args), b"");
        assert_ne!(r.code, 0);
        assert!(!r.err.is_empty());
    }
    let absent = f.root.join("absent/profile.txt");
    let cmd = f.command(&[
        OsStr::new("-i"),
        OsStr::new("--profile-startup"),
        absent.as_os_str(),
        OsStr::new("-C"),
        OsStr::new("printf bad > failed-startup-effect"),
    ]);
    let t = Session::start(&cmd, &f.probe, &f.root).finish(2);
    assert_eq!(t.original_modes, t.final_modes);
    assert!(!f.root.join("failed-startup-effect").exists());
    assert!(!f.root.join("config-effect").exists());
    // Forced UI rejects ordinary pipes; -ic remains genuine interactive parser
    // evaluation without pretending those same pipes are console input.
    let r = super::fixture::run(
        &mut f.command(&[OsStr::new("-N"), OsStr::new("-i")]),
        b"printf bad > pipe-ui-effect\n",
    );
    assert_ne!(r.code, 0);
    assert!(!r.err.is_empty());
    assert!(!f.root.join("pipe-ui-effect").exists());
    let r = super::fixture::run(
        &mut f.command(&[
            OsStr::new("-N"),
            OsStr::new("-ic"),
            OsStr::new("status is-interactive; printf '%s' $status"),
        ]),
        b"",
    );
    super::fixture::exact(r, 0, b"0", b"");
}
fn unicode(f: &Fixture) {
    let mut s = start(f, "");
    s.key(true, 1, 0, 0, 0xd83d, 0);
    // A synchronous snapshot is a barrier between the UTF-16 halves.
    let _ = s.snapshot();
    s.key(true, 1, 0, 0, 0xdc1f, 0);
    s.key(false, 1, 0, 0, b'X' as u16, 0); // key-up is never inserted
    s.key(false, 1, 0x12, 0, 0x03a9, 0); // Alt+numpad key-up commits text
    s.key(true, 3, 0x41, 0, b'a' as u16, 0);
    s.key(true, 1, 0xde, 0, 0, 0); // dead-key record has no committed text
    s.key(true, 1, 0, 0, 0x00e9, 0);
    s.key(true, 1, 0x51, 0, '@' as u16, 0x0001 | 0x0008); // AltGr committed text
    for ch in "漢字e\u{301}".encode_utf16() {
        s.key(true, 1, 0, 0, ch, 0);
    }
    // Injection proves committed IME UTF-16 translation, not a physical IME UI.
    assert_eq!(capture(&mut s, f, 1, "🐟Ωaaaé@漢字e\u{301}"), 11);
    s.send(b"\x1b[");
    let _ = s.snapshot();
    s.send(b"D\x7f");
    capture(&mut s, f, 2, "🐟Ωaaaé@漢字\u{301}");
    s.send("\x03printf '%s' '雪🐟e\u{301}' > unicode-runtime\r".as_bytes());
    assert_eq!(
        s.wait_file(&f.root.join("unicode-runtime")),
        "雪🐟e\u{301}".as_bytes()
    );
    s.send(b"\x03");
    capture(&mut s, f, 3, "");
    // U+F600 is literal native text, not a parser sentinel. Movement must cross
    // its whole UTF-8 representation and backspace must remove the whole scalar.
    for ch in ['a', '\u{f600}', 'b'] {
        s.key(true, 1, 0, 0, ch as u16, 0);
    }
    capture(&mut s, f, 4, "a\u{f600}b");
    s.key(true, 1, 0x25, 0, 0, 0);
    s.key(true, 1, 0x25, 0, 0, 0);
    s.key(true, 1, 0x27, 0, 0, 0);
    capture(&mut s, f, 5, "a\u{f600}b");
    s.key(true, 1, 0x08, 0, 0x08, 0);
    capture(&mut s, f, 6, "ab");
    finish(s);
}
fn editing(f: &Fixture) {
    let bindings = "bind f3 undo; bind f4 redo; bind f5 begin-selection; bind f6 end-selection; bind f7 kill-selection; bind f8 yank";
    let mut s = start(f, bindings);
    s.send(b"abc\x1b[D\x7fZ\x1b[H0\x1b[F9");
    capture(&mut s, f, 1, "0aZc9");
    s.send(b"\x1b[H\x1b[3~");
    capture(&mut s, f, 2, "aZc9");
    s.send(b"\x1b[13~");
    capture(&mut s, f, 3, "0aZc9");
    s.send(b"\x1b[14~");
    capture(&mut s, f, 4, "aZc9");
    s.send(b"\x1b[H\x1b[15~\x1b[C\x1b[C\x1b[18~\x1b[17~");
    capture(&mut s, f, 5, "c9");
    s.send(b"\x1b[19~");
    capture(&mut s, f, 6, "aZc9");
    s.send(b"\x01\x0b\x19");
    capture(&mut s, f, 7, "aZc9");
    s.send(b"\x03bind ctrl-g 'commandline --insert MUTATED'; bind -M n6 f12 n6_capture; bind -M n6 x 'commandline --insert MODE'; bind -M n6 ctrl-x 'set fish_bind_mode default; commandline --replace \"\"'; bind ctrl-t 'set fish_bind_mode n6'\r");
    s.send(b"\x07\x14x");
    capture(&mut s, f, 8, "MUTATEDMODE");
    assert_eq!(fs::read(f.root.join("mode-8")).unwrap(), b"n6");
    s.send(b"\x18");
    capture(&mut s, f, 9, "");
    assert_eq!(fs::read(f.root.join("mode-9")).unwrap(), b"default");
    s.send(b"fish_vi_key_bindings; bind -M insert f12 n6_capture; bind -M default f12 n6_capture; function n6_vi_default --on-variable fish_bind_mode; if test $fish_bind_mode = default; printf ready > vi-default-ready; end; end; printf ready > vi-ready\r");
    s.wait_file(&f.root.join("vi-ready"));
    s.send(b"abcd");
    capture(&mut s, f, 10, "abcd");
    assert_eq!(fs::read(f.root.join("mode-10")).unwrap(), b"insert");
    s.key(true, 1, 0x1b, 0, 0x1b, 0);
    s.wait_file(&f.root.join("vi-default-ready"));
    s.send(b"hx");
    capture(&mut s, f, 11, "abd");
    assert_eq!(fs::read(f.root.join("mode-11")).unwrap(), b"default");
    s.send(b"iZ");
    capture(&mut s, f, 12, "abZd");
    assert_eq!(fs::read(f.root.join("mode-12")).unwrap(), b"insert");
    finish(s);
}
fn paste(f: &Fixture) {
    let mut s = start(
        f,
        "bind f3 undo; bind f4 redo; bind tab 'printf bad > paste-tab-binding'",
    );
    let text = "printf first > pasted-effect\nprintf second >> pasted-effect";
    s.paste(text);
    capture(&mut s, f, 1, text);
    assert!(!f.root.join("pasted-effect").exists());
    s.send(b"\x1b[13~");
    capture(&mut s, f, 2, "");
    s.send(b"\x1b[14~");
    capture(&mut s, f, 3, text);
    assert!(!f.root.join("pasted-effect").exists());
    s.send(b"\rprintf done > pasted-executed\r");
    assert_eq!(s.wait_file(&f.root.join("pasted-executed")), b"done");
    assert_eq!(
        fs::read(f.root.join("pasted-effect")).unwrap(),
        b"firstsecond"
    );
    // Inject the frame and text through the same ordered native record queue,
    // including actual physical Return/Tab records, not VT substitutes.
    let mut records: Vec<_> = "\x1b[200~printf escaped > native-paste-effect"
        .encode_utf16()
        .map(|ch| (true, 1, 0, 0, ch, 0))
        .collect();
    records.push((true, 1, 0x0d, 0, 0x0d, 0));
    records.push((true, 1, 0x09, 0, 0x09, 0));
    records.extend(
        "printf pending\x1b[201~"
            .encode_utf16()
            .map(|ch| (true, 1, 0, 0, ch, 0)),
    );
    s.key_records(&records);
    // Normal editor paste converts CR to a logical newline and strips Tab.
    // The capture acknowledgement proves all records were consumed before the
    // negative assertions; an escaped Enter would have created the file.
    capture(
        &mut s,
        f,
        4,
        "printf escaped > native-paste-effect\nprintf pending",
    );
    assert!(!f.root.join("native-paste-effect").exists());
    assert!(!f.root.join("paste-tab-binding").exists());
    finish(s);
}
fn resize(f: &Fixture) {
    let result = f.root.join("native-query-result");
    let mut command = std::process::Command::new(&f.probe);
    command
        .arg("n6-case-native-queries")
        .arg(&result)
        .current_dir(&f.root);
    let mut query_session = Session::start(&command, &f.probe, &f.root);
    assert_eq!(
        query_session.wait_file(&result),
        b"viewport=40:24;cursor=5:6;palette=123456;reverse=a1b2c3;offscreen=unknown"
    );
    query_session.finish(0);
    let mut s = start(f, "");
    s.send("echo 雪🐟 resize preserved".as_bytes());
    s.resize(61, 17);
    capture(&mut s, f, 1, "echo 雪🐟 resize preserved");
    let view = s.snapshot();
    assert_eq!(view.viewport[2] - view.viewport[0] + 1, 61);
    assert_eq!(view.viewport[3] - view.viewport[1] + 1, 17);
    assert!(view.cursor[0] >= view.viewport[0] && view.cursor[0] <= view.viewport[2]);
    assert!(String::from_utf16_lossy(&view.cells).contains("resize preserved"));
    s.send(
        b"\x03set -gx COLUMNS 43; set -gx LINES 11; printf '%s:%s' $COLUMNS $LINES > dimensions\r",
    );
    assert_eq!(s.wait_file(&f.root.join("dimensions")), b"43:11");
    s.resize(75, 23);
    s.send(b"printf '%s:%s' $COLUMNS $LINES > dimensions-override\r");
    assert_eq!(s.wait_file(&f.root.join("dimensions-override")), b"75:23");
    s.send(b"set -e COLUMNS LINES; printf ready > resize-unset\r");
    s.wait_file(&f.root.join("resize-unset"));
    s.resize(67, 19);
    s.send(b"printf '%s:%s' $COLUMNS $LINES > dimensions-native\r");
    assert_eq!(s.wait_file(&f.root.join("dimensions-native")), b"67:19");
    finish(s);
}
fn controls(f: &Fixture) {
    let mut s = start(f, "");
    s.send(b"printf bad > typed-effect\x03");
    capture(&mut s, f, 1, "");
    assert!(!f.root.join("typed-effect").exists());
    s.send(b"function n6_observe_control --on-signal CTRL_C; printf ready > actual-control-ready; end; or exit 91\r");
    s.send(b"printf bad > actual-effect");
    capture(&mut s, f, 2, "printf bad > actual-effect");
    s.control(0);
    s.wait_file(&f.root.join("actual-control-ready"));
    capture(&mut s, f, 3, "");
    assert!(!f.root.join("actual-effect").exists());
    s.send(b"printf recovered > control-result\r");
    assert_eq!(s.wait_file(&f.root.join("control-result")), b"recovered");
    s.send(b"\x04");
    let t = s.finish(0);
    assert_eq!(t.original_modes, t.final_modes);
}
