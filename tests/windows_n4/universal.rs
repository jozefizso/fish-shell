use super::fixture::{Fixture, exact, quote};
use super::native::Event;
use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt};

struct Release<'a>(&'a Event, &'a Event);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.signal();
        self.1.signal();
    }
}

fn exported(f: &Fixture, code: &str) -> String {
    let result = f.eval(&format!("{code}; {} report", f.probe()));
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(
        result.err.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.err)
    );
    String::from_utf8(result.out).unwrap()
}

pub(super) fn exercise_sharing(f: &Fixture) {
    exact(f.eval("set --universal n4_values alpha '' 雪; set -U n4_empty; set -U n4_single ''; set -Ux FISH_ACCEPT_UVAR one; set -U --path n4_paths 'C:\\alpha' 'D:\\beta'"), 0, b"", b"");
    exact(f.eval("printf '<%s>' $n4_values; printf '\\n'; count $n4_empty; echo empty-status:$status; count $n4_single; printf '%s\\n' \"$n4_paths\"; set -q -U n4_values; echo universal:$status"), 0,
        "<alpha><><雪>\n0\nempty-status:1\n1\nC:\\alpha;D:\\beta\nuniversal:0\n".as_bytes(), b"");
    assert!(
        exported(f, "true")
            .lines()
            .any(|line| line == "FISH_ACCEPT_UVAR=6f6e65")
    );
    let shadowed = exported(f, "set -l FISH_ACCEPT_UVAR private");
    assert!(
        !shadowed
            .lines()
            .any(|line| line.starts_with("FISH_ACCEPT_UVAR=")),
        "an unexported local must mask an exported universal in the actual child"
    );
    exact(
        f.eval("set -Uu FISH_ACCEPT_UVAR $FISH_ACCEPT_UVAR"),
        0,
        b"",
        b"",
    );
    assert!(
        !exported(f, "true")
            .lines()
            .any(|line| line.starts_with("FISH_ACCEPT_UVAR="))
    );
    exact(f.eval("set -Ux FISH_ACCEPT_UVAR two"), 0, b"", b"");
    assert!(
        exported(f, "true")
            .lines()
            .any(|line| line == "FISH_ACCEPT_UVAR=74776f")
    );

    // A nested native fish imports the parent's export as a global. Matching
    // inherited exports must not shadow its own subsequently updated universal.
    let child = format!(
        "set -Ux FISH_ACCEPT_UVAR after; printf 'observed:%s\\n' $FISH_ACCEPT_UVAR; {} report",
        f.probe()
    );
    let nested = f.eval(&format!(
        "set -Ux FISH_ACCEPT_UVAR before; {} -N -c {}",
        quote(f.fish.to_str().unwrap()),
        quote(&child)
    ));
    assert_eq!(nested.code, 0, "{}", String::from_utf8_lossy(&nested.err));
    assert!(
        nested.err.is_empty(),
        "{}",
        String::from_utf8_lossy(&nested.err)
    );
    let nested = String::from_utf8(nested.out).unwrap();
    assert!(nested.lines().any(|line| line == "observed:after"));
    assert!(
        nested
            .lines()
            .any(|line| line == "FISH_ACCEPT_UVAR=6166746572")
    );

    let payload = b"0123456789abcdefghijklmnop\n";
    exact(f.input("set -U fish_read_limit 3; read value; set -l result $status; set -eU fish_read_limit; exit $result", payload), 122, b"", b"");
    exact(
        f.input(
            "set -U fish_read_limit 64; read value; printf '%s\\n' $value; set -eU fish_read_limit",
            payload,
        ),
        0,
        payload,
        b"",
    );
    exact(
        f.input(
            "set -U fish_read_limit 3; set -eU fish_read_limit; read value; printf '%s\\n' $value",
            payload,
        ),
        0,
        payload,
        b"",
    );

    // Valid reserved Unicode and a raw byte must retain different provenance
    // through the real persisted format and a newly initialized shell.
    exact(
        f.eval("set -U n4_private '\u{f6ff}'; set -U n4_raw (printf '\\xff')"),
        0,
        b"",
        b"",
    );
    let mut expected = "\u{f6ff}\n".as_bytes().to_vec();
    expected.extend_from_slice(b"\xff\n");
    exact(
        f.eval("printf '%s\\n' $n4_private $n4_raw"),
        0,
        &expected,
        b"",
    );
    exact(
        f.eval("set -eU n4_single; set -q -U n4_single; echo $status"),
        0,
        b"1\n",
        b"",
    );
    exact(f.eval("set -q -U n4_single; echo $status"), 0, b"1\n", b"");

    exact(f.eval("set -U n4_conflict initial"), 0, b"", b"");
    let ready_a = Event::new("n4-uvars-writer-a");
    let ready_b = Event::new("n4-uvars-writer-b");
    let release_a = Event::new("n4-uvars-release-a");
    let release_b = Event::new("n4-uvars-release-b");
    std::thread::scope(|scope| {
        let _release = Release(&release_a, &release_b);
        let writer = |ready: &Event, release: &Event, own: &str, value: &str| {
            format!(
                "set -l old $n4_conflict; {} n4-barrier {} {}; set -U {own} {value}; set -U n4_conflict {value}; printf 'ack:%s' $n4_conflict",
                f.probe(),
                quote(&ready.name),
                quote(&release.name),
            )
        };
        let code_a = writer(&ready_a, &release_a, "n4_left", "left");
        let code_b = writer(&ready_b, &release_b, "n4_right", "right");
        let a = scope.spawn(move || f.eval(&code_a));
        let b = scope.spawn(move || f.eval(&code_b));
        ready_a.wait();
        ready_b.wait();
        release_a.signal();
        exact(a.join().unwrap(), 0, b"ack:left", b"");
        release_b.signal();
        exact(b.join().unwrap(), 0, b"ack:right", b"");
    });
    exact(
        f.eval("printf '%s\\n' $n4_left $n4_right $n4_conflict"),
        0,
        b"left\nright\nright\n",
        b"",
    );

    // Each independent reader waits for its actual value/handler effect, not a
    // sleep or an assumption that a writer's ack has run every threadpool callback.
    exact(f.eval("set -U n4_broadcast before"), 0, b"", b"");
    let ready_a = Event::new("n4-uvars-reader-a");
    let ready_b = Event::new("n4-uvars-reader-b");
    let release_a = Event::new("n4-uvars-reader-release-a");
    let release_b = Event::new("n4-uvars-reader-release-b");
    std::thread::scope(|scope| {
        let _release = Release(&release_a, &release_b);
        let reader = |ready: &Event, release: &Event, ack: &str| {
            format!(
                "function changed --on-variable n4_broadcast; set -U {ack} $n4_broadcast; end; {} n4-barrier {} {}; while test \"$n4_broadcast\" != after; true; end; printf '%s:%s' $n4_broadcast ${ack}",
                f.probe(),
                quote(&ready.name),
                quote(&release.name),
            )
        };
        let code_a = reader(&ready_a, &release_a, "n4_ack_left");
        let code_b = reader(&ready_b, &release_b, "n4_ack_right");
        let a = scope.spawn(move || f.eval(&code_a));
        let b = scope.spawn(move || f.eval(&code_b));
        ready_a.wait();
        ready_b.wait();
        exact(f.eval("set -U n4_broadcast after"), 0, b"", b"");
        release_a.signal();
        release_b.signal();
        exact(a.join().unwrap(), 0, b"after:after", b"");
        exact(b.join().unwrap(), 0, b"after:after", b"");
    });
    exact(
        f.eval("printf '%s\\n' $n4_ack_left $n4_ack_right"),
        0,
        b"after\nafter\n",
        b"",
    );
    retry_effects(f);
}

