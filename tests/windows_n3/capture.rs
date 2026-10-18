use super::fixture::{Fixture, LIMIT, exact, quote};
use super::native::Event;
use std::ffi::c_void;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
    fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
}

struct Release<'a>(&'a Event);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.signal();
    }
}
struct HeldDescendant<'a> {
    process: OwnedHandle,
    release: &'a Event,
}
impl<'a> HeldDescendant<'a> {
    fn open(pid: u32, release: &'a Event) -> Self {
        let raw = unsafe { OpenProcess(0x0010_0000 | 0x1000 | 1, 0, pid) };
        assert!(
            !raw.is_null(),
            "open owned descendant: {}",
            std::io::Error::last_os_error()
        );
        Self {
            process: unsafe { OwnedHandle::from_raw_handle(raw) },
            release,
        }
    }
    fn assert_live(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) },
            258,
            "descendant must remain held across substitution completion"
        );
    }
    fn reap(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.process.as_raw_handle(), LIMIT.as_millis() as u32) },
            0
        );
        let mut code = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) },
            0
        );
        assert_eq!(code, 0);
    }
}
impl Drop for HeldDescendant<'_> {
    fn drop(&mut self) {
        self.release.signal();
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), LIMIT.as_millis() as u32) }
            != 0
        {
            unsafe {
                TerminateProcess(self.process.as_raw_handle(), 97);
            }
            assert_eq!(
                unsafe {
                    WaitForSingleObject(self.process.as_raw_handle(), LIMIT.as_millis() as u32)
                },
                0
            );
        }
    }
}

fn inherited_descriptors(f: &Fixture) {
    // This is the same composition used by stream/editor clipboard paste:
    // nested function substitutions must retain an enclosing error destination,
    // including through a pipeline, block, eval, or sourced file.
    let definitions = r#"
function n3_substitution_diagnostic
    set -l data (math '1 / 0' | string collect -N)
    return $status
end
function n3_nested_diagnostic
    set -l data (n3_substitution_diagnostic)
    return $status
end
"#;
    let source = f.root.join("n3 inherited substitution diagnostic.fish");
    std::fs::write(&source, format!("{definitions}\nn3_nested_diagnostic\n")).unwrap();
    let commands = [
        "n3_nested_diagnostic".to_owned(),
        "begin; n3_nested_diagnostic; end".to_owned(),
        "eval n3_nested_diagnostic".to_owned(),
        format!("source {}", quote(source.to_str().unwrap())),
    ];
    for (index, command) in commands.iter().enumerate() {
        let error_file = f.root.join(format!("n3-inherited-diagnostic-{index}"));
        let result = f.eval(&format!(
            "{definitions}\n{command} 2> {}; set -l failed $status; printf '%s' (printf restored); exit $failed",
            quote(error_file.to_str().unwrap())
        ));
        assert_ne!(result.code, 0);
        assert_eq!(result.out, b"restored");
        assert!(
            result.err.is_empty(),
            "nested diagnostic escaped its redirect"
        );
        assert!(
            !std::fs::read(error_file).unwrap().is_empty(),
            "real builtin diagnostic must reach the enclosing error file"
        );
    }
    let unredirected = f.eval(&format!("{definitions}\nn3_nested_diagnostic"));
    assert_ne!(unredirected.code, 0);
    assert!(unredirected.out.is_empty());
    assert!(!unredirected.err.is_empty());
    let closed = f.eval(&format!(
        "{definitions}\nn3_nested_diagnostic 2>&-; set -l failed $status; printf '%s' (printf restored); exit $failed"
    ));
    assert_ne!(closed.code, 0);
    assert_eq!(closed.out, b"restored");
    assert!(closed.err.is_empty());

    // Aliases retain the endpoint selected in redirection order. Replacing
    // substitution stdout must not retarget stderr that already aliases it.
    let alias_function =
        "function n3_substitution_alias; set -l data (printf inherited-alias >&2); end";
    let alias_file = f.root.join("n3-inherited-alias");
    exact(
        f.eval(&format!(
            "{alias_function}; n3_substitution_alias > {} 2>&1; printf restored",
            quote(alias_file.to_str().unwrap())
        )),
        0,
        b"restored",
        b"",
    );
    assert_eq!(std::fs::read(&alias_file).unwrap(), b"inherited-alias");
    exact(
        f.eval(&format!(
            "{alias_function}; n3_substitution_alias 2>&1 > {}; printf restored",
            quote(alias_file.to_str().unwrap())
        )),
        0,
        b"inherited-aliasrestored",
        b"",
    );
    assert!(std::fs::read(&alias_file).unwrap().is_empty());
    exact(
        f.eval(&format!(
            "function n3_substitution_extra; set -l data (printf inherited-extra >&9); end; n3_substitution_extra 9> {}; printf restored",
            quote(alias_file.to_str().unwrap())
        )),
        0,
        b"restored",
        b"",
    );
    assert_eq!(std::fs::read(alias_file).unwrap(), b"inherited-extra");

    // A real external consumer inherits logical stdin and emits binary stderr;
    // neither endpoint may silently revert to the shell's startup handles.
    let error_file = f.root.join("n3-inherited-external-error");
    exact(
        f.eval(&format!(
            "function n3_substitution_external; set -l data ({} stream); printf '%s' \"$data\"; end; printf retained-input | n3_substitution_external 2> {}",
            f.probe(),
            quote(error_file.to_str().unwrap())
        )),
        0,
        b"probe-out:retained-input",
        b"",
    );
    assert_eq!(
        std::fs::read(error_file).unwrap(),
        b"probe-err:\0\n\r\n\xff\x80"
    );
}

