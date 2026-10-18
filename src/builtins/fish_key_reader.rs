//! Print bind commands for decoded keys and, with --verbose, their input sequences.
//! Unlike byte-dump tools such as `xxd`, this interprets modifiers and special keys,
//! including Ctrl-C and Ctrl-D, as key input rather than shell interrupts or EOF.
//! On Unix this disables ICRNL so carriage-return (\cM) and newline (\cJ) remain distinct.
//!
//! In continuous mode, type "exit" or "quit", or press Ctrl-C or Ctrl-D twice to exit.
//! The native Windows tool uses the shared console input worker and key decoder;
//! it never reads VT bytes from a pipe. Native standalone input and output must
//! both be consoles. Help and version require no console or external manual.
//! Bind commands go to stdout, while prompts and errors go to stderr.

use std::ops::ControlFlow;

#[cfg(unix)]
use libc::{STDIN_FILENO, VEOF, VINTR};

#[cfg(unix)]
use crate::{
    common::shell_modes,
    env::env_init,
    nix::isatty,
    print_help::print_help,
    reader::check_exit_loop_maybe_warning,
    reader::{reader_init, set_shell_modes, signal_safe_reader_set_exit_signal, terminal_init},
    tty_handoff::TtyHandoff,
};

#[cfg(unix)]
use crate::topic_monitor;
use crate::{
    builtins::Error,
    common::{PROGRAM_NAME, get_program_name},
    env::{EnvStack, Environment as _},
    err_fmt, err_str,
    input::{
        CharEvent, ImplicitEvent, InputEventQueue, InputEventQueuer as _, KeyEvent,
        QueryResultEvent, match_key_event_to_key,
    },
    key::Key,
    panic::panic_handler,
    prelude::*,
    proc::set_interactive_session,
    text_helpers::char_to_symbol,
    threads,
};
use fish_wgetopt::{ArgType, WGetopter, WOption, wopt};
use fish_widestring::osstr2wcstring;

use super::prelude::*;

/// Native consoles have no configurable POSIX special-character table.
fn exit_keys() -> [u8; 2] {
    #[cfg(unix)]
    {
        let modes = shell_modes();
        [modes.control_chars[VINTR], modes.control_chars[VEOF]]
    }
    #[cfg(windows)]
    {
        [3, 4]
    }
}

/// Return true if the recent sequence of characters indicates the user wants to exit the program.
fn should_exit(
    streams: &mut IoStreams,
    recent_keys: &mut Vec<KeyEvent>,
    key_evt: KeyEvent,
) -> bool {
    recent_keys.push(key_evt);

    for byte in exit_keys() {
        let cc = Key::from_single_byte(byte);

        if match_key_event_to_key(&key_evt, &cc).is_some() {
            if recent_keys
                .iter()
                .rev()
                .nth(1)
                .and_then(|prev| match_key_event_to_key(prev, &cc))
                .is_some()
            {
                return true;
            }
            streams.err.appendln(&wgettext_fmt!(
                "Press ctrl-%c again to exit",
                char::from(byte + 0x60)
            ));
            return false;
        }
    }

    let Some(tail) = recent_keys
        .len()
        .checked_sub(4)
        .and_then(|start| recent_keys.get(start..))
    else {
        return false;
    };
    let tail = tail.iter().map(|c| c.codepoint);
    tail.clone().eq("exit".chars()) || tail.eq("quit".chars())
}

