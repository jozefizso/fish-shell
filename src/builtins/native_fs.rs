//! Narrow native operations required by the public, shared startup and funcsave scripts.
use super::prelude::*;
use std::io;

pub fn native_fs(
    parser: &mut Parser,
    streams: &mut IoStreams,
    args: &mut [&wstr],
) -> BuiltinResult {
    if args.len() != 3 || ![L!("mkdir"), L!("remove"), L!("atomic-write")].contains(&args[1]) {
        streams.err.appendln(L!(
            "__fish_native_fs: expected mkdir|remove|atomic-write PATH"
        ));
        return Err(STATUS_INVALID_ARGS);
    }
    let result = (|| -> io::Result<()> {
        let path = crate::path::native_resolve_path_for_environment(args[2], parser.vars())?;
        match args[1] {
            operation if operation == L!("mkdir") => std::fs::create_dir_all(path),
            operation if operation == L!("remove") => std::fs::remove_file(path),
            operation if operation == L!("atomic-write") => {
                let path = crate::path::native_path_text(&path)?;
                let input = streams.stdin_file.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "standard input is closed")
                })?;
                let input = std::cell::RefCell::new(input);
                crate::fs::rewrite_via_temporary_file(&path, |_, temporary| {
                    io::copy(&mut **input.borrow_mut(), temporary)?;
                    Ok(crate::fs::PotentialUpdate {
                        do_save: true,
                        data: (),
                    })
                })?;
                Ok(())
            }
            _ => unreachable!(),
        }
    })();
    match result {
        Ok(()) => Ok(SUCCESS),
        Err(error) => {
            streams.err.appendln(&fish_widestring::str2wcstring(format!(
                "__fish_native_fs: {} '{}': {error}",
                args[1], args[2]
            )));
            Err(STATUS_CMD_ERROR)
        }
    }
}
