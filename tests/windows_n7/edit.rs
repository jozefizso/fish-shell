//! Required editor inventory, exercised against the owning reader via console input.
//! Optional manual/browser/VCS/external-editor integrations are deliberately not
//! part of this inventory. Clipboard and observed repaint policy belong to render.
use super::{Family, console::Session, editor::{start, capture, finish, init}, fixture::Fixture};
use std::{ffi::OsStr, fs};

pub(super) const FAMILIES: &[Family] = &[
    ("ordinary-f2-unicode-edit-output-and-mode-restoration", ordinary_input),
    ("abbr-unicode-literal-position-and-command-scope", abbr_scope),
    ("abbr-regex-function-cursor-and-local-scope", abbr_function),
    ("abbr-invalid-definitions-callback-failure-recovery", abbr_errors),
    ("abbr-runtime-regex-diagnostic-and-recovery", abbr_runtime_error),
    ("abbr-delimiters-multiline-and-undo", abbr_undo),
    ("history-line-prefix-smartcase-return-original", history_lines),
    ("history-token-last-token-and-delete", history_tokens),
    ("history-private-session-and-switch-isolation", history_private),
    ("history-session-clear-and-utility-free-display", history_session),
    ("character-buffer-line-and-passive-navigation", navigation),
    ("word-bigword-token-and-path-navigation", words),
    ("suggestion-literal-reserved-scalar-accept-undo-suppress", suggestion_scalar),
    ("kill-word-token-path-and-line-actions", kills),
    ("inner-and-a-word-text-objects", text_objects),
    ("case-transpose-and-unicode-actions", transforms),
    ("selection-bounds-swap-case-kill-and-yank", selections),
    ("undo-groups-redo-and-yank-pop", undo_yank),
    ("jump-till-repeat-reverse-and-brackets", jumps),
    ("multiline-open-line-and-up-down", multiline),
    ("default-physical-bindings-and-user-precedence", default_keys),
    ("vi-count-operator-change-and-cancel", vi_operators),
    ("vi-linewise-swap-case-and-counted-undo", vi_linewise),
    ("vi-text-objects-and-operator-character-jumps", vi_objects),
    ("vi-visual-replace-and-mode-transitions", vi_modes),
    ("vi-number-final-character-and-zero-padding", vi_numbers),
    ("hybrid-inherited-emacs-and-vi-transitions", hybrid),
    ("binding-conditionals-self-insert-get-key-and-exit", binding_flow),
    ("commandline-comment-prefix-and-native-list-helpers", helpers),
    ("native-directory-history-empty-buffer-bindings", directory_history),
];

// This is an action-to-behavior inventory, not an existence test. Each row names
// the concrete family exercising its state transition. Completion/history pager
// publication is covered by complete.rs; screen/clipboard capability by render.rs.
pub(super) const ACTION_INVENTORY: &[(&str, &str)] = &[
    ("backward-char backward-char-passive forward-char forward-char-passive forward-single-char beginning-of-buffer end-of-buffer beginning-of-line end-of-line", "character-buffer-line-and-passive-navigation"),
    ("backward-word backward-word-end backward-bigword backward-bigword-end forward-word forward-word-vi forward-word-end forward-bigword forward-bigword-vi forward-bigword-end forward-token backward-token forward-path-component backward-path-component", "word-bigword-token-and-path-navigation"),
    ("backward-kill-bigword backward-kill-line backward-kill-path-component backward-kill-token backward-kill-word backward-delete-char delete-char kill-bigword kill-bigword-vi kill-line kill-inner-line kill-path-component kill-token kill-whole-line kill-word kill-word-vi", "kill-word-token-path-and-line-actions"),
    ("kill-inner-word kill-inner-bigword kill-a-word kill-a-bigword", "inner-and-a-word-text-objects"),
    ("capitalize-word downcase-word upcase-word togglecase-char transpose-chars transpose-words", "case-transpose-and-unicode-actions"),
    ("begin-selection end-selection swap-selection-start-stop downcase-selection upcase-selection togglecase-selection kill-selection yank", "selection-bounds-swap-case-kill-and-yank"),
    ("begin-undo-group end-undo-group undo redo yank-pop", "undo-groups-redo-and-yank-pop"),
    ("forward-jump backward-jump forward-jump-till backward-jump-till repeat-jump repeat-jump-reverse jump-to-matching-bracket jump-till-matching-bracket", "jump-till-repeat-reverse-and-brackets"),
    ("insert-line-over insert-line-under up-line down-line", "multiline-open-line-and-up-down"),
    ("history-search-backward history-search-forward history-prefix-search-backward history-prefix-search-forward beginning-of-history end-of-history cancel", "history-line-prefix-smartcase-return-original"),
    ("history-token-search-backward history-token-search-forward history-last-token-search-backward history-last-token-search-forward history-delete", "history-token-last-token-and-delete"),
    ("self-insert self-insert-notfirst get-key and or execute exit delete-or-exit clear-commandline cancel-commandline", "binding-conditionals-self-insert-get-key-and-exit"),
    ("expand-abbr", "abbr-unicode-literal-position-and-command-scope"),
    ("up-or-search down-or-search", "default-physical-bindings-and-user-precedence"),
    ("nextd-or-forward-word prevd-or-backward-word", "native-directory-history-empty-buffer-bindings"),
    ("accept-autosuggestion suppress-autosuggestion", "suggestion-literal-reserved-scalar-accept-undo-suppress"),
    ("complete complete-and-search history-pager history-pager-delete pager-toggle-search", "N7 completion-context console families"),
    ("clear-screen repaint force-repaint repaint-mode scrollback-push", "N7 console-layout console families"),
];
pub(super) const HELPER_INVENTORY: &[(&str, &str)] = &[
    ("fish_default_key_bindings __fish_shared_key_bindings __fish_change_key_bindings", "default-physical-bindings-and-user-precedence"),
    ("fish_vi_key_bindings fish_vi_arg_digit __fish_vi_consume_count __fish_vi_sequence fish_vi_run_count fish_vi_start_operator fish_vi_exec_motion fish_vi_operator_cancel __fish_vi_delete_char __fish_vi_backward_delete_char fish_vi_yank_selection", "vi-count-operator-change-and-cancel"),
    ("fish_vi_inc fish_vi_dec fish_vi_inc_dec", "vi-number-final-character-and-zero-padding"),
    ("fish_hybrid_key_bindings", "hybrid-inherited-emacs-and-vi-transitions"),
    ("__fish_vi_key_bindings_on_mode_change __fish_vi_key_bindings_remove_handlers", "hybrid-inherited-emacs-and-vi-transitions"),
    ("up-or-search down-or-search history", "history-session-clear-and-utility-free-display"),
    ("fish_commandline_prepend fish_commandline_append __fish_toggle_comment_commandline __fish_list_current_token __fish_echo prevd-or-backward-token nextd-or-forward-token", "commandline-comment-prefix-and-native-list-helpers"),
    ("fish_clipboard_copy fish_clipboard_paste __fish_paste fish_vi_cursor __fish_vi_cursor fish_vi_cursor_handle fish_vi_cursor_handle_preexec __fish_cursor_xterm", "N7 console-layout capability and paste families"),
];