/// Process the characters we receive as the user presses keys.
fn process_input(
    streams: &mut IoStreams,
    continuous_mode: bool,
    verbose: bool,
    mut input_queue: InputEventQueue,
    #[cfg(windows)] parser: &mut Parser,
) -> BuiltinResult {
    let mut first_char_seen = false;
    let mut recent_chars = vec![];
    streams.err.appendln("Press a key:\n");

    #[cfg(unix)]
    let mut handoff = TtyHandoff::new(|| {});
    #[cfg(unix)]
    handoff.enable_tty_protocols();

    while !first_char_seen || continuous_mode {
        #[cfg(windows)]
        native_check_output(streams)?;
        #[cfg(unix)]
        if check_exit_loop_maybe_warning(None) {
            break;
        }
        #[cfg(windows)]
        if let Some(status) = native_service_controls(parser, streams, &mut input_queue)? {
            return BuiltinResult::from_dynamic(status);
        }
        let event = input_queue.readch();
        #[cfg(windows)]
        {
            if let Some(error) = input_queue.get_input_data_mut().native_error.take() {
                return native_error(streams, &error);
            }
            if let Some(status) = native_service_controls(parser, streams, &mut input_queue)? {
                return BuiltinResult::from_dynamic(status);
            }
        }
        use QueryResultEvent::*;
        let kevt = match event {
            CharEvent::Implicit(ImplicitEvent::Eof) => {
                #[cfg(unix)]
                {
                    signal_safe_reader_set_exit_signal(libc::SIGHUP);
                    continue;
                }
                #[cfg(windows)]
                {
                    crate::reader::native_console_closed();
                    return Err(STATUS_CMD_ERROR);
                }
            }
            CharEvent::Key(kevt) => kevt,
            CharEvent::Readline(_) | CharEvent::Command(_) | CharEvent::Implicit(_) => continue,
            CharEvent::QueryResult(Timeout | Interrupted) => {
                #[cfg(unix)]
                panic!("should not be querying");
                #[cfg(windows)]
                continue;
            }
            CharEvent::QueryResult(Response(_)) => continue,
        };
        if verbose {
            streams.out.append(L!("# decoded from: "));
            for (i, byte) in kevt.seq.chars().enumerate() {
                streams.out.append(&char_to_symbol(byte, i == 0));
            }
            streams.out.append(L!("\n"));
        }
        let have_shifted_key = kevt.key.shifted_codepoint != '\0';
        let mut keys = vec![(kevt.key.key, "")];
        if have_shifted_key {
            let mut shifted_key = kevt.key.key;
            shifted_key.modifiers.shift = false;
            shifted_key.codepoint = kevt.key.shifted_codepoint;
            keys.push((shifted_key, "shifted key"));
        }
        if kevt.key.associated_text[0] != '\0' {
            let associated_text = WString::from_iter(kevt.key.text_to_insert().unwrap());
            if associated_text.char_count() > 1
                || associated_text.as_char_slice().first() != kevt.key.codepoint_text().as_ref()
            {
                streams.out.append(&sprintf!(
                    "# text for insertion (not used for bind matching): %s\n",
                    associated_text
                ));
            }
        }
        if let Some(base_layout_codepoint) = kevt.key.base_layout_codepoint() {
            let mut base_layout_key = kevt.key.key;
            base_layout_key.codepoint = base_layout_codepoint;
            keys.push((base_layout_key, "physical key"));
        }
        for (key, explanation) in keys {
            streams.out.append(&sprintf!(
                "bind %s 'do something'%s%s\n",
                key,
                if explanation.is_empty() { "" } else { " # " },
                explanation,
            ));
        }
        #[cfg(windows)]
        native_check_output(streams)?;

        if continuous_mode && should_exit(streams, &mut recent_chars, kevt.key) {
            streams.err.appendln("\nExiting at your request.");
            break;
        }

        first_char_seen = true;
    }
    Ok(SUCCESS)
}

/// Setup our environment (e.g., tty modes), process key strokes, then reset the environment.
#[cfg(unix)]
fn setup_and_process_keys(
    streams: &mut IoStreams,
    continuous_mode: bool,
    verbose: bool,
    input_queue: InputEventQueue,
) -> BuiltinResult {
    // We need to set the shell-modes for ICRNL,
    // in fish-proper this is done once a command is run.
    set_shell_modes(STDIN_FILENO, "fish_key_reader");

    if continuous_mode {
        streams.err.append(L!("\n"));
        streams
            .err
            .appendln("To terminate this program type \"exit\" or \"quit\" in this window,");
        let [interrupt, eof] = exit_keys();
        streams.err.appendln(&wgettext_fmt!(
            "or press ctrl-%c or ctrl-%c twice in a row.",
            char::from(interrupt + 0x60),
            char::from(eof + 0x60)
        ));
        streams.err.appendln(L!("\n"));
    }

    process_input(streams, continuous_mode, verbose, input_queue)
}

#[cfg(windows)]
fn native_error(streams: &mut IoStreams, error: &impl std::fmt::Display) -> BuiltinResult {
    err_fmt!("native key reader failed: %s", error.to_string())
        .cmd(L!("fish_key_reader"))
        .finish(streams);
    Err(STATUS_CMD_ERROR)
}

