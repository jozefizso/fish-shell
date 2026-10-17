//! Native Windows filesystem names, discovery, and parser-owned working directories.
//!
//! Relative names use the supplied fish cwd, never a temporary process cwd. `C:x` uses that cwd
//! when it is on C:, otherwise C:'s root (the process-global hidden `=C:` variables are not used).
//! A rooted name uses the cwd's drive or UNC share root. Ordinary names reject DOS devices, ADS,
//! and trailing-dot/space aliases; explicit extended drive/UNC/volume names retain literal names,
//! require backslashes, and reject dot components. Other device namespaces are not filesystem paths.
//! PATH is searched in list order, ignoring empty entries; only .exe providers are enabled. There
//! is no discovery cache: environment edits, cwd changes, and replacement take effect immediately.
use crate::env::Environment;
use crate::prelude::*;
use fish_widestring::{native_to_shell_text_owned, osstr2wcstring, shell_text_to_osstring};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn literal(path: &wstr) -> io::Result<String> {
    shell_text_to_osstring(path)?
        .into_string()
        .map_err(|_| invalid("invalid native Unicode"))
}

#[derive(Clone, Copy)]
struct Root {
    end: usize,
    absolute: bool,
    extended: bool,
    drive: Option<u8>,
}

fn separator(c: u8) -> bool {
    matches!(c, b'/' | b'\\')
}

fn root(path: &str) -> io::Result<Root> {
    let bytes = path.as_bytes();
    let extended = path.starts_with("\\\\?\\");
    if extended {
        if path.contains('/') {
            return Err(invalid("extended paths require backslashes"));
        }
        let rest = &path[4..];
        if rest.len() >= 3
            && rest.as_bytes()[0].is_ascii_alphabetic()
            && rest.as_bytes()[1] == b':'
            && rest.as_bytes()[2] == b'\\'
        {
            return Ok(Root {
                end: 7,
                absolute: true,
                extended: true,
                drive: Some(rest.as_bytes()[0]),
            });
        }
        if rest
            .get(..4)
            .is_some_and(|s| s.eq_ignore_ascii_case("UNC\\"))
        {
            return unc_root(path, 8, true);
        }
        if rest.starts_with("Volume{") {
            if let Some(end) = rest.find("}\\") {
                let guid = &rest[7..end];
                if guid.len() == 36
                    && guid.bytes().enumerate().all(|(i, c)| {
                        if [8, 13, 18, 23].contains(&i) {
                            c == b'-'
                        } else {
                            c.is_ascii_hexdigit()
                        }
                    })
                {
                    return Ok(Root {
                        end: 4 + end + 2,
                        absolute: true,
                        extended: true,
                        drive: None,
                    });
                }
            }
        }
        return Err(invalid("unsupported extended filesystem namespace"));
    }
    if bytes.len() >= 2 && separator(bytes[0]) && separator(bytes[1]) {
        if bytes.get(2).is_some_and(|c| matches!(c, b'.' | b'?')) {
            return Err(invalid("device namespaces are not filesystem paths"));
        }
        return unc_root(path, 2, false);
    }
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let absolute = bytes.get(2).is_some_and(|c| separator(*c));
        return Ok(Root {
            end: if absolute { 3 } else { 2 },
            absolute,
            extended: false,
            drive: Some(bytes[0]),
        });
    }
    Ok(Root {
        end: usize::from(bytes.first().is_some_and(|c| separator(*c))),
        absolute: false,
        extended: false,
        drive: None,
    })
}

fn unc_root(path: &str, start: usize, extended: bool) -> io::Result<Root> {
    let mut parts = path[start..].splitn(3, ['/', '\\']);
    let server = parts.next().unwrap_or("");
    let share = parts.next().unwrap_or("");
    if server.is_empty()
        || share.is_empty()
        || [server, share].iter().any(|s| *s == "." || *s == "..")
    {
        return Err(invalid("UNC paths require a server and share"));
    }
    let end = start + server.len() + 1 + share.len();
    Ok(Root {
        end: end + usize::from(path.len() > end),
        absolute: true,
        extended,
        drive: None,
    })
}

