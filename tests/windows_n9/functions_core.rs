use super::{Family, editor::{capture, finish, start}, fixture::{Fixture, exact, quote}};
use std::{collections::BTreeSet, fs, path::Path};

pub(super) const FAMILIES: &[Family] = &[
    ("core-alias-export-and-native-child-environment", aliases),
    ("core-path-order-dedup-move-dryrun-and-persistence", paths),
    ("core-fish-opt-argparse-cardinality-and-validation", options),
    ("core-commandline-predicate-completion-boundaries", predicates),
    ("core-completion-filesystem-prefix-suffix-and-cdpath", filesystem),
    ("core-prompt-mode-greeting-title-and-home-boundaries", prompts),
    ("core-status-exit-event-and-pager-scope-restoration", state),
    ("core-pager-preview-editor-literal-argv-and-failure", interactive_helpers),
    ("core-native-signal-completion-without-unix-probes", signals),
    ("core-owned-job-pid-completion-and-disown-expansion", jobs),
    ("core-xdg-native-drive-lists-icons-and-applications", xdg),
    ("core-native-ls-la-ll-filesystem-and-absence", listing),
    ("core-vared-value-export-array-and-eof-preservation", variable_editor),
    ("core-native-grep-file-stdin-regex-status-and-absence", searching),
    ("core-legacy-prompt-and-native-argv-quotation-consumers", legacy_shell),
    ("core-locale-context-and-startup-isolation", locales),
    ("core-legacy-vcs-configured-prompts-and-state", legacy_vcs),
    ("core-own-manual-command-filter-and-source-consumer", manual_commands),
    ("core-native-public-man-topics-aliases-errors-and-editor", manuals),
    ("core-native-git-repository-predicate-and-absence", git_repository),
];

fn text(path: &Path) -> String { quote(path.to_str().unwrap()) }
fn success(f: &Fixture, code: &str) -> String {
    let result = f.eval(code);
    assert_eq!(result.code, 0, "{code}\n{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{code}\n{}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn values(output: &str) -> BTreeSet<&str> {
    output.lines().map(|line| line.split('\t').next().unwrap()).collect()
}

fn git_repository(f: &Fixture) {
    use std::{ffi::OsStr, path::PathBuf};
    let binary = PathBuf::from(std::env::var_os("FISH_N9_GIT").expect("required genuine native Git fixture"));
    assert!(binary.is_absolute() && binary.is_file());
    let repository = f.root.join("repo 雪");
    fs::create_dir(&repository).unwrap();
    let invoke = |working: &Path, code: &str| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.current_dir(working);
        super::functions_tools::tool_environment(&mut command, &binary);
        super::fixture::run(&mut command, b"")
    };
    exact(invoke(&repository, "git init --quiet -b n9-predicate"), 0, b"", b"");
    let predicate = "if __fish_is_git_repository; printf inside; else; printf outside; end";
    exact(invoke(&repository, predicate), 0, b"inside", b"");
    exact(invoke(&f.root, predicate), 0, b"outside", b"");
    exact(f.eval("command -sq git; and exit 81; if __fish_is_git_repository; exit 82; end"), 0, b"", b"");
}

fn aliases(f: &Fixture) {
    let script = f.root.join("environment consumer 雪.fish");
    let record = f.root.join("environment-consumed");
    fs::write(&script, format!("test \"$N9_LITERAL\" = '雪=a=b ; &'; or exit 81\ntest \"$N9_COPY\" = 'copied 🐟'; or exit 82\ntest (count $PATH) -eq 2; or exit 83\ntest \"$PATH[1]\" = 'C:\\one 雪'; or exit 84\ntest \"$PATH[2]\" = 'D:\\two 🐟'; or exit 85\nprintf consumed > {}\n", text(&record))).unwrap();
    exact(f.eval(&format!("set -g N9_COPY 'copied 🐟'; export N9_COPY 'N9_LITERAL=雪=a=b ; &' 'PATH=C:\\one 雪;D:\\two 🐟'; alias n9-environment {} ; n9-environment {}", quote(&format!("{} -N", text(&f.fish))), text(&script))), 0, b"", b"");
    assert_eq!(fs::read(&record).unwrap(), b"consumed");

    let output = f.root.join("alias-arguments");
    exact(f.eval(&format!("alias printf 'printf \"<%s>\\n\"'; printf '雪 = &' '' '-leading' > {}", text(&output))), 0, b"", b"");
    assert_eq!(fs::read(&output).unwrap(), "<雪 = &>\n<>\n<-leading>\n".as_bytes());
    let wrapped = success(f, "complete -c n9-target -f -a n9-selected; alias n9-wrapper 'n9-target --literal'; complete -C 'n9-wrapper n9-'");
    assert_eq!(values(&wrapped), BTreeSet::from(["n9-selected"]));
    for code in ["alias n9-empty ''", "alias '' true", "alias n9-too-many true extra"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty());
    }
    let forbidden = f.root.join("unknown-command-must-not-run");
    let result = f.eval(&format!("n9-unavailable-command '雪 & literal'; and printf forbidden > {}", text(&forbidden)));
    assert_eq!(result.code, 127);
    assert!(result.out.is_empty());
    assert!(!forbidden.exists());
}

fn paths(f: &Fixture) {
    let a = f.root.join("path A 雪");
    let b = f.root.join("path B 🐟");
    let c = f.root.join("path C [literal]");
    for path in [&a, &b, &c] { fs::create_dir(path).unwrap(); }
    fs::write(f.root.join("path-not-directory"), b"file").unwrap();
    fs::copy(&f.fish, c.join("n9-path.exe")).unwrap();
    let record = f.root.join("path-child");
    let child = f.root.join("path consumer.fish");
    fs::write(&child, format!("test (status fish-path) -ef {}; or exit 81\nprintf selected > {}\n", text(&c.join("n9-path.exe")), text(&record))).unwrap();
    let code = format!(r#"
set -g PATH (path normalize {a}) (path normalize {b})
set -g n9_path_events 0
function n9_path_changed --on-variable PATH
    set -g n9_path_events (math $n9_path_events + 1)
end
fish_add_path --path {b} path-not-directory does-not-exist
 test $status -eq 1; or exit 81
 test $n9_path_events -eq 0; or exit 82
fish_add_path --path --append {c} {c}; or exit 83
 test $n9_path_events -eq 1; or exit 84
 test (count $PATH) -eq 3; or exit 85
 test "$PATH[3]" = (path normalize {c}); or exit 86
fish_add_path --path --move --prepend {b}; or exit 87
 test "$PATH[1]" = (path normalize {b}); or exit 88
 test "$PATH[2]" = (path normalize {a}); or exit 89
 test $n9_path_events -eq 2; or exit 90
fish_add_path --path --move --append --dry-run {b} >/dev/null; or exit 91
 test "$PATH[1]" = (path normalize {b}); or exit 92
 test $n9_path_events -eq 2; or exit 93
command n9-path.exe -N {child}; or exit 94
fish_add_path --universal {c}; or exit 95
contains -- (path normalize {c}) $fish_user_paths; or exit 96
"#, a=text(&a), b=text(&b), c=text(&c), child=text(&child));
    exact(f.eval(&code), 0, b"", b"");
    assert_eq!(fs::read(&record).unwrap(), b"selected");
    exact(f.eval(&format!("contains -- (path normalize {}) $fish_user_paths", text(&c))), 0, b"", b"");
    for code in ["fish_add_path --path --universal .", "fish_add_path --append --prepend .", "fish_add_path --global --universal ."] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
    }
}

