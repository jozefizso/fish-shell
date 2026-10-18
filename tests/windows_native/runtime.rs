//! Assertions run from the native probe's actual startup thread in an isolated process.

use std::{io::ErrorKind, sync::mpsc, time::Duration};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetLastError(error: u32);
}

pub fn run() {
    fish::threads::init();
    assert!(fish::threads::is_main_thread());
    fish::threads::assert_is_main_thread();

    let (send, receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        assert!(!fish::threads::is_main_thread());
        fish::threads::assert_is_background_thread();
        send.send(()).unwrap();
    });
    receive
        .recv_timeout(Duration::from_secs(10))
        .expect("worker failed or main-thread identity check stalled");
    worker.join().unwrap();
    assert!(fish::threads::is_main_thread());

    let missing = std::env::current_dir()
        .unwrap()
        .join("missing parent 日本語")
        .join("missing child");
    // Test the owning Win32 open boundary. Componentwise metadata traversal may fail
    // earlier with ERROR_FILE_NOT_FOUND instead of the full path's ERROR_PATH_NOT_FOUND.
    let error =
        fish::wutil::native_open(&missing, 0, true).expect_err("missing path unexpectedly exists");
    // Win32 ERROR_PATH_NOT_FOUND is 3, which is not Unix ENOENT. Capturing
    // this error in std::io::Error must preserve its native domain and value.
    unsafe { SetLastError(87) }; // ERROR_INVALID_PARAMETER, an unrelated later error.
    assert_eq!(error.raw_os_error(), Some(3));
    assert_eq!(error.kind(), ErrorKind::NotFound);
}
