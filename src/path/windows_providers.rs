//! One discovery/launch policy for native Windows images and deliberate script providers.
//!
//! EXE images are passed to CreateProcessW without image sniffing (including invalid images).
//! COM, shebang-only, extensionless and unknown formats are unsupported; there is no retry.
//! Batch is a command-language provider, NOT arbitrary argv: NUL, quote, percent, CR/LF and
//! trailing backslash arguments are rejected. Every accepted word is double quoted; /d /s
//! /v:off disables AutoRun and delayed expansion. Safe quoted ! ^ & | < > ( ) remain literal
//! at this boundary; scripts that reinterpret their own arguments are responsible for that.
//! Batch command lines must be shorter than 8191 UTF-16 units; each inherited environment
//! entry (name + '=' + value + NUL) must fit in 8191 units. Oversize input is rejected.
//! Batch paths/cwd must have an identity-equivalent ordinary local drive spelling; UNC and
//! extended-only names are rejected, never silently mapped to another cwd or alias.
//! PowerShell is the installed ProgramFiles/PowerShell/7/pwsh.exe, native file version 7.3+,
//! major 7 only. No PATH/COMSPEC override, profile, ExecutionPolicy override or fallback exists.
//! Its constant wrapper receives UTF-16/NUL-framed base64 DATA in a private environment value,
//! clears that value, then splats strings. All non-NUL Unicode arguments, including newlines,
//! quotes, empty strings, leading dashes and trailing backslashes, are representable. Script
//! errors stop and exit 1; otherwise explicit exit / final native LASTEXITCODE is preserved,
//! or normal completion exits 0. Native DWORD interpretation remains the process owner's job.
//! No -NonInteractive: N6's ordinary console donation also serves Read-Host.
//! Script and interpreter physical ancestors are locked against rename/reparse edits before
//! the final script identity check. Before resume, sharing leases are duplicated with zero
//! rights and inheritance disabled into the provider itself, surviving parent death only
//! for that provider's lifetime; no temporary script copies or inherited lease allowlist.
use super::{invalid, native_absolute_path, native_path_text};
use crate::prelude::*;
use std::ffi::{OsStr, OsString, c_void};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProvider {
    Executable,
    Batch,
    PowerShell,
    Fish,
}

pub struct NativeCommand {
    pub path: PathBuf,
    pub provider: NativeProvider,
    interpreter: Option<PathBuf>,
    // Sharing denies modification/replacement until the real child has finished reading scripts.
    pins: Vec<File>,
}

pub struct NativeProviderInvocation {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub command_line: Option<Vec<u16>>,
    pub script: Option<PathBuf>,
    pub pins: Vec<File>,
}

/// Recognized suffixes are shared by discovery, completion and predicates.
pub fn native_provider_for_path(path: &Path) -> io::Result<NativeProvider> {
    let extension = path.extension().map(OsStr::as_encoded_bytes).unwrap_or_default();
    if extension.eq_ignore_ascii_case(b"exe") {
        Ok(NativeProvider::Executable)
    } else if extension.eq_ignore_ascii_case(b"cmd") || extension.eq_ignore_ascii_case(b"bat") {
        Ok(NativeProvider::Batch)
    } else if extension.eq_ignore_ascii_case(b"ps1") {
        Ok(NativeProvider::PowerShell)
    } else if extension.eq_ignore_ascii_case(b"fish") {
        Ok(NativeProvider::Fish)
    } else {
        Err(io::Error::new(io::ErrorKind::Unsupported,
            "unsupported Windows command format (supported: .exe, .cmd, .bat, .ps1, .fish; DOS .com and implicit shebang interpretation are not supported)"))
    }
}

pub fn native_strip_provider_suffix(command: &wstr) -> Option<&wstr> {
    for suffix in [L!(".exe"), L!(".cmd"), L!(".bat"), L!(".ps1"), L!(".fish")] {
        if fish_wcstringutil::string_suffixes_string_case_insensitive(suffix, command) {
            return Some(&command[..command.len() - suffix.len()]);
        }
    }
    None
}

