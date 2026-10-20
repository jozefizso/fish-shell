//! A normal native TCP client used only against the harness's owned loopback listener.
use std::{ffi::OsString, io::{Read, Write}, net::{SocketAddr, TcpStream}, time::Duration};

pub fn run(mode: &str, args: Vec<OsString>) -> ! {
    assert_eq!(args.len(), 1);
    let address: SocketAddr = args[0].to_str().unwrap().parse().unwrap();
    assert!(address.ip().is_loopback(), "network fault helper is restricted to the owned loopback fixture");
    let mut connection = TcpStream::connect_timeout(&address, Duration::from_secs(30)).unwrap();
    connection.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    connection.set_write_timeout(Some(Duration::from_secs(30))).unwrap();
    match mode {
        "n11-network-disconnect" => {
            connection.write_all(b"READY\n").unwrap();
            let mut byte = [0];
            assert_eq!(connection.read(&mut byte).unwrap(), 0, "the real server must disconnect after receiving READY");
            std::process::exit(37);
        }
        "n11-network-reconnect" => {
            let mut bytes = Vec::new();
            connection.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"recovered\n");
            std::io::stdout().write_all(&bytes).unwrap();
            std::process::exit(0);
        }
        _ => panic!("unknown owned network mode {mode}"),
    }
}
