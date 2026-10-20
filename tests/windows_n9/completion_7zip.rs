use super::{Family, fixture::{Captured, Fixture, exact, quote, run}, functions_tools::tool_environment};
use std::{collections::BTreeMap, ffi::OsStr, fs, path::{Path, PathBuf}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-7z-completion-owned-unicode-archive-lifecycle", archives),
];

// Consumer coverage is deliberately narrower than the static constructor table.
// Commands consumed: a/d/e/h/l/rn/t/u/x. Neither b (benchmark) nor i (format
// inventory) is exercised. Attached values consumed: a:-t/-m/-p/-i/-x/-scs/-si,
// e:-o/-p, x:-o/-ao/-p, h:-scrc; also l:-slt, e:-so/-bs, a:-bd, x:-y and
// -p in t/u/rn/d. The global -scc is consumed throughout. An invalid e:-ao
// value is rejected. Other contexts of those switches remain unexercised.
// Entirely unexercised switches: -ai/-an/-ax/-bb/-bt/-r/-sa/-sdel/-seml/-sfx,
// -slp/-sni/-sns/-snh/-snl/-spd/-spe/-spf/-ssc/-ssw/-stl/-stm/-stx/-u/-v/-w.
// No email, source deletion, executable SFX, security/ACL, link, alternate-stream,
// large-page, affinity, absolute-member-path or multi-volume behavior is claimed.

const SELECT: &str = r#"
function n9_7z_select --argument-names variable context expected
    set -l candidates (complete -C "$context" | string split -f 1 \t | string unescape | string match -- "$expected")
    test (count $candidates) -eq 1; or return 81
    set -g $variable $candidates
end
n9_7z_select charset '7z -sc' -scc; or exit 81
"#;

fn evaluate(f: &Fixture, tool: &Path, directory: &Path, code: &str, input: &[u8]) -> Captured {
    let code = format!("{SELECT}\n{code}");
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    command.current_dir(directory);
    tool_environment(&mut command, tool);
    run(&mut command, input)
}

fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine 7z consumer: stdout={} stderr={}",
        String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "genuine 7z consumer stderr: {}", String::from_utf8_lossy(&result.err));
}

