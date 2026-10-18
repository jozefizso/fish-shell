use super::fixture::{Fixture, exact, quote};

pub(super) fn exercise(f: &Fixture) {
    arguments(f);
    autoload(f);
    values_and_streams(f);
    variables(f);
    control_and_registry(f);
    status_and_type(f);
    errors_and_boundaries(f);
}

fn autoload(f: &Fixture) {
    // Implicit integer validation must dispatch the ordinary function, including
    // overrides, validation scope and a tombstone after an explicit erase.
    exact(f.eval("function _validate_int; set -g n3_validator_seen $_flag_value; printf '%s:%s:%s\\n' $_argparse_cmd $_flag_name $_flag_value; end; argparse -n custom-validator 'n#' -- -n +42; printf '%s:%s\\n' $_flag_n $n3_validator_seen; set -q _flag_value; echo $status"), 0,
        b"+42:+42\n1\n", b"custom-validator:n:+42\n");
    let tombstone = f.eval(
        "function _validate_int; true; end; functions -e _validate_int; argparse 'n#' -- -42",
    );
    assert_ne!(tombstone.code, 0);
    assert!(tombstone.out.is_empty());
    assert!(String::from_utf8_lossy(&tombstone.err).contains("_validate_int"));
}

fn arguments(f: &Fixture) {
    exact(f.eval("argparse 'v/verbose' 'o/output=' 't/tag=+' 'c/color=?' -- -vv --output 雪 --tag one -t two --color=red tail; printf '<%s>\\n' $_flag_v $_flag_verbose $_flag_o $_flag_output $_flag_tag $_flag_color $argv"), 0,
        "<-v>\n<-v>\n<-v>\n<-v>\n<雪>\n<雪>\n<one>\n<two>\n<red>\n<tail>\n".as_bytes(), b"");
    for flag in ["-s", "--stop-nonopt"] {
        exact(f.eval(&format!("argparse {flag} a -- first -a; printf '<%s>\\n' $argv; set -q _flag_a; echo $status")), 0,
            b"<first>\n<-a>\n1\n", b"");
    }
    for flag in ["-i", "--ignore-unknown"] {
        exact(
            f.eval(&format!(
                "argparse {flag} a -- --other=x -a tail; printf '<%s>\\n' $_flag_a $argv"
            )),
            0,
            b"<-a>\n<--other=x>\n<tail>\n",
            b"",
        );
    }
    for flag in ["-u", "--move-unknown"] {
        exact(f.eval(&format!("argparse {flag} 'a&' -- --other=x -a tail; printf '<%s>\\n' $_flag_a $argv_opts $argv")), 0,
            b"<-a>\n<--other=x>\n<tail>\n", b"");
    }
    for flag in ["-U", "--unknown-arguments"] {
        for (kind, expected) in [
            ("none", b"<--other>\nDIVIDER\n<value>\n<tail>\n".as_slice()),
            (
                "required",
                b"<--other>\n<value>\nDIVIDER\n<tail>\n".as_slice(),
            ),
            (
                "optional",
                b"<--other>\nDIVIDER\n<value>\n<tail>\n".as_slice(),
            ),
        ] {
            exact(f.eval(&format!("argparse {flag} {kind} 'a&' -- --other value -a tail; printf '<%s>\\n' $argv_opts; echo DIVIDER; printf '<%s>\\n' $argv")), 0, expected, b"");
        }
    }
    for flag in ["-S", "--strict-longopts"] {
        let result = f.eval(&format!("argparse {flag} verbose -- --verb"));
        assert_ne!(result.code, 0);
        assert!(!result.err.is_empty());
        exact(
            f.eval(&format!(
                "argparse {flag} verbose -- --verbose; printf '%s\\n' $_flag_verbose"
            )),
            0,
            b"--verbose\n",
            b"",
        );
    }
    for (name, min, max, exclusive) in [
        ("-n", "-N", "-X", "-x"),
        ("--name", "--min-args", "--max-args", "--exclusive"),
    ] {
        exact(f.eval(&format!("argparse {name} native-options {min} 1 {max} 2 {exclusive} a,b a b -- -a one two; printf '%s\\n' $_flag_a $argv")), 0,
            b"-a\none\ntwo\n", b"");
        for args in ["-a -b one", "", "one two three"] {
            let result = f.eval(&format!(
                "argparse {name} native-options {min} 1 {max} 2 {exclusive} a,b a b -- {args}"
            ));
            assert_ne!(result.code, 0);
            assert!(String::from_utf8_lossy(&result.err).contains("native-options"));
        }
    }
    exact(f.eval("argparse 'n/number=!test $_flag_value -gt 2' -- --number 3; printf '%s\\n' $_flag_number"), 0, b"3\n", b"");
    exact(f.eval("argparse 'n/number=!test $_flag_value -gt 2' -- --number 1; echo $status; set -q _flag_value; echo $status"), 0, b"1\n1\n", b"");
    exact(
        f.eval("argparse 'n#' -- -42; printf '%s\\n' $_flag_n"),
        0,
        b"42\n",
        b"",
    );
    exact(
        f.eval("argparse 'n#' -- -n -42; printf '%s\\n' $_flag_n"),
        0,
        b"-42\n",
        b"",
    );
    let invalid_integer = f.eval("argparse 'n#' -- -n +42");
    assert_eq!(invalid_integer.code, 1);
    assert!(!invalid_integer.err.is_empty());
    exact(f.eval("argparse 'optional=?' 'many=*' 'drop=&' -- --optional --many=one --many --drop hidden tail; set -q _flag_optional; echo optional-present:$status; printf 'optional-count:%s\\n' (count $_flag_optional); printf '<%s>\\n' $_flag_many $_flag_drop; echo DIVIDER; printf '<%s>\\n' $argv_opts; echo DIVIDER; printf '<%s>\\n' $argv"), 0,
        b"optional-present:0\noptional-count:0\n<one>\n<>\n<hidden>\nDIVIDER\n<--optional>\n<--many=one>\n<--many>\nDIVIDER\n<tail>\n", b"");
}

