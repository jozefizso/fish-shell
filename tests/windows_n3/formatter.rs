use super::fixture::{Fixture, exact, quote, run};
use std::{ffi::OsStr, fs, process::Command};

fn formatter(f: &Fixture, args: &[&OsStr], input: &[u8]) -> super::fixture::Captured {
    let mut command = Command::new(f.root.join("fish_indent.exe"));
    command
        .args(args)
        .current_dir(&f.root)
        .env_clear()
        .env("fish_color_command", "--bold 123456")
        .env("fish_term24bit", "1")
        .env("HOME", f.state.join("home"));
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    run(&mut command, input)
}

fn configured_ansi(result: super::fixture::Captured) {
    assert_eq!(result.code, 0);
    assert!(result.err.is_empty());
    // Interpret the emitted SGR protocol, not one particular reset spelling.
    // Both SGR 0 and selective resets must leave the next writer unstyled.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Style {
        attributes: u8,
        foreground: Option<[u16; 3]>,
        background: Option<[u16; 3]>,
        underline_color: Option<[u16; 3]>,
    }
    let mut style = Style::default();
    let mut visible = Vec::new();
    let mut bytes = result.out.as_slice();
    while let Some((&byte, rest)) = bytes.split_first() {
        bytes = rest;
        if byte != 0x1b {
            if visible.len() < b"printf".len() {
                assert_eq!(byte, b"printf"[visible.len()]);
                assert_eq!(style.foreground, Some([18, 52, 86]));
                assert_ne!(style.attributes & 1, 0, "command must be bold");
            }
            visible.push(byte);
            continue;
        }
        assert_eq!(bytes.first(), Some(&b'['));
        let end = bytes
            .iter()
            .position(|&byte| byte == b'm')
            .expect("terminated SGR");
        let fields = std::str::from_utf8(&bytes[1..end]).unwrap();
        let mut fields = fields.split(';').map(|field| {
            if field.is_empty() {
                0
            } else {
                field
                    .split(':')
                    .next()
                    .unwrap()
                    .parse::<u16>()
                    .expect("numeric SGR")
            }
        });
        while let Some(code) = fields.next() {
            match code {
                0 => style = Style::default(),
                1 => style.attributes |= 1,
                2 => style.attributes |= 2,
                3 => style.attributes |= 4,
                4 => style.attributes |= 8,
                7 => style.attributes |= 16,
                9 => style.attributes |= 32,
                22 => style.attributes &= !3,
                23 => style.attributes &= !4,
                24 => style.attributes &= !8,
                27 => style.attributes &= !16,
                29 => style.attributes &= !32,
                30..=37 | 90..=97 => style.foreground = Some([code, 0, 0]),
                40..=47 | 100..=107 => style.background = Some([code, 0, 0]),
                39 => style.foreground = None,
                49 => style.background = None,
                59 => style.underline_color = None,
                38 | 48 | 58 => {
                    let color = match fields.next().expect("color mode") {
                        2 => [
                            fields.next().expect("red"),
                            fields.next().expect("green"),
                            fields.next().expect("blue"),
                        ],
                        5 => [5, fields.next().expect("palette index"), 0],
                        mode => panic!("unexpected color mode {mode}"),
                    };
                    match code {
                        38 => style.foreground = Some(color),
                        48 => style.background = Some(color),
                        58 => style.underline_color = Some(color),
                        _ => unreachable!(),
                    }
                }
                code => panic!("unexpected SGR {code}"),
            }
        }
        bytes = &bytes[end + 1..];
    }
    assert!(visible.starts_with(b"printf"));
    assert!(visible.windows(4).any(|word| word == b"snow"));
    assert_eq!(visible.last(), Some(&b'\n'));
    assert_eq!(
        style,
        Style::default(),
        "highlighting must not leak terminal style"
    );
}

