//! Native executables used by asset consumers: an argv editor and a completion generator.
use std::{ffi::OsString, fs, io::{self, Write}, path::Path};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::process::Command;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *mut std::ffi::c_void, manual: i32, initial: i32, name: *const u16) -> *mut std::ffi::c_void;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
}

fn wait_for_control(record: &Path, resource: &str) -> i32 {
    fs::write(record, format!("{resource}\n{}", std::process::id())).unwrap();
    let raw = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
    assert!(!raw.is_null());
    let event = unsafe { OwnedHandle::from_raw_handle(raw) };
    assert_eq!(unsafe { WaitForSingleObject(event.as_raw_handle(), 30_000) }, 0, "owned workflow was not interrupted");
    12
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn interrupted_workflow(args: &[OsString]) -> i32 {
    use super::windows_native_n6_console::Session;
    assert_eq!(args.len(), 4);
    let root = Path::new(&args[0]);
    let fish = Path::new(&args[1]);
    let probe = Path::new(&args[2]);
    let kind = args[3].to_str().unwrap();
    let evidence = root.join(format!("interrupted-{kind}"));
    fs::create_dir(&evidence).unwrap();
    let temporary = evidence.join("temporary");
    fs::create_dir(&temporary).unwrap();
    let record = evidence.join("resource-record");
    let probe_arg = quote(probe.to_str().unwrap());
    let record_arg = quote(record.to_str().unwrap());
    let saved = temporary.join("atomic-save.fish");
    let original = b"function saved; echo original; end\n";
    if kind == "save" {
        fs::write(&saved, original).unwrap();
    }
    let commandline = match kind {
        "editor" => {
            let editor = quote(&format!("{probe_arg} n9-editor-wait {record_arg}"));
            format!("funced --save --editor={editor} n9_interrupted")
        }
        "extraction" => format!("__fish_data_with_directory tools/web_config '.*' {probe_arg} n9-extraction-wait {record_arg}"),
        "cache" => format!("__fish_cache_sourced_completions --name=n9-interrupted {probe_arg} n9-generator-wait {record_arg}"),
        "save" => format!("{probe_arg} n9-save-producer-wait {record_arg} {} | __fish_native_fs atomic-write {}", quote(saved.to_str().unwrap()), quote(saved.to_str().unwrap())),
        _ => panic!("unknown interrupted consumer {kind}"),
    };
    let mut setup = String::from("function fish_prompt; printf 'N9-INTERRUPT> '; end");
    if kind == "save" {
        // Establish the writer's persistent coordination state before measuring
        // cancellation cleanup, without depending on its private lock filename.
        setup.push_str(&format!("; printf '%s' {} | __fish_native_fs atomic-write {}",
            quote(std::str::from_utf8(original).unwrap()), quote(saved.to_str().unwrap())));
    }
    let mut command = Command::new(fish);
    command.args(["-N", "-i", "-C", &setup])
        .current_dir(root).env("TEMP", &temporary).env("TMP", &temporary);
    let mut session = Session::start(&command, probe, &evidence);
    session.wait_protocol(true);
    let entries = || {
        let mut paths: Vec<_> = fs::read_dir(&temporary).unwrap()
            .map(|entry| entry.unwrap().path()).collect();
        paths.sort_unstable();
        paths
    };
    let before = entries();
    session.send(format!("{commandline}\r").as_bytes());
    let record = String::from_utf8(session.wait_file(&record)).unwrap();
    let (resource, pid) = record.split_once('\n').unwrap();
    assert!(Path::new(resource).exists(), "real consumer must own a live resource before cancel");
    let process = unsafe { OpenProcess(0x0010_0000, 0, pid.parse().unwrap()) };
    assert!(!process.is_null(), "open actual blocked consumer");
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    session.wait_protocol(false);
    session.control(0);
    assert_eq!(unsafe { WaitForSingleObject(process.as_raw_handle(), 5_000) }, 0, "cancel must reap the actual owned child");
    session.wait_protocol(true);
    let after = evidence.join("after-interrupt");
    session.send(format!("printf ready > {}\r", quote(after.to_str().unwrap())).as_bytes());
    assert_eq!(session.wait_file(&after), b"ready");
    if kind == "save" {
        assert_eq!(fs::read(&saved).unwrap(), original, "interrupted save published partial input");
        assert_eq!(entries(), before, "interrupted atomic save left a staging file");
    } else {
        assert!(!Path::new(resource).exists(), "{kind} resource survived evaluator unwind: {resource}");
        assert!(fs::read_dir(&temporary).unwrap().next().is_none(), "{kind} left a private staging resource");
    }
    if kind == "editor" {
        let config = std::path::PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap());
        assert!(!config.join("fish/functions/n9_interrupted.fish").exists(), "cancelled funced persisted a partial definition");
    }
    session.send(b"exit\r");
    let transcript = session.finish(0);
    assert_eq!(transcript.original_modes, transcript.final_modes);
    0
}

