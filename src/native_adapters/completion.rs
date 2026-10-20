//! The supported protocol is CPython 3.10–3.14 + argcomplete 3.x, a literal
//! Python script with PYTHON_ARGCOMPLETE_OK, and synchronous CRT output fds 8/9.
//! Console-script launchers, generic Unix fd tables and socket variants are not
//! interpreted or retried through another provider.
use super::*;
use crate::io::NativeCapture;
use std::fs::OpenOptions;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

const LIMIT: usize = 1024 * 1024;

#[derive(Debug)]
struct CompletionExit { raw: u32, diagnostic: String }
impl std::fmt::Display for CompletionExit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}
impl std::error::Error for CompletionExit {}

/// Preserve actual child exits separately from transport/policy/cancellation.
pub fn native_completion_exit(error: &io::Error) -> Option<u32> {
    error.get_ref()?.downcast_ref::<CompletionExit>().map(|exit| exit.raw)
}
const PREFLIGHT: &str = r#"import sys
if sys.platform != 'win32' or sys.implementation.name != 'cpython' or not ((3,10) <= sys.version_info[:2] <= (3,14)):
    raise SystemExit('fish argcomplete requires native CPython 3.10 through 3.14')
import importlib.metadata
try:
    version = importlib.metadata.version('argcomplete')
    major = int(version.split('.')[0])
    import argcomplete
except Exception as error:
    raise SystemExit('fish argcomplete requires installed argcomplete 3.x: ' + str(error))
if major != 3:
    raise SystemExit('fish argcomplete requires argcomplete 3.x; found ' + version)
sys.stdout.buffer.write(b'FISH_CPYTHON_ARGCOMPLETE_V1\n')
"#;

const BOOTSTRAP: &str = r#"import os, sys, msvcrt, runpy
if sys.platform != 'win32' or sys.implementation.name != 'cpython' or not ((3,10) <= sys.version_info[:2] <= (3,14)):
    raise SystemExit('unsupported fish argcomplete interpreter')
wire = os.environ.pop('FISH_INTERNAL_ARGCOMPLETE_FDS', '').split(';')
if len(wire) != 3 or wire[0] != 'v1' or any(not value.isascii() or not value.isdecimal() for value in wire[1:]):
    raise SystemExit('invalid fish argcomplete endpoint negotiation')
handles = [int(value) for value in wire[1:]]
if min(handles) <= 0 or len(set(handles)) != 2:
    raise SystemExit('invalid fish argcomplete endpoint identity')
# Clear HANDLE inheritance before creating CRT entries. dup2 is specifically
# CPython's CRT operation, not the shell pretending Windows HANDLEs are fds.
for handle in handles:
    os.set_handle_inheritable(handle, False)
