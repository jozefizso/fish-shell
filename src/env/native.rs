//! Windows OS boundaries around the shared fish environment stack.
use super::*;
use fish_widestring::{native_to_shell_text_owned, shell_text_to_osstring};
use std::{cmp::Ordering, ffi::OsString, io, os::windows::ffi::OsStrExt, path::Path};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CompareStringOrdinal(left: *const u16, left_len: i32, right: *const u16, right_len: i32, ignore_case: i32) -> i32;
}

fn native_name_cmp(left: &[u16], right: &[u16]) -> Ordering {
    // Both lengths are checked when constructing the export entries. This API uses ordinal
    // Windows case mapping, not the user's locale or Unicode full case folding.
    match unsafe { CompareStringOrdinal(left.as_ptr(), left.len() as i32, right.as_ptr(), right.len() as i32, 1) } {
        1 => Ordering::Less,
        2 => Ordering::Equal,
        3 => Ordering::Greater,
        _ => unreachable!("validated CompareStringOrdinal inputs"),
    }
}

fn is_path_name(name: &wstr) -> bool {
    name.char_count() == 4 && name.chars().map(|c| c.to_ascii_uppercase()).eq("PATH".chars())
}

impl EnvStack {
    /// Initialize only the native batch environment: no passwd, persistent state, terminals,
    /// locale dispatch, input initialization, config discovery or user-directory creation.
    /// Call this on the stack passed to the parser (PWD is per-stack state).
    pub fn initialize_native(&self) -> io::Result<()> {
        self.initialize_native_from(std::env::vars_os(), &std::env::current_dir()?)
    }

