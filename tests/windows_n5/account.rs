//! Real N5_ACCOUNT consumers; the probe exercises owner APIs in an isolated process.
use super::fixture::{Captured, Fixture, exact, quote, run};
use super::native::Event;
use std::ffi::OsStr;
use std::process::Command;

fn probe_command(f: &Fixture, mode: &str) -> Command {
    let environment = f.command(&[]);
    let mut command = Command::new(&f.probe);
    command.env_clear().current_dir(&f.root).arg(mode);
    for (key, value) in environment.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    command
}

pub fn owner_counters(f: &Fixture) {
    exact(
        run(&mut probe_command(f, "n5-account-owner"), b""),
        0,
        b"account-owner-ok\n",
        b"",
    );
}

pub fn owner_faults(f: &Fixture) {
    exact(
        run(&mut probe_command(f, "n5-account-faults"), b""),
        0,
        b"account-faults-ok\n",
        b"",
    );
}

pub fn output_failure(f: &Fixture) {
    exact(
        run(probe_command(f, "n5-account-cli-fault").arg(&f.fish), b""),
        0,
        b"account-cli-fault-ok\n",
        b"",
    );
}

fn number(value: &str) -> f64 {
    let value: f64 = value.parse().expect("numeric native counter");
    assert!(
        value.is_finite() && value >= 0.0,
        "nonnegative finite counter"
    );
    value
}

fn seconds(value: &str, unit: &str) -> f64 {
    number(value)
        * match unit {
            "micros" => 0.000001,
            "millis" => 0.001,
            "secs" => 1.0,
            "mins" => 60.0,
            _ => panic!("undeclared timer unit {unit}"),
        }
}

fn timed(result: Captured, output: &[u8], fish_work: bool, child_work: bool) {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert_eq!(result.out, output);
    let err = String::from_utf8(result.err).unwrap();
    let wall = err
        .lines()
        .find_map(|line| line.trim().strip_prefix("Executed in"))
        .expect("elapsed time");
    let wall: Vec<_> = wall.split_whitespace().collect();
    let elapsed = seconds(wall[0], wall[1]);
    assert!(elapsed > 0.0 && elapsed < 30.0);
    assert!(
        wall.contains(&"fish") && wall.contains(&"external"),
        "CPU attribution columns"
    );
    let mut cpu = [0.0; 3];
    for label in ["usr time", "sys time"] {
        let row = err
            .lines()
            .find_map(|line| line.trim().strip_prefix(label))
            .expect("CPU row");
        let values: Vec<_> = row.split_whitespace().collect();
        assert_eq!(values.len(), 6);
        let values = [
            seconds(values[0], values[1]),
            seconds(values[2], values[3]),
            seconds(values[4], values[5]),
        ];
        // Display rounding differs by column; allow two hundredths of each displayed unit.
        let tolerance = seconds("0.02", row.split_whitespace().nth(1).unwrap())
            + seconds("0.02", row.split_whitespace().nth(3).unwrap())
            + seconds("0.02", row.split_whitespace().nth(5).unwrap());
        assert!((values[0] - values[1] - values[2]).abs() <= tolerance);
        for i in 0..3 {
            cpu[i] += values[i];
        }
    }
    assert!(
        cpu[0] <= elapsed * 256.0 + 1.0,
        "plausible CPU seconds, not FILETIME ticks"
    );
    if fish_work {
        assert!(cpu[1] > 0.0, "builtin work must charge fish CPU");
    }
    if child_work {
        assert!(cpu[2] >= 0.10, "real CPU child must be collected");
    }
    assert!(
        err.contains("scope")
            && err.contains("concurrent")
            && err.contains("nested")
            && err.contains("cutoff")
    );
}

pub fn builtin_time(f: &Fixture) {
    timed(f.eval("time begin; set -l n 0; while test $n -lt 20000; set n (math $n + 1); end; echo builtin-done; end"), b"builtin-done\n", true, false);
}

pub fn native_time(f: &Fixture) {
    timed(
        f.eval(&format!("time {} n5-account-cpu", f.probe())),
        b"account-work\n",
        false,
        true,
    );
}