fn ordinary_input(f: &Fixture) {
    let mut s = start(f, "bind f2 'printf entered > entered; commandline --insert 雪🐟'");
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("entered")), b"entered");
    assert_eq!(capture(&mut s, f, 1, "雪🐟"), 2);
    s.key(true, 1, 0x25, 0, 0, 0);
    s.key(true, 1, 0x08, 0, 8, 0);
    assert_eq!(capture(&mut s, f, 2, "🐟"), 0);
    s.send(b"\x03printf baseline > result\r");
    assert_eq!(s.wait_file(&f.root.join("result")), b"baseline");
    finish(s);
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn f2(s: &mut Session) { s.key(true, 1, 0x71, 0, 0, 0); }
fn f3(s: &mut Session) { s.key(true, 1, 0x72, 0, 0, 0); }
fn f4(s: &mut Session) { s.key(true, 1, 0x73, 0, 0, 0); }
fn mode(f: &Fixture, n: usize, expected: &str) {
    assert_eq!(fs::read(f.root.join(format!("mode-{n}"))).unwrap(), expected.as_bytes());
}
// Capture selection before the inherited capture closes its acknowledgement.
const SELECT_CAPTURE: &str = "functions -c n6_capture n7_capture_base; function n6_capture; set -l n (math $n6_capture + 1); commandline --current-selection | string collect -N > selection-$n; commandline --selection-start > selection-start-$n; commandline --selection-end > selection-end-$n; n7_capture_base; end";
const MODES_CAPTURE: &str = "for mode in insert default visual operator replace replace_one f F t T; bind -M $mode f12 n6_capture; end; function n7_mode_ready --on-variable fish_bind_mode; if test \"$fish_bind_mode\" = default; printf ready > vi-default-ready; end; end";
fn vi(f: &Fixture, extra: &str) -> Session {
    start(f, &format!("set -g fish_autosuggestion_enabled 0; fish_vi_key_bindings; {MODES_CAPTURE}; {SELECT_CAPTURE}; {extra}"))
}
fn escape(s: &mut Session, f: &Fixture, n: usize, text: &str) {
    // Wait for mode dispatch before sending CSI bytes: immediate Escape+F12
    // is a legitimate Alt-F12 sequence, not a mode-transition barrier.
    let ready = f.root.join("vi-default-ready");
    match fs::remove_file(&ready) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("removing owned mode barrier: {error}"),
    }
    s.key(true, 1, 0x1b, 0, 0x1b, 0);
    assert_eq!(s.wait_file(&ready), b"ready");
    capture(s, f, n, text);
    mode(f, n, "default");
}
struct Case(&'static str, usize, &'static str, &'static str, Option<usize>);
fn cases(f: &Fixture, rows: &[Case]) {
    let mut setup = String::from("set -g fish_autosuggestion_enabled 0; set -g n7_index 0; function n7_case; set -g n7_index (math $n7_index + 1); switch $n7_index;");
    for (i, Case(before, cursor, action, _, _)) in rows.iter().enumerate() {
        setup.push_str(&format!("case {}; commandline --replace -- {}; commandline --cursor {}; commandline -f {};", i + 1, quote(before), cursor, action));
    }
    setup.push_str("end; end; bind f2 n7_case;");
    let mut s = start(f, &setup);
    for (i, Case(_, _, action, after, cursor)) in rows.iter().enumerate() {
        f2(&mut s);
        let observed_cursor = capture(&mut s, f, i + 1, after);
        if let Some(cursor) = cursor {
            assert_eq!(observed_cursor, *cursor, "{action}");
        }
    }
    finish(s);
}
fn abbr_scope(f: &Fixture) {
    let mut s = start(f, "abbr -a 雪 'printf 雪🐟'; abbr -a --position anywhere --command printf arg 'Ω'; abbr -a --position anywhere zz '漢字'; bind f2 expand-abbr");
    s.send("雪 ".as_bytes());
    assert_eq!(capture(&mut s, f, 1, "printf 雪🐟 "), 10);
    s.send("arg ".as_bytes());
    capture(&mut s, f, 2, "printf 雪🐟 Ω ");
    s.send("\x03echo arg zz ".as_bytes());
    capture(&mut s, f, 3, "echo arg 漢字 ");
    s.send("\x03echo 雪 ".as_bytes());
    capture(&mut s, f, 4, "echo 雪 ");
    s.send(b"\x03'zz' ");
    capture(&mut s, f, 5, "'zz' ");
    s.send(b"\x03zz"); f2(&mut s);
    capture(&mut s, f, 6, "漢字");
    finish(s);
}
fn abbr_function(f: &Fixture) {
    let extra = "set -g n7_scope outer; function n7_replace; set -l n7_scope local; printf '%s:%s:雪@🐟' $argv[1] $n7_scope; end; abbr -a --position anywhere --regex 'r[0-9]+' --function n7_replace --set-cursor=@ regex; bind f2 'printf %s $n7_scope > scope-after'";
    let mut s = start(f, extra);
    s.send(b"echo r42 ");
    assert_eq!(capture(&mut s, f, 1, "echo r42:local:雪🐟 "), 16);
    f2(&mut s);
    assert_eq!(s.wait_file(&f.root.join("scope-after")), b"outer");
    s.send(b"\x03echo xr42 ");
    capture(&mut s, f, 2, "echo xr42 ");
    s.send(b"\x03echo r7 ");
    assert_eq!(capture(&mut s, f, 3, "echo r7:local:雪🐟 "), 15);
    finish(s);
}
fn abbr_errors(f: &Fixture) {
    let mut s = start(f, "abbr -a --regex '[' malformed x 2> malformed-error; printf %s $status > malformed-status; abbr -a --set-cursor= empty x 2> marker-error; printf %s $status > marker-status; function n7_fail; printf rejected; return 7; end; abbr -a --position anywhere bad unchanged; abbr -a --position anywhere --regex bad --function n7_fail fail; abbr -a --position anywhere --function n7_missing absent; abbr -a --position anywhere good recovered");
    assert_ne!(s.wait_file(&f.root.join("malformed-status")), b"0");
    assert_ne!(s.wait_file(&f.root.join("marker-status")), b"0");
    assert!(!fs::read(f.root.join("malformed-error")).unwrap().is_empty());
    assert!(!fs::read(f.root.join("marker-error")).unwrap().is_empty());
    s.send(b"echo bad absent good ");
    capture(&mut s, f, 1, "echo unchanged absent recovered ");
    s.send(b"\x03printf '%s' healthy > recovered-runtime\r");
    assert_eq!(s.wait_file(&f.root.join("recovered-runtime")), b"healthy");
    finish(s);
}
fn abbr_undo(f: &Fixture) {
    let mut s = start(f, "abbr -a --set-cursor=@ multi 'begin\n雪@\nend'; abbr -a z printf; bind f3 undo; bind f4 redo");
    s.send(b"multi ");
    assert_eq!(capture(&mut s, f, 1, "begin\n雪\nend "), 7);
    f3(&mut s);
    capture(&mut s, f, 2, "multi ");
    f4(&mut s);
    capture(&mut s, f, 3, "begin\n雪\nend ");
    s.send(b"\x03z;"); capture(&mut s, f, 4, "printf;");
    s.send(b"\x03z|"); capture(&mut s, f, 5, "printf|");
    s.send(b"\x03z&"); capture(&mut s, f, 6, "printf&");
    s.send(b"\x03z>"); capture(&mut s, f, 7, "printf>");
    s.send(b"\x03z<"); capture(&mut s, f, 8, "printf<");
    finish(s);
}
fn history_lines(f: &Fixture) {
    let mut s = start(f, "set -g fish_autosuggestion_enabled 0; builtin history clear; builtin history append 'echo 雪 Alpha' 'echo 雪 alpha-two' 'echo other'; bind f2 history-search-backward; bind f3 history-search-forward; bind f4 history-prefix-search-backward; bind f5 history-prefix-search-forward; bind f6 beginning-of-history; bind f7 end-of-history; bind f8 cancel");
    s.send("雪 a".as_bytes()); f2(&mut s);
    capture(&mut s, f, 1, "echo 雪 alpha-two");
    f2(&mut s); capture(&mut s, f, 2, "echo 雪 Alpha");
    f3(&mut s); capture(&mut s, f, 3, "echo 雪 alpha-two");
    f3(&mut s); assert_eq!(capture(&mut s, f, 4, "雪 a"), 3);
    s.send("\x03雪 A".as_bytes()); f2(&mut s);
    capture(&mut s, f, 5, "echo 雪 Alpha");
    f2(&mut s); capture(&mut s, f, 6, "echo 雪 Alpha");
    s.key(true, 1, 0x77, 0, 0, 0); // F8 cancel restores original edit
    capture(&mut s, f, 7, "雪 A");
    s.send(b"\x03echo "); f4(&mut s);
    capture(&mut s, f, 8, "echo other");
    f4(&mut s); capture(&mut s, f, 9, "echo 雪 alpha-two");
    s.key(true, 1, 0x74, 0, 0, 0);
    capture(&mut s, f, 10, "echo other");
    s.key(true, 1, 0x75, 0, 0, 0); // F6 oldest
    capture(&mut s, f, 11, "echo 雪 Alpha");
    s.key(true, 1, 0x76, 0, 0, 0); // F7 present
    capture(&mut s, f, 12, "echo ");
    finish(s);
}
fn history_tokens(f: &Fixture) {
    let mut s = start(f, "set -g fish_autosuggestion_enabled 0; builtin history clear; builtin history append 'echo 雪-one 雪-two' 'echo 雪-three'; bind f2 history-token-search-backward; bind f3 history-token-search-forward; bind f4 history-last-token-search-backward; bind f5 history-last-token-search-forward; bind f6 history-search-backward; bind f7 history-delete");
    s.send("printf 雪".as_bytes()); f2(&mut s);
    capture(&mut s, f, 1, "printf 雪-three");
    f2(&mut s); capture(&mut s, f, 2, "printf 雪-two");
    f2(&mut s); capture(&mut s, f, 3, "printf 雪-one");
    f3(&mut s); capture(&mut s, f, 4, "printf 雪-two");
    s.send("\x03printf 雪".as_bytes()); f4(&mut s);
    capture(&mut s, f, 5, "printf 雪-three");
    f4(&mut s); capture(&mut s, f, 6, "printf 雪-two");
    s.key(true, 1, 0x74, 0, 0, 0); // F5 forward
    capture(&mut s, f, 7, "printf 雪-three");
    s.send("\x03雪".as_bytes());
    s.key(true, 1, 0x75, 0, 0, 0);
    capture(&mut s, f, 8, "echo 雪-three");
    s.key(true, 1, 0x76, 0, 0, 0);
    capture(&mut s, f, 9, "echo 雪-one 雪-two");
    finish(s);
}
fn public_history(f: &Fixture, extra: &str) -> Session {
    // Native -N deliberately selects memory-only history (the inherited N4
    // contract). Use isolated real configuration roots for persisted sessions.
    let setup = init(extra);
    let cmd = f.command(&[OsStr::new("-i"), OsStr::new("-C"), OsStr::new(&setup)]);
    let mut session = Session::start(&cmd, &f.probe, &f.root);
    session.wait_protocol(true);
    session
}

fn history_private(f: &Fixture) {
    let r = super::fixture::run(&mut f.command(&[OsStr::new("-c"), OsStr::new("builtin history append 'echo public-雪'; builtin history save")]), b"");
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.err));
    let setup = init("set -g fish_autosuggestion_enabled 0; bind f2 history-search-backward; builtin history append 'echo private-🐟'; printf ready > private-ready");
    let cmd = f.command(&[OsStr::new("-N"), OsStr::new("-i"), OsStr::new("--private"), OsStr::new("-C"), OsStr::new(&setup)]);
    let mut private = Session::start(&cmd, &f.probe, &f.root);
    private.wait_protocol(true);
    private.wait_file(&f.root.join("private-ready"));
    private.send(b"private-"); f2(&mut private);
    capture(&mut private, f, 1, "echo private-🐟");
    private.send(b"\x03public-"); f2(&mut private);
    capture(&mut private, f, 2, "public-");
    let mut public = public_history(f, "set -g n6_capture 2; set -g fish_autosuggestion_enabled 0; bind f2 history-search-backward; bind f3 'set -g fish_history n7_other; printf %s $fish_history > switched-history'; bind f4 'set -g fish_history fish'");
    public.send(b"public-"); f2(&mut public);
    capture(&mut public, f, 3, "echo public-雪");
    // Switch identity while an old-history transient match is still active.
    // Returning the original edit prevents retained matches/Arc state leaking.
    f3(&mut public);
    assert_eq!(public.wait_file(&f.root.join("switched-history")), b"n7_other");
    f2(&mut public);
    capture(&mut public, f, 4, "public-");
    f4(&mut public); f2(&mut public);
    capture(&mut public, f, 5, "echo public-雪");
    public.send(b"\x03private-"); f2(&mut public);
    capture(&mut public, f, 6, "private-");
    finish(public);
    finish(private);
}
fn history_session(f: &Fixture) {
    let seeded = super::fixture::run(&mut f.command(&[OsStr::new("-c"), OsStr::new("builtin history clear; builtin history append 'echo saved-雪'; builtin history save")]), b"");
    assert_eq!(seeded.code, 0, "{}", String::from_utf8_lossy(&seeded.err));
    let action = "builtin history append 'echo session-🐟'; history --max=10; printf %s $status > history-display-status; builtin history clear-session; builtin history search --contains session- > session-after; builtin history search --contains saved- > saved-after; printf ready > history-ready";
    let mut s = public_history(f, &format!("bind f2 {}", quote(action)));
    f2(&mut s);
    s.wait_file(&f.root.join("history-ready"));
    assert_eq!(fs::read(f.root.join("history-display-status")).unwrap(), b"0");
    assert!(fs::read(f.root.join("session-after")).unwrap().is_empty());
    assert_eq!(fs::read(f.root.join("saved-after")).unwrap(), "echo saved-雪\n".as_bytes());
    capture(&mut s, f, 1, "");
    finish(s);
}

