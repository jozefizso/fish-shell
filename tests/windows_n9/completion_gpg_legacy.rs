use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, fs, path::{Path, PathBuf}, process::Command};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-gpg1-helper-branch-key-user-signature-trust-and-literal-consumers", legacy),
];

#[repr(C)]
struct Guid { first: u32, second: u16, third: u16, tail: [u8; 8] }
#[link(name = "ole32")]
unsafe extern "system" { fn CoCreateGuid(guid: *mut Guid) -> i32; }

// Only service storage is ASCII. The identity, fish cwd, source filename and
// payload remain Unicode. No secret material is included in assertion output.
struct OwnedHomes<'a> {
    fixture: &'a Fixture,
    root: PathBuf,
    modern: PathBuf,
    legacy: PathBuf,
    modern_program: PathBuf,
}
impl<'a> OwnedHomes<'a> {
    fn new(fixture: &'a Fixture, modern_program: PathBuf) -> Self {
        let parent = PathBuf::from(std::env::var_os("FISH_N9_OPTIONAL_WORK_ROOT")
            .expect("required declared short ASCII service storage root"));
        assert!(parent.is_absolute() && parent.is_dir() && parent.as_os_str().as_encoded_bytes().is_ascii());
        let mut guid = Guid { first: 0, second: 0, third: 0, tail: [0; 8] };
        assert_eq!(unsafe { CoCreateGuid(&mut guid) }, 0, "create owned service UUID");
        // Compact UUID spelling leaves space for GnuPG's native socket suffix.
        let root = parent.join(format!("{:08x}{:04x}{:04x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            guid.first, guid.second, guid.third, guid.tail[0], guid.tail[1], guid.tail[2], guid.tail[3],
            guid.tail[4], guid.tail[5], guid.tail[6], guid.tail[7]));
        assert!(root.as_os_str().len() <= 39, "declared root must leave room for GnuPG's extra socket: {}", root.display());
        fs::create_dir(&root).unwrap();
        fs::write(root.join("owner"), b"GOAL_09 owned legacy GPG interop UUID\n").unwrap();
        let homes = Self { fixture, modern: root.join("m"), legacy: root.join("l"), root, modern_program };
        fs::create_dir(&homes.modern).unwrap();
        fs::create_dir(&homes.legacy).unwrap();
        homes
    }
    fn environment(&self, command: &mut Command, home: &Path) {
        for name in ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"] {
            command.env(name, &self.modern);
        }
        command.env("GNUPGHOME", home);
    }
}
impl Drop for OwnedHomes<'_> {
    fn drop(&mut self) {
        let configuration = self.modern_program.with_file_name("gpgconf.exe");
        let mut command = native_command(self.fixture, &configuration);
        self.environment(&mut command, &self.modern);
        command.args([OsStr::new("--homedir"), self.modern.as_os_str(), OsStr::new("--kill"), OsStr::new("gpg-agent")]);
        let stopped = run(&mut command, b"");
        let removed = fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            assert_eq!(stopped.code, 0, "owned GnuPG agent cleanup: {}", String::from_utf8_lossy(&stopped.err));
            removed.unwrap();
        }
    }
}
fn required(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("required genuine native fixture {name}")));
    assert!(path.is_absolute() && path.is_file(), "required absolute native fixture {name}: {}", path.display());
    path
}
fn native_command(f: &Fixture, program: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(&f.root).env_clear();
    for (name, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(name, value); }
    }
    tool_environment(&mut command, program);
    command
}
fn native(homes: &OwnedHomes<'_>, program: &Path, home: &Path, args: &[&OsStr], input: &[u8]) -> Captured {
    let mut command = native_command(homes.fixture, program);
    homes.environment(&mut command, home);
    command.args([OsStr::new("--homedir"), home.as_os_str(), OsStr::new("--batch"), OsStr::new("--no-tty")]).args(args);
    run(&mut command, input)
}
fn evaluate(homes: &OwnedHomes<'_>, program: &Path, code: &str) -> Captured {
    let mut command = homes.fixture.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, program);
    homes.environment(&mut command, &homes.legacy);
    run(&mut command, b"")
}
fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine GPG consumer: {}", String::from_utf8_lossy(&result.err));
}
fn status(data: &[u8], tag: &str, value: Option<&str>) -> bool {
    String::from_utf8_lossy(data).lines().filter_map(|line| line.strip_prefix("[GNUPG:] "))
        .any(|record| {
            let mut fields = record.split_whitespace();
            fields.next() == Some(tag) && value.is_none_or(|expected| fields.next() == Some(expected))
        })
}
const PICK: &str = r#"
function __n9_legacy_pick
    set -l selected (complete -C $argv[1] | string split -f 1 \t | string unescape | string match -- $argv[2])
    test (count $selected) -eq 1; or return 81
    printf '%s\n' $selected
