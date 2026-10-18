use super::fixture::{Fixture, exact};

pub(super) fn exercise(f: &Fixture) {
    exact(
        f.input(
            "read first; read second; printf '<%s><%s>' $first $second",
            b"one\ntwo\n",
        ),
        0,
        b"<one><two>",
        b"",
    );
    exact(
        f.input(&format!("read first; {} stream", f.probe()), b"one\ntwo\n"),
        0,
        b"probe-out:two\n",
        b"probe-err:\0\n\r\n\xff\x80",
    );
    exact(
        f.input(
            "read --null first; read -z second; printf '<%s><%s>' $first $second",
            b"a\0b\0tail",
        ),
        0,
        b"<a><b>",
        b"",
    );
    exact(f.input("read -n 1 first; read --nchars=1 second; read third; printf '<%s><%s><%s>' $first $second $third", "雪🐟last\n".as_bytes()), 0, "<雪><🐟><last>".as_bytes(), b"");
    exact(
        f.input("read -n 0 value; printf '<%s>' $value", b"all\n"),
        0,
        b"<all>",
        b"",
    );
    exact(
        f.input(
            "read --delimiter=:: left right; printf '<%s><%s>' $left $right",
            b"one::two::three\n",
        ),
        0,
        b"<one><two::three>",
        b"",
    );
    exact(
        f.input(
            "read -a -d , values; printf '<%s>' $values",
            b"one,,three\n",
        ),
        0,
        b"<one><><three>",
        b"",
    );
    exact(
        f.input(
            "read --list --delimiter='' values; printf '<%s>' $values",
            "雪🐟\n".as_bytes(),
        ),
        0,
        "<雪><🐟>".as_bytes(),
        b"",
    );
    exact(
        f.input(
            "read --line first second; read third; printf '<%s><%s><%s>' $first $second $third",
            b"first line\nsecond line\nthird line\n",
        ),
        0,
        b"<first line><second line><third line>",
        b"",
    );
    exact(
        f.input(
            "read --array --tokenize values; printf '<%s>' $values",
            b"one 'two words' three\\ four\n",
        ),
        0,
        b"<one><two words><three four>",
        b"",
    );
    exact(
        f.input(
            "read --array --tokenize-raw values; printf '<%s>' $values",
            b"one 'two words' three\\ four\n",
        ),
        0,
        b"<one><'two words'><three\\ four>",
        b"",
    );
    exact(
        f.input(
            "read --tokenize first rest; printf '<%s><%s>' $first $rest",
            b"'one word' 'two words' tail\n",
        ),
        0,
        b"<one word><'two words' tail>",
        b"",
    );
    exact(
        f.input(
            "read value; printf '<%s>|%s' $value $status",
            b"unterminated",
        ),
        0,
        b"<unterminated>|0",
        b"",
    );
    exact(
        f.input(
            "set -g value before; read value; printf '%s|' $status; count $value",
            b"",
        ),
        1,
        b"1|0\n",
        b"",
    );
    exact(
        f.input("read; read", b"first\nsecond\n"),
        0,
        b"firstsecond",
        b"",
    );
    exact(f.input("begin; read --local value; printf '<%s>' $value; end; set -q value; printf '|%s' $status", b"local\n"), 0, b"<local>|1", b"");
    exact(f.input("function scope; read --function value; begin; printf '<%s>' $value; end; end; scope; set -q value; printf '|%s' $status", b"function\n"), 0, b"<function>|1", b"");
    exact(f.input("begin; read --global --export value; end; set -q --global --export value; printf '%s:%s' $status $value", b"global\n"), 0, b"0:global", b"");
    exact(f.input("read -g -x value; read --unexport value; set -q --export value; printf '%s:%s' $status $value", b"first\nsecond\n"), 0, b"1:second", b"");
    // The following builtin consumes the original shared cursor, not a newly opened stdin.
    let large = "x".repeat(32768);
    exact(
        f.input(
            "read first; read second; string length -- $first; printf '%s' $second",
            format!("{large}\nretained\n").as_bytes(),
        ),
        0,
        b"32768\nretained",
        b"",
    );
    exact(
        f.input(
            "set -g fish_read_limit 3; read value; printf '%s|' $status; count $value",
            b"abcd\n",
        ),
        1,
        b"122|0\n",
        b"",
    );
    exact(
        f.input(
            "set -g fish_read_limit 3; read value; printf '<%s>' $value",
            b"abc\n",
        ),
        0,
        b"<abc>",
        b"",
    );
    exact(
        f.input("read value; printf '%s' $value", b"crlf\r\n"),
        0,
        b"crlf\r",
        b"",
    );
    exact(
        f.input(
            "read --universal n3_read_persisted",
            "persisted 雪\n".as_bytes(),
        ),
        0,
        b"",
        b"",
    );
    exact(
        f.eval("printf '%s\\n' $n3_read_persisted; set -eU n3_read_persisted"),
        0,
        "persisted 雪\n".as_bytes(),
        b"",
    );
    // Editor-only options are accepted but have no startup/prompt effects for
    // redirected input; the typed bytes alone become the assigned value.
    for options in [
        "--command 'startup text'",
        "--prompt 'printf prompt-effect'",
        "--prompt-str literal-prompt",
        "--right-prompt 'printf right-prompt-effect'",
        "--shell",
        "--silent",
        "--command 'startup text' --prompt 'printf prompt-effect' --right-prompt 'printf right-prompt-effect' --shell --silent",
    ] {
        exact(
            f.input(
                &format!("read {options} value; printf '%s:<%s>' $status $value"),
                b"batch value\n",
            ),
            0,
            b"0:<batch value>",
            b"",
        );
    }
    exact(
        f.input("read value; printf '%s' $value", b"\xff\n"),
        0,
        b"\xff",
        b"",
    );
    for code in [
        "read --unknown value",
        "read --nchars nope value",
        "read --nchars -1 value",
        "read --global --local value",
        "read --array first second",
        "read --delimiter=, --tokenize value",
        "read --line --null value",
        "read --line --delimiter=, value",
        "read --prompt 'printf marker' --prompt-str text value",
        "read 'invalid-name'",
        "read status",
        "read --help",
    ] {
        let result = f.input(code, b"must remain input\n");
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty(), "{code}");
        assert!(!result.err.is_empty(), "{code}");
    }
}
