use super::{console::Session, editor, fixture::{exact, quote, run}, native::{Event, Release}, package::Installed, resources::{Process, Sample}, support::Controlled};
use std::{ffi::OsStr, fs, io::Write, time::Instant};

fn cycle(s: &mut Session, installed: &Installed, cycle: usize) {
    let f = &installed.fixture;
    let code = format!("{} marker console-child-{cycle} &; wait; or exit 94; complete -e -c n11tool; complete -c n11tool -f -a 'snow shell'; complete -C 'n11tool sn' > completion-{cycle}; or exit 93; set -U n11_editor_value {cycle}; or exit 95; printf done > cycle-{cycle}\r", f.probe());
    s.send(code.as_bytes());
    assert_eq!(s.wait_file(&f.root.join(format!("cycle-{cycle}"))), b"done");
    assert_eq!(fs::read(f.root.join(format!("console-child-{cycle}"))).unwrap(), b"launched");
    assert_eq!(fs::read(f.root.join(format!("completion-{cycle}"))).unwrap(), b"snow\n");
    s.send("雪🐟e\u{301}".as_bytes());
    let text = editor::observe(s, f, cycle + 1).0;
    assert_eq!(text, "雪🐟e\u{301}");
    s.send(b"\x03");
    s.wait_input_drained();
}

pub fn exercise(installed: &Installed) {
    let f = &installed.fixture;
    // These retained cases make genuine I/O, launch, callback, inheritance and
    // ownership assertions. They are not reimplemented as inventory checks.
    super::lifetime::exercise(f);
    installed.record("faults/n2-stream-lifetime-inheritance-launch-rollback");
    super::jobs::disown_shell_exit(f);
    installed.record("faults/n5-disown-and-ordinary-child-survival-boundaries");
    super::notifications::exercise(f);
    installed.record("faults/n4-notification-overflow-rebind-unc-cancel-drain");
    super::storage::exercise_recovery(f);
    installed.record("faults/n4-acl-staging-lock-replacement-interrupted-save");
    fs::copy(std::env::var_os("FISH_NATIVE_GUI_PROBE_BIN").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_windows_n5_gui_probe"))), f.root.join("windows_n5_gui_probe.exe")).unwrap();
    for (name, family) in super::cancel::FAMILIES {
        family(f);
        installed.record(&format!("faults/n5-{name}"));
    }
    let effect = f.root.join("argument-must-not-execute");
    let payload = format!("(printf forbidden > {}) ; & | 雪", quote(effect.to_str().unwrap()));
    let result = f.eval(&format!("{} report {}", f.probe(), quote(&payload)));
    assert_eq!(result.code, 0);
    let encoded = payload.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert!(String::from_utf8_lossy(&result.out).lines().any(|line| line == format!("arg0={encoded}")));
    assert!(!effect.exists(), "literal argument escaped into evaluator execution");
    installed.record("faults/literal-native-argument-no-interpreter-injection");

    let extra = "printf '%s' $fish_pid > n11-fish.pid; printf ready > n11-pid-ready";
    let mut session = editor::start(f, extra);
    session.wait_file(&f.root.join("n11-pid-ready"));
    let process = Process::retain(fs::read_to_string(f.root.join("n11-fish.pid")).unwrap().parse().unwrap());
    let mut trace = fs::File::create(installed.evidence.join("resources.csv")).unwrap();
    writeln!(trace, "phase,cycle,handles,threads,private_bytes,working_bytes").unwrap();
    let record = |trace: &mut fs::File, phase: &str, cycle: usize, sample: Sample| {
        writeln!(trace, "{phase},{cycle},{},{},{},{}", sample.handles, sample.threads, sample.private_bytes, sample.working_bytes).unwrap();
        trace.flush().unwrap();
    };
    let mut warmup = Vec::new();
    for i in 0..8 { cycle(&mut session, installed, i); let sample = process.sample(); record(&mut trace, "warmup", i, sample); warmup.push(sample); }

    let ready: Vec<_> = (0..4).map(|n| Event::auto(&format!("writer-{n}"))).collect();
    let release: Vec<_> = (0..4).map(|n| Event::auto(&format!("writer-release-{n}"))).collect();
    let started = Instant::now();
    std::thread::scope(|scope| {
        let _release: Vec<_> = release.iter().map(Release).collect();
        let mut workers = Vec::new();
        for (n, ready) in ready.iter().enumerate() {
            let script = format!("for i in {}; {} n4-barrier {} {}; or exit 91; builtin history append n11-worker-{n}-$i; builtin history save; or exit 92; set -U n11_writer_{n} $i; or exit 93; {} marker writer-child-{n}-$i &; wait; or exit 94; end; printf 'writer-{n}-done'", (0..64).map(|n| n.to_string()).collect::<Vec<_>>().join(" "), f.probe(), quote(&ready.name), quote(&release[n].name), f.probe());
            workers.push(scope.spawn(move || {
                let mut command = f.command(&[OsStr::new("-c"), OsStr::new(&script)]);
                command.env("fish_history", "n11_shared");
                (n, run(&mut command, b""))
            }));
        }
        for i in 8..72 {
            for event in &ready { event.wait(); }
            for event in &release { event.signal(); }
            cycle(&mut session, installed, i);
            record(&mut trace, "sustained", i, process.sample());
        }
        for worker in workers {
            let (n, result) = worker.join().unwrap();
            exact(result, 0, format!("writer-{n}-done").as_bytes(), b"");
        }
    });
    // Fresh initialization must see every acknowledged merge, not just a resident cache.
    let result = run(&mut f.command(&[OsStr::new("-c"), OsStr::new("builtin history merge; builtin history search --prefix n11-worker-")]).env("fish_history", "n11_shared"), b"");
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    let history = String::from_utf8(result.out).unwrap();
    for worker in 0..4 {
        for entry in 0..64 {
            assert!(history.lines().any(|line| line == format!("n11-worker-{worker}-{entry}")), "lost durable history merge: worker={worker} entry={entry}");
            assert_eq!(fs::read(f.root.join(format!("writer-child-{worker}-{entry}"))).unwrap(), b"launched");
        }
        exact(f.eval(&format!("printf '%s' $n11_writer_{worker}")), 0, b"63", b"");
    }
    session.send(b"printf '%s\\n' $n11_writer_0 $n11_writer_1 $n11_writer_2 $n11_writer_3 > notified-values; printf ready > notified-ready\r");
    session.wait_file(&f.root.join("notified-ready"));
    assert_eq!(fs::read(f.root.join("notified-values")).unwrap(), b"63\n63\n63\n63\n");
    let mut settled = Vec::new();
    for i in 72..80 { cycle(&mut session, installed, i); let sample = process.sample(); record(&mut trace, "quiescent", i, sample); settled.push(sample); }
    assert!(settled.iter().map(|s| s.handles).max().unwrap() <= warmup.iter().map(|s| s.handles).max().unwrap(), "native handle growth after quiescence; inspect resources.csv");
    assert!(settled.iter().map(|s| s.threads).max().unwrap() <= warmup.iter().map(|s| s.threads).max().unwrap(), "native thread growth after quiescence; inspect resources.csv");
    fs::write(installed.evidence.join("stress-parameters.txt"), format!("workers=4\niterations_per_writer=64\nconsole_cycles=80\nwarmup_cycles=8\nquiescent_cycles=8\nelapsed_ms={}\nsynchronization=per-cycle named auto-reset ready/release events and closed capture files\n", started.elapsed().as_millis())).unwrap();
    fs::write(installed.evidence.join("stress-final-screen.txt"), format!("{:?}", session.snapshot())).unwrap();
    let transcript = editor::finish(session);
    fs::write(installed.evidence.join("stress-console.vt"), transcript.bytes).unwrap();
    installed.record("faults/sustained-jobs-unicode-edits-completion-history-uvars-resource-trace");

    // Real sharing denial must leave the last acknowledged durable value intact.
    let state = f.state.join("config/fish/fish_variables");
    exact(f.eval("set -U n11_pending before"), 0, b"", b"");
    use std::os::windows::fs::OpenOptionsExt;
    let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&state).unwrap();
    let result = f.eval("set -U n11_pending denied");
    assert_ne!(result.code, 0);
    assert!(!result.err.is_empty());
    drop(lock);
    exact(f.eval("printf '%s' $n11_pending"), 0, b"before", b"");
    exact(f.eval("set -U n11_pending recovered; printf '%s' $n11_pending"), 0, b"recovered", b"");
    exact(f.eval("printf '%s' $n11_pending"), 0, b"recovered", b"");
    installed.record("faults/sharing-denial-recovery-durable-state");

    let mut child = Controlled::new(f, "abrupt-owned-child");
    std::thread::scope(|scope| {
        let command = child.command(f, 0);
        let work = scope.spawn(move || f.eval(&format!("{command} &; wait; printf 'status:%s' $status")));
        let victim = Process::retain(child.started());
        victim.terminate_owned(37);
        child.wait(37);
        exact(work.join().unwrap(), 0, b"status:0", b"");
    });
    installed.record("faults/abrupt-owned-child-exit-and-wait-recovery");
    network_disconnect(installed);
    // Memory acceptance uses an externally recorded pre-adopted budget. This
    // consumer records all bytes even when that prerequisite is unavailable.
    installed.python("tests/windows_n11/reconcile.py", &[OsStr::new("--resources-only")]);
}

