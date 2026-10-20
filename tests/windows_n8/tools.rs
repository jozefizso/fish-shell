//! Real N8-TOOLS workflows. Every tool is mandatory and pinned by an absolute
//! FISH_NATIVE_N8_<TOOL> path and exact FISH_NATIVE_N8_<TOOL>_VERSION output.
//! Provision native Git, a real MSVC-host rustc (not a rustup proxy), Node/npm,
//! CPython with venv/ensurepip, and the production provider's PowerShell 7.3+.
//! FISH_NATIVE_N8_TOOL_PATH supplies native linker/SDK executable directories;
//! FISH_NATIVE_N8_LIB supplies the MSVC/SDK LIB search list. No network is used.
use super::fixture::{Captured, Fixture, quote, run};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "tools_startup.rs"]
mod startup;

pub const CASES: &[(&str, fn(&Fixture))] = &[
    ("pinned-native-tool-capabilities", capabilities),
    ("native-git-local-repository-commit-roundtrip", git_repository),
    ("native-rustc-msvc-build-literal-argv", compiler),
    ("native-npm-offline-package-install-and-script", package_manager),
    ("native-python-venv-package-and-literal-script", python),
    ("native-powershell-profile-free-provider-and-interpreter", powershell),
    ("native-tools-deliberate-resolution-and-missing-dependencies", resolution),
    ("native-provider-autorun-and-profile-markers", startup_markers),
];