fn values_and_streams(f: &Fixture) {
    for flag in ["-i", "--index"] {
        exact(
            f.eval(&format!("contains {flag} 雪 a '' 雪 雪")),
            0,
            b"3\n",
            b"",
        );
    }
    exact(f.eval("contains '' a ''; echo $status; contains absent a b; echo $status; contains -- --help --help; echo $status"), 0, b"0\n1\n0\n", b"");
    exact(f.eval("count a '' 雪 --help"), 0, b"4\n", b"");
    exact(
        f.eval("printf 'one\\ntwo\\nlast' | count a b"),
        0,
        b"4\n",
        b"",
    );
    exact(f.eval("count"), 1, b"0\n", b"");
    exact(
        f.eval("echo -n -s a b; echo -e 'c\\td'; echo -E 'e\\tf'; echo --help"),
        0,
        b"abc\td\ne\\tf\n--help\n",
        b"",
    );
    exact(
        f.eval("echo -e 'A\\x00B\\cC'; printf '\\x00\\101\\u96ea'; printf --help"),
        0,
        "A\0B\0A雪--help".as_bytes(),
        b"",
    );
    exact(
        f.eval("printf '%s:%04d:%.2f:%x:%o:%%\\n' 雪 7 1.25 255 8; printf '<%s>' a b"),
        0,
        "雪:0007:1.25:ff:10:%\n<a><b>".as_bytes(),
        b"",
    );
    exact(
        f.eval("printf '%b:%c:%u:%X\\n' 'a\\tb' Z 7 255"),
        0,
        b"a\tb:Z:7:FF\n",
        b"",
    );
    for (flags, expr, expected) in [
        ("", "2 + 3 * 4", "14\n"),
        ("-s 2", "10 / 3", "3.33\n"),
        ("--scale max", "1 / 8", "0.125\n"),
        ("-b hex", "255", "0xff\n"),
        ("--base octal", "8", "010\n"),
        ("-s 0 -m trunc", "2.9", "2\n"),
        ("--scale 0 --scale-mode truncate", "-2.9", "-2\n"),
        ("-s 0 -m round", "2.9", "3\n"),
        ("-s 0 -m floor", "-2.1", "-3\n"),
        ("-s 0 -m ceil", "2.1", "3\n"),
        ("--scale 0 --scale-mode ceiling", "2.1", "3\n"),
    ] {
        exact(
            f.eval(&format!("math {flags} {}", quote(expr))),
            0,
            expected.as_bytes(),
            b"",
        );
    }
    exact(f.eval("printf '6 * 7\\n' | math"), 0, b"42\n", b"");
    exact(f.eval("random 11; set first (random); random 11; test $first -eq (random); echo $status; random 9 9; random 9 2 9; random choice 雪"), 0,
        "0\n9\n9\n雪\n".as_bytes(), b"");
    let generated = f.eval("random; random 20 2 10; random choice red blue");
    assert_eq!(generated.code, 0);
    assert!(generated.err.is_empty());
    let text = String::from_utf8(generated.out).unwrap();
    let values: Vec<_> = text.lines().collect();
    assert_eq!(values.len(), 3);
    assert!((0..=32767).contains(&values[0].parse::<i64>().unwrap()));
    let stepped = values[1].parse::<i64>().unwrap();
    assert!((10..=20).contains(&stepped) && stepped % 2 == 0);
    assert!(["red", "blue"].contains(&values[2]));
}

