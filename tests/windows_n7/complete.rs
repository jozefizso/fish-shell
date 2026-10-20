use super::{Family, console::Session, editor::{start, capture, finish, observe}, fixture::Fixture};
use std::fs;

pub(super) const FAMILIES: &[Family] = &[
    ("evaluated-dynamic-provider-and-commandline-scope", dynamic),
    ("condition-cache-is-local-to-wrapper-scope", conditions),
    ("wrapper-chains-cycles-and-assignments", wrappers),
    ("autoload-negative-positive-replacement-and-removal", autoload),
    ("autoload-failure-rollback-and-retry", autoload_failure),
    ("autoload-search-path-and-relative-cwd-invalidation", autoload_paths),
    ("native-path-pathext-execution-policy", native_commands),
    ("wildcard-command-and-native-path-rule-matching", wildcard_rules),
    ("filesystem-cwd-and-changed-file-completion", files),
    ("quoted-unicode-insertion-cursor-and-execution", insertion),
    ("provider-errors-invalid-definitions-and-recovery", errors),
    ("condition-environment-and-function-definition-changes", definitions),
    ("inaccessible-autoload-path-recovery", inaccessible),
    ("complete-search-filter-toggle-select-and-accept", pager),
    ("history-pager-publication-delete-and-accept", history_pager),
    ("autoload-wrapper-ownership-and-replacement", autoload_wrappers),
    ("first-tab-autoload-and-side-effecting-provider", first_tab_provider),
    ("provider-cursor-mutation-rejects-obsolete-insertion", provider_cursor),
    ("assignment-wrapper-cycles-terminate-with-bounded-evaluation", assignment_cycles),
    ("autoload-preserves-manual-rules-before-after-reload-and-failure", manual_rules),
];

fn eval(f: &Fixture, code: &str, expected: &[u8]) {
    let result = f.eval(code);
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert_eq!(result.out, expected);
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
}

fn dynamic(f: &Fixture) {
    eval(f, "function n7cmd; end; function n7provider; test (commandline -opc)[1] = n7cmd; or return 1; printf '%s\\n' $n7_values; end; complete -c n7cmd -f -a '(n7provider)'; set -g n7_values alpha; complete -C 'n7cmd a'; set -g n7_values albatross; complete -C 'n7cmd a'", b"alpha\nalbatross\n");
}

fn conditions(f: &Fixture) {
    eval(f, "function n7base; end; function n7wrap; end; complete -c n7base -f -a visible -n 'test \"$N7_SCOPE\" = yes'; complete -c n7wrap -w 'N7_SCOPE=no n7base'; complete -c n7wrap -w 'N7_SCOPE=yes n7base'; complete -C 'n7wrap v'", b"visible\n");
}

fn wrappers(f: &Fixture) {
    eval(f, "function n7base; end; function n7middle; end; function n7outer; end; complete -c n7base -f -a '(printf wrapped)'; complete -c n7middle -w n7base; complete -c n7outer -w n7middle; complete -c n7base -w n7outer; complete -C 'n7outer w'; complete -c n7middle -e -w n7base; complete -C 'n7outer w'", b"wrapped\n");
}

fn autoload(f: &Fixture) {
    fs::create_dir(f.root.join("completions")).unwrap();
    let source = "set -g fish_complete_path ./completions; function n7auto; end; complete -C 'n7auto a' | string match -r '^a(lpha|lbatross)$'; printf '%s\\n' 'complete -c n7auto -f -a alpha' > completions/n7auto.fish; complete -C 'n7auto a'; printf '%s\\n' 'complete -c n7auto -f -a albatross' > completions/n7auto.fish; complete -C 'n7auto a'";
    eval(f, source, b"alpha\nalbatross\n");
    let mut s = start(f, "set -g fish_complete_path ./completions; function n7auto; end; bind f6 'complete -C \"n7auto alb\" > before; printf ready > loaded'; bind f7 'complete -C \"n7auto alb\" > after; printf ready > removed'");
    s.send(b"\x1b[17~");
    assert_eq!(s.wait_file(&f.root.join("loaded")), b"ready");
    assert_eq!(fs::read(f.root.join("before")).unwrap(), b"albatross\n");
    fs::remove_file(f.root.join("completions/n7auto.fish")).unwrap();
    s.send(b"\x1b[18~");
    assert_eq!(s.wait_file(&f.root.join("removed")), b"ready");
    assert!(fs::read(f.root.join("after")).unwrap().is_empty());
    finish(s);
}

