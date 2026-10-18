use super::*;
use crate::{
    abbrs::{self, Position, Replacement},
    builtins::BuiltinResultExt,
    env::EnvStack,
    io::{IoChain, IoStreams, OutputStream, StringOutputStream},
    parser::{CancelBehavior, Parser},
};
use std::{
    os::windows::io::AsRawHandle,
    process::{Command, Stdio},
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
    // Parser execution is main-thread-only. Run the owning-state assertions in
    // their own libtest process, rather than changing another test's thread ID
    // or sharing its global metadata and environment.
    if std::env::var_os("FISH_NATIVE_METADATA_TEST_CHILD").is_none() {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "complete::metadata_tests::native_declarative_metadata",
                "--nocapture",
            ])
            .env("FISH_NATIVE_METADATA_TEST_CHILD", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let wait = unsafe { WaitForSingleObject(child.as_raw_handle(), 30_000) };
        if wait != 0 {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native metadata test exceeded its process bound: {wait}");
        }
        assert!(child.wait().unwrap().success());
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
        assert!(set.has_match(L!("雪42"), Position::Anywhere, L!("tool")));
        assert!(!set.has_match(L!("雪42"), Position::Anywhere, L!("unrelated")));
        assert!(!set.has_match(L!("x雪42"), Position::Anywhere, L!("tool")));
        let replacers = set.r#match(L!("literal"), Position::Command, L!(""));
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
        assert!(set.has_match(L!("雪42"), Position::Anywhere, L!("other")));
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
