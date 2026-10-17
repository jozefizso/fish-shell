//! Assertions run from the native probe's actual startup thread in an isolated process.

use std::{ffi::CStr, io::ErrorKind, sync::mpsc, time::Duration};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetLastError(error: u32);
}

pub fn run() {
    fish::threads::init();
    assert!(fish::threads::is_main_thread());
    fish::threads::assert_is_main_thread();

    // The selected runtime deliberately ignores inherited locale categories.
    // This process has not created any locale-using worker yet.
    assert!(unsafe { fish::locale::set_libc_locales(false) });
    let current_locale = unsafe { libc::setlocale(libc::LC_ALL, std::ptr::null()) };
    assert!(!current_locale.is_null());
    assert_eq!(unsafe { CStr::from_ptr(current_locale) }.to_bytes(), b"C");
    let numeric = fish::locale::get_numeric_locale();
    assert_eq!(numeric.decimal_point, '.');
    assert_eq!(numeric.thousands_sep, None);
    assert_eq!(numeric.grouping, [0; 4]);
    assert!(!numeric.group_repeat);

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
    let wide = fish_widestring::osstr2wcstring(&missing).unwrap();
    let error = fish::wutil::wstat(&wide).expect_err("missing path unexpectedly exists");
    // Win32 ERROR_PATH_NOT_FOUND is 3, which is not Unix ENOENT. Capturing
    // this error in std::io::Error must preserve its native domain and value.
    unsafe { SetLastError(87) }; // ERROR_INVALID_PARAMETER, an unrelated later error.
    assert_eq!(error.raw_os_error(), Some(3));
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert_eq!(
        error.to_string(),
        std::io::Error::from_raw_os_error(3).to_string()
    );
}