pub fn concurrent_pipeline_time(f: &Fixture) {
    let p = f.probe();
    timed(f.eval(&format!("time begin; {p} n5-account-cpu &; set -l first $last_pid; {p} n5-account-cpu | {p} n5-account-copy; wait $first; end")), b"account-work\naccount-work\n", false, true);
}

pub fn nested_time(f: &Fixture) {
    timed(
        f.eval(&format!("time {} n5-account-nested", f.probe())),
        b"account-work\n",
        false,
        true,
    );
}

pub fn cached_completion_time(f: &Fixture) {
    // The observer retains the real process handle before releasing its exit.
    // Its bounded OS wait ensures the CPU child completed before the timer starts.
    let ready = Event::new("account-cache-ready");
    let release = Event::new("account-cache-release");
    let p = f.probe();
    let result = f.eval(&format!("{p} n5-account-cached-child {} {} &; set -l saved $last_pid; {p} n5-account-observe $saved {} {}; or exit 91; time wait $saved",
        quote(&ready.name), quote(&release.name), quote(&ready.name), quote(&release.name)));
    let report = String::from_utf8(result.err.clone()).unwrap();
    let mut external = 0.0;
    for label in ["usr time", "sys time"] {
        let row = report
            .lines()
            .find_map(|line| line.trim().strip_prefix(label))
            .expect("cached wait timer");
        let words: Vec<_> = row.split_whitespace().collect();
        external += seconds(words[4], words[5]);
    }
    assert!(
        external < 0.10,
        "cached completed CPU must not be recharged to a later interval"
    );
    timed(result, b"account-work\n", false, false);
}

pub fn self_usage(f: &Fixture) {
    for status in [0, 37] {
        let script = format!(
            "set -l n 0; while test $n -lt 10000; set n (math $n + 1); end; printf 'normal-output\\n'; exit {status}"
        );
        let result = run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("--print-rusage-self"),
                OsStr::new("-c"),
                OsStr::new(&script),
            ]),
            b"",
        );
        assert_eq!(result.code, status);
        assert_eq!(result.out, b"normal-output\n");
        let err = String::from_utf8(result.err).unwrap();
        assert!(err.contains("shell process only") && err.contains("native Windows"));
        let value = |label: &str| -> f64 {
            let row = err
                .lines()
                .find_map(|line| line.trim().strip_prefix(label))
                .unwrap_or_else(|| panic!("missing {label}: {err}"));
            let words: Vec<_> = row.split_whitespace().collect();
            assert_eq!(words[1], "seconds");
            number(words[0])
        };
        let elapsed = value("elapsed:");
        let cpu = value("user CPU:") + value("kernel CPU:");
        assert!(elapsed > 0.0 && elapsed < 30.0 && cpu > 0.0 && cpu < elapsed * 256.0 + 1.0);
        for label in ["peak working set:", "working set:", "peak pagefile usage:"] {
            let row = err
                .lines()
                .find_map(|line| line.trim().strip_prefix(label))
                .expect("memory measurement");
            let words: Vec<_> = row.split_whitespace().collect();
            number(words[0]);
            assert_eq!(words[1], "bytes");
        }
        let unavailable = err
            .lines()
            .find_map(|line| line.trim().strip_prefix("unavailable:"))
            .expect("explicit unavailable POSIX categories");
        for category in [
            "minor/major",
            "swaps",
            "block I/O",
            "IPC",
            "signals",
            "context switches",
            "memory integrals",
        ] {
            assert!(
                unavailable.contains(category),
                "missing unavailable category {category}"
            );
        }
        for label in ["I/O read:", "I/O write:", "I/O other:"] {
            let row = err
                .lines()
                .find_map(|line| line.trim().strip_prefix(label))
                .expect("native I/O counters");
            let words: Vec<_> = row.split_whitespace().collect();
            number(words[0]);
            number(words[2]);
            assert_eq!(words[1], "operations,");
            assert_eq!(words[3], "bytes");
        }
    }
}
