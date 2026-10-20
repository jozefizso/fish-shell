//! Native variable effects. Parser callbacks run only after environment locks are released.
#[path = "common.rs"]
pub(crate) mod common;
use crate::env::{EnvStack, Environment as _};
use crate::input::{update_wait_on_escape_ms, update_wait_on_sequence_key_ms};
use crate::prelude::*;
use crate::reader::{
    reader_schedule_prompt_repaint, reader_set_autosuggestion_enabled, reader_set_transient_prompt,
};
use crate::screen::{IS_DUMB, ONLY_GRAYSCALE, screen_set_midnight_commander_hack};
pub use common::{
    MIDNIGHT_COMMANDER_SID, handle_fish_cursor_end_mode_change,
    handle_fish_cursor_selection_mode_change,
};
use std::io;

pub struct VarChangeMilieu {
    pub is_repainting: bool,
    pub global_or_universal: bool,
}

fn color_support_setting(key: &wstr) -> bool {
    [
        L!("TERM"),
        L!("COLORTERM"),
        L!("TERM_PROGRAM"),
        L!("STY"),
        L!("fish_term256"),
        L!("fish_term24bit"),
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

/// This is color/renderer policy, not evidence of physical output or keyboard support.
/// Console admission and protocol negotiation belong to the installed native session.
fn init_terminal(vars: &EnvStack) {
    let term = vars.get(L!("TERM"));
    let term = term
        .as_ref()
        .and_then(|v| v.as_list().first())
        .map(|v| v.as_utfstr())
        .unwrap_or(L!(""));
    IS_DUMB.store(term == "dumb");
    ONLY_GRAYSCALE.store(term == "ansi-m" || term == "linux-m" || term == "xterm-mono");
    if vars.get(MIDNIGHT_COMMANDER_SID).is_some() {
        screen_set_midnight_commander_hack();
    }
    crate::terminal::set_color_support(crate::text_face::color_support_from_env(vars));
}

pub fn env_dispatch_var_change(milieu: VarChangeMilieu, key: &wstr, vars: &EnvStack) {
    // These effects are also used by isolated environment/formatter consumers. None
    // requires console admission, persistent state, or main-thread registration.
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
    } else if key == "fish_escape_delay_ms" {
        update_wait_on_escape_ms(vars);
    } else if key == "fish_sequence_key_delay_ms" {
        update_wait_on_sequence_key_ms(vars);
    } else if key == "LINES" || key == "COLUMNS" {
        crate::termsize::handle_columns_lines_var_change(vars);
    }
    if color_support_setting(key) || key == MIDNIGHT_COMMANDER_SID {
        init_terminal(vars);
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
        #[cfg(feature = "localize-messages")]
        crate::localization::update_from_env(vars);
    } else if key == "fish_history" || key == "fish_private_mode" {
        // This also updates the batch history context; the shared callback guards
        // its interactive reader access when no owner thread is registered.
        crate::reader::reader_change_history(crate::history::history_id(vars));
    } else if key == "fish_use_posix_spawn" {
        crate::flog::flog!(
            warning,
            "$fish_use_posix_spawn has no native Windows effect; launch uses CreateProcessW"
        );
    }

    // Reader data is main-thread-owned. Library users may initialize or modify
    // environments before registration or from background threads. Future readers
    // initialize these settings from their own environment when pushed.
    if !crate::threads::is_registered_main_thread() {
        return;
    }
    if key == "fish_cursor_selection_mode" {
        handle_fish_cursor_selection_mode_change(vars);
    } else if key == "fish_cursor_end_mode" {
        handle_fish_cursor_end_mode_change(vars);
    } else if key == "fish_autosuggestion_enabled" {
        reader_set_autosuggestion_enabled(vars);
    } else if key == "fish_transient_prompt" {
        reader_set_transient_prompt(vars);
    }

    let suppress_repaint = milieu.is_repainting || !milieu.global_or_universal;
    if !suppress_repaint
        && (color_support_setting(key)
            || key.starts_with(L!("fish_color_"))
            || key.starts_with(L!("fish_pager_color_"))
            || [
                MIDNIGHT_COMMANDER_SID,
                L!("fish_emoji_width"),
                L!("fish_ambiguous_width"),
                L!("LINES"),
                L!("COLUMNS"),
                L!("fish_cursor_selection_mode"),
                L!("fish_cursor_end_mode"),
                L!("fish_transient_prompt"),
            ]
            .contains(&key))
    {
        reader_schedule_prompt_repaint();
    }
}

pub fn env_dispatch_init(vars: &EnvStack) -> io::Result<()> {
    crate::common::init_special_chars_once();
    crate::locale::configure_native_locale(vars)
        .map_err(|errors| io::Error::new(io::ErrorKind::InvalidInput, errors.join("; ")))?;
    #[cfg(feature = "localize-messages")]
    crate::localization::update_from_env(vars);
    common::handle_read_limit_change(vars);
    common::handle_fish_trace(vars);
    common::handle_emoji_width(vars);
    common::handle_change_ambiguous_width(vars);
    update_wait_on_escape_ms(vars);
    update_wait_on_sequence_key_ms(vars);
    init_terminal(vars);
    crate::function::invalidate_path();
    crate::complete::complete_invalidate_path();
    crate::reader::reader_change_history(crate::history::history_id(vars));
    if crate::threads::is_registered_main_thread() {
        handle_fish_cursor_selection_mode_change(vars);
        handle_fish_cursor_end_mode_change(vars);
        reader_set_autosuggestion_enabled(vars);
        reader_set_transient_prompt(vars);
    }
    Ok(())
}
