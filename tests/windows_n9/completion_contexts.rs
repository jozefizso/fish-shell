use super::{Family, fixture::{Captured, Fixture, exact, quote, run}};
use std::{collections::BTreeSet, ffi::OsStr, fmt::Write as _, fs, path::PathBuf, process::Command};
use std::os::windows::fs::MetadataExt;

pub(super) const FAMILIES: &[Family] = &[
    ("native-sort-and-attrib-identity-and-filesystem-state", filesystem_cli),
    ("native-network-cli-address-family-and-argument-context", network_cli),
    ("python-real-modules-repeated-options-and-payload-boundary", python),
    ("salesforce-real-manifest-tree-without-find-or-cli", manifests),
    ("native-repository-completion-ambiguity-and-quoted-context", repositories),
    ("native-findstr-color-boundaries-files-and-search-status", search),
    ("native-ssh-owned-config-key-descriptors-and-history", ssh_descriptors),
    ("native-profile-document-and-device-descriptor-boundaries", descriptors),
    ("native-provider-completion-account-process-address-boundaries", provider_contexts),
];

fn success(result: Captured, context: &str) -> String {
    assert_eq!(result.code, 0, "{context}: {}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{context}: {}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn candidates(output: &str) -> BTreeSet<String> {
    output.lines().map(|line| line.split('\t').next().unwrap().to_owned()).collect()
}
fn table_source(names: &[&str]) -> String {
    let mut source = String::new();
    for name in names {
        source.push_str("status get-file completions/");
        source.push_str(name);
        source.push_str(".fish | source; ");
    }
    source
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes { write!(out, "{byte:02x}").unwrap(); }
    out
}
fn wide_argument(f: &Fixture, argument: &str) {
    let observed = success(f.eval(&format!("{} report {}", f.probe(), quote(argument))), "independent wide-native argv transport");
    assert!(observed.lines().any(|line| line == format!("arg0={}", hex(argument.as_bytes()))), "{observed}");
}
fn direct_native(f: &Fixture, executable: &std::path::Path, args: &[&OsStr]) -> Captured {
    let mut command = Command::new(executable);
    command.args(args).env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(key, value); }
    }
    super::functions_tools::tool_environment(&mut command, executable);
    run(&mut command, b"")
}
fn legacy_unicode_result(f: &Fixture, name: &str, argument: &str, expected: &str, fish: Captured, direct: Captured) {
    wide_argument(f, argument);
    fs::write(f.root.join(format!("{name}-unicode-consumer.txt")), format!(
        "wide_argument_utf8_hex={}\ndirect_exit={}\nfish_exit={}\ndirect_stdout_hex={}\nfish_stdout_hex={}\ndirect_stderr_hex={}\nfish_stderr_hex={}\n",
        hex(argument.as_bytes()), direct.code, fish.code, hex(&direct.out), hex(&fish.out), hex(&direct.err), hex(&fish.err)
    )).unwrap();
    assert_eq!(fish.code, direct.code, "{name}: fish must preserve the real tool's Unicode-path result");
    assert_eq!(fish.out, direct.out, "{name}: native stdout differs from direct CreateProcessW");
    assert_eq!(fish.err, direct.err, "{name}: native diagnostics differ from direct CreateProcessW");
    if direct.code == 0 {
        assert_eq!(String::from_utf8(direct.out).unwrap().replace("\r\n", "\n"), expected, "{name}: accepted Unicode input must still produce the real data result");
    } else {
        eprintln!("LIMIT {name}: the real legacy executable also rejects this Unicode path when launched directly with wide argv; transport is independently proven and diagnostics are retained");
    }
}
fn system_path() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").expect("native Windows system root")).join("System32")
}
fn native(f: &Fixture, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    super::functions_tools::tool_environment(&mut command, &system_path().join("cmd.exe"));
    run(&mut command, b"")
}
fn native_candidates(f: &Fixture, line: &str) -> BTreeSet<String> {
    candidates(&success(native(f, &format!("complete -C {}", quote(line))), line))
}

fn filesystem_cli(f: &Fixture) {
    let input = f.root.join("sort input 雪.txt");
    fs::write(&input, b"alpha\r\nzeta\r\nbeta\r\n").unwrap();
    let options = native_candidates(f, "sort /");
    for option in ["/r", "/+", "/m", "/l", "/rec", "/t", "/o", "/unique", "/?"] {
        assert!(options.contains(option), "native sort option {option}: {options:?}");
    }
    assert!(native_candidates(f, "sort --").is_empty(), "System32 sort must not advertise GNU switches");
    assert_eq!(native_candidates(f, "sort /l "), BTreeSet::from(["C".to_owned()]));
    fs::create_dir(f.root.join("n9-sort-temp")).unwrap();
    fs::write(f.root.join("n9-sort-not-a-directory"), b"").unwrap();
    assert_eq!(native_candidates(f, "sort /t n9-sort-"), BTreeSet::from(["n9-sort-temp/".to_owned()]));
    assert!(native_candidates(f, "sort /m ").is_empty(), "numeric memory argument must not suggest filenames or switches");
    let argument = input.to_str().unwrap();
    let unicode = native(f, &format!("command sort /r {}", quote(argument)));
    let direct = direct_native(f, &system_path().join("sort.exe"), &[OsStr::new("/r"), input.as_os_str()]);
    legacy_unicode_result(f, "sort", argument, "zeta\nbeta\nalpha\n", unicode, direct);
    fs::write(f.root.join("sort-input.txt"), b"alpha\r\nzeta\r\nbeta\r\n").unwrap();
    let output = success(native(f, "command sort /r sort-input.txt"), "real native sort data in the same Unicode working directory");
    assert_eq!(output.replace("\r\n", "\n"), "zeta\nbeta\nalpha\n");
    let portable = candidates(&success(f.eval(&format!("{}complete -C 'sort --rev'", table_source(&["sort"]))), "explicit portable sort table without an installed command"));
    assert!(portable.contains("--reverse"), "absent native sort must retain the portable table");

    let document = f.root.join("attribute 雪.txt");
    fs::write(&document, b"owned attribute fixture").unwrap();
    let set = native_candidates(f, "attrib +");
    let clear = native_candidates(f, "attrib -");
    for attribute in ["r", "a", "s", "h", "o", "i", "x", "p", "u", "b"] {
        assert!(set.contains(&format!("+{attribute}")), "{set:?}");
        assert!(clear.contains(&format!("-{attribute}")), "{clear:?}");
        assert!(!set.contains(&format!("-{attribute}")));
    }
    assert!(!native_candidates(f, "attrib +r").contains("+rr"), "attributes are separate arguments, not a combined switch");
    let recursive = native_candidates(f, "attrib /s /");
    assert!(recursive.contains("/d") && recursive.contains("/l"));
    let file_candidates = native_candidates(f, "attrib attribute");
    assert!(file_candidates.iter().any(|candidate| candidate.contains("attribute") && candidate.contains("雪.txt")), "native attrib must still complete filenames: {file_candidates:?}");
    let ascii_document = f.root.join("attribute.txt");
    fs::write(&ascii_document, b"owned attribute fixture").unwrap();
    for (label, document) in [("unicode", document), ("ascii-name", ascii_document)] {
        let argument = document.to_str().unwrap();
        wide_argument(f, argument);
        let path = quote(argument);
        let attrib = system_path().join("attrib.exe");
        let direct = direct_native(f, &attrib, &[OsStr::new("+r"), document.as_os_str()]);
        let direct_read_only = fs::metadata(&document).unwrap().file_attributes() & 1;
        success(direct_native(f, &attrib, &[OsStr::new("-r"), document.as_os_str()]), "clear direct native read-only observation");
        let actual = native(f, &format!("command attrib +r {path}"));
        let read_only = fs::metadata(&document).unwrap().file_attributes() & 1;
        let cleared = native(f, &format!("command attrib -r {path}"));
        let clear_attribute = fs::metadata(&document).unwrap().file_attributes() & 1;
        fs::write(f.root.join(format!("attrib-{label}-native-state.txt")), format!(
            "wide_argument_utf8_hex={}\ndirect_exit={}\nfish_exit={}\ndirect_stdout_hex={}\nfish_stdout_hex={}\ndirect_stderr_hex={}\nfish_stderr_hex={}\ndirect_read_only={direct_read_only}\nfish_read_only={read_only}\nfish_clear_read_only={clear_attribute}\n",
            hex(argument.as_bytes()), direct.code, actual.code, hex(&direct.out), hex(&actual.out), hex(&direct.err), hex(&actual.err)
        )).unwrap();
        exact(direct, 0, b"", b"");
        exact(actual, 0, b"", b"");
        exact(cleared, 0, b"", b"");
        assert_eq!(direct_read_only, 1, "direct wide native attrib must mutate the owned {label} file");
        assert_eq!(read_only, direct_read_only, "fish native attrib must preserve the actual attribute mutation for {label}");
        assert_eq!(clear_attribute, 0, "native attrib must clear read-only for {label}");
    }
}

