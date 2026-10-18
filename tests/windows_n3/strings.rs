use super::fixture::{Fixture, exact, quote, run};
use std::ffi::OsStr;
use std::fs;

pub(super) fn exercise(f: &Fixture) {
    // Each row executes the production builtin. Long and short spellings deliberately
    // share rows where their combination also exercises option precedence.
    for (name, command, status, output) in [
        ("collect", "string collect alpha beta", 0, "alpha\nbeta\n"),
        (
            "collect/allow-empty",
            "count (string collect --allow-empty)",
            0,
            "1\n",
        ),
        ("collect/a", "count (string collect -a)", 0, "1\n"),
        (
            "collect/no-trim-newlines",
            "string collect --no-trim-newlines 'a\n\n'",
            0,
            "a\n\n\n",
        ),
        ("collect/N", "string collect -N 'a\n'", 0, "a\n\n"),
        (
            "escape/script",
            "string escape --style=script 'a b'",
            0,
            "'a b'\n",
        ),
        (
            "escape/no-quoted",
            "string escape --no-quoted 'a b'",
            0,
            "a\\ b\n",
        ),
        ("escape/n", "string escape -n 'x;y'", 0, "x\\;y\n"),
        (
            "escape/url",
            "string escape --style=url 'a b/雪'",
            0,
            "a%20b/%E9%9B%AA\n",
        ),
        (
            "escape/var",
            "string escape --style=var 'a b'",
            0,
            "a_20_b\n",
        ),
        (
            "escape/regex",
            "string escape --style=regex 'a.b[c]'",
            0,
            "a\\.b\\[c\\]\n",
        ),
        ("join", "string join : a b c", 0, "a:b:c\n"),
        (
            "join/no-empty",
            "string join --no-empty : a '' b",
            0,
            "a:b\n",
        ),
        ("join/n", "string join -n : '' a ''", 1, "a\n"),
        ("join/quiet", "string join --quiet : a b", 0, ""),
        ("join/q", "string join -q : a", 1, ""),
        (
            "join0/no-empty",
            "string join0 --no-empty a '' b",
            0,
            "a\0b\0",
        ),
        ("join0/quiet", "string join0 -q a b", 0, ""),
        ("length/scalars", "string length 'a🐟雪' ''", 0, "3\n0\n"),
        ("length/visible", "string length --visible '雪a'", 0, "3\n"),
        (
            "length/V-escapes",
            "string length -V \\e\\[31mred\\e\\[0m",
            0,
            "3\n",
        ),
        ("length/quiet", "string length --quiet abc", 0, ""),
        ("length/q", "string length -q ''", 1, ""),
        ("lower", "string lower 'ÄBC雪'", 0, "äbc雪\n"),
        ("lower/quiet", "string lower --quiet abc", 1, ""),
        ("lower/q", "string lower -q ABC", 0, ""),
        ("upper", "string upper 'äbc雪'", 0, "ÄBC雪\n"),
        ("upper/quiet", "string upper --quiet ABC", 1, ""),
        ("upper/q", "string upper -q abc", 0, ""),
        ("match/glob", "string match 'a*' abc xyz", 0, "abc\n"),
        (
            "match/entire-ignore-case",
            "string match --entire --ignore-case bc ABCD",
            0,
            "ABCD\n",
        ),
        ("match/ei", "string match -ei bc ABCD", 0, "ABCD\n"),
        (
            "match/invert",
            "string match --invert 'a*' abc xyz",
            0,
            "xyz\n",
        ),
        ("match/v", "string match -v 'a*' xyz", 0, "xyz\n"),
        ("match/quiet", "string match --quiet 'a*' abc", 0, ""),
        ("match/q", "string match -q 'a*' xyz", 1, ""),
        (
            "match/regex-groups-offsets",
            "string match --regex --index '(🐟)(雪)' 'a🐟雪z'",
            0,
            "2 2\n2 1\n3 1\n",
        ),
        (
            "match/all-groups-only",
            "string match --regex --all --groups-only '(a)(b?)' aba",
            0,
            "a\nb\na\n\n",
        ),
        (
            "match/ragn",
            "string match -ragn '(a)' aba",
            0,
            "1 1\n3 1\n",
        ),
        (
            "match/max-matches",
            "string match --max-matches=1 'a*' abc ax",
            0,
            "abc\n",
        ),
        ("match/m", "string match -m 1 'a*' ax ay", 0, "ax\n"),
        (
            "pad/char-width",
            "string pad --char=雪 --width=6 a",
            0,
            "雪雪 a\n",
        ),
        (
            "pad/chars-right",
            "string pad --chars=. --right -w 4 a",
            0,
            "a...\n",
        ),
        (
            "pad/center",
            "string pad --center -c . --width=4 a",
            0,
            "..a.\n",
        ),
        ("pad/rC", "string pad -rC -c . -w 4 a", 0, ".a..\n"),
        ("repeat/positional", "string repeat 3 ab", 0, "ababab\n"),
        (
            "repeat/count-max",
            "string repeat --count=4 --max=5 ab",
            0,
            "ababa\n",
        ),
        ("repeat/nmN", "string repeat -n 4 -m 3 -N ab", 0, "aba"),
        (
            "repeat/no-newline",
            "string repeat --no-newline --count=2 x",
            0,
            "xx",
        ),
        ("repeat/quiet", "string repeat --quiet --count=2 x", 0, ""),
        ("repeat/q", "string repeat -q -n 2 ''", 1, ""),
        (
            "replace/literal",
            "string replace a Z banana",
            0,
            "bZnana\n",
        ),
        (
            "replace/all-ignore-case",
            "string replace --all --ignore-case a X aBA",
            0,
            "XBX\n",
        ),
        (
            "replace/filter",
            "string replace --filter a X banana none",
            0,
            "bXnana\n",
        ),
        (
            "replace/afim",
            "string replace -afi -m 1 a X ABA a",
            0,
            "XBX\n",
        ),
        (
            "replace/unicode-offset-preservation",
            "string replace -i a z 'İA'",
            0,
            "İz\n",
        ),
        (
            "replace/literal-dollar-and-unicode-casefold",
            "string replace -ai é '$1' 'É雪é'",
            0,
            "$1雪$1\n",
        ),
        (
            "replace/max-matches",
            "string replace --max-matches=1 a X aa aa",
            0,
            "Xa\n",
        ),
        ("replace/quiet", "string replace --quiet a X abc", 0, ""),
        ("replace/q", "string replace -q z X abc", 1, ""),
        (
            "replace/regex-named",
            "string replace --regex '(?<word>雪|🐟)' '${word}!' 'a雪b'",
            0,
            "a雪!b\n",
        ),
        (
            "replace/r-captures",
            "string replace -ra '(雪)(🐟)' '$2$1' '雪🐟雪🐟'",
            0,
            "🐟雪🐟雪\n",
        ),
        (
            "shorten/char-max",
            "string shorten --char=. --max=4 abcdef",
            0,
            "abc.\n",
        ),
        (
            "shorten/chars-left",
            "string shorten --chars=.. --left --max=4 abcdef",
            0,
            "..ef\n",
        ),
        ("shorten/cm", "string shorten -c . -m 4 abcdef", 0, "abc.\n"),
        (
            "shorten/l",
            "string shorten -l -c . -m 4 abcdef",
            0,
            ".def\n",
        ),
        (
            "shorten/no-newline",
            "string shorten --no-newline -c . -m 4 'ab\ncd'",
            1,
            "ab.\n",
        ),
        (
            "shorten/N",
            "string shorten -N -c . -m 2 'abcd\nef'",
            0,
            "a.\n",
        ),
        ("shorten/quiet", "string shorten --quiet -m 2 abcdef", 0, ""),
        ("shorten/q", "string shorten -q -m 20 abc", 1, ""),
        ("split", "string split : 'a:b:'", 0, "a\nb\n\n"),
        (
            "split/right-max",
            "string split --right --max=1 : a:b:c",
            0,
            "a:b\nc\n",
        ),
        ("split/rm", "string split -rm 1 : a:b:c", 0, "a:b\nc\n"),
        (
            "split/no-empty",
            "string split --no-empty : ':a::b:'",
            0,
            "a\nb\n",
        ),
        ("split/n", "string split -n : ':a:'", 1, "a\n"),
        (
            "split/fields",
            "string split --fields=3,1 : a:b:c",
            0,
            "c\na\n",
        ),
        ("split/fa", "string split -f 4 -a : a:b", 0, ""),
        (
            "split/allow-empty",
            "string split --fields=4 --allow-empty : a:b",
            0,
            "",
        ),
        ("split/quiet", "string split --quiet : a:b", 0, ""),
        ("split/q", "string split -q : abc", 1, ""),
        (
            "split0/explicit-elements",
            "printf '<%s>\\n' (printf 'a\\0b\\nq\\0' | string split0)",
            0,
            "<a>\n<b\nq>\n",
        ),
        (
            "sub/start-length",
            "string sub --start=2 --length=2 'a🐟雪z'",
            0,
            "🐟雪\n",
        ),
        (
            "sub/sle",
            "string sub -s -3 -l 2 abcdef; string sub -s 2 -e -1 abcdef",
            0,
            "de\nbcde\n",
        ),
        ("sub/end", "string sub --end=3 abcdef", 0, "abc\n"),
        ("sub/quiet", "string sub --quiet --start=99 abc", 0, ""),
        ("sub/q", "string sub -q -s 1 abc", 0, ""),
        ("trim", "string trim '  a b  '", 0, "a b\n"),
        (
            "trim/chars-left",
            "string trim --chars=x --left xxxaxxx",
            0,
            "axxx\n",
        ),
        ("trim/right", "string trim --right ' a  '", 0, " a\n"),
        ("trim/clr", "string trim -c x -lr xxxaxxx", 0, "a\n"),
        ("trim/quiet", "string trim --quiet ' a '", 0, ""),
        ("trim/q", "string trim -q abc", 1, ""),
        (
            "unescape/script",
            "string unescape --style=script 'a\\ b'",
            0,
            "a b\n",
        ),
        (
            "unescape/url",
            "string unescape --style=url 'a%20b%2F%E9%9B%AA'",
            0,
            "a b/雪\n",
        ),
        (
            "unescape/var",
            "string unescape --style=var a_20_b",
            0,
            "a b\n",
        ),
        (
            "unescape/no-quoted",
            "string unescape --no-quoted 'a\\ b'",
            0,
            "a b\n",
        ),
        ("unescape/n", "string unescape -n 'x\\;y'", 0, "x;y\n"),
        (
            "unescape/invalid-data",
            "string unescape --style=url '%GG'",
            1,
            "",
        ),
    ] {
        let result = f.eval(command);
        assert_eq!(
            result.code,
            status,
            "{name}: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert_eq!(result.out, output.as_bytes(), "{name}");
        assert!(
            result.err.is_empty(),
            "{name}: {}",
            String::from_utf8_lossy(&result.err)
        );
    }

    exact(
        f.eval("string match -ra '(?<piece>雪|🐟)(?<tail>x)?' '雪🐟x'; printf 'pieces=<%s>\\n' $piece; printf 'tails=<%s>\\n' $tail"),
        0,
        "雪\n雪\n🐟x\n🐟\nx\npieces=<雪>\npieces=<🐟>\ntails=<>\ntails=<x>\n".as_bytes(),
        b"",
    );
    exact(
        f.eval("string match -rn '\\p{L}+' '🐟雪é'"),
        0,
        b"2 2\n",
        b"",
    );
    exact(f.eval("string match -ra '(?=.)' ab"), 0, b"\n\n", b"");
    exact(
        f.eval("string match -r '(?<empty>a)?b' b; count $empty"),
        1,
        b"b\n0\n",
        b"",
    );

    // Literal private-use Unicode must remain distinct from raw bytes. Like
    // Unix fish, reserved scalars use the canonical encoded-byte representation;
    // their internal character counts are not Unicode scalar counts.
    let private_source = f.root.join("private Unicode regex.fish");
    fs::write(
        &private_source,
        "string match -rg '(\u{f600})(\u{f6ff})' '\u{f600}\u{f6ff}'\n\
         string replace -r '(\u{f600})(\u{f6ff})' '$2$1' '\u{f600}\u{f6ff}'\n",
    )
    .unwrap();
    exact(
        f.eval(&format!(
            "source {}",
            quote(private_source.to_str().unwrap())
        )),
        0,
        "\u{f600}\n\u{f6ff}\n\u{f6ff}\u{f600}\n".as_bytes(),
        b"",
    );
    let mut distinct_private_input = vec![0xff, b'\n'];
    distinct_private_input.extend_from_slice("\u{f6ff}\n".as_bytes());
    exact(
        f.input("string match -r '(\u{f6ff})' <&0", &distinct_private_input),
        0,
        "\u{f6ff}\n\u{f6ff}\n".as_bytes(),
        b"",
    );
    exact(
        f.input("string match -rv '\u{f6ff}' <&0", &distinct_private_input),
        0,
        &[0xff, b'\n'],
        b"",
    );

    // As on Unix, an explicit redirect selects stdin rather than argv. Preserve
    // byte/record boundaries, CR, invalid UTF-8, NULs and explicit elements.
    exact(
        f.input("string lower <&0", "ÄBC\n雪".as_bytes()),
        0,
        "äbc\n雪".as_bytes(),
        b"",
    );
    exact(f.input("string length <&0", b"a\r\nb"), 0, b"2\n1\n", b"");
    exact(
        f.input("string escape -n <&0", &[0xff, b'\n']),
        0,
        b"\\Xff\n",
        b"",
    );
    exact(
        f.input("string split0 <&0", b"a\0b\nq\0"),
        0,
        b"a\nb\nq\n",
        b"",
    );
    exact(
        f.input("string split0 --right --max=1 <&0", b"a\0b\0c"),
        0,
        b"a\0b\nc\n",
        b"",
    );
    exact(f.input("string split0 --quiet <&0", b"a\0b"), 0, b"", b"");
    exact(
        f.input(
            "string split0 --no-empty --fields=2,5 --allow-empty <&0",
            b"a\0\0b\0",
        ),
        0,
        b"b\n",
        b"",
    );
    exact(f.input("string collect -N <&0", b"a\n\n"), 0, b"a\n\n", b"");
    exact(
        f.eval("count (printf 'a\\nb\\n' | string collect)"),
        0,
        b"1\n",
        b"",
    );
    exact(f.eval("count (printf '' | string collect)"), 1, b"0\n", b"");
    exact(
        f.eval("count (printf '' | string collect -a)"),
        0,
        b"1\n",
        b"",
    );
    exact(f.input("string join0 <&0", b"a\nb\n"), 0, b"a\0b\0", b"");
    exact(
        f.input("string replace -r '(雪)' '$1$1' <&0", "雪\n🐟".as_bytes()),
        0,
        "雪雪\n🐟".as_bytes(),
        b"",
    );

    // One scalar straddles Arguments' 1024-byte input chunk, not just a UTF-32
    // index boundary. The following record must remain a distinct input value.
    let boundary_input = format!("{}🐟雪\nnext\n", "a".repeat(1023));
    exact(
        f.input("string match -rn '(🐟)' <&0", boundary_input.as_bytes()),
        0,
        b"1024 1\n1024 1\n",
        b"",
    );

    // Pattern captures are interpreted by PCRE2 in both feature modes, while
    // the legacy mode adds exactly one fish-unescape pass to the replacement.
    for (feature, replacements) in [
        ("regex-easyesc", [("\\$1", "$1\n"), ("\\\\$1", "\\x\n")]),
        ("no-regex-easyesc", [("\\$1", "x\n"), ("\\\\$1", "$1\n")]),
    ] {
        for (replacement, output) in replacements {
            let option = format!("--features={feature}");
            let code = format!("string replace -r '(x)' {} x", quote(replacement));
            exact(
                run(
                    &mut f.command(&[
                        OsStr::new("-N"),
                        OsStr::new(&option),
                        OsStr::new("-c"),
                        OsStr::new(&code),
                    ]),
                    b"",
                ),
                0,
                output.as_bytes(),
                b"",
            );
        }
        let option = format!("--features={feature}");
        let code = format!("string replace -r '(x)' {} x", quote("\\c"));
        let invalid = run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new(&option),
                OsStr::new("-c"),
                OsStr::new(&code),
            ]),
            b"",
        );
        assert_eq!(invalid.code, 2, "{feature}");
        assert!(invalid.out.is_empty(), "{feature}");
        assert!(
            !invalid.err.is_empty(),
            "{feature}: invalid replacement must diagnose its failure"
        );
    }

    // Inferred regex output charges its LF, whereas collect/split0 preserve
    // explicit elements without charging a serialized separator as content.
    for (producer, limit, expected) in [
        ("string match -r '^.*$' 1234567", 8, "<1234567>"),
        ("string collect 12345678", 8, "<12345678>"),
    ] {
        exact(
            f.eval(&format!(
                "set -g fish_read_limit {limit}; printf '<%s>' ({producer})"
            )),
            0,
            expected.as_bytes(),
            b"",
        );
    }
    // A deferred internal producer also owns a bounded byte buffer. Read the
    // NUL-delimited source directly to isolate the collector's payload budget.
    exact(
        f.input(
            "set -g fish_read_limit 8; printf '<%s>' (string split0 <&0)",
            b"12345678\0",
        ),
        0,
        b"<12345678>",
        b"",
    );
    let many_empty_arguments = format!("string collect {}", "'' ".repeat(1024));
    for (name, producer, limit) in [
        ("regex-byte-limit", "string match -r '^.*$' 12345678", 8),
        ("collect-byte-limit", "string collect 123456789", 8),
        (
            "split0-byte-limit",
            "printf '123456789\\0' | string split0",
            8,
        ),
        (
            "collect-empty-metadata-limit",
            many_empty_arguments.as_str(),
            1024,
        ),
    ] {
        let marker = f.root.join(format!("{name}.txt"));
        let result = f.eval(&format!(
            "set -g fish_read_limit {limit}; {} marker {} ({producer})",
            f.probe(),
            quote(marker.to_str().unwrap()),
        ));
        assert_eq!(
            result.code,
            122,
            "{name}: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert!(result.out.is_empty(), "{name}");
        assert!(
            !result.err.is_empty(),
            "{name}: rejected expansion must diagnose its failure"
        );
        assert!(
            !marker.exists(),
            "{name}: rejected expansion invoked its consumer"
        );
    }
    let empty_marker = f.root.join("split0 empty retention must not launch.txt");
    let result = f.input(
        &format!(
            "set -g fish_read_limit 1024; {} marker {} (string split0 <&0)",
            f.probe(),
            quote(empty_marker.to_str().unwrap()),
        ),
        &[0; 4096],
    );
    assert_eq!(result.code, 122);
    assert!(result.out.is_empty());
    assert!(!result.err.is_empty());
    assert!(!empty_marker.exists());

    // A genuine native exit 122 is not a capture-retention fault. Its empty
    // substitution still permits the consuming command to run.
    let ordinary_exit_marker = f.root.join("ordinary native exit 122.txt");
    exact(
        f.eval(&format!(
            "{} marker {} ({} exit 122)",
            f.probe(),
            quote(ordinary_exit_marker.to_str().unwrap()),
            f.probe(),
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(ordinary_exit_marker).unwrap(), b"launched");
    let recovered_marker = f.root.join("failed earlier capture must not launch.txt");
    let failed_command = format!(
        "set -g fish_read_limit 8; {} marker {} (printf '123456789\\0' | string split0)",
        f.probe(),
        quote(recovered_marker.to_str().unwrap()),
    );
    let recovered = run(
        &mut f.command(&[
            OsStr::new("-N"),
            OsStr::new("-c"),
            OsStr::new(&failed_command),
            OsStr::new("-c"),
            OsStr::new("printf 'recovered:%s' (printf ok)"),
        ]),
        b"",
    );
    assert_eq!(recovered.code, 0);
    assert_eq!(recovered.out, b"recovered:ok");
    assert!(!recovered.err.is_empty());
    assert!(!recovered_marker.exists());
    exact(f.eval("string match -r '(a)' b"), 1, b"", b"");
    exact(f.eval("string replace -r '(a)' '$1$1' b"), 1, b"b\n", b"");

    for command in [
        "string match -r '[' x",
        "string match -r '(*UTF)a' a",
        "string match -r '(?<status>x)' x",
        "string replace -r '[' x y",
        "string replace -r '(a)' '$9' a",
        "string match -en x x",
        "string match -vg x x",
        "string match -eg x x",
        "string match -m 0 x x",
        "string replace -m 0 x y x",
        "string sub -s 0 abc",
        "string sub -l 1 -e 2 abc",
        "string pad -c '' abc",
        "string pad -c \\n abc",
        "string repeat --count=-1 abc",
        "string escape --style=bogus abc",
        "string unescape --style=bogus abc",
        "printf x | string length argv",
        "string length --bogus x",
    ] {
        let result = f.eval(command);
        assert_eq!(
            result.code,
            2,
            "{command}: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert!(result.out.is_empty(), "{command}: unexpected output");
        assert!(
            !result.err.is_empty(),
            "{command}: invalid arguments must diagnose their failure"
        );
    }
    let result =
        f.eval("string match -r '(*LIMIT_MATCH=1)(a+)+$' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab");
    assert_eq!(result.code, 2);
    assert!(
        !result.err.is_empty(),
        "regex resource failure must be diagnosed"
    );
}