#[cfg(windows)]
fn native_check_output(streams: &mut IoStreams) -> BuiltinResult {
    // Attempt both streams even if stdout failed. The stream retains the real
    // native write error; a failed prompt must never leave us waiting for input.
    let out_error = streams.out.flush_and_check_error();
    let err_error = streams.err.flush_and_check_error();
    if out_error != 0 {
        if let crate::io::OutputStream::Fd(output) = &*streams.out {
            if let Some(error) = output.error() {
                let error = error.to_string();
                err_fmt!("cannot write stdout: %s", error)
                    .cmd(L!("fish_key_reader"))
                    .finish(streams);
            }
        }
    }
    if out_error != 0 || err_error != 0 {
        return Err(STATUS_CMD_ERROR);
    }
    Ok(SUCCESS)
}

#[cfg(windows)]
fn native_service_controls(
    parser: &mut Parser,
    streams: &mut IoStreams,
    input_queue: &mut InputEventQueue,
) -> Result<Option<i32>, ErrorCode> {
    let controls = crate::signal::pending_console_controls();
    if controls.is_empty() {
        return Ok(parser
            .libdata()
            .exit_current_script
            .then(|| parser.last_status()));
    }
    if let Err(error) = input_queue.get_input_data_mut().native.interrupt() {
        native_error(streams, &error)?;
    }
    if controls.ctrl_c != 0 || controls.ctrl_break != 0 {
        crate::reader::native_reader_interrupt();
    }
    if controls.close != 0 {
        crate::reader::native_console_closed();
    }
    // Rearm acknowledges cancellation, not counted event receipts. The event
    // subsystem separately transfers those original native controls and can now
    // run handlers. A new racing receipt remains pending in its own epoch.
    if let Err(error) = crate::signal::rearm_console_controls() {
        native_error(streams, &error)?;
    }
    crate::event::fire_delayed(parser);
    Ok(Some(if parser.libdata().exit_current_script {
        parser.last_status()
    } else if controls.close != 0 {
        1
    } else {
        130
    }))
}

#[cfg(windows)]
fn setup_and_process_native_keys(
    parser: &mut Parser,
    streams: &mut IoStreams,
    continuous_mode: bool,
    verbose: bool,
) -> BuiltinResult {
    use crate::terminal::Outputter;
    use crate::tty_handoff::{NativeEditorScope, initialize_tty_protocols};

    let mut scope = match NativeEditorScope::enter() {
        Ok(scope) => scope,
        Err(error) => return native_error(streams, &error),
    };
    // All fallible work after acquiring the editor is inside this closure so
    // normal errors, console close and native controls reach checked restoration.
    let result = (|| {
        initialize_tty_protocols(parser.vars());
        if let Err(error) = scope.enable_protocols() {
            return native_error(streams, &error);
        }
        let mut input_queue = InputEventQueue::new(0, None);
        if let Err(error) = Outputter::stdoutput().borrow_mut().flush() {
            return native_error(streams, &error);
        }
        if let Some(status) = native_service_controls(parser, streams, &mut input_queue)? {
            return BuiltinResult::from_dynamic(status);
        }
        if continuous_mode {
            streams.err.append(L!(
                "\nTo terminate this program type \"exit\" or \"quit\" in this window,\n\
                 or press ctrl-c or ctrl-d twice in a row.\n\n"
            ));
        }
        native_check_output(streams)?;
        if let Err(error) = input_queue.get_input_data().native.resume() {
            return native_error(streams, &error);
        }
        process_input(streams, continuous_mode, verbose, input_queue, parser)
    })();
    let restored = scope.finish();
    if let Err(error) = restored {
        native_error(streams, &error)?;
    }
    native_check_output(streams)?;
    result
}

