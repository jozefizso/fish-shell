use super::{Family, editor::{capture, finish, start}, fixture::{Fixture, exact, quote}};
use std::{collections::BTreeSet, fs};

pub(super) const FAMILIES: &[Family] = &[
    ("builtin-abbreviation-callback-rename-position-and-erase", abbreviations),
    ("builtin-argparse-exclusive-cursor-and-real-parser-boundaries", argument_parser),
    ("builtin-binding-key-function-and-mode-transitions", bindings),
    ("builtin-dispatch-function-copy-and-completion-registration", dispatch),
    ("builtin-variable-scope-erasure-colors-and-containment", variables),
    ("builtin-status-feature-assets-language-and-state-consumers", status),
    ("builtin-owned-job-query-wait-any-and-disown-transitions", jobs),
    ("builtin-wrapper-negation-source-and-bracket-consumers", wrappers),
];

// These helpers select the actual public completion result, not a duplicate of
// the table's candidate expressions. Preserve the caller's local-variable scope.
const SELECT: &str = r#"
function n9_candidates --no-scope-shadowing
    complete -C "$argv" | string split -f 1 \t
end
function n9_select --no-scope-shadowing
    set -l n9_matches (n9_candidates "$argv")
    test (count $n9_matches) -eq 1; or return 81
    printf '%s\n' $n9_matches
end
"#;

fn success(f: &Fixture, code: &str) -> String {
    let result = f.eval(&format!("{SELECT}\n{code}"));
    assert_eq!(result.code, 0, "{code}\n{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{code}\n{}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn values(output: &str) -> BTreeSet<&str> {
    output.lines().map(|line| line.split('\t').next().unwrap()).collect()
}

fn abbreviations(f: &Fixture) {
    let init = format!(r#"{SELECT}
fish_default_key_bindings
function n9_abbr_callback
    printf "printf 'expanded 雪' > abbr-consumed"
end
set -l callback (n9_select 'abbr --add --function n9_abbr_call'); or exit 81
set -l position (n9_select 'abbr --add --position comm'); or exit 82
abbr --add --position $position --function $callback n9_before; or exit 83
set -l previous (n9_select 'abbr --rename n9_bef'); or exit 84
abbr --rename $previous n9_after; or exit 85
abbr --query n9_after; or exit 86
abbr --query n9_before; and exit 87
test (count (n9_candidates 'abbr --erase n9_bef')) -eq 0; or exit 88
"#);
    let mut s = start(f, &init);
    s.send(b"n9_after ");
    capture(&mut s, f, 1, "printf 'expanded 雪' > abbr-consumed ");
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("abbr-consumed")), "expanded 雪".as_bytes());
    s.send(b"printf '%s' n9_after ");
    capture(&mut s, f, 2, "printf '%s' n9_after ");
    s.send(b"\x03abbr --erase (n9_select 'abbr --erase n9_aft'); or exit 89; abbr --query n9_after; and exit 90; printf ready > abbr-erased\r");
    s.wait_file(&f.root.join("abbr-erased"));
    s.send(b"n9_after ");
    capture(&mut s, f, 3, "n9_after ");
    s.send("\x03abbr --add --position (n9_select 'abbr --add --position anyw') n9_anywhere \"'argument 雪'\"; or exit 91; printf ready > abbr-anywhere-ready\r".as_bytes());
    s.wait_file(&f.root.join("abbr-anywhere-ready"));
    s.send(b"printf '%s' n9_anywhere ");
    capture(&mut s, f, 4, "printf '%s' 'argument 雪' ");
    s.send(b"> abbr-anywhere\r");
    assert_eq!(s.wait_file(&f.root.join("abbr-anywhere")), "argument 雪".as_bytes());
    finish(s);
    for code in ["abbr --rename n9_missing n9_new", "abbr --add --position invalid n9_invalid value"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty() && !result.err.is_empty(), "{code}");
    }
}

