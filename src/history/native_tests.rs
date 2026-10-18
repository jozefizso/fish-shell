//! Private history transition tests use controlled timestamps, not sleeps or
//! another process's wall clock. Each case runs in its own native evaluator thread.
use super::*;
use std::{
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    process::{Command, Stdio},
    time::{Duration, UNIX_EPOCH},
};

fn isolated(name: &str) -> bool {
    let name = name
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    if std::env::var("FISH_NATIVE_HISTORY_UNIT_CHILD").as_deref() == Ok(name) {
        crate::threads::init();
        return true;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
    }
    let mut child = Command::new(crate::wutil::native_current_exe_path().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("FISH_NATIVE_HISTORY_UNIT_CHILD", name)
        .env_remove("RUST_BACKTRACE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = |mut stream: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let out = reader(Box::new(stdout));
    let err = reader(Box::new(stderr));
    let waited = unsafe { WaitForSingleObject(child.as_raw_handle(), 30_000) };
    if waited != 0 {
        let _ = child.kill();
    }
    let status = child.wait().unwrap();
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert_eq!(
        waited,
        0,
        "bounded native history child: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        status.success(),
        "native history child failed:\n{}\n{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        String::from_utf8_lossy(&stdout)
            .contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
        "the isolated harness did not execute exactly its required case"
    );
    false
}

fn history(directory: &std::path::Path) -> HistoryImpl {
    let mut history = HistoryImpl::new(
        L!("generation").to_owned(),
        Some(crate::path::native_path_text(directory).unwrap()),
    );
    history.load_old_if_needed();
    history
}

fn item(command: &wstr, seconds: u64) -> HistoryItem {
    let time = UNIX_EPOCH + Duration::from_secs(seconds);
    HistoryItem::new(
        command.to_owned(),
        Timestamps {
            first_added: time,
            last_added: time,
        },
        PersistenceMode::Disk,
    )
}

#[test]
fn native_clear_session_after_failed_duplicate_save() {
    if !isolated(concat!(
        module_path!(),
        "::native_clear_session_after_failed_duplicate_save"
    )) {
        return;
    }
    for nonconsecutive in [false, true] {
        let directory = fish_tempfile::new_dir().unwrap();
        let mut active = history(directory.path());
        active.add(item(L!("owned"), 10), false, false);
        active.save(false).unwrap();
        let path = directory.path().join("generation_history");
        let original = std::fs::read(&path).unwrap();
        let denied = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        if nonconsecutive {
            active.add(item(L!("between"), 15), false, false);
        }
        active.add(item(L!("owned"), 20), false, false);
        assert!(active.save(false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        drop(denied);
        active.clear_session();
        active.save(false).unwrap();
        assert!(
            history(directory.path()).get_history().is_empty(),
            "clear-session retained an earlier acknowledged version after the latest duplicate save failed"
        );
    }
}

#[test]
fn native_clear_session_does_not_resurrect_foreign_deletion() {
    if !isolated(concat!(
        module_path!(),
        "::native_clear_session_does_not_resurrect_foreign_deletion"
    )) {
        return;
    }
    let directory = fish_tempfile::new_dir().unwrap();
    let mut seed = history(directory.path());
    seed.add(item(L!("baseline"), 10), false, false);
    seed.save(false).unwrap();
    let mut active = history(directory.path());
    let mut foreign = history(directory.path());
    foreign.remove(L!("baseline"));
    foreign.save(false).unwrap();
    assert!(history(directory.path()).get_history().is_empty());
    assert_eq!(
        active.get_history(),
        [L!("baseline").to_owned()],
        "another shell must not silently replace our pinned read baseline"
    );
    active.add(item(L!("baseline"), 20), false, false);
    active.save(false).unwrap();
    active.clear_session();
    active.save(false).unwrap();
    assert!(
        history(directory.path()).get_history().is_empty(),
        "clearing a re-created command resurrected another session's acknowledged deletion"
    );
}

#[test]
fn native_clear_session_restores_foreign_predecessor() {
    if !isolated(concat!(
        module_path!(),
        "::native_clear_session_restores_foreign_predecessor"
    )) {
        return;
    }
    let directory = fish_tempfile::new_dir().unwrap();
    let mut seed = history(directory.path());
    seed.add(item(L!("shared"), 10), false, false);
    seed.save(false).unwrap();
    let mut active = history(directory.path());
    active.add(item(L!("shared"), 20), false, false);
    active.save(false).unwrap();
    let mut foreign = history(directory.path());
    foreign.add(item(L!("shared"), 30), false, false);
    foreign.save(false).unwrap();
    active.add(item(L!("shared"), 40), false, false);
    active.save(false).unwrap();
    active.clear_session();
    active.save(false).unwrap();
    let mut restarted = history(directory.path());
    let restored = restarted.item_at_index(1).unwrap();
    assert_eq!(restored.str(), L!("shared"));
    assert_eq!(
        restored.last_added_timestamp(),
        UNIX_EPOCH + Duration::from_secs(30),
        "undoing our repeat must restore the actual intervening foreign generation, not an obsolete baseline"
    );
}