fn network_cli(f: &Fixture) {
    let ping = native_candidates(f, "ping -");
    assert!(ping.contains("-n") && ping.contains("-w") && ping.contains("-S"), "actual native ping candidates: {ping:?}");
    assert!(!ping.contains("-c") && !ping.contains("-I"), "native ping must not advertise Unix count/interface switches: {ping:?}");
    assert_eq!(native_candidates(f, "ping -r "), (1..=9).map(|n| n.to_string()).collect());
    assert_eq!(native_candidates(f, "ping -s "), (1..=4).map(|n| n.to_string()).collect());
    success(native(f, "command ping -n 1 -w 1000 -4 127.0.0.1"), "real native loopback ping");

    let addresses = success(native(f, "__fish_print_addresses"), "actual native interface address provider");
    let addresses = candidates(&addresses);
    let ipv4: BTreeSet<_> = addresses.iter().filter(|address| !address.contains(':')).cloned().collect();
    let ipv6: BTreeSet<_> = addresses.iter().filter(|address| address.contains(':')).cloned().collect();
    assert_eq!(native_candidates(f, "arp -a -N "), ipv4);
    assert_eq!(native_candidates(f, "ping -S "), ipv6);
    let arp = native_candidates(f, "arp -");
    assert!(arp.contains("-N") && arp.contains("-g"));
    assert!(!arp.contains("--device") && !arp.contains("--numeric") && !arp.contains("-i"));
    let query = native(f, "command arp -a");
    assert!(query.code <= 1, "actual read-only ARP query: {}", String::from_utf8_lossy(&query.err));
    let source = table_source(&["ping", "arp"]);
    for line in ["ping -", "ping -S ", "arp -"] {
        success(f.eval(&format!("{source}complete -C {}", quote(line))), "explicit table with optional network executable absent");
    }
    let absent_ping = candidates(&success(f.eval(&format!("{source}complete -C 'ping -'")), "native ping metadata survives absent executable"));
    assert!(absent_ping.contains("-n") && !absent_ping.contains("-c"));
    let absent_arp = candidates(&success(f.eval(&format!("{source}complete -C 'arp -'")), "native arp metadata survives absent executable"));
    assert!(absent_arp.contains("-N") && !absent_arp.contains("--numeric"));
}

fn python(f: &Fixture) {
    let program = PathBuf::from(std::env::var_os("FISH_N9_PYTHON").expect("required actual native Python"));
    assert!(program.is_file());
    let project = f.root.join("Python modules 雪");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("n9_local_module.py"), "print('module fixture 雪')\n").unwrap();
    fs::create_dir(project.join("n9_local_package")).unwrap();
    fs::write(project.join("n9_local_package/__init__.py"), "").unwrap();
    fs::write(project.join("n9_local_package/__main__.py"), "print('package fixture')\n").unwrap();
    fs::write(project.join("n9_script.py"), "print('script fixture')\n").unwrap();
    let invoke = |code: &str| {
        let code = format!("cd {}; {code}", quote(project.to_str().unwrap()));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", program.parent().unwrap()).env("PYTHONUTF8", "1");
        run(&mut command, b"")
    };
    for interpreter in ["python", "python3"] {
        let line = format!("{interpreter} -X utf8 -X dev -W ignore -W default -m n9_local_");
        assert_eq!(candidates(&success(invoke(&format!("complete -C {}", quote(&line))), &line)), BTreeSet::from(["n9_local_module".to_owned(), "n9_local_package".to_owned()]));
    }
    for line in ["python -m n9_local_", "python -X utf8 -X dev -W ignore -W default -m n9_local_"] {
        let actual = candidates(&success(invoke(&format!("complete -C {}", quote(line))), line));
        assert_eq!(actual, BTreeSet::from(["n9_local_module".to_owned(), "n9_local_package".to_owned()]));
    }
    wide_argument(f, project.to_str().unwrap());
    let mut direct = Command::new(&program);
    direct.args(["-X", "utf8", "-X", "dev", "-m", "n9_local_module"]).env_clear().current_dir(&project);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { direct.env(key, value); }
    }
    direct.env("PATH", program.parent().unwrap()).env("PYTHONUTF8", "1");
    let direct = run(&mut direct, b"");
    let actual = invoke("command python -X utf8 -X dev -m n9_local_module");
    fs::write(f.root.join("python-repeated-options-native-query.txt"), format!(
        "program={}\nworking_directory={}\ndirect_exit={}\nfish_exit={}\ndirect_stdout_hex={}\nfish_stdout_hex={}\ndirect_stderr_hex={}\nfish_stderr_hex={}\n",
        program.display(), project.display(), direct.code, actual.code, hex(&direct.out), hex(&actual.out), hex(&direct.err), hex(&actual.err)
    )).unwrap();
    assert_eq!(success(direct, "direct wide-native Python repeated options and module").replace("\r\n", "\n"), "module fixture 雪\n");
    assert_eq!(success(actual, "actual repeated Python options and module").replace("\r\n", "\n"), "module fixture 雪\n");
    assert_eq!(success(invoke("command python -m n9_local_package"), "actual Python package").replace("\r\n", "\n"), "package fixture\n");
    assert_eq!(candidates(&success(invoke("complete -C 'python n9_script'"), "actual Python script suffix")), BTreeSet::from(["n9_script.py".to_owned()]));
    assert_eq!(success(invoke("command python n9_script.py"), "actual Python script execution").replace("\r\n", "\n"), "script fixture\n");
    for line in ["python n9_script.py --hel", "python -m n9_local_module --hel", "python -c 'print(1)' --hel", "python - --hel", "python -- n9_script.py --hel"] {
        assert!(candidates(&success(invoke(&format!("complete -C {}", quote(line))), line)).is_empty(), "interpreter options must stop before payload arguments: {line}");
    }
    for command in ["python", "python3"] {
        let code = format!("{}complete -C {}; complete -C {}", table_source(&["python", "python3"]), quote(&format!("{command} --hel")), quote(&format!("{command} -m n9_unavailable_")));
        let output = success(f.eval(&code), "absent Python preserves static help without module probes");
        assert!(candidates(&output).contains("--help"));
    }
}

