//! Native fixtures and ownership assertions. Shell semantics always run in actual fish.
use fish::threads::{NativeCancellation, NativeCompletionPayload, NativeRuntime};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString, c_void};
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
const LIMIT: Duration = Duration::from_secs(20);
const BLOCK: usize = 65536;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(sa: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn CreateMutexW(sa: *const c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn DuplicateHandle(
        source: *mut c_void,
        handle: *mut c_void,
        target: *mut c_void,
        duplicate: *mut *mut c_void,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn GetStdHandle(which: u32) -> *mut c_void;
    fn GetHandleInformation(handle: *mut c_void, flags: *mut u32) -> i32;
    fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
    fn ExitProcess(code: u32) -> !;
    fn SwitchToThread() -> i32;
    fn CompareObjectHandles(first: *mut c_void, second: *mut c_void) -> i32;
    fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
    fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
    fn WriteFile(
        handle: *mut c_void,
        bytes: *const u8,
        count: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
}
#[repr(C)]
#[derive(Default)]
struct MemoryCounters {
    cb: u32,
    faults: u32,
    peak_working: usize,
    working: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
}
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut MemoryCounters, size: u32) -> i32;
}
fn own(raw: *mut c_void) -> OwnedHandle {
    assert!(
        !raw.is_null() && raw as isize != -1,
        "native handle: {}",
        io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn event() -> Arc<OwnedHandle> {
    Arc::new(own(unsafe {
        CreateEventW(std::ptr::null(), 1, 0, std::ptr::null())
    }))
}
fn named_event(name: &OsStr) -> OwnedHandle {
    let wide: Vec<_> = name.encode_wide().chain(Some(0)).collect();
    own(unsafe { OpenEventW(0x00100002, 0, wide.as_ptr()) })
}
fn signal(handle: &impl AsRawHandle) {
    assert_ne!(unsafe { SetEvent(handle.as_raw_handle()) }, 0);
}
fn wait(handle: &impl AsRawHandle) {
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "bounded wait failed"
    );
}
fn duplicate(handle: &impl AsRawHandle) -> OwnedHandle {
    let mut result = std::ptr::null_mut();
    assert_ne!(
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                handle.as_raw_handle(),
                GetCurrentProcess(),
                &mut result,
                0,
                0,
                2,
            )
        },
        0
    );
    own(result)
}
fn pattern() -> Vec<u8> {
    (0..BLOCK)
        .map(|n| ((n * 31 + n / 251) % 256) as u8)
        .collect()
}
fn write_volume(size: usize) -> io::Result<()> {
    let block = pattern();
    let mut out = io::stdout().lock();
    for _ in 0..size / BLOCK {
        out.write_all(&block)?;
    }
    out.flush()
}
fn read_volume(size: usize) {
    let expected = pattern();
    let mut block = vec![0; BLOCK];
    let mut input = io::stdin().lock();
    for index in 0..size / BLOCK {
        input.read_exact(&mut block).unwrap();
        assert_eq!(block, expected, "binary block {index}");
        unsafe {
            SwitchToThread();
        }
    }
    assert_eq!(
        input.read(&mut block).unwrap(),
        0,
        "missing EOF or extra bytes"
    );
}
fn streaming_producer(name: &OsStr) {
    let ack = named_event(name);
    io::stdout()
        .write_all(&std::process::id().to_le_bytes())
        .unwrap();
    let stderr = std::thread::spawn(|| {
        let mut err = io::stderr().lock();
        for _ in 0..32 {
            err.write_all(&[0xa7; BLOCK]).unwrap();
        }
        err.flush().unwrap();
    });
    write_volume(32 * 1024 * 1024).unwrap();
    // Remain alive, retaining stdout, until the consumer verifies every byte.
    wait(&ack);
    stderr.join().unwrap();
}
fn streaming_consumer(args: &[OsString]) {
    let ack = named_event(&args[0]);
    let chunk_ready = named_event(&args[1]);
    let next_chunk = named_event(&args[2]);
    let mut pid = [0; 4];
    io::stdin().read_exact(&mut pid).unwrap();
    let producer = own(unsafe { OpenProcess(0x00100000, 0, u32::from_le_bytes(pid)) });
    let expected = pattern();
    let mut block = vec![0; BLOCK];
    for index in 0..512 {
        io::stdin().read_exact(&mut block).unwrap();
        assert_eq!(block, expected, "binary block {index}");
        signal(&chunk_ready);
        wait(&next_chunk);
    }
    assert_eq!(
        unsafe { WaitForSingleObject(producer.as_raw_handle(), 0) },
        258,
        "producer exited before acknowledgement"
    );
    signal(&ack);
    assert_eq!(
        io::stdin().read(&mut block).unwrap(),
        0,
        "writer ownership prevented EOF"
    );
    println!("binary=33554432;producer-alive;EOF");
}
fn memory_stream(fish: &OsStr) {
    let probe = std::env::current_exe().unwrap();
    let quote = |s: &str| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"));
    let p = quote(probe.to_str().unwrap());
    let mut child = Command::new(fish)
        .args([
            "-N",
            "-c",
            &format!("{p} n2-volume | {p} n2-volume-consume"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let reader = |mut stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let out = reader(Box::new(out));
    let err = reader(Box::new(err));
    if unsafe { WaitForSingleObject(child.as_raw_handle(), LIMIT.as_millis() as u32) } != 0 {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("bounded memory pipeline timed out");
    }
    let mut counters = MemoryCounters::default();
    counters.cb = std::mem::size_of::<MemoryCounters>() as u32;
    assert_ne!(
        unsafe {
            GetProcessMemoryInfo(
                child.as_raw_handle(),
                &mut counters,
                std::mem::size_of::<MemoryCounters>() as u32,
            )
        },
        0
    );
    assert!(
        counters.peak_working < 192 * 1024 * 1024,
        "external stream retained excessive memory: {}",
        counters.peak_working
    );
    assert!(child.wait().unwrap().success());
    assert_eq!(out.join().unwrap(), b"bounded-stream=268435456;EOF\n");
    assert_eq!(err.join().unwrap(), b"");
    println!("bounded-stream=268435456;EOF");
}
fn redirect_native() {
    use fish_widestring::{native_to_shell_text, osstr2wcstring};
    let root = std::env::current_dir().unwrap();
    let path = root.join("native-denied");
    std::fs::write(&path, b"protected").unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let denied = fish::wutil::native_open(&path, 0x40000000, false);
    let missing = fish::wutil::native_open(&root.join("absent-native"), 0x80000000, false);
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    let denied = denied.unwrap_err();
    let missing = missing.unwrap_err();
    assert_eq!(denied.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    assert_ne!(denied.raw_os_error(), missing.raw_os_error());
    // Open through the same native owner, then drop and delete the actual object.
    drop(fish::wutil::native_open(&path, 0x80000000, false).unwrap());
    let text = native_to_shell_text(&osstr2wcstring(&path).unwrap());
    assert!(fish::wutil::native_file_identity(&text, true).is_ok());
    std::fs::remove_file(path).unwrap();
    // Sample the actual planner owner without an observing child. A child can
    // query before its parent's ResumeThread handle has been closed, so live
    // shell snapshots from that child do not define a stable ownership boundary.
    let vars = fish::env::EnvStack::new();
    vars.initialize_native().unwrap();
    drop(fish::io::IoChain::new().child_bindings().unwrap());
    let partial = root.join("native-partial-plan");
    let specs = vec![
        fish::redirection::RedirectionSpec::new(
            3,
            fish::redirection::RedirectionMode::Overwrite,
            native_to_shell_text(&osstr2wcstring(&partial).unwrap()),
        ),
        fish::redirection::RedirectionSpec::new(
            4,
            fish::redirection::RedirectionMode::Fd,
            fish_widestring::L!("3").to_owned(),
        ),
        fish::redirection::RedirectionSpec::new(
            5,
            fish::redirection::RedirectionMode::Fd,
            fish_widestring::L!("987").to_owned(),
        ),
    ];
    let before = handle_count();
    for _ in 0..128 {
        assert!(fish::io::IoChain::new().with_specs(&specs, &vars).is_err());
    }
    assert_eq!(
        handle_count(),
        before,
        "partial-open aliases leaked planner-owned handles"
    );
    assert_eq!(std::fs::read(&partial).unwrap(), b"");
    std::fs::remove_file(partial).unwrap();
    println!("native-open-errors-distinct;released");
}

pub fn run(mode: &str, args: Vec<OsString>) {
    match mode {
        "n2-produce" => streaming_producer(&args[0]),
        "n2-consume" => streaming_consumer(&args),
        "n2-memory-stream" => memory_stream(&args[0]),
        "n2-volume" => {
            write_volume(256 * 1024 * 1024).unwrap();
        }
        "n2-volume-consume" => {
            read_volume(256 * 1024 * 1024);
            println!("bounded-stream=268435456;EOF");
        }
        "n2-binary" => io::stdout().write_all(b"\0\xff\x80\r\nA\0B").unwrap(),
        "n2-copy" => {
            io::copy(&mut io::stdin().lock(), &mut io::stdout().lock()).unwrap();
        }
        "n2-both" => {
            io::stdout().write_all(b"OUT\0").unwrap();
            io::stdout().flush().unwrap();
            io::stderr().write_all(b"ERR\xff").unwrap();
        }
        "n2-text-produce" => {
            for _ in 0..65536 {
                io::stdout()
                    .write_all(b"abcdefghijklmnopqrstuvwxyz012345")
                    .unwrap();
            }
            io::stdout().write_all(b"\n").unwrap();
        }
        "n2-lines-produce" => {
            let mut out = io::stdout().lock();
            for _ in 0..65536 {
                out.write_all(b"folder/abcdefghijklmnopqrstuvwx\n").unwrap();
            }
            out.flush().unwrap();
        }
        "n2-text-consume" => {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes).unwrap();
            let mut expected = b"abcdefghijklmnopqrstuvwxyz012345".repeat(65536);
            expected.push(b'\n');
            assert_eq!(bytes, expected);
            println!("text=2097152;EOF");
        }
        "n2-broken-produce" => {
            if write_volume(256 * 1024 * 1024).is_err() {
                unsafe { ExitProcess(41) }
            }
            panic!("downstream did not close");
        }
        "n2-early-consume" => {
            let mut byte = [0];
            io::stdin().read_exact(&mut byte).unwrap();
            assert_eq!(byte, [0]);
            unsafe { ExitProcess(23) }
        }
        "n2-stdin-state" => {
            let raw = unsafe { GetStdHandle(-10i32 as u32) };
            let mut flags = 0;
            if raw.is_null()
                || raw as isize == -1
                || unsafe { GetHandleInformation(raw, &mut flags) } == 0
            {
                println!("closed");
            } else {
                let mut byte = [0];
                assert_eq!(io::stdin().read(&mut byte).unwrap(), 0);
                println!("empty");
            }
        }
        "n2-closed-output" => {
            let mut written = 0;
            assert_eq!(
                unsafe {
                    WriteFile(
                        GetStdHandle(-11i32 as u32),
                        b"x".as_ptr(),
                        1,
                        &mut written,
                        std::ptr::null_mut(),
                    )
                },
                0
            );
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(6));
            assert_eq!(written, 0);
            io::stderr().write_all(b"closed-output\n").unwrap();
        }
        "n2-private-env" => {
            assert!(
                !std::env::vars_os().any(|(name, _)| name
                    .to_string_lossy()
                    .eq_ignore_ascii_case("FISH_INTERNAL_NATIVE_FDS")),
                "private bridge metadata leaked to grandchild"
            );
            println!("private-metadata-absent");
        }
        "n2-redirect-native" => redirect_native(),
        "n2-runtime" => runtime(),
        "n2-capture-lifetime" => capture_lifetime(&args[0]),
        "n2-capture-descendant" => {
            let ready = named_event(&args[0]);
            let release = named_event(&args[1]);
            io::stdout().write_all(b"available:\0\xff\r\n").unwrap();
            io::stdout().flush().unwrap();
            signal(&ready);
            wait(&release);
        }
        "n2-descendant-root" => {
            let ready = named_event(&args[0]);
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("n2-capture-descendant")
                .arg(&args[0])
                .arg(&args[1])
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            std::fs::write(&args[2], child.id().to_string()).unwrap();
            if let Some(name) = args.get(3) {
                signal(&named_event(name));
            }
            if let Some(name) = args.get(4) {
                wait(&named_event(name));
            }
            wait(&ready);
            // Dropping std Child releases only this root's process handle. The
            // acceptance harness opens its own handle and releases/reaps the child.
            drop(child);
        }
        "n2-rollback-child" => {
            let ready = named_event(&args[0]);
            let release = named_event(&args[1]);
            signal(&ready);
            wait(&release);
            std::fs::write(&args[2], b"should-not-run").unwrap();
        }
        "n2-inheritance" => inheritance(&args),
        "n2-owned-descendant" => {
            let ready = named_event(&args[0]);
            let release = named_event(&args[1]);
            signal(&ready);
            wait(&release);
        }
        mode => panic!("unknown N2 mode {mode}"),
    }
}

fn handle_count() -> u32 {
    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0
    );
    count
}
fn collect(runtime: &NativeRuntime, count: usize) -> Vec<fish::threads::NativeCompletion> {
    let deadline = Instant::now() + LIMIT;
    let mut completions = Vec::new();
    while completions.len() < count {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("completion deadline expired");
        completions.extend(runtime.wait(Some(remaining), None).unwrap());
    }
    assert_eq!(completions.len(), count, "duplicate native completion");
    completions
}
fn runtime() {
    fish::threads::init();
    let baseline = handle_count();
    {
        let runtime = NativeRuntime::new().unwrap();
        assert!(runtime.drain().is_empty());
        assert_eq!(
            unsafe { WaitForSingleObject(runtime.notification_handle().as_raw_handle(), 0) },
            258
        );
        // More real simultaneous kernel registrations than one Win32 wait array.
        let mut registrations = Vec::new();
        let mut handles = Vec::new();
        for _ in 0..96 {
            let handle = event();
            registrations.push(runtime.register_wait(Arc::clone(&handle)).unwrap());
            handles.push(handle);
        }
        let expected: HashSet<_> = registrations.iter().map(|r| r.id()).collect();
        assert_eq!(expected.len(), 96);
        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            for handle in handles {
                signal(handle.as_ref());
            }
        });
        barrier.wait();
        let completions = collect(&runtime, 96);
        worker.join().unwrap();
        let mut received = HashSet::new();
        for completion in completions {
            assert!(matches!(completion.payload, NativeCompletionPayload::Ready));
            assert!(received.insert(completion.registration));
        }
        assert_eq!(received, expected);
        for registration in &mut registrations {
            assert!(registration.remove_and_drain().unwrap().is_empty());
        }
        drop(registrations);

        // Source posters are real client completion owners. Concurrent wait/reset
        // must deliver every unique owned payload exactly once, without lost wakeups.
        let mut source = runtime.register_source().unwrap();
        let id = source.id();
        let barrier = Arc::new(Barrier::new(5));
        let mut workers = Vec::new();
        for lane in 0usize..4 {
            let poster = source.poster();
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                for item in 0usize..256 {
                    assert!(
                        poster.post(NativeCompletionPayload::User(Box::new(lane * 256 + item)))
                    );
                }
            }));
        }
        barrier.wait();
        let completions = collect(&runtime, 1024);
        for worker in workers {
            worker.join().unwrap();
        }
        let mut received = HashSet::new();
        for completion in completions {
            assert_eq!(completion.registration, id);
            let NativeCompletionPayload::User(payload) = completion.payload else {
                panic!("wrong completion payload");
            };
            assert!(received.insert(*payload.downcast::<usize>().unwrap()));
        }
        assert_eq!(received, (0usize..1024).collect());
        assert!(source.remove_and_drain().unwrap().is_empty());
        assert_eq!(
            unsafe { WaitForSingleObject(runtime.notification_handle().as_raw_handle(), 0) },
            258
        );

        // Removal races an in-flight producer, not a timer. All accepted posts
        // appear in either the removed source's drained batch or the runtime queue.
        for _ in 0..128 {
            let mut source = runtime.register_source().unwrap();
            let id = source.id();
            let poster = source.poster();
            let late = poster.clone();
            let barrier = Arc::new(Barrier::new(2));
            let other = Arc::clone(&barrier);
            let worker = std::thread::spawn(move || {
                assert!(poster.post(NativeCompletionPayload::User(Box::new(0usize))));
                other.wait();
                poster.post(NativeCompletionPayload::User(Box::new(1usize)))
            });
            barrier.wait();
            let mut completions = source.remove_and_drain().unwrap();
            let accepted = worker.join().unwrap();
            completions.extend(runtime.drain());
            assert_eq!(completions.len(), 1 + usize::from(accepted));
            let mut values = HashSet::new();
            for completion in completions {
                assert_eq!(completion.registration, id);
                let NativeCompletionPayload::User(value) = completion.payload else {
                    panic!("wrong removal payload");
                };
                assert!(values.insert(*value.downcast::<usize>().unwrap()));
            }
            assert!(values.contains(&0));
            assert_eq!(values.contains(&1), accepted);
            assert!(
                !late.post(NativeCompletionPayload::Ready),
                "closed source accepted late completion"
            );
        }

        // Stop and drain a callback racing a real SetEvent. The registration owns
        // the final handle reference until kernel users and queued records drain.
        for _ in 0..128 {
            let handle = event();
            let weak = Arc::downgrade(&handle);
            let mut registration = runtime.register_wait(Arc::clone(&handle)).unwrap();
            let id = registration.id();
            let barrier = Arc::new(Barrier::new(2));
            let other = Arc::clone(&barrier);
            let worker = std::thread::spawn(move || {
                other.wait();
                signal(handle.as_ref());
            });
            barrier.wait();
            let mut records = registration.remove_and_drain().unwrap();
            worker.join().unwrap();
            records.extend(runtime.drain());
            assert!(records.len() <= 1);
            for record in records {
                assert_eq!(record.registration, id);
                assert!(matches!(record.payload, NativeCompletionPayload::Ready));
            }
            drop(registration);
            assert!(
                weak.upgrade().is_none(),
                "removed wait retained resource owner"
            );
            assert!(runtime.drain().is_empty());
        }

        // Exercise actual numeric HANDLE reuse, not a synthetic generation model.
        let old = event();
        let raw = old.as_raw_handle() as usize;
        let mut registration = runtime.register_wait(Arc::clone(&old)).unwrap();
        let old_id = registration.id();
        signal(old.as_ref());
        // Race remove against a signalled native callback before recycling the
        // actual numeric handle. Old records stay owned across the replacement.
        let mut old_records = registration.remove_and_drain().unwrap();
        old_records.extend(runtime.drain());
        assert!(old_records.len() <= 1);
        for record in &old_records {
            assert_eq!(record.registration, old_id);
        }
        drop(registration);
        drop(old);
        let deadline = Instant::now() + LIMIT;
        let recycled = loop {
            let candidate = event();
            if candidate.as_raw_handle() as usize == raw {
                break candidate;
            }
            assert!(
                Instant::now() < deadline,
                "required real HANDLE recycling was unavailable"
            );
        };
        let mut replacement = runtime.register_wait(Arc::clone(&recycled)).unwrap();
        assert_ne!(replacement.id(), old_id);
        for record in &old_records {
            assert_ne!(record.registration, replacement.id());
        }
        assert!(
            runtime.drain().is_empty(),
            "old record delivered to recycled owner"
        );
        signal(recycled.as_ref());
        let records = collect(&runtime, 1);
        assert_eq!(records[0].registration, replacement.id());
        assert!(matches!(records[0].payload, NativeCompletionPayload::Ready));
        replacement.remove_and_drain().unwrap();
        drop(replacement);
        drop(recycled);

        let cancellation = NativeCancellation::new().unwrap();
        assert!(!cancellation.is_cancelled());
        assert_eq!(
            unsafe { WaitForSingleObject(cancellation.handle().as_raw_handle(), 0) },
            258
        );
        cancellation.cancel();
        assert!(cancellation.is_cancelled());
        assert_eq!(
            unsafe { WaitForSingleObject(cancellation.handle().as_raw_handle(), 0) },
            0
        );
        let error = runtime.wait(Some(LIMIT), Some(&cancellation)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(runtime.drain().is_empty());

        // Validate real native registration failure and keep its native error,
        // then prove the same runtime still accepts a valid source.
        let invalid = event();
        let weak = Arc::downgrade(&invalid);
        let error = match runtime.register_process(std::process::id(), invalid) {
            Ok(_) => panic!("event registered as process"),
            Err(error) => error,
        };
        assert_eq!(error.raw_os_error(), Some(6));
        assert!(
            weak.upgrade().is_none(),
            "failed registration retained its handle"
        );
        // Threadpool waits cannot own mutexes. Reject the real Mutant before
        // arming, rather than allowing a callback-time native exception.
        let before = handle_count();
        let mutex = Arc::new(own(unsafe {
            CreateMutexW(std::ptr::null(), 0, std::ptr::null())
        }));
        let weak = Arc::downgrade(&mutex);
        let error = match runtime.register_wait(mutex) {
            Ok(_) => panic!("mutex accepted by threadpool wait"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            weak.upgrade().is_none(),
            "rejected mutex registration retained its handle"
        );
        assert_eq!(handle_count(), before);
        assert!(
            runtime.drain().is_empty(),
            "rejected mutex produced a completion"
        );
        let mut source = runtime.register_source().unwrap();
        assert!(source.poster().post(NativeCompletionPayload::Ready));
        assert_eq!(collect(&runtime, 1)[0].registration, source.id());
        source.remove_and_drain().unwrap();

        // 259 is a legitimate immutable completed DWORD status, not STILL_ACTIVE
        // once the retained process handle has actually signalled.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["exit", "259"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let handle = Arc::new(duplicate(&child));
        let weak = Arc::downgrade(&handle);
        let mut process = runtime
            .register_process(child.id(), Arc::clone(&handle))
            .unwrap();
        drop(handle);
        wait(&child);
        let pid = child.id();
        assert_eq!(child.wait().unwrap().code().unwrap() as u32, 259);
        drop(child);
        assert!(
            weak.upgrade().is_some(),
            "language status consumed process ownership"
        );
        let records = collect(&runtime, 1);
        assert_eq!(records[0].registration, process.id());
        match records[0].payload {
            NativeCompletionPayload::ProcessExit {
                pid: actual,
                raw_status,
            } => {
                assert_eq!(actual, pid);
                assert_eq!(raw_status, 259);
            }
            _ => panic!("missing process completion"),
        }
        process.remove_and_drain().unwrap();
        drop(process);
        assert!(weak.upgrade().is_none());
        workers_and_debounce(&runtime);
    }
    // Threadpool internals may retain a fixed process-global bootstrap allocation;
    // they may not retain one handle per each of the hundreds of removed owners.
    assert!(
        handle_count() <= baseline + 16,
        "native ownership leaked handles"
    );
    println!(
        "runtime=96;reset=1024;remove=256;recycled;cancelled;process=259;workers;debounce;drained"
    );
}

