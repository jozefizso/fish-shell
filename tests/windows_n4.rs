//! Required native N4 contracts, each using real debug and release executables.
#![cfg(windows)]
use std::path::Path;
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n4/history.rs"]
mod history;
#[path = "windows_n4/locale.rs"]
mod locale;
#[path = "windows_n2/native.rs"]
mod native;
#[path = "windows_n4/notifications.rs"]
mod notifications;
#[path = "windows_n4/startup.rs"]
mod startup;
#[path = "windows_n4/storage.rs"]
mod storage;
#[path = "windows_n4/universal.rs"]
mod universal;
use fixture::Fixture;

fn gate(id: &str, cases: &[(&str, fn(&Fixture))]) {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut failures = Vec::new();
    for (profile, fish) in fixture::artifacts() {
        eprintln!("RUN {profile} {id}");
        let before = failures.len();
        for &(name, exercise) in cases {
            // Every family owns fresh startup/config/state roots. A preceding
            // configuration test cannot manufacture another family's success.
            let fixture = Fixture::new(
                &fish,
                Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
                &format!("{profile}-{id}-{name}"),
            );
            eprintln!("RUN-CASE {profile} {id}/{name}");
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(&fixture))).is_ok()
            {
                eprintln!("PASS-CASE {profile} {id}/{name}");
            } else {
                failures.push(format!("{profile} {id}/{name}"));
            }
        }
        if before == failures.len() {
            eprintln!("PASS {profile} {id}");
        }
    }
    assert!(
        failures.is_empty(),
        "native persistent state families failed: {failures:?}"
    );
}

#[test]
#[allow(non_snake_case)]
fn N4_STARTUP() {
    gate(
        "N4-STARTUP",
        &[
            ("configuration-autoload-save", startup::exercise_startup),
            ("history-modes", history::exercise_startup),
        ],
    );
}

#[test]
#[allow(non_snake_case)]
fn N4_SHARING() {
    gate(
        "N4-SHARING",
        &[
            ("universal-values-events", universal::exercise_sharing),
            ("storage-transactions", storage::exercise_sharing),
            ("history-sessions", history::exercise_sharing),
            ("locale-timezone", locale::exercise),
        ],
    );
}

#[test]
#[allow(non_snake_case)]
fn N4_RECOVERY() {
    gate(
        "N4-RECOVERY",
        &[
            ("universal-formats-errors", universal::exercise_recovery),
            ("storage-crash-acl", storage::exercise_recovery),
            ("autoload-save-errors", startup::exercise_recovery),
            ("history-recovery", history::exercise_recovery),
            ("directory-notifications", notifications::exercise),
        ],
    );
}
