//! Help for executables and native builtins, always from the current installation.

use crate::prelude::*;

#[cfg(unix)]
use std::process::Command;

#[cfg(all(windows, feature = "embed-manpages"))]
use rust_embed::RustEmbed;

#[cfg(all(windows, feature = "embed-manpages"))]
#[derive(RustEmbed)]
#[folder = "$FISH_HELP_DIR"]
struct HelpFiles;

#[cfg(windows)]
fn help_key(topic: &str) -> std::io::Result<(&str, Option<&str>)> {
    let (name, fragment) = topic.split_once('#').map_or((topic, None), |(n, f)| (n, Some(f)));
    let name = name.strip_prefix("cmds/").unwrap_or(name);
    let name = match name {
        "introduction" => "index",
        "!" => "not",
        "." => "source",
        ":" => "true",
        "[" => "test",
        "{" => "begin",
        name => name,
    };
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid help topic"));
    }
    Ok((name, fragment))
}

/// Read installed help relative to the executable, never a fish found on PATH.
/// The version marker prevents a stale external manual being paired with a new binary.
#[cfg(windows)]
fn installed_help(executable: &std::path::Path, name: &str) -> std::io::Result<String> {
    use std::io::{Error, ErrorKind};
    let root = executable.parent().and_then(std::path::Path::parent)
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "cannot locate installation root"))?;
    let help = root.join("share/doc/fish/help");
    let version = std::fs::read_to_string(help.join("fish-help-version"))?;
    if version.trim_end() != crate::BUILD_VERSION {
        return Err(Error::new(ErrorKind::InvalidData, "installed help version does not match this executable"));
    }
    // Sphinx's text builder preserves the source directory hierarchy.
    let command = help.join("cmds").join(format!("{name}.txt"));
    match std::fs::read_to_string(command) {
        Err(error) if error.kind() == ErrorKind::NotFound => std::fs::read_to_string(help.join(format!("{name}.txt"))),
        result => result,
    }
}

#[cfg(windows)]
fn section_id(heading: &str) -> String {
    heading.trim().to_lowercase().split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty()).collect::<Vec<_>>().join("-")
}

#[cfg(windows)]
fn select_section(text: String, fragment: Option<&str>) -> std::io::Result<String> {
    let Some(fragment) = fragment.filter(|s| !s.is_empty()) else { return Ok(text) };
    let lines: Vec<_> = text.lines().collect();
    let requested = section_id(fragment);
    for (index, pair) in lines.windows(2).enumerate() {
        let underline = pair[1].trim();
        if section_id(pair[0]) == requested && underline.len() >= 3
            && underline.bytes().all(|c| matches!(c, b'=' | b'-' | b'~' | b'^' | b'*' | b'+')) {
            return Ok(lines[index..].join("\n") + "\n");
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("help section not found: {fragment}")))
}

/// Source-independent plain text help: no pager, shell, man or external formatter.
#[cfg(windows)]
pub fn help_text(topic: &str) -> std::io::Result<String> {
    let (name, fragment) = help_key(topic)?;
    #[cfg(feature = "embed-manpages")]
    let embedded = HelpFiles::get(&format!("cmds/{name}.txt"))
        .or_else(|| HelpFiles::get(&format!("{name}.txt")));
    #[cfg(feature = "embed-manpages")]
    if let Some(file) = embedded {
        let text = String::from_utf8(file.data.into_owned())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        return select_section(text, fragment).map(|text| format!("fish {} — {topic}\n\n{text}", crate::BUILD_VERSION));
    }
    let text = installed_help(&std::env::current_exe()?, name)?;
    select_section(text, fragment).map(|text| format!("fish {} — {topic}\n\n{text}", crate::BUILD_VERSION))
}

pub fn print_help(command: &str) {
    #[cfg(windows)]
    match help_text(command) {
        Ok(text) => printf!("%s", text),
        Err(error) => eprintf!("%s: %s\n", command, error),
    }
    #[cfg(unix)]
    {
        // Resolve the sibling fish rather than dispatching an unrelated installation from PATH.
        let fish = std::env::current_exe().and_then(|path| path.parent()
            .map(|directory| directory.join("fish"))
            .ok_or_else(|| std::io::Error::other("executable has no parent")));
        let result = fish.and_then(|fish| Command::new(fish)
            .args(["-c", &format!("__fish_print_help {command}")]).status());
        match result {
            Ok(status) if status.success() => (),
            Ok(_) => eprintf!("Could not show help message\n"),
            Err(error) => eprintf!("Could not show help message: %s\n", error),
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn topics_and_fragments() {
        for invalid in ["", "../fish", "C:fish", "fish/fish", "fish\\fish"] {
            assert!(help_key(invalid).is_err());
        }
        assert_eq!(help_key("fish#interactive-use").unwrap(), ("fish", Some("interactive-use")));
        for (topic, name) in [
            ("cmds/set#description", "set"), ("introduction#intro", "index"),
            ("!", "not"), ("cmds/.", "source"), (":", "true"),
            ("[", "test"), ("cmds/{", "begin"),
        ] {
            assert_eq!(help_key(topic).unwrap().0, name);
        }
        let text = "Title\n=====\n\nUnicode café\n------------\nlocal help\n".to_owned();
        assert!(select_section(text.clone(), Some("Unicode-café")).unwrap().starts_with("Unicode café"));
        assert!(select_section(text, Some("missing")).is_err());
    }

    #[test]
    fn installed_help_requires_matching_version() {
        let directory = fish_tempfile::new_dir().unwrap();
        let root = directory.path().join("安装 café");
        let bin = root.join("bin");
        let help = root.join("share/doc/fish/help/cmds");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&help).unwrap();
        let executable = bin.join("fish.exe");
        assert!(installed_help(&executable, "fish").is_err());
        std::fs::write(help.join("fish.txt"), "matching help λ").unwrap();
        let marker = help.parent().unwrap().join("fish-help-version");
        std::fs::write(&marker, "another version").unwrap();
        assert!(installed_help(&executable, "fish").is_err());
        std::fs::write(marker, crate::BUILD_VERSION).unwrap();
        assert_eq!(installed_help(&executable, "fish").unwrap(), "matching help λ");
    }
}
