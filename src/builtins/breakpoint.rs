use super::prelude::*;
use crate::builtins::Error;
use crate::parser::{Block, BlockType};
#[cfg(unix)]
use crate::reader::reader_read;
use crate::{err_fmt, err_str};
#[cfg(unix)]
use libc::STDIN_FILENO;

/// Implementation of the builtin breakpoint command, used to launch the interactive debugger.
pub fn breakpoint(
    parser: &mut Parser,
    streams: &mut IoStreams,
    argv: &mut [&wstr],
) -> BuiltinResult {
    let cmd = argv[0];
    if argv.len() != 1 {
        err_fmt!(Error::UNEXP_ARG_COUNT, 0, argv.len() - 1)
            .cmd(cmd)
            .finish(streams);
        return Err(STATUS_INVALID_ARGS);
    }

    // The debugger requires an interactive parser and an actual native console.
    if !parser.is_interactive() {
        err_str!("breakpoint requires an interactive context")
            .cmd(cmd)
            .finish(streams);
        return Err(STATUS_CMD_ERROR);
    }

    // Ensure we don't allow creating a breakpoint at an interactive prompt. There may be a simpler
    // or clearer way to do this but this works.
    {
        if parser
            .block_at_index(1)
            .is_none_or(|b| b.typ() == BlockType::Breakpoint)
        {
            err_str!("Command not valid at an interactive prompt")
                .cmd(cmd)
                .finish(streams);
            return Err(STATUS_ILLEGAL_CMD);
        }
    }
    #[cfg(windows)]
    if !streams.is_terminal(0) {
        err_str!("breakpoint requires console input")
            .cmd(cmd)
            .finish(streams);
        return Err(STATUS_CMD_ERROR);
    }
    #[cfg(windows)]
    let editor_scope = match crate::tty_handoff::NativeEditorScope::enter() {
        Ok(scope) => scope,
        Err(error) => {
            err_fmt!("cannot enter debugger: %s", error.to_string())
                .cmd(cmd)
                .finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
    };

    let bpb = parser.push_block(Block::breakpoint_block());
    #[cfg(unix)]
    let result = reader_read(parser, STDIN_FILENO, &streams.io_chain);
    #[cfg(windows)]
    let result = crate::reader::reader_read_interactive(parser);
    parser.pop_block(bpb);
    #[cfg(unix)]
    result?;
    #[cfg(windows)]
    {
        let restore = editor_scope.finish();
        if let Err(error) = result {
            err_fmt!("debugger input failed: %s", error.to_string())
                .cmd(cmd)
                .finish(streams);
            if let Err(error) = restore {
                err_fmt!("restoring debugger console failed: %s", error.to_string())
                    .cmd(cmd)
                    .finish(streams);
            }
            return Err(STATUS_CMD_ERROR);
        }
        if let Err(error) = restore {
            err_fmt!("restoring debugger console failed: %s", error.to_string())
                .cmd(cmd)
                .finish(streams);
            return Err(STATUS_CMD_ERROR);
        }
    }
    BuiltinResult::from_dynamic(parser.last_status())
}
