use super::{Family, fixture::{Captured, Fixture, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, path::{Path, PathBuf}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-msgfmt-header-format-and-mo-roundtrip-consumers", catalog),
];

fn invoke(f: &Fixture, program: &Path, code: &str, input: &[u8]) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, program);
    run(&mut command, input)
}

fn catalog(f: &Fixture) {
    let iconv = PathBuf::from(std::env::var_os("FISH_N9_ICONV").expect("required genuine GNU tool fixture"));
    let compiler = iconv.parent().unwrap().join("msgfmt.exe");
    let reader = iconv.parent().unwrap().join("msgunfmt.exe");
    assert!(compiler.is_absolute() && compiler.is_file() && reader.is_file());
    let input = r#"msgid ""
msgstr ""
"Project-Id-Version: native completion consumer\n"
"POT-Creation-Date: 2026-10-20 00:00+0000\n"
"PO-Revision-Date: 2026-10-20 00:00+0000\n"
"Last-Translator: Owned <test@example.invalid>\n"
"Language-Team: French\n"
"Language: fr\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"

#, c-format
msgid "Native value %s 雪 🐟"
msgstr "Traduction %s 🐟 雪"
"#;
    let code = "set -l header (complete -C 'msgfmt --check-h' | string split -f 1 \\t | string match -- --check-header); test (count $header) -eq 1; or exit 81; command msgfmt $header --check-format --output-file=- -";
    let produced = invoke(f, &compiler, code, input.as_bytes());
    assert_eq!(produced.code, 0, "selected header flag must compile actual catalog: {}", String::from_utf8_lossy(&produced.err));
    assert!(produced.out.starts_with(&[0xde, 0x12, 0x04, 0x95]) || produced.out.starts_with(&[0x95, 0x04, 0x12, 0xde]));
    let decoded = invoke(f, &reader, "command msgunfmt --no-wrap -", &produced.out);
    assert_eq!(decoded.code, 0, "actual GNU MO consumer: {}", String::from_utf8_lossy(&decoded.err));
    let decoded = String::from_utf8(decoded.out).unwrap();
    assert!(decoded.lines().any(|line| line == "msgid \"Native value %s 雪 🐟\""), "{decoded}");
    assert!(decoded.lines().any(|line| line == "msgstr \"Traduction %s 🐟 雪\""), "{decoded}");
    let incompatible = input.replace("Traduction %s 🐟 雪", "Traduction %d 🐟 雪");
    let rejected = invoke(f, &compiler, code, incompatible.as_bytes());
    assert_ne!(rejected.code, 0, "incompatible format must not compile as a successful catalog");
    assert!(!rejected.err.is_empty());
    let absent = f.input("command msgfmt --check-header --check-format --output-file=- -", input.as_bytes());
    assert_eq!(absent.code, 127);
    assert!(absent.out.is_empty() && !absent.err.is_empty());
}
