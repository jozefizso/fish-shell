//! Native batch startup, sharing state and bootstrap policy with the shell evaluator.
use fish::{
    common::{PROFILING_ACTIVE, PROGRAM_NAME},
    env::{EnvMode, EnvStack},
    fds::BorrowedFdFile,
    io::{IoChain, NativeOutput},
    parser::{BlockType, CancelBehavior, Parser, ParserEnvSetMode},
    prelude::*,
};
use fish_wgetopt::{ArgType::*, WGetopter, WOption, wopt};
use fish_widestring::{native_to_shell_text_owned, osstr2wcstring, wcs2bytes};
use std::{fs::File, io::Write, sync::Arc};

fn diagnostic(message: &str) -> i32 {
    if let Ok(mut stderr) = NativeOutput::stderr() {
        let _ = writeln!(stderr, "fish: {message}");
    }
    2
}

/// Diagnostic-only resource reporting must not replace the command's status or
/// write into its stdout. A guard also covers early returns after option parsing.
struct PrintRusageOnDrop;

impl Drop for PrintRusageOnDrop {
    fn drop(&mut self) {
        if let Err(error) = NativeOutput::stderr()
            .and_then(|mut stderr| fish::native_accounting::write_self_usage(&mut stderr))
        {
            let _ = diagnostic(&format!(
                "unable to query or write native shell resource usage: {error}"
            ));
        }
    }
}

pub(super) fn main() {
    fish::threads::init();
    PROGRAM_NAME.set(L!("fish")).ok();
    fish::panic::panic_handler(run);
}