fn variables(f: &Fixture) {
    for (local, global, export, unexport, query, erase, append, prepend, function) in [
        ("-l", "-g", "-x", "-u", "-q", "-e", "-a", "-p", "-f"),
        (
            "--local",
            "--global",
            "--export",
            "--unexport",
            "--query",
            "--erase",
            "--append",
            "--prepend",
            "--function",
        ),
    ] {
        exact(f.eval(&format!("set {global} n3_values a b; begin; set {local} n3_values c d; set {append} n3_values e; set {prepend} n3_values z; printf '<%s>\\n' $n3_values; end; printf '<%s>\\n' $n3_values; set n3_values[2] 雪; set {erase} n3_values[1]; printf '<%s>\\n' $n3_values; set {export} n3_export yes; set {query} {export} n3_export; echo $status; set {unexport} n3_export; set {query} {export} n3_export; echo $status; function n3_scope; set {function} n3_function outer; begin; set {local} n3_function inner; end; printf '%s\\n' $n3_function; end; n3_scope; set {query} n3_function; echo $status")), 0,
            "<z>\n<c>\n<d>\n<e>\n<a>\n<b>\n<雪>\n0\n1\nouter\n1\n".as_bytes(), b"");
    }
    exact(f.eval("set --path n3_paths a b; printf '%s\\n' \"$n3_paths\"; set --unpath n3_paths $n3_paths; printf '%s\\n' \"$n3_paths\"; set -- n3_dash --help; echo $n3_dash; set n3_slice a b c d; printf '%s\\n' $n3_slice[-1..2]; set -e n3_slice[2..3]; printf '%s\\n' $n3_slice; set -q n3_slice[3] missing; echo $status; false; set n3_status preserved; echo $status"), 0,
        b"a;b\na b\n--help\nd\nc\nb\na\nd\n2\n1\n", b"");
    for (names, long, show) in [("-n", "-L", "-S"), ("--names", "--long", "--show")] {
        let value = "x".repeat(90);
        let result = f.eval(&format!("set -g n3_inspect {value}; set {names} -g; echo DIVIDER; set {long} -g; echo DIVIDER; set {show} n3_inspect; echo DIVIDER; set -q -g n3_inspect; echo $status; set -q -x n3_inspect; echo $status; printf '%s\\n' $n3_inspect"));
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty());
        let text = String::from_utf8(result.out).unwrap();
        let sections: Vec<_> = text.split("DIVIDER\n").collect();
        assert_eq!(sections.len(), 4);
        assert!(sections[0].lines().any(|line| line == "n3_inspect"));
        assert!(
            sections[1]
                .lines()
                .any(|line| line == format!("n3_inspect {value}"))
        );
        assert!(
            sections[2].contains(&value),
            "inspection must display the stored value"
        );
        assert_eq!(sections[3], format!("0\n1\n{value}\n"));
    }
    exact(f.eval("function n3_event --on-variable n3_event_value; echo event; end; set n3_event_value one; set --no-event n3_event_value two; set --no-event -e n3_event_value; set n3_event_value three; set -e n3_event_value"), 0,
        b"event\nevent\nevent\n", b"");
}

