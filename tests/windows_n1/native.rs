//! Required native fixture provisioning; lack of capabilities is a hard failure.
use super::*;
use std::ffi::c_void;
use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle};
use std::ptr::null_mut;
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn check(ok: i32, operation: &str) {
    assert_ne!(
        ok,
        0,
        "required {operation}: {}",
        std::io::Error::last_os_error()
    );
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetFileInformationByHandle(
        handle: *mut c_void,
        class: i32,
        value: *const c_void,
        size: u32,
    ) -> i32;
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn LocalFree(value: *mut c_void) -> *mut c_void;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn SetFileSecurityW(path: *const u16, information: u32, descriptor: *const c_void) -> i32;
    fn GetFileSecurityW(
        path: *const u16,
        information: u32,
        descriptor: *mut c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;
}
pub(super) fn case_sensitive(path: &Path) {
    fs::create_dir(path).unwrap();
    let dir = fs::OpenOptions::new()
        .read(true)
        .access_mode(0x100)
        .custom_flags(0x02000000)
        .open(path)
        .expect("required case-sensitive-directory fixture handle");
    let flags = 1u32;
    check(
        unsafe {
            SetFileInformationByHandle(dir.as_raw_handle(), 23, (&flags as *const u32).cast(), 4)
        },
        "FileCaseSensitiveInfo (NTFS and case-sensitive-directory capability required)",
    );
    fs::write(path.join("Twin"), b"upper").unwrap();
    fs::write(path.join("twin"), b"lower").unwrap();
    assert_eq!(fs::read(path.join("Twin")).unwrap(), b"upper");
    assert_eq!(fs::read(path.join("twin")).unwrap(), b"lower");
    assert!(fish::wutil::native_directory_case_sensitive(&path_text(path)).unwrap());
}
pub(super) fn junction(link: &Path, target: &Path) {
    fs::create_dir(link).unwrap();
    let target = fs::canonicalize(target).unwrap();
    let target = target
        .to_str()
        .unwrap()
        .strip_prefix("\\\\?\\")
        .unwrap()
        .to_owned();
    let substitute: Vec<u16> = format!("\\??\\{target}").encode_utf16().collect();
    let print: Vec<u16> = target.encode_utf16().collect();
    let mut data = Vec::new();
    data.extend_from_slice(&0xa0000003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
    data.extend_from_slice(&((8 + (substitute.len() + print.len() + 2) * 2) as u16).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    for value in [
        0,
        (substitute.len() * 2) as u16,
        ((substitute.len() + 1) * 2) as u16,
        (print.len() * 2) as u16,
    ] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    for unit in substitute
        .into_iter()
        .chain(Some(0))
        .chain(print)
        .chain(Some(0))
    {
        data.extend_from_slice(&unit.to_le_bytes());
    }
    let dir = fs::OpenOptions::new()
        .write(true)
        .custom_flags(0x02200000)
        .open(link)
        .expect("required junction fixture handle");
    let mut returned = 0;
    check(
        unsafe {
            DeviceIoControl(
                dir.as_raw_handle(),
                0x000900a4,
                data.as_ptr().cast(),
                data.len() as u32,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        },
        "junction creation",
    );
    assert_eq!(
        fish::wutil::native_file_identity(&path_text(link), true).unwrap(),
        fish::wutil::native_file_identity(&path_text(Path::new(&target)), true).unwrap()
    );
}
pub(super) struct Denied {
    path: PathBuf,
    descriptor: Vec<u8>,
}
impl Denied {
    pub fn new(path: &Path, directory: bool) -> Self {
        if directory {
            fs::create_dir(path).unwrap();
            fs::write(path.join("hidden.txt"), b"secret").unwrap();
        } else {
            fs::write(path, b"secret").unwrap();
        }
        let path_wide = wide(path);
        let mut needed = 0;
        unsafe {
            GetFileSecurityW(path_wide.as_ptr(), 4, null_mut(), 0, &mut needed);
        }
        assert!(needed > 0, "required original DACL query");
        let mut descriptor = vec![0u8; needed as usize];
        check(
            unsafe {
                GetFileSecurityW(
                    path_wide.as_ptr(),
                    4,
                    descriptor.as_mut_ptr().cast(),
                    needed,
                    &mut needed,
                )
            },
            "original DACL capture",
        );
        let text: Vec<u16> = "D:P(D;;FA;;;WD)".encode_utf16().chain(Some(0)).collect();
        let mut denied = null_mut();
        check(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text.as_ptr(),
                    1,
                    &mut denied,
                    null_mut(),
                )
            },
            "deny DACL conversion",
        );
        let ok = unsafe { SetFileSecurityW(path_wide.as_ptr(), 0x80000004, denied) };
        unsafe {
            LocalFree(denied);
        }
        check(ok, "deny DACL installation");
        let result = Self {
            path: path.to_owned(),
            descriptor,
        };
        let error = fs::File::open(path).expect_err("required denied fixture still readable");
        assert_eq!(error.raw_os_error(), Some(5));
        result
    }
}
impl Drop for Denied {
    fn drop(&mut self) {
        check(
            unsafe {
                SetFileSecurityW(
                    wide(&self.path).as_ptr(),
                    0x80000004,
                    self.descriptor.as_ptr().cast(),
                )
            },
            "owned fixture DACL restoration",
        );
    }
}
pub(super) struct Unc {
    pub path: PathBuf,
}
impl Unc {
    pub fn new(f: &Fixture) -> Self {
        let root = std::env::var_os("FISH_NATIVE_N1_UNC_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let root = f.root.to_str().unwrap();
                assert_eq!(
                    &root[1..3],
                    ":\\",
                    "UNC default requires local drive; set FISH_NATIVE_N1_UNC_ROOT"
                );
                PathBuf::from(format!("\\\\localhost\\{}$\\{}", &root[..1], &root[3..]))
            });
        assert!(
            root.to_str().unwrap().starts_with("\\\\"),
            "FISH_NATIVE_N1_UNC_ROOT must identify a real UNC share"
        );
        let path = root.join(format!("n1-unc-{}", std::process::id()));
        fs::create_dir(&path).expect("required writable UNC share unavailable; provision FISH_NATIVE_N1_UNC_ROOT (no skip permitted)");
        let fixture = Self { path };
        fs::write(fixture.path.join("share 雪.txt"), b"UNC").unwrap();
        fixture
    }
}
impl Drop for Unc {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn unknown_reparse(path: &Path) {
    fs::create_dir(path).unwrap();
    let dir = fs::OpenOptions::new()
        .write(true)
        .custom_flags(0x02200000)
        .open(path)
        .expect("required unknown reparse fixture handle");
    // User-owned, non-Microsoft reparse tags use REPARSE_GUID_DATA_BUFFER.
    let mut data = Vec::new();
    data.extend_from_slice(&0x42u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&[
        0x9a, 0x6b, 0x13, 0x3f, 0xd8, 0x11, 0x4f, 0x43, 0x87, 0x19, 0x6a, 0xb9, 0x01, 0xa3, 0x73,
        0x2c,
    ]);
    let mut returned = 0;
    check(
        unsafe {
            DeviceIoControl(
                dir.as_raw_handle(),
                0x000900a4,
                data.as_ptr().cast(),
                data.len() as u32,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        },
        "non-Microsoft GUID reparse fixture (supported NTFS required)",
    );
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandleEx(
        handle: *mut c_void,
        class: i32,
        value: *mut c_void,
        size: u32,
    ) -> i32;
}
pub(super) fn raw_identity(path: &Path) -> (u64, [u8; 16]) {
    #[repr(C)]
    struct FileIdInfo {
        volume: u64,
        id: [u8; 16],
    }
    let file = fs::File::open(path).unwrap();
    let mut info = FileIdInfo {
        volume: 0,
        id: [0; 16],
    };
    check(
        unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                18,
                (&mut info as *mut FileIdInfo).cast(),
                std::mem::size_of::<FileIdInfo>() as u32,
            )
        },
        "independent full FILE_ID_INFO query",
    );
    (info.volume, info.id)
}
