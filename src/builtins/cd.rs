// Implementation of the cd builtin.

use super::prelude::*;
use crate::{
    env::Environment as _,
    err_fmt, err_str,
    path::path_apply_cdpath,
    wutil::wreadlink,
};
#[cfg(unix)]
use crate::{env::EnvMode, err_raw, fds::{BEST_O_SEARCH, wopen_dir},
    parser::ParserEnvSetMode, wutil::{normalize_path, wrealpath}};
#[cfg(unix)]
use fish_widestring::str2wcstring;
#[cfg(unix)]
use nix::{errno::Errno, unistd::fchdir};
use std::sync::Arc;

#[cfg(unix)]
type CdError = Errno;
#[cfg(windows)]
type CdError = std::io::Error;

#[derive(Clone, Copy, PartialEq)]
enum CdErrorClass { Missing, NotDirectory, Loop, Denied, Other }

fn error_class(error: &CdError) -> CdErrorClass {
    #[cfg(unix)]
    {
        match *error {
            Errno::ENOENT => CdErrorClass::Missing,
            Errno::ENOTDIR => CdErrorClass::NotDirectory,
            Errno::ELOOP => CdErrorClass::Loop,
            Errno::EACCES | Errno::EPERM => CdErrorClass::Denied,
            _ => CdErrorClass::Other,
        }
    }
    #[cfg(windows)]
    {
        match error.raw_os_error() {
            Some(2 | 3) => CdErrorClass::Missing,
            Some(267) => CdErrorClass::NotDirectory,
            Some(1921) => CdErrorClass::Loop,
            Some(5) => CdErrorClass::Denied,
            _ => match error.kind() {
                std::io::ErrorKind::NotFound => CdErrorClass::Missing,
                std::io::ErrorKind::PermissionDenied => CdErrorClass::Denied,
                _ => CdErrorClass::Other,
            },
        }
    }
}

// Return whether a cd path is relative to the current directory.
fn is_relative_cd_path(path: &wstr) -> bool {
    path == "." || path == ".." || path.starts_with("./") || path.starts_with("../")
        || cfg!(windows) && (path.starts_with(".\\") || path.starts_with("..\\"))
}

// Attempt to chdir to a given directory.
// If deref_symlink is set, also resolve symlinks in the new PWD before setting the variable.
// If successful, update the parser's variables and return a builtin status code of SUCCESS.
// On failure, return an error.
// The fd is stashed in the parser for eventual support for thread-backed subshells
// with distinct CWDs.
#[cfg(unix)]
fn try_chdir(parser: &mut Parser, dir: &wstr, deref_symlink: bool) -> Result<(), Errno> {
    let norm_dir = normalize_path(dir, true);
    // Try opening the directory and invoking fchdir().
    let fd = wopen_dir(&norm_dir, BEST_O_SEARCH)?;
    fchdir(&fd)?;

    // We successfully changed directories.
    // Stash the fd for the cwd in the parser so it stays open.
    // Eventually fish will support distinct CWDs for different parsers in different threads.
    parser.libdata_mut().cwd_fd = Some(Arc::new(fd));

    let mut new_pwd = norm_dir;
    if deref_symlink {
        if let Ok(real_dir) = wrealpath(&new_pwd) {
            new_pwd = real_dir;
        }
    }

    parser.set_var_and_fire(
        L!("PWD"),
        ParserEnvSetMode::new(EnvMode::GLOBAL_EXPORTED),
        vec![new_pwd],
    );
    Ok(())
}

#[cfg(windows)]
fn try_chdir(parser: &mut Parser, dir: &wstr, deref_symlink: bool) -> Result<(), CdError> {
    let cwd = parser.vars().get_pwd_slash();
    // Complete every fallible open/name operation before replacing either authoritative state.
    let directory = Arc::new(crate::path::NativeCwd::open(dir, &cwd, deref_symlink)?);
    if parser.vars().set_native_cwd(directory) != crate::env::EnvStackSetResult::Ok {
        return Err(std::io::Error::other("could not update fish working directory"));
    }
    Ok(())
}

// An error returned when trying to cd to one of many candidate directories.
struct TryCdError {
    // The "best" error.
    error: CdError,
    // If the error is a broken symlink, the path to the symlink and what it was trying to point to.
    broken_symlink: Option<(WString, WString)>,
}

