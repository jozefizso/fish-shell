use super::*;
use std::fs::{File, FileTimes};
use std::time::{Duration, SystemTime};

pub(super) fn exercise(f: &Fixture) {
    let tree = f.root.join("tree 雪 🐟 \u{f600}\u{fdd0}");
    fs::create_dir(&tree).unwrap();
    let file = tree.join("sample.txt");
    fs::write(&file, b"native bytes").unwrap();
    let hard = tree.join("hard.txt");
    fs::hard_link(&file, &hard).unwrap();
    let original = fish::wutil::native_file_identity(&path_text(&file), true).unwrap();
    assert_eq!(
        original,
        fish::wutil::native_file_identity(&path_text(&hard), true).unwrap()
    );
    let raw = native::raw_identity(&file);
    assert_eq!(
        (original.volume, original.file_id),
        raw,
        "full native volume/128-bit identity lost"
    );
    assert_eq!(
        fish::path::native_resolve_path(&path_text(Path::new("sample.txt")), &path_text(&tree))
            .unwrap(),
        file
    );
    let drive = &tree.to_str().unwrap()[..2];
    let drive_relative = format!("{drive}sample.txt");
    let resolved =
        fish::path::native_resolve_path(&path_text(Path::new(&drive_relative)), &path_text(&tree))
            .unwrap();
    assert_eq!(
        fish::wutil::native_file_identity(&path_text(&resolved), true).unwrap(),
        original
    );
    let other_drive = if drive.eq_ignore_ascii_case("C:") {
        "D:"
    } else {
        "C:"
    };
    let other = fish::path::native_resolve_path(
        &path_text(Path::new(&format!("{other_drive}leaf"))),
        &path_text(&tree),
    )
    .unwrap();
    assert_eq!(other, PathBuf::from(format!("{other_drive}\\leaf")));
    let rooted = &file.to_str().unwrap()[2..];
    let resolved =
        fish::path::native_resolve_path(&path_text(Path::new(rooted)), &path_text(&tree)).unwrap();
    assert_eq!(
        fish::wutil::native_file_identity(&path_text(&resolved), true).unwrap(),
        original
    );
    let drive_root = PathBuf::from(format!("{drive}\\"));
    same_path_output(
        f.eval(&format!("cd {}; pwd", qpath(&drive_root))),
        &drive_root,
    );
    same_path_output(
        f.eval(&format!("cd {}; realpath ..", qpath(&drive_root))),
        &drive_root,
    );
    let extended = fs::canonicalize(&file).unwrap();
    same_path_output(f.eval(&format!("realpath {}", qpath(&extended))), &file);
    for leaf in ["literal. ", "NUL"] {
        let literal = fs::canonicalize(&tree).unwrap().join(leaf);
        fs::write(&literal, b"extended literal").unwrap();
        exact(f.eval(&format!("test -f {}", qpath(&literal))), 0, b"", b"");
        same_path_output(f.eval(&format!("realpath {}", qpath(&literal))), &literal);
        fs::remove_file(literal).unwrap();
    }
    same_path_output(
        f.eval(&format!(
            "realpath {}",
            quote(&format!("{}/", tree.to_str().unwrap().replace('\\', "/")))
        )),
        &tree,
    );
    let long_base = fs::canonicalize(&tree).unwrap();
    let mut long = long_base;
    for index in 0..5 {
        long.push(format!("component-{index}-{}", "x".repeat(48)));
        fs::create_dir(&long).unwrap();
    }
    let long_file = long.join("long 雪.txt");
    fs::write(&long_file, b"long").unwrap();
    assert!(long_file.as_os_str().len() > 260);
    same_path_output(
        f.eval(&format!("realpath {}", qpath(&long_file))),
        &long_file,
    );
    exact(
        f.eval(&format!("test -f {}", qpath(&long_file))),
        0,
        b"",
        b"",
    );
    let ordinary_long = PathBuf::from(long_file.to_str().unwrap().strip_prefix("\\\\?\\").unwrap());
    same_path_output(
        f.eval(&format!("realpath {}", qpath(&ordinary_long))),
        &long_file,
    );
    let unc = native::Unc::new(f);
    same_path_output(
        f.eval(&format!(
            "realpath {}",
            qpath(&unc.path.join("share 雪.txt"))
        )),
        &unc.path.join("share 雪.txt"),
    );
    same_path_output(
        f.eval(&format!("cd {}; pwd -P", qpath(&unc.path))),
        &unc.path,
    );

    options(f, &tree, &file, &hard);
    cwd(f, &tree);
    metadata(f, &file, &hard);
    let case = tree.join("case-sensitive");
    native::case_sensitive(&case);
    let upper = fish::wutil::native_file_identity(&path_text(&case.join("Twin")), true).unwrap();
    assert_ne!(
        upper,
        fish::wutil::native_file_identity(&path_text(&case.join("twin")), true).unwrap()
    );
    exact(
        f.eval(&format!(
            "test {} -ef {}",
            qpath(&case.join("Twin")),
            qpath(&case.join("twin"))
        )),
        1,
        b"",
        b"",
    );
    let denied_path = tree.join("denied.txt");
    let denied = native::Denied::new(&denied_path, false);
    let error = fish::wutil::native_file_identity(&path_text(&denied_path), true).unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(5),
        "native metadata DWORD domain lost"
    );
    let error =
        fish::wutil::waccess(&path_text(&denied_path), fish::wutil::AccessFlags::R_OK).unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(5),
        "native access DWORD domain lost"
    );
    failed(f.eval(&format!("realpath {}", qpath(&denied_path))), true);
    for command in ["test -r", "test -w", "path is -r", "path is -w"] {
        exact(
            f.eval(&format!("{command} {}", qpath(&denied_path))),
            1,
            b"",
            b"",
        );
    }
    drop(denied);
    for identity in [
        "NUL",
        "CON",
        "sample.txt:stream",
        "trailing.",
        "trailing ",
        "\\\\.\\PhysicalDrive0",
    ] {
        failed(f.eval(&format!("realpath {}", quote(identity))), true);
    }
    strict_identity(f);
    // Explicitly remove links before the fixture's recursive cleanup.
    fs::remove_dir(tree.join("junction")).unwrap();
    fs::remove_dir(tree.join("cycle")).unwrap();
}

