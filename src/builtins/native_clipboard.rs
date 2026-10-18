//! Private clipboard bridge for the shared copy/paste functions.
use super::prelude::*;
use std::io::{self, Read};

pub fn native_clipboard(
    _parser: &mut Parser,
    streams: &mut IoStreams,
    args: &mut [&wstr],
) -> BuiltinResult {
    if args.len() != 2 || ![L!("copy"), L!("paste")].contains(&args[1]) {
        streams
            .err
            .appendln(L!("__fish_clipboard: expected copy|paste"));
        return Err(STATUS_INVALID_ARGS);
    }
    let result = (|| -> io::Result<()> {
        if args[1] == L!("copy") {
            // This is the evaluator's logical stdin (including pipes and
            // redirections), not the process-global standard handle.
            let input = streams.stdin_file.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "standard input is closed")
            })?;
            let limit = crate::clipboard_windows::MAX_INPUT_UTF8_BYTES;
            let mut bytes = Vec::new();
            input.take((limit + 1) as u64).read_to_end(&mut bytes)?;
            if bytes.len() > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "clipboard input is too large",
                ));
            }
            let text = std::str::from_utf8(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            crate::clipboard_windows::copy_text(text)
        } else {
            // Decode/validate completely before emitting any stdout. No appended
            // newline, byte escape decoding, or lossy UTF-16 substitution.
            let text = crate::clipboard_windows::paste_text()?;
            if !streams.out.append_bytes(text.as_bytes()) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "standard output write failed",
                ));
            }
            Ok(())
        }
    })();
    match result {
        Ok(()) => Ok(SUCCESS),
        Err(error) => {
            streams.err.appendln(&fish_widestring::str2wcstring(format!(
                "__fish_clipboard: {}: {error}",
                args[1]
            )));
            Err(STATUS_CMD_ERROR)
        }
    }
}