// Attempt to cd to each of the candidate directories in turn.
// If any succeeds, update the parser's environment variables, stash the fd, and return a builtin status code of SUCCESS.
// If all fail, return the "best" error, along with information about a broken symlink if relevant.
fn try_cd_to_dirs(
    parser: &mut Parser,
    dirs: &[WString],
    deref_symlink: bool,
) -> Result<Success, TryCdError> {
    assert!(!dirs.is_empty(), "dirs should not be empty");
    // Because we may have multiple candidate directories, we have to be choosy about
    // which error to report.
    let mut best_errno: Option<CdError> = None;

    // If the error is a broken symlink, keep track of that here.
    // This is a tuple of the path to the symlink, and what the symlink was trying to point to.
    // Note if broken_symlink is set, so is best_errno; however it may not be ENOENT.
    let mut broken_symlink: Option<(WString, WString)> = None;

    for dir in dirs {
        #[cfg(unix)]
        let norm_dir = normalize_path(dir, true);
        #[cfg(windows)]
        let norm_dir = dir.to_owned();
        // Try opening the directory and invoking fchdir().
        // If this succeeds then we're done!
        let res = try_chdir(parser, &norm_dir, deref_symlink);
        let Err(err) = res else {
            return Ok(SUCCESS);
        };

        // We got an error.
        // Some errors we skip and only report if nothing worked.
        // ENOENT in particular is very low priority
        // - if in another directory there was a *file* by the correct name
        // we prefer *that* error because it's more specific
        if error_class(&err) == CdErrorClass::Missing {
            // If this is the first broken symlink, record it.
            if broken_symlink.is_none() {
                if let Some(symlink_dest) = wreadlink(&norm_dir) {
                    broken_symlink = Some((norm_dir, symlink_dest));
                }
            }
            if best_errno.is_none() {
                best_errno = Some(err);
            }
            continue;
        } else if error_class(&err) == CdErrorClass::NotDirectory {
            best_errno = Some(err);
            continue;
        }
        // Hard error - stop the search.
        // This most likely indicates that the user tried to cd into a directory that exists, but the cd failed.
        // We don't keep trying further directories in CDPATH: this directory is probably what the user intended and
        // an error is better than a worse directory choice.
        best_errno = Some(err);
        break;
    }

    // We failed to cd to any of the directories. Return the best error, which must exist.
    let error = best_errno.expect("best_errno should be set");
    Err(TryCdError {
        error,
        broken_symlink,
    })
}

// cd is highlighted specially in src/highlight/highlight.rs - new options also need to be added
// there
const SHORT_OPTIONS: &wstr = L!("hLP");
const LONG_OPTIONS: &[WOption] = &[
    wopt(L!("help"), ArgType::NoArgument, 'h'),
    wopt(L!("no-dereference"), ArgType::NoArgument, 'L'),
    wopt(L!("dereference"), ArgType::NoArgument, 'P'),
];