fn redirected_input(f: &Fixture, command: &str, bytes: &[u8]) -> Captured {
    let input = f.root.join("filesystem-input.bin");
    fs::write(&input, bytes).unwrap();
    f.eval(&format!("{command} < {}", qpath(&input)))
}

fn options(f: &Fixture, tree: &Path, file: &Path, hard: &Path) {
    // This table is the executable option inventory for N1 portable path operations.
    for (command, expected) in [
        ("path basename a/b.txt", "b.txt\n"),
        ("path basename --no-extension a/b.txt", "b\n"),
        ("path basename -E a/b.txt", "b\n"),
        ("path dirname a/b.txt", "a\n"),
        ("path extension a/b.txt", ".txt\n"),
        ("path change-extension log a/b.txt", "a/b.log\n"),
        ("path normalize a/./b/../c", "a/c\n"),
        ("path normalize C:/a/../../b", "C:/b\n"),
        ("path normalize C:foo/../bar", "C:bar\n"),
        ("path normalize ../../a/../b", "../../b\n"),
        ("path normalize //server/share/a/../..", "//server/share/\n"),
        ("path sort b2 b10 b1", "b1\nb2\nb10\n"),
        ("path sort --reverse b2 b1", "b2\nb1\n"),
        ("path sort -r b2 b1", "b2\nb1\n"),
        ("path sort --unique b b a", "a\nb\n"),
        ("path sort -u b b a", "a\nb\n"),
        ("path sort --key=basename z/a a/z", "z/a\na/z\n"),
        ("path sort --key=dirname z/a a/z", "a/z\nz/a\n"),
    ] {
        exact(f.eval(command), 0, expected.as_bytes(), b"");
    }
    // Every subcommand's common output/input/quiet options go through the real builtin.
    for (command, input, output) in [
        ("path basename", "a/雪.txt", "雪.txt"),
        ("path dirname", "a/雪.txt", "a"),
        ("path extension", "a/雪.txt", ".txt"),
        ("path change-extension", "a/雪.txt", "a/雪.log"),
        ("path normalize", "a/./b", "a/b"),
        ("path sort", "a", "a"),
    ] {
        for (zi, zo, quiet) in [("-z", "-Z", "-q"), ("--null-in", "--null-out", "--quiet")] {
            let suffix = if command == "path change-extension" {
                " log"
            } else {
                ""
            };
            let input = format!("{input}\0");
            exact(
                redirected_input(f, &format!("{command} {zi} {zo}{suffix}"), input.as_bytes()),
                0,
                format!("{output}\0").as_bytes(),
                b"",
            );
            exact(
                redirected_input(
                    f,
                    &format!("{command} {zi} {quiet}{suffix}"),
                    input.as_bytes(),
                ),
                0,
                b"",
                b"",
            );
        }
    }
    let fq = qpath(file);
    for command in [
        "path resolve",
        "realpath",
        "realpath -s",
        "realpath --no-symlinks",
    ] {
        same_path_output(f.eval(&format!("{command} {fq}")), file);
    }
    for command in ["path resolve", "path mtime", "path filter", "path is"] {
        for (zi, zo, quiet) in [("-z", "-Z", "-q"), ("--null-in", "--null-out", "--quiet")] {
            let bytes = format!("{}\0", file.to_str().unwrap());
            exact(
                redirected_input(f, &format!("{command} {zi} {quiet}"), bytes.as_bytes()),
                0,
                b"",
                b"",
            );
            let result = success(redirected_input(
                f,
                &format!("{command} {zi} {zo}"),
                bytes.as_bytes(),
            ));
            match command {
                "path is" => assert_eq!(result, b""),
                "path filter" => assert_eq!(result, bytes.as_bytes()),
                "path mtime" => {
                    let seconds = fs::metadata(file)
                        .unwrap()
                        .modified()
                        .unwrap()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    assert_eq!(result, format!("{seconds}\0").as_bytes());
                }
                "path resolve" => {
                    let record = result.strip_suffix(&[0]).unwrap();
                    assert!(!record.contains(&0));
                    let resolved = Path::new(std::str::from_utf8(record).unwrap());
                    assert_eq!(native::raw_identity(resolved), native::raw_identity(file));
                }
                _ => unreachable!(),
            }
        }
    }
    for command in [
        "path filter -f",
        "path filter --type=file",
        "path filter -r",
        "path filter --perm=read",
        "path filter -w",
        "path filter --perm=write",
    ] {
        exact(
            f.eval(&format!("{command} {fq}")),
            0,
            format!("{}\n", file.to_str().unwrap()).as_bytes(),
            b"",
        );
    }
    for command in [
        "path is -f",
        "path is --type=file",
        "path is --all -f",
        "path is -r",
        "path is -w",
    ] {
        exact(f.eval(&format!("{command} {fq}")), 0, b"", b"");
    }
    for command in ["path is -d", "path is --type=dir"] {
        exact(f.eval(&format!("{command} {}", qpath(tree))), 0, b"", b"");
    }
    for command in ["path filter -v -d", "path filter --invert --type=dir"] {
        exact(
            f.eval(&format!("{command} {fq}")),
            0,
            format!("{}\n", file.to_str().unwrap()).as_bytes(),
            b"",
        );
    }
    exact(
        f.eval(&format!("path filter --all -f {fq} {}", qpath(hard))),
        0,
        b"",
        b"",
    );
    exact(
        f.eval(&format!("path filter --all -f {fq} {}", qpath(tree))),
        1,
        b"",
        b"",
    );
    exact(
        f.eval(&format!(
            "path is --all {fq} {}",
            qpath(&tree.join("missing"))
        )),
        1,
        b"",
        b"",
    );
    for perm in ["suid", "sgid", "user", "group"] {
        let result = f.eval(&format!("path is --perm={perm} {fq}"));
        assert_eq!(result.code, 2);
        failed(result, true);
        let result = f.eval(&format!("path filter --invert --perm={perm} {fq}"));
        assert_eq!(result.code, 2);
        failed(result, true);
    }
    for kind in ["block", "char", "fifo", "socket"] {
        let result = f.eval(&format!("path is --type={kind} {fq}"));
        assert_eq!(result.code, 2);
        failed(result, true);
    }
    for predicate in ["-e", "-f", "-s", "-r", "-w"] {
        exact(f.eval(&format!("test {predicate} {fq}")), 0, b"", b"");
    }
    exact(f.eval(&format!("test -d {}", qpath(tree))), 0, b"", b"");
    exact(
        f.eval(&format!("test {fq} -ef {}", qpath(hard))),
        0,
        b"",
        b"",
    );
    exact(f.eval(&format!("test -x {}", qpath(&f.probe))), 0, b"", b"");
    exact(
        f.eval(&format!("path is -x {}", qpath(&f.probe))),
        0,
        b"",
        b"",
    );
    exact(f.eval(&format!("test -x {fq}")), 1, b"", b"");
    for predicate in ["-b", "-c", "-p", "-S", "-u", "-g", "-k", "-O", "-G"] {
        let result = f.eval(&format!("test {predicate} {fq}"));
        assert_eq!(result.code, 2);
        failed(result, true);
    }
    failed(
        f.eval(&format!("realpath {}", qpath(&tree.join("missing/leaf")))),
        true,
    );
    // Only a missing final component is lexical; missing parents must still fail.
    let missing = tree.join("missing");
    for command in ["path resolve", "realpath"] {
        let result = success(f.eval(&format!("{command} {}", qpath(&missing))));
        let text = std::str::from_utf8(&result)
            .unwrap()
            .strip_suffix('\n')
            .unwrap();
        let resolved = Path::new(text);
        assert_eq!(resolved.file_name(), missing.file_name());
        assert_eq!(
            fs::canonicalize(resolved.parent().unwrap()).unwrap(),
            fs::canonicalize(tree).unwrap()
        );
    }
    exact(f.eval(&format!("test -e {}", qpath(&missing))), 1, b"", b"");
    failed(
        f.eval(&format!(
            "path resolve {}",
            qpath(&tree.join("missing/leaf"))
        )),
        false,
    );
}

