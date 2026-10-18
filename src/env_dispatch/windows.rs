//! Native variable effects. Parser callbacks run only after environment locks are released.
#[path = "common.rs"]
pub(crate) mod common;
use crate::env::EnvStack;
use crate::prelude::*;
use std::{
    io,
    sync::atomic::{AtomicU64, Ordering},
};

pub struct VarChangeMilieu {
    pub is_repainting: bool,
    pub global_or_universal: bool,
}

// N6/N7 consume the actual environment together with this invalidation generation.
// No console/editor behavior is fabricated while only the batch reader is active.
static READER_SETTINGS_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn native_reader_settings_generation() -> u64 {
    READER_SETTINGS_GENERATION.load(Ordering::Relaxed)
}

fn reader_setting(key: &wstr) -> bool {
    key.starts_with(L!("fish_color_"))
        || key.starts_with(L!("fish_pager_color_"))
        || [
            L!("TERM"),
            L!("COLORTERM"),
            L!("fish_term256"),
            L!("fish_term24bit"),
            L!("fish_escape_delay_ms"),
            L!("fish_sequence_key_delay_ms"),
            L!("fish_emoji_width"),
            L!("fish_ambiguous_width"),
            L!("LINES"),
            L!("COLUMNS"),
            L!("fish_autosuggestion_enabled"),
            L!("fish_transient_prompt"),
            L!("fish_cursor_selection_mode"),
            L!("fish_cursor_end_mode"),
            L!("fish_complete_path"),
            L!("fish_function_path"),
            L!("fish_history"),
            L!("fish_private_mode"),
        ]
        .contains(&key)
}

fn locale_setting(key: &wstr) -> bool {
    [
        L!("LANG"),
        L!("LANGUAGE"),
        L!("LC_ALL"),
        L!("LC_CTYPE"),
        L!("LC_MESSAGES"),
        L!("LC_NUMERIC"),
        L!("LC_TIME"),
        L!("LOCPATH"),
        L!("TZ"),
    ]
    .contains(&key)
}

pub fn env_dispatch_var_change(_milieu: VarChangeMilieu, key: &wstr, vars: &EnvStack) {
    if key == "fish_read_limit" {
        common::handle_read_limit_change(vars);
    } else if key == "fish_trace" {
        common::handle_fish_trace(vars);
    } else if key == "fish_function_path" {
        crate::function::invalidate_path();
    } else if key == "fish_complete_path" {
        crate::complete::complete_invalidate_path();
    } else if key == "fish_emoji_width" {
        common::handle_emoji_width(vars);
    } else if key == "fish_ambiguous_width" {
        common::handle_change_ambiguous_width(vars);
    }
    if reader_setting(key) {
        READER_SETTINGS_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    // Import and root discovery run before the complete environment is ready.
    // Initial locale/history selection is performed once by env_dispatch_init.
    if !vars.native_state_initialized() {
        return;
    }
    if locale_setting(key) {
        if let Err(errors) = crate::locale::configure_native_locale(vars) {
            for error in errors {
                crate::flog::flog!(warning, error);
            }
        }
    } else if key == "fish_history" || key == "fish_private_mode" {
        crate::reader::reader_change_history(crate::history::history_id(vars));
    } else if key == "fish_use_posix_spawn" {
        crate::flog::flog!(
            warning,
            "$fish_use_posix_spawn has no native Windows effect; launch uses CreateProcessW"
        );
    }
}

pub fn env_dispatch_init(vars: &EnvStack) -> io::Result<()> {
    crate::locale::configure_native_locale(vars)
        .map_err(|errors| io::Error::new(io::ErrorKind::InvalidInput, errors.join("; ")))?;
    common::handle_read_limit_change(vars);
    common::handle_fish_trace(vars);
    common::handle_emoji_width(vars);
    common::handle_change_ambiguous_width(vars);
    crate::function::invalidate_path();
    crate::complete::complete_invalidate_path();
    crate::reader::reader_change_history(crate::history::history_id(vars));
    READER_SETTINGS_GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(())
}
