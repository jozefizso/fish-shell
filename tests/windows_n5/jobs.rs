use super::fixture::{Fixture, exact, quote, run};
use super::native::Event;
use super::support::{Controlled, one, records, success};
use std::ffi::OsStr;

fn signal(f: &Fixture, event: &Event) -> String {
    format!("{} n5-jobs-signal {}", f.probe(), quote(&event.name))
}

pub fn background_progress(f: &Fixture) {
    for mixed in [false, true] {
        let mut child = Controlled::new(f, if mixed { "mixed" } else { "native" });
        let continued = Event::new("n5-background-root-continued");
        let command = if mixed {
            format!(
                "printf 'mixed-input\\n' | {}",
                child
                    .command(f, 17)
                    .replace("n5-jobs-barrier", "n5-jobs-copy-barrier")
            )
        } else {
            child.command(f, 17)
        };
        let code = format!(
            "{command} &; set -l owned $last_pid; {}; wait $owned; printf 'wait=%s\\n' $status",
            signal(f, &continued)
        );
        let result = std::thread::scope(|scope| {
            let shell = scope.spawn(|| f.eval(&code));
            child.started();
            continued.wait();
            child.assert_live();
            child.release.signal();
            shell.join().unwrap()
        });
        let out = success(result, "true native/mixed background progress");
        assert_eq!(one(&out, "wait"), "0");
        if mixed {
            assert!(
                out.contains("mixed-input"),
                "mixed pipeline lost bytes: {out:?}"
            );
        }
        child.wait(17);
    }
}

