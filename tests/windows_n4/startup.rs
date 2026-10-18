use super::fixture::{Captured, Fixture, LIMIT, exact, quote, run};
use std::{
    ffi::{OsStr, c_void},
    fs,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

fn launch(f: &Fixture, arguments: &[&str]) -> Captured {
    run(
        &mut f.command(&arguments.iter().map(OsStr::new).collect::<Vec<_>>()),
        b"",
    )
}
fn normal(f: &Fixture, code: &str) -> Captured {
    launch(f, &["-c", code])
}

pub(super) fn exercise_startup(f: &Fixture) {
    let config = f.state.join("config/fish");
    let admin = f.root.join("etc/fish");
    let vendor = f.root.join("share/fish/vendor_conf.d");
    for path in [config.join("conf.d"), admin.join("conf.d"), vendor.clone()] {
        fs::create_dir_all(path).unwrap();
    }
    fs::write(config.join("conf.d/10-order.fish"), "echo user-snippet\n").unwrap();
    fs::write(admin.join("conf.d/10-order.fish"), "echo masked-admin\n").unwrap();
    fs::write(vendor.join("10-order.fish"), "echo masked-vendor\n").unwrap();
    fs::write(config.join("conf.d/20-mask.fish"), "").unwrap();
    fs::write(vendor.join("20-mask.fish"), "echo masked-empty\n").unwrap();
    fs::write(admin.join("conf.d/30-admin.fish"), "echo admin-snippet\n").unwrap();
    fs::write(vendor.join("40-vendor.fish"), "echo vendor-snippet\n").unwrap();
    fs::write(admin.join("config.fish"), "echo admin-config\n").unwrap();
    fs::write(config.join("config.fish"), "echo user-config\n").unwrap();
    exact(launch(f, &["-C", "echo init-one", "--init-command", "echo init-two", "-c", "printf '<%s>\\n' $argv; exit 23", "雪", "two words"]), 23,
        "user-snippet\nadmin-snippet\nvendor-snippet\nadmin-config\nuser-config\ninit-one\ninit-two\n<雪>\n<two words>\n".as_bytes(), b"");
    exact(
        launch(
            f,
            &[
                "-N",
                "-C",
                "echo init",
                "-c",
                "echo execution; set -q fish_private_mode; echo $status",
            ],
        ),
        0,
        b"init\nexecution\n0\n",
        b"",
    );
    exact(
        launch(f, &["-N", "-l", "-c", "status is-login; echo $status"]),
        0,
        b"0\n",
        b"",
    );
    exact(
        launch(f, &["-N", "--login", "-c", "status is-login; echo $status"]),
        0,
        b"0\n",
        b"",
    );
    exact(
        launch(
            f,
            &["-N", "-P", "-c", "set -q fish_private_mode; echo $status"],
        ),
        0,
        b"0\n",
        b"",
    );
    exact(
        launch(
            f,
            &[
                "-N",
                "--private",
                "-c",
                "set -q fish_private_mode; echo $status",
            ],
        ),
        0,
        b"0\n",
        b"",
    );
    exact(
        launch(
            f,
            &[
                "-N",
                "-c",
                "status is-interactive; echo $status; status is-login; echo $status",
            ],
        ),
        0,
        b"1\n1\n",
        b"",
    );
    let untouched = f.root.join("no execute state 雪");
    let mut command = f.command(&[
        OsStr::new("-n"),
        OsStr::new("-C"),
        OsStr::new("echo init > init-side-effect"),
        OsStr::new("-c"),
        OsStr::new("echo execution > command-side-effect"),
    ]);
    for key in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"] {
        command.env(key, &untouched);
    }
    exact(run(&mut command, b""), 0, b"", b"");
    assert!(!untouched.exists());
    assert!(!f.root.join("init-side-effect").exists());
    assert!(!f.root.join("command-side-effect").exists());
    let profile = f.root.join("startup profile 雪.txt");
    exact(
        launch(
            f,
            &[
                "-N",
                "--profile-startup",
                profile.to_str().unwrap(),
                "-C",
                "echo profiled-init",
                "-c",
                "exit 19",
            ],
        ),
        19,
        b"profiled-init\n",
        b"",
    );
    let profile = fs::read_to_string(profile).unwrap();
    assert!(profile.contains("profiled-init"));
    let unavailable = f.root.join("absent parent/profile.txt");
    let failed = launch(
        f,
        &[
            "-N",
            "--profile-startup",
            unavailable.to_str().unwrap(),
            "-c",
            "echo should-not-run",
        ],
    );
    assert_ne!(failed.code, 0);
    assert!(failed.out.is_empty());
    assert!(!failed.err.is_empty());
    for arguments in [
        &["-C"][..],
        &["--init-command"][..],
        &["--profile-startup"][..],
        &["--private=bad"][..],
        &["--login=bad"][..],
    ] {
        let failed = launch(f, arguments);
        assert_ne!(failed.code, 0);
        assert!(!failed.err.is_empty());
    }
    // Invalid explicit overrides must not be silently replaced by the real known-folder roots.
    let mut invalid = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("echo should-not-run"),
    ]);
    invalid.env("XDG_CONFIG_HOME", "relative-root");
    let failed = run(&mut invalid, b"");
    assert_ne!(failed.code, 0);
    assert!(failed.out.is_empty());
    assert!(String::from_utf8_lossy(&failed.err).contains("XDG_CONFIG_HOME"));
    assert!(!f.root.join("relative-root").exists());
    // Remove markers, not configuration discovery, for the remaining real bootstrap scenarios.
    for path in [
        config.join("config.fish"),
        admin.join("config.fish"),
        config.join("conf.d/10-order.fish"),
        admin.join("conf.d/10-order.fish"),
        vendor.join("10-order.fish"),
        vendor.join("20-mask.fish"),
        admin.join("conf.d/30-admin.fish"),
        vendor.join("40-vendor.fish"),
    ] {
        fs::write(path, "").unwrap();
    }
    roots(f);
    autoload(f);
    saving(f);
    extended_roots(f);
    exercise_selected_entry_namespace(f);
}