/// An ordinary Win32 spelling is usable by a tool only when N1 validation and
/// native file identity prove it denotes the exact same existing object.
/// Never strip extended-only components, map volume names or guess aliases.
pub fn native_tool_path(path: &Path) -> io::Result<PathBuf> {
    let physical = native_absolute_path(&crate::wutil::native_realpath(&native_path_text(path)?)?)?;
    let text = unicode(physical.as_os_str())?;
    let ordinary = if text.get(..8).is_some_and(|prefix| prefix.eq_ignore_ascii_case("\\\\?\\UNC\\")) {
        format!("\\\\{}", &text[8..])
    } else if let Some(rest) = text.strip_prefix("\\\\?\\") {
        if rest.as_bytes().get(1) != Some(&b':') {
            return Err(io::Error::new(io::ErrorKind::Unsupported,
                "tool requires an identity-equivalent ordinary drive or UNC spelling; volume/extended-only names are unsupported"));
        }
        rest.to_owned()
    } else {
        text.to_owned()
    };
    let ordinary = native_absolute_path(&WString::from(ordinary)).map_err(|error| {
        io::Error::new(io::ErrorKind::Unsupported,
            format!("tool path has no valid ordinary Win32 spelling: {error}"))
    })?;
    let identity = crate::wutil::native_file_identity(&native_path_text(&ordinary)?, true)
        .map_err(|error| io::Error::new(io::ErrorKind::Unsupported,
            format!("cannot prove an ordinary tool spelling for this native object: {error}")))?;
    if identity != crate::wutil::native_file_identity(&native_path_text(&physical)?, true)?
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "ordinary tool path would change native file identity"));
    }
    Ok(ordinary)
}

fn pin_readable(path: &Path) -> io::Result<File> {
    let original = crate::wutil::native_open(path, 0x80, true)?;
    let physical = native_absolute_path(&crate::wutil::native_final_path(&original)?)?;
    let file = OpenOptions::new().read(true).share_mode(1).open(&physical)?;
    if crate::wutil::native_identity_for_file(&file)? != crate::wutil::native_identity_for_file(&original)? {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "provider identity changed while pinning"));
    }
    if !file.metadata()?.is_file() {
        return Err(io::Error::from_raw_os_error(5));
    }
    Ok(file)
}

/// Lock the physical chain root-first, denying directory writes (including reparse edits)
/// and deletion. Locking only the leaf does not prevent ancestor rename/substitution.
fn pin_ancestors(path: &Path) -> io::Result<Vec<File>> {
    let mut ancestors: Vec<_> = path.ancestors().skip(1).collect();
    ancestors.reverse();
    let mut pins = Vec::with_capacity(ancestors.len());
    for ancestor in ancestors {
        let directory = OpenOptions::new().access_mode(0x80).share_mode(1)
            .custom_flags(0x0200_0000 | 0x0020_0000).open(ancestor)?;
        let metadata = directory.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                "physical provider ancestor is not a stable non-reparse directory"));
        }
        pins.push(directory);
    }
    Ok(pins)
}

fn check_script_identity(path: &Path, pins: &[File]) -> io::Result<()> {
    if crate::wutil::native_file_identity(&native_path_text(path)?, true)?
        != crate::wutil::native_identity_for_file(&pins[0])?
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "provider script identity changed after classification"));
    }
    Ok(())
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
}
#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(path: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(path: *const u16, handle: u32, size: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(data: *const c_void, subblock: *const u16, value: *mut *mut c_void, len: *mut u32) -> i32;
}
#[repr(C)]
struct FixedFileInfo {
    signature: u32,
    structure_version: u32,
    file_version_ms: u32,
    file_version_ls: u32,
    product_version_ms: u32,
    product_version_ls: u32,
    flags_mask: u32,
    flags: u32,
    os: u32,
    file_type: u32,
    subtype: u32,
    date_ms: u32,
    date_ls: u32,
}

fn system_cmd() -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 260];
    loop {
        let count = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if count == 0 {
            return Err(io::Error::last_os_error());
        }
        if count as usize >= buffer.len() {
            buffer.resize(count as usize + 1, 0);
            continue;
        }
        buffer.truncate(count as usize);
        return Ok(PathBuf::from(OsString::from_wide(&buffer)).join("cmd.exe"));
    }
}

