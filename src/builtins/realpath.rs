//! Implementation of the realpath builtin.

use super::prelude::*;
use crate::env::Environment as _;
use crate::err_fmt;
use crate::wutil::normalize_path;
#[cfg(unix)]
use crate::{path::path_apply_working_directory, wutil::wrealpath};

#[derive(Default)]
struct Options {
    print_help: bool,
    no_symlinks: bool,
}

const SHORT_OPTIONS: &wstr = L!("+hs");
const LONG_OPTIONS: &[WOption] = &[
    wopt(L!("no-symlinks"), NoArgument, 's'),
    wopt(L!("help"), NoArgument, 'h'),
];

fn parse_options(
    args: &mut [&wstr],
    parser: &Parser,
    streams: &mut IoStreams,
) -> Result<(Options, usize), ErrorCode> {
    let cmd = args[0];

    let mut opts = Options::default();

    let mut w = WGetopter::new(SHORT_OPTIONS, LONG_OPTIONS, args);

    while let Some(c) = w.next_opt() {
        match c {
            's' => opts.no_symlinks = true,
            'h' => opts.print_help = true,
            ':' => {
                builtin_missing_argument(parser, streams, cmd, None, args[w.wopt_index - 1], false);
                return Err(STATUS_INVALID_ARGS);
            }
            ';' => {
                builtin_unexpected_argument(parser, streams, cmd, args[w.wopt_index - 1], false);
                return Err(STATUS_INVALID_ARGS);
            }
            '?' => {
                builtin_unknown_option(parser, streams, cmd, args[w.wopt_index - 1], false);
                return Err(STATUS_INVALID_ARGS);
            }
            _ => panic!("unexpected retval from WGetopter"),
        }
    }

    Ok((opts, w.wopt_index))
}

/// An implementation of the external realpath command. Doesn't support any options.
/// In general scripts shouldn't invoke this directly. They should just use `realpath` which
/// will fallback to this builtin if an external command cannot be found.
pub fn realpath(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let cmd = args[0];
    let (opts, optind) = parse_options(args, parser, streams)?;

    if opts.print_help {
        builtin_print_help(parser, streams, cmd);
        return if cfg!(windows) { Err(STATUS_CMD_ERROR) } else { Ok(SUCCESS) };
    }

    if optind == args.len() {
        builtin_print_help(parser, streams, cmd);
        return Err(STATUS_INVALID_ARGS);
    }

    let mut had_error = false;

    for &arg in &args[optind..] {
        #[cfg(unix)]
        let result = if !opts.no_symlinks {
            wrealpath(arg)
        } else {
            wrealpath(&parser.vars().get_pwd_slash()).map(|realpwd| {
                let absolute_arg = if arg.starts_with(L!("/")) {
                    arg.to_owned()
                } else {
                    path_apply_working_directory(arg, &realpwd)
                };
                normalize_path(&absolute_arg, false)
            })
        };
        #[cfg(windows)]
        let result = (|| -> std::io::Result<WString> {
            let absolute = crate::path::native_resolve_path_for_environment(arg, parser.vars())?;
            let absolute = crate::path::native_path_text(&absolute)?;
            if opts.no_symlinks {
                Ok(normalize_path(&absolute, false))
            } else {
                crate::wutil::native_realpath_with_cwd(&absolute, &absolute, true)
            }
        })();
        match result {
            Ok(real_path) => {
                streams.out.appendln(&real_path);
            }
            Err(error) => {
                err_fmt!("%s: %s", arg, error.to_string())
                    .cmd(cmd)
                    .finish(streams);
                had_error = true;
            }
        }
    }

    if had_error {
        Err(STATUS_CMD_ERROR)
    } else {
        Ok(SUCCESS)
    }
}
