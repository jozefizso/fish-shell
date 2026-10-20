//! Real N8-ARGV providers. PowerShell 7.3+ must be provisioned in its native ProgramFiles
//! installation; execution policy must permit these locally created scripts. No optional skip.
use super::fixture::{Fixture, exact, quote, run};
use std::collections::BTreeMap;
use fish_widestring::WString;
use std::fs;
use std::path::{Path, PathBuf};
#[path = "provider_sync.rs"]
mod sync;

pub const CASES: &[(&str, fn(&Fixture))] = &[
    ("native-and-loaded-fish-literal-argv", native_and_fish),
    ("trusted-cmd-npm-shim-literal-argv", batch_literals),
    ("trusted-powershell-data-only-literal-argv", powershell_literals),
    ("batch-domain-rejection-before-pipeline-side-effects", batch_rejections),
    ("powershell-frame-boundary-prelaunch-rejection", powershell_limit),
    ("unc-provider-cwd-identity-and-batch-rejection", unc_cwd),
    ("pathext-classification-and-explicit-interpreters", discovery),
    ("provider-cwd-environment-and-byte-streams", streams),
    ("provider-exit-errors-and-raw-dword", statuses),
    ("physical-ancestor-pins-bind-planned-script", ancestor_pins),
    ("provider-leases-survive-shell-without-grandchild-inheritance", lease_survival),
];
fn qp(path: &Path) -> String { quote(path.to_str().unwrap()) }
// Batch source uses ASCII syntax only. cmd expands its own script directory or
// resolves this owned ASCII alias in the explicitly retained fixture-root cwd;
// Unicode names must not be decoded through cmd's ambient source codepage.
fn batch_probe(f: &Fixture) -> &'static str {
    const NAME: &str = "n8-provider-probe.exe";
    let alias = f.root.join(NAME);
    if !alias.exists() { fs::copy(&f.probe, &alias).unwrap(); }
    NAME
}
fn script(f: &Fixture, name: &str, source: &str) -> PathBuf {
    let path = f.root.join(name);
    write_provider_source(&path, source);
    path
}

