use super::{Family, fixture::{Fixture, exact, quote, run}};
use std::{collections::BTreeSet, ffi::OsStr, fs, path::{Path, PathBuf}, process::Command, time::{Duration, SystemTime}};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

pub(super) const FAMILIES: &[Family] = &[
    ("utility-free-startup-private-and-no-config", startup),
    ("all-sample-prompts-without-vcs-or-coreutils", prompts),
    ("navigation-stack-and-directory-history-unicode", navigation),
    ("theme-selection-freeze-backup-and-reload", themes),
    ("private-embedded-extraction-concurrent-and-cleanup", extraction),
    ("funced-literal-editor-argv-save-cancel-and-cleanup", editor),
    ("generated-completion-cache-identity-and-failure", generated_cache),
    ("native-command-output-cache-and-failure", output_cache),
    ("native-account-process-network-and-drive-consumers", providers),
    ("all-vcs-prompts-real-repository-and-absent", vcs),
    ("shipped-ledger-and-static-completion-consumers", inventory),
    ("isolated-public-clipboard-unicode-newlines-and-editor", clipboard),
    ("reserved-native-facades-no-external-fallback", reserved),
];
fn text(path: &Path) -> String { quote(path.to_str().unwrap()) }
fn success(f: &Fixture, code: &str) -> String {
    let result = f.eval(code);
    assert_eq!(result.code, 0, "{code}\n{}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{code}\n{}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn startup(f: &Fixture) {
    let result = run(&mut f.command(&[OsStr::new("-c"), OsStr::new("functions -q fish_prompt fish_default_key_bindings; and printf 'startup\\n'")]), b"");
    exact(result, 0, b"startup\n", b"");
    exact(f.eval("not __fish_is_cygwin"), 0, b"", b"");
    let user_config = f.state.join("config/fish/config.fish");
    fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    fs::write(&user_config, "set -g n9_user_config loaded\n").unwrap();
    for (flags, expected) in [(vec!["-c"], "loaded\n"), (vec!["-P", "-c"], "loaded\n"), (vec!["-N", "-c"], "absent\n")] {
        let mut args: Vec<&OsStr> = flags.iter().map(OsStr::new).collect();
        args.push(OsStr::new("set -q n9_user_config; and echo $n9_user_config; or echo absent"));
        exact(run(&mut f.command(&args), b""), 0, expected.as_bytes(), b"");
    }
    assert!(!f.state.join("cache/fish/generated_completions").exists(), "startup must not require Python or pretend native manual discovery");
    exact(f.eval("setenv PATH 'C:\\native one;D:\\native two'; printf '%s\\n' $PATH"), 0, b"C:\\native one\nD:\\native two\n", b"");
}
fn prompts(f: &Fixture) {
    let names = success(f, "status list-files prompts");
    let directory = f.root.join("n9-雪");
    fs::create_dir(&directory).unwrap();
    for name in names.lines().filter(|name| name.ends_with(".fish")) {
        let code = format!("cd {}; status get-file {} | source; false | true; fish_prompt; if functions -q fish_right_prompt; fish_right_prompt; end", text(&directory), quote(name));
        assert!(success(f, &code).contains("n9-雪"), "prompt must show the actual Unicode working directory: {name}");
    }
    let mut command = Command::new(&f.probe);
    command.arg("n9-prompt-duration").arg(&f.root).arg(&f.fish).arg(&f.probe).env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(key, value); }
    }
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "actual first-prompt/duration workflow:\n{}\n{}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    for name in ["fish_git_prompt", "fish_hg_prompt", "fish_jj_prompt", "fish_darcs_prompt", "fish_svn_prompt", "fish_fossil_prompt"] {
        exact(f.eval(&format!("{name}")), 1, b"", b"");
    }
}
fn navigation(f: &Fixture) {
    for directory in ["directory 雪", "directory 🐟"] { fs::create_dir(f.root.join(directory)).unwrap(); }
    let output = success(f, "cd 'directory 雪'; pushd '../directory 🐟' >/dev/null; printf '%s\\n' (path basename $PWD); popd >/dev/null; printf '%s\\n' (path basename $PWD); cd '../directory 🐟'; prevd >/dev/null; printf '%s\\n' (path basename $PWD); nextd >/dev/null; printf '%s\\n' (path basename $PWD); dirs -c");
    assert_eq!(output, "directory 🐟\ndirectory 雪\ndirectory 雪\ndirectory 🐟\n");
    let result = f.input("cd 'directory 雪'; cd '../directory 🐟'; cdh; printf '\\nSELECTED:%s\\n' (path basename $PWD)", b"a\n");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(String::from_utf8_lossy(&result.out).ends_with("SELECTED:directory 雪\n"));
    let history = success(f, "cd 'directory 雪'; cd '../directory 🐟'; dirh");
    assert!(history.contains("directory 雪") && history.contains("directory 🐟"));
    let project = f.root.join("npm parent project 雪");
    fs::create_dir_all(project.join("nested/child 🐟")).unwrap();
    let manifest = project.join("package.json");
    fs::write(&manifest, "{\"name\":\"n9-parent-consumer\",\"version\":\"1.0.0\"}").unwrap();
    let output = success(f, &format!("cd {}; __fish_npm_helper; __npm_find_package_json", text(&project.join("nested/child 🐟"))));
    let discovered = PathBuf::from(output.trim());
    assert_eq!(fs::canonicalize(discovered).unwrap(), fs::canonicalize(manifest).unwrap(), "actual npm parent discovery must preserve drive and Unicode identity");
    npm_consumers(f);
}
fn npm_consumers(f: &Fixture) {
    struct OwnedJunctions([(PathBuf, PathBuf); 2]);
    impl Drop for OwnedJunctions {
        fn drop(&mut self) {
            for (link, target) in &self.0 {
                // Remove only declared links while they still identify their
                // owned package sources. Never recursively traverse a junction.
                if fs::symlink_metadata(link).is_ok_and(|metadata| metadata.file_attributes() & 0x400 != 0)
                    && matches!((fs::canonicalize(link), fs::canonicalize(target)), (Ok(actual), Ok(expected)) if actual == expected)
                {
                    let _ = fs::remove_dir(link);
                }
            }
        }
    }
    let required = |name: &str| {
        let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("required exact native Node package-manager fixture {name}")));
        assert!(path.is_absolute() && path.is_file(), "{name}: {}", path.display());
        path
    };
    let node = required("FISH_N9_NODE");
    let npm = required("FISH_N9_NPM");
    let yarn = required("FISH_N9_YARN");
    let search = std::env::join_paths([node.parent().unwrap(), npm.parent().unwrap(), yarn.parent().unwrap()]).unwrap();
    let prefix = f.root.join("npm global prefix 雪 🐟");
    let user_config = f.root.join("owned npm userconfig");
    fs::write(&user_config, "").unwrap();
    let invoke = |code: &str| {
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
        command.env("PATH", &search)
            .env("NPM_CONFIG_PREFIX", &prefix)
            .env("NPM_CONFIG_USERCONFIG", &user_config)
            .env("NPM_CONFIG_CACHE", f.state.join("cache/npm"));
        run(&mut command, b"")
    };
    exact(invoke(&format!("{} --version >/dev/null; and {} --version >/dev/null; and {} --version >/dev/null", text(&node), text(&npm), text(&yarn))), 0, b"", b"");
    let source = f.root.join("npm offline package sources 雪");
    for (directory, name) in [("scoped", "@n9/scoped-package"), ("names", "all-the-package-names")] {
        let package = source.join(directory);
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("package.json"), format!(r#"{{"name":"{name}","version":"1.0.0"}}"#)).unwrap();
    }
    fs::write(source.join("names/names.json"), r#"["n9-alpha","n9-\u96ea","other-package"]"#).unwrap();
    let owned_links = OwnedJunctions([
        (prefix.join("node_modules/@n9/scoped-package"), source.join("scoped")),
        (prefix.join("node_modules/all-the-package-names"), source.join("names")),
    ]);
    let install = invoke(&format!("{} install --global --offline --ignore-scripts --no-package-lock --no-audit --no-fund {} {}", text(&npm), text(&source.join("scoped")), text(&source.join("names"))));
    assert_eq!(install.code, 0, "real offline global npm fixture: {}", String::from_utf8_lossy(&install.err));
    let global = invoke("__fish_npm_helper; __npm_installed_global_packages");
    assert_eq!(global.code, 0, "{}", String::from_utf8_lossy(&global.err));
    assert!(global.err.is_empty(), "{}", String::from_utf8_lossy(&global.err));
    let packages: BTreeSet<_> = String::from_utf8(global.out).unwrap().lines().map(str::to_owned).collect();
    assert_eq!(packages, BTreeSet::from(["@n9/scoped-package".to_owned(), "all-the-package-names".to_owned()]));

    let project = f.root.join("npm JSON project 雪 🐟");
    fs::create_dir_all(project.join("nested/child")).unwrap();
    let manifest = project.join("package.json");
    // Compact objects and escaped values cannot be parsed by a line-oriented fallback.
    fs::write(&manifest, r#"{"name":"n9-json","version":"1.0.0","description":"dependencies are not fields in this string","scripts":{"n9-build":"node \"雪 🐟.js\"","n9-lines":"echo a\nb\tc"},"dependencies":{"@n9/scoped-package":"file:../snow path","n9-\u96ea":"1.0.0"},"devDependencies":{"n9-dev":"^2"}}"#).unwrap();
    let dependencies = invoke(&format!("cd {}; __fish_npm_helper; __npm_installed_local_packages", text(&project.join("nested/child"))));
    exact(dependencies, 0, "@n9/scoped-package\nn9-雪\nn9-dev\n".as_bytes(), b"");
    for (line, expected) in [
        ("npm uninstall n9-", vec!["n9-dev", "n9-雪"]),
        ("yarn remove n9-", vec!["n9-dev", "n9-雪"]),
        ("npm run n9-", vec!["n9-build", "n9-lines"]),
        ("yarn run n9-", vec!["n9-build", "n9-lines"]),
        ("npm install n9-", vec!["n9-alpha", "n9-雪"]),
        ("yarn add n9-", vec!["n9-alpha", "n9-雪"]),
    ] {
        let result = invoke(&format!("cd {}; complete -C {}", text(&project), quote(line)));
        assert_eq!(result.code, 0, "{line}: {}", String::from_utf8_lossy(&result.err));
        assert!(result.err.is_empty(), "{line}: {}", String::from_utf8_lossy(&result.err));
        let output = String::from_utf8(result.out).unwrap();
        let actual: BTreeSet<_> = output.lines().map(|entry| entry.split('\t').next().unwrap()).collect();
        assert_eq!(actual, expected.into_iter().collect(), "{line}: {output}");
    }
    exact(invoke(&format!("cd {}; complete -C 'npm run n9-' | string match '*node*'; complete -C 'yarn run n9-' | string match '*echo*'", text(&project))), 0, "n9-build\tnode \"雪 🐟.js\"\nn9-lines\techo a b c\n".as_bytes(), b"");
    fs::write(&manifest, r#"{"scripts":{},"dependencies":null,"devDependencies":{}}"#).unwrap();
    exact(invoke(&format!("cd {}; __fish_npm_helper; __npm_installed_local_packages; __npm_package_scripts", text(&project))), 0, b"", b"");
    fs::write(&manifest, "{invalid JSON").unwrap();
    exact(invoke(&format!("cd {}; __fish_npm_helper; __npm_installed_local_packages", text(&project))), 1, b"", b"");
    fs::write(&manifest, r#"{"scripts":{"n9-build":"node build.js"},"dependencies":{"n9-local":"1.0.0"}}"#).unwrap();
    for line in ["npm install n9-", "yarn add n9-"] {
        let result = f.eval(&format!("cd {}; complete -C {}; not set -q __fish_npm_pkg_info_shown", text(&project), quote(line)));
        exact(result, 0, b"", b"");
    }
    exact(f.eval("__fish_npm_helper; __npm_installed_global_packages"), 1, b"", b"");
    exact(f.eval(&format!("cd {}; __fish_npm_helper; __npm_installed_local_packages", text(&project))), 1, b"", b"");
    exact(f.eval(&format!("cd {}; __fish_npm_helper; __npm_package_scripts", text(&project))), 1, b"", b"");
    for (link, target) in &owned_links.0 {
        if fs::symlink_metadata(link).unwrap().file_attributes() & 0x400 != 0 {
            assert_eq!(fs::canonicalize(link).unwrap(), fs::canonicalize(target).unwrap(), "native npm may create only the declared owned package junction");
            fs::remove_dir(link).unwrap();
            assert!(fs::symlink_metadata(link).is_err(), "owned npm junction survived cleanup: {}", link.display());
        }
    }
}
fn themes(f: &Fixture) {
    let names = success(f, "fish_config theme list");
    assert!(names.contains("default"));
    for name in success(f, "status list-files themes").lines().filter_map(|line| line.strip_prefix("themes/").and_then(|line| line.strip_suffix(".theme"))) {
        success(f, &format!("fish_config theme choose {}; set -q fish_color_command", quote(name)));
    }
    success(f, "__fish_theme_freeze n9 'fish_color_command brmagenta' 'fish_color_quote brcyan'");
    success(f, "__fish_theme_freeze n9 'fish_color_command bryellow'");
    let frozen = f.state.join("config/fish/conf.d/fish_frozen_theme.fish");
    assert!(fs::read_to_string(&frozen).unwrap().contains("bryellow"));
    assert!(fs::read_to_string(frozen.with_file_name("fish_frozen_theme.fish.bak")).unwrap().contains("brmagenta"));
    let result = run(&mut f.command(&[OsStr::new("-c"), OsStr::new("printf '%s\\n' $fish_color_command")]), b"");
    exact(result, 0, b"bryellow\n", b"");
}
fn extraction(f: &Fixture) {
    let commands: Vec<_> = (0..2).map(|index| {
        let record = f.root.join(format!("extracted-{index}"));
        let code = format!("__fish_data_with_directory tools/web_config '.*' {} n9-extracted-file {}", f.probe(), text(&record));
        let command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        (record, command)
    }).collect();
    let workers: Vec<_> = commands.into_iter().map(|(record, mut command)| std::thread::spawn(move || (record, run(&mut command, b"")))).collect();
    let mut extracted = Vec::new();
    for worker in workers {
        let (record, result) = worker.join().unwrap();
        exact(result, 0, "extracted 雪\n".as_bytes(), b"");
        let path = PathBuf::from(fs::read_to_string(record).unwrap());
        assert!(!path.exists(), "extraction lifetime leaked {}", path.display());
        extracted.push(path);
    }
    assert_ne!(extracted[0], extracted[1]);
    let result = f.eval("__fish_data_with_directory tools/web_config '.*' false");
    assert_eq!(result.code, 1);
    interrupted(f, "extraction");
}
fn editor(f: &Fixture) {
    let replacement = f.root.join("editor contents 雪.fish");
    let record = f.root.join("editor argv 🐟");
    fs::write(&replacement, "function n9_edited; printf 'edited 雪\\n'; end\n").unwrap();
    let editor = format!("{} n9-editor {} {}", f.probe(), text(&record), text(&replacement));
    let output = success(f, &format!("funced --save --editor={} n9_edited; n9_edited", quote(&editor)));
    assert!(output.ends_with("edited 雪\n"));
    assert!(f.state.join("config/fish/functions/n9_edited.fish").is_file());
    let temporary = PathBuf::from(fs::read_to_string(&record).unwrap());
    assert!(!temporary.parent().unwrap().exists());
    let editor = format!("{} n9-editor-cancel {}", f.probe(), text(&record));
    let result = f.eval(&format!("funced --editor={} n9_cancelled", quote(&editor)));
    assert_ne!(result.code, 0);
    let temporary = PathBuf::from(fs::read_to_string(&record).unwrap());
    assert!(!temporary.parent().unwrap().exists());
    assert!(!f.state.join("config/fish/functions/n9_cancelled.fish").exists());
    let saved = f.state.join("config/fish/functions/n9_edited.fish");
    let original = fs::read(&saved).unwrap();
    let before: BTreeSet<_> = fs::read_dir(saved.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    let denied = fs::OpenOptions::new().read(true).share_mode(1).open(&saved).unwrap();
    let result = f.eval("function n9_edited; echo denied; end; funcsave n9_edited");
    assert_ne!(result.code, 0, "save must report the native sharing denial");
    assert!(!result.err.is_empty());
    assert_eq!(fs::read(&saved).unwrap(), original);
    drop(denied);
    let after: BTreeSet<_> = fs::read_dir(saved.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(before, after, "failed atomic save left a staging file");
    // Saved functions and delta discovery use the configured autoload roots.
    // A -N session intentionally skips the config.fish that initializes them.
    exact(run(&mut f.command(&[OsStr::new("-c"), OsStr::new("n9_edited")]), b""), 0, "edited 雪\n".as_bytes(), b"");
    let result = run(&mut f.command(&[OsStr::new("-c"), OsStr::new("fish_delta --no-diff --no-completions --no-config --new n9_edited")]), b"");
    assert_eq!(result.code, 1, "named delta must report an actual saved function; stdout={:?} stderr={:?}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
    let output = String::from_utf8(result.out).unwrap();
    let reported: Vec<_> = output.lines().map(|line| line.split_once(": ").expect("delta record must identify its saved file").1).collect();
    assert_eq!(reported.len(), 1, "named delta consumer must report only the selected saved function: {output}");
    assert_eq!(fs::canonicalize(reported[0]).unwrap(), fs::canonicalize(&saved).unwrap(), "delta must report the actual saved function identity");
    interrupted(f, "editor");
    interrupted(f, "save");
}
fn generated_cache(f: &Fixture) {
    let generator = f.root.join("generator 雪.exe");
    fs::copy(&f.probe, &generator).unwrap();
    let counter = f.root.join("generation-count");
    let invoke = format!("__fish_cache_sourced_completions --name=n9-generator {} n9-generate-completions {} snow; complete -C 'n9-generated --'", text(&generator), text(&counter));
    let cached = f.state.join("cache/fish/completions/n9-generator.fish");
    let stamp = cached.with_extension("stamp");
    let snapshot = || (fs::read(&cached).unwrap(), fs::read(&stamp).unwrap(), fs::metadata(&cached).unwrap().modified().unwrap(), fs::metadata(&stamp).unwrap().modified().unwrap());
    let has_snow = |output: &str| output.lines().any(|line| line.split('\t').next() == Some("--snow"));
    assert!(has_snow(&success(f, &invoke)));
    let first = snapshot();
    assert!(has_snow(&success(f, &invoke)));
    assert_eq!(fs::read_to_string(&counter).unwrap(), "1");
    assert_eq!(snapshot(), first, "an identity hit must preserve cache/stamp bytes and modification times");
    fs::File::options().write(true).open(&generator).unwrap().set_modified(SystemTime::now() + Duration::from_secs(3)).unwrap();
    assert!(has_snow(&success(f, &invoke)));
    assert_eq!(fs::read_to_string(&counter).unwrap(), "2");
    let regenerated = snapshot();
    assert_ne!(regenerated.1, first.1, "an actual generator mtime change must refresh the cached identity stamp");
    fs::File::options().write(true).open(&generator).unwrap().set_modified(SystemTime::now() + Duration::from_secs(6)).unwrap();
    let result = f.eval(&format!("__fish_cache_sourced_completions --name=n9-generator {} n9-generate-failure", text(&generator)));
    assert_eq!(result.code, 2);
    assert!(String::from_utf8_lossy(&result.err).contains("generator"));
    assert_eq!(snapshot(), regenerated, "a failing generator must preserve the exact previously published cache/stamp bytes and modification times");
    interrupted(f, "cache");
}
fn output_cache(f: &Fixture) {
    exact(f.eval(r#"__fish_cached -t 3600 -k n9-output -- 'printf "cached 雪\n"'"#), 0, "cached 雪\n".as_bytes(), b"");
    exact(f.eval("__fish_cached -t 3600 -k n9-output -- false"), 0, "cached 雪\n".as_bytes(), b"");
    assert_ne!(f.eval("__fish_cached -k n9-failure -- false").code, 0);
    assert!(!f.state.join("cache/fish/n9-failure").exists());
}
fn providers(f: &Fixture) {
    for (consumer, provider) in [
        ("__fish_complete_users", "__fish_native_services users"),
        ("__fish_complete_groups", "__fish_native_services groups"),
        ("__fish_print_interfaces", "__fish_native_services interfaces"),
        ("__fish_print_addresses", "__fish_native_services addresses | string replace -r '^([^\\t]+\\t[^\\t]+)\\t.*' '$1'"),
        ("__fish_print_hostnames", "__fish_native_services hostname; __fish_native_services hosts"),
        ("__fish_print_windows_users", "__fish_native_services users | string replace -r '^([^\\t]*)\\t.*' '$1'"),
        ("__fish_print_windows_drives", "__fish_native_services drives"),
    ] {
        assert_eq!(success(f, consumer), success(f, provider), "{consumer}: facade must preserve the real native provider records");
    }
    let parent_pid = std::process::id().to_string();
    for consumer in ["__fish_complete_pids", "__fish_ps"] {
        let output = success(f, consumer);
        assert!(output.lines().any(|line| line.split('\t').next() == Some(parent_pid.as_str())), "{consumer}: missing live owning test process {parent_pid}: {output}");
    }
    let image = f.fish.file_name().unwrap().to_str().unwrap();
    assert!(success(f, "__fish_complete_proc").lines().any(|line| line.eq_ignore_ascii_case(image)), "process-name consumer must include its actual running fish image");
    for code in ["__fish_complete_user_ids", "__fish_complete_group_ids"] {
        let result = f.eval(code);
        assert_ne!(result.code, 0);
        assert!(String::from_utf8_lossy(&result.err).contains("SID"));
    }
    let system = PathBuf::from(std::env::var_os("SystemRoot").expect("required native Windows system root")).join("System32");
    let search = std::env::join_paths([system.clone(), system.join("WindowsPowerShell/v1.0")]).unwrap();
    for (context, expected) in [("cmd /", "/c"), ("reg q", "query"), ("cmdkey /", "/add:"), ("setx /", "/s"), ("cleanmgr /", "/d"), ("schtasks /", "/query"), ("powershell -", "-NoProfile")] {
        let code = format!("complete -C {}", quote(context));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", &search);
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0, "{context}: {}", String::from_utf8_lossy(&result.err));
        assert!(result.err.is_empty(), "{context}: {}", String::from_utf8_lossy(&result.err));
        let output = String::from_utf8(result.out).unwrap();
        assert!(output.lines().any(|line| line.split('\t').next() == Some(expected)), "{context}: missing native completion {expected}: {output}");
    }
    let result = f.eval("open 'Z:\\fish-n9-does-not-exist\\missing.txt'");
    assert_ne!(result.code, 0);
    assert!(!result.err.is_empty());
    registry(f);
    rustup(f);
}
fn vcs(f: &Fixture) {
    let tools = [
        ("GIT", "git.exe", "git init -b n9-雪; and git -c user.name=N9 -c user.email=n9@example.invalid commit --allow-empty -m initial", "fish_git_prompt", "n9-雪"),
        ("HG", "hg.exe", "hg init; and hg branch n9-雪; and hg add 'tracked 雪.txt'", "fish_hg_prompt", "n9-雪"),
        ("JJ", "jj.exe", "jj git init; and jj config set --repo user.name N9; and jj config set --repo user.email n9@example.invalid; and printf 'left 雪' > conflict; and jj describe -m 'left 雪'; and set -g n9_left (jj log -r @ --no-graph -T change_id); and jj new 'root()'; and printf 'right 🐟' > conflict; and jj describe -m 'right 🐟'; and set -g n9_right (jj log -r @ --no-graph -T change_id); and jj new $n9_left $n9_right; and jj bookmark create n9-雪; and jj config set --repo colors.conflict '#12ab34'", "fish_jj_prompt", "38;2;18;171;52"),
        ("DARCS", "darcs.exe", "darcs initialize; and darcs add 'tracked 雪.txt'", "fish_darcs_prompt", "N9_ADDED"),
        ("FOSSIL", "fossil.exe", "fossil init --admin-user N9 repository.fossil; and fossil open --user N9 --keep repository.fossil; and fossil add 'tracked 雪.txt'", "fish_fossil_prompt", "N9_ADDED"),
        ("SVN", "svn.exe", "", "fish_svn_prompt", "N9_ADDED"),
    ];
    let samples: Vec<_> = success(f, "status list-files prompts").lines().filter(|path| path.ends_with(".fish")).map(str::to_owned).collect();
    let mut failures = Vec::new();
    for (name, executable, setup, prompt, marker) in tools {
        println!("RUN-VCS-PRODUCER {name}");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let variable = format!("FISH_N9_{name}");
        let binary = std::env::var_os(&variable).map(PathBuf::from).unwrap_or_else(|| panic!("required real native VCS fixture {variable}={executable}"));
        assert!(binary.is_file(), "{}", binary.display());
        let storage = matches!(name, "JJ" | "SVN").then(super::functions_tools::OptionalStorage::new);
        let direct = |label: &str, program: &Path, directory: &Path, args: &[&OsStr]| {
            let mut command = Command::new(program);
            command.args(args).current_dir(directory).env_clear();
            for (key, value) in f.command(&[]).get_envs() {
                if let Some(value) = value { command.env(key, value); }
            }
            command.env("PATH", binary.parent().unwrap());
            if name == "SVN" { storage.as_ref().unwrap().environment(&mut command); }
            let result = run(&mut command, b"");
            for (suffix, bytes) in [("stdout", result.out.as_slice()), ("stderr", result.err.as_slice())] {
                fs::write(f.root.join(format!("{label}.{suffix}")), bytes).unwrap();
            }
            fs::write(f.root.join(format!("{label}.status")), result.code.to_string()).unwrap();
            eprintln!("NATIVE-VCS-BASELINE {label} cwd={} argv={args:?} status={} stdout={} stderr={}", directory.display(), result.code, String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
            result
        };
        if name == "JJ" {
            // The 128-hex index basename exceeds this binary's MAX_PATH profile.
            // Prove both native boundaries without changing fish or the machine.
            let prefix = "jj long 雪 🐟 ";
            let padding = 165_usize.saturating_sub(f.root.to_str().unwrap().encode_utf16().count() + 1 + prefix.encode_utf16().count());
            let long = f.root.join(format!("{prefix}{}", "x".repeat(padding)));
            let short = storage.as_ref().unwrap().root.join("baseline 雪 🐟");
            for directory in [&long, &short] { fs::create_dir(directory).unwrap(); }
            let long_init = direct("jj-direct-long-init", &binary, &long, &[OsStr::new("git"), OsStr::new("init")]);
            assert_ne!(long_init.code, 0, "capability changed: restore long Unicode JJ storage");
            assert!(String::from_utf8_lossy(&long_init.err).contains("os error 3"), "{}", String::from_utf8_lossy(&long_init.err));
            assert_ne!(direct("jj-direct-long-new", &binary, &long, &[OsStr::new("new")]).code, 0);
            assert_eq!(direct("jj-direct-short-init", &binary, &short, &[OsStr::new("git"), OsStr::new("init")]).code, 0);
            assert_eq!(direct("jj-direct-short-new", &binary, &short, &[OsStr::new("new")]).code, 0);
        }
        let unicode_repository = f.root.join(format!("repository {name} 雪 🐟"));
        let svn_boundary = if name == "SVN" {
            let admin = binary.parent().unwrap().join("svnadmin.exe");
            let result = direct("svn-direct-unicode-repository", &admin, &f.root, &[OsStr::new("create"), unicode_repository.as_os_str()]);
            assert_ne!(result.code, 0, "capability changed: restore Unicode SVN storage");
            assert!(String::from_utf8_lossy(&result.err).contains("E721113"));
            Some(result)
        } else { None };
        let repository = match name {
            "JJ" => storage.as_ref().unwrap().root.join("repository JJ 雪 🐟"),
            "SVN" => storage.as_ref().unwrap().root.join("svn"),
            _ => unicode_repository,
        };
        fs::create_dir(&repository).unwrap();
        fs::write(repository.join("tracked 雪.txt"), "real native VCS change 雪 🐟\n").unwrap();
        if name == "DARCS" {
            assert_eq!(direct("darcs-direct-unicode-init", &binary, &repository, &[OsStr::new("initialize")]).code, 0);
            let added = direct("darcs-direct-unicode-add", &binary, &repository, &[OsStr::new("add"), OsStr::new("tracked 雪.txt")]);
            assert_eq!(added.code, 0, "unmet native Unicode Darcs lock/add capability: {}", String::from_utf8_lossy(&added.err));
        }
        let setup = if name == "SVN" {
            let native = repository.join("repository").to_str().unwrap().replace('\\', "/");
            let mut uri = String::from("file:///");
            for byte in native.bytes() {
                if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) {
                    uri.push(byte as char);
                } else {
                    uri.push_str(&format!("%{byte:02X}"));
                }
            }
            format!("svnadmin create repository; and svn checkout {} checkout; and cd checkout; and printf 'real native VCS change 雪 🐟' > tracked.txt; and svn add tracked.txt", quote(&uri))
        } else {
            if name == "DARCS" { "true".to_owned() } else { setup.to_owned() }
        };
        let code = format!("cd {}; {setup}", text(&repository));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", binary.parent().unwrap()).env("HGENCODING", "utf-8").env("PYTHONUTF8", "1");
        if name == "SVN" { storage.as_ref().unwrap().environment(&mut command); }
        let setup_result = run(&mut command, b"");
        assert_eq!(setup_result.code, 0, "{name} setup: {}", String::from_utf8_lossy(&setup_result.err));
        if name == "FOSSIL" {
            assert_eq!(fs::read(repository.join("tracked 雪.txt")).unwrap(), "real native VCS change 雪 🐟\n".as_bytes(), "Fossil --keep must preserve the owned Unicode payload");
        }
        let working = if name == "SVN" { repository.join("checkout") } else { repository.clone() };
        let settings = match name {
            "DARCS" => "set -g fish_prompt_darcs_status_added N9_ADDED",
            "FOSSIL" => "set -g fish_prompt_fossil_status_added N9_ADDED",
            "SVN" => "status get-file functions/fish_svn_prompt.fish | source; set -g __fish_svn_prompt_char_added_display N9_ADDED",
            _ => "true",
        };
        let code = format!("cd {}; {settings}; function fish_prompt; set -l saved_pipe $pipestatus; set -l saved_status $status; {prompt}; printf '\\nN9-STATUS:%s:%s\\n' $saved_status (string join , $saved_pipe); end; false | true; fish_prompt", text(&working));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", binary.parent().unwrap()).env("HGENCODING", "utf-8").env("PYTHONUTF8", "1");
        if name == "SVN" { storage.as_ref().unwrap().environment(&mut command); }
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0, "{name}: {}", String::from_utf8_lossy(&result.err));
        let rendered = String::from_utf8(result.out).unwrap();
        let (prompt_output, status) = rendered.split_once("\nN9-STATUS:").expect("prompt status marker");
        if name == "JJ" {
            assert!(prompt_output.split("\x1b[").filter_map(|fragment| fragment.split_once('m'))
                .any(|(parameters, _)| parameters.contains(marker)),
                "configured conflict color missing from genuine Jujutsu state: {prompt_output:?}");
        } else {
            assert!(prompt_output.contains(marker), "{name}: selected repository data/configuration {marker:?} missing from {prompt_output:?}");
        }
        assert_eq!(status, "0:1,0\n", "{name}: incoming pipeline/status");
        assert!(!String::from_utf8_lossy(&result.err).contains("Unknown command"), "{name}: {}", String::from_utf8_lossy(&result.err));
        let absent_code = format!("cd {}; {prompt}", text(&working));
        exact(run(&mut f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&absent_code)]), b""), 1, b"", b"");
        for present in [false, true] {
            for sample in &samples {
                let code = format!("cd {}; status get-file {} | source; false | true; fish_prompt; if functions -q fish_right_prompt; fish_right_prompt; end", text(&working), quote(sample));
                let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
                if present {
                    command.env("PATH", binary.parent().unwrap()).env("HGENCODING", "utf-8").env("PYTHONUTF8", "1");
                    if name == "SVN" { storage.as_ref().unwrap().environment(&mut command); }
                }
                let result = run(&mut command, b"");
                assert_eq!(result.code, 0, "{name} {sample} dependency-present={present}: {}", String::from_utf8_lossy(&result.err));
                assert!(result.err.is_empty(), "{name} {sample} dependency-present={present}: {}", String::from_utf8_lossy(&result.err));
                assert!(String::from_utf8_lossy(&result.out).contains(working.file_name().unwrap().to_str().unwrap()), "{name} {sample}: prompt lost native repository directory identity");
            }
        }
        if let Some(boundary) = svn_boundary {
            // ASCII storage exercises the genuine consumer but is not a waiver:
            // these prompts require the working-copy cwd, unlike MIME's client.
            assert_eq!(boundary.code, 0, "unmet native SVN Unicode repository/cwd requirement; only ASCII storage/cwd with Unicode file contents is supported: {}", String::from_utf8_lossy(&boundary.err));
        }
        }));
        if result.is_err() {
            failures.push(name);
        } else {
            println!("PASS-VCS-PRODUCER {name}");
        }
    }
    assert!(failures.is_empty(), "required native VCS consumers failed: {failures:?}");
}
fn registry(f: &Fixture) {
    let program = PathBuf::from(std::env::var_os("SystemRoot").expect("native Windows system root")).join("System32/reg.exe");
    assert!(program.is_file(), "required native registry CLI");
    // Read-only system fixture: never create keys in the interactive user's hive.
    let key = r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT";
    let queried = run(Command::new(&program).args(["query", key]), b"");
    assert_eq!(queried.code, 0, "{}", String::from_utf8_lossy(&queried.err));
    let prefix = format!("{key}\\");
    let expected: BTreeSet<_> = String::from_utf8(queried.out).unwrap().lines()
        .map(str::trim).filter(|line| line.starts_with(&prefix)).map(str::to_owned).collect();
    assert!(expected.contains(&format!("{prefix}CurrentVersion")), "required read-only system registry fixture");
    let line = format!("reg query {}", quote(&prefix));
    let code = format!("complete -C {}", quote(&line));
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    command.env("PATH", program.parent().unwrap());
    let completed = run(&mut command, b"");
    assert_eq!(completed.code, 0, "{}", String::from_utf8_lossy(&completed.err));
    assert!(completed.err.is_empty(), "{}", String::from_utf8_lossy(&completed.err));
    let actual: BTreeSet<_> = String::from_utf8(completed.out).unwrap().lines()
        .map(|line| line.split('\t').next().unwrap().to_owned()).collect();
    assert_eq!(actual, expected, "actual native read-only registry child completions");
}
fn rustup(f: &Fixture) {
    let program = PathBuf::from(std::env::var_os("FISH_N9_RUSTUP").expect("required provisioned native rustup executable"));
    let home = PathBuf::from(std::env::var_os("FISH_N9_RUSTUP_HOME").expect("required provisioned rustup home with stable and rust-docs"));
    let configure = |command: &mut Command| {
        for (key, value) in f.command(&[]).get_envs() {
            if let Some(value) = value { command.env(key, value); }
        }
        command.env("PATH", program.parent().unwrap()).env("RUSTUP_HOME", &home).env("RUSTUP_TOOLCHAIN", "stable");
    };
    let query = |args: &[&str]| {
        let mut command = Command::new(&program);
        command.args(args).env_clear().current_dir(&f.root);
        configure(&mut command);
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0, "real rustup {args:?}: {}", String::from_utf8_lossy(&result.err));
        String::from_utf8(result.out).unwrap()
    };
    let targets: Vec<_> = query(&["target", "list"]).lines().filter_map(|line| line.split_whitespace().next().map(str::to_owned)).collect();
    assert!(targets.iter().any(|target| target == "x86_64-pc-windows-msvc"));
    let mut all = BTreeSet::new();
    let mut installed = BTreeSet::new();
    for line in query(&["component", "list"]).lines() {
        let Some(raw) = line.split_whitespace().next() else { continue };
        let mut component = raw;
        for target in &targets {
            if let Some(name) = raw.strip_suffix(target.as_str()).and_then(|name| name.strip_suffix('-')) { component = name; break; }
        }
        all.insert(component.to_owned());
        if line.contains("(installed)") { installed.insert(component.to_owned()); }
    }
    assert!(installed.contains("cargo") && installed.contains("rustc"));
    for (line, expected) in [
        ("rustup component add ", all.difference(&installed).cloned().collect::<BTreeSet<_>>()),
        ("rustup --toolchain stable component remove ", installed),
    ] {
        let code = format!("complete -C {}", quote(line));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        configure(&mut command);
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
        let actual: BTreeSet<_> = String::from_utf8(result.out).unwrap().lines().filter_map(|line| line.split('\t').next()).filter(|item| !item.starts_with('-')).map(str::to_owned).collect();
        assert_eq!(actual, expected, "actual rustup component consumer: {line}");
    }
    for expected in ["u8", "async", "std::vec"] {
        let code = format!("complete -C {}", quote(&format!("rustup doc {expected}")));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        configure(&mut command);
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
        assert!(String::from_utf8_lossy(&result.out).lines().any(|line| line.split('\t').next() == Some(expected)), "native rustup documentation completion {expected}: {}", String::from_utf8_lossy(&result.out));
    }
    for line in ["rustup component remove ", "rustup doc u8"] {
        let result = f.eval(&format!("status get-file completions/rustup.fish | source; and complete -C {}", quote(line)));
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty(), "absent rustup must not launch Unix tools: {}", String::from_utf8_lossy(&result.err));
    }
    assert!(success(f, "status get-file completions/rustup.fish | source; and complete -C 'rustup sh'").lines().any(|line| line.split('\t').next() == Some("show")));
    let project = f.root.join("native Cargo project 雪");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "#[cfg(not(feature = \"native-雪\"))]\ncompile_error!(\"selected feature was not consumed\");\n").unwrap();
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"n9-consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[features]\n\"native-雪\" = []\n[workspace]\n").unwrap();
    let python = PathBuf::from(std::env::var_os("FISH_N9_PYTHON").expect("required native Python JSON dependency"));
    let path = std::env::join_paths([program.parent().unwrap(), python.parent().unwrap()]).unwrap();
    let mut selected = Vec::with_capacity(2);
    for (line, expected) in [("cargo build --features native-", "native-雪"), ("cargo build --package n9", "n9-consumer")] {
        let code = format!("cd {}; complete -C {}", text(&project), quote(line));
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        configure(&mut command);
        command.env("PATH", &path);
        let result = run(&mut command, b"");
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty(), "actual Cargo JSON consumer: {}", String::from_utf8_lossy(&result.err));
        let output = String::from_utf8(result.out).unwrap();
        let choice = output.split('\n').filter_map(|line| line.split('\t').next())
            .find(|candidate| *candidate == expected)
            .unwrap_or_else(|| panic!("actual Cargo JSON candidate {expected}: {output}"));
        selected.push(choice.to_owned());
        let code = format!("status get-file completions/cargo.fish | source; and {code}");
        let mut absent = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        absent.env("PATH", python.parent().unwrap());
        let result = run(&mut absent, b"");
        assert_eq!(result.code, 0);
        assert!(result.err.is_empty(), "absent Cargo must not spawn a JSON decoder: {}", String::from_utf8_lossy(&result.err));
    }
    // A resolved multicall alias must still invoke Cargo, not rustup. Consume
    // both selections through the real compiler; the source rejects a missing
    // feature rather than relying on a completion string or empty output.
    let code = format!("cd {}; set -l cargo (command -s cargo); command $cargo check --offline --features {} --package {}",
        text(&project), quote(&selected[0]), quote(&selected[1]));
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
    configure(&mut command);
    command.env("PATH", &path)
        .env("CARGO_HOME", f.root.join("cargo consumer home"))
        .env("CARGO_TARGET_DIR", project.join("selected feature output"));
    let consumed = run(&mut command, b"");
    assert_eq!(consumed.code, 0, "selected Cargo namespace and callable alias: {}", String::from_utf8_lossy(&consumed.err));
    assert!(success(f, "status get-file completions/cargo.fish | source; and complete -C 'cargo bu'").lines().any(|line| line.split('\t').next() == Some("build")));
}
fn inventory(f: &Fixture) {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeSet<String>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                output.insert(path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/"));
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut actual = BTreeSet::new();
    for directory in ["share", "etc"] { visit(root, &root.join(directory), &mut actual); }
    let ledger = include_str!("../../doc_internal/win_compat/n9-assets.tsv");
    let rows: Vec<Vec<&str>> = ledger.lines().skip(1).map(|line| line.split('\t').collect()).collect();
    assert!(rows.iter().all(|row| row.len() == 6 && !row[1].is_empty() && !row[2].is_empty() && !row[3].is_empty()));
    let recorded: BTreeSet<_> = rows.iter().map(|row| row[0].to_owned()).collect();
    assert_eq!(recorded, actual, "every shipped file must retain a concrete disposition and consumer requirement");
    let mut completion_failures = Vec::new();
    for row in rows.iter().filter(|row| row[3] == "shipped-ledger-and-static-completion-consumers") {
        let command = row[0].strip_prefix("share/completions/").unwrap().strip_suffix(".fish").unwrap();
        let context = row[5].split(';').find_map(|field| field.strip_prefix("context=")).unwrap_or(command);
        let embedded = row[0].strip_prefix("share/").unwrap();
        // Complete an unfinished option, not the already accepted full candidate.
        // Some tables intentionally stop offering an option after it is present.
        let prefix = &row[4][..row[4].char_indices().next_back().unwrap().0];
        let code = format!("status get-file {} | source; complete -C {}", quote(embedded), quote(&format!("{context} {prefix}")));
        let result = f.eval(&code);
        let output = String::from_utf8(result.out).unwrap();
        if result.code != 0 || !result.err.is_empty() || !output.lines().any(|line| line.split('\t').next() == Some(row[4])) {
            completion_failures.push(format!("{} context={context} {prefix:?} expected={} status={} stdout={output:?} stderr={:?}", row[0], row[4], result.code, String::from_utf8_lossy(&result.err)));
        }
    }
    let uncovered: Vec<_> = rows.iter().filter(|row| row.iter().any(|field| field.contains("REQUIRED:"))).map(|row| (row[0], row[2], row[3], row[5])).collect();
    assert!(completion_failures.is_empty() && uncovered.is_empty(), "N9 static consumer failures: {completion_failures:#?}\nN9 still requires real consumer/dependency contexts (static-option checks do not certify unmapped paths): {uncovered:#?}");
}
fn clipboard(f: &Fixture) {
    let mut command = Command::new(&f.probe);
    command.arg("n6-clipboard-launch").arg(&f.root).arg(&f.fish).arg(&f.probe);
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "required isolated clipboard fixture: {}", String::from_utf8_lossy(&result.err));
    assert_eq!(fs::read(f.root.join("clipboard-passed")).unwrap(), b"pipe-selection-line-paste-private");
}
fn reserved(f: &Fixture) {
    for name in ["fg", "bg", "kill", "suspend"] {
        let program = f.root.join(format!("{name}.exe"));
        fs::copy(&f.probe, &program).unwrap();
        let marker = f.root.join(format!("fallback-{name}"));
        let mut control = Command::new(&program);
        control.env("FISH_N9_FACADE_MARKER", &marker);
        assert_eq!(run(&mut control, b"").code, 93, "{name}: owned fallback probe must be executable");
        assert_eq!(fs::read_to_string(&marker).unwrap(), format!("{name}.exe"));
        fs::remove_file(&marker).unwrap();
        let code = format!("{name} 1");
        let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.env("PATH", &f.root).env("FISH_N9_FACADE_MARKER", &marker);
        let result = run(&mut command, b"");
        assert_ne!(result.code, 0, "{name}: reserved native facade must reject unsupported job control");
        assert_ne!(result.code, 93, "{name}: executed external fallback");
        assert!(!marker.exists(), "{name}: reserved facade invoked the owned external executable");
    }
}
fn interrupted(f: &Fixture, kind: &str) {
    let mut command = Command::new(&f.probe);
    command.arg("n9-interrupted-workflow").arg(&f.root).arg(&f.fish).arg(&f.probe).arg(kind)
        .env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(key, value); }
    }
    let result = run(&mut command, b"");
    assert_eq!(result.code, 0, "{kind} actual Ctrl-C workflow:\n{}\n{}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
}
