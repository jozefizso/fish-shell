//! Native state roots. Discovery never creates directories; initialization does so only after
//! every explicit override has been validated. Defaults are machine-local, not roaming.
use crate::env::{EnvMode, EnvStack, Environment};
use crate::prelude::*;
use std::{ffi::c_void, io, path::PathBuf, sync::OnceLock};

#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
const LOCAL_APP_DATA: Guid = Guid {
    a: 0xf1b32785,
    b: 0x6fba,
    c: 0x4fcf,
    d: [0x9d, 0x55, 0x7b, 0x8e, 0x7f, 0x15, 0x70, 0x91],
};
const PROFILE: Guid = Guid {
    a: 0x5e6c858f,
    b: 0x0e22,
    c: 0x4760,
    d: [0x9a, 0xfe, 0xea, 0x33, 0x17, 0xb6, 0x71, 0x73],
};
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        id: *const Guid,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(ptr: *mut c_void);
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetDriveTypeW(root: *const u16) -> u32;
}

fn known_folder(id: &Guid) -> io::Result<PathBuf> {
    let mut ptr = std::ptr::null_mut();
    // KF_FLAG_DONT_VERIFY: read-only discovery, including profiles not yet materialized.
    let hr = unsafe { SHGetKnownFolderPath(id, 0x4000, std::ptr::null_mut(), &mut ptr) };
    if hr < 0 {
        return Err(io::Error::other(format!(
            "known-folder lookup failed (HRESULT {hr:#x})"
        )));
    }
    if ptr.is_null() {
        return Err(io::Error::other("known-folder lookup returned no path"));
    }
    let result = unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = fish_widestring::utf16_to_wcstring(std::slice::from_raw_parts(ptr, len));
        CoTaskMemFree(ptr.cast());
        text
    };
    super::native_absolute_path(&fish_widestring::native_to_shell_text_owned(result?))
}

/// Read-only known-folder discovery, also used by isolated acceptance tests.
pub fn native_known_folder_roots() -> io::Result<(PathBuf, PathBuf)> {
    Ok((known_folder(&LOCAL_APP_DATA)?, known_folder(&PROFILE)?))
}

pub struct ValidatedPath<'a> {
    pub path: &'a wstr,
    pub ok: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirRemoteness {
    Unknown,
    Local,
    Remote,
}
struct NativeRoots {
    config: WString,
    data: WString,
    cache: WString,
}
static ROOTS: OnceLock<NativeRoots> = OnceLock::new();

fn explicit(vars: &dyn Environment, name: &wstr) -> io::Result<Option<PathBuf>> {
    let Some(value) = vars.getf(name, EnvMode::GLOBAL_EXPORTED) else {
        return Ok(None);
    };
    if value.as_list().len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must contain one absolute native path"),
        ));
    }
    super::native_absolute_path(&value.as_string())
        .map(Some)
        .map_err(|e| io::Error::new(e.kind(), format!("invalid {name}: {e}")))
}

/// Resolve the native state policy without creating directories or importing a
/// profile. Initialization and read-only diagnostics share this exact selection.
pub fn native_state_paths(vars: &dyn Environment) -> io::Result<[PathBuf; 3]> {
    let home = explicit(vars, L!("HOME"))?;
    let config = explicit(vars, L!("XDG_CONFIG_HOME"))?;
    let data = explicit(vars, L!("XDG_DATA_HOME"))?;
    let cache = explicit(vars, L!("XDG_CACHE_HOME"))?;
    let local = if home.is_none() && (config.is_none() || data.is_none() || cache.is_none()) {
        Some(known_folder(&LOCAL_APP_DATA)?)
    } else {
        None
    };
    let choose = |xdg: Option<PathBuf>, unix: &str, native: &str| -> PathBuf {
        if let Some(root) = xdg {
            root.join("fish")
        } else if let Some(home) = &home {
            let mut root = home.clone();
            root.extend(unix.split('/'));
            root.join("fish")
        } else {
            local.as_ref().unwrap().join("fish").join(native)
        }
    };
    Ok([
        choose(config, ".config", "config"),
        choose(data, ".local/share", "data"),
        choose(cache, ".cache", "cache"),
    ])
}

pub fn initialize_native_paths(vars: &EnvStack) -> io::Result<()> {
    if ROOTS.get().is_some() {
        return Ok(());
    }
    let paths = native_state_paths(vars)?;
    let texts = paths
        .iter()
        .map(|p| super::native_path_text(p))
        .collect::<io::Result<Vec<_>>>()?;
    for path in &paths {
        std::fs::create_dir_all(path)?;
    }
    if vars.getf(L!("HOME"), EnvMode::GLOBAL_EXPORTED).is_none() {
        let profile = super::native_path_text(&known_folder(&PROFILE)?)?;
        vars.set_one(
            L!("HOME"),
            crate::env::EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED),
            profile,
        );
    }
    let mut texts = texts.into_iter();
    let roots = NativeRoots {
        config: texts.next().unwrap(),
        data: texts.next().unwrap(),
        cache: texts.next().unwrap(),
    };
    ROOTS
        .set(roots)
        .map_err(|_| io::Error::other("native roots initialized concurrently"))
}
fn validated(path: Option<&'static WString>) -> ValidatedPath<'static> {
    match path {
        Some(path) => ValidatedPath { path, ok: true },
        None => ValidatedPath {
            path: L!(""),
            ok: false,
        },
    }
}
pub fn path_get_config() -> ValidatedPath<'static> {
    validated(ROOTS.get().map(|r| &r.config))
}
pub fn path_get_data() -> ValidatedPath<'static> {
    validated(ROOTS.get().map(|r| &r.data))
}
pub fn path_get_cache() -> ValidatedPath<'static> {
    validated(ROOTS.get().map(|r| &r.cache))
}
pub fn path_remoteness(path: &wstr) -> DirRemoteness {
    use std::os::windows::ffi::OsStrExt;
    let Ok(path) = super::native_absolute_path(path) else {
        return DirRemoteness::Unknown;
    };
    let mut components = path.components();
    let Some(std::path::Component::Prefix(prefix)) = components.next() else {
        return DirRemoteness::Unknown;
    };
    let mut root = prefix.as_os_str().to_os_string();
    root.push("\\");
    let wide: Vec<u16> = root.encode_wide().chain(Some(0)).collect();
    match unsafe { GetDriveTypeW(wide.as_ptr()) } {
        4 => DirRemoteness::Remote,
        2 | 3 | 5 | 6 => DirRemoteness::Local,
        _ => DirRemoteness::Unknown,
    }
}
pub fn path_get_data_remoteness() -> DirRemoteness {
    path_remoteness(path_get_data().path)
}
