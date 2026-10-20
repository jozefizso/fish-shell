#![cfg(windows)]

use std::{ffi::OsStr, io::Read, path::PathBuf, process::{Command, Output, Stdio}, thread, time::{Duration, Instant}};

fn capture(command: &mut Command) -> Output {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || { let mut data = Vec::new(); stdout.read_to_end(&mut data).unwrap(); data });
    let err = thread::spawn(move || { let mut data = Vec::new(); stderr.read_to_end(&mut data).unwrap(); data });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() { break status; }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            let stdout = out.join().unwrap();
            let stderr = err.join().unwrap();
            panic!("N9 child timed out: stdout={} stderr={}", String::from_utf8_lossy(&stdout), String::from_utf8_lossy(&stderr));
        }
        thread::sleep(Duration::from_millis(5));
    };
    Output { status, stdout: out.join().unwrap(), stderr: err.join().unwrap() }
}

fn successful(command: &mut Command) -> String {
    let result = capture(command);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(result.stderr.is_empty(), "{}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout).unwrap()
}

fn catalog_precedence_fallback_and_cache_invalidation() {
    for scenario in ["precedence", "fallback", "native-default", "format-errors"] {
        let output = capture(Command::new(env!("CARGO_BIN_EXE_windows_n9_catalog_probe")).arg(scenario));
        assert!(output.status.success(), "{scenario}: {}", String::from_utf8_lossy(&output.stderr));
        if cfg!(feature = "localize-messages") {
            assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("catalog-{scenario}-ok\n"));
        } else {
            assert_eq!(output.stdout, b"catalog-profile=English-only\n");
        }
    }
}

struct Install {
    directory: fish_tempfile::TempDir,
    bin: PathBuf,
    rival: PathBuf,
    config: PathBuf,
}

impl Install {
    fn new() -> Self {
        let directory = fish_tempfile::new_dir().unwrap();
        let root = directory.path().join("安装 fish café");
        let bin = root.join("bin");
        let rival = directory.path().join("rival/bin");
        let config = directory.path().join("config");
        for path in [&bin, &rival, &config.join("fish")] { std::fs::create_dir_all(path).unwrap(); }
        for (name, executable) in [
            ("fish", env!("CARGO_BIN_EXE_fish")),
            ("fish_indent", env!("CARGO_BIN_EXE_fish_indent")),
            ("fish_key_reader", env!("CARGO_BIN_EXE_fish_key_reader")),
        ] { std::fs::copy(executable, bin.join(format!("{name}.exe"))).unwrap(); }
        std::fs::copy(env!("CARGO_BIN_EXE_fish"), rival.join("fish.exe")).unwrap();
        std::fs::write(config.join("fish/config.fish"), "echo WRONG-PATH-FISH\n").unwrap();
        Self { directory, bin, rival, config }
    }

    fn command(&self, binary: &str) -> Command {
        let mut command = Command::new(self.bin.join(format!("{binary}.exe")));
        command.env("PATH", &self.rival).env("XDG_CONFIG_HOME", &self.config)
            .env("XDG_DATA_HOME", self.directory.path().join("data"))
            .env("XDG_CACHE_HOME", self.directory.path().join("cache"))
            .env("LC_ALL", "C").env_remove("LANGUAGE");
        command.current_dir(self.directory.path());
        command
    }

    fn install_text(&self, topic: &str, version: &str, text: &str) {
        let help = self.bin.parent().unwrap().join("share/doc/fish/help");
        std::fs::create_dir_all(help.join("cmds")).unwrap();
        std::fs::write(help.join("fish-help-version"), version).unwrap();
        std::fs::write(help.join("cmds").join(format!("{topic}.txt")), text).unwrap();
    }
}