fn retry_effects(f: &Fixture) {
    struct ReleaseOne<'a>(&'a Event);
    impl Drop for ReleaseOne<'_> {
        fn drop(&mut self) {
            self.0.signal();
        }
    }
    let path = f.state.join("config/fish/fish_variables");
    for erase in [false, true] {
        exact(
            f.eval(if erase {
                "set -U fish_read_limit 4"
            } else {
                "set -U fish_read_limit 64"
            }),
            0,
            b"",
            b"",
        );
        let locked = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        let ready = Event::new("n4-deferred-effect-ready");
        let release = Event::new("n4-deferred-effect-release");
        let mutation = if erase {
            "set -eU fish_read_limit; test $status -ne 0; or exit 91"
        } else {
            "set -U fish_read_limit 3; test $status -ne 0; or exit 92; set -U fish_read_limit 4; test $status -ne 0; or exit 93"
        };
        let consumer = if erase {
            "read -l value; set -l result $status; printf '<%s>:%s\\n' \"$value\" $result; count $n4_retry_events"
        } else {
            "count $n4_retry_events; set -l value (printf 0123456789); printf 'capture:%s\\n' $status; set -eU fish_read_limit; count $n4_retry_events"
        };
        let code = format!(
            "function n4_retry --on-variable fish_read_limit; set -ga n4_retry_events changed; end; {mutation}; if set -q n4_retry_events; exit 94; end; {} n4-barrier {} {}; {consumer}",
            f.probe(),
            quote(&ready.name),
            quote(&release.name)
        );
        std::thread::scope(|scope| {
            let _release = ReleaseOne(&release);
            let operation = scope.spawn(move || f.input(&code, b"0123456789\n"));
            ready.wait();
            drop(locked);
            release.signal();
            let result = operation.join().unwrap();
            assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
            assert!(
                !result.err.is_empty(),
                "failed writes require real diagnostics"
            );
            assert_eq!(
                result.out,
                if erase {
                    b"<0123456789>:0\n1\n".as_slice()
                } else {
                    b"1\ncapture:122\n2\n".as_slice()
                },
                "successful retry must replay the latest consumer effect and exactly one deferred event"
            );
        });
    }
}