fn autoload_failure(f: &Fixture) {
    fs::create_dir(f.root.join("completions")).unwrap();
    eval(f, "set -g fish_complete_path ./completions; function n7retry; end; printf '%s\\n' 'complete -c n7retry -f -a alpha' > completions/n7retry.fish; complete -C 'n7retry a'; printf '%s\\n' 'complete -c n7retry -f -a abandoned; false' > completions/n7retry.fish; complete -C 'n7retry a'; printf '%s\\n' 'complete -c n7retry -f -a accepted' > completions/n7retry.fish; complete -C 'n7retry a'", b"alpha\nalpha\naccepted\n");
}

fn autoload_paths(f: &Fixture) {
    for parent in ["one", "two"] {
        fs::create_dir_all(f.root.join(parent).join("completions")).unwrap();
        fs::write(f.root.join(parent).join("completions/n7relative.fish"), format!("complete -c n7relative -f -a {parent}\n")).unwrap();
    }
    eval(f, "function n7relative; end; set -g fish_complete_path ./completions; cd one; complete -C 'n7relative '; cd ../two; complete -C 'n7relative '; set -g fish_complete_path ../one/completions; complete -C 'n7relative '", b"one\ntwo\none\n");
}

fn native_commands(f: &Fixture) {
    fs::create_dir(f.root.join("bin")).unwrap();
    fs::copy(&f.probe, f.root.join("bin/n7native.exe")).unwrap();
    for extension in ["cmd", "bat", "ps1", "fish"] {
        fs::write(f.root.join(format!("bin/n7optional.{extension}")), "echo provider\n").unwrap();
    }
    eval(f, "set -g PATH ./absent; complete -C n7nat; set -g PATH ./bin; set -g PATHEXT .CMD .BAT .PS1 .EXE; complete -C n7nat | string split -f1 \\t; set -l providers (complete -C n7optional | string split -f1 \\t); for candidate in n7optional.cmd n7optional.bat n7optional.ps1 n7optional.fish; contains -- $candidate $providers; or return 1; end; printf '%s\\n' provider-discovery; set -g PATH ./absent; complete -C n7nat", b"n7native.exe\nprovider-discovery\n");
    // The accepted discovered image runs the same central native execution path.
    let result = f.eval("set -g PATH ./bin; n7native.exe exit 0");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    eval(f, "set -g PATH ./bin; set -g PATHEXT .FISH; n7optional", b"provider\n");
}

fn wildcard_rules(f: &Fixture) {
    fs::create_dir(f.root.join("bin")).unwrap();
    fs::copy(&f.probe, f.root.join("bin/n7wildname.exe")).unwrap();
    eval(f, r"set -g PATH ./bin; complete -c 'n7wild*' -f -a n7name; complete -C 'n7wildname n7n'; complete -c 'n7wild*' -e; complete -p '*\bin\n7wild*.exe' -f -a n7path; complete -C 'n7wildname n7p'; complete -p '*\bin\n7wild*.exe' -e; complete -p '*/bin/n7wild*.exe' -f -a n7forward; complete -C 'n7wildname n7f'", b"n7name\nn7path\nn7forward\n");
}

fn files(f: &Fixture) {
    for directory in ["first", "second"] { fs::create_dir(f.root.join(directory)).unwrap(); }
    fs::write(f.root.join("first/item-one"), b"").unwrap();
    fs::write(f.root.join("second/item-two"), b"").unwrap();
    eval(f, "cd first; complete -C 'echo item'; cd ../second; complete -C 'echo item'", b"item-one\nitem-two\n");
    let mut s = start(f, "bind f6 'complete -C \"echo first/item\" > cached; printf ready > cached-ready'; bind f7 'complete -C \"echo first/item\" > changed; printf ready > changed-ready'");
    s.send(b"\x1b[17~");
    s.wait_file(&f.root.join("cached-ready"));
    fs::rename(f.root.join("first/item-one"), f.root.join("first/item-new")).unwrap();
    s.send(b"\x1b[18~");
    s.wait_file(&f.root.join("changed-ready"));
    assert_eq!(fs::read(f.root.join("changed")).unwrap(), b"first/item-new\n");
    finish(s);
}

