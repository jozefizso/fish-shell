//! N8 provider chains under the existing real N6 console/ConPTY owner.
//! No second runner, polling sleeps, synthetic input model, or optional skips.
use super::{
    Family,
    cases_probe::{Event, RetainedProcess},
    console::Session,
    editor::{capture, finish, start},
    fixture::{Fixture, quote},
    tools::pinned_interpreter,
};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const FAMILIES: &[Family] = &[
    ("provider-python-real-input-exclusive-reclaim", python_input),
    ("provider-powershell-real-readline-exclusive-reclaim", powershell_input),
    ("provider-batch-native-tui-mode-protocol-reclaim", batch_tui),
    ("provider-powershell-native-tui-mode-protocol-reclaim", powershell_tui),
    ("provider-batch-launch-and-preflight-failure-reclaim", batch_failure),
    ("provider-batch-ignored-control-retains-input-until-release", batch_cancel),
    ("provider-powershell-ignored-control-retains-input-until-release", powershell_cancel),
];

fn qp(path: &Path) -> String { quote(path.to_str().expect("reversible provider console path")) }
fn ps_word(value: &str) -> String { format!("'{}'", value.replace('\'', "''")) }
fn batch_probe(f: &Fixture) -> &'static str {
    let alias = f.root.join("n8-provider-probe.exe");
    if !alias.exists() { fs::copy(&f.probe, &alias).unwrap(); }
    // Keep batch source ASCII; cmd obtains the real Unicode directory at runtime.
    "\"%~dp0n8-provider-probe.exe\""
}
fn script(f: &Fixture, name: &str, source: &str) -> PathBuf {
    let path = f.root.join(name);
    super::providers::write_provider_source(&path, source);
    path
}
fn modes(bytes: &[u8]) -> (u32, u32) {
    let text = std::str::from_utf8(bytes).unwrap();
    let (input, output) = text.split_once(':').expect("real probe mode pair");
    (input.parse().unwrap(), output.parse().unwrap())
}
fn snapshot_modes(s: &mut Session) -> (u32, u32) {
    let state = s.snapshot();
    (state.input_mode, state.output_mode)
}
fn protocol_off_before(s: &Session, marker: &[u8]) {
    let trace = s.trace();
    let first = trace.windows(marker.len()).position(|part| part == marker)
        .expect("actual child first-read output");
    let off = trace[..first].windows(8).rposition(|part| part == b"\x1b[?2004l")
        .expect("fish paste protocol disabled before provider child input");
    assert!(!trace[off + 8..first].windows(8).any(|part| part == b"\x1b[?2004h"),
        "fish reclaimed its input protocol while the provider child still owned input");
}
fn assert_pinned(path_file: &Path, expected: &Path) {
    let path = fs::read_to_string(path_file).unwrap();
    assert_eq!(fs::canonicalize(path.trim_end_matches(['\r', '\n'])).unwrap(), fs::canonicalize(expected).unwrap());
}
fn assert_native_capability(f: &Fixture) {
    let value = fs::read_to_string(f.root.join("provider-interpreter.pathext")).unwrap();
    assert!(value.split(';').any(|extension| extension.eq_ignore_ascii_case(".EXE")),
        "PowerShell must retain its declared native descendant capability: {value}");
}
fn reclaim(mut s: Session, f: &Fixture, editor: (u32, u32)) {
    s.wait_protocol(true);
    assert_eq!(snapshot_modes(&mut s), editor, "provider exit must restore every editor mode bit");
    assert!(!f.root.join("provider-stole-binding").exists());
    capture(&mut s, f, 1, "");
    s.send(b"after-provider-input");
    capture(&mut s, f, 2, "after-provider-input");
    finish(s);
}
fn interactive(f: &Fixture, native_descendant: bool, extra: &str) -> Session {
    let capability = if native_descendant { "set -gx PATHEXT '.EXE'; " } else { "" };
    start(f, &format!("{capability}set -gx FISH_N8_VALUE 'ordinary exported 雪'; bind ctrl-g 'printf forbidden > provider-stole-binding'; {extra}"))
}
fn python_input(f: &Fixture) {
    let python = pinned_interpreter(f, "PYTHON");
    let source = script(f, "real console input 雪.py", r#"import os,pathlib,sys
assert sys.platform == 'win32'
assert os.environ['FISH_N8_VALUE'] == 'ordinary exported 雪'
pathlib.Path('provider-interpreter').write_text(sys.executable, encoding='utf-8')
first=input('N8-PYTHON-FIRST:')
second=input('N8-PYTHON-SECOND:')
pathlib.Path('provider-input').write_bytes(first.encode('utf-8') + b'\0' + second.encode('utf-8'))
"#);
    let mut s = interactive(f, false, "");
    let editor = snapshot_modes(&mut s);
    s.send(format!("{} -I {}; printf '%s' $status > provider-status\r", qp(&python), qp(&source)).as_bytes());
    s.wait_text(b"N8-PYTHON-FIRST:");
    s.wait_protocol(false);
    protocol_off_before(&s, b"N8-PYTHON-FIRST:");
    assert_pinned(&f.root.join("provider-interpreter"), &python);
    s.send(b"\r");
    s.wait_text(b"N8-PYTHON-SECOND:");
    s.paste("ordinary 雪🐟 & | % !");
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("provider-status")), b"0");
    assert_eq!(fs::read(f.root.join("provider-input")).unwrap(), "\0ordinary 雪🐟 & | % !".as_bytes());
    reclaim(s, f, editor);
}
fn powershell_input(f: &Fixture) {
    let ps = pinned_interpreter(f, "PWSH");
    let source = script(f, "real provider console input 雪.ps1", r#"$ErrorActionPreference='Stop'
if($env:FISH_N8_VALUE -cne 'ordinary exported 雪'){throw 'exported environment'}
[IO.File]::WriteAllText((Join-Path ([Environment]::CurrentDirectory) 'provider-interpreter'),[Environment]::ProcessPath)
$originalInputEncoding=[Console]::InputEncoding
$activeInputEncoding=0
try {
    [Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
    $activeInputEncoding=[Console]::InputEncoding.CodePage
    [Console]::Write('N8-POWERSHELL-FIRST:');[Console]::Out.Flush()
    $first=[Console]::ReadLine()
    [Console]::Write('N8-POWERSHELL-SECOND:');[Console]::Out.Flush()
    $second=[Console]::ReadLine()
    if($null -eq $first -or $null -eq $second){throw 'console input unexpectedly reached EOF'}
    [IO.File]::WriteAllBytes((Join-Path ([Environment]::CurrentDirectory) 'provider-input'),[Text.Encoding]::UTF8.GetBytes($first+[char]0+$second))
} finally {
    [Console]::InputEncoding=$originalInputEncoding
    [IO.File]::WriteAllText((Join-Path ([Environment]::CurrentDirectory) 'provider-input-encoding'),[string]::Join(':',[string[]]@($originalInputEncoding.CodePage,$activeInputEncoding,[Console]::InputEncoding.CodePage)))
}
"#);
    let mut s = interactive(f, false, "");
    let editor = snapshot_modes(&mut s);
    // Deliberate .ps1 provider, not an explicit pwsh launch bypassing that chain.
    s.send(format!("{}; printf '%s' $status > provider-status\r", qp(&source)).as_bytes());
    s.wait_text(b"N8-POWERSHELL-FIRST:");
    s.wait_protocol(false);
    protocol_off_before(&s, b"N8-POWERSHELL-FIRST:");
    assert_pinned(&f.root.join("provider-interpreter"), &ps);
    s.send(b"\r");
    s.wait_text(b"N8-POWERSHELL-SECOND:");
    s.paste("ordinary 雪🐟 & | % !");
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("provider-status")), b"0");
    assert_eq!(fs::read(f.root.join("provider-input")).unwrap(), "\0ordinary 雪🐟 & | % !".as_bytes());
    let encoding = fs::read_to_string(f.root.join("provider-input-encoding")).unwrap();
    let encoding: Vec<u32> = encoding.split(':').map(|value| value.parse().unwrap()).collect();
    assert_eq!(encoding.len(), 3);
    assert_eq!(encoding[1], 65001, "cooperating Console.ReadLine declares UTF-8 input");
    assert_eq!(encoding[0], encoding[2], "the provider must restore its original console input encoding");
    reclaim(s, f, editor);
}
fn wrapped_probe(f: &Fixture, extension: &str, mode: &str, release: &Event, controlled: &Event) -> (PathBuf, Option<PathBuf>) {
    let values = [f.probe.to_str().unwrap(), mode, f.root.to_str().unwrap(), &release.name, &controlled.name];
    match extension {
        "cmd" => {
            let command = format!("{} {} \"%~dp0.\" {} {}", batch_probe(f), mode, quote_batch(&release.name), quote_batch(&controlled.name));
            (script(f, "native console provider.cmd", &format!("@echo off\r\n{command}\r\nexit /b %errorlevel%\r\n")), None)
        }
        "ps1" => {
            let ps = pinned_interpreter(f, "PWSH");
            let record = f.root.join("provider-interpreter");
            let command = values.iter().map(|value| ps_word(value)).collect::<Vec<_>>().join(" ");
            let source = format!("$ErrorActionPreference='Stop'\n[IO.File]::WriteAllText({},[Environment]::ProcessPath)\n[IO.File]::WriteAllText(({}+'.pathext'),$env:PATHEXT)\nif(($env:PATHEXT -split ';') -notcontains '.EXE'){{throw 'native descendant fixture requires declared .EXE PATHEXT capability'}}\n& {command}\nexit $LASTEXITCODE\n", ps_word(record.to_str().unwrap()), ps_word(record.to_str().unwrap()));
            (script(f, "native console provider.ps1", &source), Some(ps))
        }
        _ => unreachable!(),
    }
}
fn quote_batch(value: &str) -> String {
    assert!(!value.contains(['%', '"', '\r', '\n']));
    format!("\"{value}\"")
}
fn tui(f: &Fixture, extension: &str) {
    let release = Event::new("n8-provider-tui-release");
    let controlled = Event::new("n8-provider-tui-control");
    let (provider, interpreter) = wrapped_probe(f, extension, "n6-case-tui", &release, &controlled);
    let mut s = interactive(f, extension == "ps1", "");
    let editor = snapshot_modes(&mut s);
    s.send(format!("{}; printf '%s' $status > provider-status\r", qp(&provider)).as_bytes());
    assert_eq!(s.wait_file(&f.root.join("child-ready")), b"ready");
    s.wait_protocol(false);
    s.wait_text(b"N6-CHILD-FIRST-READ");
    protocol_off_before(&s, b"N6-CHILD-FIRST-READ");
    if let Some(expected) = interpreter {
        assert_pinned(&f.root.join("provider-interpreter"), &expected);
        assert_native_capability(f);
    }
    let donated = modes(&fs::read(f.root.join("child-original-modes")).unwrap());
    let changed = modes(&fs::read(f.root.join("child-changed-modes")).unwrap());
    assert_ne!(changed, donated, "real nested native child must change console modes");
    assert_eq!(snapshot_modes(&mut s), changed);
    s.paste("provider TUI 雪🐟 & | % !");
    s.send(b"\r");
    assert_eq!(s.wait_file(&f.root.join("child-read-complete")), b"read");
    assert_eq!(fs::read(f.root.join("child-input")).unwrap(), "provider TUI 雪🐟 & | % !".as_bytes());
    assert!(!f.root.join("provider-status").exists(), "provider returned before its blocked native child completed");
    release.set();
    assert_eq!(s.wait_file(&f.root.join("provider-status")), b"0");
    s.wait_protocol(true);
    assert_eq!(snapshot_modes(&mut s), editor);
    capture(&mut s, f, 1, "");
    s.send(b"after-provider-tui");
    capture(&mut s, f, 2, "after-provider-tui");
    let transcript = finish(s);
    assert_eq!(donated, transcript.original_modes, "provider's native grandchild must see the original donated console modes");
}
fn batch_tui(f: &Fixture) { tui(f, "cmd"); }
fn powershell_tui(f: &Fixture) { tui(f, "ps1"); }
fn batch_failure(f: &Fixture) {
    let invalid = f.root.join("invalid provider child.exe");
    fs::write(&invalid, b"owned invalid Windows executable\r\n").unwrap();
    let attempted = f.root.join("batch provider launched");
    let provider = script(f, "failed native child.cmd", &format!("@echo off\r\n{} marker \"%~dp0batch provider launched\"\r\n\"%~dp0invalid provider child.exe\"\r\nexit /b %errorlevel%\r\n", batch_probe(f)));
    let mut s = interactive(f, false, "");
    let editor = snapshot_modes(&mut s);
    // First reject unrepresentable batch argv before launching the provider.
    // Then run a representable provider whose real native child launch fails.
    for (index, arguments) in [quote("quote\"inside"), String::new()].into_iter().enumerate() {
        s.send(format!("{} {arguments}; printf '%s' $status > provider-failed-{index}\r", qp(&provider)).as_bytes());
        let status = s.wait_file(&f.root.join(format!("provider-failed-{index}")));
        assert_ne!(status, b"0");
        assert_eq!(attempted.exists(), index == 1, "batch rejection must precede provider side effects; native launch failure must actually run its provider");
        s.wait_protocol(true);
        assert_eq!(snapshot_modes(&mut s), editor);
        capture(&mut s, f, index * 3 + 1, "");
        s.send(b"editable-after-provider-failure");
        capture(&mut s, f, index * 3 + 2, "editable-after-provider-failure");
        s.send(b"\x03");
        capture(&mut s, f, index * 3 + 3, "");
    }
    finish(s);
}
fn cancellation(f: &Fixture, extension: &str) {
    let release = Event::new("n8-provider-ignore-release");
    let controlled = Event::new("n8-provider-ignore-control");
    let (provider, interpreter) = wrapped_probe(f, extension, "n8-provider-ignore-control", &release, &controlled);
    let mut s = interactive(f, extension == "ps1",
        "function Y; printf forbidden > provider-confirmation-stolen; end; function y; Y; end; function q; Y; end; function Q; Y; end");
    let editor = snapshot_modes(&mut s);
    s.send(format!("{}; printf forbidden > provider-cancel-tail\r", qp(&provider)).as_bytes());
    s.wait_file(&f.root.join("child-ready"));
    s.wait_protocol(false);
    s.wait_text(b"N6-CHILD-FIRST-READ\r\n");
    let ids = fs::read_to_string(f.root.join("provider-native-pids")).unwrap();
    let ids: Vec<u32> = ids.lines().map(|value| value.parse().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    let provider_process = RetainedProcess::acknowledged(ids[0]);
    let native_process = RetainedProcess::acknowledged(ids[1]);
    provider_process.running("provider exited before its acknowledged child was released");
    native_process.running("native child exited before its release barrier");
    if let Some(expected) = interpreter {
        assert_pinned(&f.root.join("provider-interpreter"), &expected);
        assert_native_capability(f);
    }
    let changed = modes(&fs::read(f.root.join("child-changed-modes")).unwrap());
    assert_ne!(changed, modes(&fs::read(f.root.join("child-original-modes")).unwrap()));
    assert_eq!(snapshot_modes(&mut s), changed);
    let provider_output_start = s.trace().len();
    s.control(1);
    controlled.wait();
    // The native child's actual control-handler acknowledgement is the barrier.
    // No grace-period sleep or PID rediscovery substitutes for retained ownership.
    assert_eq!(snapshot_modes(&mut s), changed);
    provider_process.running("provider cancellation reclaimed ownership while its child ignored control");
    native_process.running("the acknowledged native handler must remain alive until release");
    assert!(!f.root.join("provider-cancel-tail").exists(), "cancellation executed the original shell command tail");
    let trace = s.trace();
    let off = trace.windows(8).rposition(|part| part == b"\x1b[?2004l").unwrap();
    assert!(!trace[off + 8..].windows(8).any(|part| part == b"\x1b[?2004h"));
    release.set();
    let native_status = native_process.wait_status("released native child");
    assert_eq!(native_status, 0, "the real ignored-control child must leave through its release barrier");
    assert_eq!(modes(&fs::read(f.root.join("provider-native-restored-modes")).unwrap()),
        modes(&fs::read(f.root.join("child-original-modes")).unwrap()),
        "nested native child must restore its provider's donated modes before the provider resumes input");
    // Ctrl-Break leaves cmd's cancellation confirmation or PowerShell's debugger
    // as the legitimate input owner. Wait for provider-only output progress,
    // never localized prompt wording, before issuing its documented response.
    provider_process.running("provider must retain input for its cancellation interaction");
    s.wait_output_after(provider_output_start);
    let trace = s.trace();
    assert!(!trace[off + 8..].windows(8).any(|part| part == b"\x1b[?2004h"));
    if extension == "cmd" {
        // A pipe write only queues VT bytes for later ConPTY translation. Use
        // acknowledged native key records for cmd's cancellation confirmation.
        s.key_records(&[
            (true, 1, 0x59, 0, b'Y' as u16, 0x10),
            (false, 1, 0x59, 0, b'Y' as u16, 0x10),
            (true, 1, 0x0d, 0, 13, 0),
            (false, 1, 0x0d, 0, 13, 0),
        ]);
    } else {
        s.send(b"q\r");
    }
    s.wait_input_drained();
    let provider_status = provider_process.wait_status("provider after its cancellation interaction");
    fs::write(f.root.join("provider-native-status"),
        format!("child=0x{native_status:08X}\nprovider=0x{provider_status:08X}\n")).unwrap();
    s.wait_protocol(true);
    assert_eq!(snapshot_modes(&mut s), editor);
    assert!(!f.root.join("provider-cancel-tail").exists(), "cancellation resumed the original command tail");
    assert!(!f.root.join("provider-confirmation-stolen").exists(), "fish consumed the provider's confirmation input");
    reclaim(s, f, editor);
    assert!(!f.root.join("provider-confirmation-stolen").exists());
}
fn batch_cancel(f: &Fixture) { cancellation(f, "cmd"); }
fn powershell_cancel(f: &Fixture) { cancellation(f, "ps1"); }