fn validate(path: &str, parsed: Root) -> io::Result<()> {
    if path.is_empty() {
        return Err(invalid("empty path"));
    }
    for component in path[parsed.end..].split(['/', '\\']) {
        if component.is_empty() {
            continue;
        }
        if component
            .chars()
            .any(|c| c <= '\u{1f}' || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            return Err(invalid(
                "unsupported native filename or alternate data stream",
            ));
        }
        if component == "." || component == ".." {
            if parsed.extended {
                return Err(invalid("extended paths do not interpret dot components"));
            }
            continue;
        }
        if !parsed.extended {
            if component.ends_with(['.', ' ']) {
                return Err(invalid(
                    "ordinary paths cannot alias trailing dots or spaces",
                ));
            }
            let stem = component.split('.').next().unwrap().trim_end_matches(' ');
            if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
                .iter()
                .any(|s| stem.eq_ignore_ascii_case(s))
                || (stem.len() == 4
                    && ["COM", "LPT"]
                        .iter()
                        .any(|s| stem.as_bytes()[..3].eq_ignore_ascii_case(s.as_bytes()))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
                || ["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"]
                    .iter()
                    .any(|s| stem.eq_ignore_ascii_case(s))
            {
                return Err(invalid("DOS device aliases are not filesystem names"));
            }
        }
    }
    // The server/share are identities too, not a route into the Win32 device namespace.
    let unc_start = if !parsed.extended && parsed.drive.is_none() && parsed.absolute {
        Some(2)
    } else if parsed.extended
        && path
            .get(4..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("UNC\\"))
    {
        Some(8)
    } else {
        None
    };
    if let Some(start) = unc_start {
        for part in path[start..parsed.end]
            .trim_matches(['/', '\\'])
            .split(['/', '\\'])
        {
            if part
                .chars()
                .any(|c| c <= '\u{1f}' || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
                || part.ends_with(['.', ' '])
            {
                return Err(invalid("invalid UNC server or share"));
            }
        }
    }
    Ok(())
}

/// Validate a fully qualified identity without requiring that it exists.
pub fn native_absolute_path(path: &wstr) -> io::Result<PathBuf> {
    let text = literal(path)?;
    let parsed = root(&text)?;
    if !parsed.absolute {
        return Err(invalid("path is not fully qualified"));
    }
    validate(&text, parsed)?;
    Ok(PathBuf::from(text))
}

/// Join against an explicit cwd, preserving ordinary internal link/`..` traversal. Dot syntax
/// against a literal extended cwd is walked physically when stripping its prefix would alias a
/// name; in that case a missing final component is allowed, but intermediate components must exist.
pub fn native_resolve_path(path: &wstr, cwd: &wstr) -> io::Result<PathBuf> {
    let text = literal(path)?;
    let parsed = root(&text)?;
    validate(&text, parsed)?;
    if parsed.absolute {
        return Ok(PathBuf::from(text));
    }
    let cwd = literal(cwd)?;
    let base = root(&cwd)?;
    if !base.absolute {
        return Err(invalid("working directory is not fully qualified"));
    }
    validate(&cwd, base)?;
    let mut result = if let Some(drive) = parsed.drive {
        if base.drive.is_some_and(|c| c.eq_ignore_ascii_case(&drive)) {
            cwd.clone()
        } else {
            format!("{}:\\", char::from(drive))
        }
    } else if parsed.end == 1 {
        cwd[..base.end].to_owned()
    } else {
        cwd.clone()
    };
    let tail = &text[parsed.end..];
    if !tail.is_empty() {
        if !result.ends_with(['/', '\\']) {
            result.push('\\');
        }
        result.push_str(tail);
    }
    // Relative syntax is ordinary syntax even when the cwd was obtained as an extended name.
    // Prefer the ordinary namespace only when validation proves it cannot introduce an alias.
    if base.extended && result.starts_with("\\\\?\\") {
        if let Some(ordinary) = result.strip_prefix("\\\\?\\UNC\\") {
            let ordinary = format!("\\\\{ordinary}");
            if validate(&ordinary, root(&ordinary)?).is_ok() {
                result = ordinary;
            }
        } else if base.drive.is_some() {
            let ordinary = &result[4..];
            if validate(ordinary, root(ordinary)?).is_ok() {
                result = ordinary.to_owned();
            }
        }
        if result.starts_with("\\\\?\\") {
            result = result.replace('/', "\\");
            if tail.split(['/', '\\']).any(|c| c == "." || c == "..") {
                let directory = if parsed.end == 1 && parsed.drive.is_none() {
                    &cwd[..base.end]
                } else {
                    &cwd
                };
                let tail = native_path_text(Path::new(tail))?;
                return native_absolute_path(&crate::wutil::native_walk_relative(
                    Path::new(directory),
                    &tail,
                    true,
                )?);
            }
        }
    }
    Ok(PathBuf::from(result))
}

/// Parser/environment-relative resolution, without requiring a live cwd for absolute operands.
pub fn native_resolve_path_for_environment(
    path: &wstr,
    vars: &dyn Environment,
) -> io::Result<PathBuf> {
    let text = literal(path)?;
    let parsed = root(&text)?;
    validate(&text, parsed)?;
    if parsed.absolute {
        return Ok(PathBuf::from(text));
    }
    native_resolve_path(path, &vars.get_working_directory()?)
}

/// Checked OS identity ingress. Ordinary names use forward slashes; extended names stay literal.
pub fn native_path_text(path: &Path) -> io::Result<WString> {
    let mut text = osstr2wcstring(path)?;
    if !text.starts_with(L!("\\\\?\\")) {
        for c in text.as_char_slice_mut() {
            if *c == '\\' {
                *c = '/';
            }
        }
    }
    Ok(native_to_shell_text_owned(text))
}

/// Root prefix length in canonical fish characters, for shared lexical consumers.
pub fn native_root_length(path: &wstr) -> usize {
    let chars = path.as_char_slice();
    let sep = |c: char| c == '/' || c == '\\';
    let unc = |start: usize| -> usize {
        let Some(server_end) = chars[start..]
            .iter()
            .position(|c| sep(*c))
            .map(|i| start + i)
        else {
            return 0;
        };
        if server_end == start {
            return 0;
        }
        let share_start = server_end + 1;
        if share_start >= chars.len() || sep(chars[share_start]) {
            return 0;
        }
        chars[share_start..]
            .iter()
            .position(|c| sep(*c))
            .map_or(chars.len(), |i| share_start + i + 1)
    };
    if path.starts_with(L!("\\\\?\\")) {
        if path.contains('/') {
            return 0;
        }
        if chars.len() >= 7 && chars[4].is_ascii_alphabetic() && chars[5] == ':' && chars[6] == '\\'
        {
            return 7;
        }
        if chars.len() >= 8
            && chars[4..8]
                .iter()
                .zip(['U', 'N', 'C', '\\'])
                .all(|(a, b)| a.eq_ignore_ascii_case(&b))
        {
            return unc(8);
        }
        if path.starts_with(L!("\\\\?\\Volume{"))
            && chars.len() >= 49
            && chars[47] == '}'
            && chars[48] == '\\'
        {
            return 49;
        }
        return 0;
    }
    if chars.len() >= 2 && sep(chars[0]) && sep(chars[1]) {
        if chars.get(2).is_some_and(|c| *c == '.' || *c == '?') {
            return 0;
        }
        return unc(2);
    }
    if chars.len() >= 2 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
        return if chars.get(2).is_some_and(|c| sep(*c)) {
            3
        } else {
            2
        };
    }
    usize::from(chars.first().is_some_and(|c| sep(*c)))
}

pub fn native_is_explicit_path(path: &wstr) -> bool {
    path.contains('/') || path.contains('\\') || path.as_char_slice().get(1) == Some(&':')
}

pub fn native_has_root(path: &wstr) -> bool {
    native_root_length(path) != 0
}

/// A real open directory retained by one parser, never mislabeled as a POSIX fchdir handle.
#[derive(Debug)]
pub struct NativeCwd {
    directory: File,
    logical: WString,
}

impl NativeCwd {
    pub fn from_process() -> io::Result<Self> {
        let cwd = native_path_text(&std::env::current_dir()?)?;
        Self::open(&cwd, &cwd, false)
    }

    pub fn open(path: &wstr, cwd: &wstr, physical: bool) -> io::Result<Self> {
        let resolved = native_resolve_path(path, cwd)?;
        let mut logical = native_path_text(&resolved)?;
        let original = crate::wutil::native_realpath_with_cwd(&logical, cwd, false)?;
        if !crate::wutil::wstat(&original)?.is_dir() {
            return Err(io::Error::from_raw_os_error(267));
        }
        let canonical = if physical {
            original
        } else {
            logical = crate::wutil::normalize_path(&logical, true);
            crate::wutil::native_realpath_with_cwd(&logical, cwd, false)?
        };
        let directory = crate::wutil::native_open(&native_absolute_path(&canonical)?, 0x20, true)?;
        if !directory.metadata()?.is_dir() {
            return Err(io::Error::from_raw_os_error(267));
        }
        if physical {
            logical = crate::wutil::native_final_path(&directory)?;
        }
        Ok(Self { directory, logical })
    }

    pub fn logical_path(&self) -> &wstr {
        &self.logical
    }
    pub fn physical_path(&self) -> io::Result<WString> {
        crate::wutil::native_final_path(&self.directory)
    }

    /// Retain -L link spelling only while it still identifies this cwd. A rename/replacement must
    /// not redirect relative cd into a new object occupying the stale logical name.
    pub fn logical_path_for_cd(&self) -> io::Result<WString> {
        let identity = crate::wutil::native_identity_for_file(&self.directory)?;
        match crate::wutil::native_file_identity(&self.logical, true) {
            Ok(logical_identity) if logical_identity == identity => Ok(self.logical.clone()),
            _ => self.physical_path(),
        }
    }
}

/// Validate an explicit, already resolved native provider; no associations or script dispatch.
pub fn native_resolve_executable(command: &wstr) -> io::Result<PathBuf> {
    let path = native_absolute_path(command)?;
    if !path
        .extension()
        .is_some_and(|ext| ext.as_encoded_bytes().eq_ignore_ascii_case(b"exe"))
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only native .exe providers are enabled",
        ));
    }
    let canonical = crate::wutil::native_realpath(command)?;
    let path = native_absolute_path(&canonical)?;
    if !path
        .extension()
        .is_some_and(|ext| ext.as_encoded_bytes().eq_ignore_ascii_case(b"exe"))
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "canonical executable target must retain its .exe provider",
        ));
    }
    let file = crate::wutil::native_open(&path, 0x20, true)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::from_raw_os_error(5));
    }
    Ok(path)
}