// The cd builtin. Changes the current directory to the one specified or to $HOME if none is
// specified. The directory can be relative to any directory in the CDPATH variable.
pub fn cd(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    localizable_consts! {
        DIR_DOES_NOT_EXIST
        "The directory '%s' does not exist"
    }

    let Some(&cmd) = args.first() else {
        return Err(STATUS_INVALID_ARGS);
    };

    // Validate arguments.
    let argc = args.len();
    let mut deref_symlink = false;
    let mut w = WGetopter::new(SHORT_OPTIONS, LONG_OPTIONS, args);
    while let Some(opt) = w.next_opt() {
        match opt {
            'L' => deref_symlink = false,
            'P' => deref_symlink = true,
            'h' => {
                builtin_print_help(parser, streams, cmd);
                return if cfg!(windows) { Err(STATUS_CMD_ERROR) } else { Ok(SUCCESS) };
            }
            ';' => {
                builtin_unexpected_argument(parser, streams, cmd, args[w.wopt_index - 1], false);
                return Err(STATUS_INVALID_ARGS);
            }
            '?' => {
                builtin_unknown_option(parser, streams, cmd, args[w.wopt_index - 1], false);
                return Err(STATUS_INVALID_ARGS);
            }
            _ => panic!("unexpected option {}", opt),
        }
    }

    let optind = w.wopt_index;
    let non_option_argc = argc - optind;
    if non_option_argc > 1 {
        err_fmt!(Error::UNEXP_ARG_COUNT, 1, non_option_argc)
            .cmd(cmd)
            .finish(streams);
        return Err(STATUS_INVALID_ARGS);
    }

    let vars = parser.vars();
    let tmpstr;

    let dir_in: &wstr = if non_option_argc == 1 {
        args[optind]
    } else {
        match vars.get_unless_empty(L!("HOME")) {
            Some(v) => {
                tmpstr = v.as_string();
                &tmpstr
            }
            None => {
                err_str!("Could not find home directory")
                    .cmd(cmd)
                    .finish(streams);
                return Err(STATUS_CMD_ERROR);
            }
        }
    };

    // Stop `cd ""` from crashing
    if dir_in.is_empty() {
        let mut err = err_fmt!("Empty directory '%s' does not exist", dir_in).cmd(cmd);
        if !parser.is_interactive() {
            err = err.stacktrace(parser);
        }
        err.finish(streams);
        return Err(STATUS_CMD_ERROR);
    }

    // If given -P, deref PWD before applying any CDPATH.
    let mut pwd = vars.get_pwd_slash();
    #[cfg(windows)]
    if let Some(cwd) = vars.native_cwd() {
        match cwd.logical_path_for_cd() {
            Ok(current_name) => pwd = current_name,
            Err(error) => {
                // A deleted/unnameable cwd can be escaped, but may not redirect a relative open
                // into an unrelated replacement at its old logical spelling.
                let escape = crate::path::native_absolute_path(dir_in).is_ok()
                    || dir_in.starts_with('/') || dir_in.starts_with('\\')
                    || dir_in == ".." || dir_in.starts_with(L!("../")) || dir_in.starts_with(L!("..\\"));
                if !escape {
                    err_fmt!("Could not resolve current directory: %s", error.to_string())
                        .cmd(cmd).finish(streams);
                    return Err(STATUS_CMD_ERROR);
                }
            }
        }
    }
    if deref_symlink {
        #[cfg(unix)]
        let physical_pwd = wrealpath(&pwd);
        #[cfg(windows)]
        let physical_pwd = parser.vars().get_working_directory();
        if let Ok(mut real_pwd) = physical_pwd {
            if !real_pwd.ends_with('/') && !(cfg!(windows) && real_pwd.ends_with('\\')) {
                real_pwd.push(if cfg!(windows) && real_pwd.starts_with(L!("\\\\?\\")) { '\\' } else { '/' });
            }
            pwd = real_pwd;
        }
    }

    // Walk over candidate directories, respecting CDPATH.
    let dirs = path_apply_cdpath(dir_in, &pwd, vars);
    let mut err = match try_cd_to_dirs(parser, &dirs, deref_symlink) {
        Ok(res) => return Ok(res), // Success
        Err(err) => err,
    };

    // cd failed, so try falling back to a "real" PWD as distinct from our virtual one.
    // If $PWD no longer exists (e.g. it was moved), retry against the kernel's
    // actual cwd via realpath("."). We only do this for arguments that explicitly refer to the
    // current directory; absolute paths and bare relative names are reported as
    // ordinary errors. zsh's `cd_try_chdir` has an equivalent fallback.
    if error_class(&err.error) == CdErrorClass::Missing && is_relative_cd_path(dir_in) {
        #[cfg(unix)]
        let physical_pwd = wrealpath(L!("."));
        #[cfg(windows)]
        let physical_pwd = parser.vars().get_working_directory();
        if let Ok(mut real_pwd) = physical_pwd {
            if !real_pwd.ends_with('/') && !(cfg!(windows) && real_pwd.ends_with('\\')) {
                real_pwd.push(if cfg!(windows) && real_pwd.starts_with(L!("\\\\?\\")) { '\\' } else { '/' });
            }
            if real_pwd != pwd {
                let dirs = path_apply_cdpath(dir_in, &real_pwd, parser.vars());
                // If the real PWD is different, then any new errors are likely to be better,
                // so overwrite the old error.
                err = match try_cd_to_dirs(parser, &dirs, deref_symlink) {
                    Ok(res) => return Ok(res), // Success
                    Err(err) => err,
                };
            }
        }
    }

    // Report any error.
    let TryCdError {
        error: errno,
        broken_symlink,
    } = err;

    let mut err = if error_class(&errno) == CdErrorClass::NotDirectory {
        err_fmt!("'%s' is not a directory", dir_in)
    } else if let Some((symlink, symlink_dest)) = &broken_symlink {
        err_fmt!(
            "'%s' is a broken symbolic link to '%s'",
            symlink,
            symlink_dest
        )
    } else if error_class(&errno) == CdErrorClass::Loop {
        err_fmt!("Too many levels of symbolic links: '%s'", dir_in)
    } else if error_class(&errno) == CdErrorClass::Missing {
        err_fmt!(DIR_DOES_NOT_EXIST, dir_in)
    } else if error_class(&errno) == CdErrorClass::Denied {
        err_fmt!("Permission denied: '%s'", dir_in)
    } else {
        #[cfg(unix)]
        err_raw!(str2wcstring(errno.to_string()))
            .cmd(L!("cd"))
            .finish(streams);
        err_fmt!("Could not change directory to '%s': %s", dir_in, errno.to_string())
    };

    if !parser.is_interactive() {
        err = err.stacktrace(parser);
    }
    err.cmd(cmd).finish(streams);

    Err(STATUS_CMD_ERROR)
}
