use super::{Family, fixture::{Captured, Fixture, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, fs, path::{Path, PathBuf}, process::Command};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-age-keygen-output-recipient-and-encryption-consumers", keygen),
];

// Remove the generated private identity even when a completion or consumer
// assertion fails. No private-key material is printed or attached to assertions.
struct PrivateDirectory(PathBuf);
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn evaluate(f: &Fixture, keygen: &Path, directory: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    command.current_dir(directory);
    tool_environment(&mut command, keygen);
    run(&mut command, b"")
}

fn native(f: &Fixture, program: &Path, directory: &Path, args: &[&OsStr]) -> Captured {
    let mut command = Command::new(program);
    command.args(args).current_dir(directory).env_clear();
    for (name, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(name, value); }
    }
    tool_environment(&mut command, program);
    run(&mut command, b"")
}

fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine age consumer: {}", String::from_utf8_lossy(&result.err));
}

fn keygen(f: &Fixture) {
    let keygen = PathBuf::from(std::env::var_os("FISH_N9_AGE_KEYGEN").expect("required genuine native age-keygen fixture"));
    assert!(keygen.is_absolute() && keygen.is_file(), "real absolute age-keygen fixture: {}", keygen.display());
    let age = keygen.with_file_name("age.exe");
    assert!(age.is_file(), "matching genuine age encryption consumer required: {}", age.display());
    let private = PrivateDirectory(f.root.join("age keypair 雪 🐟"));
    fs::create_dir(&private.0).unwrap();

    let resolved = evaluate(f, &keygen, &private.0, "command -s age-keygen");
    success(&resolved);
    assert!(resolved.err.is_empty());
    assert_eq!(fs::canonicalize(PathBuf::from(String::from_utf8(resolved.out).unwrap().trim())).unwrap(), fs::canonicalize(&keygen).unwrap());

    // --version is accepted by the real key generator, not just by age. This
    // catches a registration accidentally attached to the other executable.
    let version = evaluate(f, &keygen, &private.0, r#"
set -l version_option (complete -C 'age-keygen --ver' | string split -f 1 \t | string match -- --version)
test (count $version_option) -eq 1; or exit 81
command age-keygen $version_option
"#);
    success(&version);
    assert!(version.err.is_empty());
    let direct_version = native(f, &keygen, &private.0, &[OsStr::new("--version")]);
    success(&direct_version);
    assert_eq!(version.out, direct_version.out);

    let absent_metadata = f.eval("status get-file completions/age-keygen.fish | source; complete -C 'age-keygen --ver'");
    success(&absent_metadata);
    assert!(absent_metadata.err.is_empty());
    assert!(String::from_utf8(absent_metadata.out).unwrap().lines()
        .any(|line| line.split('\t').next() == Some("--version")));
    let absent_command = f.eval("command age-keygen --version");
    assert_eq!(absent_command.code, 127);
    assert!(absent_command.out.is_empty());

    let generated = evaluate(f, &keygen, &private.0, r#"
set -l output (complete -C 'age-keygen --out' | string split -f 1 \t | string match -- --output)
test (count $output) -eq 1; or exit 82
command age-keygen $output 'private identity 雪.txt'
"#);
    success(&generated);
    assert!(generated.out.is_empty(), "private identity must only be written to the owned file");
    assert!(private.0.join("private identity 雪.txt").is_file());

    let derived = evaluate(f, &keygen, &private.0, r#"
set -l derive (complete -C 'age-keygen -' | string split -f 1 \t | string match -- -y)
test (count $derive) -eq 1; or exit 83
command age-keygen $derive 'private identity 雪.txt'
"#);
    success(&derived);
    assert!(derived.err.is_empty());
    let recipient = String::from_utf8(derived.out).unwrap();
    assert_eq!(recipient.lines().count(), 1);
    let recipient = recipient.trim_end();
    assert!(recipient.starts_with("age1") && recipient.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));

    let recipient_file = evaluate(f, &keygen, &private.0, r#"
set -l derive (complete -C 'age-keygen -' | string split -f 1 \t | string match -- -y)
set -l output (complete -C 'age-keygen -' | string split -f 1 \t | string match -- -o)
test (count $derive) -eq 1; and test (count $output) -eq 1; or exit 84
command age-keygen $derive $output 'public recipients 🐟.txt' 'private identity 雪.txt'
"#);
    success(&recipient_file);
    assert_eq!(fs::read_to_string(private.0.join("public recipients 🐟.txt")).unwrap(), format!("{recipient}\n"));

    for (context, forbidden) in [
        ("age-keygen --output 'private identity 雪.txt' --out", "--output"),
        ("age-keygen -o 'private identity 雪.txt' --out", "--output"),
        ("age-keygen --output 'private identity 雪.txt' -o", "-o"),
        ("age-keygen -y -y", "-y"),
        ("age-keygen -o --ver", "--version"),
        ("age-keygen --output --ver", "--version"),
        ("age -r --ver", "--version"),
        ("age -R --ver", "--version"),
        ("age -d -i --ver", "--version"),
        ("age -e -j --ver", "--version"),
        ("age -d --recip", "--recipient"),
        ("age -p --recip", "--recipient"),
        ("age -d --armo", "--armor"),
        ("age -d --encr", "--encrypt"),
        ("age -e --decr", "--decrypt"),
    ] {
        let result = evaluate(f, &keygen, &private.0, &format!("complete -C {}", super::fixture::quote(context)));
        success(&result);
        assert!(result.err.is_empty());
        assert!(!String::from_utf8(result.out).unwrap().split('\n')
            .any(|line| line.split('\t').next() == Some(forbidden)),
            "keygen/age option is unavailable in this context: {context}: {forbidden}");
    }

    let payload = "owned age completion payload 雪 🐟\r\n\0binary suffix\n".as_bytes();
    fs::write(private.0.join("payload 雪 🐟.txt"), payload).unwrap();
    let encrypted = evaluate(f, &keygen, &private.0, &format!(r#"
set -l encrypt (complete -C 'age --encr' | string split -f 1 \t | string match -- --encrypt)
set -l recipient_option (complete -C 'age --recip' | string split -f 1 \t | string match -- --recipient)
test (count $encrypt) -eq 1; and test (count $recipient_option) -eq 1; or exit 85
command age $encrypt $recipient_option {recipient} -o 'encrypted 雪.age' 'payload 雪 🐟.txt'
"#));
    success(&encrypted);
    let armored = evaluate(f, &keygen, &private.0, r#"
set -l recipients (complete -C 'age --recipients-f' | string split -f 1 \t | string match -- --recipients-file)
set -l public_file (complete -C 'age -R public' | string split -f 1 \t | string unescape | string match -- 'public recipients 🐟.txt')
set -l armor (complete -C 'age --arm' | string split -f 1 \t | string match -- --armor)
for selected in recipients public_file armor
    test (count $$selected) -eq 1; or exit 86
end
command age $recipients $public_file $armor -o 'armored 雪.age' 'payload 雪 🐟.txt'
"#);
    success(&armored);
    assert!(fs::read(private.0.join("armored 雪.age")).unwrap()
        .starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----\n"), "selected --armor must control the actual encoding");
    for (ciphertext, readback) in [("encrypted 雪.age", "readback 🐟.txt"), ("armored 雪.age", "armored readback 🐟.txt")] {
        let decrypted = evaluate(f, &keygen, &private.0, &format!(r#"
set -l decrypt (complete -C 'age --decr' | string split -f 1 \t | string match -- --decrypt)
set -l identity (complete -C 'age -d --iden' | string split -f 1 \t | string match -- --identity)
set -l private_file (complete -C 'age -d -i private' | string split -f 1 \t | string unescape | string match -- 'private identity 雪.txt')
for selected in decrypt identity private_file
    test (count $$selected) -eq 1; or exit 87
end
command age $decrypt $identity $private_file -o {} {}
"#, super::fixture::quote(readback), super::fixture::quote(ciphertext)));
        success(&decrypted);
        assert_eq!(fs::read(private.0.join(readback)).unwrap(), payload);
    }
}