opened = []
saved = []
reserved = []
candidate_stream = None
try:
    for handle in handles:
        opened.append(msvcrt.open_osfhandle(handle, os.O_WRONLY | os.O_BINARY))
    # os.dup chooses the lowest free CRT slot. A saved source can itself land
    # on 8/9, so keep those allocations reserved until BOTH sources are safe.
    # Never assign either target while the other source can still occupy it.
    for fd in opened:
        while True:
            source = os.dup(fd)
            os.set_inheritable(source, False)
            if source not in (8,9):
                saved.append(source)
                break
            reserved.append(source)
    for descriptors in (opened, reserved):
        for fd in descriptors:
            os.close(fd)
        descriptors.clear()
    for source, target in zip(saved, (8,9)):
        os.dup2(source, target, inheritable=False)
    for fd in saved:
        os.close(fd)
    saved.clear()
    os.set_inheritable(8, False)
    os.set_inheritable(9, False)
    os.write(9, b'FISH_ARGCOMPLETE_FD89_V1\n')
    script = sys.argv[1]
    sys.argv = sys.argv[1:]
    # Load the declared installed dependency under -I before enabling the real
    # application's local imports; a cwd/script-directory shadow is not it.
    import argcomplete.finders, argcomplete.completers
    # CPython text streams otherwise translate LF to CRLF on Windows. The
    # negotiated candidate transport is UTF-8/LF regardless of console locale.
    candidate_stream = open(8, 'w', encoding='utf-8', newline='\n', closefd=False)
    finder_call = argcomplete.finders.CompletionFinder.__call__
    def native_finder_call(self, *args, **kwargs):
        if len(args) > 3:
            args = list(args)
            if args[3] is None: args[3] = candidate_stream
        elif kwargs.get('output_stream') is None:
            kwargs['output_stream'] = candidate_stream
        return finder_call(self, *args, **kwargs)
    argcomplete.finders.CompletionFinder.__call__ = native_finder_call
    sys.path.insert(0, os.path.dirname(script))
    # The native wire contract uses UTF-8 byte offsets. argcomplete's Python
    # lexer slices Unicode code points, so translate only at that API boundary;
    # the real application's COMP_POINT remains the negotiated byte offset.
    split_line = argcomplete.finders.split_line
    def native_split_line(line, point=None):
        if point is None: return split_line(line)
        encoded = line.encode('utf-8')
        if point < 0 or point > len(encoded):
            raise ValueError('invalid native COMP_POINT byte offset')
        return split_line(line, len(encoded[:point].decode('utf-8')))
    argcomplete.finders.split_line = native_split_line
    # The 3.x FilesCompleter otherwise starts bash/compgen. This deliberate
    # child-library adapter enumerates literal native names with CPython's
    # Windows filesystem APIs; it never runs a Unix utility or shell string.
    def native_files(self, prefix, **kwargs):
        directory, leaf = os.path.split(os.path.expanduser(prefix))
        display_directory = os.path.dirname(prefix)
        matches = []
        try:
            entries = os.scandir(directory or '.')
        except (FileNotFoundError, NotADirectoryError):
            return matches
        with entries:
            for entry in entries:
                if not entry.name.startswith(leaf):
                    continue
                if entry.name.startswith('.') and not leaf.startswith('.'):
                    continue
                try:
                    is_directory = entry.is_dir()
                except FileNotFoundError:
                    continue
                candidate = os.path.join(display_directory, entry.name)
                if is_directory:
                    if self.directories: matches.append(candidate + '/')
                elif not self.allowednames or any(entry.name.endswith('.' + extension) for extension in self.allowednames):
                    matches.append(candidate)
        return matches
    argcomplete.completers.FilesCompleter.__call__ = native_files
    quote_completions = argcomplete.finders.CompletionFinder.quote_completions
    def native_quote_completions(self, *args, **kwargs):
        completions = quote_completions(self, *args, **kwargs)
        total = max(0, len(completions) - 1)
        for completion in completions:
            if any(character in completion for character in ('\0', '\r', '\n')):
                raise ValueError('argcomplete candidate contains unsupported newline/NUL framing')
            size = len(completion.encode('utf-8'))
            if size > 65536:
                raise ValueError('argcomplete candidate record over 64 KiB')
            total += size
        if total > 1048576:
            raise ValueError('argcomplete output exceeds 1 MiB')
        return completions
    argcomplete.finders.CompletionFinder.quote_completions = native_quote_completions
    # The real application calls the installed argcomplete, including its
    # argparse and filesystem completers. Normal stdout never becomes fd 8.
    runpy.run_path(script, run_name='__main__')
    raise SystemExit('application returned without completing the argcomplete protocol')
finally:
    if candidate_stream is not None:
        try: candidate_stream.close()
        except OSError: pass
    for descriptors in (opened, saved, reserved):
        for fd in descriptors:
            try: os.close(fd)
            except OSError: pass
    for fd in (8,9):
        try: os.close(fd)
        except OSError: pass
"#;

fn clean_environment(environment: &[(OsString, OsString)]) -> io::Result<Vec<(OsString, OsString)>> {
    let mut result = Vec::with_capacity(environment.len());
    for (name, value) in environment {
        if crate::native_fd_bridge::is_private_environment_key(name) { continue; }
        let text = name.to_str().ok_or_else(|| invalid("completion environment name is not strict Unicode"))?;
        if value.to_str().is_none() { return Err(invalid("completion environment value is not strict Unicode")); }
        if text.as_bytes().get(..12).is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"_ARGCOMPLETE"))
            || ["COMP_LINE", "COMP_POINT", "COMP_TYPE", "PYTHONIOENCODING", "PYTHONUTF8"]
                .iter().any(|key| text.eq_ignore_ascii_case(key)) {
            continue;
        }
        result.push((name.clone(), value.clone()));
    }
    Ok(result)
}
fn completion_io() -> io::Result<IoChain> {
    let mut io = IoChain::new();
    for binding in io.child_bindings()? { io.close(binding.fd); }
    // Completion children cannot take the reader's input. Give cooperating
    // Python/subprocess code an actual native EOF handle, not an invalid fd 0.
    io.set_endpoint(0, NativeEndpoint::from_file(File::open(r"\\.\NUL")?, NativeIoDirection::Input), false);
    Ok(io)
}
fn captured(capture: &NativeCapture) -> io::Result<Vec<u8>> {
    let result = capture.finish()?;
    if result.interrupted { return Err(io::ErrorKind::Interrupted.into()); }
    if let Some(error) = result.error { return Err(error); }
    if result.buffer.discarded() { return Err(io::Error::new(io::ErrorKind::FileTooLarge, "argcomplete output exceeds 1 MiB")); }
    Ok(result.buffer.newline_serialized())
}

