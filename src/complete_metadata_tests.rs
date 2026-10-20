use super::*;
use crate::tests::prelude::run_native_test_in_child;
use crate::{
    abbrs::{self, Position, Replacement},
    builtins::BuiltinResultExt,
    env::EnvStack,
    io::{IoChain, IoStreams, OutputStream, StringOutputStream},
    parser::{CancelBehavior, Parser},
};

fn invoke(parser: &mut Parser, arguments: &[&wstr]) -> WString {
    let mut out = OutputStream::String(StringOutputStream::new());
    let mut err = OutputStream::String(StringOutputStream::new());
    let chain = IoChain::new();
    let mut streams = IoStreams::new(&mut out, &mut err, &chain);
    let mut arguments = arguments.to_vec();
    let result = match arguments[0] {
        name if name == L!("abbr") => {
            crate::builtins::abbr::abbr(parser, &mut streams, &mut arguments)
        }
        name if name == L!("complete") => {
            crate::builtins::complete::complete(parser, &mut streams, &mut arguments)
        }
        _ => unreachable!(),
    };
    assert_eq!(result.builtin_status_code(), 0, "{}", err.contents());
    assert!(err.contents().is_empty(), "{}", err.contents());
    out.contents().to_owned()
}

#[derive(Debug, PartialEq)]
struct AbbrState {
    name: WString,
    key: WString,
    regex: bool,
    commands: Vec<WString>,
    replacement: WString,
    function: bool,
    position: Position,
    cursor: Option<WString>,
    description: Option<WString>,
    universal: bool,
}

fn abbreviation_state() -> Vec<AbbrState> {
    abbrs::with_abbrs(|set| {
        let mut states: Vec<_> = set
            .list()
            .iter()
            .map(|abbr| AbbrState {
                name: abbr.name.clone(),
                key: abbr.key.clone(),
                regex: abbr.is_regex(),
                commands: abbr.commands.clone(),
                replacement: abbr.replacement.clone(),
                function: abbr.replacement_is_function,
                position: abbr.position,
                cursor: abbr.set_cursor_marker.clone(),
                description: abbr.description.clone(),
                universal: abbr.from_universal,
            })
            .collect();
        states.sort_by(|a, b| (&a.name, &a.commands).cmp(&(&b.name, &b.commands)));
        states
    })
}

#[derive(Debug, PartialEq)]
struct RuleState {
    command: WString,
    path: bool,
    option: WString,
    typ: CompleteOptionType,
    policy: ArgumentPolicy,
    conditions: Vec<WString>,
    arguments: WString,
    description: WString,
    flags: CompleteFlags,
}

fn completion_state() -> Vec<RuleState> {
    let map = COMPLETION_MAP.lock().unwrap();
    let mut states: Vec<_> = map
        .iter()
        .flat_map(|(key, entry)| {
            entry.options.iter().map(move |rule| RuleState {
                command: key.name.clone(),
                path: key.is_path,
                option: rule.option.clone(),
                typ: rule.typ,
                policy: rule.argument_policy,
                conditions: rule.conditions.to_vec(),
                arguments: rule.comp.clone(),
                description: rule.desc.localize().to_owned(),
                flags: rule.flags,
            })
        })
        .collect();
    states.sort_by(|a, b| {
        (&a.command, a.path, &a.option, a.typ as u8).cmp(&(
            &b.command,
            b.path,
            &b.option,
            b.typ as u8,
        ))
    });
    states
}

fn replay(parser: &mut Parser, source: &wstr) {
    let result = parser.eval(source, &IoChain::new());
    assert_eq!(result.status.status_value(), 0);
    assert!(!result.break_expand);
}