pub fn background_stdin(f: &Fixture) {
    let received = f.root.join("stdin-bytes");
    let q = quote(received.to_str().unwrap());
    // Root stdin is a supplied native pipe. An unredirected background child
    // must see NUL EOF without consuming the foreground evaluator's line.
    exact(
        f.input(
            &format!(
                "{} n5-jobs-input {q} &; wait; read -l line; printf '%s\\n' $line",
                f.probe()
            ),
            b"foreground-kept\n",
        ),
        0,
        b"foreground-kept\n",
        b"",
    );
    assert_eq!(
        std::fs::read(&received).unwrap(),
        b"",
        "background default stdin did not reach EOF"
    );
    let input = f.root.join("explicit-input");
    std::fs::write(&input, b"file\0\xff\r\n").unwrap();
    exact(
        f.eval(&format!(
            "{} n5-jobs-input {q} <{} &; wait",
            f.probe(),
            quote(input.to_str().unwrap())
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(std::fs::read(&received).unwrap(), b"file\0\xff\r\n");
    exact(
        f.eval(&format!(
            "printf 'pipe-input' | {} n5-jobs-input {q} &; wait",
            f.probe()
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(
        std::fs::read(&received).unwrap(),
        b"pipe-input",
        "explicit pipeline stdin was replaced by NUL"
    );
    exact(
        f.input(
            &format!("{} n5-jobs-input {q} <&0 &; wait", f.probe()),
            b"explicit-parent-pipe",
        ),
        0,
        b"",
        b"",
    );
    assert_eq!(
        std::fs::read(&received).unwrap(),
        b"explicit-parent-pipe",
        "explicit inherited pipe was replaced by NUL"
    );
}

pub fn visible_jobs_options(f: &Fixture) {
    let mut a = Controlled::new(f, "visible-a");
    let mut b = Controlled::new(f, "visible-b");
    let inspected = Event::new("n5-jobs-inspected");
    let script = format!(
        r#"
{} &; set -l a $last_pid
{} &; set -l b $last_pid
printf 'a=%s\nb=%s\n' $a $b
printf 'pid=%s\n' (jobs --pid)
printf 'group-a=%s\n' (jobs --group $a)
printf 'group-a-again=%s\n' (jobs -g $a)
printf 'group-b=%s\n' (jobs -g $b)
printf 'last=%s\n' (jobs --last --pid)
printf 'command=%s\n' (jobs --command $a)
jobs >jobs-table
jobs --quiet $a; printf 'query=%s\n' $status
set -l visible_id (string split \t -- (jobs $a))[1]
jobs --quiet %$visible_id; printf 'percent-query=%s\n' $status
printf 'percent-group=%s\n' (jobs --group %$visible_id)
jobs --query 2147483647; printf 'unknown=%s\n' $status
jobs --not-an-option 2>jobs-invalid; printf 'invalid=%s\n' $status
jobs %not-a-number 2>jobs-invalid-id; printf 'invalid-id=%s\n' $status
{}
wait $a $b
jobs --quiet; printf 'empty=%s\n' $status
"#,
        a.command(f, 0),
        b.command(f, 0),
        signal(f, &inspected)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        let apid = a.started();
        let bpid = b.started();
        inspected.wait();
        a.assert_live();
        b.assert_live();
        a.release.signal();
        b.release.signal();
        (apid, bpid, shell.join().unwrap())
    });
    let out = success(result.2, "jobs visibility/options");
    assert_eq!(one(&out, "a"), result.0.to_string());
    assert_eq!(one(&out, "b"), result.1.to_string());
    let pids = records(&out, "pid");
    assert_eq!(
        pids.len(),
        2,
        "jobs -p must expose both real native processes: {out}"
    );
    assert!(pids.contains(&result.0.to_string()) && pids.contains(&result.1.to_string()));
    let ga: u64 = one(&out, "group-a")
        .parse()
        .expect("native launch-group identity");
    let gb: u64 = one(&out, "group-b")
        .parse()
        .expect("native launch-group identity");
    assert_ne!(ga, 0);
    assert_ne!(gb, 0);
    assert_ne!(ga, gb);
    assert_eq!(
        one(&out, "group-a-again"),
        ga.to_string(),
        "native group identity changed while live"
    );
    assert_eq!(one(&out, "last"), result.1.to_string());
    assert_eq!(one(&out, "command"), f.probe.to_str().unwrap());
    assert_eq!(one(&out, "query"), "0");
    assert_eq!(one(&out, "percent-query"), "0");
    assert_eq!(one(&out, "percent-group"), ga.to_string());
    for key in ["unknown", "invalid", "invalid-id", "empty"] {
        assert_ne!(one(&out, key), "0", "{key}: {out}");
    }
    let table = std::fs::read_to_string(f.root.join("jobs-table")).unwrap();
    assert!(
        table.contains(&ga.to_string()) && table.contains(&gb.to_string()),
        "default jobs table omitted native identities: {table}"
    );
    assert!(
        !std::fs::read(f.root.join("jobs-invalid"))
            .unwrap()
            .is_empty(),
        "invalid option omitted diagnostics"
    );
}

pub fn waits_cached_multi_any(f: &Fixture) {
    let mut a = Controlled::new(f, "wait-a");
    let mut b = Controlled::new(f, "wait-b");
    let selected = Event::new("n5-wait-any-returned");
    let all = Event::new("n5-wait-all-returned");
    let script = format!(
        r#"
{} &; set -l a $last_pid
{} &; set -l b $last_pid
wait --any $a $b; printf 'any=%s\n' $status
{}
wait $b $b; printf 'multi-duplicate=%s\n' $status
{}
wait; printf 'empty-all=%s\n' $status
wait -n; printf 'empty-any=%s\n' $status
wait 2147483647 2>unknown-wait; printf 'unknown=%s\n' $status
wait --invalid 2>invalid-wait; printf 'invalid=%s\n' $status
wait -1 2>negative-wait; printf 'negative=%s\n' $status
"#,
        a.command(f, 23),
        b.command(f, 41),
        signal(f, &selected),
        signal(f, &all)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        a.started();
        b.started();
        a.release.signal();
        a.wait(23);
        selected.wait();
        b.assert_live();
        b.release.signal();
        all.wait();
        shell.join().unwrap()
    });
    let out = success(result, "wait multi/any/duplicate/invalid");
    // Upstream wait reports whether waiting succeeded, not the child's status.
    for key in ["any", "multi-duplicate", "empty-all", "empty-any"] {
        assert_eq!(one(&out, key), "0", "{out}");
    }
    for key in ["unknown", "invalid", "negative"] {
        assert_ne!(one(&out, key), "0", "{out}");
    }
    for file in ["unknown-wait", "invalid-wait", "negative-wait"] {
        assert!(
            !std::fs::read(f.root.join(file)).unwrap().is_empty(),
            "{file} omitted diagnostics"
        );
    }

    // Completion is deliberately reaped by a registered handler before wait
    // looks it up: this must use the retained cached process object.
    let mut cached = Controlled::new(f, "cached");
    let observed = Event::new("n5-cached-reaped");
    let root = Event::new("n5-cached-root");
    let script = format!(
        r#"
{} &; set -l child $last_pid
function completed --on-process-exit $child
    {}
end
{}
{} n5-jobs-wait {}
wait $child $child; printf 'cached=%s\n' $status
wait $child 2>consumed-wait; printf 'consumed=%s\n' $status
"#,
        cached.command(f, 29),
        signal(f, &observed),
        signal(f, &root),
        f.probe(),
        quote(&observed.name)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        cached.started();
        root.wait();
        cached.release.signal();
        shell.join().unwrap()
    });
    let out = success(result, "cached already-exited waits");
    assert_eq!(one(&out, "cached"), "0");
    assert_ne!(
        one(&out, "consumed"),
        "0",
        "consumed cache entry remained eligible: {out}"
    );

    let mut named_a = Controlled::new(f, "named-a");
    let mut named_b = Controlled::new(f, "named-b");
    let named_ready = Event::new("n5-named-wait-root");
    let script = format!(
        "{} &; {} &; {}; wait {}; printf 'name=%s\\n' $status",
        named_a.command(f, 0),
        named_b.command(f, 0),
        signal(f, &named_ready),
        quote(f.probe.file_name().unwrap().to_str().unwrap())
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        named_a.started();
        named_b.started();
        named_ready.wait();
        named_a.release.signal();
        named_b.release.signal();
        shell.join().unwrap()
    });
    assert_eq!(
        one(
            &success(
                result,
                "wait process basename selects all actual matching children"
            ),
            "name"
        ),
        "0"
    );
}

pub fn events_exactly_once_reentrant(f: &Fixture) {
    let mut a = Controlled::new(f, "event-a");
    let mut b = Controlled::new(f, "event-b");
    let root_ready = Event::new("n5-events-registered");
    let callback = Event::new("n5-event-reentrant-callback");
    let script = format!(
        r#"
set -g process_count 0; set -g job_count 0; set -g late_count 0; set -g catch_count 0
set -g late_job_count 0; set -g catch_job_count 0
{} &; set -g owned_a $last_pid
{} &; set -g owned_b $last_pid
function process_a --on-process-exit $owned_a
    set -g process_count (math $process_count + 1)
    printf 'process-status=%s\n' $argv[3]
    function late --on-process-exit $owned_a
        set -g late_count (math $late_count + 1)
    end
    jobs --quiet $owned_b; printf 'callback-live=%s\n' $status
    function nested --on-event n5_nested; printf 'nested=called\n'; end
    emit n5_nested
    {}
end
function job_a --on-job-exit $owned_a
    set -g job_count (math $job_count + 1)
    function late_job --on-job-exit $owned_a
        set -g late_job_count (math $late_job_count + 1)
    end
end
function catch_all --on-process-exit 0
    if test $argv[2] = $owned_a; set -g catch_count (math $catch_count + 1); end
end
function catch_all_jobs --on-job-exit 0
    if test $argv[2] = $owned_a; set -g catch_job_count (math $catch_job_count + 1); end
end
function shell_exit --on-process-exit %self
    printf 'shell-exit=once\n'
end
{}
wait $owned_a $owned_b; printf 'active-wait=%s\n' $status
printf 'counts=%s,%s,%s,%s,%s,%s\n' $process_count $job_count $late_count $catch_count $late_job_count $catch_job_count
wait $owned_b 2>consumed-wait; printf 'future-consumed-wait=%s\n' $status
jobs --quiet $owned_b; printf 'future-completed-jobs=%s\n' $status
"#,
        a.command(f, 37),
        b.command(f, 0),
        signal(f, &callback),
        signal(f, &root_ready)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        a.started();
        b.started();
        root_ready.wait();
        a.release.signal();
        callback.wait();
        b.assert_live();
        b.release.signal();
        shell.join().unwrap()
    });
    let out = success(result, "exactly-once/reentrant lifecycle events");
    assert_eq!(one(&out, "process-status"), "37");
    assert_eq!(one(&out, "callback-live"), "0");
    assert_eq!(one(&out, "nested"), "called");
    assert_eq!(one(&out, "active-wait"), "0");
    assert_eq!(
        one(&out, "counts"),
        "1,1,1,1,1,1",
        "late replay duplicated catch-all/lifecycle events: {out}"
    );
    assert_eq!(one(&out, "shell-exit"), "once");
    assert_ne!(one(&out, "future-consumed-wait"), "0");
    assert_ne!(one(&out, "future-completed-jobs"), "0");

    let out = success(f.eval("function caller_factory; function caller_once --on-job-exit caller; printf 'caller=once\\n'; end; printf 'value\\n'; end; set -l value (caller_factory); printf 'value=%s\\n' $value; true"), "caller exit event");
    assert_eq!(one(&out, "caller"), "once");
    assert_eq!(one(&out, "value"), "value");

    // An unrelated exit callback may exit the evaluator while a foreground
    // native child is still live. Neither its completion nor a wait result may
    // overwrite the callback's status or force-terminate accepted work.
    let mut trigger = Controlled::new(f, "callback-exit-trigger");
    let mut foreground = Controlled::new(f, "callback-exit-foreground");
    let script = format!(
        "{} &; set -l trigger $last_pid; function leave --on-process-exit $trigger; exit 7; end; {} >/dev/null 2>/dev/null; printf unreachable",
        trigger.command(f, 0),
        foreground.command(f, 0),
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        trigger.started();
        foreground.started();
        trigger.release.signal();
        trigger.wait(0);
        let result = shell.join().unwrap();
        foreground.assert_live();
        foreground.release.signal();
        foreground.wait(0);
        result
    });
    assert_eq!(
        result.code,
        7,
        "foreground wait overwrote callback exit: {}",
        String::from_utf8_lossy(&result.err)
    );
    assert!(
        result.out.is_empty(),
        "evaluation continued after callback exit: {:?}",
        result.out
    );
}