fn diagnostic_text(result: &io::Result<Vec<u8>>, skip: usize) -> std::borrow::Cow<'_, str> {
    match result {
        Ok(bytes) => String::from_utf8_lossy(&bytes[skip..]),
        Err(error) => std::borrow::Cow::Owned(format!("capture failed: {error}")),
    }
}
fn wait(group: &mut NativeLaunchGroup) -> io::Result<u32> {
    let cancellation = crate::signal::console_control_cancellation();
    match group.wait(cancellation.as_ref()) {
        Ok(status) => Ok(status[0]),
        Err(error) => { group.terminate_and_wait()?; Err(error) }
    }
}
fn pinned_file(path: &Path) -> io::Result<(File, PathBuf, Vec<File>)> {
    let path = crate::path::native_absolute_path(
        &crate::wutil::native_realpath(&crate::path::native_path_text(path)?)?)?;
    let mut ancestors = Vec::new();
    let parent = path.parent().ok_or_else(|| invalid("adapter path has no parent"))?;
    let mut prefixes: Vec<_> = parent.ancestors().collect();
    prefixes.reverse();
    for prefix in prefixes {
        let file = OpenOptions::new().access_mode(0x80).share_mode(1 | 2)
            .custom_flags(0x0020_0000 | 0x0200_0000).open(prefix)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("adapter ancestor changed or is a reparse point"));
        }
        ancestors.push(file);
    }
    let file = OpenOptions::new().read(true).share_mode(1).custom_flags(0x0020_0000).open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err(invalid("argcomplete requires a regular, non-reparse native image/script"));
    }
    Ok((file, path, ancestors))
}

