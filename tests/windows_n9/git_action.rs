use super::{Family, fixture::{Captured, Fixture, exact, quote, run}};
use std::{ffi::OsStr, fs, path::{Path, PathBuf}, process::Command};

pub(super) const FAMILIES: &[Family] = &[
    ("core-native-git-action-real-operation-transitions", operations),
];

struct Repository<'a> {
    fixture: &'a Fixture,
    git: PathBuf,
    root: PathBuf,
    nested: PathBuf,
}
impl Repository<'_> {
    fn environment(&self, command: &mut Command) {
        super::functions_tools::tool_environment(command, &self.git);
        command.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.fixture.root.join("unused-global-git-config"))
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never");
    }
    fn git(&self, args: &[&str]) -> Captured {
        let mut command = Command::new(&self.git);
        command.args(args).current_dir(&self.root).env_clear();
        for (key, value) in self.fixture.command(&[]).get_envs() {
            if let Some(value) = value { command.env(key, value); }
        }
        self.environment(&mut command);
        run(&mut command, b"")
    }
    #[track_caller]
    fn success(&self, args: &[&str]) -> Vec<u8> {
        let result = self.git(args);
        assert_eq!(result.code, 0, "git {args:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
        result.out
    }
    fn helper(&self, working: &Path, explicit: bool, with_git: bool) -> Captured {
        let code = if explicit {
            format!("fish_print_git_action {}", quote(self.root.join(".git").to_str().unwrap()))
        } else {
            "fish_print_git_action".into()
        };
        let mut command = self.fixture.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]);
        command.current_dir(working);
        if with_git { self.environment(&mut command); }
        run(&mut command, b"")
    }
    #[track_caller]
    fn action(&self, expected: Option<&str>) {
        let (code, out) = match expected {
            Some(value) => (0, value.as_bytes()),
            None => (1, &b""[..]),
        };
        for working in [&self.root, &self.nested] {
            exact(self.helper(working, false, true), code, out, b"");
            exact(self.helper(working, true, true), code, out, b"");
        }
        // An explicit git directory works outside the worktree, without Git on PATH.
        exact(self.helper(&self.fixture.root, true, false), code, out, b"");
    }
    #[track_caller]
    fn clean(&self, branch: &str, content: &str) {
        self.action(None);
        assert_eq!(self.success(&["rev-parse", "HEAD"]), self.success(&["rev-parse", branch]));
        assert_eq!(fs::read(self.root.join("conflict.txt")).unwrap(), content.as_bytes());
        exact(self.git(&["status", "--porcelain=v1"]), 0, b"", b"");
        for marker in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "BISECT_LOG", "sequencer", "rebase-merge", "rebase-apply"] {
            assert!(!self.root.join(".git").join(marker).exists(), "operation state leaked: {marker}");
        }
    }
    #[track_caller]
    fn conflict(&self, result: Captured, base: &str, ours: &str, theirs: &str) {
        assert_eq!(result.code, 1, "expected a content conflict: stdout={} stderr={}",
            String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
        // A failed setup command is only accepted with the exact three-way conflict,
        // not merely its exit status or a hand-created operation marker.
        exact(self.git(&["status", "--porcelain=v1"]), 0, b"UU conflict.txt\n", b"");
        for (stage, expected) in [(":1:conflict.txt", base), (":2:conflict.txt", ours), (":3:conflict.txt", theirs)] {
            exact(self.git(&["show", stage]), 0, expected.as_bytes(), b"");
        }
        let content = fs::read_to_string(self.root.join("conflict.txt")).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 5, "{content}");
        assert!(lines[0].starts_with("<<<<<<< "), "{content}");
        assert_eq!(lines[1], ours.trim_end_matches('\n'));
        assert_eq!(lines[2], "=======");
        assert_eq!(lines[3], theirs.trim_end_matches('\n'));
        assert!(lines[4].starts_with(">>>>>>> "), "{content}");
    }
}