pub(super) fn exercise(f: &Fixture) {
    let input = b"if true;echo hello;end\n";
    let formatted = b"if true\n    echo hello\nend\n";
    exact(formatter(f, &[], input), 0, formatted, b"");
    exact(f.input("fish_indent", input), 0, formatted, b"");
    exact(
        formatter(f, &[OsStr::new("--no-indent")], input),
        0,
        b"if true\necho hello\nend\n",
        b"",
    );
    exact(
        formatter(
            f,
            &[OsStr::new("--only-indent")],
            b"if true\necho hi\nend\n",
        ),
        0,
        b"if true\n    echo hi\nend\n",
        b"",
    );
    exact(
        formatter(
            f,
            &[OsStr::new("--only-unindent")],
            b"if true\n    echo hi\nend\n",
        ),
        0,
        b"if true\necho hi\nend\n",
        b"",
    );
    exact(
        formatter(f, &[OsStr::new("-i")], input),
        0,
        b"if true\necho hello\nend\n",
        b"",
    );
    let file = f.root.join("format 雪 space.fish");
    fs::write(&file, input).unwrap();
    exact(formatter(f, &[file.as_os_str()], b""), 0, formatted, b"");
    exact(
        formatter(f, &[OsStr::new("--check"), file.as_os_str()], b""),
        1,
        b"",
        format!("{}\n", file.display()).as_bytes(),
    );
    exact(
        formatter(f, &[OsStr::new("--write"), file.as_os_str()], b""),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(&file).unwrap(), formatted);
    exact(
        formatter(f, &[OsStr::new("-c"), file.as_os_str()], b""),
        0,
        b"",
        b"",
    );
    exact(
        f.eval(&format!(
            "fish_indent -w {}; fish_indent -c {}",
            quote(file.to_str().unwrap()),
            quote(file.to_str().unwrap())
        )),
        0,
        b"",
        b"",
    );
    exact(formatter(f, &[OsStr::new("-c")], input), 1, b"", b"");
    exact(formatter(f, &[OsStr::new("-c")], formatted), 0, b"", b"");
    let crlf = b"printf 'snow \xe9\x9b\xaa'\r\n";
    exact(
        formatter(f, &[], crlf),
        0,
        "printf 'snow 雪'\n".as_bytes(),
        b"",
    );
    exact(
        formatter(f, &[], b"\xef\xbb\xbfprintf bom\n"),
        0,
        b"\xef\xbb\xbfprintf bom\n",
        b"",
    );
    configured_ansi(formatter(f, &[OsStr::new("--ansi")], b"printf 'snow'\n"));
    configured_ansi(f.input(
        "set -g fish_color_command --bold 123456; set -g fish_term24bit 1; fish_indent --ansi",
        b"printf 'snow'\n",
    ));
    let html = formatter(f, &[OsStr::new("--html")], b"printf '<&>'\n");
    assert_eq!(html.code, 0);
    assert!(html.err.is_empty());
    let html = String::from_utf8(html.out).unwrap();
    assert!(html.contains("fish_color_command"));
    assert!(html.contains("&lt;&amp;&gt;"));
    assert!(html.starts_with("<pre><code>"));
    let pygments = formatter(f, &[OsStr::new("--pygments")], "echo 🐟\n".as_bytes());
    assert_eq!(pygments.code, 0);
    assert!(pygments.err.is_empty());
    let pygments = String::from_utf8(pygments.out).unwrap();
    assert!(pygments.lines().any(|line| line.starts_with("0,4,")));
    assert!(pygments.lines().any(|line| line.starts_with("5,6,")));
    let dump = formatter(f, &[OsStr::new("--dump-parse-tree")], input);
    assert_eq!(dump.code, 0);
    assert_eq!(dump.out, formatted);
    assert!(!dump.err.is_empty());
    let runtime_version = f.eval("printf '%s' $version");
    assert_eq!(runtime_version.code, 0);
    assert!(runtime_version.err.is_empty());
    let expected_version = format!(
        "fish_indent, version {}\n",
        std::str::from_utf8(&runtime_version.out).unwrap()
    );
    for flag in ["--version", "-v"] {
        exact(
            formatter(f, &[OsStr::new(flag)], b""),
            0,
            expected_version.as_bytes(),
            b"",
        );
    }
    for flag in [
        "--unknown",
        "--ansi=always",
        "--write",
        "-w",
        "--help",
        "-h",
    ] {
        let result = formatter(f, &[OsStr::new(flag)], input);
        assert_ne!(result.code, 0, "{flag}");
        assert!(result.out.is_empty(), "{flag}");
        assert!(!result.err.is_empty(), "{flag}");
    }
    let missing = f.root.join("missing-input.fish");
    let result = formatter(f, &[missing.as_os_str()], b"");
    assert_ne!(result.code, 0);
    assert!(!result.err.is_empty());
    let readonly = f.root.join("readonly-format.fish");
    fs::write(&readonly, input).unwrap();
    let mut permissions = fs::metadata(&readonly).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&readonly, permissions.clone()).unwrap();
    let denied = formatter(f, &[OsStr::new("-w"), readonly.as_os_str()], b"");
    permissions.set_readonly(false);
    fs::set_permissions(&readonly, permissions).unwrap();
    assert_ne!(denied.code, 0);
    assert!(!denied.err.is_empty());
    assert_eq!(fs::read(&readonly).unwrap(), input);
    // End-of-options makes a leading dash a filename, not a formatter switch.
    fs::write(f.root.join("--check"), input).unwrap();
    exact(
        formatter(f, &[OsStr::new("--"), OsStr::new("--check")], b""),
        0,
        formatted,
        b"",
    );
}