fn insertion(f: &Fixture) {
    fs::write(f.root.join("雪 space 🐟.txt"), b"actual").unwrap();
    let mut s = start(f, "function n7take; printf '<%s>' $argv > accepted; end");
    s.send("n7take '' '雪\t".as_bytes());
    let expected = "n7take '' '雪 space 🐟.txt' ";
    assert_eq!(capture(&mut s, f, 1, expected), expected.chars().count());
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("accepted")), "<><雪 space 🐟.txt>".as_bytes());
    finish(s);
}

fn errors(f: &Fixture) {
    eval(f, "function n7bad; end; complete -c n7bad -f; complete -c n7bad -f -a forbidden -n false; complete -C 'n7bad f'; complete -c n7bad --not-an-option 2> invalid; set -l rejected $status; test $rejected -ne 0; or exit 91; complete -c n7bad -n 'end' -a leaked 2> invalid-condition; test $status -ne 0; or exit 92; complete -c n7bad -a '(' 2> invalid-arguments; test $status -ne 0; or exit 93; complete -c n7bad -f -a recovered; complete -C 'n7bad r'; complete -c n7bad -f -a '(false)'; complete -C 'n7bad r'", b"recovered\nrecovered\n");
    for (file, input) in [("invalid", "--not-an-option"), ("invalid-condition", "end"), ("invalid-arguments", "(")] {
        let diagnostic = fs::read_to_string(f.root.join(file)).unwrap();
        assert!(diagnostic.contains(input), "rejected input must be identified: {diagnostic}");
    }
}

fn definitions(f: &Fixture) {
    eval(f, "function n7change; end; function n7value; printf old; end; set -g n7allow yes; complete -c n7change -f; complete -c n7change -f -a '(n7value)' -n 'test $n7allow = yes'; complete -C 'n7change '; set -g n7allow no; complete -C 'n7change '; function n7value; printf new; end; set -g n7allow yes; complete -C 'n7change '", b"old\nnew\n");
}

fn await_buffer(s: &mut Session, f: &Fixture, next: &mut usize, expected: &str) -> usize {
    // Each iteration is an actual closed-file main-thread callback acknowledgement,
    // not a timer or an assumed worker duration.
    let mut actual = String::new();
    for _ in 0..128 {
        let (line, cursor) = observe(s, f, *next);
        *next += 1;
        if line == expected { return cursor; }
        actual = line;
    }
    panic!("reader publication never reached {expected:?}; actual {actual:?}");
}

fn inaccessible(f: &Fixture) {
    use std::os::windows::fs::OpenOptionsExt;
    fs::create_dir(f.root.join("completions")).unwrap();
    let path = f.root.join("completions/n7denied.fish");
    fs::write(&path, "complete -c n7denied -f -a zzzavailable\n").unwrap();
    let denied = fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
    let mut s = start(f, "set -g fish_complete_path ./completions; function n7denied; end; bind f6 'complete -C \"n7denied zzz\" > denied-result; printf ready > denied-ready'; bind f7 'complete -C \"n7denied zzz\" > recovered-result; printf ready > recovered-ready'; bind f8 'complete -C \"n7denied zzz\" > denied-again; printf ready > denied-again-ready'; bind f9 'complete -C \"n7denied zzz\" > unchanged-recovered; printf ready > unchanged-ready'");
    s.send(b"\x1b[17~");
    s.wait_file(&f.root.join("denied-ready"));
    assert!(fs::read(f.root.join("denied-result")).unwrap().is_empty());
    drop(denied);
    s.send(b"\x1b[18~");
    s.wait_file(&f.root.join("recovered-ready"));
    assert_eq!(fs::read(f.root.join("recovered-result")).unwrap(), b"zzzavailable\n");
    // Temporarily remove the loaded search entry without rewriting its bytes or file identity.
    let parked = f.root.join("completions/parked.fish");
    fs::rename(&path, &parked).unwrap();
    s.send(b"\x1b[19~");
    s.wait_file(&f.root.join("denied-again-ready"));
    assert!(fs::read(f.root.join("denied-again")).unwrap().is_empty());
    fs::rename(&parked, &path).unwrap();
    s.send(b"\x1b[20~");
    s.wait_file(&f.root.join("unchanged-ready"));
    assert_eq!(fs::read(f.root.join("unchanged-recovered")).unwrap(), b"zzzavailable\n");
    finish(s);
}