fn check_powershell_version(path: &Path) -> io::Result<()> {
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let size = unsafe { GetFileVersionInfoSizeW(name.as_ptr(), std::ptr::null_mut()) };
    if size == 0 {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "PowerShell provider has no native file-version metadata; installed PowerShell 7.3+ (major 7) is required"));
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(name.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut info = std::ptr::null_mut();
    let mut len = 0;
    if unsafe { VerQueryValueW(data.as_ptr().cast(), [92u16, 0].as_ptr(), &mut info, &mut len) } == 0
        || info.is_null() || len < std::mem::size_of::<FixedFileInfo>() as u32
    {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "PowerShell provider has invalid native version metadata"));
    }
    let info = unsafe { std::ptr::read_unaligned(info.cast::<FixedFileInfo>()) };
    let major = info.file_version_ms >> 16;
    let minor = info.file_version_ms & 0xffff;
    if info.signature != 0xfeef04bd || major != 7 || minor < 3 {
        return Err(io::Error::new(io::ErrorKind::Unsupported,
            format!("PowerShell provider version {major}.{minor} is unsupported; installed PowerShell 7.3+ (major 7) is required")));
    }
    Ok(())
}

/// Canonical classification with availability/access checks, shared by discovery and X_OK.
/// EXE contents deliberately remain unexamined until the native launch boundary.
pub fn native_classify_command(command: &wstr) -> io::Result<NativeCommand> {
    let requested = native_provider_for_path(&native_absolute_path(command)?)?;
    let path = native_absolute_path(&crate::wutil::native_realpath(command)?)?;
    if native_provider_for_path(&path)? != requested {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "canonical command target changes script/native provider"));
    }
    let mut pins = Vec::new();
    if requested == NativeProvider::Executable {
        let file = crate::wutil::native_open(&path, 0x20, true)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::from_raw_os_error(5));
        }
        return Ok(NativeCommand { interpreter: None, path, provider: requested, pins });
    }
    pins.push(pin_readable(&path)?);
    pins.extend(pin_ancestors(&path)?);
    check_script_identity(&path, &pins)?;
    let interpreter = match requested {
        NativeProvider::Batch => {
            let ordinary = batch_ordinary_path(&path)?;
            batch_word(ordinary.as_os_str())?;
            system_cmd()?
        }
        NativeProvider::PowerShell => {
            native_tool_path(&path)?;
            super::windows_config::native_program_files()?.join("PowerShell/7/pwsh.exe")
        }
        NativeProvider::Fish => crate::exec::native_loaded_fish_path()?,
        NativeProvider::Executable => unreachable!(),
    };
    let interpreter = (|| {
        native_absolute_path(&crate::wutil::native_realpath(&native_path_text(&interpreter)?)?)
    })().map_err(|e: io::Error| io::Error::new(e.kind(), format!("{requested:?} script provider unavailable at {}: {e}", interpreter.display())))?;
    pins.extend(pin_ancestors(&interpreter)?);
    pins.push(pin_readable(&interpreter).map_err(|e| io::Error::new(e.kind(), format!("{requested:?} script provider unavailable at {}: {e}", interpreter.display())))?);
    crate::wutil::native_open(&interpreter, 0x20, true)?;
    let interpreter = if requested == NativeProvider::PowerShell {
        check_powershell_version(&interpreter)?;
        native_tool_path(&interpreter)?
    } else {
        interpreter
    };
    Ok(NativeCommand { path, provider: requested, interpreter: Some(interpreter), pins })
}

fn unicode(value: &OsStr) -> io::Result<&str> {
    let value = value.to_str().ok_or_else(|| invalid("invalid native Unicode in provider input"))?;
    if value.contains('\0') {
        return Err(invalid("NUL in provider input"));
    }
    Ok(value)
}

fn batch_word(value: &OsStr) -> io::Result<&str> {
    let value = unicode(value)?;
    if value.contains(['"', '%', '\r', '\n']) || value.ends_with('\\') {
        return Err(invalid("batch literal domain rejects quotes, percent expansion, CR/LF and trailing backslashes; invoke an explicit interpreter to request command-language semantics"));
    }
    Ok(value)
}

/// Remove an extended drive prefix ONLY when N1's ordinary spelling is valid and names the
/// identical object. In particular this never strips trailing dots/spaces or maps UNC cwd.
fn batch_ordinary_path(path: &Path) -> io::Result<PathBuf> {
    let physical = crate::wutil::native_realpath(&native_path_text(path)?)?;
    let physical = native_absolute_path(&physical)?;
    let text = unicode(physical.as_os_str())?;
    let text = text.strip_prefix("\\\\?\\").unwrap_or(text);
    let bytes = text.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'\\' {
        return Err(invalid("batch provider requires a literal local-drive script path and cwd; UNC/volume cwd cannot be preserved by cmd.exe"));
    }
    let ordinary = native_absolute_path(&WString::from(text))?;
    if crate::wutil::native_file_identity(&native_path_text(&ordinary)?, true)?
        != crate::wutil::native_file_identity(&native_path_text(&physical)?, true)?
    {
        return Err(invalid("batch ordinary path would change native identity"));
    }
    Ok(ordinary)
}

