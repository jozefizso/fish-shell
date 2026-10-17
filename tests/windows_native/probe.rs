//! Real native child used by the Windows acceptance suite; never implements shell behavior.
#[cfg(windows)]
#[path = "runtime.rs"]
mod windows_native_runtime;
#[cfg(windows)]
#[path = "transport.rs"]
mod windows_native_transport;
#[cfg(windows)]
fn main() {
    use std::io::{Read, Write};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ExitProcess(code: u32) -> !;
    }
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().expect("probe mode");
    match mode.to_str().expect("Unicode mode") {
        "runtime-init" => windows_native_runtime::run(),
        "transport-console-builtin" => {
            windows_native_transport::run_console_builtin(&args.next().expect("fish script"))
        }
        "transport-file" => {
            let file = std::fs::File::create(args.next().unwrap()).unwrap();
            let mut endpoint = fish::io::NativeOutput::owned(file.into());
            endpoint.write_all(b"\0\n\r\n\xff\x80").unwrap();
            endpoint.flush().unwrap();
        }
        mode if mode.starts_with("transport-") => windows_native_transport::run(mode),
        "stream" => {
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes).unwrap();
            let mut out = std::io::stdout().lock();
            out.write_all(b"probe-out:").unwrap();
            out.write_all(&bytes).unwrap();
            out.flush().unwrap();
            let mut err = std::io::stderr().lock();
            err.write_all(b"probe-err:\0\n\r\n\xff\x80").unwrap();
            err.flush().unwrap();
        }
        "exit" => {
            let code: u32 = args.next().unwrap().to_str().unwrap().parse().unwrap();
            unsafe { ExitProcess(code) }
        }
        "marker" => {
            std::fs::write(args.next().unwrap(), b"launched").unwrap();
        }
        "report" => {
            fn record(key: &str, value: &str) {
                print!("{key}=");
                for byte in value.as_bytes() {
                    print!("{byte:02x}");
                }
                println!();
            }
            record("cwd", std::env::current_dir().unwrap().to_str().unwrap());
            for (index, value) in args.enumerate() {
                record(&format!("arg{index}"), value.to_str().unwrap());
            }
            let mut vars: Vec<_> = std::env::vars()
                .filter(|(key, _)| key.starts_with("FISH_ACCEPT_"))
                .collect();
            vars.sort();
            for (key, value) in vars {
                record(&key, &value);
            }
        }
        mode => panic!("unknown probe mode {mode}"),
    }
}
#[cfg(not(windows))]
fn main() {
    panic!("windows_native_probe requires native Windows");
}