fn all_binary_help_entrypoints_match_own_installation() {
    let install = Install::new();
    if !cfg!(feature = "embed-manpages") {
        // Require real generated Sphinx pages, also when testing the minimal binary.
        let generated = std::env::var_os("FISH_N9_HELP_DIR").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("FISH_RESOLVED_BUILD_DIR")).join("fish-docs/help"));
        for name in ["fish", "fish_indent", "fish_key_reader", "set"] {
            let text = std::fs::read_to_string(generated.join("cmds").join(format!("{name}.txt")))
                .expect("minimal N9 help tests require generated Sphinx text (FISH_N9_HELP_DIR)");
            install.install_text(name, fish::BUILD_VERSION, &text);
        }
    }
    for binary in ["fish", "fish_indent", "fish_key_reader"] {
        for flag in ["-h", "--help"] {
            let text = successful(install.command(binary).arg(flag));
            assert!(text.contains(fish::BUILD_VERSION), "{binary}: {text}");
            assert!(text.contains(binary), "{binary}: {text}");
            assert!(!text.contains("WRONG-PATH-FISH"));
            let documented_option = match binary {
                "fish" => "--init-command",
                "fish_indent" => "--only-indent",
                "fish_key_reader" => "--continuous",
                _ => unreachable!(),
            };
            assert!(text.contains(documented_option), "{binary} help omits supported {documented_option}: {text}");
        }
    }
    let info = successful(install.command("fish").args(["-N", "-c", "status build-info"]));
    assert_eq!(info.contains("localize-messages"), cfg!(feature = "localize-messages"), "{info}");
    assert_eq!(info.contains("embed-manpages"), cfg!(feature = "embed-manpages"), "{info}");
    for code in ["set --help", "__fish_print_help set", "help cmds/set", "status native-help 'fish#description'"] {
        let text = successful(install.command("fish").args(["-N", "-c", code]));
        assert!(text.contains(fish::BUILD_VERSION), "{text}");
        assert!(!text.contains("WRONG-PATH-FISH"));
    }
    let missing = capture(install.command("fish").args(["-N", "-c", "status native-help n9-no-such-topic"]));
    assert!(!missing.status.success());
    assert!(!missing.stderr.is_empty());
    let missing_section = capture(install.command("fish").args(["-N", "-c", "status native-help 'fish#n9-no-such-section'"]));
    assert!(!missing_section.status.success());
}

#[cfg(not(feature = "embed-manpages"))]
fn minimal_help_rejects_missing_and_mismatched_resources() {
    let install = Install::new();
    let missing = capture(install.command("fish").arg("--help"));
    assert!(!missing.status.success());
    install.install_text("fish", "stale-other-installation", "wrong help");
    let mismatch = capture(install.command("fish").arg("--help"));
    assert!(!mismatch.status.success());
    assert!(String::from_utf8(mismatch.stderr).unwrap().contains("version"));
}

fn localized_display_keeps_c_numeric_syntax() {
    let install = Install::new();
    let text = successful(install.command("fish").args([OsStr::new("-N"), OsStr::new("-c"), OsStr::new("set -e LC_ALL; set -gx LANG en-US; set -gx LC_NUMERIC de-DE; printf \"%'0.2f\\n\" 1234.5; printf '%.2f\\n' (math '1.5 + 1'); set -gx LC_NUMERIC en-US; printf '%.2f\\n' 1.5")]));
    assert_eq!(text, "1.234,50\n2,50\n1.50\n");
}

fn live_message_invalidation() {
    let install = Install::new();
    let code = if cfg!(feature = "localize-messages") {
        "set -e LC_ALL; set -gx LANG de-DE; _ 'Print the working directory'; echo; set -gx LC_MESSAGES fr-FR; _ 'Print the working directory'; echo; status language set de; set -gx LC_MESSAGES fr-FR; _ 'Print the working directory'; echo; status language unset; _ 'Print the working directory'; echo; set -gx LC_ALL C; _ 'Print the working directory'; echo"
    } else {
        "set -e LC_ALL; set -gx LANG de-DE; _ 'Print the working directory'"
    };
    let text = successful(install.command("fish").args(["-N", "-c", code]));
    if cfg!(feature = "localize-messages") {
        assert_eq!(text, "Das Arbeitsverzeichnis ausgeben\nAfficher le dossier de travail\nDas Arbeitsverzeichnis ausgeben\nAfficher le dossier de travail\nPrint the working directory\n");
    } else {
        assert_eq!(text, "Print the working directory");
    }
}

pub(super) fn n9_locale() {
    let mut executed = 0;
    catalog_precedence_fallback_and_cache_invalidation();
    executed += 4; // Four isolated actual-owner/profile probe scenarios.
    all_binary_help_entrypoints_match_own_installation();
    executed += 13; // Six binary flags, build features, four help routes, two missing-data cases.
    #[cfg(not(feature = "embed-manpages"))]
    {
        minimal_help_rejects_missing_and_mismatched_resources();
        executed += 2; // Missing external manual and mismatched version.
    }
    localized_display_keeps_c_numeric_syntax();
    executed += 1;
    live_message_invalidation();
    executed += 1;
    let expected = if cfg!(feature = "embed-manpages") { 19 } else { 21 };
    assert_eq!(executed, expected);
    println!("n9-locale-cases expected={expected} executed={executed} catalogs={} help={}",
        if cfg!(feature = "localize-messages") { "localized" } else { "English-only" },
        if cfg!(feature = "embed-manpages") { "embedded" } else { "installed" });
}