fn navigation(f: &Fixture) { cases(f, &[
    Case("雪🐟abc", 5, "backward-char", "雪🐟abc", Some(4)),
    Case("雪🐟abc", 1, "forward-char", "雪🐟abc", Some(2)),
    Case("abc", 2, "backward-char-passive", "abc", Some(1)),
    Case("abc", 1, "forward-char-passive", "abc", Some(2)),
    Case("abc", 1, "forward-single-char", "abc", Some(2)),
    Case("one\ntwo", 5, "beginning-of-buffer", "one\ntwo", Some(0)),
    Case("one\ntwo", 1, "end-of-buffer", "one\ntwo", Some(7)),
    Case("one\ntwo", 5, "beginning-of-line", "one\ntwo", Some(4)),
    Case("one\ntwo", 1, "end-of-line", "one\ntwo", Some(3)),
]); }
fn words(f: &Fixture) { cases(f, &[
    Case("one two", 7, "backward-word", "one two", Some(4)),
    Case("one two", 0, "forward-word", "one two", Some(3)),
    Case("one two", 0, "forward-word-vi", "one two", Some(4)),
    Case("one two", 0, "forward-word-end", "one two", Some(2)),
    Case("one two", 6, "backward-word-end", "one two", Some(2)),
    Case("one-two three", 13, "backward-bigword", "one-two three", Some(8)),
    Case("one-two three", 0, "forward-bigword", "one-two three", Some(7)),
    Case("one-two three", 0, "forward-bigword-vi", "one-two three", Some(8)),
    Case("one-two three", 0, "forward-bigword-end", "one-two three", Some(6)),
    Case("one-two three", 12, "backward-bigword-end", "one-two three", Some(6)),
    Case("echo 'two words' tail", 0, "forward-token", "echo 'two words' tail", Some(4)),
    Case("echo 'two words' tail", 16, "backward-token", "echo 'two words' tail", Some(5)),
    Case("one/two/three", 13, "backward-path-component", "one/two/three", Some(8)),
    Case("one/two/three", 0, "forward-path-component", "one/two/three", Some(4)),
    Case("雪🐟 Ωmega", 8, "backward-word", "雪🐟 Ωmega", Some(3)),
    Case("one two", 7, "prevd-or-backward-word", "one two", Some(4)),
    Case("one two", 0, "nextd-or-forward-word", "one two", Some(3)),
]); }
fn kills(f: &Fixture) { cases(f, &[
    Case("one two", 0, "kill-word", " two", Some(0)),
    Case("one two", 0, "kill-word-vi", "two", Some(0)),
    Case("one two", 7, "backward-kill-word", "one ", Some(4)),
    Case("one-two three", 0, "kill-bigword", " three", Some(0)),
    Case("one-two three", 0, "kill-bigword-vi", "three", Some(0)),
    Case("one-two three", 13, "backward-kill-bigword", "one-two ", Some(8)),
    Case("one/two/three", 13, "backward-kill-path-component", "one/two/", Some(8)),
    Case("one/two/three", 0, "kill-path-component", "two/three", Some(0)),
    Case("echo 'two words' tail", 5, "kill-token", "echo  tail", Some(5)),
    Case("echo 'two words' tail", 16, "backward-kill-token", "echo  tail", Some(5)),
    Case("one\ntwo", 1, "kill-line", "o\ntwo", Some(1)),
    Case("one\ntwo", 3, "kill-line", "onetwo", Some(3)),
    Case("one\ntwo", 6, "backward-kill-line", "one\no", Some(4)),
    Case("one\ntwo", 1, "kill-whole-line", "two", Some(0)),
    Case("one\ntwo", 5, "kill-inner-line", "one\n", Some(4)),
    Case("雪🐟a", 1, "delete-char", "雪a", Some(1)),
    Case("雪🐟a", 2, "backward-delete-char", "雪a", Some(1)),
]); }
fn text_objects(f: &Fixture) { cases(f, &[
    Case("one two three", 5, "kill-inner-word", "one  three", Some(4)),
    Case("one two three", 5, "kill-a-word", "one three", Some(4)),
    Case("one two-three four", 6, "kill-inner-bigword", "one  four", Some(4)),
    Case("one two-three four", 6, "kill-a-bigword", "one four", Some(4)),
]); }
fn transforms(f: &Fixture) { cases(f, &[
    Case("snow fish", 0, "upcase-word", "SNOW fish", None),
    Case("SNOW FISH", 0, "downcase-word", "snow FISH", None),
    Case("sNOW fish", 0, "capitalize-word", "Snow fish", None),
    Case("aBc", 1, "togglecase-char", "abc", Some(1)),
    Case("ab", 2, "transpose-chars", "ba", Some(2)),
    Case("雪🐟", 2, "transpose-chars", "🐟雪", Some(2)),
    Case("one two", 7, "transpose-words", "two one", Some(7)),
]); }
fn selections(f: &Fixture) {
    let mut s = start(f, &format!("{SELECT_CAPTURE}; bind f2 'commandline -r aBcDe; commandline -C 1; commandline -f begin-selection forward-char forward-char'; bind f3 swap-selection-start-stop; bind f4 upcase-selection; bind f5 downcase-selection; bind f6 togglecase-selection; bind f7 kill-selection; bind f8 yank; bind f9 end-selection"));
    f2(&mut s); assert_eq!(capture(&mut s, f, 1, "aBcDe"), 3);
    assert_eq!(fs::read(f.root.join("selection-1")).unwrap(), b"Bc");
    assert_eq!(fs::read(f.root.join("selection-start-1")).unwrap(), b"1\n");
    assert_eq!(fs::read(f.root.join("selection-end-1")).unwrap(), b"3\n");
    f3(&mut s); assert_eq!(capture(&mut s, f, 2, "aBcDe"), 1);
    f4(&mut s); capture(&mut s, f, 3, "aBCDe");
    s.key(true, 1, 0x74, 0, 0, 0); capture(&mut s, f, 4, "abcDe");
    s.key(true, 1, 0x75, 0, 0, 0); capture(&mut s, f, 5, "aBCDe");
    s.key(true, 1, 0x76, 0, 0, 0); capture(&mut s, f, 6, "aDe");
    s.key(true, 1, 0x78, 0, 0, 0); // end-selection before yank
    s.key(true, 1, 0x77, 0, 0, 0); capture(&mut s, f, 7, "aBCDe");
    finish(s);
}
fn undo_yank(f: &Fixture) {
    let mut s = start(f, "bind f2 'commandline -f begin-undo-group; commandline -i 雪; commandline -i 🐟; commandline -f end-undo-group'; bind f3 undo; bind f4 redo; bind f5 'commandline -r one; commandline -C 0; commandline -f kill-line'; bind f6 'commandline -r two; commandline -C 0; commandline -f kill-line'; bind f7 yank; bind f8 yank-pop");
    f2(&mut s); capture(&mut s, f, 1, "雪🐟");
    f3(&mut s); capture(&mut s, f, 2, "");
    f4(&mut s); capture(&mut s, f, 3, "雪🐟");
    s.key(true, 1, 0x74, 0, 0, 0); capture(&mut s, f, 4, "");
    s.key(true, 1, 0x75, 0, 0, 0); capture(&mut s, f, 5, "");
    s.key(true, 1, 0x76, 0, 0, 0); capture(&mut s, f, 6, "two");
    s.key(true, 1, 0x77, 0, 0, 0); capture(&mut s, f, 7, "one");
    finish(s);
}
fn jumps(f: &Fixture) {
    let mut s = start(f, "bind f2 'commandline -r axbxcxd; commandline -C 0' forward-jump; bind f3 repeat-jump; bind f4 repeat-jump-reverse; bind f5 'commandline -C 6' backward-jump; bind f6 'commandline -C 0' forward-jump-till; bind f7 'commandline -C 6' backward-jump-till; bind f8 'commandline -r \"(a[b]c)\"; commandline -C 0; commandline -f jump-to-matching-bracket'; bind f9 jump-till-matching-bracket");
    f2(&mut s); s.send(b"x"); assert_eq!(capture(&mut s, f, 1, "axbxcxd"), 1);
    f3(&mut s); assert_eq!(capture(&mut s, f, 2, "axbxcxd"), 3);
    f4(&mut s); assert_eq!(capture(&mut s, f, 3, "axbxcxd"), 1);
    s.key(true, 1, 0x74, 0, 0, 0); s.send(b"x"); assert_eq!(capture(&mut s, f, 4, "axbxcxd"), 5);
    s.key(true, 1, 0x75, 0, 0, 0); s.send(b"x"); assert_eq!(capture(&mut s, f, 5, "axbxcxd"), 0);
    s.key(true, 1, 0x76, 0, 0, 0); s.send(b"x"); assert_eq!(capture(&mut s, f, 6, "axbxcxd"), 6);
    s.key(true, 1, 0x77, 0, 0, 0); assert_eq!(capture(&mut s, f, 7, "(a[b]c)"), 6);
    s.key(true, 1, 0x78, 0, 0, 0); assert_eq!(capture(&mut s, f, 8, "(a[b]c)"), 1);
    finish(s);
}
fn multiline(f: &Fixture) { cases(f, &[
    Case("abc\ndef", 1, "down-line", "abc\ndef", Some(5)),
    Case("abc\ndef", 5, "up-line", "abc\ndef", Some(1)),
    Case("abc\ndef", 1, "insert-line-under", "abc\n\ndef", Some(4)),
    Case("abc\ndef", 5, "insert-line-over", "abc\n\ndef", Some(4)),
]); }
fn default_keys(f: &Fixture) {
    let mut s = start(f, "set -g fish_autosuggestion_enabled 0; bind ctrl-g 'commandline -i USER'; fish_default_key_bindings; builtin history clear; builtin history append history-one history-two");
    s.send(b"abcd\x01\x06\x04Z\x05\x02\x7f");
    capture(&mut s, f, 1, "aZd");
    s.send(b"\x07"); capture(&mut s, f, 2, "aZUSERd");
    s.send(b"\x03\x10"); capture(&mut s, f, 3, "history-two");
    s.send(b"\x0e"); capture(&mut s, f, 4, "");
    s.send(b"\x03snow\x01\x1bu"); capture(&mut s, f, 5, "SNOW");
    finish(s);
}
fn vi_operators(f: &Fixture) {
    let mut s = vi(f, "");
    s.send(b"one two three four"); escape(&mut s, f, 1, "one two three four");
    s.send(b"02dw"); capture(&mut s, f, 2, "three four");
    s.send(b"u"); capture(&mut s, f, 3, "one two three four");
    s.send(b"0cwSNOW"); capture(&mut s, f, 4, "SNOW two three four"); mode(f, 4, "insert");
    escape(&mut s, f, 5, "SNOW two three four");
    s.send(b"0d"); capture(&mut s, f, 6, "SNOW two three four"); mode(f, 6, "operator");
    escape(&mut s, f, 7, "SNOW two three four");
    s.send(b"2l"); assert_eq!(capture(&mut s, f, 8, "SNOW two three four"), 2);
    s.send(b"0yy"); capture(&mut s, f, 9, "SNOW two three four");
    s.send(b"P"); capture(&mut s, f, 10, "SNOW two three fourSNOW two three four");
    finish(s);
}
fn vi_linewise(f: &Fixture) {
    let mut s = vi(f, "bind -M default f2 'commandline -r \"One\nTwo\nThree\"; commandline -C 0'; bind -M default f3 undo");
    escape(&mut s, f, 1, "");
    f2(&mut s); capture(&mut s, f, 2, "One\nTwo\nThree");
    s.send(b"2g~~"); capture(&mut s, f, 3, "oNE\ntWO\nThree"); mode(f, 3, "default");
    f3(&mut s); capture(&mut s, f, 4, "One\nTwo\nThree");
    s.send(b"gg2dd"); capture(&mut s, f, 5, "Three");
    f3(&mut s); capture(&mut s, f, 6, "One\nTwo\nThree");
    s.send(b"gg2yyP"); capture(&mut s, f, 7, "One\nTwo\nOne\nTwo\nThree");
    f3(&mut s); capture(&mut s, f, 8, "One\nTwo\nThree");
    s.send(b"gg2ccchanged"); capture(&mut s, f, 9, "changed\nThree"); mode(f, 9, "insert");
    finish(s);
}
fn vi_modes(f: &Fixture) {
    let mut s = vi(f, "");
    s.send(b"abcd"); escape(&mut s, f, 1, "abcd");
    s.send(b"0vl"); assert_eq!(capture(&mut s, f, 2, "abcd"), 1); mode(f, 2, "visual");
    assert_eq!(fs::read(f.root.join("selection-2")).unwrap(), b"ab");
    s.send(b"o"); assert_eq!(capture(&mut s, f, 3, "abcd"), 0);
    s.send(b"cXY"); capture(&mut s, f, 4, "XYcd"); mode(f, 4, "insert");
    escape(&mut s, f, 5, "XYcd");
    s.send(b"0rZ"); capture(&mut s, f, 6, "ZYcd"); mode(f, 6, "default");
    s.send(b"R12"); capture(&mut s, f, 7, "12cd"); mode(f, 7, "replace");
    escape(&mut s, f, 8, "12cd");
    s.send(b"A!"); capture(&mut s, f, 9, "12cd!"); mode(f, 9, "insert");
    finish(s);
}
fn vi_numbers(f: &Fixture) {
    let mut s = vi(f, "bind -M default f2 'commandline -r x9; commandline -C 0'; bind -M default f3 'commandline -r 009; commandline -C 1'");
    escape(&mut s, f, 1, "");
    f2(&mut s); s.send(b"\x01"); assert_eq!(capture(&mut s, f, 2, "x10"), 2);
    s.send(b"\x18"); capture(&mut s, f, 3, "x09");
    f3(&mut s); s.send(b"\x01"); capture(&mut s, f, 4, "010");
    s.send(b"\x18"); capture(&mut s, f, 5, "009");
    finish(s);
}
fn hybrid(f: &Fixture) {
    let mut s = start(f, &format!("set -g fish_autosuggestion_enabled 0; fish_hybrid_key_bindings; {MODES_CAPTURE}"));
    s.send(b"one two\x01\x06Z"); capture(&mut s, f, 1, "oZne two"); mode(f, 1, "insert");
    escape(&mut s, f, 2, "oZne two");
    s.send(b"0dw"); capture(&mut s, f, 3, "two"); mode(f, 3, "default");
    s.send(b"i\x05!"); capture(&mut s, f, 4, "two!"); mode(f, 4, "insert");
    s.send(b"\x03fish_default_key_bindings\r");
    s.send(b"abc\x01X"); capture(&mut s, f, 5, "Xabc"); mode(f, 5, "default");
    s.send(b"\x05!");
    assert_eq!(capture(&mut s, f, 6, "Xabc!"), 5,
        "leaving vi/hybrid must remove its inclusive-cursor callback");
    finish(s);
}
fn binding_flow(f: &Fixture) {
    let mut s = start(f, "bind f2 jump-to-matching-bracket and delete-char; bind f3 get-key 'commandline -i $fish_key; set -e fish_key'; bind f4 jump-to-matching-bracket or forward-char; bind q self-insert-notfirst; bind f5 cancel-commandline; bind f6 exit");
    s.send(b"(ab)\x01"); f2(&mut s); capture(&mut s, f, 1, "(ab");
    s.send(b"\x03ab\x01"); f4(&mut s); assert_eq!(capture(&mut s, f, 2, "ab"), 1);
    f3(&mut s); s.send("雪".as_bytes()); capture(&mut s, f, 3, "a雪b");
    s.key(true, 1, 0x74, 0, 0, 0); capture(&mut s, f, 4, "");
    s.send(b"q"); capture(&mut s, f, 5, "");
    s.send(b"xq"); capture(&mut s, f, 6, "xq");
    s.send(b"\x03");
    // Exercise the actual exit readline action, not just the exit shell builtin.
    s.send(b"printf actual-execute > binding-executed\r");
    assert_eq!(s.wait_file(&f.root.join("binding-executed")), b"actual-execute");
    s.key(true, 1, 0x75, 0, 0, 0);
    let t = s.finish(0);
    assert_eq!(t.original_modes, t.final_modes);
    let mut eof = start(f, "");
    eof.send(b"\x04");
    let t = eof.finish(0);
    assert_eq!(t.original_modes, t.final_modes);
}
fn helpers(f: &Fixture) {
    fs::create_dir(f.root.join("listing")).unwrap();
    fs::write(f.root.join("listing/雪.txt"), b"native").unwrap();
    let mut s = start(f, "bind f2 'fish_commandline_prepend env'; bind f3 'fish_commandline_append \" tail\"'; bind f4 __fish_toggle_comment_commandline; bind f5 'commandline -r listing; __fish_list_current_token; printf ready > listing-ready'; bind f6 prevd-or-backward-token; bind f7 nextd-or-forward-token");
    s.send(b"printf x"); f2(&mut s); capture(&mut s, f, 1, "env printf x");
    f2(&mut s); capture(&mut s, f, 2, "printf x");
    f3(&mut s); capture(&mut s, f, 3, "printf x tail");
    s.send(b"\x03printf bad > commented-effect"); f4(&mut s);
    capture(&mut s, f, 4, "");
    assert!(!f.root.join("commented-effect").exists());
    f4(&mut s); capture(&mut s, f, 5, "printf bad > commented-effect");
    s.key(true, 1, 0x74, 0, 0, 0);
    s.wait_file(&f.root.join("listing-ready"));
    capture(&mut s, f, 6, "listing");
    s.send(b"\x03one two");
    s.key(true, 1, 0x75, 0, 0, 0); assert_eq!(capture(&mut s, f, 7, "one two"), 4);
    s.key(true, 1, 0x76, 0, 0, 0); assert_eq!(capture(&mut s, f, 8, "one two"), 7);
    let t = finish(s);
    assert!(t.bytes.windows("雪.txt".len()).any(|w| w == "雪.txt".as_bytes()));
}