fn options(f: &Fixture) {
    let record = f.root.join("parsed-option-consumer");
    let code = format!(r#"
set -g n9_specs (fish_opt -s o -l output --required-val) (fish_opt -s I -l include --multiple-vals) (fish_opt -l color --optional-val) (fish_opt -s n -l count --required-val --validate _validate_int --min 2 --max 4)
function n9_apply
    argparse $n9_specs -- $argv
    or return
    test "$_flag_output" = '雪=🐟'; or return 81
    test (count $_flag_include) -eq 2; or return 82
    test "$_flag_include[1]" = 'first space'; or return 83
    test "$_flag_include[2]" = 'second=雪'; or return 84
    test "$_flag_color" = always; or return 85
    test "$_flag_count" = 3; or return 86
    test "$argv" = literal; or return 87
    printf accepted > {record}
end
n9_apply -o '雪=🐟' -I 'first space' --include='second=雪' --color=always -n 3 -- literal
"#, record=text(&record));
    exact(f.eval(&code), 0, b"", b"");
    assert_eq!(fs::read(&record).unwrap(), b"accepted");
    for args in ["-n 1", "-n 5", "-n not-an-integer", "-n", "-o"] {
        let result = f.eval(&format!("set -l specs (fish_opt -s n -l count --required-val --validate _validate_int --min 2 --max 4) (fish_opt -s o -l output --required-val); argparse $specs -- {args}"));
        assert_ne!(result.code, 0, "{args}");
        assert!(result.out.is_empty());
        assert_eq!(fs::read(&record).unwrap(), b"accepted");
    }
    exact(f.eval("set -l specs (fish_opt -l color --optional-val) (fish_opt -s x -l exclusive --long-only); argparse $specs -- --color --exclusive -- operand; or exit 81; set -q _flag_color; or exit 82; test (count $_flag_color) -eq 0; or exit 83; set -q _flag_exclusive; or exit 84; not set -q _flag_x; or exit 85; test \"$argv\" = operand"), 0, b"", b"");
    for args in ["--short=long", "--long-only --short=x", "--short=x extra", "--short=x --validate", "--short=x --validate true", "--short=x --required-val --optional-val"] {
        let result = f.eval(&format!("fish_opt {args}"));
        assert_ne!(result.code, 0, "{args}");
        assert!(result.out.is_empty());
    }
}

fn predicates(f: &Fixture) {
    // The result is an actual completion choice, not a helper's echoed return value.
    // Every rule has accepting and rejecting commandline contexts.
    for (condition, yes, no, candidate) in [
        ("__fish_any_arg_in '雪 item'", "n9-core '雪 item' ", "n9-core 雪 ", "n9-choice"),
        ("__fish_commandline_is_singlequoted", "n9-core 'n9-", "n9-core \"n9-", "n9-choice"),
        ("__fish_prev_arg_in --output", "n9-core --output ", "n9-core --other ", "n9-choice"),
        ("__fish_contains_opt -s a archive", "n9-core -za ", "n9-core -z ", "n9-choice"),
        ("__fish_contains_opt archive", "n9-core --archive ", "n9-core --archived ", "n9-choice"),
        ("__fish_not_contain_opt -s a archive", "n9-core -z ", "n9-core --archive ", "n9-choice"),
        ("__fish_not_contain_opt -s a archive", "n9-core --other ", "n9-core -za ", "n9-choice"),
        ("__fish_seen_argument -s a", "n9-core -za ", "n9-core -z ", "n9-choice"),
        ("__fish_seen_argument -l output", "n9-core --output=雪 ", "n9-core --outputs=雪 ", "n9-choice"),
        ("__fish_seen_argument -o old", "n9-core -old ", "n9-core --old ", "n9-choice"),
        ("__fish_seen_argument -w native", "n9-core /native ", "n9-core /native-other ", "n9-choice"),
        ("__fish_seen_argument '雪 item'", "n9-core '雪 item' ", "n9-core other ", "n9-choice"),
        ("__fish_seen_subcommand_from '雪.*'", "n9-core '雪.*' ", "n9-core 雪x ", "n9-choice"),
        ("__fish_use_subcommand", "n9-core --verbose ", "n9-core subcommand ", "n9-choice"),
        ("__fish_is_first_arg", "n9-core ", "n9-core --verbose ", "n9-choice"),
        ("__fish_is_first_token", "n9-core --verbose ", "n9-core subcommand ", "n9-choice"),
        ("__fish_is_nth_token 2", "n9-core --verbose subcommand ", "n9-core --verbose ", "n9-choice"),
        ("__fish_is_token_n 2", "n9-core subcommand ", "n9-core ", "n9-choice"),
        ("__fish_no_arguments", "n9-core --verbose ", "n9-core operand ", "n9-choice"),
        ("__fish_no_arguments", "n9-core ", "n9-core --verbose operand ", "n9-choice"),
        ("__fish_no_arguments", "n9-core --verbose ", "n9-core '' ", "n9-choice"),
        ("__fish_is_switch", "n9-core -", "n9-core ", "-n9-choice"),
        ("__fish_should_complete_switches", "n9-core -", "n9-core ", "-n9-choice"),
        ("test (__fish_first_token) = '雪 item'", "n9-core -v '雪 item' ", "n9-core -v other ", "n9-choice"),
        ("test (__fish_nth_token 2) = '雪 item'", "n9-core -v first '雪 item' ", "n9-core -v first other ", "n9-choice"),
        ("test (__fish_number_of_cmd_args_wo_opts) -eq 2", "n9-core -v '雪 item' ", "n9-core -v ", "n9-choice"),
        ("contains_seq '雪 item' next -- (__fish_print_cmd_args_without_options)", "n9-core -v '雪 item' next ", "n9-core '雪 item' other next ", "n9-choice"),
    ] {
        for (line, expected) in [(yes, BTreeSet::from([candidate])), (no, BTreeSet::new())] {
            let result = success(f, &format!("complete -c n9-core -f; complete -c n9-core -n {} -a {}; complete -C {}", quote(condition), quote(candidate), quote(line)));
            assert_eq!(values(&result), expected, "{condition}: {line}: {result}");
        }
    }
    // Option-value helpers omit --items=: the registered option restores it.
    // Positional completions have no such option prefix to restore.
    for (helper, registration, line, expected) in [
        ("(__fish_append , two three)", "-l items -x", "n9-core --items=one,tw", "--items=one,two"),
        ("(__fish_append , two three)", "-f", "n9-core tw", "two"),
        ("(__fish_stripprefix='^--items=' __fish_complete_list , 'printf \"%s\\n\" two three')", "-l items -x", "n9-core --items=one,tw", "--items=one,two"),
        ("(__fish_complete_list , 'printf \"%s\\n\" two three')", "-f", "n9-core tw", "two"),
    ] {
        let output = success(f, &format!("complete -c n9-core {registration} -a {}; complete -C {}", quote(helper), quote(line)));
        assert_eq!(values(&output), BTreeSet::from([expected]), "{helper}: {line}: {output}");
    }
    let output = success(f, "complete -c n9-inner -f -a n9-choice; complete -c n9-core -f -a '(__fish_complete_subcommand -c)'; complete -C 'n9-core -c ignored n9-inner n9-'");
    assert_eq!(values(&output), BTreeSet::from(["n9-choice"]));
    let output = success(f, "function n9-executable; end; complete -c n9-core -f -a '(__fish_complete_command)'; complete -C 'n9-core n9-exec'");
    assert!(values(&output).contains("n9-executable"));
    exact(f.eval("complete -c n9-core -f -a '(__fish_complete_command)'; complete -C 'n9-core --invalid'"), 0, b"", b"");
}

fn filesystem(f: &Fixture) {
    let directory = f.root.join("completion files 雪");
    fs::create_dir(&directory).unwrap();
    fs::create_dir(directory.join("folder 🐟")).unwrap();
    fs::write(directory.join("data 雪.fish"), b"native-data").unwrap();
    fs::write(directory.join("data 雪.txt"), b"other-data").unwrap();
    let output = success(f, &format!("cd {}; complete -c n9-core -f -a '(__fish_complete_directories)'; complete -C 'n9-core fol'", text(&directory)));
    assert_eq!(output.lines().count(), 1, "{output}");
    assert!(output.contains("folder") && output.contains("🐟") && output.contains('/'));
    let output = success(f, &format!("cd {}; complete -c n9-core -l directory -r -f -a '(__fish_complete_directories)'; complete -C 'n9-core --directory=fol'", text(&directory)));
    assert!(output.starts_with("--directory=folder"), "{output}");
    let output = success(f, &format!("cd {}; complete -c n9-core -f -k -a '(__fish_complete_suffix .fish)'; complete -C 'n9-core data'", text(&directory)));
    assert_eq!(output.lines().count(), 2, "{output}");
    assert!(output.lines().next().unwrap().contains(".fish"), "preferred suffix must sort before fallback: {output}");
    let output = success(f, &format!("complete -c n9-core -f -k -a {}; complete -C 'n9-core data'", quote(&format!("(__fish_complete_suffix --prefix={} .fish)", text(&directory)))));
    assert_eq!(output.lines().count(), 2, "{output}");
    assert!(output.lines().next().unwrap().contains(".fish"), "{output}");
    let output = success(f, &format!("cd {}; complete -c n9-core -f -a '(__fish_complete_path data)'; complete -C 'n9-core data'", text(&directory)));
    assert_eq!(output.lines().count(), 2, "{output}");

    let cdpath = f.root.join("cd[path]+雪");
    fs::create_dir_all(cdpath.join("chosen 🐟")).unwrap();
    let output = success(f, &format!("set -g CDPATH {}; complete -C 'cd cho'", text(&cdpath)));
    assert_eq!(output.lines().count(), 1, "{output}");
    assert_eq!(values(&output), BTreeSet::from(["chosen 🐟/"]), "{output}");
    let output = success(f, &format!("set -g CDPATH {}; complete -C 'cd ./cho'", text(&cdpath)));
    assert!(output.is_empty(), "explicitly relative paths must not use CDPATH: {output}");
    exact(f.eval(&format!("set -l file (realpath -s {}); or exit 81; __fish_native_fs read $file", text(&directory.join("folder 🐟/../data 雪.fish")))), 0, b"native-data", b"");
    let missing = f.eval("realpath does-not-exist/child");
    assert_ne!(missing.code, 0);
    assert!(missing.out.is_empty());
}

fn prompts(f: &Fixture) {
    exact(f.eval("set -g fish_key_bindings fish_default_key_bindings; fish_mode_prompt"), 0, b"", b"");
    for (mode, label) in [("default", "[N]"), ("insert", "[I]"), ("replace_one", "[R]"), ("replace", "[R]"), ("visual", "[V]"), ("operator", "[N]")] {
        let output = success(f, &format!("set -g fish_key_bindings fish_vi_key_bindings; set -g fish_bind_mode {mode}; fish_mode_prompt"));
        assert!(output.contains(label), "{mode}: {output:?}");
    }
    let mut private = f.command(&[std::ffi::OsStr::new("-N"), std::ffi::OsStr::new("-P"), std::ffi::OsStr::new("-c"), std::ffi::OsStr::new("set -g fish_history n9_private_greeting; set -g fish_greeting 'custom 雪'; fish_greeting; set -q fish_private_mode; or exit 81; builtin history append 'echo private greeting 雪'; builtin history save")]);
    let result = super::fixture::run(&mut private, b"");
    assert_eq!(result.code, 0);
    assert!(result.err.is_empty());
    assert!(!f.state.join("data/fish/n9_private_greeting_history").exists(), "greeting must not disable private history isolation");
    let home_child = f.state.join("home/child 雪");
    fs::create_dir(&home_child).unwrap();
    let output = success(f, &format!("prompt_pwd -d 0 {}; prompt_pwd -d 0 {}; prompt_pwd -d 0 {}", text(&home_child), text(&f.state.join("home-other/child")), quote(r"\\server\share\snow 雪\child")));
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines[0], "~/child 雪");
    assert!(!lines[1].starts_with('~'), "HOME prefix must end on a component boundary: {output}");
    assert_eq!(lines[2], "//server/share/snow 雪/child");
    let output = success(f, &format!("cd {}; fish_title 'abcdefghijklmnopqrstuv雪'", text(&home_child)));
    assert!(output.contains("abcdefghijklmnopqrst"));
    assert!(!output.contains("uv雪"));
    assert!(output.contains("child 雪"));
}