fn metadata(f: &Fixture, file: &Path, hard: &Path) {
    // Exact 100ns-representable fraction exposes epoch and unit mistakes without sleeps.
    let stamp = SystemTime::UNIX_EPOCH + Duration::new(1_600_000_123, 456_789_000);
    File::options()
        .write(true)
        .open(file)
        .unwrap()
        .set_times(FileTimes::new().set_modified(stamp))
        .unwrap();
    let id = fish::wutil::file_id_for_path(&path_text(file)).unwrap();
    assert_eq!(id.mod_seconds, 1_600_000_123);
    assert_eq!(id.mod_nanoseconds, 456_789_000);
    let handle = File::open(file).unwrap();
    assert_eq!(id, fish::wutil::file_id_for_file(&handle).unwrap());
    assert_eq!(id, fish::wutil::file_id_for_path(&path_text(hard)).unwrap());
    exact(
        f.eval(&format!("path mtime {}", qpath(file))),
        0,
        b"1600000123\n",
        b"",
    );
    for option in ["-R", "--relative"] {
        let before = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - id.mod_seconds;
        let result = success(f.eval(&format!("path mtime {option} {}", qpath(file))));
        let age: i64 = std::str::from_utf8(&result)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let after = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - id.mod_seconds;
        assert!((before..=after).contains(&age));
    }
    let newer = file.with_file_name("newer.txt");
    fs::write(&newer, b"newer").unwrap();
    File::options()
        .write(true)
        .open(&newer)
        .unwrap()
        .set_times(FileTimes::new().set_modified(stamp + Duration::from_secs(1)))
        .unwrap();
    exact(
        f.eval(&format!("test {} -nt {}", qpath(&newer), qpath(file))),
        0,
        b"",
        b"",
    );
    exact(
        f.eval(&format!("test {} -ot {}", qpath(file), qpath(&newer))),
        0,
        b"",
        b"",
    );
    assert_eq!(
        id.native_identity,
        fish::wutil::native_file_identity(&path_text(file), true).unwrap()
    );
    let historical = file.with_file_name("before-epoch.txt");
    fs::write(&historical, b"historical").unwrap();
    File::options()
        .write(true)
        .open(&historical)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(SystemTime::UNIX_EPOCH - Duration::from_millis(1500)),
        )
        .unwrap();
    let historic = fish::wutil::file_id_for_path(&path_text(&historical)).unwrap();
    assert_eq!(
        (historic.mod_seconds, historic.mod_nanoseconds),
        (-2, 500_000_000)
    );
    exact(
        f.eval(&format!("path mtime {}", qpath(&historical))),
        0,
        b"-2\n",
        b"",
    );
}

