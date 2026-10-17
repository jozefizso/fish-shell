use super::*;

fn executable(f: &Fixture, directory: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(name);
    fs::copy(&f.probe, &path).unwrap();
    path
}
fn launched(f: &Fixture, setup: &str, command: &str, expected: &Path) {
    let out = success(f.eval(&format!("{setup}; {command} identity")));
    let record = std::str::from_utf8(&out)
        .unwrap()
        .trim_end()
        .strip_prefix("exe=")
        .unwrap();
    let bytes: Vec<u8> = (0..record.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&record[i..i + 2], 16).unwrap())
        .collect();
    let actual = PathBuf::from(String::from_utf8(bytes).unwrap());
    assert_eq!(
        fish::wutil::native_file_identity(&path_text(&actual), true).unwrap(),
        fish::wutil::native_file_identity(&path_text(expected), true).unwrap()
    );
}
fn lookup(f: &Fixture, setup: &str, name: &str, expected: &Path) {
    for option in ["-s", "-v", "--search"] {
        same_path_output(
            f.eval(&format!("{setup}; command {option} {}", quote(name))),
            expected,
        );
    }
    for option in ["-q", "--query", "--quiet"] {
        exact(
            f.eval(&format!("{setup}; command {option} {}", quote(name))),
            0,
            b"",
            b"",
        );
    }
    for option in [
        "-p",
        "-P",
        "--path",
        "--force-path",
        "-fp",
        "--no-functions --path",
    ] {
        same_path_output(
            f.eval(&format!("{setup}; type {option} {}", quote(name))),
            expected,
        );
    }
    for option in ["-q", "--query", "--quiet"] {
        exact(
            f.eval(&format!("{setup}; type {option} {}", quote(name))),
            0,
            b"",
            b"",
        );
    }
    for option in ["-t", "--type"] {
        exact(
            f.eval(&format!("{setup}; type {option} {}", quote(name))),
            0,
            b"file\n",
            b"",
        );
    }
    for option in ["-s", "--short", "--color=never"] {
        let out = success(f.eval(&format!("{setup}; type {option} {}", quote(name))));
        assert!(!out.is_empty());
        assert!(!out.contains(&0x1b));
    }
    exact(
        f.eval(&format!("{setup}; test -x {}", qpath(expected))),
        0,
        b"",
        b"",
    );
    launched(f, setup, &quote(name), expected);
}
pub(super) fn exercise(f: &Fixture) {
    let first = f.root.join("first provider 雪");
    let second = f.root.join("second provider");
    let a = executable(f, &first, "select.exe");
    let b = executable(f, &second, "select.exe");
    let setup = format!("set -gx PATH {} {}", qpath(&first), qpath(&second));
    lookup(f, &setup, "select", &a);
    lookup(f, &setup, "select.exe", &a);
    lookup(f, &setup, "SELECT", &a);
    let reversed = format!("set -gx PATH {} {}", qpath(&second), qpath(&first));
    lookup(f, &reversed, "select", &b);
    for command in [
        "command -a",
        "command --all",
        "type -ap",
        "type --all --path",
    ] {
        let out = success(f.eval(&format!("{setup}; {command} select")));
        let text = std::str::from_utf8(&out).unwrap();
        let paths: Vec<_> = text.lines().collect();
        assert_eq!(
            paths.len(),
            2,
            "all lookup must expose both executable providers"
        );
        for (found, expected) in paths.iter().zip([&a, &b]) {
            assert_eq!(
                fs::canonicalize(found).unwrap(),
                fs::canonicalize(expected).unwrap()
            );
        }
    }
    // PATH edits in the same interpreter must invalidate immediately.
    let out = success(f.eval(&format!(
        "{setup}; command -s select; {reversed}; command -s select"
    )));
    let lines: Vec<_> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        fs::canonicalize(lines[0]).unwrap(),
        fs::canonicalize(&a).unwrap()
    );
    assert_eq!(
        fs::canonicalize(lines[1]).unwrap(),
        fs::canonicalize(&b).unwrap()
    );
    launched(f, &setup, &qpath(&b), &b); // Explicit paths bypass PATH.
    launched(
        f,
        &format!("cd {}; set -gx PATH {}", qpath(&first), qpath(&second)),
        "./select.exe",
        &a,
    );
    let relative = format!(
        "{}{}",
        &first.to_str().unwrap()[..2],
        a.file_name().unwrap().to_str().unwrap()
    );
    launched(f, &format!("cd {}", qpath(&first)), &quote(&relative), &a);
    let cwd_only = executable(f, &f.root, "cwd-only.exe");
    let marker = f.root.join("forbidden provider marker");
    for value in ["set -gx PATH", "set -gx PATH ''", "set -gx PATH '' ''"] {
        let result = f.eval(&format!("{value}; cwd-only marker {}", qpath(&marker)));
        assert_eq!(
            result.code, 127,
            "missing command status must remain unknown-command"
        );
        failed(result, true);
        assert!(!marker.exists(), "native loader implicitly searched cwd");
        exact(
            f.eval(&format!("{value}; command -s cwd-only")),
            127,
            b"",
            b"",
        );
        let result = f.eval(&format!("{value}; type -p cwd-only"));
        assert_eq!(result.code, 1);
        failed(result, false);
    }
    launched(f, "set -gx PATH .", "cwd-only", &cwd_only);
    for extension in ["cmd", "bat", "ps1"] {
        let path = first.join(format!("provider.{extension}"));
        let body = if extension == "ps1" {
            format!(
                "[IO.File]::WriteAllText('{}','forbidden')",
                marker.display()
            )
        } else {
            format!("@echo forbidden>\"{}\"\r\n", marker.display())
        };
        fs::write(&path, body).unwrap();
        failed(
            f.eval(&format!("{} marker {}", qpath(&path), qpath(&marker))),
            true,
        );
        assert!(!marker.exists());
    }
    let com = executable(f, &first, "provider.com");
    let result = f.eval(&format!("{} marker {}", qpath(&com), qpath(&marker)));
    assert_eq!(result.code, 126, "unsupported native provider status");
    failed(result, true);
    assert!(!marker.exists());
    let supported = executable(f, &second, "provider.exe");
    lookup(f, &setup, "provider", &supported);
    assert!(
        !marker.exists(),
        "disabled earlier providers hid/ran before native exe"
    );
    fs::create_dir(first.join("directory.exe")).unwrap();
    let regular = executable(f, &second, "directory.exe");
    lookup(f, &setup, "directory", &regular);
    let denied_exe = executable(f, &first, "denied.exe");
    let denied = native::Denied::new(&denied_exe, false);
    let readable = executable(f, &second, "denied.exe");
    lookup(f, &setup, "denied", &readable);
    failed(
        f.eval(&format!("{} marker {}", qpath(&denied_exe), qpath(&marker))),
        true,
    );
    assert!(!marker.exists());
    drop(denied);
    // Replacement is synchronized by a foreground native child, not time/polling.
    let replace = executable(f, &first, "replace.exe");
    let invocation = format!(
        "{setup}; replace marker {}; {} n1-copy {} {}; replace -N -c 'echo replacement'",
        qpath(&marker),
        f.probe(),
        qpath(&f.fish),
        qpath(&replace)
    );
    exact(f.eval(&invocation), 0, b"ok\nreplacement\n", b"");
    assert_eq!(fs::read(&marker).unwrap(), b"launched");
    fs::remove_file(&marker).unwrap();
    // Relative PATH entries are keyed to authoritative fish cwd, not startup cwd.
    let one = f.root.join("one");
    let two = f.root.join("two");
    let one_exe = executable(f, &one.join("bin"), "relative.exe");
    let two_exe = executable(f, &two.join("bin"), "relative.exe");
    for (cwd, expected) in [(&one, &one_exe), (&two, &two_exe)] {
        lookup(
            f,
            &format!("cd {}; set -gx PATH bin", qpath(cwd)),
            "relative",
            expected,
        );
    }
    let out = success(f.eval(&format!(
        "set -gx PATH bin; cd {}; command -s relative; cd {}; command -s relative",
        qpath(&one),
        qpath(&two)
    )));
    let lines: Vec<_> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        fs::canonicalize(lines[0]).unwrap(),
        fs::canonicalize(one_exe).unwrap()
    );
    assert_eq!(
        fs::canonicalize(lines[1]).unwrap(),
        fs::canonicalize(two_exe).unwrap()
    );
    let case = f.root.join("case provider");
    native::case_sensitive(&case);
    let exact_case = executable(f, &case, "CaseOnly.exe");
    lookup(
        f,
        &format!("set -gx PATH {}", qpath(&case)),
        "CaseOnly",
        &exact_case,
    );
    failed(
        f.eval(&format!(
            "set -gx PATH {}; caseonly marker {}",
            qpath(&case),
            qpath(&marker)
        )),
        true,
    );
    assert!(!marker.exists());
    // Native PATH import is a list; export changes are observed by a real child fish.
    let code = "command -s select";
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    command.env("PATH", std::env::join_paths([&second, &first]).unwrap());
    same_path_output(run(&mut command, b""), &b);
    let parent = format!("{reversed}; {} -N -c {}", qpath(&f.fish), quote(code));
    same_path_output(f.eval(&parent), &b);
}