fn state(f: &Fixture) {
    let event = f.root.join("exit-event-consumer");
    exact(f.eval(&format!("trap {} EXIT; trap - EXIT; trap {} EXIT", quote(&format!("printf old > {}", text(&event))), quote(&format!("printf replaced > {}", text(&event))))), 0, b"", b"");
    assert_eq!(fs::read(&event).unwrap(), b"replaced");
    fs::remove_file(&event).unwrap();
    exact(f.eval(&format!("trap {} EXIT; trap - EXIT", quote(&format!("printf forbidden > {}", text(&event))))), 0, b"", b"");
    assert!(!event.exists());
    let mut controls = start(f, "set -g n9_control_seen 0; set -g n9_control_barrier 0; function n9_control_observer --on-signal CTRL_C; set -g n9_control_seen (math $n9_control_seen + 1); printf '%s' $n9_control_seen > control-seen-$n9_control_seen; end; bind f4 'set -g n9_control_barrier (math $n9_control_barrier + 1); printf ready > control-barrier-$n9_control_barrier'; trap 'printf old > trap-native-action' CTRL_C; or exit 81; trap 'printf break > trap-native-break' CTRL_BREAK; or exit 82");
    controls.control(0);
    assert_eq!(controls.wait_file(&f.root.join("control-seen-1")), b"1");
    controls.key(true, 1, 0x73, 0, 0, 0);
    controls.wait_file(&f.root.join("control-barrier-1"));
    assert_eq!(fs::read(f.root.join("trap-native-action")).unwrap(), b"old");
    controls.send(b"trap 'printf replaced > trap-native-action' CTRL_C; printf ready > trap-native-replaced\r");
    controls.wait_file(&f.root.join("trap-native-replaced"));
    controls.control(0);
    assert_eq!(controls.wait_file(&f.root.join("control-seen-2")), b"2");
    controls.key(true, 1, 0x73, 0, 0, 0);
    controls.wait_file(&f.root.join("control-barrier-2"));
    assert_eq!(fs::read(f.root.join("trap-native-action")).unwrap(), b"replaced");
    controls.send(b"trap - CTRL_C; printf ready > trap-native-cleared\r");
    controls.wait_file(&f.root.join("trap-native-cleared"));
    controls.control(0);
    assert_eq!(controls.wait_file(&f.root.join("control-seen-3")), b"3");
    controls.key(true, 1, 0x73, 0, 0, 0);
    controls.wait_file(&f.root.join("control-barrier-3"));
    assert_eq!(fs::read(f.root.join("trap-native-action")).unwrap(), b"replaced");
    controls.control(1);
    assert_eq!(controls.wait_file(&f.root.join("trap-native-break")), b"break");
    finish(controls);
    let invalid = f.eval("trap true N9_INVALID_CONTROL");
    assert_ne!(invalid.code, 0);
    assert!(invalid.out.is_empty());
    let effect = f.root.join("status-consumer");
    exact(f.eval(&format!("function n9_prior; return 7; end; function n9_effect; printf consumed > {}; end; n9_prior; __fish_with_status n9_effect; test $status -eq 7; or exit 81; function n9_denied; return 9; end; n9_prior; __fish_with_status n9_denied; test $status -eq 9", text(&effect))), 0, b"", b"");
    assert_eq!(fs::read(&effect).unwrap(), b"consumed");
    let pager_script = f.root.join("pager scope consumer.fish");
    let pager_record = f.root.join("pager-scope");
    fs::write(&pager_script, format!("test \"$MANPAGER\" = cat; or exit 81\ntest \"$WHATISPAGER\" = cat; or exit 82\nprintf scoped > {}\n", text(&pager_record))).unwrap();
    exact(f.eval(&format!("set -gx MANPAGER 'original 雪'; set -gx WHATISPAGER 'original 🐟'; __fish_without_manpager command {} -N {}; or exit 81; test \"$MANPAGER\" = 'original 雪'; or exit 82; test \"$WHATISPAGER\" = 'original 🐟'; or exit 83; not isatty stdin; and not isatty stdout; and not isatty stderr", text(&f.fish), text(&pager_script))), 0, b"", b"");
    assert_eq!(fs::read(&pager_record).unwrap(), b"scoped");
}