fn roots(f: &Fixture) {
    // This helper performs only SHGetKnownFolderPath(KF_FLAG_DONT_VERIFY), never state init.
    let discovered = run(
        &mut std::process::Command::new(&f.probe).arg("known-folder-roots"),
        b"",
    );
    assert_eq!(discovered.code, 0);
    assert!(discovered.err.is_empty());
    let discovered = String::from_utf8(discovered.out).unwrap();
    let folders: Vec<_> = discovered.lines().map(std::path::PathBuf::from).collect();
    assert_eq!(folders.len(), 5);
    assert_eq!(
        folders[2..],
        ["config", "data", "cache"].map(|role| folders[0].join("fish").join(role))
    );
    let roots = normal(
        f,
        "printf '%s\\n' $__fish_config_dir $__fish_user_data_dir $__fish_cache_dir $__fish_data_dir $__fish_sysconf_dir",
    );
    assert_eq!(roots.code, 0, "{}", String::from_utf8_lossy(&roots.err));
    assert!(roots.err.is_empty());
    let paths: Vec<_> = String::from_utf8(roots.out)
        .unwrap()
        .lines()
        .map(std::path::PathBuf::from)
        .collect();
    assert_eq!(
        paths,
        vec![
            f.state.join("config/fish"),
            f.state.join("data/fish"),
            f.state.join("cache/fish"),
            f.root.join("share/fish"),
            f.root.join("etc/fish")
        ]
    );
    let mut command = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("printf '%s\\n' $__fish_config_dir $__fish_user_data_dir $__fish_cache_dir"),
    ]);
    for name in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"] {
        command.env_remove(name);
    }
    let roots = run(&mut command, b"");
    assert_eq!(roots.code, 0, "{}", String::from_utf8_lossy(&roots.err));
    assert!(roots.err.is_empty());
    let paths: Vec<_> = String::from_utf8(roots.out)
        .unwrap()
        .lines()
        .map(std::path::PathBuf::from)
        .collect();
    assert_eq!(
        paths,
        vec![
            f.state.join("home/.config/fish"),
            f.state.join("home/.local/share/fish"),
            f.state.join("home/.cache/fish")
        ]
    );
}

