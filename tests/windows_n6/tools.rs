use super::{
    Family,
    console::Session,
    editor::{capture, finish, start},
    fixture::{Fixture, run},
};
use std::{fs, process::Command};
pub(super) const FAMILIES: &[Family] = &[
    ("key-reader-all-option-aliases-stream-errors", key_options),
    ("standalone-console-key-decoding-continuous", standalone),
    ("builtin-key-reader-nested-stream-ownership", builtin_reader),
    ("interactive-read-silent-nchars-newline-null-cancel", reads),
    ("commandline-buffer-context-binding-errors", commandline),
    ("nested-breakpoint-scope-context-restoration", breakpoint),
    ("private-logon-station-editor-clipboard", clipboard),
];
fn tool(f: &Fixture, args: &[&str]) -> Command {
    let source = f.command(&[]);
    let mut cmd = Command::new(f.root.join("fish_key_reader.exe"));
    cmd.args(args).env_clear().current_dir(&f.root);
    for (k, v) in source.get_envs() {
        if let Some(v) = v {
            cmd.env(k, v);
        }
    }
    cmd
}
fn key_options(f: &Fixture) {
    for flag in ["-h", "--help", "-v", "--version"] {
        let a = run(&mut tool(f, &[flag]), b"");
        let b = f.eval(&format!("fish_key_reader {flag}"));
        for r in [&a, &b] {
            assert_eq!(r.code, 0);
            assert!(!r.out.is_empty());
            assert!(r.err.is_empty());
        }
        if flag == "-h" || flag == "--help" {
            for option in ["--continuous", "--verbose", "--help", "--version"] {
                assert!(String::from_utf8_lossy(&a.out).contains(option));
                assert!(String::from_utf8_lossy(&b.out).contains(option));
            }
        }
    }
    for args in [
        vec!["--unknown"],
        vec!["--continuous=bad"],
        vec!["--verbose=bad"],
        vec!["--help=bad"],
        vec!["--version=bad"],
        vec!["argument"],
        vec!["--", "argument"],
        vec![],
    ] {
        let a = run(&mut tool(f, &args), b"not-a-console");
        let b = f.input(
            &format!("fish_key_reader {}", args.join(" ")),
            b"not-a-console",
        );
        for r in [&a, &b] {
            assert_ne!(r.code, 0);
            assert!(r.out.is_empty());
            assert!(!r.err.is_empty());
        }
    }
    // Redirected native output remains ordinary bytes, never terminal controls.
    let r = f.eval("fish_key_reader --help");
    assert!(!r.out.contains(&0x1b));
    let source = f.command(&[]);
    let mut cmd = Command::new(&f.probe);
    cmd.arg("n6-case-tool-streams")
        .arg(f.root.join("fish_key_reader.exe"))
        .arg(&f.root)
        .current_dir(&f.root)
        .env_clear();
    for (k, v) in source.get_envs() {
        if let Some(v) = v {
            cmd.env(k, v);
        }
    }
    let mut s = Session::start(&cmd, &f.probe, &f.root);
    assert_eq!(s.wait_file(&f.root.join("tool-streams-ready")), b"checked");
    let t = s.finish(0);
    assert_eq!(t.original_modes, t.final_modes);
}
fn standalone(f: &Fixture) {
    for (n, args, input, continuous, verbose) in [
        (0, vec![], "雪", false, false),
        (1, vec!["-V"], "🐟", false, true),
        (2, vec!["--verbose"], "x", false, true),
        (3, vec!["-c"], "quit", true, false),
        (4, vec!["--continuous", "--verbose"], "exit", true, true),
        (5, vec!["-c", "-V"], "\u{3}\u{3}", true, true),
        (6, vec!["--continuous"], "\u{4}\u{4}", true, false),
    ] {
        let evidence = f.root.join(format!("standalone-{n}"));
        fs::create_dir(&evidence).unwrap();
        let mut s = Session::start(&tool(f, &args), &f.probe, &evidence);
        s.wait_text(b"Press a key:");
        s.send(input.as_bytes());
        let t = s.finish(0);
        assert_eq!(t.original_modes, t.final_modes);
        let text = String::from_utf8_lossy(&t.bytes);
        assert!(text.contains("bind "));
        assert_eq!(text.contains("# decoded from:"), verbose);
        if continuous {
            assert!(text.contains("Exiting at your request."));
        } else if n == 0 {
            assert!(text.contains("bind 雪 "));
        } else if n == 1 {
            assert!(text.contains("bind 🐟 "));
        }
    }
    let evidence = f.root.join("standalone-invalid");
    fs::create_dir(&evidence).unwrap();
    let mut s = Session::start(&tool(f, &[]), &f.probe, &evidence);
    s.wait_text(b"Press a key:");
    s.key(true, 1, 0, 0, 0xdc1f, 0);
    let t = s.finish(1);
    assert_eq!(t.original_modes, t.final_modes);
    assert!(!String::from_utf8_lossy(&t.bytes).contains("bind "));
}
fn builtin_reader(f: &Fixture) {
    for (n, flags, input, verbose) in [
        (0, "", "雪", false),
        (1, "-V", "🐟", true),
        (2, "--verbose --continuous", "quit", true),
        (3, "-c", "\u{3}\u{3}", false),
    ] {
        let mut s = start(f, &format!("set -g n6_capture {n}"));
        let before = s.snapshot();
        s.send(
            format!(
                "fish_key_reader {flags} > key-out-{n}; printf '%s' $status > key-status-{n}\r"
            )
            .as_bytes(),
        );
        s.wait_text(b"Press a key:");
        s.send(input.as_bytes());
        assert_eq!(s.wait_file(&f.root.join(format!("key-status-{n}"))), b"0");
        let out = fs::read_to_string(f.root.join(format!("key-out-{n}"))).unwrap();
        assert!(out.contains("bind "));
        assert_eq!(out.contains("# decoded from:"), verbose);
        assert!(
            !out.contains("Press a key:"),
            "key-reader prompt belongs to stderr, not redirected stdout"
        );
        s.send(b"outer-buffer");
        capture(&mut s, f, n + 1, "outer-buffer");
        let after = s.snapshot();
        assert_eq!(
            (before.input_mode, before.output_mode),
            (after.input_mode, after.output_mode)
        );
        finish(s);
    }
    let mut s = start(f, "set -g n6_capture 4");
    let before = s.snapshot();
    s.send(b"fish_key_reader > malformed-key-output; printf '%s' $status > malformed-key-status\r");
    s.wait_text(b"Press a key:");
    s.key(true, 1, 0, 0, 0xdc1f, 0);
    assert_ne!(s.wait_file(&f.root.join("malformed-key-status")), b"0");
    assert!(
        fs::read(f.root.join("malformed-key-output"))
            .unwrap()
            .is_empty()
    );
    s.send(b"after-invalid-key");
    capture(&mut s, f, 5, "after-invalid-key");
    let after = s.snapshot();
    assert_eq!(
        (before.input_mode, before.output_mode),
        (after.input_mode, after.output_mode)
    );
    fs::write(f.root.join("key-redirected-input"), b"not-a-console").unwrap();
    s.send(b"\x03fish_key_reader < key-redirected-input > rejected-key-output 2> rejected-key-error; printf '%s' $status > rejected-key-status\r");
    assert_ne!(s.wait_file(&f.root.join("rejected-key-status")), b"0");
    assert!(
        fs::read(f.root.join("rejected-key-output"))
            .unwrap()
            .is_empty()
    );
    assert!(
        !fs::read(f.root.join("rejected-key-error"))
            .unwrap()
            .is_empty()
    );
    s.send(b"after-redirected-key-error");
    capture(&mut s, f, 6, "after-redirected-key-error");
    finish(s);
}
fn reads(f: &Fixture) {
    let mut s = start(f, "");
    let original = s.snapshot();
    s.send(b"read --prompt-str N6-READ-ONE: value; printf '%s:%s' $status $value > read-one\r");
    s.wait_text(b"N6-READ-ONE:");
    s.send("雪🐟 edited\x1b[D\x7fZ\r".as_bytes());
    assert_eq!(
        s.wait_file(&f.root.join("read-one")),
        "0:雪🐟 editZd".as_bytes()
    );
    s.send(b"read --silent --prompt-str N6-SECRET: secret; printf '%s:%s' $status $secret > read-secret\r");
    s.wait_text(b"N6-SECRET:");
    s.send(b"private-value");
    capture(&mut s, f, 1, "private-value");
    let screen = String::from_utf16_lossy(&s.snapshot().cells);
    assert!(
        !screen.contains("private-value"),
        "silent read displayed secret in real console cells"
    );
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("read-secret")), b"0:private-value");
    s.send(b"read --nchars 3 --prompt-str N6-NCHARS: count; printf '%s:%s' $status $count > read-count\r");
    s.wait_text(b"N6-NCHARS:");
    s.send("雪🐟a".as_bytes());
    assert_eq!(
        s.wait_file(&f.root.join("read-count")),
        "0:雪🐟a".as_bytes()
    );
    s.send(b"read --nchars 1 --prompt-str N6-RESERVED: reserved; printf '%s:%s' $status $reserved > read-reserved\r");
    s.wait_text(b"N6-RESERVED:");
    s.key(true, 1, 0, 0, 0xf600, 0);
    assert_eq!(
        s.wait_file(&f.root.join("read-reserved")),
        "0:\u{f600}".as_bytes()
    );
    for (n, flag) in [(0, "--line"), (1, "-L")] {
        s.send(format!("read {flag} -P N6-LINE-{n}: first second; printf '%s:%s:%s' $status $first $second > read-lines-{n}\r").as_bytes());
        s.wait_text(format!("N6-LINE-{n}:").as_bytes());
        s.send(b"first words\rsecond words\r");
        assert_eq!(
            s.wait_file(&f.root.join(format!("read-lines-{n}"))),
            b"0:first words:second words"
        );
    }
    let mut next_capture = 2;
    let mut recovered = |s: &mut Session| {
        s.wait_protocol(true);
        capture(s, f, next_capture, "");
        next_capture += 1;
        let after = s.snapshot();
        assert_eq!(
            (original.input_mode, original.output_mode),
            (after.input_mode, after.output_mode)
        );
    };
    let failed_empty = |result: Vec<u8>| {
        let result = String::from_utf8(result).unwrap();
        let (status, count) = result.split_once(':').unwrap();
        assert_ne!(status.parse::<i32>().unwrap(), 0);
        assert_eq!(count, "0", "failed read must clear the destination");
    };
    for (n, flag) in [(0, "--null"), (1, "-z")] {
        s.send(format!("bind tab 'printf bad > raw-tab-binding'; bind ctrl-g 'printf bad > raw-control-binding'; printf ready > read-null-ready-{n}; read {flag} --command startup --prompt 'printf bad > raw-prompt-effect' --right-prompt 'printf bad > raw-right-prompt-effect' --shell --silent nul; printf '%s:%s' $status $nul > read-null-{n}\r").as_bytes());
        s.wait_file(&f.root.join(format!("read-null-ready-{n}")));
        // Raw --null is not an editor: committed Tab/CR/control text, escape
        // sequences, and paste envelopes all survive without executing bindings.
        let raw = "first\r\tsecond\x07\x7f\x1b[D\x1b[16;1R\x1b[?61;6;7c\x1b[200~  pasted\tdata\r\x1b[201~";
        for ch in raw.encode_utf16() {
            let vk = match ch {
                0x09 => 0x09,
                0x0d => 0x0d,
                0x7f => 0x08, // ConPTY's physical Backspace commits DEL, not BS.
                _ => 0,
            };
            s.key(true, 1, vk, 0, ch, 0);
        }
        s.key(true, 1, 0x20, 0, 0, 8); // actual Ctrl-Space committed NUL
        assert_eq!(
            s.wait_file(&f.root.join(format!("read-null-{n}"))),
            format!("0:{raw}").as_bytes()
        );
        recovered(&mut s);
        for effect in [
            "raw-tab-binding",
            "raw-control-binding",
            "raw-prompt-effect",
            "raw-right-prompt-effect",
        ] {
            assert!(!f.root.join(effect).exists());
        }
    }
    s.send(b"printf ready > raw-nchars-ready; read --null --nchars 2 first; set -l first_status $status; read --null second; printf '%s:%s:%s:%s' $first_status $first $status $second > raw-nchars\r");
    s.wait_file(&f.root.join("raw-nchars-ready"));
    for ch in "🐟\u{f600}tail".encode_utf16() {
        s.key(true, 1, 0, 0, ch, 0);
    }
    s.key(true, 1, 0x20, 0, 0, 8);
    assert_eq!(
        s.wait_file(&f.root.join("raw-nchars")),
        "0:🐟\u{f600}:0:tail".as_bytes()
    );
    recovered(&mut s);
    s.send(b"set -g fish_read_limit 3; printf ready > raw-limit-ready; read --null limited; set -l read_status $status; printf '%s:%s' $read_status (count $limited) > raw-limit\r");
    s.wait_file(&f.root.join("raw-limit-ready"));
    for ch in "🐟".encode_utf16() {
        s.key(true, 1, 0, 0, ch, 0);
    }
    assert_eq!(s.wait_file(&f.root.join("raw-limit")), b"122:0");
    recovered(&mut s);
    s.send(b"printf ready > raw-limit-exact-ready; read --null limited; printf '%s:%s' $status $limited > raw-limit-exact; set -e fish_read_limit\r");
    s.wait_file(&f.root.join("raw-limit-exact-ready"));
    s.key(true, 1, 0, 0, '雪' as u16, 0);
    s.key(true, 1, 0x20, 0, 0, 8);
    assert_eq!(
        s.wait_file(&f.root.join("raw-limit-exact")),
        "0:雪".as_bytes()
    );
    recovered(&mut s);
    s.send(b"read -P N6-CANCEL: cancelled; printf '%s' $status > read-cancel\r");
    s.wait_text(b"N6-CANCEL:");
    s.send(b"discard\x03");
    assert_ne!(s.wait_file(&f.root.join("read-cancel")), b"0");
    s.send(b"printf ready > read-eof-ready; read --null partial; printf '%s:%s' $status $partial > read-eof\r");
    s.wait_file(&f.root.join("read-eof-ready"));
    s.send(b"partial\x04");
    assert_eq!(s.wait_file(&f.root.join("read-eof")), b"0:partial");
    recovered(&mut s);
    s.send(b"printf ready > read-empty-eof-ready; read --null empty; set -l read_status $status; printf '%s:%s' $read_status (count $empty) > read-empty-eof\r");
    s.wait_file(&f.root.join("read-empty-eof-ready"));
    s.send(b"\x04");
    failed_empty(s.wait_file(&f.root.join("read-empty-eof")));
    recovered(&mut s);
    s.send(b"printf ready > raw-cancel-ready; read --null cancelled; set -l read_status $status; printf '%s:%s' $read_status (count $cancelled) > raw-cancel\r");
    s.wait_file(&f.root.join("raw-cancel-ready"));
    s.send(b"discard\x03");
    failed_empty(s.wait_file(&f.root.join("raw-cancel")));
    recovered(&mut s);
    s.send(b"printf ready > raw-control-ready; read --null cancelled; set -l read_status $status; printf '%s:%s' $read_status (count $cancelled) > raw-control\r");
    s.wait_file(&f.root.join("raw-control-ready"));
    s.key(true, 1, 0, 0, b'x' as u16, 0);
    s.wait_input_drained();
    s.control(0);
    failed_empty(s.wait_file(&f.root.join("raw-control")));
    recovered(&mut s);
    s.send(b"read --line --null invalid 2> read-flags-error; printf '%s' $status > read-flags-status\r");
    assert_ne!(s.wait_file(&f.root.join("read-flags-status")), b"0");
    assert!(
        !fs::read(f.root.join("read-flags-error"))
            .unwrap()
            .is_empty()
    );
    s.send(b"read --nchars bad value; printf '%s' $status > read-error\r");
    assert_ne!(s.wait_file(&f.root.join("read-error")), b"0");
    s.send(b"outer-after-read");
    capture(&mut s, f, next_capture, "outer-after-read");
    let after = s.snapshot();
    assert_eq!(
        (original.input_mode, original.output_mode),
        (after.input_mode, after.output_mode)
    );
    finish(s);
    super::fixture::exact(
        f.input("read invalid; set -l read_status $status; printf '%s:' $read_status; printf '%s' $invalid; exit $read_status", b"\xff\n"),
        0,
        b"0:\xff",
        b"",
    );
}
fn commandline(f: &Fixture) {
    let script = "bind f2 'commandline --replace \"echo alpha | string upper\"; commandline --cursor 7'; bind f3 'commandline --current-token > token; commandline --current-process > process; commandline --current-job > job; commandline --line > line; commandline --column > column; printf ready > context-ready'; bind f4 'commandline --current-token --replace BETA; commandline --append \"; echo end\"'; bind f5 'commandline --cursor 0; commandline --insert \"PREFIX \"'; bind f6 'commandline --tokens-expanded --tokens-raw 2> context-error; printf \"%s\" $status > context-status'";
    let mut s = start(f, script);
    s.send(b"\x1bOQ\x1bOR");
    s.wait_file(&f.root.join("context-ready"));
    assert_eq!(fs::read(f.root.join("token")).unwrap(), b"alpha\n");
    assert_eq!(
        fs::read_to_string(f.root.join("process")).unwrap().trim(),
        "echo alpha"
    );
    assert_eq!(
        fs::read_to_string(f.root.join("job")).unwrap().trim(),
        "echo alpha | string upper"
    );
    assert_eq!(fs::read(f.root.join("line")).unwrap(), b"1\n");
    assert_eq!(fs::read(f.root.join("column")).unwrap(), b"8\n");
    s.send(b"\x1bOS\x1b[15~");
    capture(&mut s, f, 1, "PREFIX echo BETA | string upper; echo end");
    s.send(b"\x1b[17~");
    assert_ne!(s.wait_file(&f.root.join("context-status")), b"0");
    assert!(!fs::read(f.root.join("context-error")).unwrap().is_empty());
    capture(&mut s, f, 2, "PREFIX echo BETA | string upper; echo end");
    s.send(b"\x03bind ctrl-g 'commandline --insert live'; bind --erase ctrl-g; bind ctrl-g 'commandline --insert changed'\r\x07");
    capture(&mut s, f, 3, "changed");
    finish(s);
    let r = f.eval("commandline");
    assert_ne!(r.code, 0);
    assert!(!r.err.is_empty());
    let r = f.eval("breakpoint");
    assert_ne!(r.code, 0);
    assert!(!r.err.is_empty());
}
fn breakpoint(f: &Fixture) {
    let mut s = start(
        f,
        "function n6_inner; set -l scoped inner; breakpoint; printf '%s' $scoped > inner-restored; end; function n6_outer; set -l scoped outer; breakpoint; printf '%s' $scoped > outer-restored; end",
    );
    let before = s.snapshot();
    s.send(b"n6_outer\r");
    s.send(b"printf '%s' $scoped > outer-context\r");
    assert_eq!(s.wait_file(&f.root.join("outer-context")), b"outer");
    s.send(b"n6_inner\r");
    s.send(b"printf '%s' $scoped > inner-context\r");
    assert_eq!(s.wait_file(&f.root.join("inner-context")), b"inner");
    s.send(b"read --nchars invalid bad; printf '%s' $status > nested-error\r");
    assert_ne!(s.wait_file(&f.root.join("nested-error")), b"0");
    s.send(b"\x04");
    assert_eq!(s.wait_file(&f.root.join("inner-restored")), b"inner");
    s.send(b"printf '%s' $scoped > outer-again\r");
    assert_eq!(s.wait_file(&f.root.join("outer-again")), b"outer");
    s.send(b"\x04");
    assert_eq!(s.wait_file(&f.root.join("outer-restored")), b"outer");
    s.send(b"after-nested-breakpoint");
    capture(&mut s, f, 1, "after-nested-breakpoint");
    s.wait_protocol(true);
    let after = s.snapshot();
    assert_eq!(
        (before.input_mode, before.output_mode),
        (after.input_mode, after.output_mode)
    );
    finish(s);
}
fn clipboard(f: &Fixture) {
    let r = run(
        &mut {
            let mut cmd = Command::new(&f.probe);
            cmd.arg("n6-clipboard-launch")
                .arg(&f.root)
                .arg(&f.fish)
                .arg(&f.probe);
            cmd
        },
        b"",
    );
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.err));
    assert_eq!(
        fs::read(f.root.join("clipboard-passed")).unwrap(),
        b"pipe-selection-line-paste-private"
    );
}