fn signals(f: &Fixture) {
    // There is no Unix kill executable in the fixture. A native unsupported
    // command must neither invoke uname nor advertise undeliverable signals.
    exact(f.eval("__fish_make_completion_signals; or exit 81; test (count $__kill_signals) -eq 0; or exit 82; complete -C 'kill -'"), 0, b"", b"");
    let output = success(f, "function n9_exit; return 143; end; true | n9_exit; __fish_print_pipestatus '[' ']' '|' red blue $pipestatus");
    assert!(output.contains("143") && output.contains("0") && !output.contains("TERM"), "native pipeline status must remain numeric: {output:?}");
}

fn interactive_helpers(f: &Fixture) {
    let pager = f.root.join("native pager script 雪.fish");
    let paged = f.root.join("paged-content");
    fs::write(&pager, format!("read -l value; or exit 81\nprintf '%s' \"$value\" > {}\n", text(&paged))).unwrap();
    let pager_command = format!("{} -N {}", text(&f.fish), text(&pager));
    let mut s = start(f, &format!("set -g PAGER {}; bind f2 __fish_paginate; bind f3 'set -e PAGER; __fish_paginate; printf %s $status > pager-unavailable'", quote(&pager_command)));
    s.send("printf 'paged 雪\\n'".as_bytes());
    s.key(true, 1, 0x71, 0, 0, 0);
    s.send(b"\r");
    assert_eq!(s.wait_file(&paged), "paged 雪".as_bytes());
    s.send(b"printf untouched");
    s.key(true, 1, 0x72, 0, 0, 0);
    assert_eq!(s.wait_file(&f.root.join("pager-unavailable")), b"1");
    capture(&mut s, f, 1, "printf untouched");
    finish(s);

    let preview = f.root.join("preview file 雪.txt");
    fs::write(&preview, "native preview 雪 🐟\nsecond line\n").unwrap();
    let command = format!("n9-missing {}", text(&preview));
    let position_preview = format!("commandline --replace {}; commandline --cursor {};", quote(&command), command.chars().count());
    let mut s = start(f, &format!("set -g n6_capture 1; bind f2 {}", quote(&format!("{position_preview} __fish_preview_current_file; printf %s $status > preview-status"))));
    s.key(true, 1, 0x71, 0, 0, 0);
    assert_eq!(s.wait_file(&f.root.join("preview-status")), b"0");
    capture(&mut s, f, 2, &command);
    let transcript = finish(s);
    assert!(transcript.bytes.windows("native preview 雪 🐟".len()).any(|bytes| bytes == "native preview 雪 🐟".as_bytes()));

    let source = f.root.join("editable native command 雪.cmd");
    fs::write(&source, "rem original\r\n").unwrap();
    let editor = f.root.join("source editor 🐟.fish");
    fs::write(&editor, format!("test (count $argv) -eq 1; or exit 81\ntest (path basename \"$argv[1]\") = {}; or exit 82\nprintf '# edited 雪\\n' > \"$argv[1]\"\n", quote(source.file_name().unwrap().to_str().unwrap()))).unwrap();
    let editor_command = format!("{} -N {}", text(&f.fish), text(&editor));
    let source_line = text(&source);
    let position_source = format!("commandline --replace {}; commandline --cursor 0;", quote(&source_line));
    let mut s = start(f, &format!("set -g EDITOR {}; bind f2 {}; bind f3 'set -g EDITOR unavailable-n9-editor; commandline --cursor 0; __fish_preview_current_file; printf %s $status > source-editor-unavailable'", quote(&editor_command), quote(&format!("{position_source} __fish_preview_current_file; printf %s $status > source-editor-status"))));
    s.key(true, 1, 0x71, 0, 0, 0);
    assert_eq!(s.wait_file(&f.root.join("source-editor-status")), b"0");
    assert_eq!(fs::read(&source).unwrap(), "# edited 雪\n".as_bytes());
    s.key(true, 1, 0x72, 0, 0, 0);
    assert_eq!(s.wait_file(&f.root.join("source-editor-unavailable")), b"2");
    assert_eq!(fs::read(&source).unwrap(), "# edited 雪\n".as_bytes());
    finish(s);
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
        fn drop(&mut self) { unsafe { SetEvent(self.0.as_raw_handle()); } }
    }
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let event = |label: &str| {
        let name = format!("Local\\fish-N9-core-{}-{serial}-{label}", std::process::id());
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(!handle.is_null(), "{}", std::io::Error::last_os_error());
        (name, unsafe { OwnedHandle::from_raw_handle(handle) })
    };
    let (ready_name, ready) = event("ready");
    let (release_name, release) = event("release");
    let release = Release(release);
    let script = format!("{} n8-services-event {} {} &; set -g n9_child $last_pid; printf '%s' $n9_child > core-child-pid; complete -c n9-job -f -a '(__fish_complete_job_pids)'; complete -C 'n9-job ' > core-job-before; disown %1; printf '%s' $status > core-disown-status; complete -C 'n9-job ' > core-job-after; printf ready > core-job-ready",
        f.probe(), quote(&ready_name), quote(&release_name));
    let session = start(f, &script);
    assert_eq!(unsafe { WaitForSingleObject(ready.as_raw_handle(), 30_000) }, 0, "owned native child ready");
    let mut session = session;
    session.wait_file(&f.root.join("core-job-ready"));
    let pid: u32 = fs::read_to_string(f.root.join("core-child-pid")).unwrap().parse().unwrap();
    let process = unsafe { OpenProcess(0x0010_0000, 0, pid) };
    assert!(!process.is_null(), "retain actual acknowledged child");
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    drop(release);
    assert_eq!(unsafe { WaitForSingleObject(process.as_raw_handle(), 30_000) }, 0, "disowned child must terminate after controlled release");
    assert_eq!(fs::read(f.root.join("core-disown-status")).unwrap(), b"0");
    let before = fs::read_to_string(f.root.join("core-job-before")).unwrap();
    assert!(before.lines().any(|line| line.starts_with(&format!("{pid}\t"))), "{before}");
    assert!(fs::read(f.root.join("core-job-after")).unwrap().is_empty(), "disowned jobs must disappear from job-only completion");
    finish(session);
    let result = f.eval("wait %999");
    assert_ne!(result.code, 0);
    assert!(result.out.is_empty());
}