fn manifests(f: &Fixture) {
    let project = f.root.join("Salesforce descriptors 雪");
    for directory in ["manifest", "nested/雪", ".hidden/deeper", "not-a-file/package.xml"] {
        fs::create_dir_all(project.join(directory)).unwrap();
    }
    let expected: BTreeSet<_> = ["package.xml", "manifest/package.xml", "nested/雪/package.xml", ".hidden/deeper/package.xml"].into_iter().map(str::to_owned).collect();
    for path in &expected {
        fs::write(project.join(path), "<?xml version=\"1.0\"?><Package xmlns=\"http://soap.sforce.com/2006/04/metadata\"><version>66.0</version></Package>").unwrap();
    }
    fs::write(project.join("manifest/not-package.xml"), "unrelated").unwrap();
    let source = table_source(&["sfdx"]);
    let invoke = |line: &str| {
        let code = format!("{source}cd {}; complete -C {}", quote(project.to_str().unwrap()), quote(line));
        candidates(&success(f.eval(&code), line))
    };
    let all = invoke("sfdx force:source:deploy --manifest ");
    assert_eq!(all, expected, "actual descriptor tree, including hidden directories, without find or sfdx");
    fs::remove_file(project.join("manifest/package.xml")).unwrap();
    assert!(!invoke("sfdx force:source:deploy --manifest ").contains("manifest/package.xml"), "manifest completion must observe deletion");
    assert!(invoke("sfdx force:source:deploy --loglevel ").contains("debug"));
    let empty = project.join("empty");
    fs::create_dir(&empty).unwrap();
    exact(f.eval(&format!("{source}cd {}; complete -C 'sfdx force:source:deploy --manifest '", quote(empty.to_str().unwrap()))), 0, b"", b"");
}

