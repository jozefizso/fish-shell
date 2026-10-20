//! N7 observed native grids. Snapshot is ReadConsoleOutputW, never an ANSI echo oracle.
//! The owned host records its legacy/nonspacing policy independently of fish.
//! CHAR_INFO proves logical console cells, not glyph rasterization or a user's font.
use super::{Family, console::{Session, Snapshot}, editor::{capture, finish, start}, fixture::Fixture};
use std::{ffi::{OsStr, c_void}, fs, os::windows::{ffi::OsStrExt, io::{AsRawHandle, FromRawHandle, OwnedHandle}}, process::Command, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};

pub(super) const FAMILIES: &[Family] = &[
    ("unicode-cells-canonical-private-use-cursor", unicode_cells),
    ("silent-read-private-use-one-mask-cell-and-cursor", silent_mask),
    ("wide-selection-native-cell-attributes", selection_cells),
    ("wide-combining-margin-wrap-and-viewport-resize", wrap_resize),
    ("rightprompt-hide-shrink-clear-and-cursor", right_prompt),
    ("style-variable-change-and-reset-observed-cells", styles),
    ("background-child-output-restored-cursor-repaint", background_output),
    ("pager-groups-descriptions-search-filter-narrow-navigation", pager),
    ("pager-short-options-group-observed-cells", pager_groups),
    ("clear-repaint-mode-scrollback-and-vi-cursor-policy", repaint_actions),
    ("unsupported-fragmented-malformed-bounded-replies-keep-keys", replies),
    ("paste-ss3-function-keys-literal-fragmented-close-no-execution", paste_escape_safety),
    ("ss3-overflow-and-zero-stream-bounded-preserve-later-keys", ss3_bounds),
    ("redirected-output-and-cursor-helper-no-unsolicited-controls", redirected),
    ("private-station-clipboard-selection-paste-busy-denied-recovery", clipboard),
];

fn silent_mask(f: &Fixture) {
    let mut s = start(f, "");
    s.send(b"read --silent --nchars 2 --prompt-str 'MASK> ' value; printf '%s' $value > masked-value\r");
    wait_screen(&mut s, "actual silent reader prompt", |screen| contains(screen, "MASK> "));
    s.send("\u{f600}".as_bytes());
    wait_screen(&mut s, "one canonical private-use scalar is one visible mask cell", |screen| {
        screen.cells.chunks_exact(width(screen)).enumerate().any(|(y, row)| {
            row.starts_with(&"MASK> ".encode_utf16().collect::<Vec<_>>())
                && matches!(row[6], 0x2a | 0x2022 | 0x25cf)
                && row[7] == b' ' as u16 && screen.cursor == [7, y as i16]
        })
    });
    assert!(!f.root.join("masked-value").exists(), "one scalar cannot satisfy nchars=2");
    s.send(b"x");
    assert_eq!(s.wait_file(&f.root.join("masked-value")), "\u{f600}x".as_bytes());
    s.send(b"echo after-mask");
    capture(&mut s, f, 1, "echo after-mask");
    wait_screen(&mut s, "outer cursor restored after masked native read", |screen| prompt_cursor(screen, 19));
    finish(s);
}

