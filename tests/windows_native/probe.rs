//! Real native child used by the Windows acceptance suite; never implements shell behavior.
#[cfg(windows)]
#[path = "conpty.rs"]
mod conpty;
#[cfg(windows)]
#[path = "n2.rs"]
mod windows_native_n2;
#[cfg(windows)]
#[path = "../windows_n4/notification_probe.rs"]
mod windows_native_n4_notifications;
#[cfg(windows)]
#[path = "../windows_n4/storage_probe.rs"]
mod windows_native_n4_storage;
#[cfg(windows)]
#[path = "../windows_n5/account_probe.rs"]
mod windows_native_n5_account;
#[cfg(windows)]
#[path = "../windows_n5/cancel_probe.rs"]
mod windows_native_n5_cancel;
#[cfg(windows)]
#[path = "../windows_n5/jobs_probe.rs"]
mod windows_native_n5_jobs;
#[cfg(windows)]
#[path = "../windows_n6/cases_probe.rs"]
mod windows_native_n6_cases;
#[cfg(windows)]
#[path = "../windows_n6/clipboard_probe.rs"]
mod windows_native_n6_clipboard;
#[cfg(windows)]
#[path = "../windows_n6/console.rs"]
mod windows_native_n6_console;
#[cfg(windows)]
#[path = "../windows_n7/eval_input_probe.rs"]
mod windows_native_n7_eval;
#[cfg(windows)]
#[path = "../windows_n7/render_probe.rs"]
mod windows_native_n7_render;
#[cfg(windows)]
#[path = "../windows_n8/provider_probe.rs"]
mod windows_native_n8_provider;
#[cfg(windows)]
#[path = "../windows_n8/adapters_probe.rs"]
mod windows_native_n8_adapters;
#[cfg(windows)]
#[path = "../windows_n8/policy_probe.rs"]
mod windows_native_n8_policy;
#[cfg(windows)]
#[path = "../windows_n8/services_probe.rs"]
mod windows_native_n8_services;
#[cfg(windows)]
#[path = "../windows_n9/assets_probe.rs"]
mod windows_native_n9_assets;
#[cfg(windows)]
#[path = "../windows_n11/network_probe.rs"]
mod windows_native_n11_network;
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
    fn record(key: &str, value: &str) {
        let mut out = std::io::stdout().lock();
        write!(out, "{key}=").unwrap();
        for byte in value.as_bytes() {
            write!(out, "{byte:02x}").unwrap();
        }
        writeln!(out).unwrap();
    }
    if std::env::var_os("FISH_N9_FACADE_MARKER").is_some() {
        if let Some(code) = windows_native_n9_assets::facade_fallback() {
            std::process::exit(code);
        }
    }
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().expect("probe mode");
    match mode.to_str().expect("Unicode mode") {
        mode if mode.starts_with("n11-network-") => {
            windows_native_n11_network::run(mode, args.collect())
        }
        mode if mode.starts_with("n9-") => {
            let args: Vec<_> = args.collect();
            std::process::exit(
                windows_native_n9_assets::try_main(mode, &args).expect("known N9 asset mode"),
            );
        }
        mode if mode.starts_with("n8-") || mode == "--fish-native-resource-lease" => {
            let args: Vec<_> = args.collect();
            for run in [
                windows_native_n8_provider::try_main,
                windows_native_n8_adapters::try_main,
                windows_native_n8_policy::try_main,
                windows_native_n8_services::try_main,
            ] {
                if let Some(status) = run(mode, &args) {
                    std::process::exit(status);
                }
            }
            panic!("unknown N8 probe mode: {mode}");
        }
        mode if mode.starts_with("n7-eval-") => {
            let args: Vec<_> = args.collect();
            std::process::exit(
                windows_native_n7_eval::try_main(mode, &args).expect("known N7 evaluator mode"),
            );
        }
        mode if mode.starts_with("n7-render-") => {
            let args: Vec<_> = args.collect();
            std::process::exit(
                windows_native_n7_render::try_main(mode, &args).expect("known N7 rendering mode"),
            );
        }
        mode if mode.starts_with("n6-console-") => {
            windows_native_n6_console::run(mode, args.collect())
        }
        mode if mode.starts_with("n6-case-") => windows_native_n6_cases::run(mode, args.collect()),
        mode if mode.starts_with("n6-clipboard-") => {
            windows_native_n6_clipboard::run(mode, args.collect())
        }
        "runtime-init" => windows_native_runtime::run(),
        mode if mode.starts_with("n2-") => windows_native_n2::run(mode, args.collect()),
        mode if mode.starts_with("n5-jobs-") => windows_native_n5_jobs::run(mode, args.collect()),
        mode if mode.starts_with("n5-cancel-") => {
            windows_native_n5_cancel::run(mode, args.collect())
        }
        mode if mode.starts_with("n5-account-") => {
            windows_native_n5_account::run(mode, args.collect())
        }
        "known-folder-roots" => {
            let (local, profile) =
                fish::path::native_known_folder_roots().expect("native known folders");
            let mut out = std::io::stdout().lock();
            writeln!(out, "{}", local.to_str().expect("Unicode local app data")).unwrap();
            writeln!(out, "{}", profile.to_str().expect("Unicode profile")).unwrap();
            let defaults = fish::path::native_state_paths(&fish::env::EnvNull::new())
                .expect("read-only default state policy");
            for path in defaults {
                writeln!(out, "{}", path.to_str().expect("Unicode default root")).unwrap();
            }
        }
        "n4-notifications" => windows_native_n4_notifications::run(
            std::path::Path::new(&args.next().expect("local root")),
            std::path::Path::new(&args.next().expect("required UNC root")),
        ),
        "n4-barrier" => {
            let ready = args.next().expect("ready event");
            let release = args.next().expect("release event");
            if let Err(error) = windows_native_n4_storage::barrier(&ready, &release) {
                writeln!(std::io::stderr().lock(), "{error}").unwrap();
                std::process::exit(1);
            }
        }
        mode @ ("n4-storage-append" | "n4-storage-rewrite" | "n4-rewrite") => {
            if let Err(error) = windows_native_n4_storage::run(mode, args) {
                writeln!(std::io::stderr().lock(), "{error}").unwrap();
                std::process::exit(1);
            }
        }
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
                "n1-copy" => {
                    std::fs::copy(source, args.next().expect("destination path")).map(|_| ())
                }
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
