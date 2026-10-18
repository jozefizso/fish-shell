use super::fixture::{Fixture, exact, quote};
use std::path::PathBuf;

pub fn exercise(f: &Fixture) {
    let remote = std::env::var_os("FISH_NATIVE_N1_UNC_ROOT")
        .map(PathBuf::from)
        .expect("N4-RECOVERY notifications require FISH_NATIVE_N1_UNC_ROOT (writable UNC fixture; never skipped)");
    assert!(
        remote.is_absolute(),
        "required UNC fixture must be absolute"
    );
    let root = f.root.join("notification acceptance");
    std::fs::create_dir(&root).unwrap();
    exact(
        f.eval(&format!("{} n4-notifications {} {}", f.probe(), quote(root.to_str().unwrap()), quote(remote.to_str().expect("UNC fixture Unicode path")))),
        0,
        b"notifications=replacement;same-size;all-readers;lost-detail-rescan;parent-rebind;missing-parent;source-isolation;UNC;cancel-drain\n",
        b"",
    );
    std::fs::remove_dir_all(root).unwrap();
}
