use super::*;
use fish::complete::CompletionReceiver;
use fish::expand::ExpandFlags;
use fish::wildcard::{WildcardResult, wildcard_expand_string, wildcard_match};
use fish_widestring::{
    ANY_STRING, ANY_STRING_RECURSIVE, encode_byte_to_char, shell_text_to_osstring,
    utf16_to_wcstring, wcs2bytes,
};
use std::sync::mpsc;

fn pattern(text: &str) -> WString {
    // The low-level traversal API accepts the canonical internal wildcard alphabet.
    let mut result = WString::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '*' {
            if chars.peek() == Some(&'*') {
                chars.next();
                result.push(ANY_STRING_RECURSIVE);
            } else {
                result.push(ANY_STRING);
            }
        } else {
            result.push(ch);
        }
    }
    result
}
fn walk(root: &Path, text: &str, limit: usize) -> (WildcardResult, Vec<String>, usize) {
    let mut receiver = CompletionReceiver::new(limit);
    let mut checks = 0;
    let status = wildcard_expand_string(
        &pattern(text),
        &path_text(root),
        ExpandFlags::default(),
        || {
            checks += 1;
            checks > 100_000 // Detect broken loop protection without an unbounded traversal.
        },
        &mut receiver,
    );
    let mut results: Vec<_> = receiver
        .take()
        .into_iter()
        .map(|value| String::from_utf8(wcs2bytes(&value.completion)).unwrap())
        .collect();
    results.sort();
    (status, results, checks)
}
fn matched(root: &Path, text: &str, expected: &[&str]) {
    let (status, values, checks) = walk(root, text, 4096);
    assert!(matches!(status, WildcardResult::Match));
    assert!(checks < 100_000, "cycle protection required cancellation");
    let mut expected: Vec<_> = expected.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(values, expected);
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn GetProcessHandleCount(process: *mut std::ffi::c_void, count: *mut u32) -> i32;
}
fn handles() -> u32 {
    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0
    );
    count
}
pub(super) fn exercise(f: &Fixture) {
    let root = f.root.join("glob");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    for (name, bytes) in [
        ("a.txt", b"a".as_slice()),
        ("雪🐟.txt", b"unicode".as_slice()),
        ("\u{f600}\u{fdd0}.txt", b"reserved".as_slice()),
        (".hidden.txt", b"hidden".as_slice()),
        ("literal[1].txt", b"escaped".as_slice()),
        ("nested/deep.txt", b"deep".as_slice()),
    ] {
        fs::write(root.join(name), bytes).unwrap();
    }
    matched(
        &root,
        "*.txt",
        &[
            "a.txt",
            "雪🐟.txt",
            "\u{f600}\u{fdd0}.txt",
            "literal[1].txt",
        ],
    );
    matched(&root, ".*.txt", &[".hidden.txt"]);
    matched(
        &root,
        "**.txt",
        &[
            "a.txt",
            "雪🐟.txt",
            "\u{f600}\u{fdd0}.txt",
            "literal[1].txt",
            "nested/deep.txt",
        ],
    );
    matched(
        &root,
        "../glob/*.txt",
        &[
            "../glob/a.txt",
            "../glob/雪🐟.txt",
            "../glob/\u{f600}\u{fdd0}.txt",
            "../glob/literal[1].txt",
        ],
    );
    matched(
        &root.join("nested"),
        "../**.txt",
        &[
            "../a.txt",
            "../雪🐟.txt",
            "../\u{f600}\u{fdd0}.txt",
            "../literal[1].txt",
            "../nested/deep.txt",
        ],
    );
    matched(
        &root,
        "nested/../nested/*.txt",
        &["nested/../nested/deep.txt"],
    );
    assert!(wildcard_match(
        &path_text(Path::new("雪🐟.txt")),
        &pattern("*.txt"),
        true
    ));
    assert!(!wildcard_match(
        &path_text(Path::new(".hidden.txt")),
        &pattern("*.txt"),
        true
    ));
    let mut expected: Vec<_> = [
        "a.txt",
        "literal[1].txt",
        "雪🐟.txt",
        "\u{f600}\u{fdd0}.txt",
    ]
    .iter()
    .map(|s| format!("glob/{s}"))
    .collect();
    let out = success(f.eval("printf '%s\\n' glob/*.txt"));
    let mut observed: Vec<_> = std::str::from_utf8(&out)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    observed.sort();
    expected.sort();
    assert_eq!(observed, expected);
    exact(
        f.eval("printf '%s\\n' 'glob/literal[1].txt'"),
        0,
        b"glob/literal[1].txt\n",
        b"",
    );
    exact(
        f.eval("printf '%s\\n' glob/\\*.txt"),
        0,
        b"glob/*.txt\n",
        b"",
    );
    failed(f.eval("echo glob/no-match-*.absent"), true);
    exact(
        f.eval("set -l absent glob/no-match-*.absent; set -q absent[1]; echo $status"),
        0,
        b"1\n",
        b"",
    );
    let (status, values, _) = walk(&root, "no-match-*.absent", 4096);
    assert!(matches!(status, WildcardResult::NoMatch));
    assert!(values.is_empty());
    let case = root.join("case");
    native::case_sensitive(&case);
    matched(&root, "case/T*", &["case/Twin"]);
    matched(&root, "case/t*", &["case/twin"]);
    fs::write(root.join("MiXeD.dat"), b"case insensitive").unwrap();
    assert!(
        !fish::wutil::native_directory_case_sensitive(&path_text(&root)).unwrap(),
        "ordinary test root must use insensitive policy"
    );
    matched(&root, "mixed.*", &["MiXeD.dat"]);
    let denied_path = root.join("denied");
    let denied = native::Denied::new(&denied_path, true);
    let (status, values, _) = walk(&root, "**.txt", 4096);
    assert!(matches!(status, WildcardResult::Match));
    assert!(
        !values
            .iter()
            .any(|s| s.contains("hidden.txt") && s.starts_with("denied/"))
    );
    drop(denied);
    fs::remove_dir_all(&denied_path).unwrap();
    let link = root.join("nested/back");
    native::junction(&link, &root);
    let (status, values, checks) = walk(&root, "**.txt", 4096);
    assert!(matches!(status, WildcardResult::Match));
    assert!(checks < 100_000);
    assert!(
        !values.iter().any(|s| s.contains("back/nested/back")),
        "recursive identity cycle was revisited"
    );
    let out = success(f.eval("printf '%s\\n' glob/**.txt"));
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.lines().count() < 32,
        "executable traversal failed cycle bound"
    );
    assert!(!text.contains("back/nested/back"));
    fs::remove_dir(&link).unwrap();
    let broken = root.join("broken-link");
    std::os::windows::fs::symlink_file(root.join("missing-target"), &broken)
        .expect("required broken glob symlink fixture");
    matched(&root, "broken-*", &["broken-link"]);
    exact(
        f.eval("printf '%s\\n' glob/broken-*"),
        0,
        b"glob/broken-link\n",
        b"",
    );
    fs::remove_file(broken).unwrap();
    limits_and_cancel(&root);
    invalid_names(f, &root);
    // Repeated traversal must release native directory handles (gates are serialized).
    let before = handles();
    for _ in 0..64 {
        let (status, _, _) = walk(&root, "**.txt", 4096);
        assert!(matches!(status, WildcardResult::Match));
    }
    assert!(handles() <= before + 2, "directory enumeration handle leak");
    let moved = f.root.join("glob-released");
    fs::rename(&root, &moved).unwrap();
    fs::remove_dir_all(moved).unwrap();
}
fn limits_and_cancel(root: &Path) {
    let large = root.join("large");
    fs::create_dir(&large).unwrap();
    for directory in 0..48 {
        let dir = large.join(format!("branch{directory:03}"));
        fs::create_dir(&dir).unwrap();
        for entry in 0..8 {
            fs::write(dir.join(format!("item{entry}.txt")), b"walk").unwrap();
        }
    }
    let (status, values, _) = walk(root, "large/**.txt", 7);
    assert!(matches!(status, WildcardResult::Overflow));
    assert!(values.len() <= 7);
    let (ready_tx, ready_rx) = mpsc::sync_channel(0);
    let (cancel_tx, cancel_rx) = mpsc::sync_channel(0);
    let coordinator = std::thread::spawn(move || {
        ready_rx
            .recv_timeout(fixture::LIMIT)
            .expect("walk never reached cancellation barrier");
        cancel_tx.send(()).unwrap();
    });
    let mut checks = 0;
    let mut receiver = CompletionReceiver::new(4096);
    let status = wildcard_expand_string(
        &pattern("large/**.txt"),
        &path_text(root),
        ExpandFlags::default(),
        || {
            checks += 1;
            if checks == 32 {
                ready_tx.send(()).unwrap();
                cancel_rx
                    .recv_timeout(fixture::LIMIT)
                    .expect("cancel coordinator failed");
                true
            } else {
                false
            }
        },
        &mut receiver,
    );
    coordinator.join().unwrap();
    assert!(matches!(status, WildcardResult::Cancel));
    assert!(checks >= 32);
    assert!(
        receiver.size() < 384,
        "cancellation happened only after a complete walk"
    );
    // Mutation occurs at a traversal checkpoint with no enumeration races left to timing.
    let mut mutated = false;
    let mut mutation_checks = 0;
    let mut receiver = CompletionReceiver::new(4096);
    let status = wildcard_expand_string(
        &pattern("large/**.txt"),
        &path_text(root),
        ExpandFlags::default(),
        || {
            mutation_checks += 1;
            if mutation_checks == 32 {
                fs::rename(large.join("branch047"), large.join("renamed")).unwrap();
                mutated = true;
            }
            false
        },
        &mut receiver,
    );
    assert!(mutated);
    assert!(matches!(status, WildcardResult::Match));
    fs::remove_dir_all(&large).unwrap();
}
fn invalid_names(f: &Fixture, root: &Path) {
    assert!(utf16_to_wcstring(&[0xd800]).is_err());
    assert!(utf16_to_wcstring(&[0xdc00]).is_err());
    let mut byte = WString::new();
    byte.push(encode_byte_to_char(0xff));
    assert!(shell_text_to_osstring(&byte).is_err());
    let nul = WString::from("bad\0name");
    assert!(shell_text_to_osstring(&nul).is_err());
    for invalid in [&byte, &nul] {
        let mut receiver = CompletionReceiver::new(10);
        let status = wildcard_expand_string(
            invalid,
            &path_text(root),
            ExpandFlags::default(),
            || false,
            &mut receiver,
        );
        assert!(matches!(status, WildcardResult::Error(_)));
        assert!(receiver.empty());
    }
    let invalid_dir = root.join("invalid-native");
    fs::create_dir(&invalid_dir).unwrap();
    use std::os::windows::ffi::OsStringExt;
    let invalid_path = invalid_dir.join(std::ffi::OsString::from_wide(&[0xd800]));
    fs::write(&invalid_path, b"surrogate").expect("required NTFS lone-surrogate fixture");
    fs::write(invalid_dir.join("\u{fffd}"), b"must not alias").unwrap();
    let result = f.eval("echo glob/invalid-native/*");
    failed(result, true);
    fs::remove_file(invalid_path).unwrap();
    fs::remove_dir_all(invalid_dir).unwrap();
    for invalid in [
        "glob/\\xFF*",
        "glob/\\u0000*",
        "glob/\\uD800*",
        "glob/\\U00110000*",
    ] {
        failed(f.eval(&format!("echo {invalid}")), true);
    }
    let unknown = root.join("unknown-reparse");
    native::unknown_reparse(&unknown);
    let (status, _, _) = walk(root, "**.txt", 4096);
    assert!(
        matches!(&status, WildcardResult::Error(error) if error.kind() == std::io::ErrorKind::Unsupported)
    );
    failed(f.eval("echo glob/**.txt"), true);
    fs::remove_dir(unknown).unwrap();
}