fn xdg(f: &Fixture) {
    let home = f.root.join("data home 雪");
    let first = f.root.join("data first 🐟");
    let second = f.root.join("data second [literal]");
    for directory in [&home, &first, &second] {
        fs::create_dir_all(directory.join("applications")).unwrap();
        fs::create_dir_all(directory.join("icons/theme/scalable/apps")).unwrap();
    }
    for (directory, name) in [(&home, "home-app"), (&first, "first-app"), (&second, "second-app")] {
        fs::write(directory.join(format!("applications/{name}.desktop")), "[Desktop Entry]\nType=Application\n").unwrap();
    }
    fs::write(first.join("icons/theme/scalable/apps/native-snow.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n").unwrap();
    fs::write(second.join("icons/theme/scalable/apps/native-fish.png"), b"icon fixture").unwrap();
    fs::write(second.join("icons/theme/scalable/apps/native-not-an-icon.txt"), b"excluded").unwrap();
    let environment = format!("set -gx XDG_DATA_HOME {}; set -gx XDG_DATA_DIRS {};", text(&home), quote(&format!("{};{};{}", first.display(), f.root.join("missing data root").display(), second.display())));
    let apps = success(f, &format!("{environment} complete -c n9-app -f -a '(for directory in (__fish_print_xdg_applications_directories); path basename -- $directory/*.desktop | path change-extension \"\"; end)'; complete -C 'n9-app '"));
    assert_eq!(values(&apps), BTreeSet::from(["first-app", "home-app", "second-app"]));
    let icons = success(f, &format!("{environment} complete -c n9-icon -f -a '(__fish_complete_freedesktop_icons)'; complete -C 'n9-icon native-'"));
    assert_eq!(values(&icons), BTreeSet::from(["native-fish", "native-snow"]));
    let fallback = f.state.join("home/.local/share/applications");
    fs::create_dir_all(&fallback).unwrap();
    fs::write(fallback.join("fallback-app.desktop"), "[Desktop Entry]\nType=Application\n").unwrap();
    let apps = success(f, "set -e XDG_DATA_HOME XDG_DATA_DIRS; complete -c n9-app -f -a '(for directory in (__fish_print_xdg_applications_directories); path basename -- $directory/*.desktop | path change-extension \"\"; end)'; complete -C 'n9-app '");
    assert_eq!(values(&apps), BTreeSet::from(["fallback-app"]));
}

fn listing(f: &Fixture) {
    use std::{ffi::OsStr, path::PathBuf};
    let ls = PathBuf::from(std::env::var_os("FISH_N9_LS").expect("required exact native FISH_N9_LS fixture"));
    assert!(ls.is_absolute() && ls.is_file(), "required native ls: {}", ls.display());
    assert!(ls.file_name().unwrap().to_str().unwrap().eq_ignore_ascii_case("ls.exe"), "fixture must expose the actual native ls CLI");
    let directory = f.root.join("native listing 雪");
    fs::create_dir(&directory).unwrap();
    fs::create_dir(directory.join("folder 🐟")).unwrap();
    fs::write(directory.join("visible 雪.txt"), vec![b'x'; 4097]).unwrap();
    fs::write(directory.join(".hidden 雪.txt"), b"hidden").unwrap();
    fs::write(directory.join("-literal.txt"), b"literal").unwrap();
    let invoke = |code: &str| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.env("PATH", ls.parent().unwrap()).env("NO_COLOR", "1");
        let result = super::fixture::run(&mut command, b"");
        assert_eq!(result.code, 0, "{code}: {}", String::from_utf8_lossy(&result.err));
        assert!(result.err.is_empty(), "{code}: {}", String::from_utf8_lossy(&result.err));
        String::from_utf8(result.out).unwrap()
    };
    let formats = invoke("complete -C 'ls --format='");
    assert!(values(&formats).contains("--format=single-column"), "{formats}");
    let output = invoke(&format!("cd {}; ls --format=single-column --color=never --ignore='.hidden*' -- .", text(&directory)));
    assert_eq!(values(&output), BTreeSet::from(["-literal.txt", "folder 🐟", "visible 雪.txt"]));
    let output = invoke(&format!("cd {}; ll -n --color=never --block-size=1 --time-style=+N9 -- 'visible 雪.txt'", text(&directory)));
    let metadata = output.trim_end().strip_suffix(" visible 雪.txt").expect("long listing must identify actual file");
    assert_eq!(metadata.split_whitespace().nth(4), Some("4097"), "{output}");
    assert_eq!(metadata.split_whitespace().last(), Some("N9"), "{output}");
    let output = invoke(&format!("cd {}; la --color=never --time-style=+N9 -- .", text(&directory)));
    assert!(output.lines().any(|line| line.ends_with(" .hidden 雪.txt")), "{output}");
    assert!(output.lines().any(|line| line.ends_with(" folder 🐟")), "{output}");
    let result = {
        let code = format!("cd {}; ls --color=never -- does-not-exist", text(&directory));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", ls.parent().unwrap());
        super::fixture::run(&mut command, b"")
    };
    assert_ne!(result.code, 0);
    assert!(result.out.is_empty());
    let absent_formats = success(f, "complete -C 'ls --format='");
    assert!(!values(&absent_formats).contains("--format=single-column"), "GNU values require the real native ls feature probe: {absent_formats}");
    for name in ["ls", "la", "ll"] {
        let result = f.eval(&format!("{name} -- {}", text(&directory)));
        assert_eq!(result.code, 127, "{name}: {}", String::from_utf8_lossy(&result.err));
        assert!(result.out.is_empty());
    }
    let readlink = ls.with_file_name("readlink.exe");
    assert!(readlink.is_file(), "required native readlink from the same verified coreutils distribution: {}", readlink.display());
    let candidates = invoke("complete -C 'readlink --no-n'");
    assert!(values(&candidates).contains("--no-newline"), "{candidates}");
    let payload = directory.join("canonical input 雪.txt");
    fs::write(&payload, "canonical native content 雪\n").unwrap();
    let output = invoke(&format!("set -l canonical (complete -C 'readlink --canonicalize' | string split -f 1 \\t | string match -- --canonicalize); test (count $canonical) -eq 1; or exit 81; set -l resolved (readlink $canonical --no-newline -- {}); or exit 82; __fish_native_fs read $resolved", text(&payload)));
    assert_eq!(output, "canonical native content 雪\n");
    let absent_readlink = success(f, "complete -C 'readlink --canonicalize'");
    assert!(!values(&absent_readlink).contains("--canonicalize"), "GNU-only long option must not leak into the no-tool context: {absent_readlink}");
}