fn abbr_runtime_error(f: &Fixture) {
    // Recursing without consuming a character is a deterministic PCRE runtime
    // error, unlike a machine-dependent catastrophic-backtracking time bound.
    let mut s = start(f, "abbr -a --position anywhere --command printf x must-not-expand; abbr -a --position anywhere --command printf --regex '(?R)' recursive must-not-expand; abbr -a --position anywhere good recovered");
    s.send(b"printf x ");
    capture(&mut s, f, 1, "printf x ");
    s.send(b"\x03echo good ");
    capture(&mut s, f, 2, "echo recovered ");
    s.send(b"\x03printf healthy > regex-recovery\r");
    assert_eq!(s.wait_file(&f.root.join("regex-recovery")), b"healthy");
    let t = finish(s);
    assert!(t.bytes.windows(b"recursive".len()).any(|w| w == b"recursive"),
        "the failed definition must be diagnosed, not silently treated as no match");
    assert!(t.bytes.windows(b"regular expression".len()).any(|w| w == b"regular expression"));
}

fn suggestion_scalar(f: &Fixture) {
    let setup = format!(
        "builtin history clear; builtin history append {}; bind f2 forward-single-char; bind f3 undo; bind f4 redo; bind f5 accept-autosuggestion; bind f6 suppress-autosuggestion; functions -c n6_capture n7_capture_base; function n6_capture; set -l n (math $n6_capture + 1); commandline --showing-suggestion; printf %s $status > suggestion-$n; n7_capture_base; end",
        quote("echo \u{f600}tail")
    );
    let mut s = start(f, &setup);
    s.send(b"echo ");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut n = 1;
    // Each request is consumed by the actual reader and acknowledged only after
    // its closed-file snapshot. Observe publication instead of assuming that a
    // sleep, a terminal echo, or an outstanding worker means a suggestion exists.
    loop {
        capture(&mut s, f, n, "echo ");
        if fs::read(f.root.join(format!("suggestion-{n}"))).unwrap() == b"0" { break; }
        assert!(std::time::Instant::now() < deadline, "actual history suggestion did not publish");
        n += 1;
    }
    f2(&mut s); n += 1;
    // Accept, undo and redo preserve the exact suggestion text.
    capture(&mut s, f, n, "echo \u{f600}");
    f3(&mut s); n += 1; assert_eq!(capture(&mut s, f, n, "echo "), 5);
    f4(&mut s); n += 1; capture(&mut s, f, n, "echo \u{f600}");
    // Re-publication after undo/redo is observed with the same bounded barrier.
    loop {
        n += 1; capture(&mut s, f, n, "echo \u{f600}");
        if fs::read(f.root.join(format!("suggestion-{n}"))).unwrap() == b"0" { break; }
        assert!(std::time::Instant::now() < deadline, "suggestion did not republish after redo");
    }
    s.key(true, 1, 0x74, 0, 0, 0); n += 1;
    capture(&mut s, f, n, "echo \u{f600}tail");
    f3(&mut s); n += 1; capture(&mut s, f, n, "echo \u{f600}");
    s.key(true, 1, 0x75, 0, 0, 0); n += 1;
    capture(&mut s, f, n, "echo \u{f600}");
    assert_eq!(fs::read(f.root.join(format!("suggestion-{n}"))).unwrap(), b"1");
    f2(&mut s); n += 1;
    capture(&mut s, f, n, "echo \u{f600}");
    finish(s);
}