fn repositories(f: &Fixture) {
    let storage = super::functions_tools::OptionalStorage::new();
    let mut failures = Vec::new();
    let mut record = |name: &'static str, exercise: &mut dyn FnMut()| {
        eprintln!("RUN-SUBCONSUMER completion_contexts::repositories/{name}");
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(exercise)).is_ok() {
            eprintln!("PASS-SUBCONSUMER completion_contexts::repositories/{name}");
        } else {
            eprintln!("FAIL-SUBCONSUMER completion_contexts::repositories/{name}");
            failures.push(name);
        }
    };
    let setup = |result: Captured, context: &str| {
        assert_eq!(result.code, 0, "{context}: {}", String::from_utf8_lossy(&result.err));
    };
    let invoke = |tool: &str, directory: &std::path::Path, code: &str| {
        let program = PathBuf::from(std::env::var_os(format!("FISH_N9_{tool}")).unwrap_or_else(|| panic!("required actual native {tool}")));
        assert!(program.is_file());
        let code = format!("cd {}; {code}", quote(directory.to_str().unwrap()));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", program.parent().unwrap()).env("HGENCODING", "utf-8").env("PYTHONUTF8", "1");
        if tool == "SVN" { storage.environment(&mut command); }
        if tool == "GIT" {
            command.env("GIT_CONFIG_COUNT", "1").env("GIT_CONFIG_KEY_0", "core.longpaths").env("GIT_CONFIG_VALUE_0", "true");
        }
        run(&mut command, b"")
    };
    let direct = |label: &str, program: &std::path::Path, directory: &std::path::Path, args: &[&OsStr], ascii_environment: bool| {
        assert!(program.is_file(), "required genuine native producer {}", program.display());
        let mut command = std::process::Command::new(program);
        command.args(args).current_dir(directory).env_clear();
        for (key, value) in f.command(&[]).get_envs() {
            if let Some(value) = value { command.env(key, value); }
        }
        command.env("PATH", program.parent().unwrap());
        if ascii_environment { storage.environment(&mut command); }
        let result = run(&mut command, b"");
        fs::write(f.root.join(format!("{label}.stdout")), &result.out).unwrap();
        fs::write(f.root.join(format!("{label}.stderr")), &result.err).unwrap();
        fs::write(f.root.join(format!("{label}.status")), result.code.to_string()).unwrap();
        eprintln!("NATIVE-VCS-BASELINE {label} cwd={} argv={args:?} status={} stdout={} stderr={}", directory.display(), result.code, String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
        result
    };
    let outer = f.root.join("outside repositories");
    record("git", &mut || {
        let git = f.root.join("Git completion repository 雪");
        fs::create_dir(&git).unwrap();
        setup(invoke("GIT", &git, "git init -q -b local; and git -c user.name=N9 -c user.email=n9@example.invalid commit -q --allow-empty -m initial; and git update-ref refs/remotes/one/n9-unique HEAD; and git update-ref refs/remotes/one/n9-ambiguous HEAD; and git update-ref refs/remotes/two/n9-ambiguous HEAD"), "real Git remote refs");
        let branches = candidates(&success(invoke("GIT", &git, "complete -C 'git switch n9-'"), "Git unique remote branch completion without uniq"));
        assert!(branches.contains("n9-unique") && !branches.contains("n9-ambiguous"), "{branches:?}");
        fs::create_dir(git.join("n9-tree")).unwrap();
        fs::write(git.join("n9-tree/n9-leaf-雪.txt"), b"revision tree fixture").unwrap();
        setup(invoke("GIT", &git, "git add n9-tree; and git -c user.name=N9 -c user.email=n9@example.invalid commit -q -m tree"), "actual Git revision tree");
        assert!(candidates(&success(invoke("GIT", &git, "complete -C 'git show HEAD:n9-tree/n9-'"), "Git revision path without sed")).contains("HEAD:n9-tree/n9-leaf-雪.txt"));
        success(invoke("GIT", &git, "git update-ref -d refs/remotes/two/n9-ambiguous"), "remove actual conflicting remote ref");
        assert!(candidates(&success(invoke("GIT", &git, "complete -C 'git switch n9-'"), "Git ambiguity removed")).contains("n9-ambiguous"));
        fs::create_dir_all(&outer).unwrap();
        let line = format!("git -C {} switch n9-", quote(git.to_str().unwrap()));
        assert!(candidates(&success(invoke("GIT", &outer, &format!("complete -C {}", quote(&line))), "Git quoted global directory")).contains("n9-unique"));
        setup(invoke("GIT", &git, "git config user.name N9; and git config user.email n9@example.invalid; and git config n9.context actual; and git config alias.n9-checkout checkout; and git branch n9-local; and git tag n9-tag; and git remote add n9-origin .; and git symbolic-ref refs/remotes/n9-origin/HEAD refs/remotes/one/n9-unique"), "real Git config, aliases, tags and remote HEAD");
        for (line, expected) in [
            ("git checkout n9-", "n9-tag"),
            ("git checkout n9-", "n9-local"),
            ("git checkout n9-", "n9-origin"),
            ("git n9-checkout n9-", "n9-local"),
            ("git remote remove n9-", "n9-origin"),
            ("git config n9.", "n9.context"),
            ("git log local..n9-", "local..n9-tag"),
            ("git show HE", "HEAD"),
        ] {
            assert!(candidates(&success(invoke("GIT", &git, &format!("complete -C {}", quote(line))), line)).contains(expected));
        }
        let hash = success(invoke("GIT", &git, "git rev-parse --short=10 HEAD"), "actual Git commit identity");
        let hash = hash.trim();
        let line = format!("git branch --contains {}", &hash[..3]);
        assert!(candidates(&success(invoke("GIT", &git, &format!("complete -C {}", quote(&line))), &line)).contains(hash));
        fs::write(git.join("n9-unknown.txt"), b"untracked Git fixture").unwrap();
        fs::write(git.join("n9-staged.txt"), b"staged Git fixture").unwrap();
        fs::write(git.join("n9-tree/n9-leaf-雪.txt"), b"modified Git fixture").unwrap();
        setup(invoke("GIT", &git, "git add n9-staged.txt"), "actual staged Git addition");
        for (line, expected) in [
            ("git add n9-unknown", "n9-unknown.txt"),
            ("git restore --staged n9-staged", "n9-staged.txt"),
            ("git restore n9-tree/n9-", "n9-tree/n9-leaf-雪.txt"),
        ] {
            assert!(candidates(&success(invoke("GIT", &git, &format!("complete -C {}", quote(line))), line)).contains(expected));
        }
        setup(invoke("GIT", &git, "git stash push --include-untracked -m n9-stash >/dev/null"), "actual Git stash");
        assert!(candidates(&success(invoke("GIT", &git, "complete -C 'git stash show stash@'"), "actual Git stash completion")).contains("stash@{0}"));
        setup(invoke("GIT", &git, "git stash drop >/dev/null"), "drop actual Git stash");
        assert!(!candidates(&success(invoke("GIT", &git, "complete -C 'git stash show stash@'"), "Git stash removed")).contains("stash@{0}"));
        let linked = f.root.join("Git linked worktree 雪");
        setup(invoke("GIT", &git, &format!("git worktree add --detach {} HEAD", quote(linked.to_str().unwrap()))), "actual linked Git worktree");
        assert!(candidates(&success(invoke("GIT", &git, "complete -C 'git worktree remove '"), "Git linked worktree completion")).iter().any(|candidate| candidate.contains("Git linked worktree")));
        setup(invoke("GIT", &git, &format!("git worktree remove {}", quote(linked.to_str().unwrap()))), "remove actual linked Git worktree");
        assert!(!candidates(&success(invoke("GIT", &git, "complete -C 'git worktree remove '"), "Git linked worktree removed")).iter().any(|candidate| candidate.contains("Git linked worktree")));
        let submodule = f.root.join("Git local submodule source 雪");
        fs::create_dir(&submodule).unwrap();
        setup(invoke("GIT", &submodule, "git init -q -b local; and git -c user.name=N9 -c user.email=n9@example.invalid commit -q --allow-empty -m submodule"), "real local Git submodule source");
        setup(invoke("GIT", &git, &format!("git -c protocol.file.allow=always submodule add {} n9-module", quote(submodule.to_str().unwrap()))), "actual offline Git submodule");
        assert_eq!(candidates(&success(invoke("GIT", &git, "complete -C 'git submodule status n9-'"), "Git submodule candidate")), BTreeSet::from(["n9-module".to_owned()]));
        let rebase = f.root.join("Git real rebase state 雪");
        fs::create_dir(&rebase).unwrap();
        fs::write(rebase.join("n9-conflict.txt"), b"base\n").unwrap();
        setup(invoke("GIT", &rebase, "git init -q -b main; and git config user.name N9; and git config user.email n9@example.invalid; and git add n9-conflict.txt; and git commit -q -m base; and git checkout -q -b topic; and printf 'topic\\n' > n9-conflict.txt; and git commit -qam topic; and git checkout -q main; and printf 'main\\n' > n9-conflict.txt; and git commit -qam main; and git checkout -q topic"), "real Git diverging histories");
        assert!(candidates(&success(invoke("GIT", &rebase, "complete -C 'git rebase --cont'"), "no Git rebase state")).is_empty());
        let interrupted = invoke("GIT", &rebase, "git rebase --merge main");
        assert_eq!(interrupted.code, 1, "expected actual rebase conflict: {}", String::from_utf8_lossy(&interrupted.err));
        assert!(rebase.join(".git/rebase-merge").is_dir());
        assert!(candidates(&success(invoke("GIT", &rebase, "complete -C 'git rebase --cont'"), "actual Git rebase state")).contains("--continue"));
        assert!(candidates(&success(invoke("GIT", &rebase, "complete -C 'git add n9-conflict'"), "actual Git unmerged index entry")).contains("n9-conflict.txt"));
        setup(invoke("GIT", &rebase, "git rebase --abort"), "abort actual Git rebase");
        assert!(candidates(&success(invoke("GIT", &rebase, "complete -C 'git rebase --cont'"), "Git rebase state removed")).is_empty());
    });

    record("hg", &mut || {
        fs::create_dir_all(&outer).unwrap();
        let hg = f.root.join("Hg completion repository 雪");
        fs::create_dir(&hg).unwrap();
        fs::write(hg.join("tracked.txt"), b"native hg context").unwrap();
        setup(invoke("HG", &hg, "hg init; and hg add tracked.txt; and hg commit -u N9 -m initial; and hg bookmark n9-bookmark"), "real Mercurial bookmark");
        for prefix in [
            format!("hg -R {}", quote(hg.to_str().unwrap())),
            format!("hg --repository={}", quote(hg.to_str().unwrap())),
            format!("hg --cwd {}", quote(hg.to_str().unwrap())),
        ] {
            let line = format!("{prefix} bookmarks n9-");
            assert_eq!(candidates(&success(invoke("HG", &outer, &format!("complete -C {}", quote(&line))), &line)), BTreeSet::from(["n9-bookmark".to_owned()]));
        }
        success(invoke("HG", &hg, "hg bookmark --delete n9-bookmark"), "remove actual Mercurial bookmark");
        assert!(candidates(&success(invoke("HG", &hg, "complete -C 'hg bookmarks n9-'"), "Mercurial bookmark deletion")).is_empty());
        fs::write(hg.join(".hg/hgrc"), "[ui]\nusername = N9\n[extensions]\nhgext.mq =\n[paths]\nn9-source = .\n[n9]\ncustom = real repository configuration\n").unwrap();
        fs::write(hg.join("tracked.txt"), b"modified Mercurial fixture").unwrap();
        fs::write(hg.join("n9-unknown.txt"), b"untracked Mercurial fixture").unwrap();
        for (line, expected) in [
            ("hg add n9-", "n9-unknown.txt"),
            ("hg diff tracked", "tracked.txt"),
            ("hg config n9.", "n9.custom"),
            ("hg pull n9-", "n9-source"),
            ("hg help book", "bookmarks"),
            ("hg book", "bookmarks"),
        ] {
            assert!(candidates(&success(invoke("HG", &hg, &format!("complete -C {}", quote(line))), line)).contains(expected));
        }
        setup(invoke("HG", &hg, "hg branch n9-branch >/dev/null; and hg commit -m branch; and hg tag --local n9-tag"), "real Mercurial branch and label");
        assert!(candidates(&success(invoke("HG", &hg, "complete -C 'hg branch n9-'"), "Mercurial named branch")).contains("n9-branch"));
        let labels = candidates(&success(invoke("HG", &hg, "complete -C 'hg update --rev n9-'"), "Mercurial debuglabelcomplete data"));
        assert!(labels.contains("n9-branch") && labels.contains("n9-tag"));
        fs::write(hg.join("tracked.txt"), b"Mercurial shelved change").unwrap();
        setup(invoke("HG", &hg, "hg shelve --name n9-shelf >/dev/null"), "create actual Mercurial shelf");
        assert!(candidates(&success(invoke("HG", &hg, "complete -C 'hg unshelve n9-'"), "Mercurial shelf consumer")).contains("n9-shelf"));
        setup(invoke("HG", &hg, "hg shelve --delete n9-shelf"), "delete actual Mercurial shelf");
        assert!(!candidates(&success(invoke("HG", &hg, "complete -C 'hg unshelve n9-'"), "Mercurial shelf removed")).contains("n9-shelf"));
        fs::write(hg.join("tracked.txt"), b"Mercurial queued patch").unwrap();
        setup(invoke("HG", &hg, "hg qinit; and hg qnew -m patch n9-patch; and hg qpop >/dev/null; and hg qqueue --create n9-queue"), "real Mercurial patch and alternate queue");
        assert!(candidates(&success(invoke("HG", &hg, "complete -C 'hg qqueue n9-'"), "Mercurial patch queue consumer")).contains("n9-queue"));
        setup(invoke("HG", &hg, "hg qqueue patches"), "select actual Mercurial default patch queue");
        assert!(candidates(&success(invoke("HG", &hg, "complete -C 'hg qpush n9-'"), "Mercurial queued patch consumer")).contains("n9-patch"));

        let merge = f.root.join("Hg unresolved completion 雪");
        fs::create_dir(&merge).unwrap();
        fs::write(merge.join("n9-conflict.txt"), b"base\n").unwrap();
        setup(invoke("HG", &merge, "hg init; and hg add n9-conflict.txt; and hg commit -u N9 -m base; and hg branch left >/dev/null; and printf 'left\\n' > n9-conflict.txt; and hg commit -u N9 -m left; and hg update --rev 0 >/dev/null; and hg branch right >/dev/null; and printf 'right\\n' > n9-conflict.txt; and hg commit -u N9 -m right"), "actual conflicting Mercurial heads");
        fs::write(merge.join(".hg/hgrc"), format!("[merge-tools]\nn9-fc.executable = {}\nn9-fc.args = $local $other\n", system_path().join("fc.exe").display())).unwrap();
        let conflict = invoke("HG", &merge, "hg merge --rev left --tool n9-fc");
        assert_eq!(conflict.code, 1, "expected real unresolved merge: {}", String::from_utf8_lossy(&conflict.err));
        assert_eq!(candidates(&success(invoke("HG", &merge, "complete -C 'hg resolve n9-'"), "actual unresolved Mercurial fileset")), BTreeSet::from(["n9-conflict.txt".to_owned()]));
        assert!(candidates(&success(invoke("HG", &merge, "complete -C 'hg resolve --tool n9-'"), "actual configured native merge-tool descriptor")).contains("n9-fc"));
        setup(invoke("HG", &merge, "hg resolve --mark n9-conflict.txt"), "mark actual Mercurial conflict resolved");
        assert!(candidates(&success(invoke("HG", &merge, "complete -C 'hg resolve n9-'"), "resolved Mercurial file excluded")).is_empty());
    });

    record("fossil", &mut || {
        let fossil = f.root.join("Fossil completion repository 雪");
        fs::create_dir(&fossil).unwrap();
        let payload = "native Fossil change 雪 🐟";
        fs::write(fossil.join("n9-tracked-雪.txt"), payload).unwrap();
        setup(invoke("FOSSIL", &fossil, "fossil init --admin-user N9 repository.fossil >/dev/null; and fossil open --user N9 --keep repository.fossil >/dev/null; and fossil user default N9; and fossil add n9-tracked-雪.txt >/dev/null"), "real Fossil changes");
        assert_eq!(fs::read(fossil.join("n9-tracked-雪.txt")).unwrap(), payload.as_bytes(), "Fossil --keep must preserve the existing owned Unicode payload");
        assert_eq!(candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil commit n9-'"), "Fossil changed file without cut")), BTreeSet::from(["n9-tracked-雪.txt".to_owned()]));
        success(invoke("FOSSIL", &fossil, "fossil forget n9-tracked-雪.txt >/dev/null"), "remove actual Fossil pending addition");
        assert!(candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil commit n9-'"), "Fossil pending change removed")).is_empty());
        fs::write(fossil.join("n9-extra.txt"), b"real untracked Fossil file").unwrap();
        assert_eq!(candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil add n9-extra'"), "Fossil extra file provider")), BTreeSet::from(["n9-extra.txt".to_owned()]));
        fs::remove_file(fossil.join("n9-extra.txt")).unwrap();
        assert!(candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil add n9-extra'"), "Fossil extra file deleted")).is_empty());
        setup(invoke("FOSSIL", &fossil, "fossil tag add n9-tag current"), "actual Fossil tag");
        for line in ["fossil checkout n9-", "fossil tag cancel n9-"] {
            assert!(candidates(&success(invoke("FOSSIL", &fossil, &format!("complete -C {}", quote(line))), line)).contains("n9-tag"));
        }
        setup(invoke("FOSSIL", &fossil, "fossil tag cancel n9-tag current"), "cancel actual Fossil tag");
        assert!(!candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil checkout n9-'"), "Fossil tag cancellation")).contains("n9-tag"));
        assert!(candidates(&success(invoke("FOSSIL", &fossil, "complete -C 'fossil all ignore '"), "Fossil owned repository registry")).iter().any(|candidate| candidate.ends_with("repository.fossil")));
    });

    record("jj", &mut || {
        let program = PathBuf::from(std::env::var_os("FISH_N9_JJ").expect("required real Jujutsu"));
        let prefix = "Jujutsu long baseline 雪 🐟 ";
        let padding = 165_usize.saturating_sub(f.root.to_str().unwrap().encode_utf16().count() + 1 + prefix.encode_utf16().count());
        let long = f.root.join(format!("{prefix}{}", "x".repeat(padding)));
        let short = storage.root.join("jj baseline 雪 🐟");
        for directory in [&long, &short] { fs::create_dir(directory).unwrap(); }
        let long_init = direct("completion-jj-direct-long-init", &program, &long, &[OsStr::new("git"), OsStr::new("init")], false);
        assert_ne!(long_init.code, 0, "capability changed: restore long Unicode JJ storage");
        assert!(String::from_utf8_lossy(&long_init.err).contains("os error 3"), "{}", String::from_utf8_lossy(&long_init.err));
        assert_ne!(direct("completion-jj-direct-long-new", &program, &long, &[OsStr::new("new")], false).code, 0);
        assert_eq!(direct("completion-jj-direct-short-init", &program, &short, &[OsStr::new("git"), OsStr::new("init")], false).code, 0);
        assert_eq!(direct("completion-jj-direct-short-new", &program, &short, &[OsStr::new("new")], false).code, 0);
        // Short owned Unicode paths avoid the native 128-hex index/MAX_PATH
        // boundary; this is not evidence of general long-path support.
        let jj = storage.root.join("Jujutsu repository 雪 🐟");
        fs::create_dir(&jj).unwrap();
        fs::write(jj.join("tracked 雪.txt"), "native Jujutsu payload 雪 🐟\n").unwrap();
        setup(invoke("JJ", &jj, "jj git init >/dev/null; and jj config set --repo user.name N9; and jj config set --repo user.email n9@example.invalid; and jj describe -m 'initial 雪 🐟'; and jj bookmark create n9-bookmark-雪"), "real Jujutsu bookmark");
        assert!(candidates(&success(invoke("JJ", &jj, "complete -C 'jj new n9-'"), "Jujutsu generated repository completion")).contains("n9-bookmark-雪"));
        setup(invoke("JJ", &jj, "jj bookmark forget n9-bookmark-雪"), "remove actual Jujutsu bookmark");
        assert!(!candidates(&success(invoke("JJ", &jj, "complete -C 'jj new n9-'"), "Jujutsu bookmark removed")).contains("n9-bookmark-雪"));
        exact(f.eval("complete -C 'jj new n9-'"), 0, b"", b"");
    });

    record("svn", &mut || {
        let program = PathBuf::from(std::env::var_os("FISH_N9_SVN").expect("required real Subversion"));
        let admin = program.parent().unwrap().join("svnadmin.exe");
        let unicode_repository = f.root.join("SVN completion repository 雪");
        let boundary = direct("completion-svn-direct-unicode-repository", &admin, &f.root, &[OsStr::new("create"), unicode_repository.as_os_str()], true);
        assert_ne!(boundary.code, 0, "capability changed: restore Unicode SVN storage");
        assert!(String::from_utf8_lossy(&boundary.err).contains("E721113"));
        let svn = storage.root.join("svn");
        fs::create_dir(&svn).unwrap();
        let repository = svn.join("repository");
        let mut uri = String::from("file:///");
        for byte in repository.to_str().unwrap().replace('\\', "/").bytes() {
            if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) {
                uri.push(byte as char);
            } else {
                uri.push_str(&format!("%{byte:02X}"));
            }
        }
        setup(invoke("SVN", &svn, &format!("svnadmin create repository; and svn checkout {} checkout >/dev/null", quote(&uri))), "real local Subversion checkout");
        let checkout = svn.join("checkout");
        assert_eq!(candidates(&success(invoke("SVN", &checkout, "complete -C 'svn log --revision '"), "Subversion empty checkout revision")), BTreeSet::from(["0".to_owned()]));
        let payload = "native Subversion revision 雪 🐟";
        fs::write(checkout.join("n9-tracked.txt"), payload).unwrap();
        setup(invoke("SVN", &checkout, "svn add n9-tracked.txt >/dev/null; and svn commit --username N9 -m initial >/dev/null; and svn update >/dev/null"), "actual Subversion revision transition");
        assert_eq!(fs::read(checkout.join("n9-tracked.txt")).unwrap(), payload.as_bytes());
        let revisions = candidates(&success(invoke("SVN", &checkout, "complete -C 'svn log --revision '"), "Subversion revision without sed"));
        assert_eq!(revisions, BTreeSet::from(["1".to_owned()]));
        let revision = revisions.iter().next().unwrap();
        assert_eq!(success(invoke("SVN", &checkout, &format!("svn cat --revision {} n9-tracked.txt", quote(revision))), "consume selected Subversion revision"), payload);
        fs::write(checkout.join("n9-tracked.txt"), "second Subversion payload 雪 🐟").unwrap();
        setup(invoke("SVN", &checkout, "svn commit --username N9 -m second >/dev/null; and svn update >/dev/null; and svn update --revision 1 n9-tracked.txt >/dev/null"), "real mixed-revision checkout");
        let ranges = candidates(&success(invoke("SVN", &checkout, "complete -C 'svn log --revision '"), "Subversion mixed-revision range without sed"));
        assert_eq!(ranges, BTreeSet::from(["1:2".to_owned()]));
        let selected = ranges.iter().next().unwrap();
        let log = success(invoke("SVN", &checkout, &format!("svn log --xml --revision {}", quote(selected))), "consume selected Subversion revision range");
        assert!(log.contains("revision=\"1\"") && log.contains("revision=\"2\""), "{log}");
        exact(f.eval(&format!("{}complete -C 'svn log --revision '", table_source(&["svn"]))), 0, b"", b"");
        // __fish_print_svn_rev queries its cwd. Unlike MIME's absolute-target
        // client profile, ASCII storage here cannot retain a Unicode repo cwd.
        assert_eq!(boundary.code, 0, "unmet native SVN Unicode repository/cwd requirement after genuine ASCII revision-transition/Unicode-data coverage: {}", String::from_utf8_lossy(&boundary.err));
    });
    assert!(failures.is_empty(), "required native completion_contexts::repositories subconsumers failed: {failures:?}");
}

fn search(f: &Fixture) {
    let document = f.root.join("n9-search-雪.txt");
    fs::write(&document, b"haystack\r\nneedle\r\nlast line\r\n").unwrap();
    for prefix in ["/a:", "/a:0", "/A:F"] {
        let expected: BTreeSet<_> = "0123456789ABCDEF".chars().map(|digit| format!("{prefix}{digit}")).collect();
        assert_eq!(native_candidates(f, &format!("findstr {prefix}")), expected);
    }
    for complete_color in ["/a:0C", "/a:123", "/a:Q"] {
        assert!(native_candidates(f, &format!("findstr {complete_color}")).is_empty(), "a color is exactly two hexadecimal digits");
    }
    assert!(native_candidates(f, "findstr needle n9-search-").iter().any(|candidate| candidate.contains("n9-search-") && candidate.contains("雪.txt")));
    let argument = document.to_str().unwrap();
    let unicode = native(f, &format!("command findstr /l /n /a:0C needle {}", quote(argument)));
    let direct = direct_native(f, &system_path().join("findstr.exe"), &[OsStr::new("/l"), OsStr::new("/n"), OsStr::new("/a:0C"), OsStr::new("needle"), document.as_os_str()]);
    legacy_unicode_result(f, "findstr", argument, "2:needle\n", unicode, direct);
    fs::write(f.root.join("n9-search.txt"), b"haystack\r\nneedle\r\nlast line\r\n").unwrap();
    assert_eq!(success(native(f, "command findstr /l /n /a:0C needle n9-search.txt"), "real native literal search data in the same Unicode working directory").replace("\r\n", "\n"), "2:needle\n");
    exact(native(f, "command findstr /l missing-pattern n9-search.txt"), 1, b"", b"");
    let absent = candidates(&success(f.eval(&format!("{}complete -C 'findstr /a:0'", table_source(&["findstr"]))), "explicit color table without native findstr"));
    assert_eq!(absent, "0123456789ABCDEF".chars().map(|digit| format!("/a:0{digit}")).collect());
}

fn ssh_descriptors(f: &Fixture) {
    let directory = system_path().join("OpenSSH");
    assert!(directory.join("ssh.exe").is_file() && directory.join("ssh-keygen.exe").is_file(), "required real native OpenSSH tools");
    let invoke = |code: &str| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        super::functions_tools::tool_environment(&mut command, &directory.join("ssh.exe"));
        run(&mut command, b"")
    };
    let configuration = f.state.join("home/.ssh");
    fs::create_dir_all(configuration.join("includes")).unwrap();
    let alternate = configuration.join("alternate config 雪");
    fs::write(&alternate, "Include includes/*.conf\nHost n9-native-config\n  HostName 127.0.0.1\n  User n9_native_user\nHost n9-wildcard-*\n  User wildcard\n").unwrap();
    fs::write(configuration.join("includes/host.conf"), "Host n9-included-config\n  HostName 127.0.0.2\n").unwrap();
    let config = quote(alternate.to_str().unwrap());
    let direct = direct_native(f, &directory.join("ssh.exe"), &[OsStr::new("-G"), OsStr::new("-T"), OsStr::new("-F"), alternate.as_os_str(), OsStr::new("n9-native-config")]);
    let actual = invoke(&format!("command ssh -G -T -F {config} n9-native-config"));
    wide_argument(f, alternate.to_str().unwrap());
    fs::write(f.root.join("ssh-native-query.txt"), format!("direct_exit={}\nfish_exit={}\ndirect_stdout={}\ndirect_stderr={}\nfish_stdout={}\nfish_stderr={}\n", direct.code, actual.code, String::from_utf8_lossy(&direct.out), String::from_utf8_lossy(&direct.err), String::from_utf8_lossy(&actual.out), String::from_utf8_lossy(&actual.err))).unwrap();
    assert_eq!(actual.code, direct.code, "native SSH outcome differs from direct wide launch; see ssh-native-query.txt");
    assert_eq!(direct.code, 0, "direct native SSH fixture cannot evaluate its Unicode configuration; stdout={} stderr={}", String::from_utf8_lossy(&direct.out), String::from_utf8_lossy(&direct.err));
    let actual = success(actual, "actual native SSH config evaluation");
    assert!(actual.lines().any(|line| line == "hostname 127.0.0.1"));
    assert!(actual.lines().any(|line| line == "user n9_native_user"));
    for (prefix, expected) in [("n9-native-", "n9-native-config"), ("n9-included-", "n9-included-config"), ("n9-user@n9-native-", "n9-user@n9-native-config")] {
        let line = format!("ssh -F {config} {prefix}");
        assert!(candidates(&success(invoke(&format!("complete -C {}", quote(&line))), &line)).contains(expected));
    }
    let line = format!("ssh -F {config} n9-wildcard-");
    assert!(!candidates(&success(invoke(&format!("complete -C {}", quote(&line))), "SSH wildcard config is not a literal host")).contains("n9-wildcard-*"));
    let jump_line = format!("ssh -F {config} -J n9-jump@n9-native-");
    let jumps = candidates(&success(invoke(&format!("complete -C {}", quote(&jump_line))), "native SSH ProxyJump identity completion"));
    let jump = jumps.get("n9-jump@n9-native-config").expect("selected real configured jump alias");
    let jump_plan = success(invoke(&format!("command ssh -G -T -F {config} -J {} n9-native-config", quote(jump))), "consume selected ProxyJump identity without connecting");
    assert!(jump_plan.lines().any(|line| line == format!("proxyjump {jump}")), "{jump_plan}");

    let key = configuration.join("owned-ed25519");
    let generated = invoke(&format!("command ssh-keygen -q -t ed25519 -N '' -f {}", quote(key.to_str().unwrap())));
    assert_eq!(generated.code, 0, "actual owned SSH key: {}", String::from_utf8_lossy(&generated.err));
    let public = fs::read_to_string(key.with_extension("pub")).unwrap();
    let public = public.split_ascii_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let key_directory = quote(configuration.to_str().unwrap());
    let identities = candidates(&success(invoke(&format!("cd {key_directory}; and complete -C 'ssh -i owned-ed'")), "native SSH identity file completion"));
    let identity = identities.get("owned-ed25519").expect("selected genuine generated identity");
    let derived = success(invoke(&format!("cd {key_directory}; and command ssh-keygen -y -f {}", quote(identity))), "consume selected private identity cryptographically");
    assert_eq!(derived.split_ascii_whitespace().take(2).collect::<Vec<_>>().join(" "), public);
    let identity_plan = success(invoke(&format!("cd {key_directory}; and command ssh -G -T -F {config} -i {} n9-native-config", quote(identity))), "consume selected native SSH identity option");
    assert!(identity_plan.lines().any(|line| line == format!("identityfile {identity}")), "{identity_plan}");
    let hosts = configuration.join("known_hosts");
    fs::write(&hosts, format!("n9-known.example,n9-alias.example {public}\n[n9-port.example]:2222 {public}\n")).unwrap();
    for (prefix, expected) in [("n9-user@n9-known", "n9-user@n9-known.example"), ("n9-user@n9-alias", "n9-user@n9-alias.example"), ("n9-port", "n9-port.example")] {
        let line = format!("ssh {prefix}");
        assert!(candidates(&success(invoke(&format!("complete -C {}", quote(&line))), &line)).contains(expected));
    }
    let hashed = invoke(&format!("command ssh-keygen -H -f {}", quote(hosts.to_str().unwrap())));
    assert_eq!(hashed.code, 0, "actual known-host hashing: {}", String::from_utf8_lossy(&hashed.err));
    assert!(!candidates(&success(invoke("complete -C 'ssh n9-user@n9-known'"), "hashed host is not advertised as plaintext")).contains("n9-user@n9-known.example"));
    // Native -N starts private; opt into persistence only inside this owned history namespace.
    let history = "set -e fish_private_mode; set -g fish_history n9_ssh_context; ";
    exact(f.eval(&format!("{history}builtin history append 'ssh n9-history@n9-history.example' 'scp n9-scp@n9-not-ssh.example'; builtin history save")), 0, b"", b"");
    let remembered = success(invoke(&format!("{history}complete -C 'ssh n9-history@'")), "actual isolated SSH command history");
    assert!(candidates(&remembered).contains("n9-history@n9-history.example"));
    let remembered = candidates(&remembered);
    let selected = remembered.get("n9-history@n9-history.example").unwrap();
    let history_plan = success(invoke(&format!("command ssh -G -T -F {config} {}", quote(selected))), "consume selected SSH history identity without connecting");
    assert!(history_plan.lines().any(|line| line == "hostname n9-history.example"), "{history_plan}");
    assert!(history_plan.lines().any(|line| line == "user n9-history"), "{history_plan}");
    let switched = success(invoke("set -e fish_private_mode; set -g fish_history n9_ssh_empty; complete -C 'ssh n9-history@'"), "SSH history namespace isolation");
    assert!(!candidates(&switched).contains("n9-history@n9-history.example"));
    let deleted = success(invoke(&format!("{history}builtin history delete --exact --case-sensitive 'ssh n9-history@n9-history.example'; complete -C 'ssh n9-history@'")), "SSH history deletion");
    assert!(!candidates(&deleted).contains("n9-history@n9-history.example"));
}

fn descriptors(f: &Fixture) {
    let source = table_source(&["firefox", "rst2html", "systemd-cryptenroll", "mount", "df"]);
    let eval = |code: &str| f.eval(&format!("{source}{code}"));
    let profiles = f.state.join("config/Mozilla/Firefox/profiles.ini");
    fs::create_dir_all(profiles.parent().unwrap()).unwrap();
    fs::write(&profiles, "[Profile0]\r\nName=n9-native-雪\r\nIsRelative=1\r\nPath=Profiles/owned\r\n[Profile1]\r\nName=n9-second\r\n").unwrap();
    let decoy = f.state.join("home/.mozilla/firefox/profiles.ini");
    fs::create_dir_all(decoy.parent().unwrap()).unwrap();
    fs::write(&decoy, "[Profile0]\nName=n9-wrong-platform\n").unwrap();
    assert_eq!(candidates(&success(eval("complete -C 'firefox -P n9-'"), "owned Windows Firefox descriptor through explicitly loaded metadata table")), BTreeSet::from(["n9-native-雪".to_owned(), "n9-second".to_owned()]));
    assert!(candidates(&success(eval("complete -C 'firefox --display'"), "native Firefox metadata has no X11 display option")).is_empty());
    fs::write(&profiles, "[Profile0]\r\nName=n9-replaced\r\n").unwrap();
    assert_eq!(candidates(&success(eval("complete -C 'firefox -P n9-'"), "Firefox profile descriptor changed")), BTreeSet::from(["n9-replaced".to_owned()]));
    fs::remove_file(&profiles).unwrap();
    exact(eval("complete -C 'firefox -P n9-'"), 0, b"", b"");
    assert!(candidates(&success(eval("complete -C 'firefox --head'"), "absent profile does not remove ordinary browser metadata")).contains("--headless"));

    for file in ["n9-document.rst", "n9-document.txt", "n9-document.other"] {
        fs::write(f.root.join(file), "Native document title\n=====================\n\nReal reStructuredText input.\n").unwrap();
    }
    let documents = success(eval("complete -C 'rst2html n9-document'"), "owned document suffix prioritization through explicit metadata table");
    let documents: Vec<_> = documents.lines().map(|line| line.split('\t').next().unwrap()).collect();
    let other = documents.iter().position(|file| *file == "n9-document.other").unwrap();
    assert!(documents.iter().position(|file| *file == "n9-document.rst").unwrap() < other);
    assert!(documents.iter().position(|file| *file == "n9-document.txt").unwrap() < other);
    assert_eq!(candidates(&success(eval("complete -C 'rst2html --input-encoding-error-handler '"), "document error-policy argument boundary")), BTreeSet::from(["strict".to_owned(), "ignore".to_owned(), "replace".to_owned()]));
    fs::remove_file(f.root.join("n9-document.rst")).unwrap();
    assert!(!candidates(&success(eval("complete -C 'rst2html n9-document'"), "document deletion")).contains("n9-document.rst"));

    fs::create_dir(f.root.join("n9-device-directory")).unwrap();
    fs::write(f.root.join("n9-device-regular-file"), b"not a POSIX block device").unwrap();
    assert_eq!(candidates(&success(eval("complete -C 'systemd-cryptenroll n9-device-'"), "portable directory versus block-device metadata")), BTreeSet::from(["n9-device-directory/".to_owned()]));
    fs::remove_dir(f.root.join("n9-device-directory")).unwrap();
    exact(eval("complete -C 'systemd-cryptenroll n9-device-'"), 0, b"", b"");
    assert!(candidates(&success(eval("complete -C 'mount -o ro,nosu'"), "mount comma option prefix context")).contains("ro,nosuid"));
    exact(eval("complete -C 'mount -o ro,uid=S-1'"), 0, b"", b"");
    assert!(candidates(&success(eval("complete -C 'df -t nt'"), "portable filesystem-type parameter")).contains("ntfs"));
    assert!(candidates(&success(eval("complete -C 'df --ty'"), "non-GNU metadata must not advertise GNU long options")).is_empty());
}

fn provider_contexts(f: &Fixture) {
    let source = table_source(&["pgrep", "lpr", "zfs", "btdownloadheadless.py"]);
    // ZFS is explicitly loaded metadata here, not an installed OpenZFS integration.
    let eval = |code: &str| f.eval(&format!("{source}{code}"));
    let parent = std::process::id().to_string();
    let processes = candidates(&success(eval(&format!("complete -C {}", quote(&format!("pgrep -P {}", &parent[..parent.len().min(2)])))), "live parent-process operand through explicit portable metadata"));
    assert!(processes.contains(&parent), "{processes:?}");
    assert!(candidates(&success(eval("complete -C 'pgrep fish'"), "actual live fish process image")).iter().any(|process| process.eq_ignore_ascii_case("fish.exe")));
    let users = candidates(&success(eval("__fish_print_users"), "actual native account names"));
    assert!(!users.is_empty());
    assert_eq!(candidates(&success(eval("complete -C 'lpr -U '"), "native account data in portable printer username context")), users);
    assert_eq!(candidates(&success(eval("complete -C 'pgrep -U '"), "native account data in real-user selector context")), users);
    exact(eval("complete -C 'lpr -P n9-absent-printer'"), 0, b"", b"");
    let user_properties = candidates(&success(eval("complete -C 'zfs get userused@'"), "native account data in explicitly loaded ZFS property metadata"));
    assert_eq!(user_properties, users.iter().map(|user| format!("userused@{user}")).collect());
    let groups = candidates(&success(eval("__fish_print_groups"), "actual native group names"));
    assert_eq!(candidates(&success(eval("complete -C 'pgrep -G '"), "native group data in group selector context")), groups);
    assert_eq!(candidates(&success(eval("complete -C 'zfs get groupused@'"), "native group data in ZFS property context")), groups.iter().map(|group| format!("groupused@{group}")).collect());
    assert_eq!(candidates(&success(eval("complete -C 'zfs get -o '"), "ZFS static field metadata survives absent dataset producer")), ["name", "property", "value", "source"].into_iter().map(str::to_owned).collect());
    for line in ["zfs get n9-absent-dataset", "zfs destroy n9-absent-dataset", "zfs allow -c n9-absent-permission"] {
        exact(eval(&format!("complete -C {}", quote(line))), 0, b"", b"");
    }
    let addresses = candidates(&success(eval("__fish_print_addresses"), "actual native interface addresses"));
    assert_eq!(candidates(&success(eval("complete -C 'btdownloadheadless.py --ip '"), "actual advertised interface address candidates")), addresses);
    let mut bound = addresses;
    bound.extend(["0.0.0.0".to_owned(), "::".to_owned()]);
    assert_eq!(candidates(&success(eval("complete -C 'btdownloadheadless.py --bind '"), "wildcard versus advertised address context")), bound);
    assert_eq!(candidates(&success(eval("complete -C 'zfs mount -o read'"), "ZFS property-name versus assignment-value state")), BTreeSet::from(["readonly".to_owned()]));
    assert!(candidates(&success(eval("complete -C 'zfs mount -o readonly='"), "portable ZFS mount option values survive absent zfs")).contains("readonly=on"));
    assert!(candidates(&success(eval("complete -C 'zfs create -o normaliz'"), "portable create-only property-name state")).contains("normalization"));
}