pub fn early_member_late_job(f: &Fixture) {
    let mut first = Controlled::new(f, "pipeline-first");
    let mut last = Controlled::new(f, "pipeline-last");
    let first_observed = Event::new("n5-first-member-observed");
    let registered = Event::new("n5-pipeline-registered");
    let late_registered = Event::new("n5-pipeline-late-handler-registered");
    let script = format!(
        r#"
set -g jobs_done 0; set -g late_done 0
{} | {} &; set -l last $last_pid
set -l members (jobs -p $last)
set -l first $members[1]
function first_exit --on-process-exit $first
    printf 'first-status=%s\n' $argv[3]
    {}
end
function last_exit --on-process-exit $last
    printf 'last-status=%s\n' $argv[3]
end
function pipeline_exit --on-job-exit $first
    set -g jobs_done (math $jobs_done + 1)
    printf 'pipeline-status=%s\n' $argv[3]
end
{}
wait $first
function late_job --on-job-exit $first
    set -g late_done (math $late_done + 1)
end
printf 'before-final=%s,%s\n' $jobs_done $late_done
{}
wait $last
printf 'after-final=%s,%s\n' $jobs_done $late_done
"#,
        first.command(f, 11),
        last.command(f, 19),
        signal(f, &first_observed),
        signal(f, &registered),
        signal(f, &late_registered)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        first.started();
        last.started();
        registered.wait();
        first.release.signal();
        first_observed.wait();
        late_registered.wait();
        last.assert_live();
        last.release.signal();
        shell.join().unwrap()
    });
    let out = success(result, "early pipeline member vs whole-job completion");
    assert_eq!(
        one(&out, "before-final"),
        "0,0",
        "early member fabricated whole-job completion"
    );
    assert_eq!(one(&out, "after-final"), "1,1");
    assert_eq!(one(&out, "first-status"), "11");
    assert_eq!(one(&out, "last-status"), "19");
    assert_eq!(
        one(&out, "pipeline-status"),
        "19",
        "pipeline status did not follow its actual last member"
    );
}