fn operations(f: &Fixture) {
    let git = PathBuf::from(std::env::var_os("FISH_N9_GIT").expect("required genuine native Git fixture"));
    assert!(git.is_absolute() && git.is_file(), "required native Git: {}", git.display());
    let root = f.root.join("Git operation transitions 雪 🐟");
    fs::create_dir(&root).unwrap();
    let nested = root.join("nested directory 雪");
    fs::create_dir(&nested).unwrap();
    let repository = Repository { fixture: f, git, root, nested };
    let r = &repository;
    r.success(&["init", "--quiet", "--template=", "-b", "n9-main"]);
    // All configuration belongs exclusively to this owned repository. No installed
    // templates, hooks, signing agent, workstation config, or editor is consulted.
    let hooks = f.root.join("empty Git hooks 雪");
    fs::create_dir(&hooks).unwrap();
    for (name, value) in [
        ("user.name", "Native Git consumer 雪"),
        ("user.email", "native-git-consumer@example.invalid"),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.hooksPath", hooks.to_str().unwrap()),
        ("merge.conflictStyle", "merge"),
        ("rerere.enabled", "false"),
    ] {
        r.success(&["config", "--local", name, value]);
    }
    let file = r.root.join("conflict.txt");
    fs::write(&file, b"base\n").unwrap();
    r.success(&["add", "--", "conflict.txt"]);
    r.success(&["commit", "--quiet", "-m", "base"]);
    r.success(&["branch", "n9-base"]);
    r.success(&["switch", "--quiet", "-c", "n9-topic-one"]);
    fs::write(&file, b"topic-one\n").unwrap();
    r.success(&["commit", "--quiet", "-am", "topic-one"]);
    r.success(&["switch", "--quiet", "-c", "n9-topic-two"]);
    fs::write(&file, b"topic-two\n").unwrap();
    r.success(&["commit", "--quiet", "-am", "topic-two"]);
    r.success(&["switch", "--quiet", "n9-main"]);
    fs::write(&file, b"main\n").unwrap();
    r.success(&["commit", "--quiet", "-am", "main"]);
    r.clean("n9-main", "main\n");

    exact(r.helper(&f.root, false, true), 1, b"", b"");
    exact(f.eval("fish_print_git_action"), 1, b"", b"");
    exact(r.helper(&r.root, false, false), 1, b"", b"");

    r.conflict(r.git(&["merge", "--no-edit", "n9-topic-one"]), "base\n", "main\n", "topic-one\n");
    r.action(Some("merge"));
    r.success(&["merge", "--abort"]);
    r.clean("n9-main", "main\n");

    r.conflict(r.git(&["cherry-pick", "n9-topic-one"]), "base\n", "main\n", "topic-one\n");
    r.action(Some("cherry-pick"));
    r.success(&["cherry-pick", "--abort"]);
    r.clean("n9-main", "main\n");

    r.conflict(r.git(&["cherry-pick", "n9-topic-one", "n9-topic-two"]), "base\n", "main\n", "topic-one\n");
    r.action(Some("cherry-pick-sequence"));
    r.success(&["cherry-pick", "--abort"]);
    r.clean("n9-main", "main\n");

    r.conflict(r.git(&["revert", "--no-edit", "n9-topic-one"]), "topic-one\n", "main\n", "base\n");
    r.action(Some("revert"));
    r.success(&["revert", "--abort"]);
    r.clean("n9-main", "main\n");

    r.conflict(r.git(&["revert", "--no-edit", "n9-topic-two", "n9-topic-one"]), "topic-two\n", "main\n", "topic-one\n");
    r.action(Some("revert-sequence"));
    r.success(&["revert", "--abort"]);
    r.clean("n9-main", "main\n");

    r.success(&["bisect", "start", "n9-topic-two", "n9-base"]);
    assert_eq!(r.success(&["rev-parse", "HEAD"]), r.success(&["rev-parse", "n9-topic-one"]));
    assert_eq!(fs::read(&file).unwrap(), b"topic-one\n");
    r.action(Some("bisect"));
    r.success(&["bisect", "reset"]);
    r.clean("n9-main", "main\n");

    r.success(&["switch", "--quiet", "n9-topic-two"]);
    r.conflict(r.git(&["rebase", "--merge", "n9-main"]), "base\n", "main\n", "topic-one\n");
    // Modern Git's merge backend itself writes rebase-merge/interactive, even
    // without -i. No editor or fabricated historical layout is needed.
    r.action(Some("rebase-interactive"));
    r.success(&["rebase", "--abort"]);
    r.clean("n9-topic-two", "topic-two\n");

    r.conflict(r.git(&["rebase", "--apply", "n9-main"]), "base\n", "main\n", "topic-one\n");
    r.action(Some("rebase"));
    r.success(&["rebase", "--abort"]);
    r.clean("n9-topic-two", "topic-two\n");
    r.success(&["switch", "--quiet", "n9-main"]);

    let patch = f.root.join("actual format-patch 雪.patch");
    fs::write(&patch, r.success(&["format-patch", "--stdout", "-1", "n9-topic-one"])).unwrap();
    let failed = r.git(&["am", "--no-3way", "--", patch.to_str().unwrap()]);
    assert_eq!(failed.code, 128, "expected patch applicability failure: stdout={} stderr={}",
        String::from_utf8_lossy(&failed.out), String::from_utf8_lossy(&failed.err));
    // The real mail patch requires base -> topic-one, but this worktree contains
    // main. Non-three-way am must leave that file and index untouched.
    assert_eq!(fs::read(&file).unwrap(), b"main\n");
    exact(r.git(&["status", "--porcelain=v1"]), 0, b"", b"");
    let current_patch = String::from_utf8(r.success(&["am", "--show-current-patch=diff"])).unwrap();
    assert!(current_patch.contains("diff --git a/conflict.txt b/conflict.txt\n"), "{current_patch}");
    assert!(current_patch.contains("-base\n+topic-one\n"), "{current_patch}");
    r.action(Some("apply"));
    r.success(&["am", "--abort"]);
    r.clean("n9-main", "main\n");
    // Not claimed: legacy rebase/, .dotest-merge/, marker-less rebase/apply,
    // or non-interactive rebase-merge layouts not produced by modern Git here.
}