fn argument_parser(f: &Fixture) {
    let output = success(f, r#"
function n9_parser_label; end
set -l name (n9_select 'argparse --name n9_parser_lab'); or exit 81
argparse --name $name a/alpha b/beta= c#count -- --alpha --beta='value 雪' -7 rest; or exit 82
 test "$_flag_alpha" = "$_flag_a"; or exit 83
 set -q _flag_alpha; or exit 84
 test "$_flag_beta" = 'value 雪'; or exit 85
 test "$_flag_count" = 7; or exit 86
 test "$argv" = rest; or exit 87
set -l move (n9_select 'argparse --move-unk'); or exit 88
argparse $move a/alpha -- --alpha --unknown value; or exit 89
 test (count $argv_opts) -eq 2; or exit 90
 test "$argv_opts[1]" = --alpha; or exit 90
 test "$argv_opts[2]" = --unknown; or exit 90
 test "$argv" = value; or exit 91
 test (count (n9_candidates 'argparse --move-unknown --ignore-unk')) -eq 0; or exit 92
 test (count (n9_candidates 'argparse --ignore-unknown --move-unk')) -eq 0; or exit 93
printf '%s\n' $name
"#);
    assert_eq!(output, "n9_parser_label\n");
    let named = f.eval(&format!("{SELECT}\nfunction n9_parser_label; end; argparse --name (n9_select 'argparse --name n9_parser_lab') b/beta= -- --beta"));
    assert_ne!(named.code, 0);
    assert!(named.out.is_empty());
    assert!(String::from_utf8_lossy(&named.err).contains("n9_parser_label:"));
    // An exclusive argument precedes its specifications. Complete in the middle
    // of a real reader buffer so the producer must inspect tokens after cursor.
    let init = format!(r#"{SELECT}
status get-file completions/argparse.fish | source
bind f4 '__fish_argparse_exclusive_args > argparse-unused; printf ready > argparse-unused-ready'
"#);
    let mut s = start(f, &init);
    let line = "argparse -x a,bet a/alpha b/beta= c#count -- --alpha --beta=value; printf '%s' $status > argparse-exclusive-code";
    let cursor = "argparse -x a,bet".chars().count();
    s.send(format!("\x1b[200~{line}\x1b[201~").as_bytes());
    // Move the actual editor cursor, not a synthetic completion commandline.
    for _ in cursor..line.chars().count() { s.key(true, 1, 0x25, 0, 0, 0); }
    s.key(true, 1, 0x73, 0, 0, 0);
    s.wait_file(&f.root.join("argparse-unused-ready"));
    let unused = fs::read_to_string(f.root.join("argparse-unused")).unwrap();
    assert_eq!(values(&unused), BTreeSet::from(["alpha", "b", "beta", "c", "count"]));
    s.send(b"\t");
    let completed = line.replacen("a,bet ", "a,beta ", 1);
    capture(&mut s, f, 1, &completed);
    s.send(b"\r");
    assert_ne!(s.wait_file(&f.root.join("argparse-exclusive-code")), b"0");
    for (offset, (partial, completed)) in [
        ("-xb,cou", "-xb,count"),
        ("--exclusive=b,cou", "--exclusive=b,count"),
        ("--exclusive b,cou", "--exclusive b,count"),
    ].into_iter().enumerate() {
        let result = format!("argparse-numeric-exclusive-code-{offset}");
        let line = format!("argparse {partial} a/alpha b/beta= c#count -- --beta=value -7; printf '%s' $status > {result}");
        let cursor = format!("argparse {partial}").chars().count();
        s.send(format!("\x1b[200~{line}\x1b[201~").as_bytes());
        for _ in cursor..line.chars().count() { s.key(true, 1, 0x25, 0, 0, 0); }
        s.send(b"\t");
        capture(&mut s, f, offset + 2, &line.replacen(partial, completed, 1));
        s.send(b"\r");
        assert_ne!(s.wait_file(&f.root.join(result)), b"0");
    }
    finish(s);
    for code in [
        "argparse --name n9_parser_label --exclusive alpha,beta a/alpha b/beta= -- --alpha --beta=value",
        "argparse --min-args 2 --max-args 2 a/alpha -- one",
        "argparse b/beta= -- --beta",
    ] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty() && !result.err.is_empty(), "{code}");
    }
}