fn variable_editor(f: &Fixture) {
    let record = f.root.join("variable-editor-export");
    let child = f.root.join("variable editor consumer.fish");
    fs::write(&child, format!("test \"$N9_EDITED\" = 'edited 雪=🐟'; or exit 81\nprintf consumed > {}\n", text(&record))).unwrap();
    exact(f.input(&format!("set -gx N9_EDITED original; vared N9_EDITED >/dev/null; or exit 81; {} -N {}", text(&f.fish), text(&child)), "edited 雪=🐟\n".as_bytes()), 0, b"", b"");
    assert_eq!(fs::read(&record).unwrap(), b"consumed");
    exact(f.input("set -g N9_ARRAY first second third; vared 'N9_ARRAY[2]' >/dev/null; or exit 81; test (count $N9_ARRAY) -eq 3; or exit 82; test \"$N9_ARRAY[1]\" = first; or exit 83; test \"$N9_ARRAY[2]\" = 'changed 雪'; or exit 84; test \"$N9_ARRAY[3]\" = third", "changed 雪\n".as_bytes()), 0, b"", b"");
    exact(f.input("set -g N9_ARRAY first second third; vared 'N9_ARRAY[-1]' >/dev/null; or exit 81; test \"$N9_ARRAY[1]\" = first; or exit 82; test \"$N9_ARRAY[2]\" = second; or exit 83; test \"$N9_ARRAY[3]\" = 'last 雪'", "last 雪\n".as_bytes()), 0, b"", b"");
    exact(f.input("vared N9_NEW >/dev/null; or exit 81; test \"$N9_NEW\" = 'created 雪'; or exit 82; not set -q __fish_vared_temp_value; and not set -q __fish_vared_scope", "created 雪\n".as_bytes()), 0, b"", b"");
    exact(f.input("set -gx N9_EMPTY ''; vared N9_EMPTY >/dev/null; or exit 81; set --query --export N9_EMPTY; or exit 82; test \"$N9_EMPTY\" = 'filled 雪'", "filled 雪\n".as_bytes()), 0, b"", b"");
    exact(f.eval("set -g N9_EDITED original; vared N9_EDITED >/dev/null; set -l result $status; test $result -ne 0; and test \"$N9_EDITED\" = original"), 0, b"", b"");
    exact(f.eval("vared N9_MISSING >/dev/null; set -l result $status; test $result -ne 0; and not set -q N9_MISSING"), 0, b"", b"");
    for code in ["vared", "vared first second", "set -g N9_ARRAY first second; vared N9_ARRAY"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "{code}");
        assert!(result.out.is_empty());
    }
}

fn searching(f: &Fixture) {
    use std::{ffi::OsStr, path::PathBuf};
    let grep = PathBuf::from(std::env::var_os("FISH_N9_GREP").expect("required exact native FISH_N9_GREP fixture"));
    assert!(grep.is_absolute() && grep.is_file(), "required native grep: {}", grep.display());
    assert!(grep.file_name().unwrap().to_str().unwrap().eq_ignore_ascii_case("grep.exe"));
    let file = f.root.join("search input 雪.txt");
    fs::write(&file, "skip\n雪 alpha=one\n雪 beta=two\n雪 beta=three\nliteral.*[雪]\n").unwrap();
    let invoke = |code: &str, input: &[u8]| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.env("PATH", grep.parent().unwrap());
        super::fixture::run(&mut command, input)
    };
    exact(invoke(&format!("grep --color=never -E '^雪 beta=' -- {} | string split -f 2 =; test $pipestatus[1] -eq 0", text(&file)), b""), 0, b"two\nthree\n", b"");
    exact(invoke(&format!("grep --color=never -F -x -- 'literal.*[雪]' {}", text(&file)), b""), 0, "literal.*[雪]\n".as_bytes(), b"");
    exact(invoke("grep --color=never -F '雪' | string upper; test $pipestatus[1] -eq 0", "skip\nnative 雪=one\nnative 雪=two\n".as_bytes()), 0, "NATIVE 雪=ONE\nNATIVE 雪=TWO\n".as_bytes(), b"");
    exact(invoke(&format!("grep --color=never -F -- absent {}", text(&file)), b""), 1, b"", b"");
    for code in [format!("grep --color=never -E '[' -- {}", text(&file)), "grep --color=never anything does-not-exist".into()] {
        let result = invoke(&code, b"");
        assert_eq!(result.code, 2, "{code}: {}", String::from_utf8_lossy(&result.err));
        assert!(result.out.is_empty());
    }
    let completion = invoke("complete -C 'grep --color='", b"");
    assert_eq!(completion.code, 0);
    assert!(completion.err.is_empty());
    let completed = String::from_utf8(completion.out).unwrap();
    assert!(values(&completed).contains("--color=never"), "{completed}");
    let absent = f.eval(&format!("grep -F snow -- {}", text(&file)));
    assert_eq!(absent.code, 127);
    assert!(absent.out.is_empty());
}

fn legacy_shell(f: &Fixture) {
    let receipt = f.root.join("quoted-native-argv");
    let script = f.root.join("quoted native consumer 雪.fish");
    fs::write(&script, format!("test (count $argv) -eq 6; or exit 81\ntest \"$argv[1]\" = ''; or exit 82\ntest \"$argv[2]\" = 'space 雪'; or exit 83\ntest \"$argv[3]\" = \"quote'fish\"; or exit 84\ntest \"$argv[4]\" = 'tail\\\\'; or exit 85\ntest \"$argv[5]\" = '-leading=雪'; or exit 86\ntest \"$argv[6]\" = '(not-code); &'; or exit 87\nprintf consumed > {}\n", text(&receipt))).unwrap();
    let provisioned = std::path::PathBuf::from(std::env::var_os("FISH_N9_SH").expect("required genuine native Win32 POSIX sh fixture, never MSYS"));
    assert!(provisioned.is_absolute() && provisioned.is_file());
    let shell_directory = f.root.join("native shell tools 雪");
    fs::create_dir(&shell_directory).unwrap();
    fs::copy(&provisioned, shell_directory.join("sh.exe")).unwrap();
    fs::write(f.state.join("home/.profile"), "exit 81\n").unwrap();
    let shell_setup = format!("set -gx PATH {}; set -l runner (__fish_posix_shell); or exit 82;", text(&shell_directory));
    exact(f.eval(&format!("{shell_setup} set -l source (__fish_posix_quote {} -N {} '' 'space 雪' \"quote'fish\" 'tail\\\\' '-leading=雪' '(not-code); &'); $runner -c \"$source\"", text(&f.fish), text(&script))), 0, b"", b"");
    exact(f.eval(&format!("{shell_setup} $runner -c 'exit 23'")), 23, b"", b"");
    exact(f.eval(&format!("{shell_setup} $runner -c {}", quote("printf 'shell-error 雪\\n' >&2; exit 17"))), 17, b"", "shell-error 雪\n".as_bytes());
    let absent = f.eval("__fish_posix_shell");
    assert_eq!(absent.code, 127);
    assert!(absent.out.is_empty());
    assert_eq!(fs::read(&receipt).unwrap(), b"consumed");
    let program = f.root.join("legacy formatted function.fish");
    fs::write(&program, format!("function n9_formatted\nif test (count $argv) -eq 1\nprintf '%s' \"$argv[1]\" > {}\nend\nend\n", text(&f.root.join("formatted-consumed")))).unwrap();
    exact(f.eval(&format!("__fish_indent < {} | source; or exit 81; n9_formatted 'formatted 雪'", text(&program))), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("formatted-consumed")).unwrap(), "formatted 雪".as_bytes());
    fs::copy(env!("CARGO_BIN_EXE_fish_indent"), f.root.join("fish_indent.exe")).expect("required real sibling native fish_indent fixture");
    exact(f.eval(&format!("__fish_indent < {} | source; or exit 81; n9_formatted 'sibling formatted 雪'", text(&program))), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("formatted-consumed")).unwrap(), "sibling formatted 雪".as_bytes());
    let serialized = f.root.join("legacy-cursor-data");
    exact(f.eval(&format!("begin; printf 'record 雪'; __fish_cursor_konsole line; test $status -eq 1; or exit 81; printf ':complete'; end > {}", text(&serialized))), 0, b"", b"");
    assert_eq!(fs::read(&serialized).unwrap(), "record 雪:complete".as_bytes());
    let prompt = "function n9_legacy_prompt; if fish_in_macos_terminal; printf (N_ '%s:%s') (prompt_hostname) (__fish_pwd); else; printf (N_ '%s') (__fish_pwd); end; end; functions -c n9_legacy_prompt fish_prompt; bind f2 __fish_cancel_commandline; bind f3 __fish_prepend_sudo; bind f4 'n9_legacy_prompt > legacy-prompt-path; printf ready > legacy-prompt-ready'";
    let mut session = start(f, prompt);
    session.send(b"printf forbidden > legacy-cancel-effect");
    session.key(true, 1, 0x71, 0, 0, 0);
    session.send(b"function n9_fail; return 7; end; n9_fail\r");
    session.send(b"printf '%s' $status > legacy-status\r");
    assert_eq!(session.wait_file(&f.root.join("legacy-status")), b"7");
    assert!(!f.root.join("legacy-cancel-effect").exists());
    session.send(b"printf 'prefix preserved'");
    session.key(true, 1, 0x72, 0, 0, 0);
    session.key(true, 1, 0x72, 0, 0, 0);
    session.send(b" > legacy-prefix-effect\r");
    assert_eq!(session.wait_file(&f.root.join("legacy-prefix-effect")), b"prefix preserved");
    session.key(true, 1, 0x73, 0, 0, 0);
    session.wait_file(&f.root.join("legacy-prompt-ready"));
    let rendered_path = fs::read_to_string(f.root.join("legacy-prompt-path")).unwrap();
    assert_eq!(fs::canonicalize(Path::new(&rendered_path)).unwrap(), fs::canonicalize(&f.root).unwrap());
    finish(session);
}