fn autoload(f: &Fixture) {
    let first = f.root.join("autoload functions 雪");
    let second = f.root.join("autoload replacement");
    for (dir, label) in [(&first, "first"), (&second, "second")] {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("n4_disk_autoload.fish"),
            format!(
                "function n4_disk_autoload; echo {label}; for arg in $argv; echo $arg; end; end\n"
            ),
        )
        .unwrap();
    }
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; n4_disk_autoload 雪; type -t n4_disk_autoload; set fish_function_path {}; n4_disk_autoload two; functions -e n4_disk_autoload; functions -q n4_disk_autoload; echo $status",
                quote(first.to_str().unwrap()),
                quote(second.to_str().unwrap())
            ),
        ),
        0,
        "first\n雪\nfunction\nsecond\ntwo\n1\n".as_bytes(),
        b"",
    );
    exact(
        normal(
            f,
            "cd 'autoload functions 雪'; set fish_function_path .; n4_disk_autoload relative",
        ),
        0,
        b"first\nrelative\n",
        b"",
    );
    // A prior negative lookup must not conceal a newly saved disk provider.
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; functions -q n4_retry; echo $status; printf 'function n4_retry; echo retry; end\\n' | builtin __fish_native_fs atomic-write {}/n4_retry.fish; n4_retry",
                quote(first.to_str().unwrap()),
                quote(first.to_str().unwrap())
            ),
        ),
        0,
        b"1\nretry\n",
        b"",
    );
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; n4_retry; printf 'function n4_retry; echo renew; end\\n' | builtin __fish_native_fs atomic-write {}/n4_retry.fish; n4_retry",
                quote(first.to_str().unwrap()),
                quote(first.to_str().unwrap())
            ),
        ),
        0,
        b"retry\nrenew\n",
        b"",
    );
}

fn saving(f: &Fixture) {
    let directory = f.root.join("saved definitions 雪");
    exact(
        normal(
            f,
            &format!(
                "function n4_saved; printf '<%s>\\n' $argv; return 17; end; funcsave -q -d {} n4_saved",
                quote(directory.to_str().unwrap())
            ),
        ),
        0,
        b"",
        b"",
    );
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; n4_saved 雪 'two words' ''",
                quote(directory.to_str().unwrap())
            ),
        ),
        17,
        "<雪>\n<two words>\n<>\n".as_bytes(),
        b"",
    );
    exact(
        normal(
            f,
            &format!(
                "function n4_saved; echo replacement; end; funcsave -q -d {} n4_saved",
                quote(directory.to_str().unwrap())
            ),
        ),
        0,
        b"",
        b"",
    );
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; n4_saved",
                quote(directory.to_str().unwrap())
            ),
        ),
        0,
        b"replacement\n",
        b"",
    );
    exact(
        normal(
            f,
            "function n4_default_saved; printf '<%s>\\n' $argv; end; funcsave -q n4_default_saved",
        ),
        0,
        b"",
        b"",
    );
    exact(
        normal(f, "n4_default_saved '雪 space'"),
        0,
        "<雪 space>\n".as_bytes(),
        b"",
    );
}

pub(super) fn exercise_recovery(f: &Fixture) {
    let directory = f.root.join("denied save 雪");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("n4_preserved.fish");
    exact(
        normal(
            f,
            &format!(
                "function n4_preserved; echo preserved; end; funcsave -q -d {} n4_preserved",
                quote(directory.to_str().unwrap())
            ),
        ),
        0,
        b"",
        b"",
    );
    let old = fs::read(&path).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    let failed = normal(
        f,
        &format!(
            "function n4_preserved; echo broken; end; funcsave -q -d {} n4_preserved",
            quote(directory.to_str().unwrap())
        ),
    );
    assert_ne!(failed.code, 0);
    assert!(failed.out.is_empty());
    assert!(String::from_utf8_lossy(&failed.err).contains("atomic-write"));
    assert_eq!(fs::read(&path).unwrap(), old);
    drop(held);
    exact(
        normal(
            f,
            &format!(
                "set fish_function_path {}; n4_preserved",
                quote(directory.to_str().unwrap())
            ),
        ),
        0,
        b"preserved\n",
        b"",
    );
    // A malformed source must not be recorded as successfully loaded. Replace through the real
    // transaction path and retry in the same process, preserving source diagnostics.
    let malformed = directory.join("n4_malformed.fish");
    fs::write(&malformed, "function n4_malformed;\n").unwrap();
    let result = normal(
        f,
        &format!(
            "set fish_function_path {}; n4_malformed; printf 'function n4_malformed; echo repaired; end\\n' | builtin __fish_native_fs atomic-write {}; n4_malformed",
            quote(directory.to_str().unwrap()),
            quote(malformed.to_str().unwrap())
        ),
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.out, b"repaired\n");
    assert!(String::from_utf8_lossy(&result.err).contains("n4_malformed.fish"));
    interrupted_saving(f);
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        security: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn WaitForSingleObject(object: *mut c_void, millis: u32) -> u32;
}

