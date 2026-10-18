//! Variable callbacks for the noninteractive native runtime only.
use crate::env::{DEFAULT_READ_BYTE_LIMIT, EnvStack, Environment, READ_BYTE_LIMIT};
use crate::prelude::*;
use std::sync::atomic::Ordering;

pub struct VarChangeMilieu {
    pub is_repainting: bool,
    pub global_or_universal: bool,
}

pub fn env_dispatch_var_change(_milieu: VarChangeMilieu, key: &wstr, vars: &EnvStack) {
    if key == "fish_read_limit" {
        let limit = vars
            .get_unless_empty(key)
            .and_then(|value| crate::wutil::fish_wcstoul(&value.as_string()).ok())
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(DEFAULT_READ_BYTE_LIMIT);
        READ_BYTE_LIMIT.store(limit, Ordering::Relaxed);
    } else if key == "fish_trace" {
        crate::trace::trace_set_enabled(
            vars.get_unless_empty(key)
                .map(|value| value.as_list().to_vec())
                .unwrap_or_default(),
        );
    } else if key == "fish_function_path" {
        crate::function::invalidate_path();
    }
}