fn control_and_registry(f: &Fixture) {
    exact(f.eval("true ignored --help; echo $status; false ignored --help; echo $status; : ignored --help; echo $status; builtin echo decorated"), 0,
        b"0\n1\n0\ndecorated\n", b"");
    for (query, names) in [("-q", "-n"), ("--query", "--names")] {
        exact(f.eval(&format!("builtin {query} contains; echo $status; builtin {query} native_absent; echo $status")), 0, b"0\n1\n", b"");
        let result = f.eval(&format!("builtin {names}"));
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty());
        let text = String::from_utf8(result.out).unwrap();
        let names: Vec<_> = text.lines().collect();
        for name in [
            ":", "argparse", "break", "builtin", "contains", "continue", "count", "random",
            "return", "status",
        ] {
            assert!(names.contains(&name), "missing native builtin {name}");
        }
        assert!(!names.contains(&"fg") && !names.contains(&"bg"));
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    }
    exact(f.eval("for n in 1 2 3; if test $n -eq 2; continue; end; echo $n; end; set n 0; while true; set n (math $n + 1); if test $n -eq 3; break; end; echo $n; end; function n3_return; return -1; echo forbidden; end; n3_return; echo $status"), 0,
        b"1\n3\n1\n2\n255\n", b"");
    exact(f.eval("function n3_return_default; false; return; end; n3_return_default; echo $status; function n3_return_explicit; return -- 7; end; n3_return_explicit; echo $status"), 0, b"1\n7\n", b"");
    exact(f.eval("return 6; echo forbidden"), 6, b"", b"");
    exact(f.eval("exit -- 23; echo forbidden"), 23, b"", b"");
}

fn status_and_type(f: &Fixture) {
    let source = f.root.join("status context 雪.fish");
    for (queries, expected) in [
        (
            &[
                "filename",
                "current-filename",
                "--filename",
                "--current-filename",
                "-f",
            ][..],
            source.to_str().unwrap(),
        ),
        (
            &["basename", "current-basename"][..],
            source.file_name().unwrap().to_str().unwrap(),
        ),
        (
            &["dirname", "current-dirname"][..],
            f.root.to_str().unwrap(),
        ),
    ] {
        for query in queries {
            std::fs::write(&source, format!("status {query}\n")).unwrap();
            exact(
                f.eval(&format!("source {}", quote(source.to_str().unwrap()))),
                0,
                format!("{expected}\n").as_bytes(),
                b"",
            );
        }
    }
    for query in [
        "line-number",
        "current-line-number",
        "--line",
        "--line-number",
        "--current-line-number",
        "-n",
    ] {
        exact(f.eval(&format!("true\nstatus {query}")), 0, b"2\n", b"");
    }
    for query in ["function", "current-function"] {
        for level in ["-L", "--level"] {
            exact(f.eval(&format!("function n3_status_outer; n3_status_inner; end; function n3_status_inner; status {level} 2 {query}; end; n3_status_outer")), 0, b"n3_status_outer\n", b"");
        }
    }
    for query in ["is-block", "--is-block", "-b"] {
        exact(
            f.eval(&format!("begin; status {query}; echo $status; end")),
            0,
            b"0\n",
            b"",
        );
    }
    for query in ["is-command-substitution", "--is-command-substitution", "-c"] {
        exact(
            f.eval(&format!(
                "echo (status {query}; echo $status); status {query}; echo $status"
            )),
            0,
            b"0\n1\n",
            b"",
        );
    }
    for query in [
        "stack-trace",
        "print-stack-trace",
        "--print-stack-trace",
        "-t",
    ] {
        let result = f.eval(&format!("function n3_stack; status {query}; end; n3_stack"));
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty());
        assert!(String::from_utf8_lossy(&result.out).contains("n3_stack"));
    }
    for query in ["build-info", "buildinfo"] {
        let result = f.eval(&format!("status {query}"));
        assert_eq!(result.code, 0);
        let text = String::from_utf8(result.out).unwrap();
        assert!(text.contains("x86_64-pc-windows-msvc"));
        assert!(result.err.is_empty());
    }
    let feature_list = f.eval("status features");
    assert_eq!(feature_list.code, 0);
    assert!(feature_list.err.is_empty());
    let features = String::from_utf8(feature_list.out).unwrap();
    assert!(!features.is_empty());
    for line in features.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let result = f.eval(&format!("status test-feature {}", quote(fields[0])));
        assert_eq!(result.code, if fields[1] == "on" { 0 } else { 1 });
        assert!(result.out.is_empty() && result.err.is_empty());
    }
    exact(
        f.eval("status test-feature definitely_missing_feature"),
        2,
        b"",
        b"",
    );
    for query in ["fish-path", "--fish-path"] {
        let result = f.eval(&format!(
            "test (status {query}) -ef {}; echo $status",
            quote(f.fish.to_str().unwrap())
        ));
        exact(result, 0, b"0\n", b"");
    }
    exact(f.eval("function n3_type; echo body; end; type -t n3_type; type --type echo; type --no-functions -q n3_type; echo $status; type --query n3_type; echo $status; type --quiet absent; echo $status"), 0,
        b"function\nbuiltin\n1\n0\n1\n", b"");
    for short in ["-s", "--short"] {
        let result = f.eval(&format!(
            "function n3_type; true; end; type {short} n3_type"
        ));
        assert_eq!(result.code, 0);
        let output = String::from_utf8_lossy(&result.out);
        assert!(output.contains("n3_type"));
        assert!(
            !output.contains("echo body"),
            "brief type must omit the definition"
        );
        assert!(result.err.is_empty());
    }
    for all in ["-a", "--all"] {
        let result = f.eval(&format!("function echo; true; end; type {all} --type echo"));
        exact(result, 0, b"function\nbuiltin\n", b"");
    }
    for color in ["auto", "never", "always"] {
        let result = f.eval(&format!(
            "set -g fish_color_command red; set -g fish_color_param blue; function n3_type; echo body; end; type --color={color} n3_type"
        ));
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty());
        assert!(String::from_utf8_lossy(&result.out).contains("n3_type"));
        assert_eq!(
            result.out.windows(2).any(|bytes| bytes == b"\x1b["),
            color == "always"
        );
    }
    let probe = quote(f.probe.to_str().unwrap());
    for path in ["-p", "--path", "-P", "--force-path"] {
        exact(
            f.eval(&format!(
                "test (type {path} {probe}) -ef {probe}; echo $status"
            )),
            0,
            b"0\n",
            b"",
        );
    }
    exact(f.eval("type -f -q echo; echo $status"), 0, b"0\n", b"");
}