pub struct GetPathResult {
    pub path: WString,
    pub err: Option<io::Error>,
}

fn provider_candidates(command: &wstr, vars: &dyn Environment) -> io::Result<Vec<WString>> {
    // Validate bytes/NUL before doing any filesystem work, including bare command lookup.
    let command_os = shell_text_to_osstring(command)?;
    if command.is_empty() {
        return Err(io::Error::from_raw_os_error(2));
    }
    let extension = Path::new(&command_os).extension();
    if extension.is_some_and(|s| !s.as_encoded_bytes().eq_ignore_ascii_case(b"exe")) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only native .exe providers are enabled",
        ));
    }
    let mut candidates = Vec::new();
    if native_is_explicit_path(command) {
        candidates.push(command.to_owned());
    } else if let Some(paths) = vars.get(L!("PATH")) {
        for entry in paths.as_list() {
            if entry.is_empty() {
                continue;
            }
            let mut candidate = entry.clone();
            append_path_component(&mut candidate, command);
            candidates.push(candidate);
        }
    }
    if extension.is_none() {
        for candidate in &mut candidates {
            candidate.push_utfstr(L!(".exe"));
        }
    }
    Ok(candidates)
}

fn resolve_candidate(candidate: &wstr, vars: &dyn Environment) -> io::Result<WString> {
    // Resolve in search order, not eagerly: a later malformed/inaccessible PATH entry cannot
    // hide an earlier valid executable or force an unnecessary cwd query.
    let absolute = native_path_text(&native_resolve_path_for_environment(candidate, vars)?)?;
    native_path_text(&native_resolve_executable(&absolute)?)
}

