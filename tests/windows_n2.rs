//! Required real-native N2 acceptance gates; unavailable infrastructure is a failure.
#![cfg(windows)]
use std::path::Path;
#[path = "windows_native/fixture.rs"]
mod fixture;
#[path = "windows_n2/lifetime.rs"]
mod lifetime;
#[path = "windows_n2/native.rs"]
mod native;
#[path = "windows_n2/redirect.rs"]
mod redirect;
#[path = "windows_n2/stream.rs"]
mod stream;
use fixture::Fixture;

fn gate(id: &str, exercise: fn(&Fixture)) {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut count = 0;
    for (profile, fish) in fixture::artifacts() {
        let fixture = Fixture::new(
            &fish,
            Path::new(env!("CARGO_BIN_EXE_windows_native_probe")),
            &format!("{profile}-{id}"),
        );
        eprintln!("RUN {profile} {id}");
        exercise(&fixture);
        count += 1;
        eprintln!("PASS {profile} {id}");
    }
    assert_eq!(count, 2, "both native artifacts are required");
}
#[test]
#[allow(non_snake_case)]
fn N2_STREAM() {
    gate("N2-STREAM", stream::exercise);
}
#[test]
#[allow(non_snake_case)]
fn N2_REDIRECT() {
    gate("N2-REDIRECT", redirect::exercise);
}
#[test]
#[allow(non_snake_case)]
fn N2_LIFETIME() {
    gate("N2-LIFETIME", lifetime::exercise);
}
