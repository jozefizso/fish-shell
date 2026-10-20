use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{ffi::{OsStr, c_void}, fs, io::{ErrorKind, Read, Write}, net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream}, os::windows::io::AsRawHandle, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-ncat-ipv4-ipv6-source-crlf-send-only-consumers", source_and_send),
    ("real-native-ncat-listen-and-connect-recv-only-parameter-boundaries", listen_and_receive),
];

const LIMIT: Duration = Duration::from_secs(10);
const OUTBOUND: &[u8] = "owned outbound 雪 🐟\nsecond\r\n\0tail\n".as_bytes();
const CRLF_OUTBOUND: &[u8] = "owned outbound 雪 🐟\r\nsecond\r\n\0tail\r\n".as_bytes();
const INBOUND: &[u8] = "independent inbound 🐟 雪\r\n\0remote\n".as_bytes();

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32;
}

fn required() -> PathBuf {
    let path = PathBuf::from(std::env::var_os("FISH_N9_NCAT").expect("required genuine native Ncat fixture"));
    assert!(path.is_absolute() && path.is_file(), "absolute genuine Ncat fixture: {}", path.display());
    assert_eq!(path.file_name().unwrap(), "ncat.exe", "do not impersonate another netcat flavor");
    path
}

fn evaluate(f: &Fixture, ncat: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, ncat);
    run(&mut command, b"")
}

fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine Ncat consumer: {}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{}", String::from_utf8_lossy(&result.err));
}