pub fn disown_shell_exit(f: &Fixture) {
    for policy in ["ordinary", "explicit", "default"] {
        let disowned = policy != "ordinary";
        let mut child = Controlled::new(f, &format!("{policy}-survivor"));
        let shell_ready = Event::new("n5-survival-shell-ready");
        let exit_shell = Event::new("n5-survival-exit-shell");
        let command = child.command(f, 0);
        let script = format!(
            "{command} >/dev/null 2>/dev/null &; set -l owned $last_pid; {}; {}; {} n5-jobs-wait {}; exit 0",
            match policy {
                "explicit" => "disown $owned",
                "default" => "disown",
                _ => "true",
            },
            signal(f, &shell_ready),
            f.probe(),
            quote(&exit_shell.name)
        );
        let result = std::thread::scope(|scope| {
            let shell = scope.spawn(|| f.eval(&script));
            child.started();
            shell_ready.wait();
            child.assert_live();
            exit_shell.signal();
            let result = shell.join().unwrap();
            // Assert actual survival after the shell process has exited before
            // deliberately releasing/cleaning up this retained owned object.
            child.assert_live();
            child.release.signal();
            child.wait(0);
            result
        });
        assert_eq!(
            result.code,
            0,
            "graceful shell exit status changed: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert!(result.out.is_empty());
        if disowned {
            assert!(
                result.err.is_empty(),
                "disowned work produced a shutdown warning: {}",
                String::from_utf8_lossy(&result.err)
            );
        } else {
            assert!(
                !result.err.is_empty(),
                "live ordinary job omitted graceful-exit warning"
            );
        }
    }
    let mut abrupt = std::process::Command::new(&f.probe);
    abrupt.env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value {
            abrupt.env(key, value);
        }
    }
    abrupt.arg("n5-jobs-abrupt-host").arg(&f.fish).arg(&f.root);
    exact(run(&mut abrupt, b""), 0, b"", b"");
    for args in ["--invalid", "-1", ""] {
        let result = f.eval(&format!("disown {args}"));
        assert_ne!(result.code, 0, "disown {args} succeeded");
        assert!(!result.err.is_empty(), "disown {args} omitted diagnostics");
    }
    let missing = f.eval("disown 2147483647");
    assert_eq!(
        missing.code, 0,
        "upstream disown diagnoses an absent valid PID without failing"
    );
    assert!(
        !missing.err.is_empty(),
        "unknown disown target omitted diagnostics"
    );
}

