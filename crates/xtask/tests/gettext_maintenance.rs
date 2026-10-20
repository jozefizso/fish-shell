use fish_gettext_mo_file_parser::parse_mo_file;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn maintenance(extraction: &Path, po: &Path, task: &str) -> Output {
    let temporary = po.parent().unwrap().join("temporary working 雪 🐟");
    fs::create_dir_all(&temporary).unwrap();
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["gettext", "--rust-extraction-dir"])
        .arg(extraction)
        .arg(task)
        .arg(po)
        .env("LC_ALL", "C.UTF-8")
        .env("TMPDIR", &temporary)
        .env("TMP", &temporary)
        .env("TEMP", &temporary)
        .output()
        .expect("execute actual gettext maintainer CLI with inherited GNU tool PATH")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unicode_catalog_check_update_and_gnu_mo_consumer() {
    let owner = fish_tempfile::new_dir().unwrap();
    let directory = owner.path().join("gettext maintenance 雪 🐟");
    let extraction = directory.join("Rust records 雪 🐟");
    fs::create_dir_all(&extraction).unwrap();
    let records = r#"msgid "GOAL_09 greeting 雪 🐟"
msgstr ""

#, c-format
msgid "GOAL_09 value %s: %d"
msgstr ""

msgid "GOAL_09 line one\n雪"
msgstr ""

"#;
    fs::write(extraction.join("messages 雪.po"), records).unwrap();
    // Valid PO record inputs; msguniq must merge duplicates before maintenance.
    fs::write(extraction.join("duplicate 🐟.po"), records).unwrap();
    let po = directory.join("translations 雪 🐟.po");
    let original = r#"msgid ""
msgstr ""
"Project-Id-Version: fish gettext maintenance regression\n"
"PO-Revision-Date: 2026-10-20 00:00+0000\n"
"Last-Translator: Regression <test@example.invalid>\n"
"Language-Team: German\n"
"Language: de\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"

msgid "GOAL_09 greeting 雪 🐟"
msgstr "Grüße 雪 🐟"

#, c-format
msgid "GOAL_09 value %s: %d"
msgstr "Wert %s: %d 雪 🐟"

msgid "GOAL_09 line one\n雪"
msgstr "Zeile eins\n雪 🐟"

msgid "GOAL_09 removed active row"
msgstr "Entfernt 雪"

#~ msgid "GOAL_09 obsolete owned row"
#~ msgstr "Veraltet 🐟"
"#;
    fs::write(&po, original).unwrap();

    let outdated = maintenance(&extraction, &po, "check");
    assert!(!outdated.status.success());
    assert_eq!(fs::read(&po).unwrap(), original.as_bytes());
    // Inspect unified-diff structure and owned PO message data, not CLI diagnostics.
    let diff = std::str::from_utf8(&outdated.stdout).unwrap();
    let lines: Vec<_> = diff.lines().collect();
    assert!(lines.first().is_some_and(|line| line.starts_with("--- ")), "stdout: {diff}\nstderr: {}", String::from_utf8_lossy(&outdated.stderr));
    assert!(lines.get(1).is_some_and(|line| line.starts_with("+++ ")), "{diff}");
    assert!(lines.iter().any(|line| line.starts_with("@@ ")), "{diff}");
    assert!(lines.contains(&"-msgid \"GOAL_09 removed active row\""), "{diff}");
    assert!(lines.contains(&"-#~ msgid \"GOAL_09 obsolete owned row\""), "{diff}");

    assert_success(&maintenance(&extraction, &po, "update"));
    let updated = fs::read(&po).unwrap();
    assert!(!std::str::from_utf8(&updated).unwrap().lines().any(|line| line.starts_with("#~")));
    assert_success(&maintenance(&extraction, &po, "check"));
    assert_eq!(fs::read(&po).unwrap(), updated);

    let mo = directory.join("compiled translations 雪 🐟.mo");
    let compiled = Command::new("msgfmt")
        .args(["--check", "--check-format", "-o", "-", "-"])
        .stdin(fs::File::open(&po).unwrap())
        .stdout(fs::File::create(&mo).unwrap())
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("execute genuine GNU msgfmt from inherited PATH");
    assert_success(&compiled);
    let mo_bytes = fs::read(&mo).unwrap();
    let translations = parse_mo_file(&mo_bytes).unwrap();
    for (message, translation) in [
        ("GOAL_09 greeting 雪 🐟", "Grüße 雪 🐟"),
        ("GOAL_09 value %s: %d", "Wert %s: %d 雪 🐟"),
        ("GOAL_09 line one\n雪", "Zeile eins\n雪 🐟"),
    ] {
        assert_eq!(
            translations.get(message.as_bytes()).copied(),
            Some(translation.as_bytes()),
            "{message}"
        );
    }
    for removed in ["GOAL_09 removed active row", "GOAL_09 obsolete owned row"] {
        assert!(!translations.contains_key(removed.as_bytes()), "{removed}");
    }
}