fn inheritance(args: &[OsString]) {
    let handle: usize = args[0].to_str().unwrap().parse().unwrap();
    let mut flags = 0;
    let sentinel = named_event(&args[1]);
    if unsafe { GetHandleInformation(handle as *mut c_void, &mut flags) } != 0 {
        assert_eq!(
            unsafe { CompareObjectHandles(handle as *mut c_void, sentinel.as_raw_handle()) },
            0,
            "unrelated inheritable handle reached child"
        );
    }
    println!("sentinel-absent");
}

fn workers_and_debounce(runtime: &Arc<NativeRuntime>) {
    use fish::threads::{Debounce, ThreadPool};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let pool = ThreadPool::new(2, 2);
    let mut source = runtime.register_source().unwrap();
    let source_id = source.id();
    let release = event();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started, receive) = std::sync::mpsc::channel();
    for index in 0usize..8 {
        let release = Arc::clone(&release);
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        let started = started.clone();
        pool.perform_with_completion(source.poster(), move || {
            fish::threads::assert_is_background_thread();
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(count, Ordering::SeqCst);
            started.send(index).unwrap();
            wait(release.as_ref());
            active.fetch_sub(1, Ordering::SeqCst);
            index
        })
        .unwrap();
    }
    receive.recv_timeout(LIMIT).unwrap();
    receive.recv_timeout(LIMIT).unwrap();
    assert_eq!(active.load(Ordering::SeqCst), 2);
    assert!(
        runtime.drain().is_empty(),
        "blocked workers fabricated completed results"
    );
    signal(release.as_ref());
    let deadline = Instant::now() + LIMIT;
    let mut values = HashSet::new();
    while values.len() != 8 {
        let remaining = deadline.checked_duration_since(Instant::now()).unwrap();
        assert_eq!(
            unsafe {
                WaitForSingleObject(
                    runtime.notification_handle().as_raw_handle(),
                    remaining.as_millis().min(u32::MAX as u128) as u32,
                )
            },
            0
        );
        runtime.service_main(|completion| {
            fish::threads::assert_is_main_thread();
            assert_eq!(completion.registration, source_id);
            let NativeCompletionPayload::User(payload) = completion.payload else {
                panic!("worker result was not owned");
            };
            assert!(values.insert(*payload.downcast::<usize>().unwrap()));
        });
    }
    assert_eq!(values, (0usize..8).collect());
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(
        peak.load(Ordering::SeqCst),
        2,
        "worker scheduling exceeded configured bound"
    );
    source.remove_and_drain().unwrap();

    let mut debounce = Debounce::<usize>::new(&pool, runtime, Duration::ZERO).unwrap();
    let release = event();
    let (started, receive) = std::sync::mpsc::channel();
    let handler_release = Arc::clone(&release);
    debounce.perform(move || {
        fish::threads::assert_is_background_thread();
        started.send(()).unwrap();
        wait(handler_release.as_ref());
        1
    });
    receive.recv_timeout(LIMIT).unwrap();
    let obsolete = Arc::new(AtomicUsize::new(0));
    let obsolete_task = Arc::clone(&obsolete);
    debounce.perform(move || {
        obsolete_task.fetch_add(1, Ordering::SeqCst);
        2
    });
    debounce.perform(|| 3);
    signal(release.as_ref());
    let deadline = Instant::now() + LIMIT;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("latest debounce result lost");
        match debounce.take_result_with_timeout(remaining) {
            Some(3) => break,
            Some(1) => {}
            result => panic!("unexpected debounce result {result:?}"),
        }
    }
    assert_eq!(
        obsolete.load(Ordering::SeqCst),
        0,
        "replaced debounce task ran"
    );
    assert!(debounce.take_result().is_none());

    drop(debounce);
    assert!(runtime.drain().is_empty());

    // Cancellation invalidates a running request, not just its queued successor.
    // A and B occupy a fresh two-worker pool. Only A's worker can execute the
    // queued sentinel while B remains blocked, proving A's publication phase ended.
    let cancel_pool = ThreadPool::new(2, 2);
    let mut debounce = Debounce::<usize>::new(&cancel_pool, runtime, Duration::ZERO).unwrap();
    let release_a = event();
    let release_b = event();
    let (started_a, receive_a) = std::sync::mpsc::channel();
    let handler_release = Arc::clone(&release_a);
    debounce.perform(move || {
        started_a.send(()).unwrap();
        wait(handler_release.as_ref());
        4
    });
    receive_a.recv_timeout(LIMIT).unwrap();
    let cancelled = Arc::new(AtomicUsize::new(0));
    let cancelled_task = Arc::clone(&cancelled);
    debounce.perform(move || {
        cancelled_task.fetch_add(1, Ordering::SeqCst);
        5
    });
    debounce.cancel_pending();
    let (started_b, receive_b) = std::sync::mpsc::channel();
    let handler_release = Arc::clone(&release_b);
    debounce.perform(move || {
        started_b.send(()).unwrap();
        wait(handler_release.as_ref());
        6
    });
    receive_b.recv_timeout(LIMIT).unwrap();
    let sentinel = event();
    let worker_sentinel = Arc::clone(&sentinel);
    cancel_pool
        .try_perform(move || signal(worker_sentinel.as_ref()))
        .unwrap();
    signal(release_a.as_ref());
    wait(sentinel.as_ref());
    assert_eq!(
        unsafe { WaitForSingleObject(release_b.as_raw_handle(), 0) },
        258
    );
    assert_eq!(
        cancelled.load(Ordering::SeqCst),
        0,
        "cancelled pending task ran"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(debounce.notification_handle().as_raw_handle(), 0) },
        258,
        "cancelled worker published stale readiness"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(runtime.notification_handle().as_raw_handle(), 0) },
        258
    );
    assert!(runtime.drain().is_empty());
    assert_eq!(
        debounce.take_result(),
        None,
        "cancelled running request published a result"
    );
    signal(release_b.as_ref());
    assert_eq!(debounce.take_result_with_timeout(LIMIT), Some(6));
    assert!(debounce.take_result().is_none());
    drop(debounce);
    assert!(runtime.drain().is_empty());

    // Exercise actual positive timeout abandonment, then use the same sentinel
    // proof to wait for A's complete run_next phase while replacement B is blocked.
    let timeout_pool = ThreadPool::new(2, 2);
    let timeout = Duration::from_millis(10);
    let mut debounce = Debounce::<usize>::new(&timeout_pool, runtime, timeout).unwrap();
    let release_a = event();
    let release_b = event();
    let (started_a, receive_a) = std::sync::mpsc::channel();
    let handler_release = Arc::clone(&release_a);
    let token_a = debounce.perform(move || {
        started_a.send(()).unwrap();
        wait(handler_release.as_ref());
        101
    });
    receive_a.recv_timeout(LIMIT).unwrap();
    let timer_wait = event();
    let started = Instant::now();
    while started.elapsed() <= timeout {
        assert_eq!(
            unsafe { WaitForSingleObject(timer_wait.as_raw_handle(), 2) },
            258
        );
    }
    let (started_b, receive_b) = std::sync::mpsc::channel();
    let handler_release = Arc::clone(&release_b);
    let token_b = debounce.perform(move || {
        started_b.send(()).unwrap();
        wait(handler_release.as_ref());
        202
    });
    assert_ne!(token_a, token_b, "positive timeout did not abandon A");
    receive_b.recv_timeout(LIMIT).unwrap();
    let sentinel = event();
    let worker_sentinel = Arc::clone(&sentinel);
    timeout_pool
        .try_perform(move || signal(worker_sentinel.as_ref()))
        .unwrap();
    signal(release_a.as_ref());
    wait(sentinel.as_ref());
    assert_eq!(
        unsafe { WaitForSingleObject(release_b.as_raw_handle(), 0) },
        258
    );
    assert_eq!(
        unsafe { WaitForSingleObject(debounce.notification_handle().as_raw_handle(), 0) },
        258,
        "abandoned worker published stale readiness"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(runtime.notification_handle().as_raw_handle(), 0) },
        258
    );
    assert!(runtime.drain().is_empty());
    assert_eq!(
        debounce.take_result(),
        None,
        "abandoned A replaced pending B's result"
    );
    signal(release_b.as_ref());
    assert_eq!(debounce.take_result_with_timeout(LIMIT), Some(202));
    assert!(debounce.take_result().is_none());
    drop(debounce);
    assert!(runtime.drain().is_empty());
}

