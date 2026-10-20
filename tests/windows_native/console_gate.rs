//! Profile-matched native console acceptance and retained owned evidence.
use super::{Family, fixture::Fixture};
use std::path::PathBuf;
fn artifact(profile: &str, debug_key: &str, release_key: &str, built: &str) -> PathBuf {
    let debug = std::env::var_os(debug_key)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let built = PathBuf::from(built);
            // Cargo's executable constant follows the harness profile. A release
            // harness must not silently run release tools under its debug label.
            built
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("debug")
                .join(built.file_name().unwrap())
        });
    let selected = if profile == "debug" {
        debug
    } else {
        std::env::var_os(release_key)
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
        "required {profile} artifact missing: {}",
        selected.display()
    );
    selected
}
fn retain_owned_files(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(std::io::Error::other(
                "unexpected reparse point in owned native evidence",
            ));
        }
        if kind.is_dir() {
            retain_owned_files(&entry.path(), &to.join(entry.file_name()))?;
        } else if !entry
            .path()
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}
pub(super) fn gate(id: &str, groups: &[&[Family]]) {
    let phase = id.split_once('-').expect("phase-qualified native gate ID").0;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!groups.is_empty() && groups.iter().all(|group| !group.is_empty()));
    let family_count: usize = groups.iter().map(|group| group.len()).sum();
    let mut names = std::collections::BTreeSet::new();
    for &(name, _) in groups.iter().flat_map(|group| group.iter()) {
        assert!(names.insert(name), "duplicate {id}/{name}");
    }
    eprintln!(
        "DISCOVER {id} families={} profiles=2 ignored=0",
        family_count
    );
    let mut failures = Vec::new();
    let mut executed = 0;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = std::env::var_os(format!("FISH_NATIVE_{phase}_EVIDENCE_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("target/{}-gate-results", phase.to_ascii_lowercase()))
        })
        .join(format!("{}-{stamp}", std::process::id()));
    for profile in ["debug", "release"] {
        let fish = artifact(
            profile,
            "FISH_NATIVE_TEST_BIN",
            "FISH_NATIVE_TEST_RELEASE_BIN",
            env!("CARGO_BIN_EXE_fish"),
        );
        let probe = artifact(
            &profile,
            "FISH_NATIVE_TEST_PROBE",
            "FISH_NATIVE_TEST_RELEASE_PROBE",
            env!("CARGO_BIN_EXE_windows_native_probe"),
        );
        let key_reader = if id == "N6-TOOLS" {
            Some(artifact(
                &profile,
                "FISH_NATIVE_N6_KEY_READER",
                "FISH_NATIVE_N6_RELEASE_KEY_READER",
                env!("CARGO_BIN_EXE_fish_key_reader"),
            ))
        } else {
            None
        };
        eprintln!("RUN {profile} {id}");
        let before = failures.len();
        for &(name, exercise) in groups.iter().flat_map(|group| group.iter()) {
            let f = Fixture::new(&fish, &probe, &format!("{profile}-{id}-{name}"));
            if let Some(tool) = &key_reader {
                std::fs::copy(tool, f.root.join("fish_key_reader.exe"))
                    .expect("profile-matched standalone key reader");
            }
            eprintln!("RUN-CASE {profile} {id}/{name}");
            executed += 1;
            let exercised =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(&f))).is_ok();
            let retained = evidence.join(&profile).join(id).join(name);
            let retention = retain_owned_files(&f.root, &retained).and_then(|()| {
                std::fs::write(
                    retained.join("artifacts.txt"),
                    format!(
                        "profile={profile}\nfish={}\nprobe={}\nkey-reader={}\n",
                        fish.display(),
                        probe.display(),
                        key_reader
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "not-used".into())
                    ),
                )
            });
            eprintln!("EVIDENCE {profile} {id}/{name} {}", retained.display());
            if let Err(error) = &retention {
                eprintln!("EVIDENCE-ERROR {profile} {id}/{name}: {error}");
            }
            if exercised && retention.is_ok() {
                eprintln!("PASS-CASE {profile} {id}/{name}");
            } else {
                failures.push(format!("{profile} {id}/{name}"));
            }
        }
        if failures.len() == before {
            eprintln!("PASS {profile} {id}");
        }
    }
    assert_eq!(executed, family_count * 2);
    eprintln!(
        "EXECUTED {id} discovered={} executed={executed} ignored=0 failed={}",
        family_count * 2,
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "required native {id} families failed: {failures:?}"
    );
}