    /// Import an explicit native snapshot, also used by isolated environment-service tests.
    pub fn initialize_native_from(
        &self,
        inherited: impl IntoIterator<Item = (OsString, OsString)>,
        cwd: &Path,
    ) -> io::Result<()> {
        let mut pwd = native_to_shell_text_owned(osstr2wcstring(cwd)?);
        crate::path::native_absolute_path(&pwd)?;
        if !std::fs::metadata(cwd)?.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, "native cwd is not a directory"));
        }
        crate::path::path_make_canonical(&mut pwd);
        let mut imported = HashMap::new();
        let mut path_value = None;
        let mut uppercase_path = false;
        for (key, value) in inherited {
            let key = native_to_shell_text_owned(osstr2wcstring(key)?);
            let value = native_to_shell_text_owned(osstr2wcstring(value)?);
            // Windows' hidden per-drive cwd entries are not fish variables. Drive-relative
            // resolution is outside this runtime's path contract.
            if key.starts_with('=') { continue; }
            if key.is_empty() || key.contains('=') {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid native environment key"));
            }
            if is_path_name(&key) {
                if key == "PATH" || (!uppercase_path && path_value.is_none()) {
                    path_value = Some(value);
                }
                uppercase_path |= key == "PATH";
            } else {
                imported.entry(key).or_insert(value);
            }
        }
        if let Some(value) = path_value { imported.insert(L!("PATH").to_owned(), value); }
        // Validate the complete snapshot before making any variable visible.
        for (key, value) in &imported {
            if !is_electric_var(key) && key != "fish_user_paths" {
                self.set_one(key, EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED), value.clone());
            }
        }
        // Erasing this one canonical spelling cannot revive an inherited Path alias.
        let _ = INHERITED_VARS.set(imported);
        let global = EnvSetMode::new_at_early_startup(EnvMode::GLOBAL);
        self.set_one(L!("IFS"), global, L!("\n \t").to_owned());
        self.set_one(L!("version"), global, str2wcstring(crate::BUILD_VERSION));
        self.set_one(L!("FISH_VERSION"), global, str2wcstring(crate::BUILD_VERSION));
        self.set_one(L!("fish_pid"), global, std::process::id().to_wstring());
        self.set_one(L!("PWD"), EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED), pwd);
        Ok(())
    }

    /// Build Command's environment from fish's live exported view, never process-global state.
    /// Canonical PATH wins; all other case-equivalent names are ambiguous and rejected.
    pub fn export_native_env(&self) -> io::Result<Vec<(OsString, OsString)>> {
        let table = self.lock().base.exported_native_table();
        let canonical_path = table.contains_key(L!("PATH"));
        let mut entries = Vec::with_capacity(table.len());
        for (key, value) in table {
            if canonical_path && key != "PATH" && is_path_name(&key) { continue; }
            if key.is_empty() || key.contains('=') {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid native environment key"));
            }
            let key = shell_text_to_osstring(&key)?;
            let value = shell_text_to_osstring(&value.as_string())?;
            let units: Vec<_> = key.encode_wide().collect();
            if units.len() > i32::MAX as usize {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "native environment key is too long"));
            }
            entries.push((key, value, units));
        }
        entries.sort_unstable_by(|a, b| native_name_cmp(&a.2, &b.2));
        for pair in entries.windows(2) {
            if native_name_cmp(&pair[0].2, &pair[1].2) == Ordering::Equal {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                    format!("ambiguous case-equivalent exported names: {:?} and {:?}", pair[0].0, pair[1].0)));
            }
        }
        Ok(entries.into_iter().map(|(key, value, _)| (key, value)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exported(vars: &EnvStack, name: &str) -> Option<OsString> {
        vars.export_native_env().unwrap().into_iter()
            .find_map(|(key, value)| (key == name).then_some(value))
    }

    #[test]
    #[serial_test::serial]
    fn w4_scopes_native_export_selection() {
        let vars = EnvStack::new();
        let key = L!("fish_native_scope_acceptance");
        let global = EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED);
        vars.set_one(key, global, L!("global").to_owned());
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some("global".into()));
        vars.push(false);
        vars.set_one(key, EnvSetMode::new(EnvMode::LOCAL, false), L!("local").to_owned());
        assert_eq!(vars.get(key).unwrap().as_string(), L!("local"));
        assert_eq!(vars.getf(key, EnvMode::GLOBAL).unwrap().as_string(), L!("global"));
        assert_eq!(vars.getf(key, EnvMode::LOCAL).unwrap().as_string(), L!("local"));
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), None);
        vars.set_one(key, EnvSetMode::new(EnvMode::LOCAL_EXPORTED, false), L!("exported local").to_owned());
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some("exported local".into()));
        vars.set_one(key, EnvSetMode::new(EnvMode::UNEXPORT, false), L!("hidden").to_owned());
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), None);
        vars.remove(key, EnvSetMode::new(EnvMode::LOCAL, false));
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some("global".into()));
        vars.set(key, EnvSetMode::new(EnvMode::LOCAL_EXPORTED, false), vec![WString::new()]);
        assert_eq!(vars.get(key).unwrap().as_list(), &[WString::new()]);
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some(OsString::new()));
        vars.set_empty(key, EnvSetMode::new(EnvMode::LOCAL_EXPORTED, false));
        assert!(vars.get(key).unwrap().as_list().is_empty());
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some(OsString::new()));
        vars.pop(false);
        assert_eq!(vars.get(key).unwrap().as_string(), L!("global"));
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), Some("global".into()));
        vars.remove(key, global);
        assert_eq!(exported(&vars, "fish_native_scope_acceptance"), None);
    }

    #[test]
    #[serial_test::serial]
    fn w4_env_native_path_and_name_policy() {
        let vars = EnvStack::new();
        let input = [
            ("Path", "D:\\stale"),
            ("PATH", "C:\\Program Files;D:\\Unicode λ\\工具"),
            ("pAtH", "E:\\also stale"),
            ("LANGUAGE", "en:fr"),
        ].map(|(key, value)| (OsString::from(key), OsString::from(value)));
        vars.initialize_native_from(input, &std::env::current_dir().unwrap()).unwrap();
        assert_eq!(vars.get(L!("PATH")).unwrap().as_list(), &[L!("C:\\Program Files").to_owned(), L!("D:\\Unicode λ\\工具").to_owned()]);
        assert!(vars.get(L!("Path")).is_none());
        assert_eq!(vars.get(L!("LANGUAGE")).unwrap().as_list(), &[L!("en:fr").to_owned()]);
        assert_eq!(exported(&vars, "PATH"), Some("C:\\Program Files;D:\\Unicode λ\\工具".into()));
        let mode = EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED);
        vars.set_one(L!("Path"), mode, L!("E:\\explicit alternate").to_owned());
        let env = vars.export_native_env().unwrap();
        assert_eq!(env.iter().filter(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH")).count(), 1);
        assert_eq!(exported(&vars, "Path"), None);
        vars.remove(L!("Path"), mode);
        vars.remove(L!("PATH"), mode);
        assert!(vars.export_native_env().unwrap().iter().all(|(key, _)| !key.to_string_lossy().eq_ignore_ascii_case("PATH")));
        vars.set_one(L!("fish_native_collision"), mode, L!("one").to_owned());
        vars.set_one(L!("FISH_NATIVE_COLLISION"), mode, L!("two").to_owned());
        assert_eq!(vars.export_native_env().unwrap_err().kind(), io::ErrorKind::InvalidInput);
        vars.remove(L!("fish_native_collision"), mode);
        vars.remove(L!("FISH_NATIVE_COLLISION"), mode);
        vars.remove(L!("LANGUAGE"), mode);
    }
}