struct SaveEvent {
    name: String,
    handle: OwnedHandle,
}
impl SaveEvent {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "Local\\fish-n4-function-save-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(
            !raw.is_null(),
            "CreateEventW: {}",
            std::io::Error::last_os_error()
        );
        Self {
            name,
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
        }
    }
    fn wait(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle.as_raw_handle(), LIMIT.as_millis() as u32) },
            0,
            "required function save barrier {}",
            self.name
        );
    }
}

struct InterruptedSave(Option<Child>);
impl InterruptedSave {
    fn spawn(command: &mut Command) -> Self {
        Self(Some(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }
    fn kill(mut self, stage: &str) {
        let child = self.0.as_mut().unwrap();
        assert!(
            child.try_wait().unwrap().is_none(),
            "{stage} writer exited before interruption"
        );
        child.kill().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) },
            0,
            "{stage} function save writer did not terminate"
        );
        // This probe has no descendants; its pipes reach EOF after the bounded native wait.
        let result = self.0.take().unwrap().wait_with_output().unwrap();
        let code = result
            .status
            .code()
            .expect("native interrupted writer exit code");
        assert_ne!(
            code,
            0,
            "{stage} interrupted save falsely succeeded: status={}, stderr={}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stdout.is_empty(),
            "{stage} interrupted save emitted partial success: {:?}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
impl Drop for InterruptedSave {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            if unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) } == 0
            {
                let _ = child.wait();
            }
        }
    }
}

fn interrupted_saving(f: &Fixture) {
    let directory = f.root.join("interrupted function save 雪");
    let payload_directory = f.root.join("replacement function payload 雪");
    let target = directory.join("n4_interrupted.fish");
    let payload = payload_directory.join("n4_interrupted.fish");
    for stage in ["partial", "complete"] {
        exact(
            normal(
                f,
                &format!(
                    "function n4_interrupted; printf 'preserved:<%s>\\n' $argv; return 17; end; funcsave -q -d {} n4_interrupted",
                    quote(directory.to_str().unwrap())
                ),
            ),
            0,
            b"",
            b"",
        );
        let prior = fs::read(&target).unwrap();
        exact(
            normal(
                f,
                &format!(
                    "function n4_interrupted; echo unacknowledged-replacement; return 29; end; funcsave -q -d {} n4_interrupted",
                    quote(payload_directory.to_str().unwrap())
                ),
            ),
            0,
            b"",
            b"",
        );
        let ready = SaveEvent::new(stage);
        let release = SaveEvent::new("never-released");
        let mut command = Command::new(&f.probe);
        command
            .args([
                OsStr::new("n4-rewrite"),
                target.as_os_str(),
                payload.as_os_str(),
                OsStr::new(&ready.name),
                OsStr::new(&release.name),
                OsStr::new(stage),
            ])
            .current_dir(&f.root)
            .env_clear()
            .env("PATH", "");
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        // Keep release closed until the owned writer is killed, including during panic cleanup:
        // releasing this barrier would allow the real transaction to commit.
        let writer = InterruptedSave::spawn(&mut command);
        ready.wait();
        assert_eq!(
            fs::read(&target).unwrap(),
            prior,
            "{stage} save changed the definition before commit"
        );
        writer.kill(stage);
        assert_eq!(
            fs::read(&target).unwrap(),
            prior,
            "{stage} killed save lost the prior definition"
        );
        exact(
            normal(
                f,
                &format!(
                    "set fish_function_path {}; n4_interrupted 雪 'two words'",
                    quote(directory.to_str().unwrap())
                ),
            ),
            17,
            "preserved:<雪>\npreserved:<two words>\n".as_bytes(),
            b"",
        );
    }
}