fn locales(f: &Fixture) {
    use std::ffi::OsStr;
    fs::write(f.state.join("config/locale.conf"), "LANG=bogus-unix-profile\nLC_NUMERIC=bogus-unix-profile\n").unwrap();
    let code = "not set -q LANG; or exit 81; test \"$LC_NUMERIC\" = C; or exit 82; printf '%.1f\\n' 1.5";
    let mut command = f.command(&[OsStr::new("-c"), OsStr::new(code)]);
    command.env_remove("LANG").env("LC_NUMERIC", "C");
    exact(super::fixture::run(&mut command, b""), 0, b"1.5\n", b"");
    let code = "complete -C 'set LC_NUMERIC POSI'";
    let output = success(f, code);
    assert_eq!(values(&output), BTreeSet::from(["POSIX"]));
    let output = success(f, "complete -C 'set N9_UNRELATED POSI'");
    assert!(!values(&output).contains("POSIX"), "locale-only value must not leak into an unrelated variable's file completions: {output}");
    exact(f.eval("set -l locale (complete -C 'set LC_NUMERIC POSI' | string split -f 1 \\t); test (count $locale) -eq 1; or exit 81; set -gx LC_NUMERIC $locale; printf '%.1f\\n' 1.5; set -gx LC_NUMERIC C; printf '%.1f\\n' 2.5"), 0, b"1.5\n2.5\n", b"");
}

fn legacy_vcs(f: &Fixture) {
    use std::{ffi::OsStr, path::PathBuf};
    let mut failures = Vec::new();
    for (variable, name, setup, prompts, marker) in [
        ("FISH_N9_GIT", "git", "git init -b legacy-雪; and git add 'tracked 雪.txt'; and git -c user.name=N9 -c user.email=n9@example.invalid commit -m initial", &["__fish_git_prompt", "__fish_vcs_prompt", "__terlar_git_prompt"][..], "legacy-雪"),
        ("FISH_N9_HG", "hg", "hg init; and hg branch legacy-雪; and hg add 'tracked 雪.txt'; and hg commit -u 'N9 <n9@example.invalid>' -m initial", &["__fish_hg_prompt"][..], "legacy-雪"),
        ("FISH_N9_SVN", "svn", "", &["__fish_svn_prompt"][..], "M"),
    ] {
        eprintln!("RUN-SUBCONSUMER functions_core::legacy_vcs/{name}");
        let exercised = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let binary = PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| panic!("required exact native {variable}")));
            assert!(binary.is_absolute() && binary.is_file(), "{variable}: {}", binary.display());
            let storage = (name == "svn").then(super::functions_tools::OptionalStorage::new);
            let unicode_directory = f.root.join(format!("legacy {name} repository 雪"));
            let svn_boundary = storage.as_ref().map(|storage| {
                let admin = binary.parent().unwrap().join("svnadmin.exe");
                assert!(admin.is_file(), "required real SVN repository producer");
                let mut command = std::process::Command::new(admin);
                command.arg("create").arg(&unicode_directory).current_dir(&f.root).env_clear();
                for (key, value) in f.command(&[]).get_envs() {
                    if let Some(value) = value { command.env(key, value); }
                }
                storage.environment(&mut command);
                let baseline = super::fixture::run(&mut command, b"");
                fs::write(f.root.join("legacy-svn-direct-unicode.stdout"), &baseline.out).unwrap();
                fs::write(f.root.join("legacy-svn-direct-unicode.stderr"), &baseline.err).unwrap();
                fs::write(f.root.join("legacy-svn-direct-unicode.status"), baseline.code.to_string()).unwrap();
                eprintln!("NATIVE-VCS-BASELINE legacy-svn-direct-unicode path={} status={} stderr={}", unicode_directory.display(), baseline.code, String::from_utf8_lossy(&baseline.err));
                assert_ne!(baseline.code, 0, "capability changed: restore Unicode SVN storage");
                assert!(String::from_utf8_lossy(&baseline.err).contains("E721113"));
                baseline
            });
            let directory = storage.as_ref().map_or(unicode_directory, |storage| storage.root.join("legacy-svn"));
            fs::create_dir(&directory).unwrap();
            let tracked = if name == "svn" { "tracked.txt" } else { "tracked 雪.txt" };
            fs::write(directory.join(tracked), "initial native file\n").unwrap();
            let invoke = |working: &Path, code: &str| {
                let code = format!("cd {}; {code}", text(working));
                let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
                command.env("PATH", binary.parent().unwrap()).env("HGENCODING", "utf-8").env("PYTHONUTF8", "1");
                if let Some(storage) = &storage { storage.environment(&mut command); }
                let result = super::fixture::run(&mut command, b"");
                assert_eq!(result.code, 0, "{variable}: {code}: {}", String::from_utf8_lossy(&result.err));
                String::from_utf8(result.out).unwrap()
            };
            let working = if name == "svn" {
                let repository = directory.join("repository");
                let native = repository.to_str().unwrap().replace('\\', "/");
                let mut uri = String::from("file:///");
                for byte in native.bytes() {
                    if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) { uri.push(byte as char); }
                    else { uri.push_str(&format!("%{byte:02X}")); }
                }
                invoke(&directory, &format!("svnadmin create repository; and svn checkout {} checkout; and cd checkout; and printf 'initial native file 雪 🐟' > tracked.txt; and svn add tracked.txt; and svn commit --username N9 -m initial", quote(&uri)));
                directory.join("checkout")
            } else {
                invoke(&directory, setup);
                directory.clone()
            };
            for &prompt in prompts {
                let code = format!("set -g __fish_git_prompt_showdirtystate 1; set -g __fish_git_prompt_char_dirtystate LEGACY-DIRTY; function fish_prompt; set -l incoming $status $pipestatus; {prompt}; printf '\\nINCOMING:%s:%s\\n' $incoming[1] (string join , $incoming[2..]); end; true | false; fish_prompt");
                let clean = invoke(&working, &code);
                assert!(clean.contains("INCOMING:1:0,1"), "{name}: {clean}");
                if name != "svn" { assert!(clean.contains(marker), "{prompt}: {clean}"); }
                if name == "git" { assert!(!clean.contains("LEGACY-DIRTY"), "{clean}"); }
                fs::write(working.join(tracked), "changed native file 雪\n").unwrap();
                let dirty = invoke(&working, &code);
                assert!(dirty.contains("INCOMING:1:0,1"), "{name}: {dirty}");
                if name == "git" { assert!(dirty.contains("LEGACY-DIRTY"), "{prompt}: {dirty}"); }
                if name == "hg" { assert!(dirty.contains(marker) && dirty.contains('⚡'), "{dirty}"); }
                if name == "svn" { assert!(dirty.contains(marker), "{dirty}"); }
                fs::write(working.join(tracked), if name == "svn" { "initial native file 雪 🐟" } else { "initial native file\n" }).unwrap();
                let absent = success(f, &format!("cd {}; function fish_prompt; set -l incoming $status $pipestatus; if not {prompt}; printf '%s> ' (__fish_pwd); end; printf '\\nINCOMING:%s:%s\\n' $incoming[1] (string join , $incoming[2..]); end; true | false; fish_prompt", text(&working)));
                assert!(absent.contains(working.file_name().unwrap().to_str().unwrap()) && absent.contains("INCOMING:1:0,1"), "{prompt}: {absent}");
                assert!(!absent.contains("LEGACY-DIRTY"), "{absent}");
            }
            if let Some(boundary) = svn_boundary {
                // The legacy prompt delegates to the cwd-based SVN consumer;
                // ASCII working-copy coverage cannot satisfy Unicode cwd.
                assert_eq!(boundary.code, 0, "unmet native SVN Unicode repository/cwd requirement after genuine ASCII working-copy/Unicode-data coverage: {}", String::from_utf8_lossy(&boundary.err));
            }
        })).is_ok();
        if exercised {
            eprintln!("PASS-SUBCONSUMER functions_core::legacy_vcs/{name}");
        } else {
            eprintln!("FAIL-SUBCONSUMER functions_core::legacy_vcs/{name}");
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "required native functions_core::legacy_vcs subconsumers failed: {failures:?}");
}

