//! Native batch startup. Configuration policy and terminal editing remain separate owners.
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
            'C' | 'l' | 'P' | '\x03' => {
                return diagnostic("startup/login/private/startup-profile options require N4");
            }
            'i' => return diagnostic("interactive mode requires N6"),
            'h' => return diagnostic("help delivery requires N9"),
            '\x01' => return diagnostic("resource usage reporting requires N5"),
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
    if !no_config {
        return diagnostic("native Windows batch mode requires -N (--no-config)");
    }
    if !unsafe { fish::locale::set_libc_locales(false) } {
        return diagnostic("unable to initialize the native C locale");
    }
    let variables = EnvStack::new();
    if let Err(error) = variables.initialize_native() {
        return diagnostic(&format!(
            "unable to initialize native environment/current directory: {error}"
        ));
    }
    let mut parser = match Parser::new(variables, CancelBehavior::Return) {
        Ok(parser) => parser,
        Err(error) => return diagnostic(&format!("unable to initialize native parser: {error}")),
    };
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
    let mut profile = match profile_output {
        Some(path) => match fish::path::native_resolve_path_for_environment(&path, parser.vars())
            .and_then(File::create)
        {
            Ok(file) => Some(file),
            Err(error) => {
                return diagnostic(&format!("unable to open profile output '{path}': {error}"));
            }
        },
        None => None,
    };
    PROFILING_ACTIVE.store(profile.is_some());
    if no_execute {
        fish::proc::mark_no_exec();
    }
    let io = IoChain::new();
    let mut status = 0;
    if !commands.is_empty() {
        parser.set_var(
            L!("argv"),
            ParserEnvSetMode::new(EnvMode::GLOBAL),
            operands.iter().map(|arg| (*arg).to_owned()).collect(),
        );
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
            if parser.libdata().exit_current_script {
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
                Err(error) => status = diagnostic(&format!("unable to read input: {error}")),
            }
        } else {
            status = match fish::reader::reader_read(&mut parser, &mut input, &io) {
                Ok(()) => parser.last_status(),
                Err(_) => 127,
            };
        }
    }
    if let Some(file) = profile.as_mut() {
        if let Err(error) = parser.write_profiling(file) {
            let _ = diagnostic(&format!("unable to write profile output: {error}"));
        }
    }
    if let Some(error) = fish::flog::take_flog_error() {
        let _ = diagnostic(&format!("unable to write debug output: {error}"));
    }
    // Diagnostic sink failures are reported separately and never replace command status.
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
