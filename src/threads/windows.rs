//! Main-thread identity for the synchronous native Windows runtime.
//!
//! Thread pools, signal masks and post-fork state belong to the Unix backend.

use std::sync::OnceLock;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

static MAIN_THREAD_ID: OnceLock<u32> = OnceLock::new();

fn thread_id() -> u32 {
    // This API has no failure return and does not allocate or create a handle.
    unsafe { GetCurrentThreadId() }
}

/// Register the startup thread before creating any worker threads.
pub fn init() {
    MAIN_THREAD_ID
        .set(thread_id())
        .expect("threads::init() must only be called once (at startup)!");
}

pub fn is_main_thread() -> bool {
    thread_id()
        == *MAIN_THREAD_ID
            .get()
            .expect("threads::init() was not called at startup!")
}

pub fn assert_is_main_thread() {
    assert!(is_main_thread(), "Function is not running on the main thread!");
}

pub fn assert_is_background_thread() {
    assert!(
        !is_main_thread(),
        "Function is not allowed to be called on the main thread!"
    );
}
