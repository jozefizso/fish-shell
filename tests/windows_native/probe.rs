//! Real native child used by the Windows acceptance suite; never implements shell behavior.
#[cfg(windows)]
#[path = "runtime.rs"]
mod windows_native_runtime;
#[cfg(windows)]
#[path = "transport.rs"]
mod windows_native_transport;
#[cfg(windows)]
#[path = "n2.rs"]
mod windows_native_n2;
#[cfg(windows)]
fn main() {
    use std::io::{Read, Write};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ExitProcess(code: u32) -> !;
    }
    fn record(key: &str, value: &str) {
        let mut out = std::io::stdout().lock();
        write!(out, "{key}=").unwrap();
        for byte in value.as_bytes() {
            write!(out, "{byte:02x}").unwrap();
        }
        writeln!(out).unwrap();
    }
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().expect("probe mode");
    match mode.to_str().expect("Unicode mode") {
        "runtime-init" => windows_native_runtime::run(),
        mode if mode.starts_with("n2-") => windows_native_n2::run(mode, args.collect()),
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
        mode @ ("n1-rename" | "n1-remove-dir" | "n1-copy") => {
            let source = args.next().expect("source path");
            if matches!(mode, "n1-rename" | "n1-remove-dir") {
                // The helper must not retain its own process cwd inside the directory being
                // mutated. Fish's parser-owned handle, not this incidental lock, is under test.
                std::env::set_current_dir(std::path::Path::new(&source).parent().unwrap())
                    .expect("release helper process cwd before directory mutation");
            }
            let result = match mode {
                "n1-rename" => std::fs::rename(source, args.next().expect("destination path")),
                "n1-copy" => std::fs::copy(source, args.next().expect("destination path")).map(|_| ()),
                "n1-remove-dir" => std::fs::remove_dir(source),
                _ => unreachable!(),
            };
            let mut out = std::io::stdout().lock();
            match result {
                Ok(()) => writeln!(out, "ok").unwrap(),
                Err(error) => writeln!(
                    out,
                    "error={}",
                    error.raw_os_error().expect("native filesystem error code")
                )
                .unwrap(),
            }
        }
        "identity" => {
            record("exe", std::env::current_exe().unwrap().to_str().unwrap());
        }
        "exit" => {
            let code: u32 = args.next().unwrap().to_str().unwrap().parse().unwrap();
            unsafe { ExitProcess(code) }
        }
        "marker" => {
            std::fs::write(args.next().unwrap(), b"launched").unwrap();
        }
        "report" => {
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
