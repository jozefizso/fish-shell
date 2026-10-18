//! Noninteractive source reading, deliberately independent of the terminal editor.
use crate::{
    builtins::{ErrorCode, STATUS_CMD_ERROR},
    fds::BorrowedFdFile,
    io::{IoChain, NativeOutput},
    parser::{BlockType, Parser},
    prelude::*,
};
use fish_widestring::{bytes2wcstring, wcs2bytes};
use std::io::Write;

/// Read the remaining bytes without CRT newline translation, consume one leading UTF-8 BOM,
/// then parse the complete source before evaluating any of it. CRLF is handled by the grammar.
pub fn reader_read(
    parser: &mut Parser,
    input: &mut BorrowedFdFile,
    io: &IoChain,
) -> Result<(), ErrorCode> {
    let _interactive = parser.push_scope(|scope| scope.is_interactive = false);
    let result = read_ni(parser, input, io);
    parser.libdata_mut().exit_current_script = false;
    result
}

pub fn read_ni(
    parser: &mut Parser,
    input: &mut BorrowedFdFile,
    io: &IoChain,
) -> Result<(), ErrorCode> {
    let mut bytes = Vec::new();
    if let Err(error) = input.read_to_end(&mut bytes) {
        if let Ok(mut stderr) = NativeOutput::stderr() {
            let _ = writeln!(stderr, "fish: Unable to read input file: {error}");
        }
        return Err(STATUS_CMD_ERROR);
    }
    let mut source = bytes2wcstring(&bytes);
    drop(bytes);
    if source.chars().next() == Some('\u{feff}') {
        source.remove(0);
    }
    match parser.eval_wstr(source, io, None, BlockType::Top) {
        Ok(_) => Ok(()),
        Err(message) => {
            if let Ok(mut stderr) = NativeOutput::stderr() {
                let _ = stderr.write_all(&wcs2bytes(&message));
            }
            Err(STATUS_CMD_ERROR)
        }
    }
}