end
"#;

fn legacy(f: &Fixture) {
    let tool = required("FISH_N9_GPG_LEGACY");
    assert_eq!(tool.file_name(), Some(OsStr::new("gpg.exe")), "publisher ships gpg.exe, not an invented gpg1 alias");
    let modern = required("FISH_N9_GPG");
    assert!(modern.with_file_name("gpgconf.exe").is_file());
    let homes = OwnedHomes::new(f, modern.clone());
    let home = quote(homes.legacy.to_str().unwrap());
    let modern_home = quote(homes.modern.to_str().unwrap());
    let modern_command = quote(modern.to_str().unwrap());

    let resolved = evaluate(&homes, &tool, "command -s gpg");
    success(&resolved);
    assert_eq!(fs::canonicalize(PathBuf::from(String::from_utf8(resolved.out).unwrap().trim())).unwrap(), fs::canonicalize(&tool).unwrap());
    let version = native(&homes, &tool, &homes.legacy, &[OsStr::new("--version")], b"");
    success(&version);
    assert!(String::from_utf8(version.out).unwrap().lines().next().is_some_and(|line| line.starts_with("gpg (GnuPG) 1.")));

    let metadata = evaluate(&homes, &tool, "complete -C 'gpg --'");
    success(&metadata);
    let metadata = String::from_utf8(metadata.out).unwrap();
    for option in ["--simple-sk-checksum", "--no-sig-create-check", "--pgp2", "--rfc1991"] {
        assert!(metadata.lines().any(|line| line.split('\t').next() == Some(option)), "missing actual GnuPG1 branch option {option}");
    }
    for option in ["--pinentry-mode", "--quick-gen-key", "--no-keyring"] {
        assert!(!metadata.lines().any(|line| line.split('\t').next() == Some(option)), "modern-only metadata leaked into GnuPG1: {option}");
    }
    let rejected = native(&homes, &tool, &homes.legacy,
        &[OsStr::new("--pinentry-mode"), OsStr::new("loopback"), OsStr::new("--list-keys")], b"");
    assert_eq!(rejected.code, 2);
    assert!(rejected.out.is_empty() && !rejected.err.is_empty());

    // Real modern key generation is fixture preparation only. GnuPG1's own
    // RSA2048 generation and strong-entropy timeouts remain separate obligations.
    let identity = "N9 Legacy: 雪: 🐟";
    let batch = format!("Key-Type: RSA\nKey-Length: 2048\nKey-Usage: sign encrypt\nName-Real: {identity}\nName-Email: n9-legacy@example.invalid\nExpire-Date: 1d\n%no-protection\n%commit\n");
    let generated = native(&homes, &modern, &homes.modern,
        &[OsStr::new("--pinentry-mode"), OsStr::new("loopback"), OsStr::new("--gen-key")], batch.as_bytes());
    success(&generated);
    assert!(generated.out.is_empty());
    let exported = native(&homes, &modern, &homes.modern,
        &[OsStr::new("--export"), OsStr::new("n9-legacy@example.invalid")], b"");
    success(&exported);
    let imported = native(&homes, &tool, &homes.legacy, &[OsStr::new("--import")], &exported.out);
    success(&imported);
    let records = native(&homes, &tool, &homes.legacy,
        &[OsStr::new("--with-colons"), OsStr::new("--fixed-list-mode"), OsStr::new("--fingerprint"), OsStr::new("--list-keys")], b"");
    success(&records);
    let records = String::from_utf8(records.out).unwrap();
    let public: Vec<_> = records.lines().filter(|line| line.starts_with("pub:")).collect();
    assert_eq!(public.len(), 1);
    let fields: Vec<_> = public[0].split(':').collect();
    assert_eq!((fields[2], fields[3]), ("2048", "1"), "real RSA2048 public key required");
    let key = fields[4];
    assert_eq!(key.len(), 16);
    assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let fingerprints: Vec<_> = records.lines().filter(|line| line.starts_with("fpr:")).map(|line| line.split(':').nth(9).unwrap()).collect();
    assert_eq!(fingerprints.len(), 1);
    let fingerprint = fingerprints[0];
    assert_eq!(fingerprint.len(), 40);
    assert!(fingerprint.ends_with(key));
    let users: Vec<_> = records.lines().filter(|line| line.starts_with("uid:")).map(|line| line.split(':').nth(9).unwrap().replace("\\x3a", ":")).collect();
    assert_eq!(users, [format!("{identity} <n9-legacy@example.invalid>")]);

    let payload = "owned legacy payload 雪 🐟\r\n\0binary suffix\n".as_bytes();
    let payload_name = "legacy payload 雪 🐟.bin";
    fs::write(f.root.join(payload_name), payload).unwrap();
    let signed = evaluate(&homes, &tool, &format!(r#"{PICK}
set -l user (__n9_legacy_pick 'gpg --local-user N9' {user}); or exit
command {modern_command} --homedir {modern_home} --batch --no-tty --local-user $user --output legacy-signature.sig --detach-sign {payload_name}; or exit
command {modern_command} --homedir {modern_home} --batch --no-tty --local-user $user --output legacy-signed.gpg --sign {payload_name}
"#, user = quote(identity), payload_name = quote(payload_name)));
    success(&signed);
    assert!(signed.out.is_empty());
    let untrusted = native(&homes, &tool, &homes.legacy,
        &[OsStr::new("--status-fd"), OsStr::new("1"), OsStr::new("--verify"), OsStr::new("legacy-signature.sig"), OsStr::new("-")], payload);
    success(&untrusted);
    assert!(status(&untrusted.out, "VALIDSIG", Some(fingerprint)));
    assert!(status(&untrusted.out, "TRUST_UNDEFINED", None));

    let trusted = evaluate(&homes, &tool, &format!(r#"{PICK}
set -l option (__n9_legacy_pick 'gpg --trusted-k' --trusted-key); or exit
set -l key (__n9_legacy_pick 'gpg --trusted-key ' {key}); or exit
set -l decrypt (__n9_legacy_pick 'gpg --decr' --decrypt); or exit
command gpg --homedir {home} --batch --no-tty $option $key --status-fd 2 $decrypt < legacy-signed.gpg
"#));
    success(&trusted);
    assert_eq!(trusted.out, payload);
    assert!(status(&trusted.err, "VALIDSIG", Some(fingerprint)));
    assert!(status(&trusted.err, "TRUST_ULTIMATE", None));

    let mut tampered_payload = payload.to_vec();
    tampered_payload[0] ^= 1;
    fs::write(f.root.join(payload_name), &tampered_payload).unwrap();
    let bad = evaluate(&homes, &tool, &format!(r#"{PICK}
set -l option (__n9_legacy_pick 'gpg --trusted-k' --trusted-key); or exit
set -l key (__n9_legacy_pick 'gpg --trusted-key ' {key}); or exit
set -l verify (__n9_legacy_pick 'gpg --veri' --verify); or exit
command gpg --homedir {home} --batch --no-tty $option $key --status-fd 1 $verify legacy-signature.sig - < {payload_name}
"#, payload_name = quote(payload_name)));
    assert_eq!(bad.code, 1);
    assert!(status(&bad.out, "BADSIG", Some(key)));
    assert!(!status(&bad.out, "VALIDSIG", None));
    fs::write(f.root.join(payload_name), payload).unwrap();

    // RFC1991 is exercised only as literal-packet compatibility, never as a
    // weak encrypted mode. Its indeterminate framing has a real decoding effect:
    // removing the final source byte is legal EOF only for that framing.
    let stored = evaluate(&homes, &tool, &format!(r#"{PICK}
set -l compatibility (__n9_legacy_pick 'gpg --rfc199' --rfc1991); or exit
set -l store (__n9_legacy_pick 'gpg --stor' --store); or exit
command gpg --homedir {home} --batch --no-tty --compress-algo none $store < {payload_name} > definite.gpg; or exit
command gpg --homedir {home} --batch --no-tty $compatibility --compress-algo none $store < {payload_name} > rfc1991.gpg
"#, payload_name = quote(payload_name)));
    success(&stored);
    for (name, truncated_code) in [("definite.gpg", 2), ("rfc1991.gpg", 0)] {
        let packet = fs::read(f.root.join(name)).unwrap();
        let decoded = native(&homes, &tool, &homes.legacy, &[OsStr::new("--decrypt")], &packet);
        success(&decoded);
        assert_eq!(decoded.out, payload);
        let truncated = native(&homes, &tool, &homes.legacy, &[OsStr::new("--decrypt")], &packet[..packet.len() - 1]);
        assert_eq!(truncated.code, truncated_code);
        assert_eq!(truncated.out, &payload[..payload.len() - 1]);
    }

    // A missing producer must not fabricate the actual imported key or user.
    // Unrelated filesystem/fuzzy fallback is deliberately not forbidden.
    for helper in ["__fish_complete_gpg_key_id gpg", "__fish_complete_gpg_user_id gpg"] {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(helper)]);
        homes.environment(&mut command, &homes.legacy);
        let absent = run(&mut command, b"");
        assert_eq!(absent.code, 127);
        assert!(absent.out.is_empty());
    }
    let absent = f.eval(&format!("command gpg --homedir {home} --version"));
    assert_eq!(absent.code, 127);
    assert!(absent.out.is_empty());
}
