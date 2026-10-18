use super::prelude::*;
#[cfg(unix)]
use crate::{
    builtins::Error, err_fmt, err_raw, err_str, fds::wopen_cloexec, nix::isatty, parser::Block,
    reader::reader_read,
};
#[cfg(unix)]
use fish_common::{FilenameRef, escape};
#[cfg(unix)]
use fish_widestring::str2wcstring;
#[cfg(unix)]
use nix::{fcntl::OFlag, sys::stat::Mode};
#[cfg(unix)]
use std::os::fd::AsRawFd as _;

/// The  source builtin, sometimes called `.`. Evaluates the contents of a file in the current
/// context.
#[cfg(unix)]
pub fn source(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let argc = args.len();

    let opts = HelpOnlyCmdOpts::parse(args, parser, streams)?;
    let cmd = match args.first() {
        Some(&cmd) => cmd,
        None => return Err(STATUS_INVALID_ARGS),
    };

    if opts.print_help {
        return builtin_print_help(parser, streams, cmd);
    }

    // If we open a file, this ensures it stays open until the end of scope.
    let opened_file;

    // The fd that we read from, either from opened_fd or stdin.
    let fd;
    let func_filename;
    let optind = opts.optind;

    if argc == optind || args[optind] == "-" {
        if streams.is_stdin_closed() {
            err_str!(Error::STDIN_CLOSED).cmd(cmd).finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
        // Either a bare `source` which means to implicitly read from stdin or an explicit `-`.
        if argc == optind && isatty(streams.stdin_fd()) {
            // Don't implicitly read from the terminal.
            err_str!("missing filename argument or input redirection")
                .cmd(cmd)
                .finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
        func_filename = FilenameRef::new(L!("-").to_owned());
        fd = streams.stdin_fd();
    } else {
        match wopen_cloexec(args[optind], OFlag::O_RDONLY, Mode::empty()) {
            Ok(file) => {
                opened_file = file;
            }
            Err(err) => {
                let esc = escape(args[optind]);
                err_fmt!("Error encountered while sourcing file '%s':", &esc)
                    .append_to_msg('\n')
                    .append_to_msg(&err_raw!(str2wcstring(err.desc())).cmd(cmd).to_string())
                    .cmd(cmd)
                    .finish(streams);

                return Err(STATUS_CMD_ERROR);
            }
        }

        fd = opened_file.as_raw_fd();

        func_filename = FilenameRef::new(args[optind].to_owned());
    }

    assert!(fd >= 0, "Should have a valid fd");

    let sb = parser.push_block(Block::source_block(func_filename.clone()));
    let _filename_push = parser
        .current_filename
        .scoped_replace(Some(func_filename.clone()));

    // Construct argv for the sourced file from our remaining args.
    // This is slightly subtle. If this is a bare `source` with no args then `argv + optind` already
    // points to the end of argv. Otherwise we want to skip the file name to get to the args if any.
    let remaining_args = &args[optind + if argc == optind { 0 } else { 1 }..];
    let argv_list = remaining_args.iter().map(|&arg| arg.to_owned()).collect();
    parser.vars().set_argv(argv_list, parser.is_repainting());

    let retval = reader_read(parser, fd, streams.io_chain);

    parser.pop_block(sb);

    match retval {
        Ok(_) => BuiltinResult::from_dynamic(parser.last_status()),
        Err(err) => {
            let esc = escape(&func_filename);
            err_fmt!(
                "Error while reading file '%s'",
                if esc == "-" { L!("<stdin>") } else { &esc }
            )
            .cmd(cmd)
            .finish(streams);
            Err(err)
        }
    }
}

#[cfg(windows)]
pub fn source(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    use crate::{
        builtins::Error, err_fmt, err_str, fds::BorrowedFdFile, parser::Block, reader::reader_read,
    };
    use fish_common::FilenameRef;
    use std::sync::Arc;
    let opts = HelpOnlyCmdOpts::parse(args, parser, streams)?;
    let cmd = args[0];
    if opts.print_help {
        return builtin_print_help(parser, streams, cmd);
    }
    let filename = args.get(opts.optind).copied();
    let mut input = if filename.is_none() || filename == Some(L!("-")) {
        let Some(input) = streams.stdin_file.as_ref() else {
            err_str!(Error::STDIN_CLOSED).cmd(cmd).finish(streams);
            return Err(STATUS_CMD_ERROR);
        };
        if filename.is_none() && input.is_console() {
            err_str!("missing filename argument or input redirection")
                .cmd(cmd)
                .finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
        input.clone()
    } else {
        let filename = filename.unwrap();
        let path = match crate::path::native_resolve_path_for_environment(filename, parser.vars()) {
            Ok(path) => path,
            Err(error) => {
                err_fmt!(
                    "Error encountered while sourcing file '%s': %s",
                    filename,
                    error.to_string()
                )
                .cmd(cmd)
                .finish(streams);
                return Err(STATUS_CMD_ERROR);
            }
        };
        match std::fs::File::open(path) {
            Ok(file) => BorrowedFdFile::from_file(Arc::new(file)),
            Err(error) => {
                err_fmt!(
                    "Error encountered while sourcing file '%s': %s",
                    filename,
                    error.to_string()
                )
                .cmd(cmd)
                .finish(streams);
                return Err(STATUS_CMD_ERROR);
            }
        }
    };
    let filename = FilenameRef::new(filename.unwrap_or(L!("-")).to_owned());
    let block = parser.push_block(Block::source_block(filename.clone()));
    let _filename = parser.current_filename.scoped_replace(Some(filename));
    let remaining = opts.optind + usize::from(args.len() > opts.optind);
    parser.vars().set_argv(
        args[remaining..]
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect(),
        parser.is_repainting(),
    );
    let result = reader_read(parser, &mut input, streams.io_chain);
    parser.pop_block(block);
    result?;
    BuiltinResult::from_dynamic(parser.last_status())
}