pub(super) fn exercise(f: &Fixture) {
    inherited_descriptors(f);
    let probe = f.probe();
    exact(f.eval(r#"
set n3_none (printf '')
set n3_quoted "$(printf '')"
set n3_empty (printf '' | string collect --allow-empty)
printf 'counts:%s:%s:%s\n' (count $n3_none) (count $n3_quoted) (count $n3_empty)
set n3_lines (printf 'one\ntwo\n\n')
printf '<%s>\n' $n3_lines
set n3_explicit (printf 'a\0\0b\0' | string split0)
printf 'nul-count:%s\n' (count $n3_explicit)
printf '<%s>\n' $n3_explicit
set n3_collected (printf 'a\nb\n\n' | string collect)
printf 'collect-count:%s\n%s:end\n' (count $n3_collected) "$n3_collected"
set n3_nested (printf '<%s>' (printf '%s\n' (printf 'inner\n')))
printf 'nested:%s\n' $n3_nested
false
set n3_failed (false)
printf 'sub-status:%s\n' $status
"#), 0, b"counts:0:1:1\n<one>\n<two>\n<>\nnul-count:3\n<a>\n<>\n<b>\ncollect-count:1\na\nb:end\nnested:<inner>\nsub-status:1\n", b"");

    exact(
        f.eval(
            r#"
set -g IFS ''
set n3_unsplit (printf 'a\nb\n')
printf 'unsplit:%s:%s:end\n' (count $n3_unsplit) "$n3_unsplit"
function n3_capture_return
    set -l n3_return_local private
    printf 'before-return\n'
    return 29
    printf 'unreachable\n'
end
set n3_returned (n3_capture_return)
set n3_return_status $status
printf 'returned:%s:%s\n' $n3_return_status "$n3_returned"
set -q n3_return_local; or printf 'return-scope-restored\n'
"#,
        ),
        0,
        b"unsplit:1:a\nb:end\nreturned:29:before-return\nreturn-scope-restored\n",
        b"",
    );
    exact(
        f.eval(&format!(
            "set n3_binary ({probe} n2-binary); printf '%s' $n3_binary"
        )),
        0,
        b"\0\xff\x80\rA\0B",
        b"",
    );

    // Every path evaluates real functions/blocks on the main thread. The native
    // text producer/consumer verifies a payload larger than the kernel pipe.
    let definitions = format!(
        "function n3_producer; {probe} n2-text-produce; end; \
         function n3_copy; {probe} n2-copy; end; \
         function n3_consumer; {probe} n2-text-consume; end; "
    );
    for pipeline in [
        format!("n3_producer | {probe} n2-text-consume"),
        format!("{probe} n2-text-produce | n3_consumer"),
        "n3_producer | n3_consumer".to_owned(),
        format!("n3_producer | {probe} n2-copy | n3_consumer"),
        format!("n3_producer | n3_copy | {probe} n2-text-consume"),
        format!(
            "begin; {probe} n2-text-produce; end | begin; eval {}; end | {probe} n2-text-consume",
            quote(&format!("{probe} n2-copy"))
        ),
    ] {
        exact(
            f.eval(&format!("{definitions} {pipeline}")),
            0,
            b"text=2097152;EOF\n",
            b"",
        );
    }
    exact(
        f.eval(&format!(
            "set n3_large ({probe} n2-text-produce); string length -- $n3_large"
        )),
        0,
        b"2097152\n",
        b"",
    );

    // A child stdout read may still be pending after its retained root exits.
    // The next builtin must drain a real available-byte boundary BEFORE adding
    // explicit elements, including empties, to the same separated buffer.
    let ordered = format!(
        r#"
function n3_ordered
    printf 'before\n'
    printf 'middle\n' | {probe} n2-copy
    string split : 'x::y'
    printf 'after\n'
    printf 'final\n' | {probe} n2-copy
    printf 'tail\n'
end
"#
    );
    exact(
        f.eval(&format!(
            "{ordered} set n3_values (n3_ordered); count $n3_values; printf '<%s>\\n' $n3_values"
        )),
        0,
        b"8\n<before>\n<middle>\n<x>\n<>\n<y>\n<after>\n<final>\n<tail>\n",
        b"",
    );
    exact(
        f.eval(&format!(
            "{ordered} n3_ordered | begin; {probe} n2-copy; end"
        )),
        0,
        b"before\nmiddle\nx\n\ny\nafter\nfinal\ntail\n",
        b"",
    );

    // An explicit local fd0 redirect selects stdin for shared Arguments.
    // Enclosing block IO alone must not replace a builtin's argv source.
    exact(f.eval(&format!("printf 'dir/first\\ndir/second\\n' | begin; path basename --quiet <&0; {probe} n2-copy <&0; end")),
        0, b"dir/second\n", b"");
    exact(f.eval(&format!("printf 'one\\ntwo\\nthree\\n' | begin; read -l first; printf '%s\\n' $first; {probe} n2-copy <&0; end")),
        0, b"one\ntwo\nthree\n", b"");
    exact(f.eval(&format!("printf 'dir/first\\ndir/second\\n' | {probe} n2-copy | begin; path basename --quiet <&0; {probe} n2-copy <&0; end")),
        0, b"dir/second\n", b"");
    exact(
        f.eval(&format!(
            "printf 'dir/a\\0dir/b\\0' | begin; {probe} exit 0; path basename <&0; end"
        )),
        0,
        b"a\nb\n",
        b"",
    );
    exact(f.eval(&format!("printf 'dir/a\\0dir/b\\0' | {probe} n2-copy | begin; {probe} exit 0; path basename <&0; end")),
        0, b"a\nb\n", b"");
    exact(f.eval(&format!("printf 'one\\ntwo\\nthree\\n' | begin; {probe} exit 0; read -l first; {probe} exit 0; read -l second; {probe} n2-copy; printf '%s:%s\\n' $first $second; end")),
        0, b"three\none:two\n", b"");
    exact(f.eval(&format!("printf 'retained-input\\n' | begin; path basename dir/argument; {probe} n2-copy <&0; end")),
        0, b"argument\nretained-input\n", b"");

    // Failed plan/launch ownership cannot consume the caller's logical input.
    let invalid = f.root.join("n3-invalid-image.exe");
    std::fs::write(&invalid, b"MZ-not-a-valid-Windows-image").unwrap();
    for command in [
        quote(invalid.to_str().unwrap()),
        format!("{probe} n2-copy 9>&1"),
    ] {
        let result = f.eval(&format!("printf 'first\\nsecond\\n' | begin; {command}; read -l value; printf '%s\\n' $value; end"));
        assert_eq!(result.code, 0);
        assert_eq!(result.out, b"first\n");
        assert!(!result.err.is_empty(), "failed native launch must diagnose");
    }

    let limit_marker = f.root.join("n3-limit-marker");
    let limited = f.eval(&format!(
        "set -g fish_read_limit 8; {} marker {} (printf '0123456789')",
        probe,
        quote(limit_marker.to_str().unwrap())
    ));
    assert_ne!(limited.code, 0);
    assert!(!limited.err.is_empty());
    assert!(
        !limit_marker.exists(),
        "output-limit failure executed affected command"
    );
    let empty_marker = f.root.join("n3-empty-metadata-limit-marker");
    let metadata_limit = f.eval(&format!(
        "set -g fish_read_limit 8; {probe} marker {} (printf '\\0\\0\\0' | string split0)",
        quote(empty_marker.to_str().unwrap())
    ));
    assert_ne!(metadata_limit.code, 0);
    assert!(!metadata_limit.err.is_empty());
    assert!(
        !empty_marker.exists(),
        "explicit empty fields exceeded bounded metadata storage"
    );
    let tail_marker = f.root.join("n3-tail-error-marker");
    let tail_error = f.eval(&format!(
        "{} marker {} (printf first)(n3_no_such_command)",
        probe,
        quote(tail_marker.to_str().unwrap())
    ));
    assert_ne!(tail_error.code, 0);
    assert!(!tail_error.err.is_empty());
    assert!(
        !tail_marker.exists(),
        "tail substitution failure executed affected command"
    );

    let broken = f.eval(&format!("begin; {probe} n2-broken-produce; end | {probe} n2-early-consume; printf '%s\\n' $pipestatus"));
    assert_eq!(broken.code, 0);
    assert_eq!(
        broken.out, b"41\n23\n",
        "producer IO failure replaced a retained native exit code"
    );
    assert!(
        !broken.err.is_empty(),
        "forwarding failure must still be diagnosed"
    );
    let multiple = f.eval(&format!("begin; {probe} n2-broken-produce; end | {probe} n2-early-consume | {probe} exit 29; printf '%s\\n' $pipestatus"));
    assert_eq!(multiple.code, 0);
    assert_eq!(
        multiple.out, b"41\n23\n29\n",
        "producer failure erased independently retained downstream statuses"
    );
    assert!(!multiple.err.is_empty());

    held_descendant(f);
}

fn held_descendant(f: &Fixture) {
    let ready = Event::new("n3-descendant-ready");
    let root_ready = Event::new("n3-root-pid-ready");
    let root_release = Event::new("n3-root-release");
    let release = Event::new("n3-descendant-release");
    let pid_file = f.root.join("n3-held-descendant.pid");
    // The helper's own descendant launch may also inherit its parent's stderr
    // handle even though the descendant selects NUL for stderr. Keep that handle
    // out of Fixture::run's EOF-based collectors; stdout must remain the actual
    // nested pipeline capture, including the descendant's inherited writer.
    let helper_stderr = f.root.join("n3-held-descendant.stderr");
    let probe = f.probe();
    let code = format!(
        "function n3_holder; {probe} n2-descendant-root {} {} {} {} {} 2>{}; end; \
         set n3_outer (begin; set n3_inner (n3_holder | begin; {probe} n2-copy; end); printf '%s' $n3_inner; end); \
         printf '%s' $n3_outer",
        quote(&ready.name),
        quote(&release.name),
        quote(pid_file.to_str().unwrap()),
        quote(&root_ready.name),
        quote(&root_release.name),
        quote(helper_stderr.to_str().unwrap()),
    );
    std::thread::scope(|scope| {
        let _root_cleanup = Release(&root_release);
        let _descendant_cleanup = Release(&release);
        let fish = scope.spawn(|| f.eval(&code));
        root_ready.wait();
        ready.wait();
        let pid: u32 = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
        let descendant = HeldDescendant::open(pid, &release);
        descendant.assert_live();
        let started = std::time::Instant::now();
        root_release.signal();
        let captured = fish.join().unwrap();
        assert!(
            started.elapsed() < LIMIT / 2,
            "nested capture waited for the held descendant's stream closure"
        );
        descendant.assert_live();
        exact(captured, 0, b"available:\0\xff\r", b"");
        release.signal();
        descendant.reap();
        assert_eq!(
            std::fs::read(&helper_stderr).unwrap(),
            b"",
            "held helper emitted diagnostics"
        );
    });
}
