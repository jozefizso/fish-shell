//! Native noninteractive startup. No reader, configuration or persistence services are initialized.
use fish::{
    common::PROGRAM_NAME,
    env::{EnvMode, EnvStack},
    io::{IoChain, NativeOutput},
    parser::{CancelBehavior, Parser, ParserEnvSetMode},
    prelude::*,
};
use fish_widestring::{native_to_shell_text_owned, osstr2wcstring};
use std::io::Write;

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
    let mut arguments = std::env::args_os().skip(1);
    let mut no_config = false;
    let mut command = None;
    while let Some(argument) = arguments.next() {
        let argument = match osstr2wcstring(argument) {
            Ok(argument) => argument,
            Err(error) => return diagnostic(&format!("invalid native argument: {error}")),
        };
        match &argument[..] {
            option if option == L!("-N") || option == L!("--no-config") => no_config = true,
            option if option == L!("-c") || option == L!("--command") => {
                let Some(source) = arguments.next() else {
                    return diagnostic("native Windows batch mode requires a command after -c");
                };
                command = match osstr2wcstring(source) {
                    Ok(source) => Some(native_to_shell_text_owned(source)),
                    Err(error) => return diagnostic(&format!("invalid native command: {error}")),
                };
                break;
            }
            _ => {
                return diagnostic(
                    "unsupported mode; native Windows fish requires -N -c <fish-code> [arguments...]; interactive use is unavailable",
                );
            }
        }
    }
    let Some(command) = command else {
        return diagnostic(
            "unsupported mode; native Windows fish requires -N -c <fish-code> [arguments...]; interactive use is unavailable",
        );
    };
    if !no_config {
        return diagnostic("native Windows batch mode requires -N (--no-config)");
    }
    let script_args: Vec<WString> = match arguments
        .map(|argument| osstr2wcstring(argument).map(native_to_shell_text_owned))
        .collect()
    {
        Ok(arguments) => arguments,
        Err(error) => return diagnostic(&format!("invalid native argument: {error}")),
    };

    // The native runtime has a fixed C numeric locale and does not register POSIX signal handlers.
    // Ctrl-C retains the Windows console's normal process termination behavior.
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
    parser.set_var(
        L!("argv"),
        ParserEnvSetMode::new(EnvMode::GLOBAL),
        script_args,
    );
    let result = parser.eval(&command, &IoChain::new());
    if result.status.is_empty() {
        parser.last_status()
    } else {
        result.status.status_value()
    }
}