fn members(f: &Fixture, tool: &Path, directory: &Path, archive: &str) -> BTreeMap<String, u64> {
    let result = evaluate(f, tool, directory, &format!(r#"
n9_7z_select list '7z l' l; or exit 81
n9_7z_select technical '7z l -sl' -slt; or exit 81
command 7z $list $technical $charset'UTF-8' {}
"#, quote(archive)), b"");
    success(&result);
    let text = String::from_utf8(result.out).unwrap().replace('\r', "");
    let (_, entries) = text.split_once("\n----------\n").expect("7z technical member records");
    let mut members = BTreeMap::new();
    for record in entries.split("\n\n").filter(|record| !record.trim().is_empty()) {
        let path = record.lines().find_map(|line| line.strip_prefix("Path = ")).expect("member path");
        let size = record.lines().find_map(|line| line.strip_prefix("Size = ")).expect("member byte size").parse().unwrap();
        assert!(members.insert(path.replace('\\', "/"), size).is_none(), "duplicate archive member: {path}");
    }
    members
}

fn readback(f: &Fixture, tool: &Path, directory: &Path, archive: &str, member: &str, password: Option<&str>, expected: &[u8]) {
    let password = password.map_or_else(String::new, |password| format!(
        "n9_7z_select password '7z e -p' -p; or exit 81\nset -l protection $password{}\n", quote(password)));
    let result = evaluate(f, tool, directory, &format!(r#"
n9_7z_select extract '7z e' e; or exit 81
n9_7z_select stdout '7z e -so' -so; or exit 81
n9_7z_select streams '7z e -bs' -bs; or exit 81
{password}
command 7z $extract $stdout $streams'o0' $streams'p0' $charset'UTF-8' $protection {} {}
"#, quote(archive), quote(member)), b"");
    exact(result, 0, expected, b"");
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn archives(f: &Fixture) {
    let tool = PathBuf::from(std::env::var_os("FISH_N9_7Z").expect("required genuine native 7z fixture"));
    assert!(tool.is_absolute() && tool.is_file(), "real absolute 7z fixture: {}", tool.display());
    let directory = f.root.join("7z lifecycle 雪 🐟");
    fs::create_dir(&directory).unwrap();
    fs::create_dir(directory.join("nested 雪")).unwrap();
    let payload = "owned payload 雪 🐟\r\n\0binary suffix\n".as_bytes();
    let nested = b"\0\x01\xffnested bytes\r\n";
    let included = "included outside listfile 雪\n".as_bytes();
    fs::write(directory.join("payload 雪 🐟.txt"), payload).unwrap();
    fs::write(directory.join("nested 雪/inside 🐟.bin"), nested).unwrap();
    fs::write(directory.join("included 雪.txt"), included).unwrap();
    fs::write(directory.join("skip 雪.txt"), b"must not be archived\n").unwrap();
    fs::write(directory.join("members 雪 🐟.lst"), "\"payload 雪 🐟.txt\"\n\"nested 雪/inside 🐟.bin\"\n\"skip 雪.txt\"\n").unwrap();
    let password = "owned secret 雪 🐟";
    let archive = "owned archive 雪 🐟.7z";

    let identity = evaluate(f, &tool, &directory, "command -s 7z", b"");
    success(&identity);
    assert_eq!(fs::canonicalize(PathBuf::from(String::from_utf8(identity.out).unwrap().trim())).unwrap(), fs::canonicalize(&tool).unwrap());

    // Public context predicates must not offer extraction-only switches while
    // adding, or another root subcommand after the command slot is occupied.
    for (context, forbidden) in [("7z a -", "-o"), ("7z a -", "-ao"), ("7z l -", "-si"), ("7z t -", "-so"), ("7z a r", "rn")] {
        let result = evaluate(f, &tool, &directory, &format!("complete -C {}", quote(context)), b"");
        success(&result);
        assert!(!String::from_utf8(result.out).unwrap().lines().any(|line| line.split('\t').next() == Some(forbidden)), "unavailable 7z context: {context}: {forbidden}");
    }

    let create = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select add '7z a' a; or exit 81
n9_7z_select kind '7z a -t' -t; or exit 81
n9_7z_select method '7z a -m' -m; or exit 81
n9_7z_select password '7z a -p' -p; or exit 81
n9_7z_select include '7z a -i' -i; or exit 81
n9_7z_select exclude '7z a -x' -x; or exit 81
n9_7z_select list_encoding '7z a -scs' -scs; or exit 81
n9_7z_select progress '7z a -bd' -bd; or exit 81
command 7z $add $kind'7z' $method'x=0' $password{} $include'!included 雪.txt' $exclude'!skip 雪.txt' $list_encoding'UTF-8' $progress $charset'UTF-8' {} '@members 雪 🐟.lst'
"#, quote(password), quote(archive)), b"");
    success(&create);
    assert!(fs::read(directory.join(archive)).unwrap().starts_with(b"7z\xbc\xaf\x27\x1c"), "selected -t7z must produce a genuine 7z archive");
    let mut expected = BTreeMap::from([
        ("payload 雪 🐟.txt".to_owned(), payload.len() as u64),
        ("nested 雪/inside 🐟.bin".to_owned(), nested.len() as u64),
        ("included 雪.txt".to_owned(), included.len() as u64),
    ]);
    assert_eq!(members(f, &tool, &directory, archive), expected);
    readback(f, &tool, &directory, archive, "payload 雪 🐟.txt", Some(password), payload);
    readback(f, &tool, &directory, archive, "nested 雪/inside 🐟.bin", Some(password), nested);
    readback(f, &tool, &directory, archive, "included 雪.txt", Some(password), included);

    for (secret, status) in [(password, 0), ("wrong secret 雪", 2)] {
        let tested = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select test '7z t' t; or exit 81
n9_7z_select password '7z t -p' -p; or exit 81
command 7z $test $password{} $charset'UTF-8' {}
"#, quote(secret), quote(archive)), b"");
        assert_eq!(tested.code, status, "real password test: stdout={} stderr={}", String::from_utf8_lossy(&tested.out), String::from_utf8_lossy(&tested.err));
        if status == 0 { success(&tested); } else { assert!(!tested.out.is_empty() || !tested.err.is_empty()); }
    }

    let updated = "longer updated payload 雪 🐟\r\n\0different binary suffix\n".as_bytes();
    let added = "new member 雪 🐟\n".as_bytes();
    fs::write(directory.join("payload 雪 🐟.txt"), updated).unwrap();
    fs::write(directory.join("new 雪 🐟.txt"), added).unwrap();
    let update = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select update '7z u' u; or exit 81
n9_7z_select password '7z u -p' -p; or exit 81
n9_7z_select new_file '7z u archive.7z new' 'new 雪 🐟.txt'; or exit 81
command 7z $update $password{} $charset'UTF-8' {} 'payload 雪 🐟.txt' $new_file
"#, quote(password), quote(archive)), b"");
    success(&update);
    expected.insert("payload 雪 🐟.txt".to_owned(), updated.len() as u64);
    expected.insert("new 雪 🐟.txt".to_owned(), added.len() as u64);
    assert_eq!(members(f, &tool, &directory, archive), expected);
    readback(f, &tool, &directory, archive, "payload 雪 🐟.txt", Some(password), updated);
    readback(f, &tool, &directory, archive, "new 雪 🐟.txt", Some(password), added);

    let rename = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select rename '7z rn' rn; or exit 81
n9_7z_select password '7z rn -p' -p; or exit 81
command 7z $rename $password{} $charset'UTF-8' {} 'payload 雪 🐟.txt' 'renamed 雪 🐟.txt'
"#, quote(password), quote(archive)), b"");
    success(&rename);
    expected.remove("payload 雪 🐟.txt");
    expected.insert("renamed 雪 🐟.txt".to_owned(), updated.len() as u64);
    assert_eq!(members(f, &tool, &directory, archive), expected);
    readback(f, &tool, &directory, archive, "renamed 雪 🐟.txt", Some(password), updated);

    let delete = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select delete '7z d' d; or exit 81
n9_7z_select password '7z d -p' -p; or exit 81
command 7z $delete $password{} $charset'UTF-8' {} 'included 雪.txt'
"#, quote(password), quote(archive)), b"");
    success(&delete);
    expected.remove("included 雪.txt");
    assert_eq!(members(f, &tool, &directory, archive), expected);
    assert_eq!(fs::read(directory.join("included 雪.txt")).unwrap(), included, "archive deletion must not delete the original source");

    let extract = format!(r#"
n9_7z_select extract '7z x' x; or exit 81
n9_7z_select output '7z x -o' -o; or exit 81
n9_7z_select overwrite '7z x -ao' -ao; or exit 81
n9_7z_select yes '7z x -y' -y; or exit 81
n9_7z_select password '7z x -p' -p; or exit 81
command 7z $extract $output'readback 雪 🐟' $overwrite'a' $yes $password{} $charset'UTF-8' {}
"#, quote(password), quote(archive));
    success(&evaluate(f, &tool, &directory, &extract, b""));
    fs::write(directory.join("readback 雪 🐟/renamed 雪 🐟.txt"), b"owned overwrite sentinel").unwrap();
    success(&evaluate(f, &tool, &directory, &extract, b""));
    for (member, bytes) in [("renamed 雪 🐟.txt", updated), ("nested 雪/inside 🐟.bin", nested.as_slice()), ("new 雪 🐟.txt", added)] {
        assert_eq!(fs::read(directory.join("readback 雪 🐟").join(member)).unwrap(), bytes);
    }
    for absent in ["payload 雪 🐟.txt", "included 雪.txt", "skip 雪.txt"] {
        assert!(!directory.join("readback 雪 🐟").join(absent).exists(), "unexpected extracted member: {absent}");
    }
    let flat = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select extract '7z e' e; or exit 81
n9_7z_select output '7z e -o' -o; or exit 81
n9_7z_select password '7z e -p' -p; or exit 81
command 7z $extract $output'flat 雪 🐟' $password{} $charset'UTF-8' {} 'nested 雪/inside 🐟.bin'
"#, quote(password), quote(archive)), b"");
    success(&flat);
    assert_eq!(fs::read(directory.join("flat 雪 🐟/inside 🐟.bin")).unwrap(), nested);
    assert!(!directory.join("flat 雪 🐟/nested 雪").exists(), "e must flatten the selected member path");

    let stdin_payload = "stdin data 雪 🐟\0\r\n".as_bytes();
    let stdin_archive = "stdin archive 雪 🐟.7z";
    let stdin = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select add '7z a' a; or exit 81
n9_7z_select kind '7z a -t' -t; or exit 81
n9_7z_select stdin '7z a -si' -si; or exit 81
command 7z $add $kind'7z' $stdin'stdin member 雪 🐟.bin' $charset'UTF-8' {}
"#, quote(stdin_archive)), stdin_payload);
    success(&stdin);
    assert_eq!(members(f, &tool, &directory, stdin_archive), BTreeMap::from([("stdin member 雪 🐟.bin".to_owned(), stdin_payload.len() as u64)]));
    readback(f, &tool, &directory, stdin_archive, "stdin member 雪 🐟.bin", None, stdin_payload);

    let hashed = evaluate(f, &tool, &directory, r#"
n9_7z_select hash '7z h' h; or exit 81
n9_7z_select algorithm '7z h -scrc' -scrc; or exit 81
n9_7z_select source '7z h pay' 'payload 雪 🐟.txt'; or exit 81
command 7z $hash $algorithm'CRC32' $charset'UTF-8' $source
"#, b"");
    success(&hashed);
    let crc = format!("{:08X}", crc32(updated));
    assert!(String::from_utf8(hashed.out).unwrap().lines().any(|line|
        line.split_whitespace().next() == Some(crc.as_str()) && line.ends_with("payload 雪 🐟.txt")), "selected CRC32 must hash the actual Unicode source bytes");

    fs::write(directory.join("corrupt 雪.7z"), b"not a 7z archive\0\xff").unwrap();
    let corrupt = evaluate(f, &tool, &directory, r#"
n9_7z_select test '7z t' t; or exit 81
command 7z $test $charset'UTF-8' 'corrupt 雪.7z'
"#, b"");
    assert_eq!(corrupt.code, 2, "corrupt archive must fail in the genuine provider");
    assert!(!corrupt.out.is_empty() || !corrupt.err.is_empty());
    let invalid = evaluate(f, &tool, &directory, &format!(r#"
n9_7z_select extract '7z e' e; or exit 81
n9_7z_select overwrite '7z e -ao' -ao; or exit 81
command 7z $extract $overwrite'invalid' $charset'UTF-8' {}
"#, quote(archive)), b"");
    assert_eq!(invalid.code, 7, "invalid attached overwrite value must be a native command-line error");
    assert!(!invalid.out.is_empty() || !invalid.err.is_empty());

    // This constructor is static: missing native 7z does not erase its options,
    // but selected metadata must never impersonate an available archive tool.
    let unavailable = f.eval("status get-file completions/7z.fish | source; set -l add (complete -C '7z a' | string split -f 1 \\t | string match -- a); test (count $add) -eq 1; or exit 81; command 7z $add 'absent provider 雪.7z'");
    assert_eq!(unavailable.code, 127);
    assert!(unavailable.out.is_empty() && !unavailable.err.is_empty());
    assert!(!f.root.join("absent provider 雪.7z").exists());
}