fn bindings(f: &Fixture) {
    let init = format!(r#"{SELECT}
bind --mode n9_mode f3 forward-char
set -l mode (n9_select 'bind --mode n9_mo'); or exit 81
set -l next (n9_select 'bind --sets-mode defau'); or exit 82
set -l key (n9_candidates "bind --mode $mode f3" | string match -- f3)
test (count $key) -eq 1; or exit 83
set -l backward (n9_candidates "bind --mode $mode $key backward-ch" | string match -- backward-char)
test (count $backward) -eq 1; or exit 84
set -l forward (n9_candidates "bind --mode $mode $key forward-ch" | string match -- forward-char)
test (count $forward) -eq 1; or exit 85
contains -- backward-char (n9_candidates "bind --mode $mode "); and exit 86
bind --mode default --sets-mode $mode $key $backward; or exit 87
bind --mode $mode --sets-mode $next $key $forward; or exit 88
set -l modified (n9_candidates "bind --mode $mode ctrl-alt-f3" | string match -- ctrl-alt-f3)
test (count $modified) -eq 1; or exit 89
bind --mode default $modified $backward; or exit 90
set -l sequence (n9_candidates "bind --mode $mode ctrl-f3,alt-f5" | string match -- ctrl-f3,alt-f5)
test (count $sequence) -eq 1; or exit 91
set -l word (n9_candidates "bind --mode $mode $key backward-wo" | string match -- backward-word)
test (count $word) -eq 1; or exit 92
bind --mode default $sequence $word; or exit 92
bind --mode default f10 'commandline --replace "alpha beta"; commandline -C 10'
bind --mode $mode f12 n6_capture
"#);
    let mut s = start(f, &init);
    s.send("雪ab".as_bytes());
    s.key(true, 1, 0x72, 0, 0, 0);
    assert_eq!(capture(&mut s, f, 1, "雪ab"), 2);
    assert_eq!(fs::read(f.root.join("mode-1")).unwrap(), b"n9_mode");
    s.key(true, 1, 0x72, 0, 0, 0);
    assert_eq!(capture(&mut s, f, 2, "雪ab"), 3);
    assert_eq!(fs::read(f.root.join("mode-2")).unwrap(), b"default");
    s.key(true, 1, 0x72, 0, 0, 0x000a);
    assert_eq!(capture(&mut s, f, 3, "雪ab"), 2);
    assert_eq!(fs::read(f.root.join("mode-3")).unwrap(), b"default");
    // Use unambiguous Latin words for the sequence action; CJK-to-Latin
    // boundaries exercise a different word-motion contract.
    s.key(true, 1, 0x79, 0, 0, 0);
    s.key_records(&[(true, 1, 0x72, 0, 0, 0x0008), (true, 1, 0x74, 0, 0, 0x0002)]);
    assert_eq!(capture(&mut s, f, 4, "alpha beta"), 6);
    s.send(b"\x03bind --erase --all --mode n9_mode; test (count (n9_candidates 'bind --mode n9_mo')) -eq 0; or exit 89; printf ready > binding-mode-erased\r");
    s.wait_file(&f.root.join("binding-mode-erased"));
    finish(s);
    let invalid = f.eval("bind --mode n9_mode --sets-mode n9_next ctrl-not-a-real-key self-insert");
    assert_ne!(invalid.code, 0);
    assert!(!invalid.err.is_empty());
}

fn dispatch(f: &Fixture) {
    let output = success(f, r#"
function n9_action --argument-names value; printf '<%s>\n' "$value"; end
function _n9_hidden; printf hidden; end
set -l original (n9_select 'functions --copy n9_act'); or exit 81
functions --copy $original n9_clone; or exit 82
n9_clone 'copy 雪'
set -l hidden (n9_select 'functions --erase _n9_hid'); or exit 83
functions --erase $hidden; or exit 84
functions --query _n9_hidden; and exit 85
 test (count (n9_candidates 'functions --erase _n9_hid')) -eq 0; or exit 86
set -l cloned (n9_select 'functions --erase n9_clo'); or exit 86
functions --erase $cloned; or exit 86
functions --query n9_clone; and exit 86
set -l target (n9_select 'complete --command n9_act'); or exit 87
complete --command $target --no-files --arguments n9_payload
set -l payload (n9_select 'n9_action n9_pay'); or exit 88
n9_action $payload
set -l wrapped (n9_select 'complete --wraps n9_act'); or exit 89
function n9_wrapper; n9_action $argv; end
complete --command n9_wrapper --wraps $wrapped
n9_wrapper (n9_select 'n9_wrapper n9_pay'); or exit 90
set -l erase_target (n9_select 'complete --command=n9_act'); or exit 91
complete --erase $erase_target
 test (count (n9_candidates 'n9_action n9_pay')) -eq 0; or exit 91
function contains; return 99; end
set -l name (n9_select 'builtin conta'); or exit 92
set -l index (n9_select 'builtin contains --ind'); or exit 93
builtin $name $index -- -needle hay -needle -needle; or exit 94
contains ignored; test $status -eq 99; or exit 95
builtin $name -- absent hay; test $status -eq 1; or exit 96
set -g PATH (pwd)
set -l external (n9_select 'command fish.ex'); or exit 97
set -l script_option (n9_select "command $external --comman"); or exit 98
set -l no_config (n9_select "command -- $external --no-con"); or exit 98
test (count (n9_candidates "command --search $external --comman")) -eq 0; or exit 98
command -- $external $no_config $script_option "printf 'native child 雪\n'"; or exit 98
set -l search (n9_select 'command --sear'); or exit 99
set -l query_target (n9_select "command --search fish.ex"); or exit 100
set -l resolved (builtin command $search $query_target); or exit 100
 test (path resolve -- $resolved) = (path resolve -- ./fish.exe); or exit 101
command --query n9_no_such_external; test $status -ne 0; or exit 102
"#);
    assert_eq!(output, "<copy 雪>\n<n9_payload>\n<n9_payload>\n2\nnative child 雪\n");
    for code in ["functions --copy n9_missing n9_copy", "complete --command n9_bad --short-option ''", "builtin n9_not_a_builtin"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty() && !result.err.is_empty(), "{code}");
    }
}

fn variables(f: &Fixture) {
    fs::write(f.root.join("n9_locale_value"), b"not a locale candidate").unwrap();
    let output = success(f, r#"
set -g n9_scope_global 'global 雪'
set -U n9_scope_universal 'universal 🐟'
begin
    set -l n9_scope_local 'local value'
    test (count (n9_candidates 'set --erase --universal n9_scope_')) -eq 1; or exit 81
    test (n9_select 'set --erase --universal n9_scope_') = n9_scope_universal; or exit 82
    test (n9_select 'set --erase --global n9_scope_') = n9_scope_global; or exit 83
    test (n9_select 'set --erase --local n9_scope_') = n9_scope_local; or exit 84
    set -l local_name (n9_select 'set --erase --local n9_scope_'); or exit 85
    set --erase --local $local_name; or exit 86
    set --query n9_scope_local; and exit 87
end
set -l universal_name (n9_select 'set --erase --universal n9_scope_'); or exit 88
set --erase --universal $universal_name; or exit 89
 test (count (n9_candidates 'set --erase --universal n9_scope_')) -eq 0; or exit 90
 test "$n9_scope_global" = 'global 雪'; or exit 91
set -l name (n9_select 'set n9_scope_glob'); or exit 92
set --append $name 'second 🐟'; or exit 93
set -l index (n9_select 'contains --ind'); or exit 94
contains $index -- 'second 🐟' $n9_scope_global; or exit 95
set --erase --global (n9_select 'set --erase --global n9_scope_'); or exit 96
test (count (n9_candidates 'set --erase n9_scope_')) -eq 0; or exit 97
set -g __n9_private 'private data'
contains -- __n9_private (n9_candidates 'set __n9_priv'); or exit 98
contains -- __n9_private (n9_candidates 'set n9_priv'); and exit 99
set -l foreground (n9_select 'set fish_color_command re'); or exit 100
set fish_color_command $foreground
 test "$(set_color $fish_color_command)" != "$(set_color normal)"; or exit 101
set -l background (n9_select 'set fish_color_search_match --background=blu'); or exit 102
set fish_color_search_match $background
 test "$(set_color $fish_color_search_match)" != "$(set_color $fish_color_command)"; or exit 103
set -l underline (n9_select 'set fish_color_selection --underline-color=gree'); or exit 103
set fish_color_selection $underline
 test "$(set_color $fish_color_selection)" != "$(set_color normal)"; or exit 103
 test (count (n9_candidates 'set n9_not_color --background=blu')) -eq 0; or exit 104
set -e fish_color_option
contains -- fish_color_option (n9_candidates 'set fish_color_opt'); or exit 105
set -l special (n9_select 'set fish_color_opt'); or exit 105
set $special $foreground
 test "$(set_color $fish_color_option)" = "$(set_color $foreground)"; or exit 105
set -e fish_color_option
contains -- fish_color_option (n9_candidates 'set --erase fish_color_opt'); and exit 106
set -g LC_CTYPE C
 test (count (n9_candidates 'set LC_CTYPE n9_locale_')) -eq 0; or exit 107
contains -- n9_locale_value (n9_candidates 'set n9_not_locale n9_locale_'); or exit 107
contains -- missing 'snow 雪' present; test $status -eq 1; or exit 107
"#);
    assert_eq!(output, "2\n");
    let output = success(f, r#"
for variable in LANG LC_ALL LC_NUMERIC LC_TIME LC_CTYPE
    set -l locales (n9_candidates "set $variable ")
    for alias in C POSIX C.UTF-8 C.utf8
        contains -- $alias $locales; or exit 108
    end
    contains -- de-DE $locales; or exit 109
    string match -q '*_*' -- $locales; and exit 110
end
set -e LC_ALL
set -gx LC_NUMERIC C
set -l selected (n9_select 'set LC_NUMERIC de-D'); or exit 111
printf '%s\n' $selected
printf '%.2f\n' 1234.5
set -gx LC_NUMERIC $selected; or exit 112
printf '%.2f\n' 1234.5
printf "%'0.2f\n" 1234.5
set -l lang (n9_select 'set LANG de-D'); or exit 113
set -e LC_NUMERIC
set -gx LANG $lang; or exit 114
printf "%'0.2f\n" 1234.5
set -gx LC_ALL (n9_select 'set LC_ALL C.UTF-'); or exit 115
printf '%.2f\n' 1234.5
set -gx LC_ALL (n9_select 'set LC_ALL POSI'); or exit 116
printf '%.2f\n' 1234.5
set -gx LC_ALL (n9_select 'set LC_ALL C.utf8'); or exit 117
printf '%.2f\n' 1234.5
"#);
    let (selected, formatted) = output.split_once('\n').unwrap();
    assert_eq!(selected, "de-DE");
    // Query NLS independently of fish's locale service and printf consumer.
    // The completion-selected value, not a copied provider record, configures
    // the real exported LC_NUMERIC/LANG formatting path above.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocaleInfoEx(name: *const u16, kind: u32, data: *mut u16, capacity: i32) -> i32;
    }
    let name: Vec<u16> = selected.encode_utf16().chain(Some(0)).collect();
    let punctuation = |kind: u32| {
        let mut value = [0u16; 128];
        let count = unsafe {
            GetLocaleInfoEx(name.as_ptr(), kind | 0x8000_0000, value.as_mut_ptr(), value.len() as i32)
        };
        assert!(count > 0, "{}", std::io::Error::last_os_error());
        String::from_utf16(&value[..count as usize - 1]).unwrap()
    };
    let decimal = punctuation(0x0e);
    let thousands = punctuation(0x0f);
    assert_ne!(decimal, ".", "selected locale must exercise non-C numeric punctuation");
    assert_eq!(punctuation(0x10), "3;0");
    assert_eq!(formatted, format!(
        "1234.50\n1234{decimal}50\n1{thousands}234{decimal}50\n1{thousands}234{decimal}50\n1234.50\n1234.50\n1234.50\n"
    ));
    exact(f.eval("set --query n9_scope_universal"), 1, b"", b"");
    for code in ["set --global --universal n9_invalid value", "contains --index", "set 'not-a-variable' value"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty() && !result.err.is_empty(), "{code}");
    }
}

fn status(f: &Fixture) {
    let metadata = success(f, "status features");
    let selected = success(f, "n9_candidates 'status test-feature '");
    let expected: BTreeSet<_> = metadata.lines().map(|line| line.split_whitespace().next().unwrap()).collect();
    assert_eq!(values(&selected), expected);
    for line in metadata.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let code = format!("set -l selected (n9_candidates 'status test-feature ' | string match -- {}); test (count $selected) -eq 1; or exit 81; status test-feature $selected", quote(fields[0]));
        let result = f.eval(&format!("{SELECT}\n{code}"));
        exact(result, if fields[1] == "on" { 0 } else { 1 }, b"", b"");
    }
    exact(f.eval("status test-feature n9_unknown_feature"), 2, b"", b"");
    let output = success(f, r#"
set -l asset (n9_select 'status get-file completions/contains.fi'); or exit 81
status get-file $asset | source; or exit 82
set -l index (n9_select 'contains --ind'); or exit 83
contains $index -- payload other payload; or exit 84
set -l prefix (n9_select 'status list-files completions/contains.fi'); or exit 85
 test (status list-files $prefix) = $asset; or exit 86
set -l none (n9_select 'status job-control no'); or exit 87
status job-control $none; or exit 88
status is-no-job-control; or exit 89
 test (count (n9_candidates 'status job-control fu')) -eq 0; or exit 90
 test (count (n9_candidates 'status job-control inter')) -eq 0; or exit 91
status is-full-job-control; and exit 92
status is-interactive-job-control; and exit 93
status is-no-job-control; or exit 94
 test (count (n9_candidates 'status language n9_missing')) -eq 0; or exit 96
"#);
    assert_eq!(output, "2\n");
    if cfg!(feature = "localize-messages") {
        let output = success(f, r#"
set -l german (n9_candidates 'status language set de' | string match -- de)
test (count $german) -eq 1; or exit 81
status language set $german; or exit 82
_ 'Print the working directory'; printf '\n'
status language unset; or exit 83
_ 'Print the working directory'; printf '\n'
"#);
        assert_eq!(output, "Das Arbeitsverzeichnis ausgeben\nPrint the working directory\n");
    }
    for code in ["status get-file n9_absent_asset", "status list-files n9_absent_prefix"] {
        exact(f.eval(code), 1, b"", b"");
    }
    for mode in ["full", "interactive", "n9_invalid"] {
        let invalid = f.eval(&format!("status job-control {mode}"));
        assert_ne!(invalid.code, 0);
        assert!(invalid.out.is_empty() && !invalid.err.is_empty());
    }
}

fn jobs(f: &Fixture) {
    use std::{ffi::c_void, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle}, sync::atomic::{AtomicU32, Ordering}};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
        fn SetEvent(event: *mut c_void) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    }
    struct Release(OwnedHandle);
    impl Drop for Release {
        fn drop(&mut self) { assert_ne!(unsafe { SetEvent(self.0.as_raw_handle()) }, 0); }
    }
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let event = |label: &str| {
        let name = format!("Local\\fish-N9-builtin-{}-{serial}-{label}", std::process::id());
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(!handle.is_null(), "{}", std::io::Error::last_os_error());
        (name, unsafe { OwnedHandle::from_raw_handle(handle) })
    };
    let (ready_one_name, ready_one) = event("one-ready");
    let (release_one_name, release_one) = event("one-release");
    let release_one = Release(release_one);
    let (ready_two_name, ready_two) = event("two-ready");
    let (release_two_name, release_two) = event("two-release");
    let release_two = Release(release_two);
    let init = format!(r#"{SELECT}
{} n8-services-event {} {} &
set -g n9_job_one $last_pid
{} n8-services-event {} {} &
set -g n9_job_two $last_pid
printf '%s\n' $n9_job_one $n9_job_two > builtin-job-pids
"#,
        f.probe(), quote(&ready_one_name), quote(&release_one_name),
        f.probe(), quote(&ready_two_name), quote(&release_two_name));
    let mut s = start(f, &init);
    for ready in [&ready_one, &ready_two] {
        assert_eq!(unsafe { WaitForSingleObject(ready.as_raw_handle(), 30_000) }, 0, "owned child acknowledgement");
    }
    let pids = fs::read_to_string(f.root.join("builtin-job-pids")).unwrap();
    let pids: Vec<u32> = pids.lines().map(|line| line.parse().unwrap()).collect();
    assert_eq!(pids.len(), 2);
    let processes: Vec<_> = pids.iter().map(|pid| {
        let handle = unsafe { OpenProcess(0x0010_0000, 0, *pid) };
        assert!(!handle.is_null(), "retain acknowledged native child");
        unsafe { OwnedHandle::from_raw_handle(handle) }
    }).collect();
    s.send(br#"set -l pid_option (n9_select 'jobs --pi'); or exit 81; jobs $pid_option > builtin-job-listed; set -l query (n9_candidates 'jobs --que' | string match -- --query); test (count $query) -eq 1; or exit 82; jobs $query $n9_job_one; or exit 83; complete -C 'wait ' > builtin-wait-before; complete -C 'disown ' > builtin-disown-before; set -l any (n9_select 'wait --an'); or exit 84; set -l first (n9_select "wait $n9_job_one"); or exit 85; set -l second (n9_select "wait $n9_job_two"); or exit 86; printf ready > builtin-wait-entered; wait $any $first $second; or exit 87; printf ready > builtin-wait-returned
"#);
    s.send(b"\r");
    s.wait_file(&f.root.join("builtin-wait-entered"));
    let listed = fs::read_to_string(f.root.join("builtin-job-listed")).unwrap();
    assert_eq!(listed.lines().map(str::to_owned).collect::<BTreeSet<_>>(), pids.iter().map(u32::to_string).collect());
    for name in ["builtin-wait-before", "builtin-disown-before"] {
        let candidates = fs::read_to_string(f.root.join(name)).unwrap();
        for pid in &pids { assert!(values(&candidates).contains(pid.to_string().as_str()), "{name}: {candidates}"); }
    }
    drop(release_one);
    assert_eq!(unsafe { WaitForSingleObject(processes[0].as_raw_handle(), 30_000) }, 0);
    s.wait_file(&f.root.join("builtin-wait-returned"));
    assert_eq!(unsafe { WaitForSingleObject(processes[1].as_raw_handle(), 0) }, 258, "--any must not wait for the unreleased second child");
    s.send(br#"set -l selected (n9_select "disown $n9_job_two"); or exit 88; disown $selected; or exit 89; jobs --query $n9_job_two; and exit 90; test (count (n9_candidates "wait $n9_job_two")) -eq 0; or exit 91; test (count (n9_candidates "disown $n9_job_two")) -eq 0; or exit 92; printf ready > builtin-disowned
"#);
    s.send(b"\r");
    s.wait_file(&f.root.join("builtin-disowned"));
    drop(release_two);
    assert_eq!(unsafe { WaitForSingleObject(processes[1].as_raw_handle(), 30_000) }, 0);
    finish(s);
    for code in ["wait %999", "disown %999", "jobs --query %999"] {
        let invalid = f.eval(code);
        assert_ne!(invalid.code, 0, "{code}");
        assert!(invalid.out.is_empty(), "{code}");
    }
    // Keep native facade rejections: these tables must not create POSIX process
    // control semantics, nor launch the command supplied to exec.
    let marker = f.root.join("builtin-exec-must-not-run");
    for code in [format!("exec {} report > {}", f.probe(), quote(marker.to_str().unwrap())), "fg".to_owned(), "bg".to_owned(), "suspend".to_owned()] {
        let result = f.eval(&code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty() && !result.err.is_empty(), "{code}");
    }
    assert!(!marker.exists() || fs::read(marker).unwrap().is_empty());
}

fn wrappers(f: &Fixture) {
    fs::write(f.root.join("n9_wrapper_input.fish"), "set -g n9_wrapper_value $argv\nfunction n9_sourced_value; printf '<%s>\\n' $n9_wrapper_value; end\n").unwrap();
    fs::write(f.root.join("n9_wrapper_payload.txt"), b"owned payload").unwrap();
    let output = success(f, r#"
status get-file 'completions/!.fish' | source
status get-file 'completions/..fish' | source
status get-file 'completions/[.fish' | source
set -l index (n9_select '! contains --ind'); or exit 81
! contains $index -- absent payload; or exit 82
! contains $index -- payload payload >/dev/null; test $status -eq 1; or exit 83
set -l script (n9_select '. n9_wrapper_input.fi'); or exit 84
. $script 'source 雪' second; or exit 85
n9_sourced_value
set -l size (n9_candidates '[ -' | string match -- -s)
test (count $size) -eq 1; or exit 86
set -l payload (n9_select '[ -s n9_wrapper_payload.tx'); or exit 87
[ $size $payload ]; or exit 88
printf '' > $payload
[ $size $payload ]; test $status -eq 1; or exit 89
! [ $size $payload ]; or exit 90
"#);
    assert_eq!(output, "<source 雪>\n<second>\n");
    assert!(fs::read(f.root.join("n9_wrapper_payload.txt")).unwrap().is_empty());
    for code in [". n9_absent_source.fish", "[ -s n9_wrapper_payload.txt", "! contains --bad-option"] {
        let invalid = f.eval(code);
        // Negation deliberately inverts the builtin's diagnostic failure.
        assert_eq!(invalid.code == 0, code.starts_with('!'), "{code}");
        assert!(invalid.out.is_empty() && !invalid.err.is_empty(), "{code}");
    }
}