fn selected(f: &Fixture, ncat: &Path, context: &str, candidate: &str) -> String {
    let result = evaluate(f, ncat, &format!(r#"
set -l selected (complete -C {} | string split -f 1 \t | string match -- {})
test (count $selected) -eq 1; or exit 81
printf '%s\n' $selected
"#, quote(context), quote(candidate)));
    success(&result);
    let text = String::from_utf8(result.out).unwrap();
    text.trim_end().to_owned()
}

// Every native child is directly owned, including on assertion unwinding. Files
// avoid pipe-capacity deadlocks; stdin stays open until the protocol needs EOF.
struct Native {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}
impl Native {
    fn start(f: &Fixture, ncat: &Path, label: &str, args: &[String]) -> Self {
        let stdout = f.root.join(format!("{label}.stdout"));
        let stderr = f.root.join(format!("{label}.stderr"));
        let mut command = Command::new(ncat);
        command.args(args).current_dir(&f.root).env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
        for (name, value) in f.command(&[]).get_envs() {
            if let Some(value) = value { command.env(name, value); }
        }
        tool_environment(&mut command, ncat);
        Self { child: command.spawn().expect("launch genuine native Ncat"), stdout, stderr }
    }
    fn input(&mut self, bytes: &[u8]) {
        let input = self.child.stdin.as_mut().unwrap();
        input.write_all(bytes).unwrap();
        input.flush().unwrap();
    }
    fn eof(&mut self) { drop(self.child.stdin.take()); }
    fn finish(&mut self) -> Captured {
        let wait = unsafe { WaitForSingleObject(self.child.as_raw_handle(), LIMIT.as_millis() as u32) };
        assert_eq!(wait, 0, "bounded Ncat lifetime: {}", String::from_utf8_lossy(&fs::read(&self.stderr).unwrap()));
        let status = self.child.wait().unwrap();
        self.eof();
        Captured {
            code: status.code().expect("native Ncat exit code") as u32,
            out: fs::read(&self.stdout).unwrap(),
            err: fs::read(&self.stderr).unwrap(),
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn listener(ip: IpAddr) -> TcpListener {
    assert!(ip.is_loopback());
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn configure(stream: &TcpStream) {
    assert!(stream.local_addr().unwrap().ip().is_loopback());
    assert!(stream.peer_addr().unwrap().ip().is_loopback());
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(LIMIT)).unwrap();
    stream.set_write_timeout(Some(LIMIT)).unwrap();
}

fn accept(listener: &TcpListener, child: &mut Native) -> TcpStream {
    let deadline = Instant::now() + LIMIT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => { configure(&stream); return stream; }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => panic!("owned Ncat peer accept: {error}"),
        }
        assert!(child.child.try_wait().unwrap().is_none(), "Ncat exited before connecting: {}", String::from_utf8_lossy(&fs::read(&child.stderr).unwrap()));
        assert!(Instant::now() < deadline, "bounded owned Ncat connect readiness");
        std::thread::yield_now();
    }
}

fn connect(endpoint: SocketAddr, child: &mut Native) -> TcpStream {
    assert!(endpoint.ip().is_loopback());
    let deadline = Instant::now() + LIMIT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "bounded owned Ncat listen readiness");
        match TcpStream::connect_timeout(&endpoint, remaining.min(Duration::from_millis(100))) {
            Ok(stream) => { configure(&stream); return stream; }
            Err(error) if matches!(error.kind(), ErrorKind::ConnectionRefused | ErrorKind::TimedOut) => {}
            Err(error) => panic!("owned Ncat listen connection: {error}"),
        }
        assert!(child.child.try_wait().unwrap().is_none(), "Ncat exited before accepting: {}", String::from_utf8_lossy(&fs::read(&child.stderr).unwrap()));
        std::thread::yield_now();
    }
}

fn source_and_send(f: &Fixture) {
    let ncat = required();
    let resolved = evaluate(f, &ncat, "command -s ncat");
    success(&resolved);
    assert_eq!(fs::canonicalize(String::from_utf8(resolved.out).unwrap().trim()).unwrap(), fs::canonicalize(&ncat).unwrap());
    let source = selected(f, &ncat, "ncat --sour", "--source");
    let source_port = selected(f, &ncat, "ncat --source-p", "--source-port");
    let send_only = selected(f, &ncat, "ncat --send-o", "--send-only");
    let crlf = selected(f, &ncat, "ncat --cr", "--crlf");
    for (ip, flag, label) in [(IpAddr::V4(Ipv4Addr::LOCALHOST), "-4", "ipv4"), (IpAddr::V6(Ipv6Addr::LOCALHOST), "-6", "ipv6")] {
        let protocol = selected(f, &ncat, "ncat -", flag);
        for convert in [false, true] {
            let observer = listener(ip);
            let destination = observer.local_addr().unwrap();
            // Reserve an OS-chosen source port, then release it for Ncat's bind.
            let reservation = listener(ip);
            let expected_source = reservation.local_addr().unwrap();
            drop(reservation);
            let mut args = vec![protocol.clone(), source.clone(), ip.to_string(), source_port.clone(), expected_source.port().to_string(), send_only.clone()];
            if convert { args.push(crlf.clone()); }
            args.extend([ip.to_string(), destination.port().to_string()]);
            let mut child = Native::start(f, &ncat, &format!("ncat-{label}-send-{convert}"), &args);
            child.input(OUTBOUND);
            child.eof();
            let mut peer = accept(&observer, &mut child);
            assert_eq!(peer.local_addr().unwrap(), destination);
            assert_eq!(peer.peer_addr().unwrap(), expected_source, "selected source address and source port must bind the genuine socket");
            let mut received = Vec::new();
            peer.read_to_end(&mut received).unwrap();
            assert_eq!(received, if convert { CRLF_OUTBOUND } else { OUTBOUND }, "selected --crlf must transform only line endings, preserving UTF-8 and NUL bytes");
            let result = child.finish();
            success(&result);
            assert!(result.out.is_empty());
        }
    }
}

fn listen_and_receive(f: &Fixture) {
    let ncat = required();
    let listen = selected(f, &ncat, "ncat --lis", "--listen");
    let recv_only = selected(f, &ncat, "ncat --recv-o", "--recv-only");
    for (ip, flag, label) in [(IpAddr::V4(Ipv4Addr::LOCALHOST), "-4", "ipv4"), (IpAddr::V6(Ipv6Addr::LOCALHOST), "-6", "ipv6")] {
        let protocol = selected(f, &ncat, "ncat -", flag);
        let reservation = listener(ip);
        let endpoint = reservation.local_addr().unwrap();
        drop(reservation);
        let mut child = Native::start(f, &ncat, &format!("ncat-{label}-listen"), &[protocol.clone(), listen.clone(), ip.to_string(), endpoint.port().to_string()]);
        let mut peer = connect(endpoint, &mut child);
        assert_eq!(peer.peer_addr().unwrap(), endpoint, "selected --listen must expose exactly the owned loopback endpoint");
        child.input(OUTBOUND);
        let mut received = vec![0; OUTBOUND.len()];
        peer.read_exact(&mut received).unwrap();
        assert_eq!(received, OUTBOUND, "native listener must transfer stdin, not merely accept a connection");
        peer.write_all(INBOUND).unwrap();
        peer.shutdown(Shutdown::Write).unwrap();
        let result = child.finish();
        success(&result);
        assert_eq!(result.out, INBOUND, "listener must independently receive the peer's UTF-8/CRLF/NUL bytes");
        drop(peer);

        // recv-only is a separate genuine connecting consumer. Ncat 7.992's
        // recv-only + single-address listen combination exits before accept;
        // that vendor limitation is not hidden with keep-open or a fake peer.
        let observer = listener(ip);
        let endpoint = observer.local_addr().unwrap();
        let mut child = Native::start(f, &ncat, &format!("ncat-{label}-receive"), &[protocol, recv_only.clone(), ip.to_string(), endpoint.port().to_string()]);
        child.input(OUTBOUND);
        let mut peer = accept(&observer, &mut child);
        assert_eq!(peer.local_addr().unwrap(), endpoint);
        peer.write_all(INBOUND).unwrap();
        peer.shutdown(Shutdown::Write).unwrap();
        let mut sent = Vec::new();
        peer.read_to_end(&mut sent).unwrap();
        assert!(sent.is_empty(), "selected --recv-only must not send the available stdin payload");
        let result = child.finish();
        success(&result);
        assert_eq!(result.out, INBOUND);
    }

    for (context, forbidden) in [
        ("ncat --source --lis", "--listen"),
        ("ncat -s --send-o", "--send-only"),
        ("ncat --source-port --cr", "--crlf"),
        ("ncat -p --recv-o", "--recv-only"),
        ("ncat --proxy-type --lis", "--listen"),
    ] {
        let result = evaluate(f, &ncat, &format!("complete -C {}", quote(context)));
        success(&result);
        assert!(!String::from_utf8(result.out).unwrap().lines().any(|line| line.split('\t').next() == Some(forbidden)), "option must not fill a required parameter role: {context}");
    }
    let absent_metadata = f.eval("status get-file completions/ncat.fish | source; complete -C 'ncat --lis'");
    success(&absent_metadata);
    assert!(String::from_utf8(absent_metadata.out).unwrap().lines().any(|line| line.split('\t').next() == Some("--listen")));
    let absent = f.eval("command ncat --version");
    assert_eq!(absent.code, 127);
    assert!(absent.out.is_empty());
}

// Deliberately not blanket netcat closure: hostname/hosts/SSH namespaces,
// __fish_complete_netcat's delegated entrypoint, nc/netcat and other real
// flavors, TLS/cipher/SNI/ALPN, proxies, datagram/other transports, no-shutdown,
// keep-open/broker/chat, access-control, logs, telnet and timing roles retain
// their separate consumer obligations. No command execution or scanning runs.