fn pager(f: &Fixture) {
    let mut s = start(f, "function n7pick; printf '%s' $argv > picked; end; complete -c n7pick -f -a 'alpha beta' -d N7_PICK_READY; bind f6 complete-and-search; bind f7 pager-toggle-search");
    s.send(b"n7pick \x1b[17~");
    s.wait_text(b"N7_PICK_READY");
    s.send(b"beta");
    let mut next = 1;
    let expected = "n7pick beta ";
    assert_eq!(await_buffer(&mut s, f, &mut next, expected), expected.len());
    s.send(b"\x1b[18~");
    assert_eq!(await_buffer(&mut s, f, &mut next, expected), expected.len());
    s.send(b"\r");
    assert_eq!(await_buffer(&mut s, f, &mut next, expected), expected.len());
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("picked")), b"beta");
    finish(s);
}

fn history_pager(f: &Fixture) {
    let mut s = start(f, "set -g fish_history n7_pager; set -g fish_autosuggestion_enabled 0; function n7history; printf '%s' $argv > history-picked; end; builtin history append 'n7history older' 'n7history newest'; bind f6 history-pager; bind f7 history-pager-delete");
    s.send(b"n7history \x1b[17~");
    s.wait_text(b"newest");
    let mut next = 1;
    let selected = "n7history newest";
    assert_eq!(await_buffer(&mut s, f, &mut next, selected), selected.len());
    s.send(b"\x1b[18~");
    let remaining = "n7history older";
    assert_eq!(await_buffer(&mut s, f, &mut next, remaining), remaining.len());
    s.send(b"\r");
    assert_eq!(await_buffer(&mut s, f, &mut next, remaining), remaining.len());
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("history-picked")), b"older");
    s.send(b"builtin history search --exact 'n7history newest' > deleted-history; printf ready > deleted-history-ready\r");
    s.wait_file(&f.root.join("deleted-history-ready"));
    assert!(fs::read(f.root.join("deleted-history")).unwrap().is_empty());
    finish(s);
}

fn autoload_wrappers(f: &Fixture) {
    fs::create_dir(f.root.join("completions")).unwrap();
    eval(f, "function n7auto; end; function n7first; end; function n7second; end; complete -c n7first -f -a first; complete -c n7second -f -a second; set -g fish_complete_path ./completions; printf '%s\\n' 'complete -c n7auto -w n7first' > completions/n7auto.fish; complete -C 'n7auto '; printf '%s\\n' 'complete -c n7auto -w n7second' > completions/n7auto.fish; complete -C 'n7auto '", b"first\nsecond\n");
}

fn first_tab_provider(f: &Fixture) {
    fs::create_dir(f.root.join("completions")).unwrap();
    fs::write(f.root.join("completions/n7counter.fish"), "set -g n7loaded 1; complete -c n7counter -f -a '(n7counter_provider)'\n").unwrap();
    let mut s = start(f, "set -g fish_complete_path ./completions; set -g n7calls 0; set -g n7loaded 0; function n7counter; end; function n7counter_provider; set -g n7calls (math $n7calls + 1); printf option; end; bind f6 'printf \"%s:%s\" $n7calls $n7loaded > invoked'");
    s.send(b"n7counter o\t");
    let expected = "n7counter option ";
    assert_eq!(capture(&mut s, f, 1, expected), expected.len());
    s.send(b"\x1b[17~");
    assert_eq!(s.wait_file(&f.root.join("invoked")), b"1:1");
    finish(s);
}

fn provider_cursor(f: &Fixture) {
    let mut s = start(f, "function n7cursor; end; function n7cursor_provider; commandline --column 3; printf option; end; complete -c n7cursor -f -a '(n7cursor_provider)'");
    s.send(b"n7cursor o\t");
    assert_eq!(capture(&mut s, f, 1, "n7cursor o"), 2);
    finish(s);
}