struct NamedEvent {
    name: OsString,
    handle: OwnedHandle,
}
impl NamedEvent {
    fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = OsString::from(format!(
            "Local\\fish-n2-probe-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let wide: Vec<_> = name.encode_wide().chain(Some(0)).collect();
        let handle = own(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) });
        Self { name, handle }
    }
}
struct OwnedProcess(std::process::Child);
impl OwnedProcess {
    fn reap(&mut self) {
        wait(&self.0);
        assert!(self.0.wait().unwrap().success());
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
struct OwnedNativeProcess(OwnedHandle);
impl OwnedNativeProcess {
    fn assert_live(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) },
            258,
            "descendant exited before capture boundary"
        );
    }
    fn reap(&self) {
        wait(&self.0);
        let mut code = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &mut code) },
            0
        );
        assert_eq!(code, 0);
    }
}
impl Drop for OwnedNativeProcess {
    fn drop(&mut self) {
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } == 258 {
            unsafe {
                TerminateProcess(self.0.as_raw_handle(), 97);
            }
            wait(&self.0);
        }
    }
}
fn capture_lifetime(fish: &OsStr) {
    use fish::exec::NativeLaunchGroup;
    use fish::io::{IoChain, NativeCapture, SeparatedBuffer, SeparationType};
    fish::threads::init();
    let runtime = NativeRuntime::new().unwrap();
    let probe = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let environment: Vec<_> = std::env::vars_os().collect();

    // A prepared image must resist replacement before CreateProcess, even when
    // it has never run. Attribute-only handles do not enforce this exclusion.
    let candidate = cwd.join("prepared-image.exe");
    let renamed = cwd.join("released-image.exe");
    let marker = cwd.join("unstarted-prepared-image.marker");
    std::fs::copy(&probe, &candidate).unwrap();
    let group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let argv = vec![
        candidate.as_os_str().to_owned(),
        "marker".into(),
        marker.as_os_str().to_owned(),
    ];
    let plan = group
        .prepare(
            &candidate,
            &argv,
            &cwd,
            &environment,
            &IoChain::new(),
            false,
        )
        .unwrap();
    assert!(
        std::fs::rename(&candidate, &renamed).is_err(),
        "prepared executable remained replaceable"
    );
    assert!(!marker.exists(), "prepare started a child");
    drop(plan);
    std::fs::rename(&candidate, &renamed).unwrap();
    group.finish().unwrap();
    assert!(!marker.exists());
    std::fs::remove_file(renamed).unwrap();

    // Test the shared production separated buffer, including explicit empties,
    // inferred merging, binary provenance, exact limit and reset-after-discard.
    let mut buffer = SeparatedBuffer::new(4);
    assert!(buffer.append(b"", SeparationType::Explicitly));
    assert!(buffer.append(b"\0\xff", SeparationType::Inferred));
    assert!(buffer.append(b"ab", SeparationType::Inferred));
    assert_eq!(buffer.len(), 4);
    assert_eq!(buffer.elements().len(), 2);
    assert!(buffer.elements()[0].is_explicitly_separated());
    assert!(buffer.elements()[0].contents.is_empty());
    assert_eq!(buffer.elements()[1].contents, b"\0\xffab");
    assert_eq!(buffer.newline_serialized(), b"\n\0\xffab");
    assert!(!buffer.append(b"x", SeparationType::Inferred));
    assert!(buffer.discarded());
    assert_eq!(buffer.len(), 0);
    assert!(buffer.elements().is_empty());
    buffer.clear();
    assert!(!buffer.discarded());
    assert!(buffer.append(b"new", SeparationType::Explicitly));
    assert_eq!(buffer.newline_serialized(), b"new\n");

    for interrupted in [false, true] {
        let capture = NativeCapture::new(4096, Arc::clone(&runtime)).unwrap();
        assert!(capture.buffer().append(b"", SeparationType::Explicitly));
        let ready = NamedEvent::new("capture-ready");
        let release = NamedEvent::new("capture-release");
        let pid_file = cwd.join(format!("capture-descendant-{interrupted}.pid"));
        let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
        let argv = vec![
            probe.as_os_str().to_owned(),
            "n2-descendant-root".into(),
            ready.name.clone(),
            release.name.clone(),
            pid_file.as_os_str().to_owned(),
        ];
        let root = group
            .spawn(
                &probe,
                &argv,
                &cwd,
                &environment,
                &capture.io_chain(),
                false,
            )
            .unwrap();
        assert_eq!(root.wait(None).unwrap(), 0);
        let pid = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
        let child = OwnedNativeProcess(own(unsafe { OpenProcess(0x00101001, 0, pid) }));
        assert_eq!(group.wait(None).unwrap(), vec![0]);
        group.finish().unwrap();
        child.assert_live();
        if interrupted {
            capture.cancel();
        }
        let started = Instant::now();
        let result = capture.finish().unwrap();
        assert!(
            started.elapsed() < LIMIT / 2,
            "capture waited for descendant EOF"
        );
        child.assert_live();
        assert_eq!(result.interrupted, interrupted);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(!result.buffer.discarded());
        assert_eq!(result.buffer.elements().len(), 2);
        assert!(result.buffer.elements()[0].is_explicitly_separated());
        assert!(result.buffer.elements()[0].contents.is_empty());
        assert_eq!(
            result.buffer.elements()[1].contents,
            b"available:\0\xff\r\n"
        );
        assert!(!result.buffer.elements()[1].is_explicitly_separated());
        assert!(
            capture.finish().is_err(),
            "capture finalized the same ownership twice"
        );
        signal(&release.handle);
        child.reap();
        std::fs::remove_file(pid_file).unwrap();
        drop(root);
        drop(capture);
        assert!(
            runtime.drain().is_empty(),
            "capture left late owned completion records"
        );
    }

    let capture = NativeCapture::new(1024, Arc::clone(&runtime)).unwrap();
    let mut child = OwnedProcess(
        Command::new(&probe)
            .arg("n2-text-produce")
            .current_dir(&cwd)
            .env_clear()
            .envs(environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::from(capture.writer_file().unwrap()))
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    child.reap();
    let result = capture.finish().unwrap();
    assert!(result.buffer.discarded());
    assert_eq!(result.buffer.len(), 0);
    assert!(result.buffer.elements().is_empty());
    drop(capture);
    assert!(runtime.drain().is_empty());
    // A byte-only limit must not permit arbitrarily many retained zero-byte
    // fields. This calls the actual capture's shared bounded IoBuffer.
    let capture = NativeCapture::new(1, Arc::clone(&runtime)).unwrap();
    let buffer = capture.buffer();
    assert!(buffer.append(b"", SeparationType::Explicitly));
    assert!(!buffer.append(b"", SeparationType::Explicitly));
    assert!(buffer.discarded());
    let result = capture.finish().unwrap();
    assert!(result.buffer.discarded());
    assert_eq!(result.buffer.len(), 0);
    assert!(result.buffer.elements().is_empty());
    assert!(result.error.is_none());
    assert!(!result.interrupted);
    drop(buffer);
    drop(capture);
    assert!(runtime.drain().is_empty());

    // Real partial spawn failure: first child has acknowledged readiness and is
    // blocked before any marker; second is an existing but corrupt PE image.
    let ready = NamedEvent::new("rollback-ready");
    let release = NamedEvent::new("rollback-release");
    let marker = cwd.join("rollback-marker");
    let corrupt = cwd.join("rollback-invalid.exe");
    std::fs::write(&corrupt, b"MZ-invalid-owned-native-image").unwrap();
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let argv = vec![
        probe.as_os_str().to_owned(),
        "n2-rollback-child".into(),
        ready.name.clone(),
        release.name.clone(),
        marker.as_os_str().to_owned(),
    ];
    let child = group
        .spawn(&probe, &argv, &cwd, &environment, &IoChain::new(), false)
        .unwrap();
    wait(&ready.handle);
    assert_eq!(
        unsafe { WaitForSingleObject(child.handle().as_raw_handle(), 0) },
        258
    );
    let failure = group.spawn(
        &corrupt,
        &[corrupt.as_os_str().to_owned()],
        &cwd,
        &environment,
        &IoChain::new(),
        false,
    );
    assert!(failure.is_err(), "corrupt native image spawned");
    drop(group);
    assert_eq!(
        unsafe { WaitForSingleObject(child.handle().as_raw_handle(), LIMIT.as_millis() as u32) },
        0,
        "partial-spawn rollback retained owned child"
    );
    assert!(!marker.exists());
    std::fs::remove_file(corrupt).unwrap();
    drop(child);
    runtime.drain();

    // Real wait cancellation does not invent an exit result, and explicit group
    // termination reaps only the acknowledged owned child.
    let ready = NamedEvent::new("cancel-ready");
    let release = NamedEvent::new("cancel-release");
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let argv = vec![
        probe.as_os_str().to_owned(),
        "n2-owned-descendant".into(),
        ready.name.clone(),
        release.name.clone(),
    ];
    let child = group
        .spawn(&probe, &argv, &cwd, &environment, &IoChain::new(), false)
        .unwrap();
    wait(&ready.handle);
    let cancellation = NativeCancellation::new().unwrap();
    cancellation.cancel();
    assert_eq!(
        group.wait(Some(&cancellation)).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(
        unsafe { WaitForSingleObject(child.handle().as_raw_handle(), 0) },
        258
    );
    group.terminate_and_wait().unwrap();
    assert_eq!(
        unsafe { WaitForSingleObject(child.handle().as_raw_handle(), 0) },
        0
    );
    drop(group);
    drop(child);
    runtime.drain();

    // Mark a real unrelated event inheritable in this process. The production
    // STARTUPINFOEX list must exclude it even while inheriting capture's writer.
    let sentinel = NamedEvent::new("unintended-inheritance");
    let mut inheritable = std::ptr::null_mut();
    assert_ne!(
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                sentinel.handle.as_raw_handle(),
                GetCurrentProcess(),
                &mut inheritable,
                0,
                1,
                2,
            )
        },
        0
    );
    let inheritable = own(inheritable);
    let capture = NativeCapture::new(4096, Arc::clone(&runtime)).unwrap();
    let mut group = NativeLaunchGroup::new(Arc::clone(&runtime)).unwrap();
    let quote = |s: &str| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"));
    let code = format!(
        "{} n2-inheritance {} {}",
        quote(probe.to_str().unwrap()),
        inheritable.as_raw_handle() as usize,
        quote(sentinel.name.to_str().unwrap())
    );
    let argv = vec![fish.to_owned(), "-N".into(), "-c".into(), code.into()];
    let child = group
        .spawn(
            std::path::Path::new(fish),
            &argv,
            &cwd,
            &environment,
            &capture.io_chain(),
            false,
        )
        .unwrap();
    assert_eq!(child.wait(None).unwrap(), 0);
    assert_eq!(group.wait(None).unwrap(), vec![0]);
    group.finish().unwrap();
    let result = capture.finish().unwrap();
    assert_eq!(result.buffer.newline_serialized(), b"sentinel-absent\n");
    assert!(result.error.is_none());
    assert!(!result.interrupted);
    assert_eq!(
        unsafe { WaitForSingleObject(sentinel.handle.as_raw_handle(), 0) },
        258
    );
    drop(child);
    drop(capture);
    drop(inheritable);
    assert!(runtime.drain().is_empty());
    println!("capture=available;descendant-live;cancelled;drained;rollback;inheritance-clean");
}