pub fn path_try_get_path(command: &wstr, vars: &dyn Environment) -> GetPathResult {
    let candidates = match provider_candidates(command, vars) {
        Ok(candidates) => candidates,
        Err(error) => {
            return GetPathResult {
                path: command.to_owned(),
                err: Some(error),
            };
        }
    };
    let mut best = GetPathResult {
        path: command.to_owned(),
        err: Some(io::Error::from_raw_os_error(2)),
    };
    for candidate in candidates {
        match resolve_candidate(&candidate, vars) {
            Ok(path) => return GetPathResult { path, err: None },
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound
                    && best
                        .err
                        .as_ref()
                        .is_some_and(|e| e.kind() == io::ErrorKind::NotFound)
                {
                    best = GetPathResult {
                        path: candidate,
                        err: Some(error),
                    };
                }
            }
        }
    }
    best
}

pub fn path_get_path(command: &wstr, vars: &dyn Environment) -> Option<WString> {
    let result = path_try_get_path(command, vars);
    result.err.is_none().then_some(result.path)
}

pub fn path_get_paths(command: &wstr, vars: &dyn Environment) -> Vec<WString> {
    provider_candidates(command, vars)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| resolve_candidate(&p, vars).ok())
        .collect()
}

pub fn append_path_component(path: &mut WString, component: &wstr) {
    if !path.is_empty() && !component.is_empty() && !path.ends_with('/') && !path.ends_with('\\') {
        path.push(if path.starts_with(L!("\\\\?\\")) {
            '\\'
        } else {
            '/'
        });
    }
    path.push_utfstr(component);
}

