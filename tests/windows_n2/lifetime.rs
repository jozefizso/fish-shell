use super::fixture::{Fixture, exact};
use std::fs;

pub fn exercise(f: &Fixture) {
    let p = f.probe();
    exact(f.eval(&format!("{p} n2-runtime")), 0, b"runtime=96;reset=1024;remove=256;recycled;cancelled;process=259;workers;debounce;drained\n", b"");
    // Distinct owned children launched concurrently must not retain one another's
    // writers, cwd, status or redirection targets. Every stream is concurrently
    // drained by the shared fixture and every launch has a bounded process wait.
    let barrier = std::sync::Barrier::new(13);
    let results = std::thread::scope(|scope| {
        let mut children = Vec::new();
        for index in 0..12 {
            let barrier = &barrier;
            let p = &p;
            children.push(scope.spawn(move || {
                barrier.wait();
                let result = f.eval(&format!(
                    "{p} n2-binary >concurrent-{index}; {p} exit {index}"
                ));
                (index, result)
            }));
        }
        barrier.wait();
        children
            .into_iter()
            .map(|child| child.join().unwrap())
            .collect::<Vec<_>>()
    });
    for (index, result) in results {
        exact(result, index, b"", b"");
        assert_eq!(
            fs::read(f.root.join(format!("concurrent-{index}"))).unwrap(),
            b"\0\xff\x80\r\nA\0B"
        );
        fs::remove_file(f.root.join(format!("concurrent-{index}"))).unwrap();
    }
    // A real invalid executable passes filesystem lookup, then fails native spawn.
    fs::write(f.root.join("invalid.exe"), b"MZ-invalid-native-image").unwrap();
    let result = f.eval("./invalid.exe");
    assert_ne!(result.code, 0);
    assert!(result.out.is_empty());
    assert!(!result.err.is_empty());
    fs::remove_file(f.root.join("invalid.exe")).unwrap();
    // Pending production capture and partial-launch rollback are exercised in the
    // isolated probe so their native callbacks never impersonate the harness main thread.
    exact(
        f.eval(&format!(
            "{p} n2-capture-lifetime {}",
            super::fixture::quote(f.fish.to_str().unwrap())
        )),
        0,
        b"capture=available;descendant-live;cancelled;drained;rollback;inheritance-clean\n",
        b"",
    );
}
