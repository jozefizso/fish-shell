use super::fixture::{Captured, Fixture, exact, quote};
use std::fs;
struct RestoreWritable(std::path::PathBuf);
impl Drop for RestoreWritable {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::metadata(&self.0) {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            let _ = fs::set_permissions(&self.0, permissions);
        }
    }
}

fn fails(result: Captured) -> Vec<u8> {
    assert_ne!(result.code, 0, "redirection unexpectedly succeeded");
    assert!(
        result.out.is_empty(),
        "failed redirect launched command: {:?}",
        result.out
    );
    assert!(!result.err.is_empty(), "missing failure diagnostic");
    result.err
}
pub fn exercise(f: &Fixture) {
    let p = f.probe();
    let binary = b"\0\xff\x80\r\nA\0B";
    fs::write(f.root.join("input"), binary).unwrap();
    fs::write(f.root.join("empty"), b"").unwrap();
    exact(f.eval(&format!("{p} n2-copy <input")), 0, binary, b"");
    exact(f.eval(&format!("{p} n2-copy <?missing")), 0, b"", b"");
    exact(
        f.eval("printf first >output; printf second >>output"),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(f.root.join("output")).unwrap(), b"firstsecond");
    exact(f.eval("printf replaced >output"), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("output")).unwrap(), b"replaced");
    fails(f.eval("printf forbidden >?output"));
    assert_eq!(fs::read(f.root.join("output")).unwrap(), b"replaced");
    exact(f.eval("printf created >?created"), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("created")).unwrap(), b"created");
    exact(f.eval(&format!("{p} n2-both >merged 2>&1")), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("merged")).unwrap(), b"OUT\0ERR\xff");
    exact(
        f.eval(&format!("{p} n2-both 2>&1 >separate")),
        0,
        b"ERR\xff",
        b"",
    );
    assert_eq!(fs::read(f.root.join("separate")).unwrap(), b"OUT\0");
    exact(f.eval(&format!("{p} n2-both &>merge-short")), 0, b"", b"");
    assert_eq!(
        fs::read(f.root.join("merge-short")).unwrap(),
        b"OUT\0ERR\xff"
    );
    exact(f.eval("printf final >first >last"), 0, b"", b"");
    assert_eq!(fs::read(f.root.join("first")).unwrap(), b"");
    assert_eq!(fs::read(f.root.join("last")).unwrap(), b"final");
    exact(f.eval("printf self 1>&1"), 0, b"self", b"");
    exact(f.eval("printf alias 3>&1 1>&- 1>&3 3>&-"), 0, b"alias", b"");
    exact(
        f.eval(&format!("{p} n2-stdin-state <empty")),
        0,
        b"empty\n",
        b"",
    );
    exact(
        f.eval(&format!("{p} n2-stdin-state 0<&-")),
        0,
        b"closed\n",
        b"",
    );
    exact(
        f.eval(&format!("{p} n2-closed-output 1>&-")),
        0,
        b"",
        b"closed-output\n",
    );
    exact(f.eval("true 1>&-"), 0, b"", b"");
    exact(f.eval("printf ok 2>&-"), 0, b"ok", b"");
    exact(f.eval("printf discarded 1>&-"), 0, b"", b"");
    exact(
        f.eval("begin; printf one; printf two; end >block"),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(f.root.join("block")).unwrap(), b"onetwo");
    exact(
        f.eval(&format!(
            "{p} n2-both >/dev/null 2>/dev/null; {p} n2-copy </dev/null"
        )),
        0,
        b"",
        b"",
    );
    exact(
        f.eval(&format!("{p} n2-both >NUL 2>NUL; {p} n2-copy <NUL")),
        0,
        b"",
        b"",
    );
    fails(f.eval("printf forbidden >?NUL"));

    // Only this exact native fish image is a cooperating descriptor-bridge child.
    let fish = quote(f.fish.to_str().unwrap());
    exact(
        f.eval(&format!("{fish} -N -c {} 1>&-", quote("true"))),
        0,
        b"",
        b"",
    );
    exact(
        f.eval(&format!("{fish} -N -c {} 1>&-", quote("printf forbidden"))),
        1,
        b"",
        b"",
    );
    exact(
        f.eval(&format!("{fish} -N -c {} 2>&-", quote("printf ok"))),
        0,
        b"ok",
        b"",
    );
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3>extra",
            quote("printf bridge >&3")
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(f.root.join("extra")).unwrap(), b"bridge");
    fs::write(
        f.root.join("alias-lines"),
        b"firstdir/first\notherdir/second\n",
    )
    .unwrap();
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3<alias-lines 4<&3",
            quote("path basename -q <&3; path basename <&4")
        )),
        0,
        b"second\n",
        b"",
    );
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3>&1 1>&-",
            quote("printf retained >&3")
        )),
        0,
        b"retained",
        b"",
    );
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3<input",
            quote(&format!("{p} n2-copy <&3 3<&-"))
        )),
        0,
        binary,
        b"",
    );
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3>&1 4>&3 3>&-",
            quote("printf deduplicated >&4")
        )),
        0,
        b"deduplicated",
        b"",
    );
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3>old-extra 3>new-extra",
            quote("printf newest >&3")
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(f.root.join("old-extra")).unwrap(), b"");
    assert_eq!(fs::read(f.root.join("new-extra")).unwrap(), b"newest");
    exact(
        f.eval(&format!(
            "{fish} -N -c {} 3>metadata-extra",
            quote(&format!("{p} n2-private-env 3>&-"))
        )),
        0,
        b"private-metadata-absent\n",
        b"",
    );
    fails(f.eval(&format!("{p} marker unsupported-marker 3>unsupported")));
    assert!(!f.root.join("unsupported-marker").exists());
    fails(f.eval(&format!(
        "{p} marker rejected-upstream 3>pipeline-extra | {p} marker forbidden-downstream"
    )));
    fails(f.eval(&format!(
        "{p} marker forbidden-upstream | {p} marker rejected-downstream 3>pipeline-extra"
    )));
    fails(f.eval(&format!(
        "{p} n2-copy <missing-pipeline-input | {p} marker forbidden-open-downstream"
    )));
    for marker in [
        "rejected-upstream",
        "forbidden-downstream",
        "forbidden-upstream",
        "rejected-downstream",
        "forbidden-open-downstream",
    ] {
        assert!(
            !f.root.join(marker).exists(),
            "failed pipeline preparation launched {marker}"
        );
    }
    fs::create_dir(f.root.join("impostor")).unwrap();
    let impostor = f.root.join("impostor/fish.exe");
    fs::copy(&f.probe, &impostor).unwrap();
    fails(f.eval(&format!(
        "{} marker impostor-marker 3>impostor-extra",
        quote(impostor.to_str().unwrap())
    )));
    assert!(
        !f.root.join("impostor-marker").exists(),
        "basename impersonated actual executable identity"
    );

    let missing = fails(f.eval(&format!("{p} marker missing-marker <no-such-input")));
    let invalid = fails(f.eval(&format!("{p} marker invalid-marker 1>&987")));
    let overflow = fails(f.eval(&format!(
        "{p} marker overflow-marker 999999999999999999999999>overflow"
    )));
    fails(f.eval(&format!(
        "{p} marker partial-open-marker >partial-open <no-such-input"
    )));
    assert!(!f.root.join("partial-open-marker").exists());
    assert_eq!(fs::read(f.root.join("partial-open")).unwrap(), b"");
    fs::rename(f.root.join("partial-open"), f.root.join("released-open")).unwrap();
    fs::remove_file(f.root.join("released-open")).unwrap();
    fs::write(f.root.join("denied"), b"protected").unwrap();
    let mut permissions = fs::metadata(f.root.join("denied")).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(f.root.join("denied"), permissions).unwrap();
    let restore_writable = RestoreWritable(f.root.join("denied"));
    let denied = fails(f.eval(&format!("{p} marker denied-marker >denied")));
    drop(restore_writable);
    assert_eq!(fs::read(f.root.join("denied")).unwrap(), b"protected");
    assert_ne!(missing, invalid);
    assert_ne!(invalid, overflow);
    assert_ne!(missing, denied);
    for name in [
        "missing-marker",
        "invalid-marker",
        "overflow-marker",
        "denied-marker",
    ] {
        assert!(!f.root.join(name).exists(), "failure launched {name}");
    }
    // Reopen/rename/delete after failures: no planning/open handle may remain live.
    fs::rename(f.root.join("denied"), f.root.join("released")).unwrap();
    fs::remove_file(f.root.join("released")).unwrap();
    exact(
        f.eval(&format!("{p} n2-redirect-native")),
        0,
        b"native-open-errors-distinct;released\n",
        b"",
    );

    // Simultaneous CREATE_NEW contenders: exactly one wins and writes its complete
    // payload. This exercises atomic creation rather than a check-then-create path.
    let barrier = std::sync::Barrier::new(3);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            f.eval("printf winner-a >?race")
        });
        let second = scope.spawn(|| {
            barrier.wait();
            f.eval("printf winner-b >?race")
        });
        barrier.wait();
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.code == 0).count(), 1);
    let bytes = fs::read(f.root.join("race")).unwrap();
    assert!(bytes == b"winner-a" || bytes == b"winner-b");
    for result in results {
        if result.code == 0 {
            exact(result, 0, b"", b"");
        } else {
            fails(result);
        }
    }
    exact(f.eval(&format!("{p} n2-copy <?input")), 0, binary, b"");
    // The native owner probe checks handle release at a stable planner boundary.
    // Keep the shell-level side-effect assertion separate from transient child handles.
    let mut code = String::new();
    for _ in 0..32 {
        code.push_str(&format!("{p} marker leak-marker >leak-open <absent-input;"));
    }
    code.push_str("printf 'plans-released\\n'");
    let result = f.eval(&code);
    assert_eq!(result.code, 0);
    assert_eq!(result.out, b"plans-released\n");
    assert!(!result.err.is_empty());
    assert!(!f.root.join("leak-marker").exists());
    assert_eq!(fs::read(f.root.join("leak-open")).unwrap(), b"");
}