pub fn path_apply_working_directory(path: &wstr, cwd: &wstr) -> WString {
    if path.is_empty() || cwd.is_empty() {
        return path.to_owned();
    }
    // Infallible lexical callers must not turn a rejected identity into a different accepted one.
    native_resolve_path(path, cwd)
        .and_then(|p| native_path_text(&p))
        .unwrap_or_else(|_| path.to_owned())
}

pub fn path_apply_cdpath(dir: &wstr, cwd: &wstr, vars: &dyn Environment) -> Vec<WString> {
    if native_has_root(dir)
        || dir == "."
        || dir == ".."
        || dir.starts_with(L!("./"))
        || dir.starts_with(L!("../"))
        || dir.starts_with(L!(".\\"))
        || dir.starts_with(L!("..\\"))
    {
        return vec![crate::wutil::path_normalize_for_cd(cwd, dir)];
    }
    let mut bases = vars
        .get(L!("CDPATH"))
        .map(|v| v.as_list().to_vec())
        .unwrap_or_default();
    bases.push(L!(".").to_owned());
    bases
        .into_iter()
        .map(|mut base| {
            crate::expand::expand_tilde(&mut base, vars);
            if base.is_empty() {
                base.push('.');
            }
            let base = path_apply_working_directory(&base, cwd);
            path_apply_working_directory(dir, &base)
        })
        .collect()
}

pub fn path_get_cdpath(dir: &wstr, _cwd: &wstr, vars: &dyn Environment) -> Option<WString> {
    if dir.is_empty() {
        return None;
    }
    let candidates = if native_absolute_path(dir).is_ok() {
        vec![dir.to_owned()]
    } else {
        path_apply_cdpath(dir, &vars.get_working_directory().ok()?, vars)
    };
    candidates
        .into_iter()
        .find(|p| crate::wutil::wstat(p).is_ok_and(|m| m.is_dir()))
}

pub fn path_as_implicit_cd(path: &wstr, cwd: &wstr, vars: &dyn Environment) -> Option<WString> {
    let mut path = path.to_owned();
    crate::expand::expand_tilde(&mut path, vars);
    if native_has_root(&path)
        || path.starts_with(L!("./"))
        || path.starts_with(L!("../"))
        || path.starts_with(L!(".\\"))
        || path.starts_with(L!("..\\"))
        || path.ends_with('/')
        || path.ends_with('\\')
        || path == ".."
    {
        path_get_cdpath(&path, cwd, vars)
    } else {
        None
    }
}

pub fn path_make_canonical(path: &mut WString) {
    if path.starts_with(L!("\\\\?\\")) {
        return;
    }
    let root = native_root_length(path);
    if root == 0
        && path
            .as_char_slice()
            .get(..2)
            .is_some_and(|s| s.iter().all(|c| *c == '/' || *c == '\\'))
    {
        return;
    }
    for c in path.as_char_slice_mut() {
        if *c == '\\' {
            *c = '/';
        }
    }
    let sep = '/';
    let mut i = if root > 0 && path.char_at(root - 1) == sep {
        root - 1
    } else {
        root
    };
    while i + 1 < path.len() {
        if path.char_at(i) == sep && path.char_at(i + 1) == sep {
            path.replace_range(i..i + 1, L!(""));
        } else {
            i += 1;
        }
    }
    while path.len() > root && path.ends_with(sep) {
        path.pop();
    }
    if path.len() == root && path.starts_with(L!("//")) && !path.ends_with('/') {
        path.push('/');
    }
}

pub fn paths_are_equivalent(first: &wstr, second: &wstr) -> bool {
    let mut first = first.to_owned();
    let mut second = second.to_owned();
    path_make_canonical(&mut first);
    path_make_canonical(&mut second);
    first == second
}

pub fn paths_are_same_file(first: &wstr, second: &wstr) -> bool {
    match (
        crate::wutil::native_file_identity(first, true),
        crate::wutil::native_file_identity(second, true),
    ) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

pub fn path_is_valid(path: &wstr, cwd: &wstr) -> bool {
    native_resolve_path(path, cwd)
        .and_then(|p| native_path_text(&p))
        .and_then(|p| crate::wutil::wstat(&p))
        .is_ok()
}
