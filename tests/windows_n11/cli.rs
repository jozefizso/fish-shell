use super::{fixture::{exact, run}, package::{Installed, success}};
use std::fs;

pub fn exercise(installed: &Installed) {
    let f = &installed.fixture;
    let version = fs::read_to_string(installed.evidence.join("artifact-version.txt")).unwrap();
    // Execute the established real language/CLI families against the installed image.
    super::scripts::exercise(f);
    installed.record("cli/n3-script-stdin-argv-repeated-command-parse-debug-profile-errors");
    let config = f.state.join("config/fish");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("config.fish"), "echo configured\n").unwrap();
    exact(run(&mut installed.command("fish", &["-C", "echo first", "--init-command", "echo second", "-c", "echo execution"]), b""), 0, b"configured\nfirst\nsecond\nexecution\n", b"");
    exact(run(&mut installed.command("fish", &["-N", "-C", "echo first", "--init-command", "echo second", "-c", "echo execution; set -q fish_private_mode; echo $status"]), b""), 0, b"first\nsecond\nexecution\n0\n", b"");
    fs::remove_file(config.join("config.fish")).unwrap();
    installed.record("cli/n4-config-init-login-private-no-config-profile-order");
    for flag in ["-c", "--command", "-C", "--init-command", "-f", "--features", "-d", "--debug", "-o", "--debug-output", "-p", "--profile", "--profile-startup"] {
        let output = run(&mut installed.command("fish", &[flag]), b"");
        assert_ne!(output.code, 0, "{flag} must reject its missing argument");
        installed.record(&format!("cli/fish/{flag}/missing-argument"));
    }
    for flag in ["-f", "--features"] {
        exact(run(&mut installed.command("fish", &["-N", flag, "no-qmark-noglob", "-c", "status test-feature qmark-noglob"]), b""), 1, b"", b"");
        installed.record(&format!("cli/fish/{flag}/feature-consumer"));
    }
    let forbidden = f.root.join("parse-only-side-effect");
    let effect = format!("echo forbidden > {}", super::fixture::quote(forbidden.to_str().unwrap()));
    for flag in ["-n", "--no-execute"] {
        exact(run(&mut installed.command("fish", &["-N", flag, "-C", &effect, "-c", &effect]), b""), 0, b"", b"");
        assert!(!forbidden.exists());
        installed.record(&format!("cli/fish/{flag}/init-and-execution-suppressed"));
    }
    let profile = f.root.join("long-option-profile");
    exact(run(&mut installed.command("fish", &["-N", "--profile", profile.to_str().unwrap(), "-c", "printf long-profile"]), b""), 0, b"long-profile", b"");
    let profile = fs::read_to_string(profile).unwrap();
    assert!(profile.lines().any(|line| {
        let mut fields = line.split_ascii_whitespace();
        let own = fields.next().and_then(|value| value.parse::<f64>().ok());
        let total = fields.next().and_then(|value| value.parse::<f64>().ok());
        own.is_some_and(|time| time.is_finite() && time >= 0.0)
            && total.is_some_and(|time| time.is_finite() && time >= 0.0)
            && fields.map(|word| word.trim_start_matches('>')).filter(|word| !word.is_empty())
                .eq(["printf", "long-profile"])
    }), "profile must measure the executed command");
    installed.record("cli/fish/--profile/native-sink");
    let poison = f.root.join("competing fish PATH");
    fs::create_dir(&poison).unwrap();
    fs::copy(f.fish.parent().unwrap().join("fish_key_reader.exe"), poison.join("fish.exe")).unwrap();
    for binary in ["fish", "fish_indent", "fish_key_reader"] {
        for flag in ["-v", "--version"] {
            let output = success(run(&mut installed.command(binary, &[flag]), b""));
            assert!(String::from_utf8_lossy(&output.out).split_ascii_whitespace().any(|token| token == version.trim()), "installed binary version must match its package manifest");
            assert!(output.err.is_empty());
            installed.record(&format!("cli/{binary}/{flag}"));
        }
        for flag in ["-h", "--help"] {
            let output = success(run(installed.command(binary, &[flag]).env("PATH", &poison), b""));
            assert!(output.err.is_empty());
            assert!(!output.out.contains(&0x1b), "installed offline help must be plain text");
            let options: &[&str] = match binary {
                "fish" => &["--init-command", "--no-config", "--profile-startup"],
                "fish_indent" => &["--check", "--write", "--only-unindent"],
                "fish_key_reader" => &["--continuous", "--verbose"],
                _ => unreachable!(),
            };
            for option in options {
                assert!(String::from_utf8_lossy(&output.out).contains(option), "installed {binary} help must document supported option {option}");
            }
            installed.record(&format!("cli/{binary}/{flag}"));
        }
        for flag in ["--n11-invalid-option", "--version=unexpected"] {
            let output = run(&mut installed.command(binary, &[flag]), b"");
            assert_ne!(output.code, 0);
            installed.record(&format!("cli/{binary}/invalid/{flag}"));
        }
    }
    let source = b"if true;echo formatted;end\n";
    let formatted = b"if true\n    echo formatted\nend\n";
    exact(run(&mut installed.command("fish_indent", &[]), source), 0, formatted, b"");
    for flag in ["-i", "--no-indent", "--only-unindent"] {
        exact(run(&mut installed.command("fish_indent", &[flag]), formatted), 0, b"if true\necho formatted\nend\n", b"");
        installed.record(&format!("cli/fish_indent/{flag}"));
    }
    exact(run(&mut installed.command("fish_indent", &["--only-indent"]), b"if true\necho formatted\nend\n"), 0, formatted, b"");
    installed.record("cli/fish_indent/--only-indent");
    for flag in ["--html", "--ansi", "--pygments", "--dump-parse-tree"] {
        let output = success(run(&mut installed.command("fish_indent", &[flag]), source));
        match flag {
            "--html" => {
                let html = String::from_utf8(output.out).unwrap();
                assert!(html.contains("<span"));
                let mut inside_tag = false;
                let plain: String = html.chars().filter(|&ch| {
                    if ch == '<' { inside_tag = true; false }
                    else if ch == '>' { inside_tag = false; false }
                    else { !inside_tag }
                }).collect();
                assert_eq!(plain.as_bytes(), formatted);
                assert!(output.err.is_empty());
            }
            "--ansi" => {
                let mut parts = output.out.split(|&byte| byte == 0x1b);
                let mut plain = parts.next().unwrap().to_vec();
                for part in parts {
                    assert_eq!(part.first(), Some(&b'['));
                    let end = part.iter().position(|&byte| byte == b'm').expect("complete SGR color sequence");
                    assert!(part[1..end].iter().all(|byte| byte.is_ascii_digit() || matches!(byte, b';' | b':')));
                    plain.extend_from_slice(&part[end + 1..]);
                }
                assert_eq!(plain, formatted);
                assert!(output.err.is_empty());
            }
            "--pygments" => {
                let mut position = 0;
                for line in String::from_utf8(output.out).unwrap().lines() {
                    let fields: Vec<_> = line.split(',').collect();
                    assert_eq!(fields.len(), 3);
                    let start = fields[0].parse::<usize>().unwrap();
                    let end = fields[1].parse::<usize>().unwrap();
                    assert_eq!(start, position);
                    assert!(end > start && end <= source.len());
                    if start == 0 { assert_eq!(fields[2], "keyword"); }
                    position = end;
                }
                assert_eq!(position, source.len(), "highlight spans must cover the original source exactly");
                assert!(output.err.is_empty());
            }
            "--dump-parse-tree" => {
                assert_eq!(output.out, formatted);
                assert!(String::from_utf8_lossy(&output.err).contains("formatted"), "parse dump must retain the input token");
            }
            _ => unreachable!(),
        }
        installed.record(&format!("cli/fish_indent/{flag}"));
    }
    let file = f.root.join("-format 雪.fish");
    fs::write(&file, source).unwrap();
    let file_name = file.to_str().unwrap();
    for flag in ["-c", "--check"] {
        let output = run(&mut installed.command("fish_indent", &[flag, "--", file_name]), b"");
        assert_ne!(output.code, 0);
        assert_eq!(fs::read(&file).unwrap(), source);
        installed.record(&format!("cli/fish_indent/{flag}/unchanged"));
    }
    for flag in ["-w", "--write"] {
        fs::write(&file, source).unwrap();
        success(run(&mut installed.command("fish_indent", &[flag, "--", file_name]), b""));
        assert_eq!(fs::read(&file).unwrap(), formatted);
        exact(run(&mut installed.command("fish_indent", &["--check", file_name]), b""), 0, b"", b"");
        installed.record(&format!("cli/fish_indent/{flag}/durable-file"));
    }
    for args in [vec!["-c"], vec!["--continuous"], vec!["-V"], vec!["--verbose"]] {
        let output = run(&mut installed.command("fish_key_reader", &args), b"not a console\n");
        assert_ne!(output.code, 0, "key reader must not pretend a pipe is a console");
        installed.record(&format!("cli/fish_key_reader/{}/nonterminal", args[0]));
    }
    let usage = success(run(&mut installed.command("fish", &["-N", "--print-rusage-self", "-c", "echo usage"]), b""));
    assert_eq!(usage.out, b"usage\n");
    let usage = String::from_utf8(usage.err).unwrap();
    let value = |name: &str, unit: &str| {
        usage.lines().find_map(|line| line.trim().strip_prefix(name)?.strip_suffix(unit).map(str::trim))
            .unwrap_or_else(|| panic!("missing native counter {name} ({unit})"))
    };
    for name in ["elapsed:", "user CPU:", "kernel CPU:"] {
        let seconds = value(name, " seconds").parse::<f64>().unwrap();
        assert!(seconds.is_finite() && seconds >= 0.0);
    }
    let peak = value("peak working set:", " bytes").parse::<u64>().unwrap();
    let current = value("working set:", " bytes").parse::<u64>().unwrap();
    assert!(current > 0 && peak >= current, "native memory counters must retain byte units and peak/current ordering");
    installed.record("cli/fish/--print-rusage-self/native-units");
}