fn errors_and_boundaries(f: &Fixture) {
    for code in [
        "argparse a",
        "argparse '=' --",
        "argparse -U invalid a --",
        "argparse --strict-longopts a -- --absent",
        "contains --absent",
        "contains",
        "math --scale 16 1",
        "math --base 3 1",
        "math -m bad 1",
        "math -s 2 -b 16 1",
        "math '1 / 0'",
        "math '1 +'",
        "printf '%Q' a",
        "printf '%d' invalid",
        "random choice",
        "random 1 0 9",
        "random not-a-number",
        "random --absent",
        "set -l -g n3_bad value",
        "set -x -u n3_bad value",
        "set --path --unpath n3_bad value",
        "set -a 'n3_bad[1]' value",
        "set 'n3_bad[0]' value",
        "set -S 'n3_bad[1]'",
        "return not-a-number",
        "return 1 2",
        "exit not-a-number",
        "builtin --absent",
        "status --level -1 function",
        "status absent",
        "status test-feature",
        "type -q -t echo",
        "type --color invalid echo",
    ] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(!result.err.is_empty(), "{code}");
    }
    for code in ["eval break", "eval continue"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(!result.err.is_empty());
    }
    for query in [
        "job-control none",
        "-j none",
        "--job-control none",
        "is-full-job-control",
        "--is-full-job-control",
        "is-interactive-job-control",
        "--is-interactive-job-control",
        "is-no-job-control",
        "--is-no-job-control",
        "is-breakpoint",
        "terminal",
        "terminal-os",
        "test-terminal-feature scroll-content-up",
        "language",
    ] {
        let result = f.eval(&format!("status {query}"));
        assert_ne!(result.code, 0, "{query}");
        assert!(!result.err.is_empty(), "{query}");
    }
    for builtin in [
        "argparse", "builtin", "contains", "random", "return", "math", "status", "set", "break",
        "continue", "exit", "type",
    ] {
        for help in ["-h", "--help"] {
            let result = f.eval(&format!("{builtin} {help}"));
            assert_ne!(result.code, 0, "{builtin} {help}");
            assert!(!result.err.is_empty());
        }
    }
}