fn run() -> i32 {
    let args: Vec<WString> = match std::env::args_os()
        .map(|arg| osstr2wcstring(arg).map(native_to_shell_text_owned))
        .collect()
    {
        Ok(args) => args,
        Err(error) => return diagnostic(&format!("invalid native argument: {error}")),
    };
    let mut refs: Vec<&wstr> = args.iter().map(|arg| arg.as_ref()).collect();
    const LONG: &[WOption<'static>] = &[
        wopt(L!("command"), RequiredArgument, 'c'),
        wopt(L!("no-config"), NoArgument, 'N'),
        wopt(L!("no-execute"), NoArgument, 'n'),
        wopt(L!("features"), RequiredArgument, 'f'),
        wopt(L!("version"), NoArgument, 'v'),
        wopt(L!("debug"), RequiredArgument, 'd'),
        wopt(L!("debug-output"), RequiredArgument, 'o'),
        wopt(L!("profile"), RequiredArgument, 'p'),
        wopt(L!("print-debug-categories"), NoArgument, '\x02'),
        wopt(L!("debug-stack-frames"), RequiredArgument, 'D'),
        wopt(L!("init-command"), RequiredArgument, 'C'),
        wopt(L!("login"), NoArgument, 'l'),
        wopt(L!("private"), NoArgument, 'P'),
        wopt(L!("profile-startup"), RequiredArgument, '\x03'),
        wopt(L!("interactive"), NoArgument, 'i'),
        wopt(L!("help"), NoArgument, 'h'),
        wopt(L!("print-rusage-self"), NoArgument, '\x01'),
    ];
    let mut options = WGetopter::new(L!("+hPilNnvc:C:p:d:f:D:o:"), LONG, &mut refs);
    let mut no_config = false;
    let mut no_execute = false;
    let mut commands = Vec::new();
    let mut debug = Vec::new();
    let mut debug_output = None;
    let mut profile_output = None;
    let mut init_commands = Vec::new();
    let mut private = false;
    let mut login = args.first().is_some_and(|arg| arg.starts_with('-'));
    let mut startup_profile_output = None;
    let mut print_rusage_self = false;
    while let Some(option) = options.next_opt() {
        match option {
            'N' => no_config = true,
            'n' => no_execute = true,
            'c' => commands.push(options.woptarg.unwrap().to_owned()),
            'f' => fish_feature_flags::set_from_string(options.woptarg.unwrap()),
            'd' => debug.push(options.woptarg.unwrap().to_owned()),
            'o' => debug_output = Some(options.woptarg.unwrap().to_owned()),
            'p' => profile_output = Some(options.woptarg.unwrap().to_owned()),
            'v' => {
                return match NativeOutput::stdout()
                    .and_then(|mut out| writeln!(out, "fish, version {}", fish::BUILD_VERSION))
                {
                    Ok(()) => 0,
                    Err(error) => diagnostic(&format!("unable to write version: {error}")),
                };
            }
            '\x02' => {
                let mut out = match NativeOutput::stdout() {
                    Ok(out) => out,
                    Err(error) => return diagnostic(&error.to_string()),
                };
                for category in fish::flog::categories::all_categories() {
                    if let Err(error) = writeln!(
                        out,
                        "{}  {}",
                        category.name,
                        category.description.localize()
                    ) {
                        return diagnostic(&format!("unable to print debug categories: {error}"));
                    }
                }
                return 0;
            }
            'D' => {
                return diagnostic(
                    "--debug-stack-frames (-D) is retired; use --debug category selection",
                );
            }
            'C' => init_commands.push(options.woptarg.unwrap().to_owned()),
            'l' => login = true,
            'P' => private = true,
            '\x03' => startup_profile_output = Some(options.woptarg.unwrap().to_owned()),
            'i' => return diagnostic("interactive mode requires N6"),
            'h' => return diagnostic("help delivery requires N9"),
            '\x01' => print_rusage_self = true,
            ':' => {
                return diagnostic(&format!(
                    "option requires an argument: {}",
                    options.argv[options.wopt_index - 1]
                ));
            }
            ';' => {
                return diagnostic(&format!(
                    "option does not take an argument: {}",
                    options.argv[options.wopt_index - 1]
                ));
            }
            _ => {
                return diagnostic(&format!(
                    "unknown option: {}",
                    options.argv[options.wopt_index - 1]
                ));
            }
        }
    }
    let operands = &options.argv[options.wopt_index..];
    let _rusage = print_rusage_self.then(|| PrintRusageOnDrop);
    let mut console_controls = match fish::signal::initialize_console_controls() {
        Ok(guard) => guard,
        Err(error) => {
            return diagnostic(&format!(
                "unable to initialize console control handling: {error}"
            ));
        }
    };
    if !unsafe { fish::locale::set_libc_locales(false) } {
        return diagnostic("unable to initialize the native C locale");
    }
    let variables = EnvStack::globals().create_child(true /* dispatches_var_changes */);
    if let Err(error) = variables.initialize_native() {
        return diagnostic(&format!(
            "unable to initialize native environment/current directory: {error}"
        ));
    }
    let state_start = std::time::Instant::now();
    if !no_execute {
        if private || no_config {
            fish::history::start_private_mode(&variables);
        }
        if let Err(error) = variables.initialize_native_state() {
            return diagnostic(&format!("unable to initialize native state: {error}"));
        }
    }
    let _state_cleanup = variables.native_state_guard();
    if login {
        fish::proc::mark_login();
    }
    let state_elapsed = state_start.elapsed();
    let mut parser = match Parser::new(variables, CancelBehavior::Return) {
        Ok(parser) => parser,
        Err(error) => return diagnostic(&format!("unable to initialize native parser: {error}")),
    };
    let mut profile: Option<File> = None;
    // Keep final lifecycle dispatch outside all fallible command/setup returns.
    let status = (|| -> i32 {
        if let Some(path) = debug_output {
            let file = match fish::path::native_resolve_path_for_environment(&path, parser.vars())
                .and_then(File::create)
            {
                Ok(file) => file,
                Err(error) => {
                    return diagnostic(&format!("unable to open debug output '{path}': {error}"));
                }
            };
            if let Err(error) = fish::flog::set_flog_file(file.into()) {
                return diagnostic(&format!("unable to set debug output: {error}"));
            }
        }
        for pattern in debug {
            fish::flog::activate_flog_categories_by_pattern(&pattern);
        }
        // Open sinks before evaluating any user command, so an unusable output path cannot run code.
        profile = match profile_output {
            Some(path) => {
                match fish::path::native_resolve_path_for_environment(&path, parser.vars())
                    .and_then(File::create)
                {
                    Ok(file) => Some(file),
                    Err(error) => {
                        return diagnostic(&format!(
                            "unable to open profile output '{path}': {error}"
                        ));
                    }
                }
            }
            None => None,
        };
        let mut startup_profile = match startup_profile_output {
            Some(path) => {
                match fish::path::native_resolve_path_for_environment(&path, parser.vars())
                    .and_then(File::create)
                {
                    Ok(file) => Some(file),
                    Err(error) => {
                        return diagnostic(&format!(
                            "unable to open startup profile output '{path}': {error}"
                        ));
                    }
                }
            }
            None => None,
        };
        PROFILING_ACTIVE.store(profile.is_some() || startup_profile.is_some());
        if no_execute {
            fish::proc::mark_no_exec();
        }
        let io = IoChain::new();
        let mut status = 0;
        let mut syntax_status = 0;
        parser.set_var(
            L!("argv"),
            ParserEnvSetMode::new(EnvMode::GLOBAL),
            if commands.is_empty() {
                operands
                    .iter()
                    .skip(1)
                    .map(|arg| (*arg).to_owned())
                    .collect()
            } else {
                operands.iter().map(|arg| (*arg).to_owned()).collect()
            },
        );
        if !no_execute && !no_config {
            let started = std::time::Instant::now();
            if let Some(config) = fish::autoload::Asset::get("config.fish") {
                if let Err(message) = parser.eval_file_wstr(
                    fish_widestring::bytes2wcstring(&config.data),
                    Arc::new(L!("embedded:config.fish").to_owned()),
                    &io,
                    None,
                ) {
                    print_error(&message);
                }
            } else {
                return diagnostic("embedded startup configuration is missing");
            }
            for root in [L!("__fish_sysconf_dir"), L!("__fish_config_dir")] {
                use fish::env::Environment;
                if let Some(root) = parser.vars().get(root) {
                    let mut path = root.as_string();
                    fish::path::append_path_component(&mut path, L!("config.fish"));
                    match fish::path::native_resolve_path_for_environment(&path, parser.vars())
                        .and_then(std::fs::metadata)
                    {
                        Ok(metadata) if metadata.is_file() => {
                            let command =
                                L!("source ").to_owned() + &fish_common::escape(&path)[..];
                            parser.eval(&command, &io);
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            print_error(&fish_widestring::str2wcstring(format!(
                                "fish: unable to inspect configuration '{path}': {error}\n"
                            )));
                        }
                    }
                }
                if parser.libdata().exit_current_script {
                    break;
                }
            }
            if let Some(out) = startup_profile.as_mut() {
                if let Err(error) =
                    writeln!(out, "# configuration\t{} us", started.elapsed().as_micros())
                {
                    let _ = diagnostic(&format!("unable to write startup profile: {error}"));
                }
            }
        }
        if !parser.libdata().exit_current_script {
            for command in init_commands {
                if no_execute {
                    status = check_source(&parser, &command);
                    if status != 0 {
                        syntax_status = status;
                    }
                } else {
                    status = match parser.eval_wstr(command, &io, None, BlockType::Top) {
                        Ok(result) => {
                            if result.status.is_empty() {
                                parser.last_status()
                            } else {
                                result.status.status_value()
                            }
                        }
                        Err(message) => {
                            print_error(&message);
                            127
                        }
                    };
                }
                if parser.libdata().exit_current_script {
                    break;
                }
            }
        }
        if let Some(out) = startup_profile.as_mut() {
            let result = writeln!(
                out,
                "# native state initialization\t{} us",
                state_elapsed.as_micros()
            )
            .and_then(|_| parser.write_profiling(out));
            if let Err(error) = result {
                let _ = diagnostic(&format!("unable to write startup profile: {error}"));
            }
        }
        PROFILING_ACTIVE.store(profile.is_some());
        if parser.libdata().exit_current_script {
            status = parser.last_status();
        } else if !commands.is_empty() {
            for command in commands {
                if no_execute {
                    status = check_source(&parser, &command);
                } else {
                    status = match parser.eval_wstr(command, &io, None, BlockType::Top) {
                        Ok(result) => {
                            if result.status.is_empty() {
                                parser.last_status()
                            } else {
                                result.status.status_value()
                            }
                        }
                        Err(message) => {
                            print_error(&message);
                            127
                        }
                    };
                }
                if parser.libdata().exit_current_script || fish::signal::signal_check_cancel() != 0
                {
                    break;
                }
            }
        } else {
            let mut input = if let Some(filename) = operands.first() {
                *parser.current_filename.borrow_mut() = Some(Arc::new((*filename).to_owned()));
                parser.set_var(
                    L!("argv"),
                    ParserEnvSetMode::new(EnvMode::GLOBAL),
                    operands[1..].iter().map(|arg| (*arg).to_owned()).collect(),
                );
                match fish::path::native_resolve_path_for_environment(filename, parser.vars())
                    .and_then(File::open)
                {
                    Ok(file) => BorrowedFdFile::from_file(Arc::new(file)),
                    Err(error) => {
                        return diagnostic(&format!("unable to read script '{filename}': {error}"));
                    }
                }
            } else {
                match BorrowedFdFile::stdin() {
                    Ok(input) if !input.is_console() => input,
                    Ok(_) => {
                        return diagnostic(
                            "interactive input requires N6; supply a script, redirected stdin or -c",
                        );
                    }
                    Err(error) => {
                        return diagnostic(&format!("standard input is unavailable: {error}"));
                    }
                }
            };
            if no_execute {
                let mut bytes = Vec::new();
                match input.read_to_end(&mut bytes) {
                    Ok(_) => {
                        let mut source = fish_widestring::bytes2wcstring(&bytes);
                        if source.chars().next() == Some('\u{feff}') {
                            source.remove(0);
                        }
                        status = check_source(&parser, &source);
                    }
                    Err(error) => {
                        status = if fish::signal::signal_check_cancel() != 0 {
                            130
                        } else {
                            diagnostic(&format!("unable to read input: {error}"))
                        }
                    }
                }
            } else {
                status = match fish::reader::reader_read(&mut parser, &mut input, &io) {
                    Ok(()) => parser.last_status(),
                    Err(_) if fish::signal::signal_check_cancel() != 0 => 130,
                    Err(_) => 127,
                };
            }
        }
        // Diagnostic sink failures are reported separately and never replace command status.
        if no_execute && syntax_status != 0 {
            syntax_status
        } else {
            status
        }
    })();
    if !no_execute {
        if fish::signal::signal_check_cancel() != 0 {
            if let Err(error) = console_controls.rearm() {
                let _ = diagnostic(&format!(
                    "unable to rearm console controls for exit handlers: {error}"
                ));
            }
        }
        // An explicit `exit` ends the command stream, not its exit handlers.
        parser.libdata_mut().exit_current_script = false;
        fish::proc::job_reap(&mut parser, false, None);
        fish::event::fire(&mut parser, fish::event::Event::shell_exit(status));
        fish::event::fire_generic(
            &mut parser,
            L!("fish_exit").to_owned(),
            vec![status.to_wstring()],
        );
        fish::proc::native_shutdown_jobs(&mut parser);
    }
    if let Some(file) = profile.as_mut() {
        if let Err(error) = parser.write_profiling(file) {
            let _ = diagnostic(&format!("unable to write profile output: {error}"));
        }
    }
    if let Some(error) = fish::flog::take_flog_error() {
        let _ = diagnostic(&format!("unable to write debug output: {error}"));
    }
    drop(_state_cleanup);
    if let Err(error) = console_controls.check_health() {
        let _ = diagnostic(&format!("console control handoff failed: {error}"));
    }
    if let Err(error) = console_controls.shutdown() {
        let _ = diagnostic(&format!(
            "unable to shut down console control handling: {error}"
        ));
    }
    status
}

fn check_source(parser: &Parser, source: &wstr) -> i32 {
    let mut errors = Vec::new();
    if fish::parse_util::detect_parse_errors(source, Some(&mut errors), false).is_err() {
        print_error(&parser.get_backtrace(source, &errors));
        127
    } else {
        0
    }
}

fn print_error(message: &wstr) {
    if let Ok(mut stderr) = NativeOutput::stderr() {
        let _ = stderr.write_all(&wcs2bytes(message));
    }
}