const PS_FRAME: &str = "__FISH_NATIVE_POWERSHELL_FRAME";
// Only this constant is executable syntax. The variable contains base64 UTF-16 data, never code.
const PS_WRAPPER: &str = "$ErrorActionPreference='Stop';try{$d=[Text.Encoding]::Unicode.GetString([Convert]::FromBase64String([Environment]::GetEnvironmentVariable('__FISH_NATIVE_POWERSHELL_FRAME','Process')));[Environment]::SetEnvironmentVariable('__FISH_NATIVE_POWERSHELL_FRAME',$null,'Process');$v=$d.Split([char]0);$p=$v[0];[string[]]$a=@();if($v.Length -gt 2){$a=[string[]]$v[1..($v.Length-2)]};Remove-Variable LASTEXITCODE -Scope Global -ErrorAction SilentlyContinue;& $p @a;$ok=$?;if(Test-Path variable:global:LASTEXITCODE){exit $global:LASTEXITCODE};if(-not $ok){exit 1};exit 0}catch{[Console]::Error.WriteLine($_.ToString());exit 1}";

fn base64(bytes: impl IntoIterator<Item = u8>) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut bytes = bytes.into_iter();
    while let Some(a) = bytes.next() {
        let b = bytes.next();
        let c = bytes.next();
        out.push(ALPHABET[(a >> 2) as usize] as char);
        out.push(ALPHABET[(((a & 3) << 4) | (b.unwrap_or(0) >> 4)) as usize] as char);
        out.push(b.map_or('=', |b| ALPHABET[(((b & 15) << 2) | (c.unwrap_or(0) >> 6)) as usize] as char));
        out.push(c.map_or('=', |c| ALPHABET[(c & 63) as usize] as char));
    }
    out
}