fn network_disconnect(installed: &Installed) {
    use std::{io::Read, net::{Shutdown, TcpListener}, os::windows::io::AsRawSocket, time::Duration};
    #[repr(C)]
    struct Poll { socket: usize, events: i16, returned: i16 }
    #[link(name = "ws2_32")]
    unsafe extern "system" { fn WSAPoll(sockets: *mut Poll, count: u32, timeout: i32) -> i32; }
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let accept = || {
        let mut poll = Poll { socket: listener.as_raw_socket() as usize, events: 0x0100, returned: 0 };
        assert_eq!(unsafe { WSAPoll(&mut poll, 1, 30_000) }, 1, "bounded owned loopback accept");
        let (stream, peer) = listener.accept().unwrap();
        assert!(peer.ip().is_loopback());
        stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(30))).unwrap();
        stream
    };
    std::thread::scope(|scope| {
        let server = scope.spawn(|| {
            let mut first = accept();
            let mut ready = [0; 6];
            first.read_exact(&mut ready).unwrap();
            assert_eq!(&ready, b"READY\n");
            first.shutdown(Shutdown::Both).unwrap();
            drop(first);
            let mut second = accept();
            second.write_all(b"recovered\n").unwrap();
            second.shutdown(Shutdown::Write).unwrap();
        });
        let p = installed.fixture.probe();
        exact(installed.fixture.eval(&format!("{p} n11-network-disconnect {address}; set -l lost $status; {p} n11-network-reconnect {address}; printf 'disconnect:%s;reconnect:%s\\n' $lost $status; echo shell-recovered")), 0, b"recovered\ndisconnect:37;reconnect:0\nshell-recovered\n", b"");
        server.join().unwrap();
    });
    installed.record("faults/real-loopback-disconnect-native-status-reconnect-shell-recovery");
}