pub fn argcomplete(
    interpreter: &Path, script: &Path, arguments: &[OsString], line: &str, prefix: &str,
    cwd: &Path, environment: &[(OsString, OsString)], runtime: Arc<NativeRuntime>,
    limits: &crate::native_limits::NativeLimits,
) -> io::Result<Vec<u8>> {
    crate::threads::assert_is_main_thread();
    if !line.starts_with(prefix) || line.contains('\0') || prefix.contains('\0') {
        return Err(invalid("argcomplete cursor prefix must be a literal prefix of COMP_LINE"));
    }
    if !interpreter.extension().is_some_and(|extension| extension.as_encoded_bytes().eq_ignore_ascii_case(b"exe"))
        || !script.extension().is_some_and(|extension| extension.as_encoded_bytes().eq_ignore_ascii_case(b"py")) {
        return Err(io::Error::new(io::ErrorKind::Unsupported,
            "native argcomplete requires an explicit CPython .exe and cooperating .py script; generic descriptor/socket/console-launcher variants are unsupported"));
    }
    let (_interpreter_pin, interpreter, _interpreter_ancestors) = pinned_file(interpreter)?;
    let (mut script_pin, script, _script_ancestors) = pinned_file(script)?;
    let mut marker = [0; 1024];
    let bytes = script_pin.read(&mut marker)?;
    if !marker[..bytes].windows(b"PYTHON_ARGCOMPLETE_OK".len()).any(|slice| slice == b"PYTHON_ARGCOMPLETE_OK") {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "Python application has not opted into argcomplete (missing PYTHON_ARGCOMPLETE_OK)"));
    }
    let mut environment = clean_environment(environment)?;
    let out = NativeCapture::new(65536, Arc::clone(&runtime))?;
    let err = NativeCapture::new(65536, Arc::clone(&runtime))?;
    let mut io = completion_io()?;
    io.set_endpoint(1, NativeEndpoint::from_file(out.writer_file()?, NativeIoDirection::Output), false);
    io.set_endpoint(2, NativeEndpoint::from_file(err.writer_file()?, NativeIoDirection::Output), false);
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime))?;
    group.apply_limits(limits)?;
    let argv = [interpreter.as_os_str().to_owned(), "-I".into(), "-c".into(), PREFLIGHT.into()];
    group.spawn(&interpreter, &argv, cwd, &environment, &io, false)?;
    drop(io);
    let status = wait(&mut group)?;
    let diagnostics = captured(&err);
    let proof = captured(&out);
    if status != 0 {
        return Err(io::Error::other(CompletionExit {
            raw: status,
            diagnostic: format!("argcomplete interpreter/dependency preflight failed (0x{status:08X}): {}{}",
                diagnostic_text(&diagnostics, 0), diagnostic_text(&proof, 0)),
        }));
    }
    let diagnostics = diagnostics?;
    if proof? != b"FISH_CPYTHON_ARGCOMPLETE_V1\n" {
        return Err(io::Error::other(format!("argcomplete interpreter/dependency preflight proof is invalid: {}",
            String::from_utf8_lossy(&diagnostics))));
    }
    group.finish()?;
    for (key, value) in [
        ("_ARGCOMPLETE", "1".to_owned()), ("_ARGCOMPLETE_IFS", "\n".to_owned()),
        ("_ARGCOMPLETE_SUPPRESS_SPACE", "1".to_owned()), ("_ARGCOMPLETE_SHELL", "fish".to_owned()),
        ("COMP_LINE", line.to_owned()), ("COMP_POINT", prefix.len().to_string()), ("COMP_TYPE", String::new()),
        ("PYTHONIOENCODING", "utf-8".to_owned()), ("PYTHONUTF8", "1".to_owned()),
    ] {
        environment.push((key.into(), value.into()));
    }
    let completions = NativeCapture::new(LIMIT, Arc::clone(&runtime))?;
    let diagnostics = NativeCapture::new(LIMIT, Arc::clone(&runtime))?;
    let ordinary = NativeCapture::new(LIMIT, Arc::clone(&runtime))?;
    // Only fd9 carries the bootstrap protocol. Installed Python startup code
    // can write fd2 before the bootstrap runs; that is ordinary diagnostics,
    // never an acknowledgement frame.
    let ordinary_error = NativeCapture::new(LIMIT, Arc::clone(&runtime))?;
    let mut io = completion_io()?;
    io.set_endpoint(1, NativeEndpoint::from_file(ordinary.writer_file()?, NativeIoDirection::Output), false);
    io.set_endpoint(2, NativeEndpoint::from_file(ordinary_error.writer_file()?, NativeIoDirection::Output), false);
    let mut argv = vec![interpreter.as_os_str().to_owned(), "-I".into(), "-X".into(), "utf8".into(),
        "-c".into(), BOOTSTRAP.into(), script.as_os_str().to_owned()];
    argv.extend_from_slice(arguments);
    let mut group = NativeLaunchGroup::new(runtime)?;
    group.apply_limits(limits)?;
    group.attach_substitution_inputs(&argv, &environment);
    let fd8 = completions.writer_file()?;
    let fd9 = diagnostics.writer_file()?;
    let plan = group.prepare_argcomplete(&interpreter, &argv, cwd, &environment, &io, fd8.as_handle(), fd9.as_handle())?;
    group.spawn_prepared(plan)?;
    drop((io, fd8, fd9));
    let status = wait(&mut group)?;
    // Finalize every collector even if one fails. An observed nonzero native
    // exit remains authoritative when negotiation or collection also fails.
    let data = captured(&completions);
    let diagnostics = captured(&diagnostics);
    let ordinary = captured(&ordinary);
    let ordinary_error = captured(&ordinary_error);
    const ACK: &[u8] = b"FISH_ARGCOMPLETE_FD89_V1\n";
    let negotiated = diagnostics.as_ref().is_ok_and(|bytes| bytes.starts_with(ACK));
    let skip = if negotiated { ACK.len() } else { 0 };
    if status != 0 {
        let framing = if negotiated { "" } else { "missing or invalid CRT fd 8/9 negotiation; " };
        let collection = data.as_ref().err().map(|error| format!("completion capture failed: {error}; ")).unwrap_or_default();
        return Err(io::Error::other(CompletionExit {
            raw: status,
            diagnostic: format!("argcomplete application failed (0x{status:08X}): {framing}{collection}{}{}{}",
                diagnostic_text(&diagnostics, skip), diagnostic_text(&ordinary_error, 0), diagnostic_text(&ordinary, 0)),
        }));
    }
    let data = data?;
    let diagnostics = diagnostics?;
    let ordinary_error = ordinary_error?;
    let ordinary = ordinary?;
    if !negotiated {
        return Err(io::Error::other(format!("argcomplete child did not acknowledge CRT fd 8/9 negotiation: {}{}{}",
            String::from_utf8_lossy(&diagnostics), String::from_utf8_lossy(&ordinary_error), String::from_utf8_lossy(&ordinary))));
    }
    let text = std::str::from_utf8(&data).map_err(|_| invalid("argcomplete completion payload is not UTF-8"))?;
    if text.contains(['\0', '\r']) || text.split('\n').any(|line| line.len() > 65536) {
        return Err(invalid("argcomplete malformed newline framing (NUL/CR or record over 64 KiB)"));
    }
    // argcomplete writes a separator between records, not a required trailing
    // separator. The final nonempty record is valid; empty output means no matches.
    group.finish()?;
    Ok(data)
}
