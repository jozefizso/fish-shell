//! Runs in an isolated real probe process: the harness thread is never fish's
//! registered evaluator thread. No sleeps, mock records or source-text checks.
use fish::threads::{NativeCompletionPayload, NativeRuntime};
use fish::universal_notifier::NativeDirectoryWatcher;
use std::ffi::{OsStr, c_void};
use std::fs;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn ReplaceFileW(
        target: *const u16,
        replacement: *const u16,
        backup: *const u16,
        flags: u32,
        exclude: *mut c_void,
        reserved: *mut c_void,
    ) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
}
fn handle_count() -> u32 {
    let mut result = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut result) },
        0
    );
    result
}
fn replace(source: &Path, target: &Path) {
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let source_wide = wide(source);
    let target_wide = wide(target);
    assert_ne!(
        unsafe {
            ReplaceFileW(
                target_wide.as_ptr(),
                source_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0,
        "atomic replacement {source:?} -> {target:?}: {}",
        io::Error::last_os_error()
    );
}
fn write(path: &Path, bytes: &[u8]) {
    let mut file = fs::File::create(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn wait_hint(runtime: &Arc<NativeRuntime>, watchers: &[&NativeDirectoryWatcher]) {
    let start = Instant::now();
    let limit = Duration::from_secs(15);
    let mut seen = vec![false; watchers.len()];
    while seen.iter().any(|value| !value) {
        runtime.drain();
        for (index, watcher) in watchers.iter().enumerate() {
            seen[index] |= watcher
                .check()
                .unwrap_or_else(|error| panic!("watcher check: {error}"));
        }
        if seen.iter().all(|value| *value) {
            break;
        }
        let remaining = limit
            .checked_sub(start.elapsed())
            .expect("directory invalidation deadline");
        assert_eq!(
            unsafe {
                WaitForSingleObject(
                    runtime.notification_handle().as_raw_handle(),
                    remaining.as_millis().min(u32::MAX as u128) as u32,
                )
            },
            0,
            "bounded real directory wakeup"
        );
    }
}
fn wait_hint_allow_recovery(runtime: &Arc<NativeRuntime>, watcher: &NativeDirectoryWatcher) {
    let start = Instant::now();
    let limit = Duration::from_secs(15);
    loop {
        runtime.drain();
        match watcher.check() {
            Ok(true) => return,
            Ok(false) => (),
            // Deleting the old parent can complete the pending read with an
            // actual OS error. That error is observed, never represented as a
            // successful read; the retained owner must subsequently recover.
            Err(error) => assert!(
                error.raw_os_error().is_some(),
                "unexpected watcher lifecycle error: {error}"
            ),
        }
        let remaining = limit
            .checked_sub(start.elapsed())
            .expect("parent recovery deadline");
        assert_eq!(
            unsafe {
                WaitForSingleObject(
                    runtime.notification_handle().as_raw_handle(),
                    remaining.as_millis() as u32,
                )
            },
            0
        );
    }
}
fn consume_initial(runtime: &Arc<NativeRuntime>, watcher: &NativeDirectoryWatcher) {
    assert!(watcher.check().unwrap());
    runtime.drain();
}

fn local(root: &Path) {
    let runtime = NativeRuntime::new().unwrap();
    let parent = root.join("notification state 雪");
    fs::create_dir(&parent).unwrap();
    let target = parent.join("fish_variables");
    write(&target, b"before-1");
    let first = NativeDirectoryWatcher::new(
        &parent,
        Some(OsStr::new("fish_variables").to_owned()),
        Arc::clone(&runtime),
    )
    .unwrap();
    let second = NativeDirectoryWatcher::new(
        &parent,
        Some(OsStr::new("fish_variables").to_owned()),
        Arc::clone(&runtime),
    )
    .unwrap();
    let history = NativeDirectoryWatcher::new(&parent, None, Arc::clone(&runtime)).unwrap();
    for watcher in [&first, &second, &history] {
        consume_initial(&runtime, watcher);
    }
    // A retained reader proves the watcher observes replacement of the name,
    // not just mutation of its original file object. Both readers must wake.
    let retained = fs::File::open(&target).unwrap();
    let replacement = parent.join("replacement");
    write(&replacement, b"after--2");
    replace(&replacement, &target);
    wait_hint(&runtime, &[&first, &second, &history]);
    assert_eq!(fs::read(&target).unwrap(), b"after--2");
    drop(retained);
    write(&target, b"equal--3");
    wait_hint(&runtime, &[&first, &second, &history]);
    assert_eq!(fs::read(&target).unwrap(), b"equal--3");

    // Real lost-detail/rescan case: suspend servicing/re-arming after an actual
    // completion, then exceed the documented 64 KiB notification capacity.
    // We do NOT claim a deterministic ERROR_NOTIFY_ENUM_DIR: Windows may report
    // zero bytes or coalesce records instead. The required guarantee is that a
    // subsequent full invalidation reloads the final real state on every reader.
    write(&target, b"burst--4");
    assert_eq!(
        unsafe { WaitForSingleObject(runtime.notification_handle().as_raw_handle(), 15_000) },
        0
    );
    for index in 0..512 {
        let name = format!("notification-overflow-{index:04}-{}", "x".repeat(120));
        fs::write(parent.join(name), b"x").unwrap();
    }
    write(&target, b"final--5");
    wait_hint(&runtime, &[&first, &second, &history]);
    assert_eq!(fs::read(&target).unwrap(), b"final--5");

    // Source isolation: check must not steal a queued record from any other
    // runtime client, even when servicing directory and debounce completions.
    let mut unrelated = runtime.register_source().unwrap();
    unrelated
        .poster()
        .post(NativeCompletionPayload::User(Box::new(73u32)));
    let _ = first.check().unwrap();
    let owned = runtime.drain();
    assert!(owned.into_iter().any(|record| record.registration == unrelated.id()
        && matches!(record.payload, NativeCompletionPayload::User(value) if value.downcast_ref::<u32>() == Some(&73))));
    unrelated.remove_and_drain().unwrap();

    let moved = root.join("old notification state");
    fs::rename(&parent, &moved).unwrap();
    fs::create_dir(&parent).unwrap();
    write(&target, b"newdir-6");
    for watcher in [&first, &second, &history] {
        wait_hint_allow_recovery(&runtime, watcher);
    }
    assert_eq!(fs::read(&target).unwrap(), b"newdir-6");
    write(&target, b"newdir-7");
    for watcher in [&first, &second, &history] {
        wait_hint_allow_recovery(&runtime, watcher);
    }
    assert_eq!(fs::read(&target).unwrap(), b"newdir-7");
    for watcher in [&first, &second, &history] {
        watcher.shutdown().unwrap();
        watcher.shutdown().unwrap();
        assert_eq!(
            watcher.check().unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
    }
    drop((first, second, history));
    runtime.drain();

    let absent = root.join("missing watch chain").join("nested state");
    let watcher =
        NativeDirectoryWatcher::new(&absent, Some("fish_variables".into()), Arc::clone(&runtime))
            .unwrap();
    consume_initial(&runtime, &watcher);
    fs::create_dir_all(&absent).unwrap();
    write(&absent.join("fish_variables"), b"created");
    wait_hint_allow_recovery(&runtime, &watcher);
    assert_eq!(fs::read(absent.join("fish_variables")).unwrap(), b"created");
    watcher.shutdown().unwrap();
    drop(watcher);
    runtime.drain();

    // Warm native threadpool infrastructure before measuring registration
    // ownership, then repeatedly cancel genuinely pending kernel directory I/O.
    let pending = root.join("pending shutdown");
    fs::create_dir(&pending).unwrap();
    {
        let watcher = NativeDirectoryWatcher::new(&pending, None, Arc::clone(&runtime)).unwrap();
        watcher.shutdown().unwrap();
    }
    let baseline = handle_count();
    for _ in 0..96 {
        let watcher = NativeDirectoryWatcher::new(&pending, None, Arc::clone(&runtime)).unwrap();
        watcher.shutdown().unwrap();
        drop(watcher);
    }
    runtime.drain();
    assert!(
        handle_count() <= baseline + 2,
        "native directory/timer handles leaked after cancel-and-drain"
    );
    fs::remove_dir(&pending).unwrap();
}

fn remote(remote_root: &Path) {
    assert!(
        remote_root.as_os_str().encode_wide().take(2).eq([92, 92]),
        "required remote root must be a real UNC path"
    );
    let parent = remote_root.join(format!("fish-n4-notifications-{}", std::process::id()));
    fs::create_dir(&parent).expect("required writable UNC fixture");
    let runtime = NativeRuntime::new().unwrap();
    let watcher =
        NativeDirectoryWatcher::new(&parent, Some("fish_variables".into()), Arc::clone(&runtime))
            .expect("required UNC directory notifications");
    consume_initial(&runtime, &watcher);
    let target = parent.join("fish_variables");
    write(&target, b"remote-a");
    wait_hint(&runtime, &[&watcher]);
    let replacement = parent.join("replace");
    write(&replacement, b"remote-b");
    replace(&replacement, &target);
    wait_hint(&runtime, &[&watcher]);
    assert_eq!(fs::read(&target).unwrap(), b"remote-b");
    watcher.shutdown().unwrap();
    drop(watcher);
    fs::remove_dir_all(&parent).unwrap();
}

pub fn run(root: &Path, remote_root: &Path) {
    fish::threads::init();
    local(root);
    remote(remote_root);
    println!(
        "notifications=replacement;same-size;all-readers;lost-detail-rescan;parent-rebind;missing-parent;source-isolation;UNC;cancel-drain"
    );
}