fn vi_objects(f: &Fixture) {
    let rows = [
        ("one two three", 5, "diw", "one  three", 4),
        ("one two three", 5, "daw", "one three", 4),
        ("one two-three four", 6, "diW", "one  four", 4),
        ("one two-three four", 6, "daW", "one four", 4),
        ("echo (one two)", 8, "dib", "echo ()", 6),
        ("echo (one two)", 8, "dab", "echo ", 4),
        ("axbxcxd", 0, "dfx", "bxcxd", 0),
        ("axbxcxd", 0, "dtx", "xbxcxd", 0),
    ];
    let mut setup = String::from("set -g n7_index 0; function n7_case; set -g n7_index (math $n7_index + 1); switch $n7_index;");
    for (i, (before, cursor, _, _, _)) in rows.iter().enumerate() {
        setup.push_str(&format!("case {}; commandline -r {}; commandline -C {};", i + 1, quote(before), cursor));
    }
    setup.push_str("end; end; bind -M default f2 n7_case");
    let mut s = vi(f, &setup);
    escape(&mut s, f, 1, "");
    for (i, (before, _, keys, after, cursor)) in rows.iter().enumerate() {
        f2(&mut s);
        capture(&mut s, f, 2 * i + 2, before);
        s.send(keys.as_bytes());
        assert_eq!(capture(&mut s, f, 2 * i + 3, after), *cursor, "{keys}");
        mode(f, 2 * i + 3, "default");
    }
    finish(s);
}