fn parse_flags(
    _parser: Option<&mut Parser>,
    streams: &mut IoStreams,
    args: Vec<WString>,
    continuous_mode: &mut bool,
    verbose: &mut bool,
) -> ControlFlow<BuiltinResult> {
    let short_opts: &wstr = L!("+chvV");
    let long_opts: &[WOption] = &[
        wopt(L!("continuous"), ArgType::NoArgument, 'c'),
        wopt(L!("help"), ArgType::NoArgument, 'h'),
        wopt(L!("version"), ArgType::NoArgument, 'v'),
        wopt(L!("verbose"), ArgType::NoArgument, 'V'),
    ];
    let mut shim_args: Vec<&wstr> = args.iter().map(|s| s.as_ref()).collect();
    let mut w = WGetopter::new(short_opts, long_opts, &mut shim_args);
    while let Some(opt) = w.next_opt() {
        match opt {
            'c' => {
                *continuous_mode = true;
            }
            'h' => {
                #[cfg(windows)]
                {
                    streams.out.append(L!(
                        "Usage: fish_key_reader [OPTIONS]\n\
                         Print fish bind commands for keys read from the console.\n\n\
                         Options:\n\
                           -c, --continuous  Read keys until \"exit\", \"quit\", or two consecutive\n\
                                             Ctrl-C or Ctrl-D keys (default: read one key).\n\
                           -V, --verbose     Also print the decoded input sequence.\n\
                           -h, --help        Print this help and exit.\n\
                           -v, --version     Print the version and exit.\n\n\
                         No positional arguments are accepted. Bind commands and verbose\n\
                         sequences go to stdout; prompts and errors go to stderr.\n\
                         Reading requires console stdin and an installed console session;\n\
                         the standalone program also requires console stdout. Redirected\n\
                         pipes are not interpreted as terminal input. Help and version\n\
                         do not require a console.\n"
                    ));
                    return ControlFlow::Break(native_check_output(streams));
                }
                #[cfg(unix)]
                {
                    if let Some(parser) = _parser {
                        return ControlFlow::Break(builtin_print_help(
                            parser,
                            streams,
                            L!("fish_key_reader"),
                        ));
                    }
                    print_help("fish_key_reader");
                    return ControlFlow::Break(Ok(SUCCESS));
                }
            }
            'v' => {
                streams
                    .out
                    .appendln(&localized_version_string(get_program_name()));
                return ControlFlow::Break(Ok(SUCCESS));
            }
            'V' => {
                *verbose = true;
            }
            ';' => {
                err_fmt!(Error::UNEXP_OPT_ARG, w.argv[w.wopt_index - 1])
                    .cmd(L!("fish_key_reader"))
                    .finish(streams);
                return ControlFlow::Break(Err(STATUS_CMD_ERROR));
            }
            '?' => {
                err_fmt!(Error::UNKNOWN_OPT, w.argv[w.wopt_index - 1])
                    .cmd(L!("fish_key_reader"))
                    .finish(streams);
                return ControlFlow::Break(Err(STATUS_CMD_ERROR));
            }
            _ => panic!(),
        }
    }

    let argc = args.len() - w.wopt_index;
    if argc != 0 {
        err_fmt!("Expected no arguments, got %d", argc).finish(streams);
        return ControlFlow::Break(Err(STATUS_CMD_ERROR));
    }

    ControlFlow::Continue(())
}