fn assignment_cycles(f: &Fixture) {
    eval(f, "function n7cycle; end; function n7leaf; end; set -g n7visits 0; complete -c n7cycle -f -a candidate -n 'set -g n7visits (math $n7visits + 1); true'; complete -c n7leaf -f -a leaf -n 'set -g n7visits (math $n7visits + 1); true'; complete -c n7cycle -w 'X=0 n7cycle'; complete -c n7cycle -w 'X=1 n7cycle'; complete -c n7cycle -w n7leaf; complete -c n7leaf -w 'X=2 n7cycle'; complete -C 'n7cycle '; printf '%s\\n' $n7visits", b"candidate\nleaf\n2\n");
}

fn manual_rules(f: &Fixture) {
    fs::create_dir(f.root.join("completions")).unwrap();
    fs::create_dir(f.root.join("empty-completions")).unwrap();
    let path = f.root.join("completions/n7manual.fish");
    fs::write(&path, "complete -c n7manual -e; complete -c n7manual -f -l providerone\n").unwrap();
    let mut s = start(f, "set -g fish_complete_path ./completions; function n7manual; end; complete -c n7manual -f -l localbefore; bind f6 'complete -C \"n7manual --\" > manual-first; printf ready > manual-first-ready'; bind f7 'complete -c n7manual -l localafter; complete -C \"n7manual --\" > manual-reload; printf ready > manual-reload-ready'; bind f8 'complete -C \"n7manual --\" > manual-rollback; printf ready > manual-rollback-ready'; bind f9 'complete -C \"n7manual --\" > manual-removed; printf ready > manual-removed-ready'; bind f10 'set -g fish_complete_path ./empty-completions; complete -C \"n7manual --\" > manual-path; printf ready > manual-path-ready'; bind f11 'set -g fish_complete_path ./completions; complete -C \"n7manual --\" > manual-restored; printf ready > manual-restored-ready'");
    s.send(b"n7manual --loc\t");
    let expected = "n7manual --localbefore ";
    assert_eq!(capture(&mut s, f, 1, expected), expected.len());
    s.send(b"\x03\x1b[17~");
    s.wait_file(&f.root.join("manual-first-ready"));
    assert_eq!(fs::read(f.root.join("manual-first")).unwrap(), b"--localbefore\n--providerone\n");
    fs::write(&path, "complete -c n7manual -e; complete -c n7manual -f -l providertwo\n").unwrap();
    s.send(b"\x1b[18~");
    s.wait_file(&f.root.join("manual-reload-ready"));
    let stable = b"--localafter\n--localbefore\n--providertwo\n";
    assert_eq!(fs::read(f.root.join("manual-reload")).unwrap(), stable);
    fs::write(&path, "complete -c n7manual -e; complete -c n7manual -f -l partial; false\n").unwrap();
    s.send(b"\x1b[19~");
    s.wait_file(&f.root.join("manual-rollback-ready"));
    assert_eq!(fs::read(f.root.join("manual-rollback")).unwrap(), stable);
    // The failed source's last successful rules still have ownership even though
    // its autoload file identity was deliberately forgotten to permit retry.
    s.send(b"\x1b[21~");
    s.wait_file(&f.root.join("manual-path-ready"));
    assert_eq!(fs::read(f.root.join("manual-path")).unwrap(), b"--localafter\n--localbefore\n");
    fs::write(&path, "complete -c n7manual -e; complete -c n7manual -f -l providerthree\n").unwrap();
    s.send(b"\x1b[23~");
    s.wait_file(&f.root.join("manual-restored-ready"));
    assert_eq!(fs::read(f.root.join("manual-restored")).unwrap(), b"--localafter\n--localbefore\n--providerthree\n");
    fs::remove_file(&path).unwrap();
    s.send(b"\x1b[20~");
    s.wait_file(&f.root.join("manual-removed-ready"));
    assert_eq!(fs::read(f.root.join("manual-removed")).unwrap(), b"--localafter\n--localbefore\n");
    finish(s);
}