fn wait_screen(s: &mut Session, why: &str, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let screen = s.snapshot();
        if predicate(&screen) { return screen; }
        assert!(Instant::now() < deadline, "native cell deadline: {why}; cursor={:?}, viewport={:?}, cells={:?}", screen.cursor, screen.viewport, String::from_utf16_lossy(&screen.cells));
        std::thread::yield_now();
    }
}
fn width(screen: &Snapshot) -> usize { screen.buffer_width as usize }
fn prompt_row(screen: &Snapshot) -> Option<usize> {
    let prefix: Vec<_> = "N6> ".encode_utf16().collect();
    screen.cells.chunks_exact(width(screen)).enumerate()
        .filter_map(|(y, row)| row.starts_with(&prefix).then_some(y)).last()
}
fn row(screen: &Snapshot, y: usize) -> &[u16] {
    &screen.cells[y * width(screen)..(y + 1) * width(screen)]
}
fn contains(screen: &Snapshot, text: &str) -> bool {
    let units: Vec<_> = text.encode_utf16().collect();
    screen.cells.chunks_exact(width(screen)).any(|row| row.windows(units.len()).any(|part| part == units))
}
fn prompt_cursor(screen: &Snapshot, columns: usize) -> bool {
    prompt_row(screen).is_some_and(|y| screen.cursor == [(columns % width(screen)) as i16, (y + columns / width(screen)) as i16])
}
fn unicode_cells(f: &Fixture) {
    let mut s = start(f, "set -g fish_ambiguous_width 1; set -g fish_emoji_width 2; set -g fish_autosuggestion_enabled 0; bind f8 'commandline --cursor 10; commandline --column > scalar-column'; bind f9 'commandline --column 10; commandline --cursor > scalar-cursor'");
    let text = "echo e\u{301}界😀\u{f600}Z";
    s.send(text.as_bytes());
    assert_eq!(capture(&mut s, f, 1, text), text.chars().count());
    let screen = wait_screen(&mut s, "decoded Unicode cells and cursor", |screen| {
        let nonspacing = usize::from(screen.nonspacing_width);
        prompt_cursor(screen, 16 + nonspacing) && prompt_row(screen).is_some_and(|y| {
            row(screen, y)[10 + nonspacing] == '界' as u16 && row(screen, y)[14 + nonspacing] == 0xf600 && row(screen, y)[15 + nonspacing] == b'Z' as u16
        })
    });
    assert_eq!(screen.output_mode & 4, 4, "required native VT host policy");
    s.key(true, 1, 0x25, 0, 0, 0);
    s.key(true, 1, 0x25, 0, 0, 0);
    assert_eq!(capture(&mut s, f, 2, text), text.chars().count() - 2);
    wait_screen(&mut s, "cursor crosses literal private-use scalar once", |screen| prompt_cursor(screen, 14 + usize::from(screen.nonspacing_width)));
    s.key(true, 1, 0x77, 0, 0, 0);
    assert_eq!(capture(&mut s, f, 3, text), 10);
    assert_eq!(fs::read(f.root.join("scalar-column")).unwrap(), b"11\n");
    s.key(true, 1, 0x78, 0, 0, 0);
    assert_eq!(capture(&mut s, f, 4, text), 9);
    assert_eq!(fs::read(f.root.join("scalar-cursor")).unwrap(), b"9\n");
    finish(s);
}
fn selection_cells(f: &Fixture) {
    let mut s = start(f, "set -g fish_color_selection --background=red; bind f8 'commandline --cursor 6; commandline -f begin-selection forward-char'; bind f9 end-selection; bind f12 'commandline --current-selection > render-selection; n6_capture'");
    s.send("echo A界B".as_bytes());
    capture(&mut s, f, 1, "echo A界B");
    let before = wait_screen(&mut s, "unselected wide cell", |screen| prompt_cursor(screen, 13));
    let y = prompt_row(&before).unwrap();
    let old = before.attributes[y * width(&before) + 10] & 0xf0;
    s.send(b"\x1b[19~");
    assert_eq!(capture(&mut s, f, 2, "echo A界B"), 7);
    assert_eq!(fs::read(f.root.join("render-selection")).unwrap(), "界".as_bytes());
    let selected = wait_screen(&mut s, "selection covers both wide cells", |screen| {
        prompt_row(screen).is_some_and(|y| {
            let at = y * width(screen) + 10;
            screen.attributes[at] & 0xf0 == 0x40 && screen.attributes[at + 1] & 0xf0 == 0x40
        })
    });
    assert_ne!(old, selected.attributes[prompt_row(&selected).unwrap() * width(&selected) + 10] & 0xf0);
    s.send(b"\x1b[20~");
    capture(&mut s, f, 3, "echo A界B");
    wait_screen(&mut s, "selection style removed", |screen| prompt_row(screen).is_some_and(|y| screen.attributes[y * width(screen) + 10] & 0xf0 == old));
    finish(s);
}
fn wrap_resize(f: &Fixture) {
    let mut s = start(f, "set -e COLUMNS; set -e LINES; set -g fish_autosuggestion_enabled 0; bind f8 'commandline --replace \"\"'");
    s.resize(20, 12);
    let text = format!("{}e\u{301}界Z", "a".repeat(15));
    s.send(text.as_bytes());
    capture(&mut s, f, 1, &text);
    wait_screen(&mut s, "combining last-column base then wide wrap", |screen| {
        let nonspacing = usize::from(screen.nonspacing_width);
        width(screen) == 20 && prompt_cursor(screen, 23 + nonspacing) && prompt_row(screen).is_some_and(|y| {
            row(screen, y)[19] == b'e' as u16 && row(screen, y + 1)[nonspacing] == '界' as u16 && row(screen, y + 1)[2 + nonspacing] == b'Z' as u16
                && (nonspacing == 0 || row(screen, y + 1)[0] == 0x0301)
        })
    });
    s.resize(31, 15);
    capture(&mut s, f, 2, &text);
    wait_screen(&mut s, "resize reconciles model and observed cursor", |screen| width(screen) == 31 && prompt_cursor(screen, 23 + usize::from(screen.nonspacing_width)));
    s.send(b"\x1b[19~");
    let wide_margin = format!("{}界", "b".repeat(25));
    s.send(wide_margin.as_bytes());
    capture(&mut s, f, 3, &wide_margin);
    wait_screen(&mut s, "wide glyph fills margin and cursor enters next line", |screen| prompt_cursor(screen, 31));
    finish(s);
}
fn right_prompt(f: &Fixture) {
    let mut s = start(f, "function fish_right_prompt; printf RIGHT; end; bind f8 'functions --erase fish_right_prompt; commandline -f repaint'; bind f9 'commandline --replace \"\"'; bind f11 'function fish_right_prompt; printf R; end; commandline -f repaint'");
    s.resize(30, 12);
    s.send(b"abc");
    capture(&mut s, f, 1, "abc");
    wait_screen(&mut s, "right prompt alignment leaves editor cursor intact", |screen| {
        width(screen) == 30 && prompt_cursor(screen, 7) && prompt_row(screen).is_some_and(|y| &row(screen, y)[25..30] == &"RIGHT".encode_utf16().collect::<Vec<_>>()[..])
    });
    let text = "x".repeat(24);
    s.send(b"\x1b[20~");
    s.send(text.as_bytes());
    capture(&mut s, f, 2, &text);
    wait_screen(&mut s, "overlapping right prompt erased", |screen| prompt_cursor(screen, 28) && prompt_row(screen).is_some_and(|y| row(screen, y)[28..].iter().all(|c| *c == b' ' as u16)));
    s.send(b"\x1b[20~");
    capture(&mut s, f, 3, "");
    wait_screen(&mut s, "right prompt restored after deletion", |screen| contains(screen, "RIGHT") && prompt_cursor(screen, 4));
    s.send(b"\x1b[23~");
    capture(&mut s, f, 4, "");
    wait_screen(&mut s, "shrinking right prompt erases its previous extent", |screen| {
        prompt_cursor(screen, 4) && prompt_row(screen).is_some_and(|y|
            row(screen, y)[29] == b'R' as u16 && row(screen, y)[25..29].iter().all(|c| *c == b' ' as u16))
    });
    s.send(b"\x1b[19~");
    capture(&mut s, f, 5, "");
    wait_screen(&mut s, "removed right prompt clears native cells", |screen| prompt_cursor(screen, 4) && prompt_row(screen).is_some_and(|y| row(screen, y)[4..].iter().all(|c| *c == b' ' as u16)));
    finish(s);
}
fn styles(f: &Fixture) {
    let mut s = start(f, "set -g fish_color_command red --bold; bind f8 'set -g fish_color_command green; commandline -f repaint'; bind f9 'set -g fish_color_command normal; commandline -f force-repaint'");
    s.send(b"echo style");
    capture(&mut s, f, 1, "echo style");
    let red = wait_screen(&mut s, "red command cells", |screen| prompt_row(screen).is_some_and(|y| screen.attributes[y * width(screen) + 4] & 0x0f == 0x0c));
    let y = prompt_row(&red).unwrap();
    assert_eq!(red.attributes[y * width(&red) + 7] & 0x0f, 0x0c);
    s.send(b"\x1b[19~");
    capture(&mut s, f, 2, "echo style");
    wait_screen(&mut s, "color variable invalidates unchanged text cells", |screen| prompt_row(screen).is_some_and(|y| screen.attributes[y * width(screen) + 4] & 0x0f == 2));
    s.send(b"\x1b[20~");
    capture(&mut s, f, 3, "echo style");
    wait_screen(&mut s, "normal resets bold and command color", |screen| prompt_row(screen).is_some_and(|y| screen.attributes[y * width(screen) + 4] & 0x0f == 7));
    finish(s);
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *mut c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
}
fn background_output(f: &Fixture) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let event = format!("Local\\fish-n7-render-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    let name: Vec<_> = OsStr::new(&event).encode_wide().chain([0]).collect();
    let raw = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, name.as_ptr()) };
    assert!(!raw.is_null(), "owned child-output event: {}", std::io::Error::last_os_error());
    let release = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut s = start(f, &format!("{} n7-render-background render-child-ready '{}' render-child-done &", f.probe(), event.replace('\\', "\\\\")));
    assert_eq!(s.wait_file(&f.root.join("render-child-ready")), b"ready");
    s.send(b"echo preserved-output");
    capture(&mut s, f, 1, "echo preserved-output");
    wait_screen(&mut s, "editor before background output", |screen| prompt_cursor(screen, 25));
    assert_ne!(unsafe { SetEvent(release.as_raw_handle()) }, 0);
    assert_eq!(s.wait_file(&f.root.join("render-child-done")), b"written");
    capture(&mut s, f, 2, "echo preserved-output");
    wait_screen(&mut s, "background output generation repaints restored cursor", |screen| contains(screen, "N6> echo preserved-output") && prompt_cursor(screen, 25));
    finish(s);
}
fn pager(f: &Fixture) {
    let mut s = start(f, "complete -c n7render -f -l alpha -d 'shared description'; complete -c n7render -f -l beta -d 'shared description'; complete -c n7render -f -l gamma -d 'other description'; bind f8 'commandline -f pager-toggle-search'; bind f9 'commandline -f down-line'; bind f10 'commandline -f execute'");
    s.resize(26, 12);
    s.send(b"n7render --\t");
    capture(&mut s, f, 1, "n7render --");
    wait_screen(&mut s, "narrow pager displays actual completion and descriptions", |screen| contains(screen, "alpha") && contains(screen, "shared"));
    s.send(b"\x1b[19~other");
    // Filtering consults actual completion descriptions, not fake completion answers.
    wait_screen(&mut s, "pager description filter and search row", |screen| contains(screen, "search: other") && contains(screen, "gamma"));
    s.send(b"\x15no-such-result");
    wait_screen(&mut s, "no-match filter replaces stale completion cells", |screen| contains(screen, "no matches"));
    s.send(b"\x15other");
    wait_screen(&mut s, "cleared filter republishes description match", |screen| contains(screen, "search: other") && contains(screen, "gamma"));
    s.send(b"\x1b[20~\x1b[21~");
    assert_eq!(capture(&mut s, f, 2, "n7render --gamma "), 17);
    wait_screen(&mut s, "accepted completion removes pager", |screen| contains(screen, "N6> n7render --gamma ") && prompt_cursor(screen, 21));
    finish(s);
}
fn pager_groups(f: &Fixture) {
    let mut s = start(f, "complete -c n7group -f -s a -d shared; complete -c n7group -f -s b -d shared; complete -c n7group -f -s g -d other");
    s.resize(40, 12);
    s.send(b"n7group -\t");
    capture(&mut s, f, 1, "n7group -");
    wait_screen(&mut s, "real short options with shared description form one group", |screen| contains(screen, "-a  -b") && contains(screen, "(shared)"));
    finish(s);
}
fn repaint_actions(f: &Fixture) {
    let mut s = start(f, "function fish_mode_prompt; end; bind f8 'commandline -f clear-screen'; bind f9 'commandline -f repaint force-repaint repaint-mode scrollback-push'; bind f10 'fish_vi_key_bindings insert; bind --mode insert f12 n6_capture; set -g fish_cursor_insert line; fish_vi_cursor; commandline -f repaint-mode'");
    s.send(b"echo repaint");
    capture(&mut s, f, 1, "echo repaint");
    let before = wait_screen(&mut s, "initial repaint cells", |screen| contains(screen, "N6> echo repaint") && prompt_cursor(screen, 16));
    s.send(b"\x1b[19~");
    capture(&mut s, f, 2, "echo repaint");
    let clear = wait_screen(&mut s, "clear-screen returns exact buffer and cursor", |screen| contains(screen, "N6> echo repaint") && prompt_cursor(screen, 16));
    assert_eq!((before.input_mode, before.output_mode), (clear.input_mode, clear.output_mode));
    s.send(b"\x1b[20~");
    capture(&mut s, f, 3, "echo repaint");
    wait_screen(&mut s, "repaint and unsupported scrollback fallback retain cells", |screen| contains(screen, "N6> echo repaint") && prompt_cursor(screen, 16));
    s.send(b"\x1b[21~");
    capture(&mut s, f, 4, "echo repaint");
    wait_screen(&mut s, "vi cursor policy retains native cell cursor", |screen| contains(screen, "N6> echo repaint") && prompt_cursor(screen, 16));
    finish(s);
}
fn compatibility_records(s: &mut Session, bytes: &[u8]) {
    let records: Vec<_> = bytes.iter().map(|&byte| (true, 1, 0, 0, u16::from(byte), 0)).collect();
    s.key_records(&records);
}
fn paste_escape_safety(f: &Fixture) {
    let mut s = start(f, "bind f1 'printf fired > paste-function-executed'; bind f3 undo; set -g fish_escape_delay_ms 1000");
    let original = "printf unsafe > paste-should-not-execute ";
    s.send(original.as_bytes());
    capture(&mut s, f, 1, original);
    // ESC O M is Enter under general SS3 decoding; ESC O P is the bound F1.
    // Neither may become a key/binding while enclosed in committed paste text.
    compatibility_records(&mut s, b"\x1b[200~\x1bOM\x1bOP\x1b[13u\x1b]99:data\x07\x1b[200~\x1b[201Xtail");
    // Exercise the same framing path through a committed physical Escape record.
    s.key(true, 1, 0x1b, 0, 0x1b, 0);
    compatibility_records(&mut s, b"OM\x1b[20");
    s.resize(43, 13); // actual resize notification while a closing envelope is partial
    s.wait_input_drained();
    assert!(!f.root.join("paste-should-not-execute").exists());
    assert!(!f.root.join("paste-function-executed").exists());
    compatibility_records(&mut s, b"1~");
    let expected = format!("{original}OMOP[13u]99:data[200~[201XtailOM");
    assert_eq!(capture(&mut s, f, 2, &expected), expected.chars().count());
    assert!(!f.root.join("paste-should-not-execute").exists());
    assert!(!f.root.join("paste-function-executed").exists());
    // The real paste helper's retained control-byte sanitization is expected;
    // preservation here means literal data, never hidden binding execution.
    s.send(b"\x1bOR");
    capture(&mut s, f, 3, original);
    finish(s);
}
fn ss3_bounds(f: &Fixture) {
    let mut s = start(f, "");
    compatibility_records(&mut s, b"\x1bO4294967296ss3");
    assert_eq!(capture(&mut s, f, 1, "ss3"), 3);
    let mut oversized = vec![0x1b, b'O'];
    oversized.resize(4096, b'0');
    oversized.extend_from_slice(b"-bounded");
    compatibility_records(&mut s, &oversized);
    assert_eq!(capture(&mut s, f, 2, "ss3-bounded"), 11);
    wait_screen(&mut s, "SS3 rejection preserves later native keys and cursor", |screen|
        contains(screen, "N6> ss3-bounded") && prompt_cursor(screen, 15));
    finish(s);
}
fn replies(f: &Fixture) {
    let mut s = start(f, "set -g fish_escape_delay_ms 1000");
    compatibility_records(&mut s, b"\x1b]99;unsupported");
    s.snapshot(); // ordered fixture round-trip between fragments, no sleeps
    compatibility_records(&mut s, b"\x07keep");
    assert_eq!(capture(&mut s, f, 1, "keep"), 4);
    compatibility_records(&mut s, b"\x1b]11;not-a-color\x1b\\-keys");
    assert_eq!(capture(&mut s, f, 2, "keep-keys"), 9);
    let mut oversized = vec![0x1b, b']'];
    oversized.resize(4096, b'x');
    oversized.extend_from_slice(b"-bounded");
    compatibility_records(&mut s, &oversized);
    assert_eq!(capture(&mut s, f, 3, "keep-keys-bounded"), 17);
    wait_screen(&mut s, "unsupported reports never lose subsequent user keys", |screen| contains(screen, "N6> keep-keys-bounded") && prompt_cursor(screen, 21));
    let t = finish(s);
    for query in [&b"\x1b[6n"[..], &b"\x1b[0c"[..], &b"\x1b]11;?"[..], &b"\x1b[?u"[..]] {
        assert!(!t.bytes.windows(query.len()).any(|part| part == query), "native direct-query fallback must not emit unsupported VT query {query:?}");
    }
}
fn redirected(f: &Fixture) {
    let r = f.eval("printf '%s' 'plain 界😀'; fish_vi_cursor; __fish_vi_cursor fish_cursor_insert; printf done");
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.err));
    assert_eq!(r.out, "plain 界😀done".as_bytes());
    assert!(!r.err.contains(&0x1b));
    let r = f.eval("__fish_cursor_xterm block; printf '%s' $status");
    assert!(!r.out.contains(&0x1b), "redirected helper cannot emit cursor controls");
    assert_eq!(r.out, b"1");
}
fn clipboard(f: &Fixture) {
    // Retain the N6 real fresh-logon/private-station clipboard proof. Never read/write
    // the developer's desktop clipboard and never turn denied isolation into a skip.
    let r = super::fixture::run(&mut {
        let mut command = Command::new(&f.probe);
        command.arg("n6-clipboard-launch").arg(&f.root).arg(&f.fish).arg(&f.probe);
        command
    }, b"");
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.err));
    assert_eq!(fs::read(f.root.join("clipboard-passed")).unwrap(), b"pipe-selection-line-paste-private");
}