pub fn fish_key_reader(
    parser: &mut Parser,
    streams: &mut IoStreams,
    args: &mut [&wstr],
) -> BuiltinResult {
    let mut continuous_mode = false;
    let mut verbose = false;

    let args = args.iter_mut().map(|x| x.to_owned()).collect();
    if let ControlFlow::Break(s) = parse_flags(
        Some(parser),
        streams,
        args,
        &mut continuous_mode,
        &mut verbose,
    ) {
        #[cfg(windows)]
        native_check_output(streams)?;
        return s;
    }

    #[cfg(unix)]
    {
        if streams.stdin_fd() < 0 || !isatty(streams.stdin_fd()) {
            err_str!("Stdin must be attached to a tty.").finish(streams);
            return Err(STATUS_CMD_ERROR);
        }

        setup_and_process_keys(
            streams,
            continuous_mode,
            verbose,
            // Won't be querying, so no timeout value needed.
            InputEventQueue::new(streams.stdin_fd(), None),
        )
    }
    #[cfg(windows)]
    {
        if streams.stdin_is_directly_redirected || !streams.is_terminal(0) {
            err_str!("Stdin must be attached directly to the native console.")
                .cmd(L!("fish_key_reader"))
                .finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
        setup_and_process_native_keys(parser, streams, continuous_mode, verbose)
    }
}

pub fn main() {
    PROGRAM_NAME.set(L!("fish_key_reader")).unwrap();
    #[cfg(windows)]
    threads::init();
    panic_handler(throwing_main)
}

#[cfg(unix)]
fn throwing_main() -> i32 {
    use crate::io::FdOutputStream;
    use crate::io::IoChain;
    use crate::io::OutputStream::Fd;
    use libc::{STDERR_FILENO, STDOUT_FILENO};

    set_interactive_session(true);
    topic_monitor::init();
    threads::init();
    #[cfg(feature = "localize-messages")]
    if let Err(error) = crate::localization::initialize_localization() {
        eprintf!(
            "fish_key_reader: unable to initialize localization: %s\n",
            error.to_string()
        );
        return 2;
    }
    env_init(None, false);
    reader_init(false);
    if let Some(features_var) = EnvStack::globals().get(L!("fish_features")) {
        for s in features_var.as_list() {
            fish_feature_flags::set_from_string(s.as_utfstr());
        }
    }

    let mut out = Fd(FdOutputStream::new(STDOUT_FILENO));
    let mut err = Fd(FdOutputStream::new(STDERR_FILENO));
    let io_chain = IoChain::new();
    let streams = &mut IoStreams::new(&mut out, &mut err, &io_chain);

    let mut continuous_mode = false;
    let mut verbose = false;

    let args: Vec<WString> = std::env::args_os().map(osstr2wcstring).collect();
    if let ControlFlow::Break(s) =
        parse_flags(None, streams, args, &mut continuous_mode, &mut verbose)
    {
        return s.builtin_status_code();
    }

    if !isatty(STDIN_FILENO) {
        err_str!("Stdin must be attached to a tty.").finish(streams);
        return 1;
    }

    let input_queue = {
        let vars = EnvStack::new();
        env_stack_set_from_env!(vars, "STY");
        env_stack_set_from_env!(vars, "TERM");
        terminal_init(&vars, STDIN_FILENO).input_queue
    };

    setup_and_process_keys(streams, continuous_mode, verbose, input_queue).builtin_status_code()
}

#[cfg(windows)]
fn throwing_main() -> i32 {
    use crate::console_windows::{ConsoleSession, install_session};
    use crate::fds::{is_console, standard_handle};
    use crate::io::{IoChain, OutputStream};
    use crate::parser::CancelBehavior;
    use fish_widestring::native_to_shell_text_owned;

    let mut out = OutputStream::stdout();
    let mut err = OutputStream::stderr();
    let io_chain = IoChain::new();
    let streams = &mut IoStreams::new(&mut out, &mut err, &io_chain);
    let args: Vec<WString> = match std::env::args_os()
        .map(|arg| osstr2wcstring(arg).map(native_to_shell_text_owned))
        .collect()
    {
        Ok(args) => args,
        Err(error) => return native_error(streams, &error).builtin_status_code(),
    };
    let mut continuous_mode = false;
    let mut verbose = false;
    if let ControlFlow::Break(result) =
        parse_flags(None, streams, args, &mut continuous_mode, &mut verbose)
    {
        return native_check_output(streams)
            .and(result)
            .builtin_status_code();
    }
    // Validate the independently obtained actual standard handles. A pipe that
    // happens to carry VT bytes is not a console and must never start this UI.
    let handles = (|| {
        let input = standard_handle(-10i32 as u32)?;
        let output = standard_handle(-11i32 as u32)?;
        if !is_console(input) || !is_console(output) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "stdin and stdout must both be attached to native consoles",
            ));
        }
        Ok((input, output))
    })();
    let (input, output) = match handles {
        Ok(handles) => handles,
        Err(error) => return native_error(streams, &error).builtin_status_code(),
    };
    if !unsafe { crate::locale::set_libc_locales(false) } {
        err_str!("Unable to initialize the native C locale.")
            .cmd(L!("fish_key_reader"))
            .finish(streams);
        return STATUS_CMD_ERROR;
    }
    let variables = EnvStack::globals().create_child(true);
    if let Err(error) = variables.initialize_native() {
        return native_error(streams, &error).builtin_status_code();
    }
    if let Some(features_var) = variables.get(L!("fish_features")) {
        for value in features_var.as_list() {
            fish_feature_flags::set_from_string(value.as_utfstr());
        }
    }
    let mut controls = match crate::signal::initialize_console_controls() {
        Ok(controls) => controls,
        Err(error) => return native_error(streams, &error).builtin_status_code(),
    };
    let result = (|| {
        let session = match ConsoleSession::new(input, output) {
            Ok(session) => session,
            Err(error) => return native_error(streams, &error),
        };
        let _session = install_session(session);
        let mut parser = match Parser::new(variables, CancelBehavior::Return) {
            Ok(parser) => parser,
            Err(error) => return native_error(streams, &error),
        };
        set_interactive_session(true);
        setup_and_process_native_keys(&mut parser, streams, continuous_mode, verbose)
    })();
    let shutdown = controls.shutdown();
    if let Err(error) = shutdown {
        return native_error(streams, &error).builtin_status_code();
    }
    native_check_output(streams)
        .and(result)
        .builtin_status_code()
}
