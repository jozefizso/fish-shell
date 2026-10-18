use super::fixture::{Fixture, exact, quote};

pub(super) fn exercise(f: &Fixture) {
    exact(
        f.eval(
            r#"
set -g n3_total 0
set -l n3_i 0
while test $n3_i -lt 6
    set n3_i (math $n3_i + 1)
    if test $n3_i -eq 2
        continue
    end
    switch $n3_i
        case 1 3
            set n3_total (math $n3_total + $n3_i)
        case '4*'
            break
        case '*'
            set n3_total 100
    end
end
not false; and printf 'control:%s:%s\n' $n3_i $n3_total
begin
    set -l n3_scope block
    for item in a b
        printf '%s:%s\n' $n3_scope $item
    end
end
set -q n3_scope; or printf 'scope-restored\n'
false; or true; and printf 'boolean:%s\n' $status
"#,
        ),
        0,
        b"control:4:4\nblock:a\nblock:b\nscope-restored\nboolean:0\n",
        b"",
    );

    exact(
        f.eval(
            r#"
function n3_recurse --argument-names depth
    if test $depth -eq 0
        return 17
    end
    n3_recurse (math $depth - 1)
    return $status
end
n3_recurse 12
printf 'recursive:%s\n' $status
function n3_empty
end
false
n3_empty
printf 'empty:%s\n' $status
function n3_last_status
    false
    return
    printf 'unreachable\n'
end
n3_last_status
printf 'return:%s\n' $status
"#,
        ),
        0,
        b"recursive:17\nempty:0\nreturn:1\n",
        b"",
    );

    exact(
        f.eval("function n3_status; return 17; end; n3_status"),
        17,
        b"",
        b"",
    );
    exact(f.eval("eval false"), 1, b"", b"");
    exact(
        f.eval("function n3_explicit_zero; false; return 0; end; n3_explicit_zero"),
        0,
        b"",
        b"",
    );
    let source_zero = f.root.join("n3 sourced explicit zero.fish");
    std::fs::write(&source_zero, b"false\nreturn 0\nprintf unreachable\n").unwrap();
    exact(
        f.eval(&format!(
            "source {}; printf 'source-zero:%s\\n' $status",
            quote(source_zero.to_str().unwrap())
        )),
        0,
        b"source-zero:0\n",
        b"",
    );
    exact(
        f.eval(
            r#"
function n3_copy_source --argument-names value
    printf 'copied:%s\n' $value
end
function n3_copy_inside_function
    functions --copy n3_copy_source n3_nested_copy
end
n3_copy_inside_function
n3_nested_copy retained
"#,
        ),
        0,
        b"copied:retained\n",
        b"",
    );

    exact(f.eval(r#"
set -g n3_capture initial
function n3_named --description 'named description' --inherit-variable n3_capture --argument-names first second
    set -l n3_only_here private
    printf 'named:%s:%s:%s:%s\n' "$first" "$second" "$argv" "$n3_capture"
end
set n3_capture changed
n3_named one two three
n3_named alone
set -q n3_only_here; or printf 'function-scope-restored\n'
functions --copy n3_named n3_copy
functions --erase n3_named
functions --query n3_named; or printf 'erased\n'
n3_copy copied value
functions --description 'copy description' n3_copy
set n3_meta (functions --details --verbose n3_copy)
printf 'metadata:%s:%s:' $n3_meta[1] $n3_meta[4]
string unescape -- $n3_meta[5]
test $n3_meta[3] -gt 0; and printf 'copy-line-recorded\n'
function n3_named
    printf 'redefined\n'
end
n3_named
"#), 0, b"named:one:two:one two three:initial\nnamed:alone::alone:initial\nfunction-scope-restored\nerased\nnamed:copied:value:copied value:initial\nmetadata:stdin:scope-shadowing:copy description\ncopy-line-recorded\nredefined\n", b"");

    exact(f.eval(r#"
function n3_mutate --no-scope-shadowing
    set n3_visible child
end
function n3_outer
    set -l n3_visible parent
    n3_mutate
    printf 'shared:%s\n' $n3_visible
end
n3_outer
set -q n3_visible; or printf 'outer-restored\n'
function n3_invoke --argument-names value
    set -l n3_assignment original
    n3_assignment=temporary begin
        printf 'assignment:%s:%s\n' $n3_assignment $value
    end
    printf 'assignment-restored:%s\n' $n3_assignment
end
n3_invoke supplied
"#), 0, b"shared:child\nouter-restored\nassignment:temporary:supplied\nassignment-restored:original\n", b"");

    exact(f.eval(r#"
function n3_first --on-event n3_outer_event --argument-names value
    printf 'first:%s\n' $value
    emit n3_inner_event nested
    false
end
function n3_second --on-event n3_outer_event --argument-names value
    printf 'second:%s\n' $value
end
function n3_inner --on-event n3_inner_event --argument-names value
    printf 'inner:%s\n' $value
end
function n3_variable --on-variable n3_watched
    printf 'variable:%s:%s:%s\n' $argv
end
block --global
emit n3_outer_event queued
set -g n3_watched present
printf 'blocked\n'
block --erase
printf 'delivered\n'
set --erase n3_watched
functions --erase n3_first n3_second n3_inner n3_variable
emit n3_outer_event erased
printf 'handlers-erased\n'
"#), 0, b"blocked\nfirst:queued\ninner:nested\nsecond:queued\nvariable:VARIABLE:SET:n3_watched\ndelivered\nvariable:VARIABLE:ERASE:n3_watched\nhandlers-erased\n", b"");

    exact(
        f.eval(
            r#"
function n3_context_event --on-event n3_context
    printf 'handler-status:%s\n' $status
    return 9
end
function n3_context_caller
    block
    emit n3_context
    return 23
end
n3_context_caller
printf 'caller-status:%s\n' $status
"#,
        ),
        0,
        b"handler-status:23\ncaller-status:23\n",
        b"",
    );

    exact(f.eval("function n3_quit --on-event n3_quit_event; exit 17; end; emit n3_quit_event; printf continued"),
        17, b"", b"");
    exact(f.eval("function n3_quit_var --on-variable n3_quit_value; exit 19; end; set -g n3_quit_value yes; printf continued"),
        19, b"", b"");

    exact(f.eval(r#"
function n3_colored --argument-names value
    printf '%s\n' $value
end
set -g fish_color_command red
set -g fish_color_param green
set n3_plain (functions --color=never --no-details n3_colored | string collect)
set n3_ansi (functions --color=always --no-details n3_colored | string collect)
string match --regex --quiet '\e\[' -- "$n3_ansi"; and printf 'colored-definition\n'
set n3_uncolored (printf '%s' "$n3_ansi" | string replace --all --regex '\e\[[0-9;]*m' '' | string collect)
test "$n3_plain" = "$n3_uncolored"; and printf 'same-definition\n'
"#), 0, b"colored-definition\nsame-definition\n", b"");

    exact(f.eval(r#"
function n3_local --on-event n3_local_event
    printf 'local-delivered\n'
end
begin
    block --local
    emit n3_local_event
    printf 'local-blocked\n'
end
function n3_metadata --on-event n3_metadata_event --on-variable n3_metadata_var --wraps echo --argument-names value
    printf '%s\n' $value
end
functions --handlers --handlers-type generic
functions --handlers --handlers-type variable
functions --no-details n3_metadata | string match --quiet '*--wraps=echo*'; and printf 'wrapper-retained\n'
functions --names | string match --entire 'n3_metadata'
function _n3_hidden; end
functions --all --names | string match --entire '_n3_hidden'
"#), 0, b"local-blocked\nlocal-delivered\nEvent generic\nn3_local_event n3_local\nn3_metadata_event n3_metadata\nEvent variable\nn3_metadata_var n3_metadata\nwrapper-retained\nn3_metadata\n_n3_hidden\n", b"");

    exact(f.eval("function n3_option_data --description '--on-signal'; printf literal-option-data; end; n3_option_data"),
        0, b"literal-option-data", b"");

    for code in [
        "function n3_bad --argument-names bad-name; end",
        "function n3_bad --inherit-variable bad-name; end",
        "function n3_bad --on-variable bad-name; end",
        "functions --copy missing n3_copy",
        "functions --erase --query n3_bad",
        "functions --handlers-type not-a-type",
        "block --erase --local",
        "emit",
        "function n3_bad --on-signal INT; end",
        "function n3_bad --on-job-exit caller; end",
        "function n3_bad --help; end",
        "functions --help",
        "block --help",
        "emit --help",
    ] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "invalid operation succeeded: {code}");
        assert!(
            result.out.is_empty(),
            "invalid operation emitted stdout: {code}"
        );
        assert!(!result.err.is_empty(), "missing diagnostic: {code}");
    }

    // Both immediate and nontrivial recursive errors must unwind the actual
    // function stack; the following independent command can still run.
    let recursive = f.eval(
        "function n3_recursive; n3_recursive; end; n3_recursive; printf 'after:%s\\n' $status",
    );
    assert_eq!(recursive.code, 0);
    assert_eq!(recursive.out, b"after:1\n");
    assert!(!recursive.err.is_empty());

    let marker = f.root.join("n3-permanent-exclusion-marker");
    for excluded in ["exec", "fg", "bg"] {
        let result = f.eval(&format!(
            "{excluded} ({} marker {}) > {}",
            f.probe(),
            quote(marker.to_str().unwrap()),
            quote(f.root.join("n3-forbidden-redirection").to_str().unwrap())
        ));
        assert_ne!(result.code, 0);
        assert!(!result.err.is_empty());
        assert!(!marker.exists(), "excluded command evaluated substitution");
        assert!(
            !f.root.join("n3-forbidden-redirection").exists(),
            "excluded command opened redirection"
        );
    }
}