fn extended_roots(f: &Fixture) {
    // These names are literal NTFS identities, not ordinary-path aliases. Both
    // executable-relative assets and user state must compose their suffixes
    // without inserting '/' into the explicit extended namespace.
    let physical = fs::canonicalize(&f.root).unwrap();
    let installation = physical.join("literal installation . ");
    let state = physical.join("literal state . ");
    fs::create_dir(&installation).unwrap();
    let fish = installation.join("fish.exe");
    fs::copy(&f.fish, &fish).unwrap();
    let launch = |code: &str, use_xdg: bool| {
        let base = f.command(&[OsStr::new("-c"), OsStr::new(code)]);
        let mut command = Command::new(&fish);
        command
            .args(base.get_args())
            .current_dir(&f.root)
            .env_clear();
        for (key, value) in base.get_envs() {
            if let Some(value) = value {
                command.env(key, value);
            }
        }
        for (key, role) in [
            ("HOME", "home"),
            ("USERPROFILE", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ] {
            command.env(key, state.join(role));
        }
        if !use_xdg {
            for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"] {
                command.env_remove(key);
            }
        }
        command.env("fish_history", "n4_literal");
        run(&mut command, b"")
    };
    exact(
        launch(
            "set -U n4_literal_stamp 雪; or exit 81; function n4_literal_saved; printf '<%s>\\n' $argv; return 17; end; funcsave -q n4_literal_saved; or exit 82; builtin history append 'literal history'; builtin history save",
            true,
        ),
        0,
        b"",
        b"",
    );
    exact(
        launch(
            "test \"$n4_literal_stamp\" = 雪; or exit 83; n4_literal_saved 雪 'two words'; set -l result $status; builtin history search --exact 'literal history'; exit $result",
            true,
        ),
        17,
        "<雪>\n<two words>\nliteral history\n".as_bytes(),
        b"",
    );
    assert!(
        state
            .join("config")
            .join("fish")
            .join("fish_variables")
            .is_file()
    );
    assert!(
        state
            .join("data")
            .join("fish")
            .join("n4_literal_history")
            .is_file()
    );
    exact(
        launch(
            "set -U n4_home_stamp home; or exit 84; function n4_home_saved; echo home-function; end; funcsave -q n4_home_saved; or exit 85; builtin history append home-history; builtin history save",
            false,
        ),
        0,
        b"",
        b"",
    );
    exact(
        launch(
            "test \"$n4_home_stamp\" = home; or exit 86; n4_home_saved; builtin history search --exact home-history",
            false,
        ),
        0,
        b"home-function\nhome-history\n",
        b"",
    );
    assert!(
        state
            .join("home")
            .join(".config")
            .join("fish")
            .join("fish_variables")
            .is_file()
    );
    assert!(
        state
            .join("home")
            .join(".local")
            .join("share")
            .join("fish")
            .join("n4_literal_history")
            .is_file()
    );
    assert!(
        !f.root.join("literal state").exists(),
        "extended state was redirected to an ordinary-path alias"
    );
    assert!(!f.root.join("literal installation").exists());
    fs::remove_dir_all(&installation).unwrap();
    fs::remove_dir_all(&state).unwrap();
}

pub(super) fn exercise_selected_entry_namespace(f: &Fixture) {
    let ordinary = f.root.join("selected entry namespace");
    let config = ordinary.join("config/fish");
    let functions = ordinary.join("functions");
    fs::create_dir_all(config.join("conf.d")).unwrap();
    fs::create_dir_all(&functions).unwrap();
    let physical = fs::canonicalize(&ordinary).unwrap();
    let literal_name = "literal image . ";
    let literals = [
        physical.join(literal_name),
        fs::canonicalize(config.join("conf.d"))
            .unwrap()
            .join(literal_name),
        fs::canonicalize(&functions).unwrap().join(literal_name),
    ];
    for path in &literals {
        fs::create_dir(path).unwrap();
    }
    fs::write(
        config.join("conf.d/10-selected.fish"),
        "echo selected-config\n",
    )
    .unwrap();
    fs::write(
        functions.join("n4_selected.fish"),
        "function n4_selected; echo selected-autoload; end\n",
    )
    .unwrap();
    let launch = |code: &str| {
        let mut command = f.command(&[OsStr::new("-c"), OsStr::new(code)]);
        command
            .current_dir(&ordinary)
            .env("XDG_CONFIG_HOME", ordinary.join("config"));
        run(&mut command, b"")
    };
    exact(
        launch(&format!(
            "set fish_function_path {}; n4_selected; printf '%s\\n' functions/*.fish",
            quote(functions.to_str().unwrap()),
        )),
        0,
        b"selected-config\nselected-autoload\nfunctions/n4_selected.fish\n",
        b"",
    );
    // Selecting the literal through an ordinary spelling must still fail, rather
    // than silently skipping it or resolving a trailing-dot/space alias.
    let rejected = launch("printf '%s\\n' literal*; exit $status");
    assert_ne!(rejected.code, 0);
    assert_eq!(rejected.out, b"selected-config\n");
    assert!(!rejected.err.is_empty());
    exact(
        launch(&format!(
            "for path in {}*; test -d $path; or exit 91; echo literal-selected; end",
            quote(physical.join("literal").to_str().unwrap()),
        )),
        0,
        b"selected-config\nliteral-selected\n",
        b"",
    );
    for path in &literals {
        fs::remove_dir(path).unwrap();
    }
    fs::remove_dir_all(ordinary).unwrap();
}
