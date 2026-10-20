use std::{fs, path::Path, process::{Command, Output}};

fn format(path: &Path, check: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.arg("format").arg(if check { "--check" } else { "--force" }).arg(path);
    command.output().expect("execute actual maintainer formatter")
}

#[test]
fn unicode_rewrite_is_consumed_by_fluent_and_releases_temporary_files() {
    let directory = fish_tempfile::new_dir().unwrap();
    let path = directory.path().join("messages 雪 🐟.ftl");
    fs::write(&path, "greeting= 雪 🐟\n").unwrap();
    assert!(!format(&path, true).status.success());
    let rewritten = format(&path, false);
    assert!(rewritten.status.success(), "{}", String::from_utf8_lossy(&rewritten.stderr));
    let resource = fluent_ftl_tools::parse_as_fluent_resource(&path).unwrap();
    let bundle = fluent_ftl_tools::make_bundle(&resource).unwrap();
    let message = bundle.get_message("greeting").unwrap();
    let mut errors = Vec::new();
    assert_eq!(bundle.format_pattern(message.value().unwrap(), None, &mut errors), "雪 🐟");
    assert!(errors.is_empty());
    assert!(format(&path, true).status.success());
    let remaining: Vec<_> = fs::read_dir(directory.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(remaining, [path]);
}

#[cfg(windows)]
#[test]
fn denied_replacement_preserves_source_and_releases_temporary_files() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let directory = fish_tempfile::new_dir().unwrap();
    let path = directory.path().join("locked messages 雪 🐟.ftl");
    let original = "greeting= source 雪 🐟\n";
    fs::write(&path, original).unwrap();
    const FILE_SHARE_READ: u32 = 1;
    let _reader = fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(&path).unwrap();
    let failed = format(&path, false);
    assert!(!failed.status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let remaining: Vec<_> = fs::read_dir(directory.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(remaining, [path]);
}