fn prompt_duration(args: &[OsString]) -> i32 {
    use super::windows_native_n6_console::Session;
    assert_eq!(args.len(), 3);
    let root = Path::new(&args[0]);
    let fish = Path::new(&args[1]);
    let probe = Path::new(&args[2]);
    let evidence = root.join("prompt-duration");
    fs::create_dir(&evidence).unwrap();
    let first = evidence.join("first-prompt");
    let after = evidence.join("after-command");
    let setup = format!(r#"
        status get-file prompts/disco.fish | source
        functions -c fish_right_prompt __n9_disco_right_prompt
        function fish_right_prompt
            set -l duration "$CMD_DURATION"
            set -l rendered (__n9_disco_right_prompt | string replace -ar '\e\[[0-9;]*m' '' | string collect)
            if set -q __n9_after_duration
                printf '%s\n%s' "$duration" "$rendered" >{}
            else
                printf '%s\n%s' "$duration" "$rendered" >{}
            end
            printf '%s' "$rendered"
        end
    "#, quote(after.to_str().unwrap()), quote(first.to_str().unwrap()));
    let mut command = Command::new(fish);
    command.args(["-N", "-i", "-C", &setup]).current_dir(root);
    let mut session = Session::start(&command, probe, &evidence);
    session.wait_protocol(true);
    let first = String::from_utf8(session.wait_file(&first)).unwrap();
    let (duration, rendered) = first.split_once('\n').unwrap();
    assert!(duration.is_empty(), "the first real prompt must not invent CMD_DURATION: {duration}");
    assert!(!rendered.split_whitespace().any(|word| word.strip_suffix('s').is_some_and(|value| value.parse::<f64>().is_ok())),
        "first prompt rendered an invented command duration: {rendered}");
    session.send(format!("set -g __n9_after_duration 1; {} n9-duration-command\r", quote(probe.to_str().unwrap())).as_bytes());
    let after = String::from_utf8(session.wait_file(&after)).unwrap();
    let (duration, rendered) = after.split_once('\n').unwrap();
    let milliseconds = duration.parse::<u64>().expect("actual completed command duration in milliseconds");
    assert!(milliseconds > 100, "actual native command was not timed: {milliseconds}");
    let seconds = rendered.split_whitespace().find_map(|word| word.strip_suffix('s').and_then(|value| value.parse::<f64>().ok()))
        .expect("disco right prompt must render the measured duration in seconds");
    assert!((seconds * 1000.0 - milliseconds as f64).abs() < 1.0,
        "rendered duration {seconds}s does not match actual CMD_DURATION {milliseconds}ms");
    session.wait_protocol(true);
    session.send(b"exit\r");
    let transcript = session.finish(0);
    assert_eq!(transcript.original_modes, transcript.final_modes);
    assert!(!String::from_utf8_lossy(&transcript.bytes).contains("Invalid number"),
        "real prompt diagnosed an absent or measured duration as invalid");
    0
}

pub fn facade_fallback() -> Option<i32> {
    let executable = std::env::current_exe().unwrap();
    let name = executable.file_name().unwrap().to_str().unwrap();
    if !["fg.exe", "bg.exe", "kill.exe", "suspend.exe"].iter().any(|expected| name.eq_ignore_ascii_case(expected)) {
        return None;
    }
    fs::write(std::env::var_os("FISH_N9_FACADE_MARKER").expect("owned external fallback marker"), name.as_bytes()).unwrap();
    Some(93)
}

pub fn try_main(mode: &str, args: &[OsString]) -> Option<i32> {
    Some(match mode {
        "n9-prompt-duration" => prompt_duration(args),
        "n9-duration-command" => {
            assert!(args.is_empty());
            std::thread::sleep(std::time::Duration::from_millis(150));
            0
        }
        "n9-interrupted-workflow" => interrupted_workflow(args),
        "n9-editor-wait" | "n9-extraction-wait" => {
            assert_eq!(args.len(), 2);
            wait_for_control(Path::new(&args[0]), args[1].to_str().unwrap())
        }
        "n9-save-producer-wait" => {
            assert_eq!(args.len(), 2);
            io::stdout().write_all(b"function saved; echo partial").unwrap();
            io::stdout().flush().unwrap();
            wait_for_control(Path::new(&args[0]), args[1].to_str().unwrap())
        }
        "n9-generator-wait" => {
            assert_eq!(args.len(), 1);
            println!("complete -c n9-interrupted -l unpublished");
            io::stdout().flush().unwrap();
            let stdout = io::stdout();
            let handle = unsafe { BorrowedHandle::borrow_raw(stdout.as_raw_handle()) }.try_clone_to_owned().unwrap();
            let file = fs::File::from(handle);
            let path = fish::wutil::native_final_path(&file).unwrap().to_string();
            wait_for_control(Path::new(&args[0]), &path)
        }
        "n9-editor" => {
            assert_eq!(args.len(), 3, "literal editor argv plus one filename");
            fs::write(&args[0], args[2].to_string_lossy().as_bytes()).unwrap();
            fs::copy(&args[1], &args[2]).unwrap();
            0
        }
        "n9-editor-cancel" => {
            assert_eq!(args.len(), 2);
            fs::write(&args[0], args[1].to_string_lossy().as_bytes()).unwrap();
            7
        }
        "n9-generate-completions" => {
            assert_eq!(args.len(), 2);
            let count = fs::read_to_string(&args[0]).unwrap_or_else(|_| "0".into())
                .parse::<usize>().unwrap();
            fs::write(&args[0], (count + 1).to_string()).unwrap();
            println!("complete -c n9-generated -f -l {} -d 'native generated Unicode 雪'", args[1].to_str().unwrap());
            0
        }
        "n9-generate-failure" => {
            println!("complete -c n9-generated -l partial");
            9
        }
        "n9-extracted-file" => {
            assert_eq!(args.len(), 2);
            // __fish_data_with_directory appends its extracted directory after
            // the caller's fixed arguments (the record path in this mode).
            let directory = Path::new(&args[1]);
            const WEB_ASSETS: &[(&str, &[u8])] = &[
                ("webconfig.py", include_bytes!("../../share/tools/web_config/webconfig.py")),
                ("index.html", include_bytes!("../../share/tools/web_config/index.html")),
                ("fishconfig.css", include_bytes!("../../share/tools/web_config/fishconfig.css")),
                ("fishconfig_print.css", include_bytes!("../../share/tools/web_config/fishconfig_print.css")),
                ("favicon.png", include_bytes!("../../share/tools/web_config/favicon.png")),
                ("js/alpine.js", include_bytes!("../../share/tools/web_config/js/alpine.js")),
                ("js/colorutils.js", include_bytes!("../../share/tools/web_config/js/colorutils.js")),
                ("js/main.js", include_bytes!("../../share/tools/web_config/js/main.js")),
            ];
            for (relative, expected) in WEB_ASSETS {
                assert_eq!(fs::read(directory.join(relative)).unwrap(), *expected, "extracted byte consumer: {relative}");
            }
            fs::write(&args[0], directory.to_string_lossy().as_bytes()).unwrap();
            io::stdout().write_all("extracted 雪\n".as_bytes()).unwrap();
            0
        }
        _ => return None,
    })
}
