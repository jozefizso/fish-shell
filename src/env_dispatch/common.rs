//! Portable variable effects used by both native and Unix dispatchers.
use crate::env::{DEFAULT_READ_BYTE_LIMIT, EnvStack, Environment, READ_BYTE_LIMIT};
use crate::flog::flog;
use crate::prelude::*;
use crate::wutil::fish_wcstoi;
use std::sync::atomic::Ordering;

pub const MIDNIGHT_COMMANDER_SID: &wstr = L!("MC_SID");

pub fn handle_fish_cursor_selection_mode_change(vars: &EnvStack) {
    #[cfg(windows)]
    if !crate::threads::is_registered_main_thread() {
        return;
    }
    use crate::reader::CursorSelectionMode;

    let inclusive = vars
        .get(L!("fish_cursor_selection_mode"))
        .as_ref()
        .map(|v| v.as_string())
        .is_some_and(|v| v == "inclusive");
    let mode = if inclusive {
        CursorSelectionMode::Inclusive
    } else {
        CursorSelectionMode::Exclusive
    };

    crate::reader::reader_change_cursor_selection_mode(mode);
}

pub fn handle_fish_cursor_end_mode_change(vars: &EnvStack) {
    #[cfg(windows)]
    if !crate::threads::is_registered_main_thread() {
        return;
    }
    use crate::reader::CursorEndMode;

    let inclusive = vars
        .get(L!("fish_cursor_end_mode"))
        .as_ref()
        .map(|v| v.as_string())
        .is_some_and(|v| v == "inclusive");
    let mode = if inclusive {
        CursorEndMode::Inclusive
    } else {
        CursorEndMode::Exclusive
    };

    crate::reader::reader_change_cursor_end_mode(mode);
}

/// Update the value of [`FISH_EMOJI_WIDTH`](fish_fallback::FISH_EMOJI_WIDTH).
pub(crate) fn handle_emoji_width(vars: &EnvStack) {
    use fish_fallback::FISH_EMOJI_WIDTH;

    if let Some(width_str) = vars.get(L!("fish_emoji_width")) {
        // The only valid values are 1 or 2; we default to 1 if it was an invalid int.
        let new_width = fish_wcstoi(&width_str.as_string()).unwrap_or(1).clamp(1, 2);
        let new_width = usize::try_from(new_width).unwrap_or_default();
        if FISH_EMOJI_WIDTH.swap(new_width, Ordering::Relaxed) != new_width {
            crate::screen::screen_clear_layout_cache();
        }
        flog!(
            term_support,
            "Overriding default fish_emoji_width w/",
            new_width
        );
    } else {
        let width = 2_usize;
        if FISH_EMOJI_WIDTH.swap(width, Ordering::Relaxed) != width {
            crate::screen::screen_clear_layout_cache();
        }
        flog!(term_support, "default emoji width:", width);
    }
}
pub(super) fn handle_change_ambiguous_width(vars: &EnvStack) {
    let new_width = vars
        .get(L!("fish_ambiguous_width"))
        .map(|v| v.as_string())
        // We use the default value of 1 if it was an invalid int.
        .and_then(|fish_ambiguous_width| fish_wcstoi(&fish_ambiguous_width).ok())
        .unwrap_or(1);
    // Clamp in case of negative values.
    let new_width = usize::try_from(new_width).unwrap_or_default();
    if fish_fallback::FISH_AMBIGUOUS_WIDTH.swap(new_width, Ordering::Relaxed) != new_width {
        crate::screen::screen_clear_layout_cache();
    }
}
/// Allow the user to override the limits on how much data the `read` command will process. This is
/// primarily intended for testing, but could also be used directly by users in special situations.
pub(super) fn handle_read_limit_change(vars: &EnvStack) {
    let read_byte_limit = vars
        .get_unless_empty(L!("fish_read_limit"))
        .map(|v| v.as_string())
        .and_then(|v| {
            // We use fish_wcstoul() to support leading/trailing whitespace
            match (crate::wutil::fish_wcstoul(&v).ok())
                // wcstoul() returns a u64 but want a usize. Handle overflow on 32-bit platforms.
                .and_then(|_u64| usize::try_from(_u64).ok())
            {
                Some(v) => Some(v),
                None => {
                    // We intentionally warn here even in non-interactive mode.
                    flog!(warning, "Ignoring invalid $fish_read_limit");
                    None
                }
            }
        });

    match read_byte_limit {
        Some(new_limit) => READ_BYTE_LIMIT.store(new_limit, Ordering::Relaxed),
        None => READ_BYTE_LIMIT.store(DEFAULT_READ_BYTE_LIMIT, Ordering::Relaxed),
    }
}
pub(super) fn handle_fish_trace(vars: &EnvStack) {
    crate::trace::trace_set_enabled(
        vars.get_unless_empty(L!("fish_trace"))
            .map(|var| var.as_list().to_vec())
            .unwrap_or_default(),
    );
}
