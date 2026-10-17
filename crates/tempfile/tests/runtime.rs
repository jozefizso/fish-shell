use fish_tempfile::{create_dir_with_retry, create_file_with_retry, new_dir, new_file};
use std::{cell::Cell, fs, io::Write as _};

/// W2-RUNTIME's retained-tempfile contract: collisions never clobber existing
/// identities, and dropping a successful owner actually removes its storage.
#[test]
fn w2_runtime_tempfile_collision_and_cleanup() {
    let root = new_dir().unwrap();
    let fixture = root.path().join("fish space 日本語 🦀");
    fs::create_dir(&fixture).unwrap();
    let occupied_file = fixture.join("occupied");
    let occupied_directory = fixture.join("directory");
    fs::write(&occupied_file, b"do not truncate\0\xff").unwrap();
    fs::create_dir(&occupied_directory).unwrap();
    fs::write(occupied_directory.join("marker"), b"do not remove").unwrap();

    let file_attempts = Cell::new(0);
    let file_path = fixture.join("exclusive-file");
    let (created_path, result) = create_file_with_retry(|| {
        let attempt = file_attempts.get();
        file_attempts.set(attempt + 1);
        if attempt == 0 {
            occupied_file.clone()
        } else {
            file_path.clone()
        }
    });
    let mut file = result.unwrap();
    file.write_all(b"new file").unwrap();
    drop(file);
    assert_eq!(file_attempts.get(), 2);
    assert_eq!(created_path, file_path);
    assert_eq!(fs::read(&created_path).unwrap(), b"new file");
    assert_eq!(fs::read(&occupied_file).unwrap(), b"do not truncate\0\xff");

    let directory_attempts = Cell::new(0);
    let directory_path = fixture.join("exclusive-directory");
    let directory = create_dir_with_retry(|| {
        let attempt = directory_attempts.get();
        directory_attempts.set(attempt + 1);
        match attempt {
            0 => occupied_file.clone(),
            1 => occupied_directory.clone(),
            _ => directory_path.clone(),
        }
    })
    .unwrap();
    assert_eq!(directory_attempts.get(), 3);
    assert_eq!(directory.path(), &directory_path);
    fs::write(directory.path().join("owned-child"), b"temporary").unwrap();
    drop(directory);
    assert!(!directory_path.exists());
    assert_eq!(fs::read(&occupied_file).unwrap(), b"do not truncate\0\xff");
    assert_eq!(
        fs::read(occupied_directory.join("marker")).unwrap(),
        b"do not remove"
    );

    let mut temporary_file = new_file().unwrap();
    temporary_file.get_mut().write_all(b"temporary").unwrap();
    let temporary_path = temporary_file.path().clone();
    assert_eq!(fs::read(&temporary_path).unwrap(), b"temporary");
    drop(temporary_file);
    assert!(!temporary_path.exists());

    let root_path = root.path().clone();
    drop(root);
    assert!(!root_path.exists());
}

#[test]
fn creation_errors_are_not_retried_as_collisions() {
    let root = new_dir().unwrap();
    let missing_parent = root.path().join("missing-parent").join("child");
    let attempts = Cell::new(0);
    let result = create_dir_with_retry(|| {
        attempts.set(attempts.get() + 1);
        missing_parent.clone()
    });
    let error = match result {
        Ok(_) => panic!("creation beneath a missing parent unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(attempts.get(), 1);
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.raw_os_error().is_some());
    assert!(!missing_parent.exists());
}

#[cfg(unix)]
#[test]
fn temporary_directories_are_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = new_dir().unwrap();
    let mode = fs::metadata(directory.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0);
}
