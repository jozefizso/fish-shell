use super::{Family, fixture::{Fixture, Captured, exact, quote, run}, editor::{start, capture, finish, init}};
use std::{ffi::{OsStr, c_void}, fs, os::windows::io::AsRawHandle, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant}};
#[link(name = "kernel32")]
unsafe extern "system" { fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32; }

pub(super) const FAMILIES: &[Family] = &[
    ("native-list-delimiters-description-and-extant-filtering", lists),
    ("native-md5-stream-file-string-and-text-boundaries", hashes),
    ("native-manual-summary-alias-section-compression-and-absence", manuals),
    ("real-native-editor-buffer-source-cancel-and-absence", editors),
    ("history-exact-mutation-and-symbolic-umask-errors", history_and_umask),
    ("real-native-openssh-algorithm-list-contexts-and-absence", ssh),
    ("native-ftp-attached-script-and-mounted-drive-identity", ftp),
    ("portable-optional-tool-absence-is-not-success", absence),
    ("real-native-gpg-algorithm-key-user-and-crypto-consumers", gpg),
    ("real-native-magick-format-color-font-consumers", magick),
    ("real-native-tshark-protocol-and-heuristic-consumers", tshark),
    ("real-native-iconv-encoding-and-invalid-input", iconv),
    ("real-native-diff-equality-difference-and-error", diff),
    ("native-xdg-mime-cache-files-and-filtered-consumer", mime_types),
    ("real-native-postgresql-database-user-context-and-connection-errors", postgres),
    ("native-command-not-found-hook-argv-status-and-launch-boundaries", command_not_found),
    ("native-sudo-directory-and-subcommand-context-without-elevation", sudo),
    ("real-native-archive-members-unicode-corrupt-and-absence", archives),
];
fn required(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("required real native tool fixture {name}")));
    assert!(path.is_file(), "required real native tool fixture {name}: {}", path.display());
    path
}
pub(super) fn tool_environment(command: &mut Command, tool: &Path) {
    let windows = PathBuf::from(std::env::var_os("SystemRoot").expect("required Windows SystemRoot"));
    let system32 = windows.join("System32");
    command.env("PATH", std::env::join_paths([tool.parent().unwrap(), system32.as_path()]).unwrap())
        .env("WINDIR", &windows).env("ComSpec", system32.join("cmd.exe")).env("PYTHONUTF8", "1");
    for key in ["SystemDrive", "ProgramData", "ProgramFiles", "ProgramFiles(x86)", "CommonProgramFiles", "USERNAME", "USERDOMAIN"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
}
fn with_tool(f: &Fixture, tool: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, tool);
    run(&mut command, b"")
}
fn external(f: &Fixture, tool: &Path, args: &[&OsStr]) -> Command {
    let mut command = Command::new(tool);
    command.args(args).current_dir(&f.root).env_clear();
    for (name, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(name, value); }
    }
    tool_environment(&mut command, tool);
    command
}
fn evidence(f: &Fixture, name: &str, result: &Captured) {
    fs::write(f.root.join(format!("{name}.stdout")), &result.out).unwrap();
    fs::write(f.root.join(format!("{name}.stderr")), &result.err).unwrap();
    fs::write(f.root.join(format!("{name}.status")), result.code.to_string()).unwrap();
}
// These third-party services have native ACP/environment/socket restrictions.
// Only their declared storage is ASCII; fish and its clients retain Unicode cwd,
// arguments and payloads, and each family retains a direct-wide negative baseline.
pub(super) struct OptionalStorage { pub(super) root: PathBuf }
impl OptionalStorage {
    pub(super) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let parent = PathBuf::from(std::env::var_os("FISH_N9_OPTIONAL_WORK_ROOT").expect("required declared short ASCII optional-service storage root"));
        assert!(parent.is_absolute() && parent.is_dir() && parent.as_os_str().as_encoded_bytes().is_ascii(), "declared ASCII optional-service root: {}", parent.display());
        let root = parent.join(format!("n{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        assert!(root.as_os_str().len() < 55, "GnuPG socket namespace requires a short owned service root: {}", root.display());
        fs::create_dir(&root).unwrap();
        for directory in ["home", "config", "data", "temp"] { fs::create_dir(root.join(directory)).unwrap(); }
        Self { root }
    }
    pub(super) fn environment(&self, command: &mut Command) {
        for (name, directory) in [
            ("HOME", "home"), ("USERPROFILE", "home"), ("APPDATA", "config"),
            ("LOCALAPPDATA", "data"), ("TEMP", "temp"), ("TMP", "temp"),
        ] { command.env(name, self.root.join(directory)); }
    }
}
impl Drop for OptionalStorage {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
fn success(result: Captured) -> String {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn lists(f: &Fixture) {
    for name in ["alpha.txt", "beta.txt", "gamma.txt", "literal+.txt"] { fs::write(f.root.join(name), name).unwrap(); }
    let code = "complete -c n9list -f -a '(__fish_complete_list , \"path basename -- *.txt\")'; complete -C 'n9list alpha.txt,b'; complete -c n9dot -f -a '(__fish_complete_list . \"path basename -- *.txt\" prefix- item-)'; complete -C 'n9dot one.item-b'; function n9files; printf '%s\\tfixture file\\n' (path basename -- *.txt) | __fish_concat_completions; end; complete -c n9concat -f -a '(n9files)'; complete -C 'n9concat alpha.txt,b'; complete -C 'n9concat '";
    let text = success(f.eval(code));
    let lines: Vec<_> = text.lines().collect();
    assert!(lines.contains(&"alpha.txt,beta.txt"), "{text}");
    assert!(lines.contains(&"one.item-beta.txt"), "{text}");
    assert!(lines.contains(&"alpha.txt,beta.txt\tfixture file"), "{text}");
    assert!(lines.contains(&"literal+.txt\tfixture file"), "{text}");
    // Exact extant filtering must not treat a filename's regex metacharacters as a pattern.
    let text = success(f.eval("function n9files; printf '%s\\tfixture file\\n' (path basename -- *.txt) | __fish_concat_completions; end; complete -c n9concat -f -a '(n9files)'; complete -C 'n9concat alpha.txt,a'; complete -C 'n9concat alpha.txt,literal+'"));
    assert!(!text.lines().any(|line| line.starts_with("alpha.txt,alpha.txt")), "{text}");
    assert!(text.contains("literal+.txt"), "{text}");
}
fn hashes(f: &Fixture) {
    fs::write(f.root.join("checksum 雪.bin"), b"abc").unwrap();
    fs::write(f.root.join("million.bin"), vec![b'a'; 1_000_000]).unwrap();
    exact(f.eval("__fish_md5 -s ''; __fish_md5 -s abc; __fish_md5 'checksum 雪.bin'; __fish_md5 million.bin"), 0,
        b"d41d8cd98f00b204e9800998ecf8427e\n900150983cd24fb0d6963f7d28e17f72\n900150983cd24fb0d6963f7d28e17f72\n7707d6ae4e027c70eea2a935c2296f21\n", b"");
    exact(f.input("__fish_md5", b"abc"), 0, b"900150983cd24fb0d6963f7d28e17f72\n", b"");
    let missing = f.eval("__fish_md5 missing.bin");
    assert_ne!(missing.code, 0); assert!(missing.out.is_empty());
    let arity = f.eval("__fish_md5 bad extra");
    assert_eq!(arity.code, 2);
    assert!(arity.out.is_empty());
    fs::write(f.root.join("source 雪.cmd"), "@echo source-雪\r\n").unwrap();
    fs::write(f.root.join("binary.bin"), b"MZ\0\0binary").unwrap();
    let mut boundary = vec![b'a'; 65535]; boundary.extend_from_slice("雪".as_bytes());
    fs::write(f.root.join("boundary.txt"), boundary).unwrap();
    fs::write(f.root.join("invalid.txt"), [0xff, 0x80]).unwrap();
    fs::write(f.root.join("utf16.ps1"), [0xff, 0xfe, b'e', 0, b'x', 0, b'i', 0, b't', 0]).unwrap();
    let mut incomplete = vec![b'a'; 65536]; incomplete[65535] = 0xe9;
    fs::write(f.root.join("incomplete-at-limit.txt"), incomplete).unwrap();
    let mut utf16 = Vec::with_capacity(65538);
    utf16.extend_from_slice(&[0xff, 0xfe]);
    for _ in 0..32766 { utf16.extend_from_slice(&[b'a', 0]); }
    utf16.extend_from_slice(&[0x3d, 0xd8, 0x1f, 0xdc]);
    fs::write(f.root.join("utf16-boundary.ps1"), utf16).unwrap();
    exact(f.eval("for file in 'source 雪.cmd' binary.bin boundary.txt invalid.txt utf16.ps1 incomplete-at-limit.txt utf16-boundary.ps1; __fish_native_fs is-text $file; printf '%s\\n' $status; end"), 0, b"0\n1\n0\n1\n0\n1\n0\n", b"");
}
fn manuals(f: &Fixture) {
    let python = required("FISH_N9_PYTHON");
    let install = f.root.join("native manual install");
    let bin = install.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let fish = bin.join("fish.exe");
    fs::copy(&f.fish, &fish).unwrap();
    let root = f.root.join("manual inventory 雪");
    for section in ["man1", "man2", "man8"] { fs::create_dir_all(root.join(section)).unwrap(); }
    fs::write(root.join("man1/archive.1"), ".TH ARCHIVE 1\n.SH NAME\narchive, archive-alias \\- inspect archive files\n.SH OPTIONS\n.TP\n.B \\-l\nList contents.\n").unwrap();
    fs::write(root.join("man2/archive.2"), ".TH ARCHIVE 2\n.SH NAME\narchive \\- native file interface\n.SH DESCRIPTION\nSystem interface.\n").unwrap();
    fs::write(root.join("man8/archive-admin.8"), ".Dd October 19, 2026\n.Dt ARCHIVE-ADMIN 8\n.Sh NAME\n.Nm archive-admin\n.Nd administer archive files\n.Sh DESCRIPTION\nAdministrative interface.\n").unwrap();
    fs::write(root.join("man1/fish.1"), ".TH FISH 1\n.SH NAME\nfish \\- interactive command language\n.SH DESCRIPTION\nOwned manual fixture.\n").unwrap();
    let compressed = root.join("man1/compressed.1.gz");
    let result = run(&mut external(f, &python, &[OsStr::new("-B"), OsStr::new("-c"), OsStr::new("import gzip,sys; gzip.open(sys.argv[1], 'wb').write(b'.TH COMPRESSED 1\\n.SH NAME\\ncompressed \\\\- read compressed archives\\n.SH DESCRIPTION\\nCompressed inventory.\\n')"), compressed.as_os_str()]), b"");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let fish_man = std::env::var_os("FISH_N9_FISH_MAN_DIR").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/fish-docs/man"));
    assert!(fish_man.join("man1/source.1").is_file(), "required real generated fish manuals {}", fish_man.display());
    let installed_man = install.join("share/fish/man/man1");
    fs::create_dir_all(&installed_man).unwrap();
    for entry in fs::read_dir(fish_man.join("man1")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() { fs::copy(&path, installed_man.join(path.file_name().unwrap())).unwrap(); }
    }
    let eval = |code: &str| {
        let mut command = external(f, &fish, &[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.env("PATH", python.parent().unwrap()).env("MANPATH", std::env::join_paths([root.as_path(), install.join("share/fish/man").as_path()]).unwrap()).env("PYTHONUTF8", "1");
        run(&mut command, b"")
    };
    let text = success(eval("__fish_apropos '^archive'; __fish_describe_command --exact archive; __fish_whatis archive; __fish_apropos '^compressed$'"));
    assert!(text.contains("archive (1) - inspect archive files\n"), "{text}");
    assert!(text.contains("archive-alias (1) - inspect archive files\n"), "{text}");
    assert!(text.contains("archive-admin (8) - administer archive files\n"), "{text}");
    assert!(text.contains("archive\tinspect archive files\n"), "{text}");
    assert!(text.contains("compressed (1) - read compressed archives\n"), "{text}");
    let text = success(eval("complete -C 'man 2 arc'"));
    assert!(text.lines().any(|line| line == "archive\t2: native file interface"), "{text}");
    assert!(!text.contains("archive-alias"), "{text}");
    let text = success(eval("complete -C 'man 1 arc'; complete -C 'man 8 '"));
    assert!(text.contains("archive-alias\t1: inspect archive files"), "{text}");
    assert!(text.contains("archive-admin\t8: administer archive files"), "{text}");
    assert!(!text.contains("2: native file interface"), "{text}");
    let inventory = success(eval("complete -C 'man 1 sou'"));
    assert!(inventory.lines().any(|line| line.split('\t').next() == Some("source")), "real fish manual inventory not consumed: {inventory}");
    let excluded = success(eval("complete -C 'man 2 sou'"));
    assert!(!excluded.lines().any(|line| line.starts_with("source\t")), "fish section-one page leaked into section two: {excluded}");
    exact(eval("__fish_whatis no-such-native-manual explicit-user-description"), 0, b"explicit-user-description\n", b"");
    exact(eval("__fish_whatis no-such-native-manual"), 1, b"", b"");
    let missing = eval("__fish_apropos '^no-such-native-manual$'");
    assert_eq!(missing.code, 1); assert!(missing.out.is_empty());
    let invalid = eval("__fish_apropos '['");
    assert_eq!(invalid.code, 2);
    let setup = format!("set -gx PATH {} {}; set -gx PYTHONUTF8 1; set -gx MANPATH {}; function n9description -d 'native metadata 雪'; end; bind f2 '__fish_whatis_current_token; printf %s $status > token-status'", quote(f.root.to_str().unwrap()), quote(python.parent().unwrap().to_str().unwrap()), quote(root.to_str().unwrap()));
    let startup = init(&setup);
    let command = external(f, &fish, &[OsStr::new("-N"), OsStr::new("-i"), OsStr::new("-C"), OsStr::new(&startup)]);
    let mut session = super::console::Session::start(&command, &f.probe, &f.root);
    session.wait_protocol(true);
    for (token, expected) in [("fish", "fish (1) - interactive command language"), ("n9description", "n9description - native metadata 雪")] {
        session.send(token.as_bytes()); session.send(b"\x1bOQ");
        assert_eq!(session.wait_file(&f.root.join("token-status")), b"0");
        session.wait_text(expected.as_bytes());
        session.send(b"\x03");
        fs::remove_file(f.root.join("token-status")).unwrap();
    }
    finish(session);
    fs::write(root.join("man1/broken.1.gz"), b"not gzip").unwrap();
    let corrupt = eval("__fish_apropos '^broken$'");
    assert_eq!(corrupt.code, 1); assert!(corrupt.out.is_empty());
    let absent = f.eval("__fish_apropos '^archive$'");
    assert_eq!(absent.code, 127); assert!(absent.out.is_empty());
    let inventory_absent = with_tool(f, &python, "__fish_apropos '^archive$'");
    assert_eq!(inventory_absent.code, 1); assert!(inventory_absent.out.is_empty());
}
fn editors(f: &Fixture) {
    let editor = required("FISH_N9_EDITOR");
    let editor_command = format!("{} -u NONE -i NONE -n -es -c {} -c wq", quote(editor.to_str().unwrap()), quote("call setline(1, 'echo edited-雪')"));
    let setup = format!("set -g VISUAL {}; bind f2 'edit_command_buffer; printf %s $status > editor-status'; bind f3 'commandline -C 0; __fish_edit_command_if_at_cursor; printf %s $status > source-status'", quote(&editor_command));
    let mut session = start(f, &setup);
    session.send(b"echo original"); session.send(b"\x1bOQ");
    assert_eq!(session.wait_file(&f.root.join("editor-status")), b"0");
    capture(&mut session, f, 1, "echo edited-雪");
    session.send(b"\x03");
    fs::write(f.root.join("editable.cmd"), "@echo original\r\n").unwrap();
    session.send(b"./editable.cmd"); session.send(b"\x1bOR");
    assert_eq!(session.wait_file(&f.root.join("source-status")), b"0");
    assert_eq!(fs::read_to_string(f.root.join("editable.cmd")).unwrap().trim(), "echo edited-雪");
    capture(&mut session, f, 2, "./editable.cmd");
    session.send(b"\x03");
    let cancel = format!("{} -u NONE -i NONE -n -es -c {}", quote(editor.to_str().unwrap()), quote("cq 19"));
    let action = format!("set -g VISUAL {}; edit_command_buffer; printf %s $status > cancel-status", quote(&cancel));
    session.send(format!("bind f4 {}\r", quote(&action)).as_bytes());
    session.send(b"echo keep-cancelled"); session.send(b"\x1bOS");
    assert_eq!(session.wait_file(&f.root.join("cancel-status")), b"19");
    capture(&mut session, f, 3, "echo keep-cancelled");
    session.send(b"\x03bind f5 'set -e VISUAL EDITOR; edit_command_buffer; printf %s $status > absent-editor-status'\r");
    session.send(b"echo keep-absent"); session.send(b"\x1b[15~");
    assert_ne!(session.wait_file(&f.root.join("absent-editor-status")), b"0");
    capture(&mut session, f, 4, "echo keep-absent");
    session.send(b"\x03");
    fs::remove_file(f.root.join("source-status")).unwrap();
    session.send(quote(f.fish.to_str().unwrap()).as_bytes()); session.send(b"\x1bOR");
    assert_eq!(session.wait_file(&f.root.join("source-status")), b"1");
    finish(session);
    assert!(fs::read_dir(f.state.join("temp")).unwrap().next().is_none(), "source/buffer editor temporary resources leaked");
}
fn history_and_umask(f: &Fixture) {
    exact(f.eval("history append 'echo alpha-雪' 'ECHO alpha-雪' 'echo beta-🐟'; history search --exact --case-sensitive 'echo alpha-雪'; history delete --exact 'echo alpha-雪'; history search --exact 'echo alpha-雪'; printf 'missing:%s\\n' $status; history search --exact 'echo beta-🐟'; set -g umask 0022; umask -S; umask 'u=rw,go=r'; umask; umask -S; umask 077; umask -p; umask urw 2>/dev/null; printf 'invalid:%s:%s\\n' $status $umask"), 0,
        "echo alpha-雪\nmissing:0\necho beta-🐟\nu=rwx,g=rx,o=rx\n0133\nu=rw,g=r,o=r\numask 0077\ninvalid:1:0077\n".as_bytes(), b"");
}
fn ssh(f: &Fixture) {
    let root = PathBuf::from(std::env::var_os("SystemRoot").expect("required Windows SystemRoot"));
    let tool = root.join("System32/OpenSSH/ssh.exe");
    assert!(tool.is_file(), "required real Windows OpenSSH client {}", tool.display());
    let direct = run(&mut external(f, &tool, &[OsStr::new("-Q"), OsStr::new("mac")]), b"");
    fs::write(f.root.join("openssh-native-query.txt"), format!("executable: {}\nexit: {}\nstdout:\n{}\nstderr:\n{}", tool.display(), direct.code, String::from_utf8_lossy(&direct.out), String::from_utf8_lossy(&direct.err))).unwrap();
    assert_eq!(direct.code, 0, "genuine OpenSSH query with declared Windows runtime environment; see openssh-native-query.txt");
    let configuration = f.state.join("home/.ssh");
    fs::create_dir_all(&configuration).unwrap();
    fs::write(configuration.join("config"), "Host n9-owned-host\n  HostName 127.0.0.1\n  User n9_owned\n").unwrap();
    let text = success(with_tool(f, &tool, "complete -C 'ssh -c aes256-'; complete -C 'ssh -c aes256-ctr,aes128-'; complete -C 'ssh n9-owned-'; __fish_ssh_macs"));
    assert!(text.lines().any(|line| line.split('\t').next() == Some("aes256-ctr")), "{text}");
    assert!(text.contains("aes256-ctr,aes128-ctr"), "{text}");
    assert!(text.lines().any(|line| line == "hmac-sha2-256"), "{text}");
    assert!(text.lines().any(|line| line.split('\t').next() == Some("n9-owned-host")), "{text}");
    let missing = f.eval("__fish_complete_ssh ssh; __fish_ssh_ciphers");
    assert_eq!(missing.code, 127); assert!(missing.out.is_empty());
}
fn ftp(f: &Fixture) {
    let root = PathBuf::from(std::env::var_os("SystemRoot").expect("required Windows SystemRoot"));
    let tool = root.join("System32/ftp.exe");
    assert!(tool.is_file(), "required native Windows FTP client {}", tool.display());
    fs::write(f.root.join("n9-ftp-script.txt"), b"quit\r\n").unwrap();
    let text = success(with_tool(f, &tool, "complete -C 'ftp -s:n9-ftp-'; __fish_print_mounted"));
    let lines: Vec<_> = text.lines().collect();
    let selected = lines.iter().find_map(|line| line.split('\t').next().filter(|candidate| *candidate == "-s:n9-ftp-script.txt")).expect("native attached command-file completion");
    let result = run(&mut external(f, &tool, &[OsStr::new("-n"), OsStr::new("-i"), OsStr::new("-v"), OsStr::new(selected)]), b"");
    assert_eq!(result.code, 0, "native FTP owned quit script: {}", String::from_utf8_lossy(&result.err));
    let mounted: Vec<_> = lines.iter().filter(|line| line.ends_with(":\\")).collect();
    assert!(mounted.iter().any(|line| f.root.starts_with(Path::new(**line))), "owned fixture drive absent: {text}");
}
fn absence(f: &Fixture) {
    for code in ["__fish_complete_magick_options Format", "__fish_complete_pg_database", "__fish_complete_pg_user", "__fish_print_gpg_algo gpg Cipher", "__fish_complete_gpg_key_id gpg", "__fish_complete_gpg_user_id gpg", "__fish_complete_wireshark tshark; __fish_wireshark_protocol", "__fish_print_encodings", "__fish_print_VBox_vms", "__fish_print_packages", "diff missing-left missing-right"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0, "optional missing tool reported success: {code}");
        assert!(result.out.is_empty(), "missing tool fabricated output: {code}");
    }
}
struct GpgAgent<'a> { fixture: &'a Fixture, program: PathBuf, home: PathBuf, storage: &'a OptionalStorage }
impl Drop for GpgAgent<'_> {
    fn drop(&mut self) {
        let mut command = external(self.fixture, &self.program, &[OsStr::new("--homedir"), self.home.as_os_str(), OsStr::new("--kill"), OsStr::new("gpg-agent")]);
        self.storage.environment(&mut command);
        let result = run(&mut command, b"");
        if !std::thread::panicking() { assert_eq!(result.code, 0, "owned GnuPG agent cleanup: {}", String::from_utf8_lossy(&result.err)); }
    }
}
fn gpg(f: &Fixture) {
    let tool = required("FISH_N9_GPG");
    let unicode_home = f.root.join("GnuPG owned home 雪"); fs::create_dir(&unicode_home).unwrap();
    let mut direct = external(f, &tool, &[OsStr::new("--batch"), OsStr::new("--list-keys")]);
    direct.env("GNUPGHOME", &unicode_home);
    let baseline = run(&mut direct, b"");
    evidence(f, "gpg-direct-wide-environment-home", &baseline);
    assert_ne!(baseline.code, 0, "fixture capability changed: direct-wide Unicode GNUPGHOME now works; restore Unicode keyring coverage");
    assert!(!baseline.err.is_empty(), "native GNUPGHOME limitation must retain its diagnostic");
    let wide_argument = run(&mut external(f, &tool, &[OsStr::new("--homedir"), unicode_home.as_os_str(), OsStr::new("--batch"), OsStr::new("--list-keys")]), b"");
    evidence(f, "gpg-direct-wide-argument-home", &wide_argument);
    assert_eq!(wide_argument.code, 0, "wide argv must preserve GnuPG's supported Unicode keybox path");
    let storage = OptionalStorage::new();
    let home = storage.root.join("gpg"); fs::create_dir(&home).unwrap();
    let configuration = tool.parent().unwrap().join("gpgconf.exe");
    assert!(configuration.is_file(), "required native GnuPG agent cleanup binary {}", configuration.display());
    let _agent = GpgAgent { fixture: f, program: configuration, home: home.clone(), storage: &storage };
    let code = format!("set -gx GNUPGHOME {}; set -l cipher (complete -C 'gpg --cipher-algo AES2' | string split -f 1 \\t | string match -- AES256); set -q cipher[1]; or exit 1; command gpg --batch --yes --pinentry-mode loopback --passphrase n9-owned-only --cipher-algo $cipher --symmetric --output encrypted.gpg plain.bin; or exit; command gpg --batch --yes --pinentry-mode loopback --passphrase n9-owned-only --output decrypted.bin --decrypt encrypted.gpg; or exit; command gpg --batch --yes --pinentry-mode loopback --passphrase '' --quick-generate-key 'N9 Native: 雪 <n9-native@example.invalid>' ed25519 sign 1d; or exit; set -l key (complete -C 'gpg --trusted-key ' | string split -f 1 \\t | string match -r '^[A-F0-9]{{16}}$')[1]; set -q key[1]; or exit 1; command gpg --batch --yes --pinentry-mode loopback --passphrase '' --local-user $key --output owned.sig --detach-sign plain.bin; or exit; command gpg --batch --verify owned.sig plain.bin; or exit; complete -C 'gpg --recipient N9'; complete -C 'gpg --try-secret-key '; complete -C 'gpg --list-secret-keys N9'; complete -C 'gpg --digest-algo SHA2'; complete -C 'gpg --compress-algo Z'; complete -C 'gpg --disable-pubkey-algo ED'", quote(home.to_str().unwrap()));
    let content = "owned payload 雪 🐟\n".as_bytes(); fs::write(f.root.join("plain.bin"), content).unwrap();
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    tool_environment(&mut command, &tool);
    storage.environment(&mut command);
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert_eq!(fs::read(f.root.join("decrypted.bin")).unwrap(), content);
    assert_ne!(fs::read(f.root.join("encrypted.gpg")).unwrap(), content);
    let output = String::from_utf8(result.out).unwrap();
    assert!(output.contains("N9 Native: 雪") && output.contains("n9-native@example.invalid"), "{output}");
    assert!(output.lines().any(|line| line.split('\t').next().is_some_and(|candidate| candidate.len() == 16 && candidate.bytes().all(|byte| byte.is_ascii_hexdigit()))), "{output}");
    assert!(output.contains("SHA256") && output.contains("ZIP") && output.contains("EDDSA"), "{output}");
    let unsupported = with_tool(f, &tool, "__fish_print_gpg_algo gpg N9NonexistentAlgorithmFamily");
    assert_eq!(unsupported.code, 1); assert!(unsupported.out.is_empty());
}
fn magick(f: &Fixture) {
    let tool = required("FISH_N9_MAGICK");
    let code = "set -l formats (__fish_complete_magick_options Format | string split -f 1 \\t); set -l format (string match -- PNG $formats)[1]; set -q format[1]; or exit 1; set -l color (complete -C 'magick -background red' | string split -f 1 \\t | string match -- red)[1]; set -q color[1]; or exit 1; command magick -size 2x3 xc:$color $format':color.png'; or exit; command magick identify -format '%m %w %h' color.png; set -l fonts (complete -C 'magick -font ' | string split -f 1 \\t | string unescape); set -q fonts[1]; or exit 1; set -l families (complete -C 'magick -family ' | string split -f 1 \\t | string unescape); set -q families[1]; or exit 1; command magick -family $families[1] -font $fonts[1] -pointsize 12 label:fish font.png; or exit; set -l interlace (__fish_complete_magick_options interlace | string match -- None)[1]; set -q interlace[1]; or exit 1; command magick color.png -interlace $interlace normalized.png; or exit";
    let text = success(with_tool(f, &tool, code));
    assert_eq!(text, "PNG 2 3");
    // These are owned pixels, not a provider transcript: every selected value
    // below reaches the native CLI and is checked through image data or state.
    fs::write(f.root.join("enum.ppm"), b"P6\n2 1\n255\n\xff\0\0\0\0\xff").unwrap();
    fs::write(f.root.join("center.pgm"), b"P5\n3 3\n255\n\0\0\0\0\xff\0\0\0\0").unwrap();
    let code = r#"
function n9_magick_choice
    set -l selected (complete -C $argv[1] | string split -f 1 \t | string unescape | string match -- $argv[2])
    test (count $selected) -eq 1; or return 1
    printf '%s\n' $selected
end
set -l colorspace (n9_magick_choice 'magick -colorspace G' Gray); or exit
command magick enum.ppm -colorspace $colorspace gray.miff; or exit
command magick identify -format 'colorspace:%[colorspace]\n' gray.miff; or exit
set -l quantize (n9_magick_choice 'magick -quantize G' Gray); or exit
command magick enum.ppm -quantize $quantize +dither -colors 2 quantized.miff; or exit
command magick identify -format 'quantize:%[fx:r==g&&g==b]\n' quantized.miff; or exit
set -l compress (n9_magick_choice 'magick -compress Z' Zip); or exit
command magick enum.ppm -compress $compress compressed.miff; or exit
command magick identify -format 'compress:%C\n' compressed.miff; or exit
set -l dispose (n9_magick_choice 'magick -dispose B' Background); or exit
command magick -dispose $dispose enum.ppm enum.ppm disposed.gif; or exit
command magick identify -format 'dispose:%D:%n\n' disposed.gif; or exit
set -l dither (n9_magick_choice 'magick -dither F' FloydSteinberg); or exit
command magick -size 16x16 gradient: -dither $dither -colors 2 dithered.miff; or exit
command magick identify -format 'dither:%k\n' dithered.miff; or exit
set -l msb (n9_magick_choice 'magick -endian M' MSB); or exit
set -l lsb (n9_magick_choice 'magick -endian L' LSB); or exit
command magick -size 1x1 'xc:gray(25%)' -depth 16 -endian $msb gray:msb.raw; or exit
command magick -size 1x1 'xc:gray(25%)' -depth 16 -endian $lsb gray:lsb.raw; or exit
set -l filter (n9_magick_choice 'magick -filter P' Point); or exit
command magick enum.ppm -filter $filter -resize 4x1! -depth 8 rgb:filter.raw; or exit
set -l gravity (n9_magick_choice 'magick -gravity S' SouthEast); or exit
command magick -size 1x1 xc:red -background blue -gravity $gravity -extent 3x3 -depth 8 rgb:gravity.raw; or exit
set -l intent (n9_magick_choice 'magick -intent R' Relative); or exit
command magick enum.ppm -intent $intent intent.miff; or exit
command magick identify -verbose intent.miff | string replace -rf '^\s*Rendering intent: ' 'intent:'; or exit
set -l interlace (n9_magick_choice 'magick -interlace P' PNG); or exit
command magick enum.ppm -interlace $interlace interlaced.png; or exit
command magick identify -format 'interlace:%[interlace]\n' interlaced.png; or exit
set -l interpolate (n9_magick_choice 'magick -interpolate N' Nearest); or exit
command magick enum.ppm -interpolate $interpolate -interpolative-resize 4x1! -depth 8 rgb:interpolate.raw; or exit
set -l orient (n9_magick_choice 'magick -orient R' RightTop); or exit
command magick -size 2x3 xc:red -orient $orient oriented.miff; or exit
command magick oriented.miff -auto-orient upright.miff; or exit
command magick identify -format 'orient:%wx%h:%[orientation]\n' upright.miff; or exit
set -l units (n9_magick_choice 'magick -units P' PixelsPerInch); or exit
command magick enum.ppm -units $units -density 72 density.miff; or exit
command magick identify -format 'units:%U:%x:%y\n' density.miff; or exit
set -l virtual (n9_magick_choice 'magick -virtual-pixel T' Tile); or exit
command magick enum.ppm -virtual-pixel $virtual -fx 'p{i+1,j}' -depth 8 rgb:virtual.raw; or exit
set -l function (n9_magick_choice 'magick -function P' Polynomial); or exit
command magick enum.ppm -function $function 0,1 -depth 8 rgb:function.raw; or exit
set -l layers (n9_magick_choice 'magick -layers F' Flatten); or exit
# Isolate the blue image's page assignment; an unscoped -page also moves red.
command magick -size 2x1 xc:red '(' -size 1x1 xc:blue -set page +1+0 ')' -layers $layers +repage -depth 8 rgb:layers.raw; or exit
set -l morphology (n9_magick_choice 'magick -morphology D' Dilate); or exit
command magick center.pgm -morphology $morphology Diamond:1 -depth 8 gray:morphology.raw; or exit
set -l type (n9_magick_choice 'magick -type G' Grayscale); or exit
command magick enum.ppm -type $type typed.miff; or exit
command magick identify -format 'type:%[type]\n' typed.miff; or exit
# The list constructor is a two-stage consumer: its selected category supplies
# the colorspace used to transform real pixels, rather than just printing names.
set -l list (n9_magick_choice 'magick -list C' Colorspace); or exit
set -l listed (command magick -list $list | string trim | string match -- Gray)
contains -- Gray $listed; or exit 1
command magick enum.ppm -colorspace $listed listed.miff; or exit
command magick identify -format 'list:%[colorspace]\n' listed.miff; or exit
set -l family (complete -C 'magick -family ' | string split -f 1 \t | string unescape)[1]
set -q family[1]; or exit 1
set -l font (complete -C 'magick -font ' | string split -f 1 \t | string unescape)[1]
set -q font[1]; or exit 1
set -l direction (n9_magick_choice 'magick -direction l' left-to-right); or exit
set -l style (n9_magick_choice 'magick -style N' Normal); or exit
set -l encoding (n9_magick_choice 'magick -encoding U' Unicode); or exit
command magick -size 160x48 xc:white -family $family -style $style -direction $direction -encoding $encoding -pointsize 18 -fill black -gravity center -annotate 0 fish text.miff; or exit
command magick identify -format 'text:%w:%h:%k\n' text.miff; or exit
set -l preview (n9_magick_choice 'magick -preview G' Gamma); or exit
command magick enum.ppm -font $font -preview $preview preview.miff; or exit
command magick identify -format 'preview:%w:%h:%k\n' preview.miff; or exit
set -l debug (n9_magick_choice 'magick -debug R' Resource); or exit
set -l log (n9_magick_choice 'magick -log %e' %e); or exit
command magick -debug $debug -log $log enum.ppm debug.miff 2> magick-events.txt; or exit
command magick identify -format 'debug:%wx%h\n' debug.miff; or exit
"#;
    let text = success(with_tool(f, &tool, code));
    let lines: Vec<_> = text.lines().collect();
    for expected in ["colorspace:Gray", "quantize:1", "compress:Zip", "dispose:Background:2",
        "dither:2", "intent:Relative", "interlace:PNG", "orient:3x2:TopLeft",
        "units:PixelsPerInch:72:72", "type:Grayscale", "list:Gray", "debug:2x1"] {
        assert!(lines.contains(&expected), "native Magick state {expected:?}: {text}");
    }
    assert_eq!(lines.iter().filter(|line| **line == "dispose:Background:2").count(), 2, "both GIF frames must retain the selected disposal state");
    for prefix in ["text:", "preview:"] {
        let line = lines.iter().find_map(|line| line.strip_prefix(prefix)).unwrap_or_else(|| panic!("missing rendered image metadata {prefix}: {text}"));
        let values: Vec<u32> = line.split(':').map(|value| value.parse().unwrap()).collect();
        assert!(values.len() == 3 && values[0] > 2 && values[1] > 1 && values[2] > 1, "selected text/preview settings did not render image content: {prefix}{line}");
    }
    let msb = fs::read(f.root.join("msb.raw")).unwrap();
    let lsb = fs::read(f.root.join("lsb.raw")).unwrap();
    assert_eq!(msb.len(), 2);
    assert_eq!(lsb, [msb[1], msb[0]], "selected endian values must reverse the same Q16 sample");
    assert_ne!(msb[0], msb[1], "owned quarter-gray sample must expose byte order");
    for (name, expected) in [
        ("filter.raw", &[255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255][..]),
        ("interpolate.raw", &[255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255][..]),
        ("virtual.raw", &[0, 0, 255, 255, 0, 0][..]),
        ("function.raw", &[255, 255, 255, 255, 255, 255][..]),
        ("layers.raw", &[255, 0, 0, 0, 0, 255][..]),
        ("morphology.raw", &[0, 255, 0, 255, 255, 255, 0, 255, 0][..]),
    ] {
        assert_eq!(fs::read(f.root.join(name)).unwrap(), expected, "selected enum changed actual pixel result: {name}");
    }
    let mut gravity = [0u8, 0, 255].repeat(8); gravity.extend_from_slice(&[255, 0, 0]);
    assert_eq!(fs::read(f.root.join("gravity.raw")).unwrap(), gravity, "SouthEast must place the original pixel in the last corner");
    let events = fs::read_to_string(f.root.join("magick-events.txt")).unwrap();
    assert!(events.contains("Area:") && events.contains("Memory:"), "selected Resource events must describe real pixel-cache allocation: {events}");
    assert!(!events.contains("%e"), "selected log format was not expanded: {events}");
    // Multi-operand operators stop enum completion after their method argument.
    // In particular, kernels and polynomial coefficients are not method names.
    for (line, forbidden) in [
        ("magick -function Polynomial ", "Polynomial"),
        ("magick -morphology Dilate ", "Dilate"),
        ("magick -log ", "PNG"),
        ("magick -encoding ", "UTF-8"),
        ("magick -process ", "Point"),
    ] {
        let text = success(with_tool(f, &tool, &format!("complete -C {}", quote(line))));
        assert!(!text.lines().any(|line| line.split('\t').next() == Some(forbidden)), "wrong argument grammar offered {forbidden}: {text}");
    }
    for code in [
        "command magick enum.ppm -function Polynomial",
        "command magick enum.ppm -morphology Dilate",
        "command magick enum.ppm -colorspace N9InvalidColorspace null:",
    ] {
        let result = with_tool(f, &tool, code);
        assert_ne!(result.code, 0, "invalid native enum/operand unexpectedly accepted: {code}");
        assert!(!result.err.is_empty(), "native parser did not diagnose its invalid operand: {code}");
    }
    let invalid = with_tool(f, &tool, "__fish_complete_magick_options N9NonexistentListKind");
    assert_ne!(invalid.code, 0); assert!(invalid.out.is_empty());
    // Keep this required CLI path after the independent enum/pixel consumers:
    // a native option failure must remain visible without hiding their results.
    let stretch = with_tool(f, &tool, r#"
set -l stretch (complete -C 'magick -stretch N' | string split -f 1 \t | string unescape | string match -- Normal)
test (count $stretch) -eq 1; or exit 81
set -l family (complete -C 'magick -family ' | string split -f 1 \t | string unescape)[1]
set -q family[1]; or exit 82
command magick -size 160x48 xc:white -family $family -stretch $stretch -pointsize 18 -fill black -gravity center -annotate 0 fish stretched.miff; or exit
command magick identify -format '%w:%h:%k' stretched.miff
"#);
    evidence(f, "magick-selected-cli-stretch", &stretch);
    let text = success(stretch);
    let values: Vec<u32> = text.split(':').map(|value| value.parse().unwrap()).collect();
    assert!(values.len() == 3 && values[0] == 160 && values[1] == 48 && values[2] > 1, "selected CLI stretch must reach real text pixels: {text}");
}
fn tshark(f: &Fixture) {
    let tool = required("FISH_N9_TSHARK");
    let packet: [u8; 54] = [0xff,0xff,0xff,0xff,0xff,0xff,0,1,2,3,4,5,8,0,0x45,0,0,40,0,0,0,0,64,6,0,0,127,0,0,1,127,0,0,1,4,0xd2,0,80,0,0,0,1,0,0,0,0,0x50,2,0x20,0,0,0,0,0];
    let mut pcap = vec![0xd4,0xc3,0xb2,0xa1,2,0,4,0,0,0,0,0,0,0,0,0,0xff,0xff,0,0,1,0,0,0];
    for value in [1u32, 0, packet.len() as u32, packet.len() as u32] { pcap.extend_from_slice(&value.to_le_bytes()); }
    pcap.extend_from_slice(&packet); fs::write(f.root.join("owned.pcap"), pcap).unwrap();
    let configuration = f.root.join("Wireshark owned config 雪");
    fs::create_dir_all(configuration.join("profiles/owned profile 雪")).unwrap();
    let code = format!("set -gx WIRESHARK_CONFIG_DIR {}; __fish_complete_wireshark tshark; set -l protocols (__fish_wireshark_protocol | string split -f 1 \\t); contains tcp $protocols; or exit 1; command tshark -r owned.pcap -Y tcp -T fields -e tcp.dstport; or exit; set -l heuristics (__fish_wireshark_heuristic | string split -f 1 \\t); set -q heuristics[1]; or exit 1; command tshark --enable-heuristic $heuristics[1] -r owned.pcap -T fields -e tcp.dstport; or exit; set -l profile (complete -C 'tshark -C owned' | string split -f 1 \\t | string unescape)[1]; set -q profile[1]; or exit 1; command tshark -C $profile -r owned.pcap -T fields -e tcp.dstport; or exit; set -l preference (complete -C 'tshark -o tcp.check_checksum' | string split -f 1 \\t | string match -- tcp.check_checksum:)[1]; set -q preference[1]; or exit 1; command tshark -o $preference'FALSE' -r owned.pcap -T fields -e tcp.dstport; or exit; set -l decode (complete -C 'tshark -d tcp.port==80,htt' | string split -f 1 \\t | string match -- 'tcp.port==80,http')[1]; set -q decode[1]; or exit 1; command tshark -d $decode -r owned.pcap -T fields -e tcp.dstport; or exit; set -l resolving (complete -C 'tshark -N d' | string split -f 1 \\t | string match -- dm)[1]; set -q resolving[1]; or exit 1; command tshark -N $resolving -r owned.pcap -T fields -e tcp.dstport", quote(configuration.to_str().unwrap()));
    let result = with_tool(f, &tool, &code);
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let output = String::from_utf8(result.out).unwrap();
    assert!(output.lines().eq(["80"; 6]), "offline dissection changed selected TCP port data: {output}");
    let selector = success(with_tool(f, &tool, &format!("set -gx WIRESHARK_CONFIG_DIR {}; complete -C 'tshark -d tcp.po'", quote(configuration.to_str().unwrap()))));
    assert!(selector.lines().any(|line| line.split('\t').next() == Some("tcp.port==")), "actual decode-as selector metadata: {selector}");
    exact(with_tool(f, &tool, &format!("set -gx WIRESHARK_CONFIG_DIR {}; complete -C 'tshark -d tcp.port=='", quote(configuration.to_str().unwrap()))), 0, b"", b"");
}
fn iconv(f: &Fixture) {
    let tool = required("FISH_N9_ICONV");
    fs::write(f.root.join("latin1.txt"), [0xe9]).unwrap();
    let code = "set -l encodings (__fish_print_encodings); contains ISO-8859-1 $encodings; or exit 1; command iconv -f ISO-8859-1 -t UTF-8 latin1.txt";
    exact(with_tool(f, &tool, code), 0, "é".as_bytes(), b"");
    let invalid = with_tool(f, &tool, "command iconv -f N9NoSuchEncoding -t UTF-8 latin1.txt");
    assert_ne!(invalid.code, 0); assert!(invalid.out.is_empty());
}
fn diff(f: &Fixture) {
    let tool = required("FISH_N9_DIFF");
    fs::write(f.root.join("left 雪.txt"), b"one\n").unwrap();
    fs::write(f.root.join("right 雪.txt"), b"two\n").unwrap();
    exact(with_tool(f, &tool, "diff 'left 雪.txt' 'left 雪.txt'"), 0, b"", b"");
    let changed = with_tool(f, &tool, "diff 'left 雪.txt' 'right 雪.txt'");
    assert_eq!(changed.code, 1); assert!(changed.out.windows(3).any(|bytes| bytes == b"one")); assert!(changed.out.windows(3).any(|bytes| bytes == b"two"));
    let missing = with_tool(f, &tool, "diff 'left 雪.txt' missing.txt");
    assert_eq!(missing.code, 2);
}
fn mime_types(f: &Fixture) {
    let first = f.root.join("MIME user data 雪");
    let second = f.root.join("MIME shared data 🐟");
    for directory in [&first, &second] { fs::create_dir_all(directory.join("applications")).unwrap(); }
    fs::write(first.join("applications/mimeinfo.cache"), "[MIME Cache]\nvideo/x-n9-native=owned-video.desktop;\napplication/x-n9-native=owned-data.desktop;\n").unwrap();
    fs::write(second.join("applications/mimeinfo.cache"), "[MIME Cache]\nvideo/x-n9-shared=shared-video.desktop;\n").unwrap();
    let code = format!("set -gx XDG_DATA_HOME {}; set -gx XDG_DATA_DIRS {}; status get-file completions/castnow.fish | source; complete -C 'castnow --type video/x-n9-'", quote(first.to_str().unwrap()), quote(second.to_str().unwrap()));
    let text = success(f.eval(&code));
    assert!(text.contains("video/x-n9-native\towned-video.desktop;"), "{text}");
    assert!(text.contains("video/x-n9-shared\tshared-video.desktop;"), "{text}");
    assert!(!text.contains("application/x-n9-native") && !text.contains("[MIME Cache]"), "{text}");
    let svn = required("FISH_N9_SVN");
    let svnadmin = svn.parent().unwrap().join("svnadmin.exe");
    assert!(svnadmin.is_file(), "required actual native SVN repository fixture tool {}", svnadmin.display());
    let unicode_repository = f.root.join("MIME repository 雪");
    let baseline = run(&mut external(f, &svnadmin, &[OsStr::new("create"), unicode_repository.as_os_str()]), b"");
    evidence(f, "svnadmin-direct-wide-unicode-repository", &baseline);
    assert_ne!(baseline.code, 0, "fixture capability changed: restore native Unicode SVN repository storage");
    assert!(!baseline.err.is_empty(), "native repository path limitation must retain its diagnostic");
    let storage = OptionalStorage::new();
    let repository = storage.root.join("repo");
    let checkout = storage.root.join("wc");
    let configuration = storage.root.join("config");
    let mut uri = String::from("file:///");
    for byte in repository.to_str().unwrap().replace('\\', "/").bytes() {
        if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) { uri.push(byte as char); }
        else { uri.push_str(&format!("%{byte:02X}")); }
    }
    let created = run(&mut external(f, &svnadmin, &[OsStr::new("create"), repository.as_os_str()]), b"");
    assert_eq!(created.code, 0, "real owned MIME repository: {}", String::from_utf8_lossy(&created.err));
    let checked_out = run(&mut external(f, &svn, &[OsStr::new("--config-dir"), configuration.as_os_str(), OsStr::new("checkout"), OsStr::new("--quiet"), OsStr::new("--non-interactive"), OsStr::new(&uri), checkout.as_os_str()]), b"");
    assert_eq!(checked_out.code, 0, "real owned MIME checkout: {}", String::from_utf8_lossy(&checked_out.err));
    // APR rejects Unicode filesystem argv in this fixture; keep the native
    // negative baseline above, Unicode client cwd, and actual UTF-8 file data.
    let file = checkout.join("owned.bin");
    let payload = "owned native MIME payload 雪 🐟\n".as_bytes();
    fs::write(&file, payload).unwrap();
    let code = format!("set -gx XDG_DATA_HOME {}; set -gx XDG_DATA_DIRS {}; command svn --config-dir {} add --quiet -- {}; or exit; set -l mime (complete -C 'svn propset svn:mime-type application/x-n9-' | string split -f 1 \\t | string match -- application/x-n9-native)[1]; set -q mime[1]; or exit 1; command svn --config-dir {} propset --quiet svn:mime-type $mime {}; or exit; command svn --config-dir {} propget --strict svn:mime-type {}", quote(first.to_str().unwrap()), quote(second.to_str().unwrap()), quote(configuration.to_str().unwrap()), quote(file.to_str().unwrap()), quote(configuration.to_str().unwrap()), quote(file.to_str().unwrap()), quote(configuration.to_str().unwrap()), quote(file.to_str().unwrap()));
    let result = with_tool(f, &svn, &code);
    assert_eq!(result.code, 0, "real MIME property mutation: {}", String::from_utf8_lossy(&result.err));
    assert_eq!(result.out, b"application/x-n9-native");
    assert_eq!(fs::read(&file).unwrap(), payload, "native MIME property mutation must preserve actual Unicode data");
    fs::remove_file(first.join("applications/mimeinfo.cache")).unwrap();
    fs::remove_file(second.join("applications/mimeinfo.cache")).unwrap();
    let code = format!("set -gx XDG_DATA_HOME {}; set -gx XDG_DATA_DIRS {}; __fish_print_xdg_mimetypes", quote(first.to_str().unwrap()), quote(second.to_str().unwrap()));
    exact(f.eval(&code), 1, b"", b"");
}
struct Postgres<'a> { fixture: &'a Fixture, control: PathBuf, data: PathBuf, storage: &'a OptionalStorage, child: Child }
impl Drop for Postgres<'_> {
    fn drop(&mut self) {
        if self.child.try_wait().expect("observe retained PostgreSQL process").is_some() { return; }
        let owned_pid = fs::read_to_string(self.data.join("postmaster.pid")).ok()
            .and_then(|text| text.lines().next().and_then(|pid| pid.parse::<u32>().ok())) == Some(self.child.id());
        let stopped = if owned_pid {
            let mut command = external(self.fixture, &self.control, &[OsStr::new("-D"), self.data.as_os_str(), OsStr::new("-w"), OsStr::new("-t"), OsStr::new("5"), OsStr::new("stop"), OsStr::new("-m"), OsStr::new("immediate")]);
            self.storage.environment(&mut command);
            let result = run(&mut command, b"");
            evidence(self.fixture, "postgres-owned-server-stop", &result);
            Some(result)
        } else { None };
        let waited = unsafe { WaitForSingleObject(self.child.as_raw_handle(), 5_000) };
        let forced = waited == 0x102;
        if forced { self.child.kill().expect("terminate only retained PostgreSQL process"); }
        else { assert_eq!(waited, 0, "observe owned PostgreSQL shutdown"); }
        assert_eq!(unsafe { WaitForSingleObject(self.child.as_raw_handle(), 5_000) }, 0, "owned PostgreSQL process must finish bounded cleanup");
        self.child.wait().expect("collect retained PostgreSQL process");
        if !std::thread::panicking() {
            assert!(owned_pid, "refuse pg_ctl shutdown without the retained process's datadir identity");
            let result = stopped.unwrap();
            assert_eq!(result.code, 0, "owned PostgreSQL shutdown: {}", String::from_utf8_lossy(&result.err));
            assert!(!forced, "owned PostgreSQL did not stop through its bounded shutdown protocol");
        }
    }
}
fn postgres(f: &Fixture) {
    let tool = required("FISH_N9_PSQL");
    let bin = tool.parent().unwrap();
    let initialize = bin.join("initdb.exe");
    let control = bin.join("pg_ctl.exe");
    let daemon = bin.join("postgres.exe");
    for required in [&initialize, &control, &daemon] { assert!(required.is_file(), "required owned PostgreSQL server binary {}", required.display()); }
    let unicode_data = f.root.join("owned PostgreSQL data 雪");
    let baseline = run(&mut external(f, &initialize, &[OsStr::new("-D"), unicode_data.as_os_str(), OsStr::new("-A"), OsStr::new("trust"), OsStr::new("--username=n9_owner"), OsStr::new("--encoding=UTF8"), OsStr::new("--no-locale")]), b"");
    evidence(f, "postgres-initdb-direct-wide-unicode-data", &baseline);
    assert_ne!(baseline.code, 0, "fixture capability changed: restore native Unicode PostgreSQL cluster storage");
    assert!(!baseline.err.is_empty(), "native datadir limitation must retain its diagnostic");
    let storage = OptionalStorage::new();
    let data = storage.root.join("cluster");
    let mut command = external(f, &initialize, &[OsStr::new("-D"), data.as_os_str(), OsStr::new("-A"), OsStr::new("trust"), OsStr::new("--username=n9_owner"), OsStr::new("--encoding=UTF8"), OsStr::new("--no-locale")]);
    storage.environment(&mut command);
    let result = run(&mut command, b"");
    evidence(f, "postgres-initdb-ascii-storage", &result);
    assert_eq!(result.code, 0, "owned PostgreSQL initdb: {}", String::from_utf8_lossy(&result.err));
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = f.root.join("owned-postgres.log");
    let output = fs::File::create(&log).unwrap();
    // Retain the foreground postmaster. A pg_ctl daemon can inherit the client's
    // captured pipes and keep them open after its launcher has already exited.
    let mut command = external(f, &daemon, &[OsStr::new("-D"), data.as_os_str(), OsStr::new("-h"), OsStr::new("127.0.0.1"), OsStr::new("-p"), OsStr::new(&port.to_string()), OsStr::new("-F")]);
    storage.environment(&mut command);
    command.stdin(Stdio::null()).stdout(output.try_clone().unwrap()).stderr(output);
    drop(listener);
    let mut server = Postgres { fixture: f, control, data, storage: &storage, child: command.spawn().expect("start retained owned PostgreSQL postmaster") };
    let deadline = Instant::now() + Duration::from_secs(10);
    let pidfile = loop {
        assert!(server.child.try_wait().expect("observe PostgreSQL startup").is_none(), "owned PostgreSQL exited during startup: {}", fs::read_to_string(&log).unwrap());
        let text = match fs::read_to_string(server.data.join("postmaster.pid")) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => panic!("read owned PostgreSQL startup identity: {error}"),
        };
        if text.lines().nth(7).map(str::trim) == Some("ready") { break text; }
        assert!(Instant::now() < deadline, "owned PostgreSQL startup did not publish readiness: {}", fs::read_to_string(&log).unwrap());
        std::thread::sleep(Duration::from_millis(25));
    };
    fs::write(f.root.join("postgres-owned-server-start.pid"), &pidfile).unwrap();
    let identity: Vec<_> = pidfile.lines().collect();
    assert_eq!(identity[0].parse::<u32>().unwrap(), server.child.id(), "datadir must belong to the retained process");
    assert_eq!(fs::canonicalize(identity[1]).unwrap(), fs::canonicalize(&server.data).unwrap(), "postmaster must own only the declared datadir");
    assert_eq!(identity[3].parse::<u16>().unwrap(), port, "postmaster must own the reserved port");
    assert_eq!(identity[5], "127.0.0.1", "owned server must listen only on loopback");
    let connection = format!("set -gx PGHOST 127.0.0.1; set -gx PGPORT {port}; set -gx PGUSER n9_owner; set -gx PGDATABASE postgres; set -gx PGCLIENTENCODING UTF8; set -gx PGCONNECT_TIMEOUT 3;");
    let identity_sql = "SELECT current_setting('data_directory'), host(inet_server_addr()), inet_server_port()::text";
    let identity = with_tool(f, &tool, &format!("{connection} command psql -XAtqw -F '|' -c {}", quote(identity_sql)));
    evidence(f, "postgres-owned-server-identity", &identity);
    assert_eq!(identity.code, 0, "read-only PostgreSQL ownership query: {}", String::from_utf8_lossy(&identity.err));
    let identity = String::from_utf8(identity.out).unwrap();
    let identity: Vec<_> = identity.trim_end_matches(['\r', '\n']).split('|').collect();
    assert_eq!(identity.len(), 3, "real PostgreSQL ownership fields");
    assert_eq!(fs::canonicalize(identity[0]).unwrap(), fs::canonicalize(&server.data).unwrap(), "refuse SQL mutation outside the owned datadir");
    assert_eq!(identity[1], "127.0.0.1", "refuse SQL mutation through a non-loopback endpoint");
    assert_eq!(identity[2].parse::<u16>().unwrap(), port, "refuse SQL mutation on a different service port");
    let direct = |args: &[&OsStr], input: &[u8]| {
        let mut command = external(f, &tool, args);
        command.env("PGHOST", "127.0.0.1").env("PGPORT", port.to_string())
            .env("PGUSER", "n9_owner").env("PGDATABASE", "postgres")
            .env("PGCLIENTENCODING", "UTF8").env("PGCONNECT_TIMEOUT", "3");
        storage.environment(&mut command);
        run(&mut command, input)
    };
    // This publisher's narrow-main argv loses Unicode before SQL parsing, not
    // when rendering UTF8 results. Keep both the raw-argv and stored-byte proof.
    let raw_sql = direct(&[OsStr::new("-XAtqw"), OsStr::new("-v"), OsStr::new("ON_ERROR_STOP=1"),
        OsStr::new("-c"), OsStr::new("SELECT current_setting('client_encoding'), current_setting('server_encoding'), encode(convert_to('雪', 'UTF8'), 'hex')"),
        OsStr::new("-c"), OsStr::new("CREATE DATABASE \"n9 argv 雪\""),
        OsStr::new("-c"), OsStr::new("SELECT datname, encode(convert_to(datname, 'UTF8'), 'hex') FROM pg_database WHERE datname LIKE 'n9 argv %'")], b"");
    evidence(f, "postgres-direct-wide-unicode-sql-storage", &raw_sql);
    assert_eq!(success(raw_sql).lines().collect::<Vec<_>>(), ["UTF8|UTF8|3f", "n9 argv ?|6e392061726776203f"], "fixture argv capability changed: restore raw Unicode SQL arguments");
    // SQL stdin is a public byte-preserving interface, unlike this native argv.
    let setup = format!("{connection} printf '%s\\n' {} | command psql -Xqw -v ON_ERROR_STOP=1", quote("CREATE ROLE \"n9_reader_雪\" LOGIN; CREATE ROLE \"n9_group_雪\" NOLOGIN; CREATE DATABASE \"n9 data 雪\" OWNER \"n9_reader_雪\";"));
    let setup = with_tool(f, &tool, &setup);
    evidence(f, "postgres-utf8-stdin-setup", &setup);
    assert!(success(setup).is_empty());
    let metadata = direct(&[OsStr::new("-XAtqw"), OsStr::new("-c"), OsStr::new("SELECT encode(convert_to(datname, 'UTF8'), 'hex') FROM pg_database WHERE datname LIKE 'n9 data %'; SELECT encode(convert_to(rolname, 'UTF8'), 'hex') FROM pg_roles WHERE rolname LIKE 'n9_reader_%'")], b"");
    evidence(f, "postgres-utf8-stored-identifiers", &metadata);
    assert_eq!(success(metadata).lines().collect::<Vec<_>>(), ["6e39206461746120e99baa", "6e395f7265616465725fe99baa"]);
    for (name, flag, value) in [
        ("postgres-direct-wide-unicode-database-argv", "-d", "n9 data 雪"),
        ("postgres-direct-wide-unicode-user-argv", "-U", "n9_reader_雪"),
    ] {
        let baseline = direct(&[OsStr::new("-XAtqw"), OsStr::new(flag), OsStr::new(value), OsStr::new("-c"), OsStr::new("SELECT current_database(), current_user")], b"");
        evidence(f, name, &baseline);
        assert_ne!(baseline.code, 0, "fixture argv capability changed: restore raw Unicode connection arguments");
        assert!(baseline.out.is_empty());
        assert!(String::from_utf8_lossy(&baseline.err).contains(&value.replace('雪', "?")), "native connection argv limitation must retain the lossy identifier diagnostic");
    }
    let code = format!("{connection} __fish_complete_pg_database; __fish_complete_pg_user; complete -C 'psql -d n9'; complete -C 'psql -U n9_r'");
    let completion = with_tool(f, &tool, &code);
    evidence(f, "postgres-unicode-completions", &completion);
    let text = success(completion);
    assert!(text.lines().any(|line| line == "n9 data 雪"), "database whitespace was truncated: {text}");
    assert!(text.lines().any(|line| line == "n9_reader_雪"), "database user missing: {text}");
    assert!(!text.contains("n9_group_雪"), "non-login role was offered as a connection user: {text}");
    // Percent-encoded libpq URIs preserve both selected names through narrow
    // argv. Neither the database/user completion nor the SQL payload is ASCII.
    let sql = format!("{connection} set -l database (complete -C 'psql -d n9' | string split -f 1 \\t | string unescape | string match -- 'n9 data 雪')[1]; set -q database[1]; or exit 1; set -l user (complete -C 'psql -U n9_r' | string split -f 1 \\t | string unescape | string match -- 'n9_reader_雪')[1]; set -q user[1]; or exit 1; set -l uri postgresql://(string escape --style=url -- $user)@127.0.0.1:{port}/(string escape --style=url -- $database); printf '%s\\n' {} | command psql -XAtqw -v ON_ERROR_STOP=1 -F '|' -d $uri", quote("CREATE TABLE owned_payload (value text); INSERT INTO owned_payload VALUES ('owned SQL 雪 🐟'); SELECT current_database(), current_user, value, encode(convert_to(value, 'UTF8'), 'hex') FROM owned_payload;"));
    let consumer = with_tool(f, &tool, &sql);
    evidence(f, "postgres-selected-unicode-uri-stdin-consumer", &consumer);
    let rows = success(consumer);
    assert_eq!(rows.lines().collect::<Vec<_>>(), ["n9 data 雪|n9_reader_雪|owned SQL 雪 🐟|6f776e65642053514c20e99baa20f09f909f"]);
    let missing_user = with_tool(f, &tool, &format!("{connection} set -gx PGUSER n9_missing_user; __fish_complete_pg_database"));
    assert_ne!(missing_user.code, 0); assert!(missing_user.out.is_empty());
    drop(server);
    let disconnected = with_tool(f, &tool, &format!("{connection} __fish_complete_pg_user"));
    assert_ne!(disconnected.code, 0); assert!(disconnected.out.is_empty());
}
fn command_not_found(f: &Fixture) {
    let missing = f.eval("n9_missing_native_default 'payload 雪'");
    assert_eq!(missing.code, 127);
    assert!(missing.out.is_empty());
    let code = "function fish_command_not_found; printf '%s\\0' $argv > hook-arguments; printf 'owned hook output\\n'; return 37; end; n9_missing_native_custom 'argument 雪' '' 'semi;quote\\'value'; printf 'after:%s\\n' $status";
    let result = f.eval(code);
    assert_eq!(result.code, 0);
    assert_eq!(result.out, b"after:127\n");
    assert!(String::from_utf8_lossy(&result.err).contains("owned hook output\n"), "hook stdout did not follow diagnostic redirection");
    assert_eq!(fs::read(f.root.join("hook-arguments")).unwrap(), "n9_missing_native_custom\0argument 雪\0\0semi;quote'value\0".as_bytes());
    fs::remove_file(f.root.join("hook-arguments")).unwrap();
    fs::write(f.root.join("not-a-native-image.exe"), b"not a Windows executable image").unwrap();
    let result = f.eval("function fish_command_not_found; printf '%s\\0' $argv > hook-arguments; return 37; end; ./not-a-native-image.exe 'argument 雪'");
    assert_eq!(result.code, 126, "{}", String::from_utf8_lossy(&result.err));
    assert!(!f.root.join("hook-arguments").exists(), "invalid-image launch was mislabeled as command-not-found");
}
fn sudo(f: &Fixture) {
    let root = PathBuf::from(std::env::var_os("SystemRoot").expect("required Windows SystemRoot"));
    let tool = root.join("System32/sudo.exe");
    assert!(tool.is_file(), "required real native Windows sudo {}", tool.display());
    let version = run(&mut external(f, &tool, &[OsStr::new("--version")]), b"");
    assert_eq!(version.code, 0, "query-only sudo version: {}", String::from_utf8_lossy(&version.err));
    fs::create_dir(f.root.join("sudo directory 雪")).unwrap();
    let evaluate = |code: &str| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.env("PATH", std::env::join_paths([tool.parent().unwrap(), &f.root]).unwrap());
        run(&mut command, b"")
    };
    let directory = success(evaluate("complete -C 'sudo --chdir sudo'"));
    assert!(directory.contains("sudo directory 雪"), "{directory}");
    for line in ["sudo --chdir \"sudo directory 雪\" fish --no-c", "sudo run --chdir \"sudo directory 雪\" fish --no-c"] {
        let text = success(evaluate(&format!("complete -C {}", quote(line))));
        assert!(text.lines().any(|line| line.split('\t').next() == Some("--no-config")), "quoted native sudo process context: {text}");
    }
    let reserved = success(evaluate("complete -C 'sudo config --enable normal fish --no-c'"));
    assert!(!reserved.contains("--no-config"), "configuration grammar was mistaken for an elevated process: {reserved}");
}
fn archives(f: &Fixture) {
    let tool = required("FISH_N9_7Z");
    let directory = f.root.join("archive source 雪");
    fs::create_dir_all(directory.join("nested")).unwrap();
    let content = "owned archive member 雪 🐟\n".as_bytes();
    fs::write(directory.join("member space 雪.txt"), content).unwrap();
    fs::write(directory.join("nested/second.txt"), b"second\n").unwrap();
    let archive = f.root.join("archive.zip");
    let mut create = external(f, &tool, &[OsStr::new("a"), OsStr::new("-tzip"), archive.as_os_str(), OsStr::new("member space 雪.txt"), OsStr::new("nested/second.txt")]);
    create.current_dir(&directory);
    let result = run(&mut create, b"");
    assert_eq!(result.code, 0, "real archive fixture creation: {}", String::from_utf8_lossy(&result.err));
    let result = with_tool(f, &tool, "status get-file completions/acat.fish | source; set -l member (complete -C 'acat archive.zip member' | string split -f 1 \\t | string unescape); set -q member[1]; or exit 1; command 7z e -so -sccUTF-8 -- archive.zip $member[1]");
    assert_eq!(result.code, 0, "real archive candidate extraction: {}", String::from_utf8_lossy(&result.err));
    assert_eq!(result.out, content);
    fs::write(f.root.join("corrupt.zip"), b"not an archive").unwrap();
    let context = "function n9_archive_members; __fish_complete_atool_archive_contents; printf %s $status > archive-status; end; complete -c n9archive -f -a '(n9_archive_members)'; complete -C 'n9archive corrupt.zip '";
    let result = with_tool(f, &tool, context);
    assert_eq!(result.code, 0);
    assert!(result.out.is_empty(), "corrupt archive fabricated members");
    assert_ne!(fs::read(f.root.join("archive-status")).unwrap(), b"0");
    fs::remove_file(f.root.join("archive-status")).unwrap();
    let absent = f.eval("function n9_archive_members; __fish_complete_atool_archive_contents; printf %s $status > archive-status; end; complete -c n9archive -f -a '(n9_archive_members)'; complete -C 'n9archive archive.zip '");
    assert_eq!(absent.code, 0);
    assert!(absent.out.is_empty(), "absent archive tool fabricated members");
    assert_eq!(fs::read(f.root.join("archive-status")).unwrap(), b"127");
}
