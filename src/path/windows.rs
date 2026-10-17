//! Native batch paths: only ordinary local, absolute drive paths are supported.
use crate::env::Environment;
use crate::prelude::*;
use fish_widestring::{native_to_shell_text_owned, osstr2wcstring, shell_text_to_osstring};
use std::{io, path::PathBuf};

/// Validate canonical fish text before handing an identity to a Windows API.
pub fn native_absolute_path(path: &wstr) -> io::Result<PathBuf> {
    let chars = path.as_char_slice();
    if chars.len() < 3
        || !chars[0].is_ascii_alphabetic()
        || chars[1] != ':'
        || !matches!(chars[2], '/' | '\\')
        || chars[3..]
            .iter()
            .any(|c| matches!(c, ':' | '\0' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native paths must be ordinary local absolute drive paths",
        ));
    }
    // Reject DOS device aliases and trailing-dot/space aliasing, too. These are not ordinary
    // file identities, even when Windows accepts them under a drive-qualified spelling.
    for component in chars[3..].split(|c| *c == '/' || *c == '\\') {
        let component = wstr::from_char_slice(component);
        if component == "." || component == ".." {
            continue;
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported native path component",
            ));
        }
        let stem = component.split('.').next().unwrap_or(component);
        let mut uppercase = ['\0'; 4];
        if stem.char_count() > uppercase.len() {
            continue;
        }
        for (target, source) in uppercase.iter_mut().zip(stem.chars()) {
            *target = source.to_ascii_uppercase();
        }
        if matches!(
            &uppercase[..stem.char_count()],
            ['C', 'O', 'N']
                | ['P', 'R', 'N']
                | ['A', 'U', 'X']
                | ['N', 'U', 'L']
                | ['C', 'O', 'M', '1'..='9']
                | ['L', 'P', 'T', '1'..='9']
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native device paths are unsupported",
            ));
        }
    }
    Ok(PathBuf::from(shell_text_to_osstring(path)?))
}

/// No PATH/PATHEXT search, script dispatch, or execute-bit emulation.
pub fn native_resolve_executable(command: &wstr) -> io::Result<PathBuf> {
    let path = native_absolute_path(command)?;
    if !path
        .extension()
        .is_some_and(|ext| ext.to_str().is_some_and(|s| s.eq_ignore_ascii_case("exe")))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native commands require an explicit absolute .exe path",
        ));
    }
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native command is not a file",
        ));
    }
    Ok(path)
}

pub fn path_get_path(command: &wstr, _vars: &dyn Environment) -> Option<WString> {
    osstr2wcstring(native_resolve_executable(command).ok()?)
        .ok()
        .map(native_to_shell_text_owned)
}

/// Retain evaluator command-resolution shape without translating Win32 errors to Unix errno.
pub struct GetPathResult {
    pub path: WString,
    pub err: Option<io::Error>,
}

pub fn path_try_get_path(command: &wstr, _vars: &dyn Environment) -> GetPathResult {
    match native_resolve_executable(command) {
        Ok(_) => GetPathResult {
            path: command.to_owned(),
            err: None,
        },
        Err(error) => GetPathResult {
            path: command.to_owned(),
            err: Some(error),
        },
    }
}

/// Normalize separators without stripping the slash from a drive root.
pub fn path_make_canonical(path: &mut WString) {
    for c in path.as_char_slice_mut() {
        if *c == '\\' {
            *c = '/';
        }
    }
    while path.char_count() > 3 && path.ends_with('/') {
        path.pop();
    }
}