pub(super) fn exercise_recovery(f: &Fixture) {
    let directory = f.state.join("config/fish");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("fish_variables");
    fs::write(&path, b"SET_EXPORT legacy:old\\x1evalue\n").unwrap();
    exact(f.eval("printf '<%s>' $legacy; printf '\\n'; set -q -x legacy; echo $status; set -U modern added"), 0, b"<old><value>\n0\n", b"");
    exact(
        f.eval("printf '%s\\n' $legacy $modern"),
        0,
        b"old\nvalue\nadded\n",
        b"",
    );

    let future = b"# VERSION: 99.0\nSETUVAR future:kept\n";
    fs::write(&path, future).unwrap();
    let refused = f.eval("set -U refused value");
    assert_ne!(refused.code, 0);
    assert!(!refused.err.is_empty());
    assert_eq!(fs::read(&path).unwrap(), future);
    exact(f.eval("printf '%s\\n' $future"), 0, b"kept\n", b"");

    // An empty/truncated declaration and embedded NUL comment must not panic.
    fs::write(
        &path,
        b"# harmless\0comment\n# VERSION: 3.0\nSETUVAR --export\nSETUVAR stable:before\n",
    )
    .unwrap();
    let recovered = f.eval("printf '%s\\n' $stable");
    assert_eq!(recovered.code, 0);
    assert_eq!(recovered.out, b"before\n");
    assert!(
        !recovered.err.is_empty(),
        "a malformed declaration requires a diagnostic, not a panic"
    );
    fs::write(&path, b"# VERSION: 3.0\nSETUVAR stable:before\n").unwrap();
    let before = fs::read(&path).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(&path)
        .unwrap();
    let failed = f.eval("set -U stable unacknowledged");
    assert_ne!(failed.code, 0);
    assert!(!failed.err.is_empty());
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(locked);
    exact(
        f.eval("printf '%s\\n' $stable; set -U stable after"),
        0,
        b"before\n",
        b"",
    );
    exact(f.eval("printf '%s\\n' $stable"), 0, b"after\n", b"");

    // Escaping can make a valid in-memory value exceed the reader's limit.
    // The writer must reject before replacing the still-readable prior state.
    let before = fs::read(&path).unwrap();
    let oversized = f.eval("set -U n4_oversized (string repeat -n 4194304 ' ')");
    assert_ne!(
        oversized.code, 0,
        "an unreadable future snapshot was falsely acknowledged"
    );
    assert!(!oversized.err.is_empty());
    assert_eq!(fs::read(&path).unwrap(), before);
    exact(f.eval("printf '%s\\n' $stable"), 0, b"after\n", b"");

    let mut too_large = b"# VERSION: 3.0\nSETUVAR kept:prior\n".to_vec();
    too_large.resize(16 * 1024 * 1024 + 1, b' ');
    fs::write(&path, &too_large).unwrap();
    let failed = f.eval("set -U kept overwritten");
    assert_ne!(failed.code, 0);
    assert!(!failed.err.is_empty());
    assert_eq!(
        fs::read(&path).unwrap(),
        too_large,
        "an unread tail must never be acknowledged as a safely rewritten file"
    );
}