pub fn function_scheduling(f: &Fixture) {
    // An outer native member completes while its under-construction function
    // evaluates wait for another job. Completion must be serviced without a
    // sticky completion queue busy-loop, but the upstream synchronous function
    // itself must not pretend to have returned before its wait finishes.
    let mut other = Controlled::new(f, "function-other");
    let mut outer = Controlled::new(f, "function-outer");
    let entered = Event::new("n5-function-entered");
    let root_returned = Event::new("n5-function-root-returned");
    let shell_pid = f.root.join("function-shell.pid");
    let script = format!(
        r#"
printf '%s' $fish_pid >{}
{} &; set -g other $last_pid
function inside
    {}
    wait $other
    printf 'function=finished\n'
end
{} | inside &
{}
wait
printf 'root=usable\n'
"#,
        quote(shell_pid.to_str().unwrap()),
        other.command(f, 0),
        signal(f, &entered),
        outer.command(f, 0),
        signal(f, &root_returned)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        other.started();
        outer.started();
        entered.wait();
        outer.release.signal();
        outer.wait(0);
        super::support::assert_pending(&root_returned);
        super::spin::assert_wait_is_idle(&shell_pid);
        super::support::assert_pending(&root_returned);
        other.assert_live();
        other.release.signal();
        root_returned.wait();
        shell.join().unwrap()
    });
    let out = success(
        result,
        "under-construction completion while internal wait runs",
    );
    assert_eq!(one(&out, "function"), "finished");
    assert_eq!(one(&out, "root"), "usable");

    // The deferred producer can fill its pipe while the consumer waits for the
    // next root command. '&' must return without waiting for its pump to drain.
    let release = Event::new("n5-deferred-consumer-release");
    let ready = Event::new("n5-deferred-consumer-ready");
    let out = success(
        f.eval(&format!(
            r#"
set -l chunk abcdefghijklmnopqrstuvwxyz012345
for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; set chunk "$chunk$chunk"; end
printf '%s' "$chunk" | {} n5-jobs-deferred-consumer {} {} 2097152 &
{} n5-jobs-wait {}
{}
wait
printf 'deferred=complete\n'
"#,
            f.probe(),
            quote(&ready.name),
            quote(&release.name),
            f.probe(),
            quote(&ready.name),
            signal(f, &release)
        )),
        "deferred producer returns before pump drains",
    );
    assert_eq!(one(&out, "deferred"), "complete");

    // A final internal producer must forward its first line before evaluating
    // the rest of the function. A spool-until-function-return implementation
    // deadlocks against the native consumer's checked acknowledgement.
    for background in [false, true] {
        let ack = Event::new("n5-streaming-function-ack");
        let out = success(
            f.eval(&format!(
                r#"
function final_producer
    printf 'readiness\n'
    {} n5-jobs-wait {}
    printf 'tail\n'
end
final_producer | {} n5-jobs-streaming-consumer {} {}
{}
printf 'streaming=complete\n'
"#,
                f.probe(),
                quote(&ack.name),
                f.probe(),
                quote(&ack.name),
                if background { "&" } else { "" },
                if background { "wait" } else { "true" }
            )),
            "final function output streams before return",
        );
        assert_eq!(one(&out, "streaming"), "complete");
    }
}