pub(super) fn write_provider_source(path: &Path, source: &str) {
    if path.extension().is_some_and(|extension| extension == "ps1") {
        // Declare UTF-8 explicitly; source decoding must not depend on a host
        // codepage, even though all provider argv remains full Unicode.
        fs::write(path, format!("\u{feff}{source}")).unwrap();
    } else {
        fs::write(path, source).unwrap();
    }
}
fn ownership_source(extension: &str, source: &str) -> String {
    if extension != "ps1" { return source.to_owned(); }
    format!(
        "[IO.File]::WriteAllLines($PSCommandPath+'.runtime',[string[]]@([Environment]::ProcessPath,$PSVersionTable.PSVersion.ToString(),$PSCommandPath,('PATHEXT='+[Environment]::GetEnvironmentVariable('PATHEXT'))),[Text.UTF8Encoding]::new($false))\n\
         if($PSVersionTable.PSVersion.Major -ne 7 -or $PSVersionTable.PSVersion.Minor -lt 3){{throw 'ownership fixture requires actual PowerShell 7.3+ major 7'}}\n\
         if(([Environment]::GetEnvironmentVariable('PATHEXT') -split ';') -notcontains '.EXE'){{throw 'ownership fixture requires declared native .EXE PATHEXT capability'}}\n{source}"
    )
}
fn assert_ownership_runtime(path: &Path) {
    let mut record = path.as_os_str().to_owned();
    record.push(".runtime");
    let record = fs::read_to_string(PathBuf::from(record)).expect("actual interpreter runtime identity record");
    let fields: Vec<_> = record.lines().collect();
    assert_eq!(fields.len(), 4, "runtime context: {record}");
    let installed = PathBuf::from(std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .expect("required native x64 ProgramFiles fixture")).join("PowerShell/7/pwsh.exe");
    same_identity(fields[0], &installed);
    same_identity(fields[2], path);
    let version: Vec<u32> = fields[1].split('.').map(|value| value.parse().unwrap()).collect();
    assert_eq!(version[0], 7, "actual interpreter version: {record}");
    assert!(version[1] >= 3, "actual interpreter version: {record}");
    assert!(fields[3].strip_prefix("PATHEXT=").unwrap().split(';')
        .any(|extension| extension.eq_ignore_ascii_case(".EXE")),
        "PowerShell must retain the declared native descendant capability: {record}");
}
fn args(values: &[&str]) -> String { values.iter().map(|v| quote(v)).collect::<Vec<_>>().join(" ") }
fn records(bytes: &[u8]) -> BTreeMap<String, String> {
    std::str::from_utf8(bytes).unwrap().lines().map(|line| {
        let (key, hex) = line.split_once('=').unwrap_or_else(|| panic!("invalid record {line:?}"));
        assert_eq!(hex.len() % 2, 0);
        let value = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        (key.to_owned(), String::from_utf8(value).unwrap())
    }).collect()
}
fn same_identity(actual: &str, expected: &Path) {
    let actual: WString = actual.into();
    let expected: WString = expected.to_str().unwrap().into();
    assert_eq!(fish::wutil::native_file_identity(&actual, true).unwrap(), fish::wutil::native_file_identity(&expected, true).unwrap());
}
fn report(f: &Fixture, command: &str, values: &[&str]) {
    let result = f.eval(&format!("set -gx FISH_ACCEPT_PROVIDER {} ; {command} {}", quote("literal 雪\n% ! ^ & |"), args(values)));
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
    let mut values_seen = records(&result.out);
    same_identity(&values_seen.remove("cwd").unwrap(), &f.root);
    assert_eq!(values_seen.remove("FISH_ACCEPT_PROVIDER").unwrap(), "literal 雪\n% ! ^ & |");
    for (index, expected) in values.iter().enumerate() {
        assert_eq!(values_seen.remove(&format!("arg{index}")).as_deref(), Some(*expected));
    }
    assert!(values_seen.is_empty(), "unexpected records {values_seen:?}");
}
fn fish_report(f: &Fixture) -> PathBuf {
    script(f, "literal provider.fish", &format!("{} report $argv\n", f.probe()))
}
fn batch_report(f: &Fixture, extension: &str) -> PathBuf {
    // npm's %~dp0 executable + %* forwarding shape, with a real native child decoding argv.
    fs::copy(&f.probe, f.root.join("native shim.exe")).unwrap();
    script(f, &format!("literal provider.{extension}"), "@echo off\r\n\"%~dp0native shim.exe\" report %*\r\nexit /b %errorlevel%\r\n")
}
const PS_REPORT: &str = "function record([string]$key,[string]$value) { [Console]::Out.WriteLine($key+'='+[Convert]::ToHexString([Text.Encoding]::UTF8.GetBytes($value)).ToLowerInvariant()) }; record 'cwd' ([Environment]::CurrentDirectory); for($i=0;$i -lt $args.Length;$i++){record ('arg'+$i) $args[$i]};record 'FISH_ACCEPT_PROVIDER' $env:FISH_ACCEPT_PROVIDER\n";
fn ps_report(f: &Fixture) -> PathBuf { script(f, "literal provider.ps1", PS_REPORT) }
const FULL: &[&str] = &["", "space value", "quote\"inside", "tail\\", "雪🐟", "-leading", "%PATH%", "! ^ & | < > ( )", "a\nb", "a\rb", "a\r\nb", "'; & 'not a command' #", "$(not-a-command)"];
const BATCH: &[&str] = &["", "space value", "雪🐟", "-leading", "!", "^", "&", "|", "<", ">", "(", ")", "! ^ & | < > ( )", "internal\\slash", "a;b", "$(not-a-command)"];
fn native_and_fish(f: &Fixture) {
    report(f, &format!("{} report", f.probe()), FULL);
    let path = fish_report(f);
    // A PATH fish shadow is not the script interpreter; the loaded fish image must be used.
    let shadow = f.root.join("shadow fish");
    fs::create_dir(&shadow).unwrap();
    fs::copy(&f.probe, shadow.join("fish.exe")).unwrap();
    report(f, &format!("set -gx PATH {}; {}", qp(&shadow), qp(&path)), FULL);
}
fn batch_literals(f: &Fixture) {
    for extension in ["cmd", "bat"] {
        let path = batch_report(f, extension);
        let shadow = f.root.join(format!("shadow {extension}"));
        fs::create_dir(&shadow).unwrap();
        fs::copy(&f.probe, shadow.join("cmd.exe")).unwrap();
        report(f, &format!("set -gx PATH {}; set -gx COMSPEC {}; {}", qp(&shadow), qp(&shadow.join("cmd.exe")), qp(&path)), BATCH);
    }
}
fn powershell_literals(f: &Fixture) {
    let path = ps_report(f);
    let shadow = f.root.join("shadow pwsh");
    fs::create_dir(&shadow).unwrap();
    fs::copy(&f.probe, shadow.join("pwsh.exe")).unwrap();
    let injected = f.root.join("injected powershell marker");
    let payload = format!("';[IO.File]::WriteAllText('{}','injected');#", injected.display());
    let mut values = FULL.to_vec();
    values.push(&payload);
    report(f, &format!("set -gx PATH {}; {}", qp(&shadow), qp(&path)), &values);
    assert!(!injected.exists(), "PowerShell input became code");
}
fn batch_rejections(f: &Fixture) {
    let provider_marker = f.root.join("batch was launched");
    let first_marker = f.root.join("pipeline first was launched");
    let injected_marker = f.root.join("batch injection marker");
    let probe = batch_probe(f);
    let path = script(f, "reject provider.cmd", &format!("@echo off\r\n\"%~dp0{probe}\" marker \"%~dp0batch was launched\"\r\n"));
    let payload = format!("x\" & \"{}\" marker \"{}\" & rem \"", f.probe.display(), injected_marker.display());
    for value in ["quote\"", "%PATH%", "line\nfeed", "carriage\rreturn", "tail\\", &payload] {
        let result = f.eval(&format!("{} marker {} | {} {}", f.probe(), qp(&first_marker), qp(&path), quote(value)));
        assert_ne!(result.code, 0);
        let diagnostic = String::from_utf8(result.err).unwrap();
        assert!(diagnostic.contains("batch literal domain"), "{diagnostic}");
        assert!(!first_marker.exists(), "earlier stage ran before provider validation");
        assert!(!provider_marker.exists(), "rejected provider ran");
        assert!(!injected_marker.exists(), "rejected data became command syntax");
    }
    let long = "x".repeat(8192);
    let result = f.eval(&format!("{} {}", qp(&path), quote(&long)));
    assert_ne!(result.code, 0);
    assert!(String::from_utf8(result.err).unwrap().contains("8191"));
    assert!(!provider_marker.exists());
    // Script path, not just argv, participates in the same literal policy.
    let bad_path = script(f, "percent%PATH%.cmd", "@echo forbidden>\"%~dp0batch was launched\"\r\n");
    let result = f.eval(&qp(&bad_path));
    assert_ne!(result.code, 0);
    assert!(!provider_marker.exists());
    let key = "FISH_ACCEPT_PROVIDER";
    let maximum = 8191 - key.encode_utf16().count() - 2;
    let shim = batch_report(f, "cmd");
    for length in [maximum - 1, maximum] {
        let value = "x".repeat(length);
        let result = f.eval(&format!("set -gx {key} {}; {} boundary", quote(&value), qp(&shim)));
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        assert!(result.err.is_empty());
        let received = records(&result.out);
        assert_eq!(received[key], value);
        assert_eq!(received["arg0"], "boundary");
    }
    // One non-BMP scalar consumes two UTF-16 units; the accepted boundary is not UTF-8 bytes.
    let value = format!("{}🐟", "x".repeat(maximum - 2));
    let result = f.eval(&format!("set -gx {key} {}; {} unicode-boundary", quote(&value), qp(&shim)));
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert_eq!(records(&result.out)[key], value);
    for length in [maximum + 1, 8192] {
        let result = f.eval(&format!("set -gx {key} {}; {} marker {} | {}", quote(&"x".repeat(length)), f.probe(), qp(&first_marker), qp(&path)));
        assert_ne!(result.code, 0);
        assert!(String::from_utf8(result.err).unwrap().contains("inherited environment entry"));
        assert!(!provider_marker.exists());
        assert!(!first_marker.exists(), "earlier stage ran before batch environment validation");
    }
}
fn powershell_limit(f: &Fixture) {
    let marker = f.root.join("oversized powershell launched");
    let first = f.root.join("oversized pipeline first launched");
    let path = script(f, "oversized provider.ps1", &format!("[IO.File]::WriteAllText('{}','launched')", marker.display()));
    let result = f.eval(&format!("{} marker {} | {} {}", f.probe(), qp(&first), qp(&path), quote(&"x".repeat(13_000))));
    assert_ne!(result.code, 0);
    assert!(String::from_utf8(result.err).unwrap().contains("data frame exceeds"));
    assert!(!marker.exists(), "PowerShell executed before frame validation");
    assert!(!first.exists(), "earlier pipeline stage executed before frame validation");
}
fn unc_cwd(f: &Fixture) {
    struct Remote(PathBuf);
    impl Drop for Remote {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    let root = std::env::var_os("FISH_NATIVE_N1_UNC_ROOT").map(PathBuf::from)
        .expect("N8-ARGV requires the existing writable FISH_NATIVE_N1_UNC_ROOT fixture; never skipped");
    let remote = Remote(root.join(f.root.file_name().unwrap()));
    fs::create_dir(&remote.0).expect("required writable UNC fixture");
    let marker = f.root.join("UNC batch launched");
    let batch = script(f, "UNC reject provider.cmd", "@echo forbidden>\"%~dp0UNC batch launched\"\r\n");
    let result = f.eval(&format!("cd {}; {}", qp(&remote.0), qp(&batch)));
    assert_ne!(result.code, 0);
    assert!(String::from_utf8(result.err).unwrap().contains("local-drive"));
    assert!(!marker.exists(), "cmd.exe changed UNC cwd and executed anyway");
    let remote_batch = remote.0.join("reject.cmd");
    fs::copy(&batch, &remote_batch).unwrap();
    assert_ne!(f.eval(&qp(&remote_batch)).code, 0);
    assert!(!marker.exists());
    for path in [fish_report(f), ps_report(f)] {
        let result = f.eval(&format!("set -gx FISH_ACCEPT_PROVIDER UNC; cd {}; {} literal", qp(&remote.0), qp(&path)));
        assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
        assert!(result.err.is_empty());
        let values = records(&result.out);
        same_identity(&values["cwd"], &remote.0);
        assert_eq!(values["arg0"], "literal");
        assert_eq!(values["FISH_ACCEPT_PROVIDER"], "UNC");
    }
}
fn interpreter(script: &Path) -> PathBuf {
    let text: WString = script.to_str().unwrap().into();
    let command = fish::path::native_classify_command(&text).unwrap();
    fish::path::native_plan_command(command, vec![script.as_os_str().to_owned()], script.parent().unwrap().to_owned(), std::env::vars_os().collect()).unwrap().executable
}
fn discovery(f: &Fixture) {
    let first = f.root.join("collision first");
    let second = f.root.join("collision second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::copy(&f.probe, first.join("choose.exe")).unwrap();
    fs::write(first.join("choose.cmd"), "@echo off\r\nexit /b 23\r\n").unwrap();
    fs::write(second.join("choose.cmd"), "@echo off\r\nexit /b 29\r\n").unwrap();
    let setup = format!("set -gx PATH {} {}; set -gx PATHEXT '.cMd;.ExE;.BAT;.PS1;.FISH'", qp(&first), qp(&second));
    exact(f.eval(&format!("{setup}; choose")), 23, b"", b"");
    exact(f.eval(&format!("{setup}; set -gx PATHEXT '.EXE;.CMD'; choose exit 19")), 19, b"", b"");
    exact(f.eval(&format!("{setup}; choose.exe exit 17")), 17, b"", b"");
    exact(f.eval(&format!("{setup}; set -gx PATH {} {}; choose", qp(&second), qp(&first))), 29, b"", b"");
    exact(f.eval(&format!("test -x {}", qp(&first.join("choose.cmd")))), 0, b"", b"");
    for extension in ["com", "txt", "", "sh"] {
        let name = if extension.is_empty() { "unknown".to_owned() } else { format!("unknown.{extension}") };
        let path = script(f, &name, &format!("#!{}\n{} marker {}\n", f.fish.display(), f.probe(), qp(&f.root.join("unknown marker"))));
        let result = f.eval(&qp(&path));
        assert_ne!(result.code, 0);
        assert!(!f.root.join("unknown marker").exists());
    }
    let batch = batch_report(f, "cmd");
    // Explicit cmd.exe is an ordinary native command, not an implicit fallback.
    exact(f.eval(&format!("{} /d /s /v:off /c {}", qp(&interpreter(&batch)), quote("exit 31"))), 31, b"", b"");
    let cmd = interpreter(&batch);
    let marker = f.root.join("explicit cmd launched");
    let first_marker = f.root.join("explicit cmd pipeline launched");
    let ordinary_root = fish::path::native_tool_path(&f.root).unwrap();
    let code = format!("echo explicit-marker>\"{}\"", ordinary_root.join("explicit cmd launched").display());
    // Positive control: the same intentional code string can create its marker
    // through the supported explicit interpreter contract.
    exact(f.eval(&format!("{} /q /c {}", qp(&cmd), quote(&code))), 0, b"", b"");
    assert_eq!(fs::read(&marker).unwrap(), b"explicit-marker\r\n");
    fs::remove_file(&marker).unwrap();
    for (arguments, expected) in [
        (format!("/v:on /c {}", quote(&code)), "unsupported explicit system cmd switch"),
        (format!("/c {} extra", quote("exit 31")), "exactly one command-language string"),
    ] {
        let result = f.eval(&format!("{} marker {} | {} {arguments}", f.probe(), qp(&first_marker), qp(&cmd)));
        assert_ne!(result.code, 0, "unsupported explicit cmd input must fail before launch");
        let diagnostic = String::from_utf8(result.err).unwrap();
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert!(!marker.exists(), "unsupported switches executed explicit cmd code");
        assert!(!first_marker.exists(), "an earlier pipeline stage ran before explicit cmd prelaunch validation");
    }
    let unc = std::env::var_os("FISH_NATIVE_N1_UNC_ROOT").map(PathBuf::from)
        .expect("N8-ARGV explicit cmd requires the existing mandatory UNC fixture");
    assert!(unc.is_dir() && unc.to_str().unwrap().starts_with("\\\\"), "required real UNC cwd fixture: {}", unc.display());
    let result = f.eval(&format!("cd {}; {} marker {} | {} /c {}", qp(&unc), f.probe(), qp(&first_marker), qp(&cmd), quote(&code)));
    assert_ne!(result.code, 0, "explicit cmd must not silently replace UNC cwd");
    let diagnostic = String::from_utf8(result.err).unwrap();
    assert!(diagnostic.contains("local-drive"), "{diagnostic}");
    assert!(!marker.exists(), "explicit cmd executed after changing UNC cwd");
    assert!(!first_marker.exists(), "UNC cwd rejection must precede pipeline side effects");
    let ps = ps_report(f);
    let text: WString = ps.to_str().unwrap().into();
    let command = fish::path::native_classify_command(&text).unwrap();
    let invocation = fish::path::native_plan_command(command, vec![ps.as_os_str().to_owned()], f.root.clone(), std::env::vars_os().collect()).unwrap();
    assert!(!invocation.executable.to_str().unwrap().starts_with("\\\\?\\"), "PowerShell must receive an ordinary identity-proven application path");
    assert!(!invocation.cwd.to_str().unwrap().starts_with("\\\\?\\"), "PowerShell must receive an ordinary identity-proven cwd");
    let installed = PathBuf::from(std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .expect("required native x64 ProgramFiles fixture")).join("PowerShell/7/pwsh.exe");
    same_identity(invocation.executable.to_str().unwrap(), &installed);
    same_identity(invocation.cwd.to_str().unwrap(), &f.root);
    let ordinary_probe = fish::path::native_tool_path(&f.probe).unwrap();
    let extended_probe = fs::canonicalize(&f.probe).unwrap();
    assert!(extended_probe.to_str().unwrap().starts_with("\\\\?\\"), "native canonical fixture must exercise the extended spelling");
    let from_extended = fish::path::native_tool_path(&extended_probe).unwrap();
    assert!(!ordinary_probe.to_str().unwrap().starts_with("\\\\?\\"));
    assert!(!from_extended.to_str().unwrap().starts_with("\\\\?\\"));
    same_identity(ordinary_probe.to_str().unwrap(), &f.probe);
    same_identity(from_extended.to_str().unwrap(), &ordinary_probe);
    let extended_only = fs::canonicalize(&f.root).unwrap().join("extended-only.");
    fs::write(&extended_only, b"requires exact extended native name").unwrap();
    let error = fish::path::native_tool_path(&extended_only).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "a trailing-dot object must not alias an ordinary name: {error}");
    // Generic evidence copying uses ordinary directory-entry names. This
    // deliberately extended-only object must be removed by its exact capability.
    fs::remove_file(&extended_only).unwrap();
    report(f, &format!("{} -NoLogo -NoProfile -File {}", qp(&invocation.executable), qp(&ps)), &["", "space", "雪", "-dash", "tail\\"]);
}
fn streams(f: &Fixture) {
    let bytes = b"\0\r\n\xff\x80utf8:\xe9\x9b\xaa";
    let fish = script(f, "stream provider.fish", &format!("{} stream\n", f.probe()));
    let probe = batch_probe(f);
    let batch = script(f, "stream provider.cmd", &format!("@echo off\r\n\"%~dp0{probe}\" stream\r\nexit /b %errorlevel%\r\n"));
    let ps = script(f, "stream provider.ps1", "$i=[Console]::OpenStandardInput();$o=[Console]::OpenStandardOutput();$e=[Console]::OpenStandardError();$b=[Text.Encoding]::ASCII.GetBytes('probe-out:');$o.Write($b,0,$b.Length);$i.CopyTo($o);$o.Flush();[byte[]]$x=112,114,111,98,101,45,101,114,114,58,0,10,13,10,255,128;$e.Write($x,0,$x.Length);$e.Flush()\n");
    for path in [&fish, &batch, &ps] {
        let result = f.input(&qp(path), bytes);
        let mut expected = b"probe-out:".to_vec();
        expected.extend_from_slice(bytes);
        exact(result, 0, &expected, b"probe-err:\0\n\r\n\xff\x80");
    }
    // Cwd is checked by actual consumers, not environment PWD/source-text assertions.
    let nested = f.root.join("nested cwd 雪");
    fs::create_dir(&nested).unwrap();
    let drive_root = f.root.ancestors().last().unwrap().to_owned();
    for cwd in [&nested, &drive_root] {
        for path in [fish_report(f), batch_report(f, "cmd"), ps_report(f)] {
            let code = format!("set -gx FISH_ACCEPT_PROVIDER cwd-env; cd {}; {} 'x'", qp(cwd), qp(&path));
            let result = f.eval(&code);
            assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
            assert!(result.err.is_empty());
            let output = records(&result.out);
            same_identity(&output["cwd"], cwd);
            assert_eq!(output["FISH_ACCEPT_PROVIDER"], "cwd-env");
            assert_eq!(output["arg0"], "x");
            assert_eq!(output.len(), 3);
        }
    }
}
fn statuses(f: &Fixture) {
    let batch = script(f, "status provider.cmd", "@exit /b -1\r\n");
    let ps = script(f, "status provider.ps1", "exit -1\n");
    let fish = script(f, "status provider.fish", "exit 37\n");
    for (path, code, err) in [
        (&batch, 1, b"fish: native process exited with status 0xFFFFFFFF\n".as_slice()),
        (&ps, 1, b"fish: native process exited with status 0xFFFFFFFF\n".as_slice()),
        (&fish, 37, b"".as_slice()),
    ] {
        exact(f.eval(&qp(path)), code, b"", err);
    }
    for path in [&batch, &ps] {
        exact(run(&mut probe_command(f).arg("n8-provider-raw").arg(path).arg(u32::MAX.to_string()), b""), 0, b"", b"");
    }
    for raw in [0u32, 37, 255, 256] {
        let mapped = if raw <= 255 { raw } else { 1 };
        let diagnostic = if raw > 255 { format!("fish: native process exited with status 0x{raw:08X}\n") } else { String::new() };
        exact(f.eval(&format!("{} exit {raw}", f.probe())), mapped, b"", diagnostic.as_bytes());
        fs::write(&batch, format!("@exit /b {raw}\r\n")).unwrap();
        fs::write(&ps, format!("exit {raw}\n")).unwrap();
        for path in [&batch, &ps] {
            exact(f.eval(&qp(path)), mapped, b"", diagnostic.as_bytes());
            exact(run(&mut probe_command(f).arg("n8-provider-raw").arg(path).arg(raw.to_string()), b""), 0, b"", b"");
        }
    }
    let error = script(f, "error provider.ps1", "throw 'deliberate-provider-error'\n");
    let result = f.eval(&qp(&error));
    assert_eq!(result.code, 1);
    assert!(String::from_utf8(result.err).unwrap().contains("deliberate-provider-error"));
    // Invalid EXE files must still reach native CreateProcessW, with no script retry.
    let marker = f.root.join("invalid exe marker");
    let invalid = script(f, "invalid image.exe", &format!("{} marker {}", f.probe(), qp(&marker)));
    let result = f.eval(&qp(&invalid));
    assert_ne!(result.code, 0);
    assert!(!result.err.is_empty());
    assert!(!marker.exists());
}

fn probe_command(f: &Fixture) -> std::process::Command {
    let mut command = std::process::Command::new(&f.probe);
    command.env_clear().current_dir(&f.root);
    for (name, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(name, value); }
    }
    command
}
fn owned_shell(mut command: std::process::Command, stderr: &Path, input: bool) -> sync::Shell {
    command.stdin(if input { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(stderr).unwrap());
    sync::Shell(command.spawn().unwrap())
}
fn denied_write(path: &Path) {
    let error = match fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => panic!("live provider script unexpectedly writable: {}", path.display()),
        Err(error) => error,
    };
    assert_eq!(error.raw_os_error(), Some(32), "expected sharing enforcement, not incidental missing access: {error}");
}
fn denied_rename(source: &Path, target: &Path) {
    let error = fs::rename(source, target).expect_err("physical provider ancestor was movable");
    assert!(matches!(error.raw_os_error(), Some(5 | 32)), "{error}");
}
fn provider_word(extension: &str, value: &str) -> String {
    match extension {
        "cmd" => { assert!(!value.contains('"')); format!("\"{value}\"") }
        "ps1" => format!("'{}'", value.replace('\'', "''")),
        "fish" => quote(value),
        _ => unreachable!(),
    }
}
fn provider_call(f: &Fixture, extension: &str, mode: &str, values: &[&str]) -> String {
    if extension == "cmd" {
        let probe = batch_probe(f);
        let mut words = vec![provider_word(extension, probe), provider_word(extension, mode)];
        for value in values {
            let relative = Path::new(value).strip_prefix(&f.root);
            let value = relative.as_ref().map_or(*value, |path| path.to_str().unwrap());
            assert!(value.is_ascii(), "batch fixture arguments must use ASCII source or runtime Unicode expansion");
            words.push(provider_word(extension, value));
        }
        return words.join(" ");
    }
    let mut words = vec![provider_word(extension, f.probe.to_str().unwrap()), provider_word(extension, mode)];
    words.extend(values.iter().map(|value| provider_word(extension, value)));
    if extension == "ps1" {
        // An unpipelined native command can be backgrounded by PowerShell's
        // application classification. The fixture protocol explicitly waits
        // for each real child and propagates its native failure.
        format!("& {} | Out-Null; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}", words.join(" "))
    } else {
        words.join(" ")
    }
}
fn ancestor_pins(f: &Fixture) {
    for extension in ["cmd", "ps1", "fish"] {
        let directory = f.root.join(format!("physical ancestor {extension}"));
        fs::create_dir(&directory).unwrap();
        let path = directory.join(format!("original.{extension}"));
        let original = f.root.join(format!("original {extension} marker"));
        let replacement_marker = f.root.join(format!("replacement {extension} marker"));
        let suffix = if extension == "cmd" { "\r\nexit /b 0\r\n" } else { "\n" };
        let source = format!("{}{}{}", if extension == "cmd" { "@echo off\r\n" } else { "" }, provider_call(f, extension, "marker", &[original.to_str().unwrap()]), suffix);
        write_provider_source(&path, &ownership_source(extension, &source));
        let replacement = script(f, &format!("replacement.{extension}"), &ownership_source(extension,
            &format!("{}{}", provider_call(f, extension, "marker", &[replacement_marker.to_str().unwrap()]), suffix)));
        let ready = sync::Event::new("classified");
        let release = sync::Event::new("plan-after-rename-attempt");
        let stderr = f.root.join(format!("ancestor {extension}.stderr"));
        let mut command = probe_command(f);
        if extension == "ps1" { command.env("PATHEXT", ".EXE"); }
        command.current_dir(&f.root).arg("n8-provider-ancestor").arg(&path)
            .arg(&ready.name).arg(&release.name).arg(&replacement);
        let mut owner = owned_shell(command, &stderr, false);
        ready.wait();
        // This is a physical ancestor, not merely the original junction spelling. Neither
        // the script nor the child's cwd is the directory being renamed.
        let moved = f.root.join(format!("moved physical {extension}"));
        denied_rename(&directory, &moved);
        denied_write(&path);
        assert!(!replacement_marker.exists());
        release.signal();
        owner.wait(0);
        assert!(fs::read(&stderr).unwrap().is_empty());
        // Positive control: these operations are permitted once the real leases are gone.
        fs::rename(&directory, &moved).unwrap();
        fs::rename(&moved, &directory).unwrap();
        let _writable = fs::OpenOptions::new().write(true).open(&path).unwrap();
        drop(_writable);
        let code = qp(&path);
        let mut command = f.command(&[std::ffi::OsStr::new("-N"), std::ffi::OsStr::new("-c"), std::ffi::OsStr::new(&code)]);
        if extension == "ps1" { command.env("PATHEXT", ".EXE"); }
        exact(run(&mut command, b""), 0, b"", b"");
        if extension == "ps1" { assert_ownership_runtime(&path); }
        assert_eq!(fs::read(&original).unwrap(), b"launched");
        assert!(!replacement_marker.exists());
    }
}
fn lease_survival(f: &Fixture) {
    use std::io::Write;
    for extension in ["cmd", "ps1", "fish"] {
        for abrupt in [false, true] {
            let label = format!("{extension}-{}", if abrupt { "abrupt" } else { "orderly" });
            let directory = f.root.join(format!("surviving provider {label}"));
            fs::create_dir(&directory).unwrap();
            let path = directory.join(format!("survivor.{extension}"));
            let ready = sync::Event::new("provider-barrier-ready");
            let release = sync::Event::new("provider-barrier-release");
            let shell_ready = sync::Event::new("provider-disowned");
            let survivor_ready = sync::Event::new("grandchild-ready");
            let survivor_release = sync::Event::new("grandchild-release");
            let pids = f.root.join(format!("provider {label}.pids"));
            let survivor_pid = f.root.join(format!("grandchild {label}.pid"));
            let marker = f.root.join(format!("provider {label} finished"));
            let mut source = if extension == "cmd" { "@echo off\r\n".to_owned() } else { String::new() };
            source.push_str(&provider_call(f, extension, "n8-provider-barrier", &[&ready.name, &release.name, pids.to_str().unwrap()]));
            source.push_str(if extension == "cmd" { "\r\n" } else { "\n" });
            if extension == "cmd" {
                // Later consumption must be beyond cmd's prefetched script region.
                for _ in 0..4096 { source.push_str("rem synchronized incremental script consumption padding\r\n"); }
            }
            source.push_str(&provider_call(f, extension, "n8-provider-spawn-survivor", &[&survivor_ready.name, &survivor_release.name, survivor_pid.to_str().unwrap()]));
            source.push_str(if extension == "cmd" { "\r\n" } else { "\n" });
            source.push_str(&provider_call(f, extension, "marker", &[marker.to_str().unwrap()]));
            source.push_str(if extension == "cmd" { "\r\nexit /b 0\r\n" } else { "\n" });
            write_provider_source(&path, &ownership_source(extension, &source));
            let code = format!("{} &; disown $last_pid; {} n8-provider-signal {}; read -l provider_fence; exit 0", qp(&path), f.probe(), quote(&shell_ready.name));
            let stderr = f.root.join(format!("shell {label}.stderr"));
            let mut command = f.command(&[std::ffi::OsStr::new("-N"), std::ffi::OsStr::new("-c"), std::ffi::OsStr::new(&code)]);
            if extension == "ps1" { command.env("PATHEXT", ".EXE"); }
            let mut shell = owned_shell(command, &stderr, true);
            ready.wait();
            shell_ready.wait();
            if extension == "ps1" { assert_ownership_runtime(&path); }
            let ids: Vec<u32> = fs::read_to_string(&pids).unwrap().lines().map(|line| line.parse().unwrap()).collect();
            assert_eq!(ids.len(), 2);
            // Declare the root last: cleanup kills it before its barrier helper can advance it.
            let barrier = sync::Process::open(ids[1]);
            let provider = sync::Process::open(ids[0]);
            denied_write(&path);
            let moved = f.root.join(format!("moved survivor {label}"));
            denied_rename(&directory, &moved);
            if abrupt {
                shell.kill();
            } else {
                shell.0.stdin.take().unwrap().write_all(b"release\n").unwrap();
                shell.wait(0);
                assert!(fs::read(&stderr).unwrap().is_empty());
            }
            provider.live();
            barrier.live();
            // No shell-side File owner remains now. Only the non-inheritable, zero-rights
            // leases duplicated into the real provider can enforce these operations.
            denied_write(&path);
            denied_rename(&directory, &moved);
            release.signal();
            survivor_ready.wait();
            let grandchild = sync::Process::from_file(&survivor_pid);
            barrier.wait(0);
            provider.wait(0);
            grandchild.live();
            assert_eq!(fs::read(&marker).unwrap(), b"launched");
            // The still-live descendant must not retain any script/directory reservation.
            let mut writable = fs::OpenOptions::new().write(true).open(&path).unwrap();
            writable.write_all(source.as_bytes()).unwrap();
            writable.flush().unwrap();
            drop(writable);
            fs::rename(&directory, &moved).unwrap();
            fs::rename(&moved, &directory).unwrap();
            survivor_release.signal();
            grandchild.wait(0);
        }
    }
}