fn manual_commands(f: &Fixture) {
    let setup = "complete -c n9-doc -f -a '(__fish_print_commands)';";
    let directory = std::env::var_os("FISH_N9_FISH_MAN_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/fish-docs/man"));
    assert!(directory.join("man1/source.1").is_file(), "required real generated fish source manual: {}", directory.display());
    let build_info = success(f, "status build-info");
    if !build_info.split_whitespace().any(|feature| feature == "embed-manpages") {
        let installed = std::path::PathBuf::from(success(f, "printf '%s' \"$__fish_man_dir\"")).join("man1");
        assert!(installed.starts_with(&f.root), "manual fixture must remain under its owned installation: {}", installed.display());
        exact(f.eval(&format!("{setup} complete -C 'n9-doc sou'")), 0, b"", b"");
        fs::create_dir_all(&installed).unwrap();
        for entry in fs::read_dir(directory.join("man1")).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                fs::copy(entry.path(), installed.join(entry.file_name())).unwrap();
            }
        }
    }
    let script = f.root.join("documented source consumer.fish");
    let receipt = f.root.join("documented-source-effect");
    fs::write(&script, format!("set -g n9_documented_value 'source state 雪'\nprintf '%s' \"$n9_documented_value\" > {}\n", text(&receipt))).unwrap();
    exact(f.eval(&format!("{setup} set -l selected (complete -C 'n9-doc sou' | string split -f 1 \\t); test (count $selected) -eq 1; or exit 81; $selected {}; or exit 82; test \"$n9_documented_value\" = 'source state 雪'", text(&script))), 0, b"", b"");
    assert_eq!(fs::read(&receipt).unwrap(), "source state 雪".as_bytes());
    for excluded in ["fish-faq", "fish-language", "fish-tutorial", "fish-interactive", "fish-completions", "fish-for-bash-users", "fish-terminal-compatibility"] {
        exact(f.eval(&format!("{setup} complete -C {}", quote(&format!("n9-doc {excluded}")))), 0, b"", b"");
    }
}

fn manuals(f: &Fixture) {
    let generated = std::env::var_os("FISH_N9_HELP_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("FISH_RESOLVED_BUILD_DIR")).join("fish-docs/help"));
    let install = f.root.join("manual installation 雪");
    let bin = install.join("bin");
    let help = install.join("share/doc/fish/help");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(help.join("cmds")).unwrap();
    let executable = bin.join("fish.exe");
    fs::copy(&f.fish, &executable).unwrap();
    fs::write(help.join("fish-help-version"), format!("{}\n", fish::BUILD_VERSION)).unwrap();
    let mut published = std::collections::BTreeMap::new();
    for topic in ["set", "source", "not", "true", "test", "begin", "string-match"] {
        let source = generated.join("cmds").join(format!("{topic}.txt"));
        let page = fs::read_to_string(&source).unwrap_or_else(|error| panic!("required actual generated Sphinx text {}: {error}", source.display()));
        fs::write(help.join("cmds").join(format!("{topic}.txt")), &page).unwrap();
        published.insert(topic, page);
    }
    let invoke = |code: &str| f.eval(&format!("{} -N -c {}", text(&executable), quote(code)));
    for (request, topic) in [
        ("set", "set"), ("1 set", "set"), ("-- set", "set"), ("'.'", "source"),
        ("'!'", "not"), ("':'", "true"), ("'['", "test"), ("'{'", "begin"),
    ] {
        let expected = format!("fish {} — {topic}\n\n{}", fish::BUILD_VERSION, published[topic]);
        exact(invoke(&format!("man {request}")), 0, expected.as_bytes(), b"");
    }
    for request in ["", "2 set", "set extra", "--not-a-viewer-option", "n9-no-such-topic", "'set#n9-no-such-section'"] {
        let result = invoke(&format!("man {request}"));
        assert_ne!(result.code, 0, "man {request}");
        assert!(result.out.is_empty());
    }
    let init = "set -g n9_manual_action 0; function n9_manual_action; __fish_man_page; set -l result $status; set -g n9_manual_action (math $n9_manual_action + 1); printf '%s' $result > manual-action-$n9_manual_action; end; bind f2 n9_manual_action";
    let initialization = super::editor::init(init);
    let mut command = std::process::Command::new(&executable);
    command.args(["-N", "-i", "-C", &initialization]).current_dir(&f.root).env_clear();
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(key, value); }
    }
    let mut session = super::console::Session::start(&command, &f.probe, &f.root);
    session.wait_protocol(true);
    for (index, line, expected_status) in [
        (1, "env LANG=C builtin set --global n9_unexecuted", b"0".as_slice()),
        (2, "string match", b"0".as_slice()),
        (3, "n9-no-such-topic", b"1".as_slice()),
        (4, "", b"0".as_slice()),
    ] {
        if index > 1 { session.send(b"\x03"); }
        session.send(line.as_bytes());
        session.key(true, 1, 0x71, 0, 0, 0);
        assert_eq!(session.wait_file(&f.root.join(format!("manual-action-{index}"))), expected_status);
        capture(&mut session, f, index, line);
    }
    session.send(b"set -q n9_unexecuted; printf '%s' $status > manual-original-not-executed\r");
    assert_eq!(session.wait_file(&f.root.join("manual-original-not-executed")), b"1");
    let transcript = finish(session);
    for topic in ["set", "string-match"] {
        let heading = published[topic].lines().find(|line| !line.trim().is_empty()).unwrap().as_bytes();
        assert!(transcript.bytes.windows(heading.len()).any(|bytes| bytes == heading), "actual {topic} manual must be displayed");
    }
}
