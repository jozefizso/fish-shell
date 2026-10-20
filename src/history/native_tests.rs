//! Private history transition tests use controlled timestamps, not sleeps or
//! another process's wall clock. Each case runs in its own native evaluator thread.
use super::*;
use std::{
    os::windows::fs::OpenOptionsExt,
    time::{Duration, UNIX_EPOCH},
};

fn history(directory: &std::path::Path) -> HistoryImpl {
    let mut history = HistoryImpl::new(
        L!("generation").to_owned(),
        Some(crate::path::native_path_text(directory).unwrap()),
    );
    history.load_old_if_needed();
    history
}

fn item(command: &wstr, seconds: u64) -> HistoryItem {
    let time = UNIX_EPOCH + Duration::from_secs(seconds);
    HistoryItem::new(
        command.to_owned(),
        Timestamps {
            first_added: time,
            last_added: time,
        },
        PersistenceMode::Disk,
    )
}

#[test]
fn native_clear_session_after_failed_duplicate_save() {
    if !crate::threads::isolated_test(concat!(
        module_path!(),
        "::native_clear_session_after_failed_duplicate_save"
    )) {
        return;
    }
    for nonconsecutive in [false, true] {
        let directory = fish_tempfile::new_dir().unwrap();
        let mut active = history(directory.path());
        active.add(item(L!("owned"), 10), false, false);
        active.save(false).unwrap();
        let path = directory.path().join("generation_history");
        let original = std::fs::read(&path).unwrap();
        let denied = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        if nonconsecutive {
            active.add(item(L!("between"), 15), false, false);
        }
        active.add(item(L!("owned"), 20), false, false);
        assert!(active.save(false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        drop(denied);
        active.clear_session();
        active.save(false).unwrap();
        assert!(
            history(directory.path()).get_history().is_empty(),
            "clear-session retained an earlier acknowledged version after the latest duplicate save failed"
        );
    }
}

#[test]
fn native_clear_session_does_not_resurrect_foreign_deletion() {
    if !crate::threads::isolated_test(concat!(
        module_path!(),
        "::native_clear_session_does_not_resurrect_foreign_deletion"
    )) {
        return;
    }
    let directory = fish_tempfile::new_dir().unwrap();
    let mut seed = history(directory.path());
    seed.add(item(L!("baseline"), 10), false, false);
    seed.save(false).unwrap();
    let mut active = history(directory.path());
    let mut foreign = history(directory.path());
    foreign.remove(L!("baseline"));
    foreign.save(false).unwrap();
    assert!(history(directory.path()).get_history().is_empty());
    assert_eq!(
        active.get_history(),
        [L!("baseline").to_owned()],
        "another shell must not silently replace our pinned read baseline"
    );
    active.add(item(L!("baseline"), 20), false, false);
    active.save(false).unwrap();
    active.clear_session();
    active.save(false).unwrap();
    assert!(
        history(directory.path()).get_history().is_empty(),
        "clearing a re-created command resurrected another session's acknowledged deletion"
    );
}

#[test]
fn native_clear_session_restores_foreign_predecessor() {
    if !crate::threads::isolated_test(concat!(
        module_path!(),
        "::native_clear_session_restores_foreign_predecessor"
    )) {
        return;
    }
    let directory = fish_tempfile::new_dir().unwrap();
    let mut seed = history(directory.path());
    seed.add(item(L!("shared"), 10), false, false);
    seed.save(false).unwrap();
    let mut active = history(directory.path());
    active.add(item(L!("shared"), 20), false, false);
    active.save(false).unwrap();
    let mut foreign = history(directory.path());
    foreign.add(item(L!("shared"), 30), false, false);
    foreign.save(false).unwrap();
    active.add(item(L!("shared"), 40), false, false);
    active.save(false).unwrap();
    active.clear_session();
    active.save(false).unwrap();
    let mut restarted = history(directory.path());
    let restored = restarted.item_at_index(1).unwrap();
    assert_eq!(restored.str(), L!("shared"));
    assert_eq!(
        restored.last_added_timestamp(),
        UNIX_EPOCH + Duration::from_secs(30),
        "undoing our repeat must restore the actual intervening foreign generation, not an obsolete baseline"
    );
}

#[test]
fn native_reader_history_snapshot_is_owned_revision_cached_and_pending_aware() {
    if !crate::threads::isolated_test(concat!(module_path!(), "::native_reader_history_snapshot_is_owned_revision_cached_and_pending_aware")) {
        return;
    }
    let source = History::new_with_directory(HistoryId::Memory(MemoryHistoryId::PrivateMode), None);
    source.add(item(L!("first"), 1), false);
    let revision = source.revision();
    let snapshot = source.reader_snapshot();
    assert!(Arc::ptr_eq(&snapshot, &source.reader_snapshot()));
    assert_eq!(snapshot.get_history(), vec![L!("first").to_owned()]);
    source.size();
    source.get_history();
    source.resolve_pending();
    assert_eq!(source.revision(), revision, "ordinary reads or no-op resolve changed revision");
    source.add(item(L!("pending"), 2), true);
    let pending = source.reader_snapshot();
    assert_eq!(pending.get_history(), vec![L!("first").to_owned()]);
    let before_resolve = source.revision();
    source.resolve_pending();
    assert_ne!(source.revision(), before_resolve);
    assert_eq!(source.reader_snapshot().get_history(), vec![L!("pending").to_owned(), L!("first").to_owned()]);
    source.remove(L!("first"));
    assert_eq!(source.reader_snapshot().get_history(), vec![L!("pending").to_owned()]);
    assert_eq!(snapshot.get_history(), vec![L!("first").to_owned()], "mutation rewrote a worker's pinned history");
}

#[test]
fn native_history_search_cancels_between_nonmatching_entries() {
    if !crate::threads::isolated_test(concat!(module_path!(), "::native_history_search_cancels_between_nonmatching_entries")) {
        return;
    }
    let source = History::new_with_directory(HistoryId::Memory(MemoryHistoryId::BuiltinRead), None);
    for index in 0..128 {
        let command = WString::from(format!("entry-{index}").as_str());
        source.imp().add(item(&command, index + 1), false, false);
    }
    let mut search = HistorySearch::new_with(
        source.reader_snapshot(), L!("absent").to_owned(), SearchType::Prefix, SearchFlags::default(), 0,
    );
    let checks = std::cell::Cell::new(0);
    let cancel = || {
        checks.set(checks.get() + 1);
        checks.get() >= 7
    };
    assert!(!search.go_to_next_match_with_cancel(SearchDirection::Backward, &cancel));
    assert_eq!(checks.get(), 7, "cancellation was not checked per scanned entry");
    let mut search = HistorySearch::new_with(
        source.reader_snapshot(), L!("entry-").to_owned(), SearchType::Prefix, SearchFlags::default(), 0,
    );
    assert!(search.go_to_next_match(SearchDirection::Backward));
    assert_eq!(search.current_string(), L!("entry-127"));
    for _ in 0..32 {
        assert!(search.go_to_next_match(SearchDirection::Backward));
    }
    checks.set(0);
    search.search_forward_with_cancel(128, &cancel);
    assert_eq!(checks.get(), 7, "forward search ignored cancellation");
}
