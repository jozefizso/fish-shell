use super::{console::Session, editor, fixture::{quote, run, exact}, package::Installed};
use std::{ffi::OsStr, fs};

fn await_buffer(session: &mut Session, f: &super::fixture::Fixture, next: &mut usize, expected: &str) {
    let deadline = std::time::Instant::now() + super::fixture::LIMIT;
    loop {
        let (actual, _) = editor::observe(session, f, *next);
        *next += 1;
        if actual == expected { return; }
        assert!(std::time::Instant::now() < deadline, "reader model never reached {expected:?}: {actual:?}");
    }
}

pub fn exercise(installed: &Installed) {
    let f = &installed.fixture;
    let config = f.state.join("config/fish");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("config.fish"), "function fish_prompt; printf 'N11> '; end\n").unwrap();
    fs::create_dir(f.root.join("navigation 雪")).unwrap();
    let init = editor::init("complete -c journey -f -a 'alpha雪'; set -g fish_history n11_daily; function n11_history_pick; printf '%s' $argv > history-picked; end; builtin history append 'n11_history_pick older' 'n11_history_pick newest'; bind f6 history-pager");
    let mut command = f.command(&[OsStr::new("-i"), OsStr::new("-C"), OsStr::new(&init)]);
    command.env("fish_history", "n11_daily");
    let mut session = Session::start(&command, &f.probe, &f.root);
    session.wait_protocol(true);
    session.wait_text(b"N6> "); // editor::init installs the deterministic fixture prompt.
    let original = session.snapshot();
    assert!(String::from_utf16_lossy(&original.cells).contains("N6>"));
    let mut next = 1;
    session.send("echo 雪🐟x".as_bytes());
    session.send(b"\x7f");
    await_buffer(&mut session, f, &mut next, "echo 雪🐟");
    session.send(format!(" > {}\r", quote(f.root.join("unicode-result").to_str().unwrap())).as_bytes());
    assert_eq!(session.wait_file(&f.root.join("unicode-result")), "雪🐟\n".as_bytes());
    installed.record("daily/typed-unicode-backspace-commandline-output");

    session.send(b"journey al\t");
    await_buffer(&mut session, f, &mut next, "journey alpha雪 ");
    let cells = session.snapshot().cells;
    assert!(String::from_utf16_lossy(&cells).contains("journey alpha雪"));
    fs::write(installed.evidence.join("daily-completion-screen.txt"), String::from_utf16_lossy(&cells).as_bytes()).unwrap();
    session.send(b"\x03");
    installed.record("daily/completion-model-and-native-screen");

    let nav = f.root.join("navigation 雪");
    session.send(format!("cd {}; pwd > {}; cd {}; printf ready > {}\r", quote(nav.to_str().unwrap()), quote(f.root.join("navigation-result").to_str().unwrap()), quote(f.root.to_str().unwrap()), quote(f.root.join("navigation-ready").to_str().unwrap())).as_bytes());
    session.wait_file(&f.root.join("navigation-ready"));
    let observed = fs::read_to_string(f.root.join("navigation-result")).unwrap().replace('\\', "/");
    assert!(observed.trim_end().ends_with("/navigation 雪"));
    installed.record("daily/navigation-real-cwd");

    session.send(b"echo DAILY-HISTORY\r");
    session.wait_text(b"DAILY-HISTORY\r\n");
    session.send(b"\x1b[A");
    await_buffer(&mut session, f, &mut next, "echo DAILY-HISTORY");
    session.send(b"\x03echo DAILY\x1b[A");
    await_buffer(&mut session, f, &mut next, "echo DAILY-HISTORY");
    session.send(b"\x03");
    installed.record("daily/history-up-and-prefix-search");

    session.send(b"n11_history_pick \x1b[17~");
    session.wait_text(b"newest");
    await_buffer(&mut session, f, &mut next, "n11_history_pick newest");
    let pager_snapshot = session.snapshot();
    let pager = String::from_utf16_lossy(&pager_snapshot.cells);
    assert!(pager.contains("older") && pager.contains("newest"));
    fs::write(installed.evidence.join("daily-history-pager-screen.txt"), pager.as_bytes()).unwrap();
    session.send(b"\r");
    await_buffer(&mut session, f, &mut next, "n11_history_pick newest");
    session.send(b"\r");
    assert_eq!(session.wait_file(&f.root.join("history-picked")), b"newest");
    installed.record("daily/history-pager-screen-selection-and-execution");

    session.send(format!("{} marker {}; printf done > {}\r", f.probe(), quote(f.root.join("child-marker").to_str().unwrap()), quote(f.root.join("child-done").to_str().unwrap())).as_bytes());
    session.wait_file(&f.root.join("child-done"));
    assert_eq!(fs::read(f.root.join("child-marker")).unwrap(), b"launched");
    // Bound cancellation by a reader ACK, not by an arbitrary grace-period sleep.
    session.send(format!("printf ready > {}; while true; end\r", quote(f.root.join("cancel-ready").to_str().unwrap())).as_bytes());
    session.wait_file(&f.root.join("cancel-ready"));
    session.control(0);
    session.send(b"\x03");
    await_buffer(&mut session, f, &mut next, "");
    installed.record("daily/native-tool-and-evaluator-cancel-recovery");

    session.send(format!("set -U n11_saved_setting '雪🐟'; builtin history save; printf saved > {}\r", quote(f.root.join("saved").to_str().unwrap())).as_bytes());
    session.wait_file(&f.root.join("saved"));
    let transcript = editor::finish(session);
    assert_eq!(transcript.original_modes, transcript.final_modes);
    fs::write(installed.evidence.join("daily-console.vt"), &transcript.bytes).unwrap();
    let restart = "printf '%s\\n' $n11_saved_setting; builtin history search --exact 'echo DAILY-HISTORY'";
    exact(run(&mut installed.command("fish", &["-c", restart]).env("fish_history", "n11_daily"), b""), 0, "雪🐟\necho DAILY-HISTORY\n".as_bytes(), b"");
    let help = run(&mut installed.command("fish", &["--help"]), b"");
    assert_eq!(help.code, 0);
    assert!(std::str::from_utf8(&help.out).unwrap().contains("--init-command"), "installed help must document the supported init-command workflow");
    fs::write(installed.evidence.join("installed-help.txt"), help.out).unwrap();
    installed.record("daily/save-restart-durable-history-universals-installed-help");
    fs::write(installed.evidence.join("daily-parameters.txt"), "transport=owned ConPTY\ncolumns=100\nrows=30\nkeys=UTF-8 Unicode, backspace, Tab, Up, Ctrl-C, real CTRL_C_EVENT\nhistory=n11_daily\nstate=isolated fresh HOME/XDG roots\nscreen=native CHAR_INFO buffer and VT transcript\nassistive-output=not measured by automation\n").unwrap();
    let clean = std::env::var("FISH_N10_CLEAN_MACHINE").as_deref() == Ok("1");
    fs::write(installed.evidence.join("daily-clean-machine.json"), format!("{{\"externally_provisioned_clean_machine\":{clean}}}\n")).unwrap();
    if clean { installed.record("daily/externally-provisioned-clean-machine"); }
}
