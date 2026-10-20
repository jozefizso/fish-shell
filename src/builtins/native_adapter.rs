//! Private entrypoints used by the shared psub/argcomplete functions.
use super::prelude::*;
use crate::env::Environment as _;
use fish_widestring::shell_text_to_osstring;
use std::io::{self, Read};

fn text(argument: &wstr) -> io::Result<String> {
    shell_text_to_osstring(argument)?.into_string().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "adapter argument is not strict Unicode")
    })
}

fn complete(
    parser: &mut Parser, streams: &mut IoStreams, line: &str, prefix: &str, argv: &[&wstr],
) -> io::Result<()> {
    if argv.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "native argcomplete requires EXPLICIT-PYTHON.exe APPLICATION.py [ARG ...]"));
    }
    let interpreter = crate::path::native_resolve_path_for_environment(argv[0], parser.vars())?;
    let script = crate::path::native_resolve_path_for_environment(argv[1], parser.vars())?;
    let arguments = argv[2..].iter().map(|value| shell_text_to_osstring(value).map_err(io::Error::from)).collect::<io::Result<Vec<_>>>()?;
    let cwd = crate::path::native_absolute_path(&parser.vars().get_working_directory()?)?;
    let environment = parser.vars().export_native_env()?;
    let runtime = crate::proc::native_runtime(parser)?;
    let limits = parser.libdata().native_limits;
    let data = crate::native_adapters::argcomplete(&interpreter, &script, &arguments, line, prefix, &cwd, &environment, runtime, &limits)?;
    if !streams.out.append_bytes(&data) { return Err(io::ErrorKind::BrokenPipe.into()); }
    Ok(())
}

pub fn native_adapter(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let result = (|| -> io::Result<()> {
        match args.get(1).copied() {
            Some(operation) if operation == L!("psub") => {
                if args.len() != 4 || ![L!("caller"), L!("testing")].contains(&args[2]) {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected psub caller|testing SUFFIX"));
                }
                let testing = args[2] == L!("testing");
                if !parser.scope().is_subshell {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "psub must run in a command substitution"));
                }
                let suffix = text(args[3])?;
                // Reject suffixes before launching a keeper or creating any file.
                crate::fs::validate_substitution_suffix(&suffix)?;
                let caller = parser.scope().caller_id;
                let cwd = crate::path::native_absolute_path(&parser.vars().get_working_directory()?)?;
                let environment = parser.vars().export_native_env()?;
                let runtime = crate::proc::native_runtime(parser)?;
                let input = streams.stdin_file.as_mut().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "psub input is closed"))?;
                let path = crate::native_adapters::create_substitution(input, &suffix, caller, testing, &cwd, &environment, runtime)?;
                if !streams.out.appendln(&crate::path::native_path_text(&path)?) {
                    if testing { crate::native_adapters::cleanup_test_substitution(&path)?; }
                    else { crate::native_adapters::release_caller(caller); }
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "cannot write psub native consumer path"));
                }
                Ok(())
            }
            Some(operation) if operation == L!("psub-test-cleanup") => {
                if args.len() != 3 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected psub-test-cleanup OWNED-PATH")); }
                let path = crate::path::native_resolve_path_for_environment(args[2], parser.vars())?;
                crate::native_adapters::cleanup_test_substitution(&path)
            }
            Some(operation) if operation == L!("argcomplete") => {
                if args.len() < 7 || args[4] != L!("--") {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput,
                        "expected argcomplete COMP_LINE CURSOR_PREFIX -- EXPLICIT-PYTHON.exe APPLICATION.py [ARG ...]; Unix fd/socket variants are unsupported"));
                }
                let line = text(args[2])?;
                let prefix = text(args[3])?;
                complete(parser, streams, &line, &prefix, &args[5..])
            }
            Some(operation) if operation == L!("argcomplete-current") => {
                if args.len() < 5 || args[2] != L!("--") {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput,
                        "expected argcomplete-current -- EXPLICIT-PYTHON.exe APPLICATION.py [ARG ...]"));
                }
                // commandline writes one LF terminator per field. A NUL
                // delimiter preserves empty/multiline text and trailing LFs;
                // command substitution splitting must not compute byte offsets.
                let input = streams.stdin_file.as_mut().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "completion context input is closed"))?;
                let mut wire = Vec::new();
                input.take(262145).read_to_end(&mut wire)?;
                if wire.len() > 262144 {
                    return Err(io::Error::new(io::ErrorKind::FileTooLarge, "completion context exceeds 256 KiB"));
                }
                let malformed = || io::Error::new(io::ErrorKind::InvalidData, "partial or malformed completion context framing");
                let separator = wire.iter().position(|byte| *byte == 0).ok_or_else(malformed)?;
                let (line, prefix) = wire.split_at(separator);
                let prefix = &prefix[1..];
                if prefix.contains(&0) { return Err(malformed()); }
                let line = line.strip_suffix(b"\n").ok_or_else(malformed)?;
                let prefix = prefix.strip_suffix(b"\n").ok_or_else(malformed)?;
                let line = std::str::from_utf8(line)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "completion context is not UTF-8"))?;
                let prefix = std::str::from_utf8(prefix)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "completion cursor prefix is not UTF-8"))?;
                complete(parser, streams, line, prefix, &args[3..])
            }
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput,
                "expected psub|psub-test-cleanup|argcomplete; generic Unix descriptor/socket adapters are unsupported")),
        }
    })();
    match result {
        Ok(()) => Ok(SUCCESS),
        Err(error) => {
            streams.err.appendln(&fish_widestring::str2wcstring(format!("__fish_native_adapter: {error}")));
            Err(crate::native_adapters::native_completion_exit(&error)
                .map(|raw| crate::proc::ProcStatus::native(raw).status_value())
                .unwrap_or(STATUS_CMD_ERROR))
        }
    }
}
