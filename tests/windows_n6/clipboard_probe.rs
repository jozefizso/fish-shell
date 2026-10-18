//! Runs actual editor clipboard bindings only after fresh-logon/station isolation.
use super::windows_native_n6_console as console;
#[path = "private_station.rs"]
mod private_station;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
    process::Command,
};
#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: *mut std::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: *mut std::ffi::c_void,
        menu: *mut std::ffi::c_void,
        instance: *mut std::ffi::c_void,
        param: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn DestroyWindow(window: *mut std::ffi::c_void) -> i32;
}
struct ClipboardLock(*mut std::ffi::c_void);
impl ClipboardLock {
    fn acquire() -> Self {
        // A real owner window establishes the same cross-process exclusion as
        // the provider unit's busy holder; NULL is not a reliable busy fixture.
        let class = [83u16, 84, 65, 84, 73, 67, 0];
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                -3isize as *mut _,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !window.is_null(),
            "private clipboard holder window: {}",
            std::io::Error::last_os_error()
        );
        if unsafe { OpenClipboard(window) } == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                DestroyWindow(window);
            }
            panic!("lock only the isolated clipboard: {error}");
        }
        Self(window)
    }
}
impl Drop for ClipboardLock {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
            DestroyWindow(self.0);
        }
    }
}
fn capture(s: &mut console::Session, root: &Path, n: usize, expected: &str) {
    s.send(b"\x1b[24~");
    s.wait_file(&root.join(format!("clip-ready-{n}")));
    assert_eq!(
        fs::read(root.join(format!("clip-capture-{n}"))).unwrap(),
        expected.as_bytes()
    );
}
pub fn run(mode: &str, args: Vec<OsString>) {
    match mode {
        "n6-clipboard-launch" => {
            assert_eq!(args.len(), 3);
            private_station::launch(
                Path::new(&args[0]),
                Path::new(&args[1]),
                Path::new(&args[2]),
            );
        }
        "n6-clipboard-private" => {
            assert_eq!(args.len(), 4);
            let outer = Path::new(&args[0]);
            let root = outer.join("private-clipboard");
            // This must precede Session, fish, console bootstrap, and every access
            // to CF_UNICODETEXT. The launcher checked fresh auth before resume.
            let station = private_station::PrivateStation::create(args[3].to_str().unwrap());
            fs::write(
                root.join("isolation.txt"),
                format!(
                    "auth={};station={};private-desktop=fresh",
                    private_station::authentication_text(),
                    station.name
                ),
            )
            .unwrap();
            let init = "function fish_prompt; printf 'CLIP> '; end; set -g clip_capture 0; function clip_capture; set -g clip_capture (math $clip_capture + 1); printf '%s' (commandline | string collect) > clip-capture-$clip_capture; printf ready > clip-ready-$clip_capture; end; bind f12 clip_capture; bind f2 fish_clipboard_copy; bind f3 fish_clipboard_paste; bind f4 begin-selection; bind f5 end-selection; bind f6 'commandline --cursor 0'; bind f7 'commandline --replace \"\"'";
            let mut cmd = Command::new(&args[1]);
            cmd.args(["-N", "-i", "-C", init])
                .current_dir(&root)
                .env_clear()
                .env("PATH", "")
                .env("LANG", "C")
                .env("FISH_N6_PRIVATE_STATION", &station.name)
                .env("FISH_N6_PRIVATE_DESKTOP", station.desktop_path());
            if let Some(system) = std::env::var_os("SystemRoot") {
                cmd.env("SystemRoot", system);
            }
            for (key, dir) in [
                ("HOME", "home"),
                ("USERPROFILE", "home"),
                ("XDG_CONFIG_HOME", "config"),
                ("APPDATA", "config"),
                ("XDG_DATA_HOME", "data"),
                ("LOCALAPPDATA", "data"),
                ("XDG_CACHE_HOME", "cache"),
            ] {
                fs::create_dir_all(root.join(dir)).unwrap();
                cmd.env(key, root.join(dir));
            }
            let mut s = console::Session::start(&cmd, Path::new(&args[2]), &root);
            s.wait_protocol(true);
            // Pipe copy preserves real UTF-8/trailing LF and normalizes Windows CRLF.
            s.send("printf 'pipe 雪🐟\\nsecond\\n\\n' | fish_clipboard_copy; printf '%s' $status > pipe-copy-status\r".as_bytes());
            assert_eq!(s.wait_file(&root.join("pipe-copy-status")), b"0");
            s.send(
                b"fish_clipboard_paste > pipe-roundtrip; printf '%s' $status > pipe-paste-status\r",
            );
            assert_eq!(s.wait_file(&root.join("pipe-paste-status")), b"0");
            assert_eq!(
                fs::read(root.join("pipe-roundtrip")).unwrap(),
                "pipe 雪🐟\nsecond\n\n".as_bytes()
            );
            // Public copy uses the actual reader selection, including surrogate
            // pairs and combining characters; no copied model is involved.
            s.send("雪🐟e\u{301} tail".as_bytes());
            s.send(b"\x1b[17~\x1bOS\x1b[C\x1b[C\x1b[C\x1b[C\x1bOQ\x1b[15~\x1b[18~\x1bOR");
            capture(&mut s, &root, 1, "雪🐟e\u{301}");
            s.send(b"\x03fish_clipboard_paste > selected-copy; printf ready > selected-ready\r");
            s.wait_file(&root.join("selected-ready"));
            assert_eq!(
                fs::read(root.join("selected-copy")).unwrap(),
                "雪🐟e\u{301}".as_bytes()
            );
            // With no selection, copy the entire logical commandline and paste
            // into the actual editor without executing it.
            s.send(b"printf line > must-not-execute\x1bOQ\x1b[18~\x1bOR");
            capture(&mut s, &root, 2, "printf line > must-not-execute");
            assert!(!root.join("must-not-execute").exists());
            s.send(b"\x03printf '\\0' | fish_clipboard_copy 2> nul-error; printf '%s' $status > nul-status\r");
            assert_ne!(s.wait_file(&root.join("nul-status")), b"0");
            assert!(!fs::read(root.join("nul-error")).unwrap().is_empty());
            let probe = format!(
                "'{}'",
                args[2]
                    .to_str()
                    .unwrap()
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'")
            );
            s.send(format!("{probe} n6-case-invalid-utf8 | fish_clipboard_copy 2> utf8-error; printf '%s' $status > utf8-status\r").as_bytes());
            assert_ne!(s.wait_file(&root.join("utf8-status")), b"0");
            assert!(!fs::read(root.join("utf8-error")).unwrap().is_empty());
            {
                let _locked = ClipboardLock::acquire();
                s.send(b"fish_clipboard_paste > denied-out 2> denied-error; printf '%s' $status > denied-status\r");
                assert_ne!(s.wait_file(&root.join("denied-status")), b"0");
                assert!(!fs::read(root.join("denied-error")).unwrap().is_empty());
                assert!(fs::read(root.join("denied-out")).unwrap().is_empty());
                // Without a redirect the same real busy provider must still
                // diagnose on the console, not lose stderr while capturing text.
                s.send(b"fish_clipboard_paste > busy-console-out; printf '%s' $status > busy-console-status\r");
                assert_ne!(s.wait_file(&root.join("busy-console-status")), b"0");
                assert!(fs::read(root.join("busy-console-out")).unwrap().is_empty());
                s.wait_text(b"__fish_clipboard: paste:");
            }
            s.send(b"fish_clipboard_paste > after-denied; printf '%s' $status > after-denied-status; printf ready > after-denied-ready\r");
            s.wait_file(&root.join("after-denied-ready"));
            assert_eq!(fs::read(root.join("after-denied-status")).unwrap(), b"0");
            assert_eq!(
                fs::read(root.join("after-denied")).unwrap(),
                b"printf line > must-not-execute"
            );
            s.send(b"after-clipboard-errors");
            capture(&mut s, &root, 3, "after-clipboard-errors");
            s.send(b"\x03exit 0\r");
            let t = s.finish(0);
            assert_eq!(t.original_modes, t.final_modes);
            assert_eq!(
                t.bytes
                    .windows(b"__fish_clipboard: paste:".len())
                    .filter(|bytes| *bytes == b"__fish_clipboard: paste:")
                    .count(),
                1,
                "only the unredirected busy paste may diagnose on the console"
            );
            fs::write(
                outer.join("clipboard-passed"),
                b"pipe-selection-line-paste-private",
            )
            .unwrap();
        }
        _ => panic!("unknown private clipboard case {mode}"),
    }
}