#[test]
fn native_declarative_metadata() {
    if !run_native_test_in_child("complete::metadata_tests::native_declarative_metadata") {
        return;
    }
    crate::threads::init();
    crate::common::PROGRAM_NAME.set(L!("fish")).unwrap();
    let variables = EnvStack::new();
    variables.initialize_native().unwrap();
    let mut parser = Parser::new(variables, CancelBehavior::default()).unwrap();

    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("--add"),
            L!("--global"),
            L!("--position=command"),
            L!("--set-cursor"),
            L!("literal"),
            L!("echo %"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("-a"),
            L!("--regex"),
            L!("雪[0-9]+"),
            L!("--position=anywhere"),
            L!("--command"),
            L!("tool"),
            L!("-c"),
            L!("other"),
            L!("--function"),
            L!("expand_token"),
            L!("--set-cursor=@"),
            L!("--description=snow token"),
            L!("rx"),
        ],
    );
    invoke(
        &mut parser,
        &[L!("abbr"), L!("short"), L!("echo"), L!("-n"), L!("hello")],
    );
    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("-a"),
            L!("-f"),
            L!("other_function"),
            L!("function_short"),
        ],
    );
    assert_eq!(
        abbreviation_state(),
        vec![
            AbbrState {
                name: L!("function_short").to_owned(),
                key: L!("function_short").to_owned(),
                regex: false,
                commands: vec![],
                replacement: L!("other_function").to_owned(),
                function: true,
                position: Position::Command,
                cursor: None,
                description: None,
                universal: false,
            },
            AbbrState {
                name: L!("literal").to_owned(),
                key: L!("literal").to_owned(),
                regex: false,
                commands: vec![],
                replacement: L!("echo %").to_owned(),
                function: false,
                position: Position::Command,
                cursor: Some(L!("%").to_owned()),
                description: None,
                universal: false,
            },
            AbbrState {
                name: L!("rx").to_owned(),
                key: L!("雪[0-9]+").to_owned(),
                regex: true,
                commands: vec![L!("tool").to_owned(), L!("other").to_owned()],
                replacement: L!("expand_token").to_owned(),
                function: true,
                position: Position::Anywhere,
                cursor: Some(L!("@").to_owned()),
                description: Some(L!("snow token").to_owned()),
                universal: false,
            },
            AbbrState {
                name: L!("short").to_owned(),
                key: L!("short").to_owned(),
                regex: false,
                commands: vec![],
                replacement: L!("echo -n hello").to_owned(),
                function: false,
                position: Position::Command,
                cursor: None,
                description: None,
                universal: false,
            },
        ]
    );
    abbrs::with_abbrs(|set| {
        assert!(set.has_match(L!("雪42"), Position::Anywhere, L!("tool")).unwrap());
        assert!(!set.has_match(L!("雪42"), Position::Anywhere, L!("unrelated")).unwrap());
        assert!(!set.has_match(L!("x雪42"), Position::Anywhere, L!("tool")).unwrap());
        let replacers = set.r#match(L!("literal"), Position::Command, L!("")).unwrap();
        assert_eq!(replacers.len(), 1);
        let replacement = Replacement::new(
            crate::parse_constants::SourceRange {
                start: 3,
                length: 7,
            },
            replacers[0].replacement.clone(),
            replacers[0].set_cursor_marker.clone(),
        );
        assert_eq!(replacement.text, L!("echo "));
        assert_eq!(replacement.cursor, Some(8));
    });
    let before = abbreviation_state();
    let serialized = invoke(
        &mut parser,
        &[L!("abbr"), L!("--show"), L!("--color=never")],
    );
    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("--erase"),
            L!("literal"),
            L!("short"),
            L!("function_short"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("-e"),
            L!("-c"),
            L!("tool"),
            L!("-c"),
            L!("other"),
            L!("rx"),
        ],
    );
    assert!(abbreviation_state().is_empty());
    replay(&mut parser, &serialized);
    assert_eq!(abbreviation_state(), before);
    invoke(
        &mut parser,
        &[
            L!("abbr"),
            L!("--rename"),
            L!("-c"),
            L!("tool"),
            L!("-c"),
            L!("other"),
            L!("rx"),
            L!("renamed"),
        ],
    );
    abbrs::with_abbrs(|set| {
        assert!(!set.has_name(L!("rx")));
        assert!(set.has_name(L!("renamed")));
        assert!(set.has_match(L!("雪42"), Position::Anywhere, L!("other")).unwrap());
    });

    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("--command=first"),
            L!("--short-option=x"),
            L!("--arguments=one two"),
            L!("--description=pick one"),
            L!("--condition=true"),
            L!("--condition=test 1 = 1"),
            L!("--exclusive"),
            L!("--keep-order"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("first"),
            L!("-l"),
            L!("long"),
            L!("-a"),
            L!("red blue"),
            L!("--no-files"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("second"),
            L!("--old-option=old"),
            L!("--force-files"),
            L!("-a"),
            L!("value"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("second"),
            L!("-o"),
            L!("requires"),
            L!("--require-parameter"),
            L!("-a"),
            L!("parameter"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("--path=/virtual/tool"),
            L!("-s"),
            L!("z"),
            L!("-d"),
            L!("desc"),
            L!("-a"),
            L!("file"),
            L!("-n"),
            L!("false"),
            L!("-x"),
            L!("-k"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("second"),
            L!("--long-option=long"),
            L!("-r"),
            L!("-a"),
            L!("required"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("first"),
            L!("-a"),
            L!("a b"),
            L!("-n"),
            L!("true"),
            L!("-f"),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("second"),
            L!("-s"),
            L!("q"),
            L!("-F"),
            L!("-A"),
            L!("--unauthoritative"),
            L!("--authoritative"),
            L!("-u"),
        ],
    );
    let before = completion_state();
    assert_eq!(before.len(), 8);
    let cases = [
        (
            L!("first"),
            false,
            L!("x"),
            CompleteOptionType::Short,
            FileCompletionPolicy::Skip,
            true,
            vec![L!("true").to_owned(), L!("test 1 = 1").to_owned()],
            L!("one two"),
            L!("pick one"),
            true,
        ),
        (
            L!("first"),
            false,
            L!("long"),
            CompleteOptionType::DoubleLong,
            FileCompletionPolicy::Skip,
            false,
            vec![],
            L!("red blue"),
            L!(""),
            false,
        ),
        (
            L!("second"),
            false,
            L!("old"),
            CompleteOptionType::SingleLong,
            FileCompletionPolicy::Force,
            false,
            vec![],
            L!("value"),
            L!(""),
            false,
        ),
        (
            L!("second"),
            false,
            L!("requires"),
            CompleteOptionType::SingleLong,
            FileCompletionPolicy::Inherit,
            true,
            vec![],
            L!("parameter"),
            L!(""),
            false,
        ),
        (
            L!("/virtual/tool"),
            true,
            L!("z"),
            CompleteOptionType::Short,
            FileCompletionPolicy::Skip,
            true,
            vec![L!("false").to_owned()],
            L!("file"),
            L!("desc"),
            true,
        ),
        (
            L!("second"),
            false,
            L!("long"),
            CompleteOptionType::DoubleLong,
            FileCompletionPolicy::Inherit,
            true,
            vec![],
            L!("required"),
            L!(""),
            false,
        ),
        (
            L!("first"),
            false,
            L!(""),
            CompleteOptionType::ArgsOnly,
            FileCompletionPolicy::Skip,
            false,
            vec![L!("true").to_owned()],
            L!("a b"),
            L!(""),
            false,
        ),
        (
            L!("second"),
            false,
            L!("q"),
            CompleteOptionType::Short,
            FileCompletionPolicy::Force,
            false,
            vec![],
            L!(""),
            L!(""),
            false,
        ),
    ];
    for (
        command,
        path,
        option,
        typ,
        files,
        requires_param,
        conditions,
        arguments,
        description,
        dont_sort,
    ) in cases
    {
        let stored = before
            .iter()
            .find(|state| {
                state.command == command
                    && state.path == path
                    && state.option == option
                    && state.typ == typ
            })
            .unwrap();
        assert_eq!(
            stored.policy,
            ArgumentPolicy {
                files,
                requires_param
            }
        );
        assert_eq!(stored.conditions, conditions);
        assert_eq!(stored.arguments, arguments);
        assert_eq!(stored.description, description);
        assert_eq!(stored.flags.dont_sort, dont_sort);
        assert_eq!(stored.flags.wants_suffix, WantsSuffix::Auto);
        assert_eq!(
            stored.flags.wants_escaping,
            WantsEscaping::Yes {
                escape_tildes: false
            }
        );
    }
    invoke(
        &mut parser,
        &[L!("complete"), L!("-c"), L!("first"), L!("--wraps=base")],
    );
    invoke(
        &mut parser,
        &[L!("complete"), L!("-c"), L!("second"), L!("-w"), L!("base")],
    );
    assert_eq!(
        completion_state(),
        before,
        "wrapper-only declarations must not add rules"
    );
    assert_eq!(
        complete_get_wrap_targets(L!("first")),
        vec![L!("base").to_owned()]
    );
    assert_eq!(
        complete_get_wrap_targets(L!("second")),
        vec![L!("base").to_owned()]
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-e"),
            L!("-c"),
            L!("first"),
            L!("-w"),
            L!("base"),
        ],
    );
    assert!(complete_get_wrap_targets(L!("first")).is_empty());
    assert_eq!(
        complete_get_wrap_targets(L!("second")),
        vec![L!("base").to_owned()]
    );
    assert_eq!(
        completion_state(),
        before,
        "erasing a wrapper must preserve actual rules"
    );
    let serialized = invoke(&mut parser, &[L!("complete"), L!("--color=never")]);
    invoke(
        &mut parser,
        &[L!("complete"), L!("--erase"), L!("--command=first")],
    );
    invoke(
        &mut parser,
        &[L!("complete"), L!("-e"), L!("-c"), L!("second")],
    );
    invoke(
        &mut parser,
        &[L!("complete"), L!("--erase"), L!("--path=/virtual/tool")],
    );
    assert!(completion_state().is_empty());
    assert!(complete_get_wrap_targets(L!("second")).is_empty());
    replay(&mut parser, &serialized);
    assert_eq!(completion_state(), before);
    assert_eq!(
        complete_get_wrap_targets(L!("second")),
        vec![L!("base").to_owned()]
    );
    assert!(complete_get_wrap_targets(L!("first")).is_empty());

    // Wrapping and adding a real rule in one declaration must still do both.
    replay(
        &mut parser,
        L!("complete -c mixed --wraps=base -a 'mixed value' --no-files"),
    );
    assert_eq!(
        complete_get_wrap_targets(L!("mixed")),
        vec![L!("base").to_owned()]
    );
    let mixed = completion_state();
    let rule = mixed
        .iter()
        .find(|rule| rule.command == L!("mixed"))
        .unwrap();
    assert_eq!(rule.typ, CompleteOptionType::ArgsOnly);
    assert_eq!(rule.arguments, L!("mixed value"));
    assert_eq!(rule.policy.files, FileCompletionPolicy::Skip);
    replay(&mut parser, L!("complete -e -c mixed -w base"));
    assert!(complete_get_wrap_targets(L!("mixed")).is_empty());
    assert_eq!(completion_state(), mixed);
    assert_eq!(
        complete_get_wrap_targets(L!("second")),
        vec![L!("base").to_owned()]
    );
    replay(&mut parser, L!("complete -e -c mixed"));
    assert_eq!(completion_state(), before);

    // Distinct option identities survive replay regardless of declaration print order.
    replay(
        &mut parser,
        L!(
            "complete -c replay_options -s a -a alpha; complete -c replay_options -o old -a older; complete -c replay_options -s z -a last"
        ),
    );
    let option_source = invoke(
        &mut parser,
        &[L!("complete"), L!("--color=never"), L!("replay_options")],
    );
    let option_before = completion_state();
    replay(&mut parser, L!("complete -e -c replay_options"));
    replay(&mut parser, &option_source);
    assert_eq!(completion_state(), option_before);
    for (selected, typ, arguments, others, erase) in [
        (
            L!("a"),
            CompleteOptionType::Short,
            L!("alpha"),
            L!("complete -e -c replay_options -o old -s z"),
            L!("complete -e -c replay_options -s a"),
        ),
        (
            L!("old"),
            CompleteOptionType::SingleLong,
            L!("older"),
            L!("complete -e -c replay_options -s a -s z"),
            L!("complete -e -c replay_options -o old"),
        ),
        (
            L!("z"),
            CompleteOptionType::Short,
            L!("last"),
            L!("complete -e -c replay_options -s a -o old"),
            L!("complete -e -c replay_options -s z"),
        ),
    ] {
        replay(&mut parser, others);
        let state = completion_state();
        let mut remaining = state
            .iter()
            .filter(|rule| rule.command == L!("replay_options"));
        let rule = remaining.next().unwrap();
        assert!(remaining.next().is_none());
        assert_eq!(rule.option, selected);
        assert_eq!(rule.typ, typ);
        assert_eq!(rule.arguments, arguments);
        assert_eq!(rule.policy, ArgumentPolicy::default());
        assert!(rule.conditions.is_empty());
        assert!(rule.description.is_empty());
        replay(&mut parser, erase);
        assert_eq!(completion_state(), before);
        replay(&mut parser, &option_source);
    }
    replay(&mut parser, L!("complete -e -c replay_options"));
    assert_eq!(completion_state(), before);
}

#[test]
fn native_disk_completion_autoload() {
    if !run_native_test_in_child("complete::metadata_tests::native_disk_completion_autoload") {
        return;
    }
    crate::threads::init();
    crate::common::PROGRAM_NAME.set(L!("fish")).unwrap();

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "fish-native-completions-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    let first = fixture.0.join("first completions 雪");
    let second = fixture.0.join("second completions 雪");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let variables = EnvStack::new();
    // No inherited profile, universal store, config, or history initialization.
    variables
        .initialize_native_from(std::iter::empty(), &fixture.0)
        .unwrap();
    let mut parser = Parser::new(variables, CancelBehavior::default()).unwrap();
    replay(&mut parser, L!("set -g fish_function_path"));
    let select_path = |parser: &mut Parser, path: &std::path::Path| {
        let path = crate::path::native_path_text(path).unwrap();
        replay(
            parser,
            &(L!("set -g fish_complete_path ").to_owned() + &escape(&path)[..]),
        );
    };
    let rules_for = |command: &wstr| {
        completion_state()
            .into_iter()
            .filter(|rule| rule.command == command)
            .collect::<Vec<_>>()
    };
    let command = L!("n4_native_disk");
    std::fs::write(
        first.join("n4_native_disk.fish"),
        "set -l disk_choices 'alpha beta'\n\
         complete -c n4_native_disk -s x -l choice -x -k \
         -n 'test 1 = 1' -a \"$disk_choices\" -d 'disk choice'\n",
    )
    .unwrap();
    std::fs::write(
        second.join("n4_native_disk.fish"),
        "complete -c n4_native_disk -o legacy -F -a 'new value' -d 'replacement rule'\n",
    )
    .unwrap();
    select_path(&mut parser, &first);
    assert!(complete_load(command, &mut parser, true));
    let initial = rules_for(command);
    assert_eq!(initial.len(), 2);
    for (option, typ) in [
        (L!("x"), CompleteOptionType::Short),
        (L!("choice"), CompleteOptionType::DoubleLong),
    ] {
        let rule = initial.iter().find(|rule| rule.option == option).unwrap();
        assert!(!rule.path);
        assert_eq!(rule.typ, typ);
        assert_eq!(
            rule.policy,
            ArgumentPolicy {
                files: FileCompletionPolicy::Skip,
                requires_param: true,
            }
        );
        assert_eq!(rule.conditions, vec![L!("test 1 = 1").to_owned()]);
        assert_eq!(rule.arguments, L!("alpha beta"));
        assert_eq!(rule.description, L!("disk choice"));
        assert!(rule.flags.dont_sort);
    }
    assert!(!complete_load(command, &mut parser, true));
    assert_eq!(rules_for(command), initial);

    // Cache an actual miss before making a new command available. Changing the
    // parser-owned search path must discard that miss and the old loaded rules.
    let missing = L!("n4_native_late");
    assert!(!complete_load(missing, &mut parser, true));
    assert!(rules_for(missing).is_empty());
    std::fs::write(
        second.join("n4_native_late.fish"),
        "complete -c n4_native_late -l recovered -r -a 'late choice' -d 'newly available'\n",
    )
    .unwrap();
    select_path(&mut parser, &second);
    assert!(rules_for(command).is_empty());
    assert!(complete_load(command, &mut parser, true));
    let replacement = rules_for(command);
    assert_eq!(replacement.len(), 1);
    let rule = &replacement[0];
    assert_eq!(rule.option, L!("legacy"));
    assert_eq!(rule.typ, CompleteOptionType::SingleLong);
    assert_eq!(rule.policy.files, FileCompletionPolicy::Force);
    assert!(!rule.policy.requires_param);
    assert!(rule.conditions.is_empty());
    assert_eq!(rule.arguments, L!("new value"));
    assert_eq!(rule.description, L!("replacement rule"));
    assert!(complete_load(missing, &mut parser, true));
    let recovered = rules_for(missing);
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].option, L!("recovered"));
    assert_eq!(recovered[0].typ, CompleteOptionType::DoubleLong);
    assert!(recovered[0].policy.requires_param);
    assert_eq!(recovered[0].arguments, L!("late choice"));
    assert_eq!(recovered[0].description, L!("newly available"));

    // A syntax failure cannot leave the command permanently pending or loaded.
    let broken = L!("n4_native_broken");
    let broken_file = second.join("n4_native_broken.fish");
    std::fs::write(&broken_file, "if true\n").unwrap();
    assert!(!complete_load(broken, &mut parser, true));
    assert!(!complete_load(broken, &mut parser, true));
    assert!(rules_for(broken).is_empty());
    std::fs::write(
        &broken_file,
        "complete -c n4_native_broken -s b -f -a repaired -d 'repaired syntax'\n",
    )
    .unwrap();
    assert!(complete_load(broken, &mut parser, true));
    let repaired = rules_for(broken);
    assert_eq!(repaired.len(), 1);
    assert_eq!(repaired[0].option, L!("b"));
    assert_eq!(repaired[0].typ, CompleteOptionType::Short);
    assert_eq!(repaired[0].policy.files, FileCompletionPolicy::Skip);
    assert_eq!(repaired[0].arguments, L!("repaired"));
    assert_eq!(repaired[0].description, L!("repaired syntax"));
    assert!(!complete_load(broken, &mut parser, true));
    assert_eq!(rules_for(broken), repaired);

    // Retry a failed source without touching its bytes or file identity. This
    // distinguishes failure recovery from ordinary modified-file reloading.
    let retry = L!("n4_native_retry");
    std::fs::write(
        second.join("n4_native_retry.fish"),
        "if not set -q n4_allow_completion_load\n\
             return 1\n\
         end\n\
         complete -c n4_native_retry -l ready -a 'ready now' -d 'retried source'\n",
    )
    .unwrap();
    assert!(!complete_load(retry, &mut parser, true));
    assert!(rules_for(retry).is_empty());
    replay(&mut parser, L!("set -g n4_allow_completion_load 1"));
    assert!(complete_load(retry, &mut parser, true));
    let retried = rules_for(retry);
    assert_eq!(retried.len(), 1);
    assert_eq!(retried[0].option, L!("ready"));
    assert_eq!(retried[0].typ, CompleteOptionType::DoubleLong);
    assert_eq!(retried[0].arguments, L!("ready now"));
    assert_eq!(retried[0].description, L!("retried source"));
    assert!(!complete_load(retry, &mut parser, true));
    assert_eq!(rules_for(retry), retried);
}

