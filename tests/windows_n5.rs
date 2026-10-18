//! Mandatory real-native N5 gates. Each declared family runs against both fish artifacts.
//! Missing artifacts, real consoles, policy jobs or GUI fixtures are failures, never skips.
#![cfg(windows)]
use std::path::PathBuf;
#[path = "windows_n5/account.rs"]
mod account;
#[path = "windows_n5/cancel.rs"]
mod cancel;
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n5/jobs.rs"]
mod jobs;
#[path = "windows_n2/native.rs"]
mod native;
#[path = "windows_n5/spin.rs"]
mod spin;
#[path = "windows_n5/support.rs"]
mod support;
use fixture::Fixture;

type Family = (&'static str, fn(&Fixture));
const JOBS_FAMILIES: &[Family] = &[
    (
        "native-mixed-background-progress",
        jobs::background_progress,
    ),
    ("background-nul-explicit-file-pipe", jobs::background_stdin),
    (
        "visible-jobs-options-native-group-identities",
        jobs::visible_jobs_options,
    ),
    (
        "wait-cached-multi-any-duplicate-invalid",
        jobs::waits_cached_multi_any,
    ),
    (
        "process-job-caller-shell-events-late-reentrant",
        jobs::events_exactly_once_reentrant,
    ),
    (
        "callback-disown-after-active-wait-captures-handles",
        jobs::callback_disown_active_wait,
    ),
    (
        "early-member-not-whole-job-late-replay",
        jobs::early_member_late_job,
    ),
    (
        "late-targeted-replay-block-removal-callback-drain",
        jobs::blocked_late_replay_removal,
    ),
    (
        "disown-ordinary-graceful-shell-exit-survival",
        jobs::disown_shell_exit,
    ),
    (
        "under-construction-deferred-streaming-function-scheduling",
        jobs::function_scheduling,
    ),
    (
        "retained-pid-cache-callback-drain-partial-rollback",
        jobs::retained_identity_and_rollback,
    ),
    (
        "actual-shell-partial-pipeline-no-orphan",
        jobs::shell_partial_launch_rollback,
    ),
];
const ACCOUNT_FAMILIES: &[Family] = &[
    (
        "owner-process-job-counters-transition-cutoff",
        account::owner_counters,
    ),
    (
        "owner-query-arithmetic-output-faults",
        account::owner_faults,
    ),
    ("builtin-cpu-time", account::builtin_time),
    ("native-cpu-time", account::native_time),
    (
        "concurrent-native-pipeline-cpu-time",
        account::concurrent_pipeline_time,
    ),
    ("nested-child-cpu-time", account::nested_time),
    (
        "cached-completion-no-recharge",
        account::cached_completion_time,
    ),
    (
        "print-rusage-self-native-units-unavailable-categories",
        account::self_usage,
    ),
    (
        "time-rusage-output-failure-preserves-stdout-status",
        account::output_failure,
    ),
];

fn artifact(
    profile: &str,
    debug_override: &str,
    release_override: &str,
    built_debug: &str,
) -> PathBuf {
    let debug = std::env::var_os(debug_override)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(built_debug));
    let selected = if profile == "debug" {
        debug
    } else {
        std::env::var_os(release_override)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                debug
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("release")
                    .join(debug.file_name().unwrap())
            })
    };
    assert!(
        selected.is_file(),
        "mandatory {profile} fixture artifact missing: {}",
        selected.display()
    );
    selected
}

fn gate(id: &str, families: &[Family]) {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        !families.is_empty(),
        "mandatory {id} has no registered behavioral families"
    );
    let mut names = std::collections::BTreeSet::new();
    for &(name, _) in families {
        assert!(names.insert(name), "duplicate {id} family {name}");
    }
    let mut failures = Vec::new();
    let mut executed = 0;
    eprintln!(
        "DISCOVER {id} families={} profiles=2 ignored=0",
        families.len()
    );
    for (profile, fish) in fixture::artifacts() {
        let probe = artifact(
            &profile,
            "FISH_NATIVE_TEST_PROBE",
            "FISH_NATIVE_TEST_RELEASE_PROBE",
            env!("CARGO_BIN_EXE_windows_native_probe"),
        );
        let gui = (id == "N5-CANCEL").then(|| {
            artifact(
                &profile,
                "FISH_NATIVE_N5_GUI_FIXTURE",
                "FISH_NATIVE_N5_RELEASE_GUI_FIXTURE",
                env!("CARGO_BIN_EXE_windows_n5_gui_probe"),
            )
        });
        eprintln!("RUN {profile} {id}");
        let before = failures.len();
        for &(name, exercise) in families {
            let fixture = Fixture::new(&fish, &probe, &format!("{profile}-{id}-{name}"));
            if let Some(gui) = &gui {
                std::fs::copy(gui, fixture.root.join("windows_n5_gui_probe.exe"))
                    .expect("required profile-matched GUI fixture copy");
            }
            eprintln!("RUN-CASE {profile} {id}/{name}");
            executed += 1;
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(&fixture))).is_ok()
            {
                eprintln!("PASS-CASE {profile} {id}/{name}");
            } else {
                failures.push(format!("{profile} {id}/{name}"));
            }
        }
        if failures.len() == before {
            eprintln!("PASS {profile} {id}");
        }
    }
    assert_eq!(
        executed,
        families.len() * 2,
        "mandatory {id} omitted a declared artifact/family"
    );
    eprintln!(
        "EXECUTED {id} discovered={} executed={executed} ignored=0 failed={}",
        families.len() * 2,
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "required native {id} families failed: {failures:?}"
    );
}

#[test]
#[allow(non_snake_case)]
fn N5_JOBS() {
    gate("N5-JOBS", JOBS_FAMILIES);
}

#[test]
#[allow(non_snake_case)]
fn N5_CANCEL() {
    gate("N5-CANCEL", cancel::FAMILIES);
}

#[test]
#[allow(non_snake_case)]
fn N5_ACCOUNT() {
    gate("N5-ACCOUNT", ACCOUNT_FAMILIES);
}