fn cwd(f: &Fixture, tree: &Path) {
    let target = tree.join("actual");
    fs::create_dir(&target).unwrap();
    fs::create_dir(target.join("nested")).unwrap();
    let symlink = tree.join("symlink");
    std::os::windows::fs::symlink_dir(target.join("nested"), &symlink).expect(
        "required symlink capability: enable Developer Mode or SeCreateSymbolicLinkPrivilege",
    );
    same_path_output(
        f.eval(&format!("cd -P {}; pwd -P", qpath(&symlink))),
        &target.join("nested"),
    );
    same_path_output(
        f.eval(&format!("cd -L {}; cd ..; pwd -L", qpath(&symlink))),
        tree,
    );
    let broken = tree.join("broken-link");
    std::os::windows::fs::symlink_file(tree.join("absent-target"), &broken)
        .expect("required broken symlink fixture");
    for command in ["test -L", "test -h", "path is -l"] {
        exact(
            f.eval(&format!("{command} {}", qpath(&broken))),
            0,
            b"",
            b"",
        );
    }
    exact(f.eval(&format!("test -e {}", qpath(&broken))), 1, b"", b"");
    failed(f.eval(&format!("realpath {}", qpath(&broken))), true);
    fs::remove_file(&broken).unwrap();
    fs::remove_dir(&symlink).unwrap();
    let link = tree.join("junction");
    native::junction(&link, &target.join("nested"));
    native::junction(&tree.join("cycle"), tree);
    same_path_output(
        f.eval(&format!("cd -L {}; cd ..; pwd -L", qpath(&link))),
        tree,
    );
    same_path_output(
        f.eval(&format!("cd -P {}; cd ..; pwd -P", qpath(&link))),
        &target,
    );
    same_path_output(
        f.eval(&format!(
            "cd --no-dereference {}; cd ..; pwd --logical",
            qpath(&link)
        )),
        tree,
    );
    same_path_output(
        f.eval(&format!(
            "cd --dereference {}; cd ..; pwd --physical",
            qpath(&link)
        )),
        &target,
    );
    same_path_output(
        f.eval(&format!("cd {}; realpath sample.txt", qpath(tree))),
        &tree.join("sample.txt"),
    );
    exact(
        f.eval(&format!("cd {}; test -f sample.txt", qpath(tree))),
        0,
        b"",
        b"",
    );
    same_path_output(f.eval("cd; pwd"), &f.state.join("home"));
    same_path_output(
        f.eval(&format!("realpath {}", qpath(&link))),
        &target.join("nested"),
    );
    for opt in ["-s", "--no-symlinks"] {
        let out = success(f.eval(&format!("realpath {opt} {}", qpath(&link))));
        let path = PathBuf::from(std::str::from_utf8(&out).unwrap().trim());
        assert_eq!(
            path.file_name(),
            link.file_name(),
            "no-symlinks erased lexical link identity"
        );
    }
    for command in ["path is -l", "path is --type=link", "test -L", "test -h"] {
        exact(f.eval(&format!("{command} {}", qpath(&link))), 0, b"", b"");
    }
    same_path_output(
        f.eval(&format!("set -gx CDPATH {}; cd actual; pwd", qpath(tree))),
        &target,
    );
    same_path_output(
        f.eval(&format!(
            "set -gx CDPATH {}; cd {}; pwd",
            qpath(&target),
            qpath(tree)
        )),
        tree,
    );
    same_path_output(f.eval(&format!("{}; pwd", qpath(tree))), tree); // implicit cd
    let out = success(f.eval(&format!(
        "cd {}; printf '%s\\n' $PWD; {} report",
        qpath(tree),
        f.probe()
    )));
    let text = std::str::from_utf8(&out).unwrap();
    let mut lines = text.lines();
    let pwd = lines.next().unwrap();
    let cwd_hex = lines.next().unwrap().strip_prefix("cwd=").unwrap();
    let child_cwd = String::from_utf8(
        (0..cwd_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cwd_hex[i..i + 2], 16).unwrap())
            .collect(),
    )
    .unwrap();
    assert_eq!(
        fs::canonicalize(pwd).unwrap(),
        fs::canonicalize(&child_cwd).unwrap()
    );
    let result = f.eval(&format!(
        "cd {}; cd missing; echo $status; pwd",
        qpath(tree)
    ));
    assert_eq!(result.code, 0);
    assert!(!result.err.is_empty());
    let text = String::from_utf8(result.out).unwrap();
    let (status, pwd) = text.trim_end().split_once('\n').unwrap();
    assert_ne!(status, "0");
    assert_eq!(
        fs::canonicalize(pwd).unwrap(),
        fs::canonicalize(tree).unwrap()
    );
    let mutable = f.root.join("n1-rename");
    let renamed = f.root.join("n1-rename-moved");
    fs::create_dir(&mutable).unwrap();
    fs::create_dir(mutable.join("nested")).unwrap();
    fs::write(mutable.join("retained.txt"), b"retained directory").unwrap();
    let out = success(f.eval(&format!(
        "cd {}; {} n1-rename {} {}; printf 'logical=%s\\n' $PWD; pwd -P; {} report; {} n1-copy {} {}; test -d nested; echo metadata=$status; printf 'glob=%s\\n' *.txt; cd nested; pwd -P; cd {}; pwd",
        qpath(&mutable), f.probe(), qpath(&mutable), qpath(&renamed), f.probe(),
        f.probe(), qpath(&f.probe), qpath(&mutable), qpath(&f.root))));
    let lines: Vec<_> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(lines.len(), 9);
    assert_eq!(lines[0], "ok", "required live-cwd rename fixture failed");
    assert_eq!(
        Path::new(lines[1].strip_prefix("logical=").unwrap()),
        mutable
    );
    let physical = &renamed;
    assert_eq!(
        fs::canonicalize(lines[2]).unwrap(),
        fs::canonicalize(physical).unwrap()
    );
    let record = lines[3].strip_prefix("cwd=").unwrap();
    let child = String::from_utf8(
        (0..record.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&record[i..i + 2], 16).unwrap())
            .collect(),
    )
    .unwrap();
    assert_eq!(
        fs::canonicalize(child).unwrap(),
        fs::canonicalize(physical).unwrap()
    );
    assert_eq!(lines[4], "ok", "old cwd name replacement failed");
    assert_eq!(
        lines[5], "metadata=0",
        "relative metadata used the stale cwd spelling"
    );
    assert_eq!(
        lines[6], "glob=retained.txt",
        "relative glob used the stale cwd spelling"
    );
    assert_eq!(
        fs::canonicalize(lines[7]).unwrap(),
        fs::canonicalize(renamed.join("nested")).unwrap()
    );
    assert_eq!(
        fs::canonicalize(lines[8]).unwrap(),
        fs::canonicalize(&f.root).unwrap()
    );
    let deleted = f.root.join("n1-remove-dir");
    let marker = f.root.join("deleted-cwd-child");
    fs::create_dir(&deleted).unwrap();
    let result = f.eval(&format!("cd {}; {} n1-remove-dir {}; printf 'logical=%s\\n' $PWD; pwd -P; test -d {}; echo absolute=$status; path is -d {}; echo absolute_path=$status; {} marker {}; echo child=$status; cd {}; pwd",
        qpath(&deleted), f.probe(), qpath(&deleted), qpath(&f.root), qpath(&f.root), f.probe(), qpath(&marker), qpath(&f.root)));
    assert_eq!(result.code, 0);
    let text = std::str::from_utf8(&result.out).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(
        Path::new(lines[1].strip_prefix("logical=").unwrap()),
        deleted
    );
    assert_eq!(lines[0], "ok", "required deleted-cwd fixture failed");
    assert_eq!(lines.len(), 6, "deleted cwd must not fabricate pwd -P");
    assert!(!result.err.is_empty());
    assert_eq!(lines[2], "absolute=0");
    assert_eq!(lines[3], "absolute_path=0");
    assert_ne!(lines[4], "child=0");
    assert!(!marker.exists(), "child launched with a replacement cwd");
    assert_eq!(
        fs::canonicalize(lines.last().unwrap()).unwrap(),
        fs::canonicalize(&f.root).unwrap()
    );
    let parent_escape = f.root.join("n1-remove-parent-escape");
    fs::create_dir(&parent_escape).unwrap();
    let out = success(f.eval(&format!(
        "cd {}; {} n1-remove-dir {}; cd ..; pwd -P",
        qpath(&parent_escape),
        f.probe(),
        qpath(&parent_escape)
    )));
    let text = std::str::from_utf8(&out).unwrap();
    let (removed, parent) = text.trim_end().split_once('\n').unwrap();
    assert_eq!(removed, "ok");
    assert_eq!(
        fs::canonicalize(parent).unwrap(),
        fs::canonicalize(&f.root).unwrap()
    );
}

fn strict_identity(f: &Fixture) {
    for invalid in ["\\xFF", "\\u0000", "\\uD800", "\\U00110000"] {
        failed(f.eval(&format!("realpath {invalid}")), true);
    }
    use std::os::windows::ffi::OsStringExt;
    let invalid = std::ffi::OsString::from_wide(&[0xd800]);
    let code = OsStr::new("realpath $argv[1]");
    failed(
        run(
            &mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), code, &invalid]),
            b"",
        ),
        true,
    );
}