fn directory_history(f: &Fixture) {
    fs::create_dir(f.root.join("dir-one")).unwrap();
    fs::create_dir(f.root.join("dir-two")).unwrap();
    // Keep captures in the original fixture root while observing actual cwd;
    // temporarily cd'ing back for capture would itself corrupt directory history.
    let init = "set -g n7_capture_root $PWD; cd dir-one; cd ../dir-two; bind f2 prevd-or-backward-word; bind f3 nextd-or-forward-word; function n6_capture; set -g n6_capture (math $n6_capture + 1); printf '%s' (commandline | string collect) > $n7_capture_root/capture-$n6_capture; commandline --cursor > $n7_capture_root/cursor-$n6_capture; printf '%s' $fish_bind_mode > $n7_capture_root/mode-$n6_capture; path basename -- $PWD > $n7_capture_root/directory-$n6_capture; printf ready > $n7_capture_root/ready-$n6_capture; end";
    let mut s = start(f, init);
    f2(&mut s); capture(&mut s, f, 1, "");
    assert_eq!(fs::read(f.root.join("directory-1")).unwrap(), b"dir-one\n");
    f3(&mut s); capture(&mut s, f, 2, "");
    assert_eq!(fs::read(f.root.join("directory-2")).unwrap(), b"dir-two\n");
    // Nonempty buffers use word movement, never directory history.
    s.send(b"one two"); f2(&mut s);
    assert_eq!(capture(&mut s, f, 3, "one two"), 4);
    assert_eq!(fs::read(f.root.join("directory-3")).unwrap(), b"dir-two\n");
    finish(s);
}
