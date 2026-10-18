use super::fixture::{Fixture, exact, quote, run};
use std::{ffi::OsStr, fs};

pub(super) fn exercise(f: &Fixture) {
    let script = f.root.join("script 雪 space.fish");
    fs::write(
        &script,
        b"\xef\xbb\xbfprintf '%s\\n' $argv\r\nprintf done\r\n",
    )
    .unwrap();
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                script.as_os_str(),
                OsStr::new("--literal"),
                OsStr::new("雪 🐟"),
            ]),
            b"",
        ),
        0,
        "--literal\n雪 🐟\ndone".as_bytes(),
        b"",
    );
    exact(
        run(
            &mut f.command(&[OsStr::new("-N")]),
            b"\xef\xbb\xbfprintf stdin\r\n",
        ),
        0,
        b"stdin",
        b"",
    );
    fs::write(f.root.join("-script.fish"), "printf '%s' $argv\n").unwrap();
    exact(
        run(
            &mut f.command(&[
                OsStr::new("--no-config"),
                OsStr::new("--"),
                OsStr::new("-script.fish"),
                OsStr::new("--literal"),
            ]),
            b"",
        ),
        0,
        b"--literal",
        b"",
    );
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-c"),
                OsStr::new("printf one"),
                OsStr::new("--command=printf '%s' $argv"),
                OsStr::new("--"),
                OsStr::new("-c"),
                OsStr::new("two words"),
            ]),
            b"",
        ),
        0,
        b"one-ctwo words",
        b"",
    );
    exact(
        f.eval("eval 'set -l nested value; printf %s $nested'; printf '|'; eval ''"),
        0,
        b"value|",
        b"",
    );
    let sourced = f.root.join("sourced 雪.fish");
    fs::write(
        &sourced,
        "set -l source_local hidden\nprintf '%s\\n' $argv\nreturn 7\nprintf forbidden\n",
    )
    .unwrap();
    exact(f.eval(&format!("set -g argv outer; source {} first 'second arg'; printf '%s|' $status $argv; set -q source_local; printf '%s' $status", quote(sourced.to_str().unwrap()))), 0, b"first\nsecond arg\n7|outer|1", b"");
    exact(
        f.input("source - argument", b"printf '%s' $argv\n"),
        0,
        b"argument",
        b"",
    );
    exact(f.input(". -", b"printf dot\n"), 0, b"dot", b"");
    let marker = f.root.join("parse-only-marker");
    let code = format!(
        "printf forbidden > {}; set -g changed yes",
        quote(marker.to_str().unwrap())
    );
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-n"),
                OsStr::new("-c"),
                OsStr::new(&code),
            ]),
            b"",
        ),
        0,
        b"",
        b"",
    );
    exact(
        run(
            &mut f.command(&[OsStr::new("-N"), OsStr::new("--no-execute")]),
            code.as_bytes(),
        ),
        0,
        b"",
        b"",
    );
    assert!(!marker.exists());
    fs::write(
        &script,
        format!(
            "printf forbidden > {}\nif\n",
            quote(marker.to_str().unwrap())
        ),
    )
    .unwrap();
    let syntax = run(&mut f.command(&[OsStr::new("-N"), script.as_os_str()]), b"");
    assert_ne!(syntax.code, 0);
    assert!(syntax.out.is_empty());
    assert!(!marker.exists());
    let diagnostic = String::from_utf8_lossy(&syntax.err);
    assert!(diagnostic.contains("script 雪 space.fish"));
    assert!(diagnostic.contains("line 2"));
    let syntax_only = run(
        &mut f.command(&[
            OsStr::new("-N"),
            OsStr::new("--no-execute"),
            script.as_os_str(),
        ]),
        b"",
    );
    assert_ne!(syntax_only.code, 0);
    assert!(!marker.exists());
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("--features=no-qmark-noglob"),
                OsStr::new("-c"),
                OsStr::new("status test-feature qmark-noglob"),
            ]),
            b"",
        ),
        1,
        b"",
        b"",
    );
    let runtime_version = f.eval("printf '%s' $version");
    assert_eq!(runtime_version.code, 0);
    assert!(runtime_version.err.is_empty());
    let expected_version = format!(
        "fish, version {}\n",
        std::str::from_utf8(&runtime_version.out).unwrap()
    );
    exact(
        run(&mut f.command(&[OsStr::new("--version")]), b""),
        0,
        expected_version.as_bytes(),
        b"",
    );
    let recovered = run(
        &mut f.command(&[
            OsStr::new("-N"),
            OsStr::new("-c"),
            OsStr::new("echo $$"),
            OsStr::new("-c"),
            OsStr::new("printf recovered"),
        ]),
        b"",
    );
    assert_eq!(recovered.code, 0);
    assert_eq!(recovered.out, b"recovered");
    assert!(String::from_utf8_lossy(&recovered.err).contains("echo $$"));
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-c"),
                OsStr::new("exit 9"),
                OsStr::new("-c"),
                OsStr::new("printf forbidden"),
            ]),
            b"",
        ),
        9,
        b"",
        b"",
    );
    let categories = run(
        &mut f.command(&[OsStr::new("--print-debug-categories")]),
        b"",
    );
    assert_eq!(categories.code, 0);
    assert!(categories.out.windows(5).any(|word| word == b"debug"));
    assert!(categories.err.is_empty());
    for args in [
        vec!["-N", "-D", "5", "-c", &code],
        vec!["-N", "--debug-stack-frames=5", "-c", &code],
        vec!["-N", "--unknown", "-c", &code],
        vec!["-N", "-c"],
        vec!["-c", &code],
        vec!["-N", "--init-command", &code],
        vec!["-N", "--login"],
        vec!["-N", "--private"],
        vec!["-N", "--profile-startup", "startup.profile"],
        vec!["-N", "--interactive"],
        vec!["-N", "--print-rusage-self"],
        vec!["-N", "--help"],
    ] {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let result = run(&mut f.command(&args), b"");
        assert_ne!(result.code, 0);
        assert!(result.out.is_empty());
        assert!(!result.err.is_empty());
        assert!(!marker.exists());
    }
    let debug = f.root.join("debug 雪.log");
    let profile = f.root.join("profile 雪.log");
    let workload = format!(
        "function measured; begin; {} exit 0; end; end; set -g fish_trace 1; measured; printf command; false",
        f.probe()
    );
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-d"),
                OsStr::new("debug"),
                OsStr::new("-o"),
                debug.as_os_str(),
                OsStr::new("-p"),
                profile.as_os_str(),
                OsStr::new("-c"),
                OsStr::new(&workload),
            ]),
            b"",
        ),
        1,
        b"command",
        b"",
    );
    let trace = fs::read(&debug).unwrap();
    assert!(trace.windows(6).any(|part| part == b"printf"));
    let toggle_log = f.root.join("trace-toggle.log");
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("--debug-output"),
                toggle_log.as_os_str(),
                OsStr::new("-c"),
                OsStr::new(
                    "set -g fish_trace 1; printf first; set -e fish_trace; printf last; false",
                ),
            ]),
            b"",
        ),
        1,
        b"firstlast",
        b"",
    );
    let toggle_log = fs::read_to_string(toggle_log).unwrap();
    assert!(toggle_log.contains("printf first"));
    assert!(!toggle_log.contains("printf last"));
    let timing = fs::read_to_string(&profile).unwrap();
    let rows: Vec<(i64, i64, usize, String)> = timing
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let own = fields.next()?.parse::<i64>().ok()?;
            let inclusive = fields
                .next()?
                .parse::<i64>()
                .expect("numeric inclusive duration");
            let depth = fields.next().expect("profile nesting marker");
            assert!(
                depth.ends_with('>') && depth[..depth.len() - 1].bytes().all(|byte| byte == b'-')
            );
            Some((
                own,
                inclusive,
                depth.len(),
                fields.collect::<Vec<_>>().join(" "),
            ))
        })
        .collect();
    assert!(
        rows.iter()
            .all(|(own, inclusive, _, _)| *own >= 0 && *inclusive >= *own)
    );
    assert!(rows.iter().any(|(_, inclusive, depth, cmd)| *inclusive > 0
        && *depth > 1
        && cmd.contains("native probe")
        && cmd.contains("exit 0")));
    let measured = rows
        .iter()
        .find(|(_, _, _, cmd)| cmd == "measured")
        .expect("profiled function invocation");
    assert!(measured.1 > 0);
    assert!(rows.iter().any(|(_, inclusive, depth, cmd)| *inclusive > 0
        && *depth > measured.2
        && cmd.contains("native probe")));
    for (name, selection, expected_ast) in [
        ("default", None, false),
        ("enabled", Some("ast-*"), true),
        ("excluded", Some("ast-*,-ast-construction"), false),
    ] {
        let log = f.root.join(format!("categories-{name}.log"));
        let mut args = vec![
            OsStr::new("-N"),
            OsStr::new("--debug-output"),
            log.as_os_str(),
        ];
        if let Some(selection) = selection {
            args.extend([OsStr::new("--debug"), OsStr::new(selection)]);
        }
        args.extend([OsStr::new("-c"), OsStr::new("printf category; false")]);
        exact(run(&mut f.command(&args), b""), 1, b"category", b"");
        let log = fs::read_to_string(log).unwrap();
        assert_eq!(
            log.lines()
                .any(|line| line.starts_with("ast-construction:")),
            expected_ast,
            "{name}"
        );
        assert!(
            !log.lines().any(|line| line.starts_with('>')),
            "trace must remain off for {name}"
        );
    }
    let directory = f.root.join("sink directory");
    fs::create_dir(&directory).unwrap();
    for option in ["--debug-output", "--profile"] {
        let result = run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new(option),
                directory.as_os_str(),
                OsStr::new("-c"),
                OsStr::new(&code),
            ]),
            b"",
        );
        assert_ne!(result.code, 0);
        assert!(!result.err.is_empty());
        assert!(!marker.exists());
    }
}