#[derive(Clone)]
struct Tool {
    name: &'static str,
    path: PathBuf,
    version: String,
}
impl Tool {
    fn required(name: &'static str) -> Self {
        let key = format!("FISH_NATIVE_N8_{name}");
        let path = std::env::var_os(&key).map(PathBuf::from)
            .unwrap_or_else(|| panic!("N8-TOOLS prerequisite: set {key} to the pinned native tool's absolute path; this case is never skipped"));
        assert!(path.is_absolute() && path.is_file(), "N8-TOOLS prerequisite {key}: missing absolute tool {}", path.display());
        let expected = if name == "NPM" { "cmd" } else { "exe" };
        assert!(path.extension().and_then(OsStr::to_str).is_some_and(|v| v.eq_ignore_ascii_case(expected)),
            "{key} must be the real native .{expected} tool, not an MSYS shell wrapper: {}", path.display());
        let version_key = format!("{key}_VERSION");
        let version = std::env::var(&version_key)
            .unwrap_or_else(|_| panic!("N8-TOOLS prerequisite: set {version_key} to the exact pinned --version stdout (PowerShell: PSVersion.ToString())"));
        assert!(!version.is_empty() && !version.contains(['\r', '\n']), "{version_key} must be a nonempty single version line");
        Self { name, path, version }
    }
    fn word(&self) -> String { qp(&self.path) }
    fn verify(&self, f: &Fixture, peers: &[&Tool]) {
        if self.name == "NPM" {
            let node = peers.iter().find(|tool| tool.name == "NODE").expect("npm requires its pinned native Node peer");
            let adjacent = self.path.parent().unwrap().join("node.exe");
            assert!(adjacent.is_file(), "N8-TOOLS requires npm.cmd with its adjacent pinned node.exe: {}", adjacent.display());
            same_path(adjacent.to_str().unwrap(), &node.path);
        }
        let args = if self.name == "PWSH" {
            "-NoLogo -NoProfile -NonInteractive -Command '$PSVersionTable.PSVersion.ToString()'"
        } else { "--version" };
        let result = evaluate(f, peers, &format!("{} {args}", self.word()), b"");
        success(&result);
        assert!(result.err.is_empty(), "{} version diagnostic: {}", self.name, String::from_utf8_lossy(&result.err));
        let actual = std::str::from_utf8(&result.out).expect("native tool version must be UTF-8").trim_end_matches(['\r', '\n']);
        assert_eq!(actual, self.version, "{} version pin at {}", self.name, self.path.display());
        if self.name == "PWSH" {
            let parts: Vec<u32> = actual.split('.').map(|v| v.parse().expect("PowerShell must be a stable numeric release")).collect();
            assert!(parts.len() >= 2 && (parts[0], parts[1]) >= (7, 3), "N8 provider requires PowerShell >=7.3, got {actual}");
        }
        let record = format!("tool={}\npath={}\nversion={}\n", self.name, self.path.display(), actual);
        fs::write(f.root.join(format!("tool-{}.txt", self.name)), &record).unwrap();
        eprintln!("N8-TOOLS {} {} {actual}", self.name, self.path.display());
    }
}
/// Share the mandatory executable/version contract with the real console cases.
pub(super) fn pinned_interpreter(f: &Fixture, name: &'static str) -> PathBuf {
    assert!(matches!(name, "PYTHON" | "PWSH"), "console fixture interpreter must be deliberate");
    let tool = Tool::required(name);
    tool.verify(f, &[&tool]);
    tool.path
}
fn qp(path: &Path) -> String { quote(path.to_str().expect("tool fixture paths must be reversible UTF-8")) }
fn words(values: &[&str]) -> String { values.iter().map(|v| quote(v)).collect::<Vec<_>>().join(" ") }
fn configured(f: &Fixture, tools: &[&Tool], code: &str) -> Command {
    let mut cmd = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    let mut paths = Vec::new();
    for tool in tools {
        let parent = tool.path.parent().unwrap().to_owned();
        if !paths.contains(&parent) { paths.push(parent); }
    }
    if let Some(extra) = std::env::var_os("FISH_NATIVE_N8_TOOL_PATH") {
        paths.extend(std::env::split_paths(&extra));
    }
    let system = PathBuf::from(std::env::var_os("SystemRoot").expect("required native SystemRoot"));
    paths.push(system.join("System32"));
    let temp = f.root.join("tool temporary files");
    fs::create_dir_all(&temp).unwrap();
    cmd.env("PATH", std::env::join_paths(paths).expect("native tool PATH"))
        .env("PATHEXT", ".EXE;.CMD;.BAT;.PS1;.FISH")
        .env("TEMP", &temp).env("TMP", &temp)
        .env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", f.root.join("no-global-gitconfig"))
        .env("GIT_TERMINAL_PROMPT", "0").env("GIT_PAGER", "")
        .env("npm_config_cache", f.root.join("npm-cache"))
        .env("npm_config_userconfig", f.root.join("npm-userconfig"))
        .env("npm_config_globalconfig", f.root.join("npm-globalconfig"))
        .env("npm_config_update_notifier", "false")
        .env("PIP_CONFIG_FILE", "NUL").env("PIP_NO_INDEX", "1")
        .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
        .env("PYTHONUTF8", "1").env("PYTHONNOUSERSITE", "1")
        .env("FISH_N8_VALUE", "exported 雪 & | % literal");
    if let Some(lib) = std::env::var_os("FISH_NATIVE_N8_LIB") { cmd.env("LIB", lib); }
    cmd
}
fn evaluate(f: &Fixture, tools: &[&Tool], code: &str, input: &[u8]) -> Captured {
    let result = run(&mut configured(f, tools, code), input);
    // Retain the real command and both streams even when its assertions fail.
    use std::io::Write;
    let mut evidence = fs::OpenOptions::new().create(true).append(true).open(f.root.join("tools-transcript.txt")).unwrap();
    writeln!(evidence, "COMMAND {code}\nSTATUS {}\nSTDOUT", result.code).unwrap();
    evidence.write_all(&result.out).unwrap();
    evidence.write_all(b"\nSTDERR\n").unwrap();
    evidence.write_all(&result.err).unwrap();
    evidence.write_all(b"\nEND\n").unwrap();
    result
}
fn success(result: &Captured) {
    assert_eq!(result.code, 0, "stdout={}\nstderr={}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
}
fn checked(f: &Fixture, tools: &[&Tool], code: &str) -> Captured {
    let result = evaluate(f, tools, code, b"");
    success(&result);
    result
}
fn create(f: &Fixture, name: &str, contents: impl AsRef<[u8]>) -> PathBuf {
    let path = f.root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let contents = contents.as_ref();
    if path.extension().is_some_and(|extension| extension == "ps1") {
        super::providers::write_provider_source(&path, std::str::from_utf8(contents).expect("UTF-8 PowerShell fixture"));
    } else {
        fs::write(&path, contents).unwrap();
    }
    path
}
fn same_path(actual: &str, expected: &Path) {
    assert_eq!(fs::canonicalize(actual.trim_end_matches(['\r', '\n'])).unwrap(), fs::canonicalize(expected).unwrap());
}
fn capabilities(f: &Fixture) {
    let tools: Vec<_> = ["GIT", "RUSTC", "NODE", "NPM", "PYTHON", "PWSH"].into_iter().map(Tool::required).collect();
    let refs: Vec<_> = tools.iter().collect();
    for tool in &tools { tool.verify(f, &refs); }
    let rustc = &tools[1];
    let host = checked(f, &refs, &format!("{} -vV", rustc.word()));
    assert!(String::from_utf8_lossy(&host.out).lines().any(|v| v == "host: x86_64-pc-windows-msvc"),
        "compiler prerequisite: provision a native x86_64-pc-windows-msvc rustc, not an MSYS/Unix compiler");
    let python = &tools[4];
    let result = checked(f, &refs, &format!("{} -I -c {}", python.word(), quote("import sys,venv,ensurepip; assert sys.platform == 'win32'; print(sys.executable)")));
    same_path(std::str::from_utf8(&result.out).unwrap(), &python.path);
    let node = &tools[2];
    let result = checked(f, &refs, &format!("{} -e {}", node.word(), quote("if(process.platform!=='win32')process.exit(19);console.log(process.execPath)")));
    same_path(std::str::from_utf8(&result.out).unwrap(), &node.path);
}
fn git_repository(f: &Fixture) {
    let git = Tool::required("GIT");
    git.verify(f, &[&git]);
    let repo = f.root.join("repository 雪 & ordinary");
    fs::create_dir(&repo).unwrap();
    let g = format!("{} -C {}", git.word(), qp(&repo));
    checked(f, &[&git], &format!("{g} init --quiet"));
    checked(f, &[&git], &format!("{g} config user.name 'N8 Native'; and {g} config user.email n8@example.invalid; and {g} config core.autocrlf false; and {g} config commit.gpgsign false"));
    let name = "literal 雪 & percent%.txt";
    let original = b"native repository\r\n\0literal bytes\xff\n";
    fs::write(repo.join(name), original).unwrap();
    checked(f, &[&git], &format!("{g} add -- {}; and {g} commit --quiet -m {}", quote(name), quote("ordinary 雪 & | % ! commit")));
    let show = checked(f, &[&git], &format!("{g} cat-file blob {}", quote(&format!("HEAD:{name}"))));
    assert_eq!(show.out, original);
    fs::write(repo.join(name), b"changed\n").unwrap();
    let diff = evaluate(f, &[&git], &format!("{g} diff --quiet -- {}", quote(name)), b"");
    assert_eq!(diff.code, 1, "real repository modification must be observable");
    checked(f, &[&git], &format!("{g} checkout -- {}; and {g} status --porcelain=v1", quote(name)));
    assert_eq!(fs::read(repo.join(name)).unwrap(), original);
    assert!(checked(f, &[&git], &format!("{g} status --porcelain=v1")).out.is_empty());
}
fn compiler(f: &Fixture) {
    let rustc = Tool::required("RUSTC");
    rustc.verify(f, &[&rustc]);
    for key in ["FISH_NATIVE_N8_TOOL_PATH", "FISH_NATIVE_N8_LIB"] {
        assert!(std::env::var_os(key).is_some_and(|v| !v.is_empty()), "N8-TOOLS compiler prerequisite: provision {key} for the real MSVC linker/Windows SDK");
    }
    let source = create(f, "compiled literal tool.rs", r#"use std::io::{Read,Write};
fn main() {
    assert_eq!(std::env::consts::OS, "windows");
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args, ["", "space value", "quote\"inside", "tail\\", "雪🐟", "-leading", "%PATH% ! ^ & |", "line\nfeed"]);
    assert_eq!(std::env::var("FISH_N8_VALUE").unwrap(), "exported 雪 & | % literal");
    assert_eq!(std::env::current_dir().unwrap(), std::path::PathBuf::from(std::env::var_os("FISH_N8_CWD").unwrap()));
    let mut bytes=Vec::new(); std::io::stdin().read_to_end(&mut bytes).unwrap();
    std::io::stdout().write_all(&bytes).unwrap();
    std::io::stderr().write_all(b"native-build-stderr").unwrap();
}
"#);
    let exe = f.root.join("compiled native 雪.exe");
    checked(f, &[&rustc], &format!("{} --crate-name n8_native --target x86_64-pc-windows-msvc {} -o {}", rustc.word(), qp(&source), qp(&exe)));
    assert!(exe.is_file(), "compiler did not produce its native artifact");
    let args = words(&["", "space value", "quote\"inside", "tail\\", "雪🐟", "-leading", "%PATH% ! ^ & |", "line\nfeed"]);
    let bytes = b"native compiled stdin\0\xff\r\n";
    let result = evaluate(f, &[&rustc], &format!("set -gx FISH_N8_CWD {}; {} {args}", qp(&f.root), qp(&exe)), bytes);
    success(&result);
    assert_eq!(result.out, bytes);
    assert_eq!(result.err, b"native-build-stderr");
    let bad = create(f, "deliberate compile error.rs", "fn main() { let broken: u8 = \"not a number\"; }\n");
    let rejected = evaluate(f, &[&rustc], &format!("{} --crate-name n8_error {} -o {}", rustc.word(), qp(&bad), qp(&f.root.join("must-not-build.exe"))), b"");
    assert_ne!(rejected.code, 0);
    assert!(!rejected.err.is_empty(), "compiler error must survive native stderr");
    assert!(!f.root.join("must-not-build.exe").exists());
}
fn package_manager(f: &Fixture) {
    let npm = Tool::required("NPM");
    let node = Tool::required("NODE");
    for t in [&npm, &node] { t.verify(f, &[&npm, &node]); }
    let package = f.root.join("local package 雪");
    create(f, "local package 雪/package.json", r#"{"name":"fish-n8-local","version":"1.0.0","main":"index.js"}"#);
    create(f, "local package 雪/index.js", "module.exports = value => 'package:' + value;\n");
    let app = f.root.join("package consumer 雪");
    create(f, "package consumer 雪/package.json", r#"{"name":"fish-n8-consumer","version":"1.0.0","private":true,"scripts":{"verify":"node verify.js"}}"#);
    create(f, "package consumer 雪/verify.js", r#"const assert = require('node:assert/strict');
assert.equal(process.platform, 'win32');
assert.equal(require('fish-n8-local')('雪 & % !'), 'package:雪 & % !');
assert.deepEqual(process.argv.slice(2), ['', 'space value', '雪🐟', '-leading', '! ^ & |']);
assert.equal(process.env.FISH_N8_VALUE, 'exported 雪 & | % literal');
require('node:fs').writeFileSync('package-result', 'installed-and-ran:雪');
process.stdout.write('npm-workflow-ok\n');
"#);
    let n = npm.word();
    checked(f, &[&npm, &node], &format!("cd {}; {n} pack {} --offline --ignore-scripts --pack-destination {}", qp(&app), qp(&package), qp(&app)));
    let archive = app.join("fish-n8-local-1.0.0.tgz");
    assert!(archive.is_file(), "real npm pack must create its archive");
    checked(f, &[&npm, &node], &format!("cd {}; {n} install --offline --ignore-scripts --no-audit --no-fund {}", qp(&app), qp(&archive)));
    let result = checked(f, &[&npm, &node], &format!("cd {}; {n} --silent run verify -- {}", qp(&app), words(&["", "space value", "雪🐟", "-leading", "! ^ & |"])));
    assert!(String::from_utf8_lossy(&result.out).contains("npm-workflow-ok"));
    assert_eq!(fs::read(app.join("package-result")).unwrap(), "installed-and-ran:雪".as_bytes());
    assert!(app.join("package-lock.json").is_file());
    let absent = app.join("not-installed-offline.tgz");
    let result = evaluate(f, &[&npm, &node], &format!("cd {}; {n} install --offline --ignore-scripts --no-audit --no-fund {}", qp(&app), qp(&absent)), b"");
    assert_ne!(result.code, 0);
    assert!(!result.err.is_empty(), "npm must diagnose a missing offline package");
}
fn python(f: &Fixture) {
    let python = Tool::required("PYTHON");
    python.verify(f, &[&python]);
    let venv = f.root.join("venv 雪");
    checked(f, &[&python], &format!("{} -I -m venv {}", python.word(), qp(&venv)));
    let interpreter = venv.join("Scripts/python.exe");
    assert!(interpreter.is_file(), "CPython venv/ensurepip is a mandatory N8 prerequisite");
    // Install a real local wheel using the real venv pip, without build isolation,
    // external build backends, package indexes, or a network connection.
    let wheel_script = create(f, "make local wheel.py", include_str!("tools_wheel.py"));
    let wheel = f.root.join("fish_n8_local-1.0-py3-none-any.whl");
    checked(f, &[&python], &format!("{} -I {} {}", python.word(), qp(&wheel_script), qp(&wheel)));
    checked(f, &[&python], &format!("{} -I -m pip install --no-index --no-deps {}", qp(&interpreter), qp(&wheel)));
    let script = create(f, "ordinary python script 雪.py", r#"import os,sys
import fish_n8_local
assert sys.platform == 'win32'
assert sys.prefix != sys.base_prefix
assert sys.argv[1:] == ['', 'space value', 'quote"inside', 'tail\\', '雪🐟', '-leading', '%PATH% ! ^ & |', 'line\nfeed']
assert os.environ['FISH_N8_VALUE'] == 'exported 雪 & | % literal'
assert fish_n8_local.message('雪') == 'installed:雪'
sys.stdout.buffer.write(sys.stdin.buffer.read())
sys.stderr.buffer.write(b'python-workflow-stderr')
"#);
    let args = words(&["", "space value", "quote\"inside", "tail\\", "雪🐟", "-leading", "%PATH% ! ^ & |", "line\nfeed"]);
    let bytes = b"python stdin\0\xff\r\n";
    let result = evaluate(f, &[&python], &format!("{} -I {} {args}", qp(&interpreter), qp(&script)), bytes);
    success(&result);
    assert_eq!(result.out, bytes);
    assert_eq!(result.err, b"python-workflow-stderr");
    let result = evaluate(f, &[&python], &format!("{} -I -m fish_n8_deliberately_absent", qp(&interpreter)), b"");
    assert_ne!(result.code, 0);
    assert!(String::from_utf8_lossy(&result.err).contains("fish_n8_deliberately_absent"));
}
fn powershell_script(f: &Fixture) -> PathBuf {
    create(f, "ordinary PowerShell workflow 雪.ps1", r#"$ErrorActionPreference='Stop'
if($args.Length -ne 7){throw 'literal argument count'}
$expected=@('', 'space value', 'quote"inside', 'tail\', '雪🐟', '-leading', '%PATH% ! ^ & |')
for($i=0;$i -lt $expected.Length;$i++){if($args[$i] -cne $expected[$i]){throw ('literal argument '+$i)}}
if($env:FISH_N8_VALUE -cne 'exported 雪 & | % literal'){throw 'exported environment'}
$target=Join-Path ([Environment]::CurrentDirectory) 'powershell-result.json'
@{message='serialized 雪';count=$args.Length} | ConvertTo-Json -Compress | Set-Content -LiteralPath $target -Encoding utf8NoBOM
$value=Get-Content -LiteralPath $target -Raw | ConvertFrom-Json
if($value.message -cne 'serialized 雪' -or $value.count -ne 7){throw 'JSON roundtrip'}
[Console]::WriteLine([Environment]::ProcessPath)
"#)
}
fn powershell(f: &Fixture) {
    let ps = Tool::required("PWSH");
    ps.verify(f, &[&ps]);
    let script = powershell_script(f);
    let args = words(&["", "space value", "quote\"inside", "tail\\", "雪🐟", "-leading", "%PATH% ! ^ & |"]);
    // The implicit provider must choose the same pinned production interpreter as
    // this explicit native interpreter, not whatever pwsh happens to be on PATH.
    for command in [qp(&script), format!("{} -NoLogo -NoProfile -NonInteractive -File {}", ps.word(), qp(&script))] {
        let result = checked(f, &[&ps], &format!("{command} {args}"));
        same_path(std::str::from_utf8(&result.out).unwrap(), &ps.path);
        assert!(f.root.join("powershell-result.json").is_file());
    }
    let failure = create(f, "missing module.ps1", "Import-Module FishN8DefinitelyAbsentModule -ErrorAction Stop\n");
    let result = evaluate(f, &[&ps], &qp(&failure), b"");
    assert_ne!(result.code, 0);
    assert!(String::from_utf8_lossy(&result.err).contains("FishN8DefinitelyAbsentModule"));
}
fn resolution(f: &Fixture) {
    let python = Tool::required("PYTHON");
    let ps = Tool::required("PWSH");
    let npm = Tool::required("NPM");
    let node = Tool::required("NODE");
    let git = Tool::required("GIT");
    let rustc = Tool::required("RUSTC");
    let tools = [&python, &ps, &npm, &node, &git, &rustc];
    for t in tools { t.verify(f, &tools); }
    let shadow = f.root.join("earlier conflicting interpreters");
    fs::create_dir(&shadow).unwrap();
    // These are real native programs, not mocked resolver answers. If selected,
    // the established native probe writes a marker or fails its command contract.
    for name in ["python.exe", "pwsh.exe", "powershell.exe", "cmd.exe", "node.exe", "fish.exe", "git.exe", "rustc.exe"] {
        fs::copy(&f.probe, shadow.join(name)).unwrap();
    }
    let marker = f.root.join("shadow interpreter executed");
    let setup = format!("set -gx PATH {} $PATH; set -gx COMSPEC {}", qp(&shadow), qp(&shadow.join("cmd.exe")));
    for tool in [&git, &rustc, &node] {
        let result = checked(f, &tools, &format!("{setup}; {} --version", tool.word()));
        assert_eq!(std::str::from_utf8(&result.out).unwrap().trim(), tool.version);
    }
    let result = checked(f, &tools, &format!("{setup}; {} -I -c {}", python.word(), quote("import sys;print(sys.executable)")));
    same_path(std::str::from_utf8(&result.out).unwrap(), &python.path);
    let script = create(f, "resolved powershell.ps1", "[Console]::WriteLine([Environment]::ProcessPath)\n");
    let result = checked(f, &tools, &format!("{setup}; {}", qp(&script)));
    same_path(std::str::from_utf8(&result.out).unwrap(), &ps.path);
    // Real npm's adjacent node.exe selection must beat PATH's conflicting node.
    let result = checked(f, &tools, &format!("{setup}; {} --version", npm.word()));
    assert_eq!(std::str::from_utf8(&result.out).unwrap().trim(), npm.version);
    let fish_script = create(f, "loaded fish interpreter.fish", "printf loaded-fish-ok\n");
    assert_eq!(checked(f, &tools, &format!("{setup}; {}", qp(&fish_script))).out, b"loaded-fish-ok");
    let missing = f.root.join("missing interpreter.exe");
    let result = evaluate(f, &tools, &format!("{setup}; {} {} marker {}", qp(&missing), f.probe(), qp(&marker)), b"");
    assert_ne!(result.code, 0);
    let diagnostic = String::from_utf8_lossy(&result.err);
    assert!(!diagnostic.is_empty() && diagnostic.contains("missing interpreter.exe"), "missing dependency must identify itself: {diagnostic}");
    for extension in ["unknown", "exe"] {
        let invalid = create(f, &format!("not a fallback.{extension}"), format!("{} marker {}\n", f.probe(), qp(&marker)));
        let result = evaluate(f, &tools, &format!("{setup}; {}", qp(&invalid)), b"");
        assert_ne!(result.code, 0);
        assert!(!result.err.is_empty(), "unsupported tool format must fail usefully");
        assert!(!marker.exists(), "native launch failure was retried as shell source");
    }
}
fn startup_markers(f: &Fixture) {
    let ps = Tool::required("PWSH");
    let npm = Tool::required("NPM");
    let node = Tool::required("NODE");
    let tools = [&ps, &npm, &node];
    for t in tools { t.verify(f, &tools); }
    let _lock = startup::Lock::acquire();
    let marker = f.root.join("forbidden cmd autorun marker");
    let autorun = startup::Autorun::install(&marker);
    let profile_marker = f.root.join("forbidden powershell profile marker");
    let query = checked(f, &tools, &format!("{} -NoLogo -NoProfile -NonInteractive -Command {}", ps.word(), quote("[Console]::WriteLine($PROFILE.CurrentUserAllHosts);[Console]::WriteLine($PROFILE.CurrentUserCurrentHost)")));
    let profile_paths: Vec<_> = std::str::from_utf8(&query.out).unwrap().lines().map(PathBuf::from).collect();
    assert_eq!(profile_paths.len(), 2);
    let profiles = startup::Profiles::install(&profile_paths, &profile_marker);
    let provider = create(f, "profile free.ps1", "[Console]::WriteLine('profile-free')\n");
    assert_eq!(checked(f, &tools, &qp(&provider)).out, b"profile-free\r\n");
    let n = checked(f, &tools, &format!("{} --version", npm.word()));
    assert_eq!(std::str::from_utf8(&n.out).unwrap().trim(), npm.version);
    assert!(!marker.exists(), "implicit batch interpreter ran HKCU Command Processor AutoRun");
    assert!(!profile_marker.exists(), "implicit PowerShell interpreter ran a user profile");
    // Positive controls prove these installed fixtures are live, rather than
    // relying only on absence of markers that could never have been executed.
    let system_cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
    checked(f, &tools, &format!("{} /s /v:off /c {}", qp(&system_cmd), quote("exit /b 0")));
    assert!(marker.is_file(), "AutoRun positive control did not execute");
    checked(f, &tools, &format!("{} -NoLogo -NonInteractive -Command {}", ps.word(), quote("[Console]::WriteLine('profile-positive-control')")));
    assert!(profile_marker.is_file(), "PowerShell profile positive control did not execute");
    drop(profiles);
    drop(autorun);
}