#[test]
fn native_completion_cursor_context() {
    if !run_native_test_in_child("complete::metadata_tests::native_completion_cursor_context") {
        return;
    }
    crate::threads::init();
    crate::common::PROGRAM_NAME.set(L!("fish")).unwrap();
    let variables = EnvStack::new();
    variables
        .initialize_native_from(std::iter::empty(), &std::env::temp_dir())
        .unwrap();
    let mut parser = Parser::new(variables, CancelBehavior::default()).unwrap();
    replay(&mut parser, L!("set -g fish_function_path; set -g fish_complete_path"));
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("n9_cursor_target"),
            L!("-f"),
            L!("-a"),
            L!("prefix"),
            L!("-n"),
            L!("test (commandline -b) = \"$__n9_context_text\"; and test (commandline -C) = \"$__n9_context_cursor\"; and test (commandline -ct) = \"$__n9_context_token\""),
        ],
    );
    invoke(
        &mut parser,
        &[
            L!("complete"),
            L!("-c"),
            L!("n9_cursor_wrapper"),
            L!("-w"),
            L!("N9_CURSOR_LOCAL=assigned n9_cursor_target --fixed"),
        ],
    );

    fn expect_context(parser: &mut Parser, text: &wstr, cursor: usize, token: &wstr) {
        replay(
            parser,
            &(L!("set -g __n9_context_text ").to_owned() + &escape(text)[..]),
        );
        replay(parser, &sprintf!("set -g __n9_context_cursor %u", cursor));
        replay(
            parser,
            &(L!("set -g __n9_context_token ").to_owned() + &escape(token)[..]),
        );
    }

    let direct = L!("n9_cursor_target pr tail 雪");
    let wrapped = L!("N9_CURSOR_LOCAL=assigned n9_cursor_target --fixed pr tail 雪");
    for (text, before_cursor, expected_text, expected_before_cursor) in [
        (direct, L!("n9_cursor_target p"), direct, L!("n9_cursor_target p")),
        (
            L!("N9_CURSOR_LOCAL=original n9_cursor_target pr tail 雪"),
            L!("N9_CURSOR_LOCAL=original n9_cursor_target p"),
            direct,
            L!("n9_cursor_target p"),
        ),
        (
            L!("n9_cursor_wrapper pr tail 雪"),
            L!("n9_cursor_wrapper p"),
            wrapped,
            L!("N9_CURSOR_LOCAL=assigned n9_cursor_target --fixed p"),
        ),
        (
            L!("echo (n9_cursor_wrapper pr tail 雪) outside"),
            L!("echo (n9_cursor_wrapper p"),
            wrapped,
            L!("N9_CURSOR_LOCAL=assigned n9_cursor_target --fixed p"),
        ),
    ] {
        expect_context(&mut parser, expected_text, expected_before_cursor.len(), L!("p"));
        let (completions, _) = complete(
            text,
            Some(before_cursor.len()),
            CompletionRequestOptions::default(),
            &mut parser.context(),
        );
        // Matching still includes the token's trailing 'r', while commandline
        // reads the real cursor after 'p' and retains the following arguments.
        assert_eq!(completions.len(), 1, "{text}");
        assert_eq!(completions[0].completion, L!("efix"), "{text}");
        assert!(parser.libdata().transient_commandline.borrow().is_none());
    }

    // Explicit complete -C remains an independent synthetic end-of-line request.
    let synthetic = L!("n9_cursor_target pr");
    expect_context(&mut parser, synthetic, synthetic.len(), L!("pr"));
    assert_eq!(
        invoke(&mut parser, &[L!("complete"), L!("-C"), synthetic]),
        L!("prefix\n"),
    );
    assert!(parser.libdata().transient_commandline.borrow().is_none());
}
