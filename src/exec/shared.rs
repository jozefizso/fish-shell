//! Platform-independent function scope and separated command-substitution semantics.
use crate::env::EnvMode;
use crate::function::FunctionProperties;
use crate::io::SeparatedBuffer;
use crate::parser::{Block, BlockId, Parser, ParserEnvSetMode};
use crate::prelude::*;
use fish_widestring::bytes2wcstring;

pub(super) fn function_prepare_environment(
    parser: &mut Parser,
    mut argv: Vec<WString>,
    props: &FunctionProperties,
) -> BlockId {
    let mut func_name = WString::new();
    if !argv.is_empty() {
        func_name = argv.remove(0);
    }
    let fb = parser.push_block(Block::function_block(
        func_name,
        argv.clone(),
        props.shadow_scope,
    ));
    let vars = parser.vars();
    let mode = parser.convert_env_set_mode(ParserEnvSetMode::user(EnvMode::LOCAL));
    let mut overwrite_argv = false;
    for (idx, named_arg) in props.named_arguments.iter().enumerate() {
        if named_arg == L!("argv") {
            overwrite_argv = true;
        }
        if idx < argv.len() {
            vars.set_one(named_arg, mode, argv[idx].clone());
        } else {
            vars.set_empty(named_arg, mode);
        }
    }
    for (key, value) in &*props.inherit_vars {
        if key == L!("argv") {
            overwrite_argv = true;
        }
        vars.set(key, mode, value.clone());
    }
    if !overwrite_argv {
        vars.set_argv(argv, mode.is_repainting);
    }
    fb
}

pub(super) fn function_restore_environment(parser: &mut Parser, block: BlockId) {
    parser.pop_block(block);
    parser.libdata_mut().returning = false;
}

pub(super) fn populate_subshell_output(
    list: &mut Vec<WString>,
    buffer: &SeparatedBuffer,
    split: bool,
) {
    for element in buffer.elements() {
        let data = &element.contents;
        if element.is_explicitly_separated() {
            list.push(bytes2wcstring(data));
        } else if split {
            let mut cursor = 0;
            while cursor < data.len() {
                let stop = data[cursor..].iter().position(|byte| *byte == b'\n');
                let hit_separator = stop.is_some();
                let stop = stop.map_or(data.len(), |relative| cursor + relative);
                list.push(bytes2wcstring(&data[cursor..stop]));
                cursor = stop + usize::from(hit_separator);
            }
        } else {
            let trailing_newline = usize::from(data.last() == Some(&b'\n'));
            list.push(bytes2wcstring(&data[..data.len() - trailing_newline]));
        }
    }
}