pub fn native_plan_command(
    command: NativeCommand,
    argv: Vec<OsString>,
    cwd: PathBuf,
    mut environment: Vec<(OsString, OsString)>,
) -> io::Result<NativeProviderInvocation> {
    if argv.is_empty() {
        return Err(invalid("provider argv must include argv[0]"));
    }
    for value in &argv { unicode(value)?; }
    // Explicit system cmd is a deliberate command-language request, not CRT argv.
    // Never select this policy by basename alone or fall back after a failed launch.
    if command.provider == NativeProvider::Executable
        && command.path.file_name().is_some_and(|name| name.as_encoded_bytes().eq_ignore_ascii_case(b"cmd.exe"))
        && crate::wutil::native_file_identity(&native_path_text(&command.path)?, true)?
            == crate::wutil::native_file_identity(&native_path_text(&system_cmd()?)?, true)?
    {
        let mut pins = pin_ancestors(&command.path)?;
        pins.push(pin_readable(&command.path)?);
        if crate::wutil::native_identity_for_file(pins.last().unwrap())?
            != crate::wutil::native_file_identity(&native_path_text(&system_cmd()?)?, true)?
        {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                "explicit system cmd identity changed while pinning"));
        }
        let executable = native_tool_path(&command.path)?;
        let cwd = batch_ordinary_path(&cwd)?;
        let mut line = format!("\"{}\" /d /s /v:off", unicode(executable.as_os_str())?);
        let mut index = 1;
        while index < argv.len() {
            let word = unicode(&argv[index])?;
            if word.eq_ignore_ascii_case("/c") || word.eq_ignore_ascii_case("/k") {
                if index + 2 != argv.len() {
                    return Err(invalid("explicit system cmd requires exactly one command-language string after /c or /k"));
                }
                line.push(' ');
                line.push_str(word);
                line.push_str(" \"");
                line.push_str(unicode(&argv[index + 1])?);
                line.push('"');
                index = argv.len();
            } else if ["/d", "/s", "/q", "/a", "/u", "/?"].iter().any(|switch| word.eq_ignore_ascii_case(switch)) {
                line.push(' ');
                line.push_str(word);
                index += 1;
            } else if word.eq_ignore_ascii_case("/v:off") {
                index += 1;
            } else {
                return Err(invalid("unsupported explicit system cmd switch; use /c or /k with one intentional command-language string"));
            }
        }
        if line.encode_utf16().count() >= 8191
            || environment.iter().any(|(name, value)| name.encode_wide().count() + value.encode_wide().count() + 2 > 8191)
        {
            return Err(invalid("explicit system cmd input exceeds its 8191 UTF-16 unit domain"));
        }
        let command_line = Some(line.encode_utf16().chain(Some(0)).collect());
        return Ok(NativeProviderInvocation { executable, argv, cwd, environment, command_line, script: None, pins });
    }
    let NativeCommand { path, provider, interpreter, pins } = command;
    if provider == NativeProvider::Executable {
        return Ok(NativeProviderInvocation { executable: path, argv, cwd, environment, command_line: None, script: None, pins });
    }
    check_script_identity(&path, &pins)?;
    let mut invocation = NativeProviderInvocation { executable: interpreter.expect("classified script interpreter"), argv: Vec::new(), cwd, environment: Vec::new(), command_line: None, script: Some(path.clone()), pins };
    match provider {
        NativeProvider::Executable => unreachable!(),
        NativeProvider::Fish => {
            invocation.argv = vec![invocation.executable.clone().into_os_string(), OsString::from("-N"), OsString::from("--"), path.into_os_string()];
            invocation.argv.extend(argv.into_iter().skip(1));
        }
        NativeProvider::Batch => {
            for (name, value) in &environment {
                // Include name, '=', value and NUL; never drop or truncate an entry.
                if name.encode_wide().count() + value.encode_wide().count() + 2 > 8191 {
                    return Err(invalid("batch inherited environment entry exceeds cmd.exe's 8191 UTF-16 unit domain"));
                }
            }
            let script = batch_ordinary_path(&path)?;
            check_script_identity(&script, &invocation.pins)?;
            invocation.cwd = batch_ordinary_path(&invocation.cwd)?;
            let mut line = format!("\"{}\" /d /s /v:off /c \"\"{}\"", unicode(invocation.executable.as_os_str())?, batch_word(script.as_os_str())?);
            for argument in &argv[1..] {
                line.push_str(" \"");
                line.push_str(batch_word(argument)?);
                line.push('"');
            }
            line.push('"');
            let mut line: Vec<u16> = line.encode_utf16().collect();
            // cmd.exe has its own 8191-character limit, smaller than CreateProcessW's.
            if line.len() >= 8191 {
                return Err(invalid("batch command line exceeds cmd.exe's 8191 UTF-16 unit domain"));
            }
            line.push(0);
            invocation.command_line = Some(line);
            invocation.argv.push(invocation.executable.clone().into_os_string());
        }
        NativeProvider::PowerShell => {
            // PowerShell's authorization/module policy treats extended names as
            // UNC on supported releases. Supply only proven ordinary spellings;
            // a refusal remains authoritative and is never retried or bypassed.
            let path = native_tool_path(&path)?;
            invocation.cwd = native_tool_path(&invocation.cwd)?;
            let mut frame = Vec::new();
            for value in std::iter::once(path.as_os_str()).chain(argv[1..].iter().map(OsString::as_os_str)) {
                unicode(value)?;
                frame.extend(value.encode_wide());
                frame.push(0);
            }
            let data = base64(frame.into_iter().flat_map(u16::to_le_bytes));
            if PS_FRAME.len() + 1 + data.len() >= 32767 {
                return Err(invalid("PowerShell provider data frame exceeds the native environment-value limit"));
            }
            environment.retain(|(name, _)| !name.to_str().is_some_and(|name| name.eq_ignore_ascii_case(PS_FRAME)));
            environment.push((OsString::from(PS_FRAME), OsString::from(data)));
            invocation.argv = vec![invocation.executable.clone().into_os_string(), OsString::from("-NoLogo"), OsString::from("-NoProfile"), OsString::from("-EncodedCommand"), OsString::from(base64(PS_WRAPPER.encode_utf16().flat_map(u16::to_le_bytes)))];
        }
    }
    invocation.environment = environment;
    Ok(invocation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_batch_domain() {
        for value in ["", "spaces here", "雪🐟", "!^&|<>()", "-dash", "a\\b"] {
            assert_eq!(batch_word(OsStr::new(value)).unwrap(), value);
        }
        for value in ["a\"b", "%PATH%", "a\r", "a\nb", "tail\\", "\0"] {
            assert!(batch_word(OsStr::new(value)).is_err(), "{value:?}");
        }
    }
    #[test]
    fn data_encoding() {
        for (input, expected) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg==")] {
            assert_eq!(base64(input.bytes()), expected);
        }
    }
}