pub fn retained_identity_and_rollback(f: &Fixture) {
    let mut cmd = f.command(&[]);
    cmd.args([
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new(&format!("{} n5-jobs-retained", f.probe())),
    ]);
    exact(
        run(&mut cmd, b""),
        0,
        b"retained-identity=stable;cache;callbacks-drained;partial-rollback;event-lease\n",
        b"",
    );
}

pub fn shell_partial_launch_rollback(f: &Fixture) {
    let mut cmd = std::process::Command::new(&f.probe);
    cmd.env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value {
            cmd.env(key, value);
        }
    }
    cmd.arg("n5-jobs-rollback-host").arg(&f.fish).arg(&f.root);
    exact(
        run(&mut cmd, b""),
        0,
        b"shell-partial=failed;roots-reaped;sentinel-live;host-live\n",
        b"",
    );
}

pub fn blocked_late_replay_removal(f: &Fixture) {
    let mut child = Controlled::new(f, "blocked-late");
    let observed = Event::new("n5-blocked-late-completed");
    let root = Event::new("n5-blocked-late-root");
    let script = format!(
        r#"
set -g late_count 0; set -g catch_count 0
{} &; set -g owned $last_pid
function catch_all --on-process-exit 0
    if test $argv[2] = $owned; set -g catch_count (math $catch_count + 1); end
end
function completed --on-process-exit $owned
    {}
end
{}
{} n5-jobs-wait {}
block --global
function late --on-process-exit $owned
    set -g late_count (math $late_count + 1)
end
function removed_late --on-process-exit $owned
    printf 'removed-handler=called\n'
end
functions --erase removed_late
printf 'blocked=%s,%s\n' $late_count $catch_count
block --erase
printf 'unblocked=%s,%s\n' $late_count $catch_count
wait $owned
"#,
        child.command(f, 0),
        signal(f, &observed),
        signal(f, &root),
        f.probe(),
        quote(&observed.name)
    );
    let result = std::thread::scope(|scope| {
        let shell = scope.spawn(|| f.eval(&script));
        child.started();
        root.wait();
        child.release.signal();
        shell.join().unwrap()
    });
    let out = success(
        result,
        "targeted late replay blocking/removal/callback drain",
    );
    assert_eq!(
        one(&out, "blocked"),
        "0,1",
        "blocked late handler escaped the owning evaluator block"
    );
    assert_eq!(
        one(&out, "unblocked"),
        "1,1",
        "late replay was lost or broadcast to catch-all handlers"
    );
    assert!(
        records(&out, "removed-handler").is_empty(),
        "removed handler was called from a retained replay token"
    );
}

pub fn callback_disown_active_wait(f: &Fixture) {
    let mut cmd = std::process::Command::new(&f.probe);
    cmd.env_clear().current_dir(&f.root);
    for (key, value) in f.command(&[]).get_envs() {
        if let Some(value) = value {
            cmd.env(key, value);
        }
    }
    cmd.arg("n5-jobs-active-wait");
    exact(
        run(&mut cmd, b""),
        0,
        b"active-wait=retained;callback-once;disowned;no-future-events\n",
        b"",
    );
}
