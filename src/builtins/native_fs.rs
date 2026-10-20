//! Narrow native operations required by the public, shared startup and funcsave scripts.
use super::prelude::*;
use crate::env::Environment as _;
use std::io;

// MD5 is a compatibility/cache checksum, not a security primitive. Keep the
// operation here so ordinary completion caches do not require Python/coreutils.
mod md5 {
    use std::{ffi::c_void, io::{self, Read}, ptr};
    type Handle = *mut c_void;
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(handle: *mut Handle, algorithm: *const u16, implementation: *const u16, flags: u32) -> i32;
        fn BCryptCloseAlgorithmProvider(handle: Handle, flags: u32) -> i32;
        fn BCryptCreateHash(algorithm: Handle, hash: *mut Handle, object: *mut u8, object_size: u32, secret: *const u8, secret_size: u32, flags: u32) -> i32;
        fn BCryptDestroyHash(hash: Handle) -> i32;
        fn BCryptHashData(hash: Handle, input: *const u8, size: u32, flags: u32) -> i32;
        fn BCryptFinishHash(hash: Handle, output: *mut u8, size: u32, flags: u32) -> i32;
    }
    fn checked(status: i32) -> io::Result<()> {
        if status < 0 { Err(io::Error::other(format!("native MD5 failed ({status:#x})"))) }
        else { Ok(()) }
    }
    struct Algorithm(Handle);
    impl Drop for Algorithm {
        fn drop(&mut self) { unsafe { BCryptCloseAlgorithmProvider(self.0, 0); } }
    }
    struct Hash(Handle);
    impl Drop for Hash {
        fn drop(&mut self) { unsafe { BCryptDestroyHash(self.0); } }
    }
    pub(super) fn digest(input: &mut impl Read) -> io::Result<[u8; 16]> {
        let mut algorithm = ptr::null_mut();
        checked(unsafe { BCryptOpenAlgorithmProvider(&mut algorithm, [77u16, 68, 53, 0].as_ptr(), ptr::null(), 0) })?;
        let algorithm = Algorithm(algorithm);
        let mut hash = ptr::null_mut();
        // Windows 7+ owns the hash object storage when no object buffer is supplied.
        checked(unsafe { BCryptCreateHash(algorithm.0, &mut hash, ptr::null_mut(), 0, ptr::null(), 0, 0) })?;
        let hash = Hash(hash);
        let cancellation = crate::signal::console_control_cancellation();
        let mut bytes = [0u8; 65536];
        loop {
            if cancellation.as_ref().is_some_and(|event| event.is_cancelled())
                || crate::signal::signal_check_cancel() != 0 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = input.read(&mut bytes)?;
            if count == 0 { break; }
            checked(unsafe { BCryptHashData(hash.0, bytes.as_ptr(), count as u32, 0) })?;
        }
        let mut output = [0u8; 16];
        checked(unsafe { BCryptFinishHash(hash.0, output.as_mut_ptr(), output.len() as u32, 0) })?;
        Ok(output)
    }
}

pub fn native_fs(
    parser: &mut Parser,
    streams: &mut IoStreams,
    args: &mut [&wstr],
) -> BuiltinResult {
    let make_temporary = args.len() == 2
        && [L!("make-temp-directory"), L!("make-temp-file")].contains(&args[1]);
    let digest_operation = (args.len() == 2 && args[1] == L!("md5"))
        || (args.len() == 3 && [L!("md5-file"), L!("md5-string")].contains(&args[1]));
    let path_operation = args.len() == 3
        && [
            L!("mkdir"),
            L!("remove"),
            L!("atomic-write"),
            L!("remove-temp-directory"),
            L!("read"),
            L!("is-text"),
            L!("transfer-temp-file"),
        ]
        .contains(&args[1]);
    if !make_temporary && !path_operation && !digest_operation {
        streams.err.appendln(L!(
            "__fish_native_fs: expected mkdir|remove|read|is-text|atomic-write|remove-temp-directory|transfer-temp-file|md5-file PATH, md5-string STRING, md5, or make-temp-directory|make-temp-file"
        ));
        return Err(STATUS_INVALID_ARGS);
    }
    let mut predicate = true;
    let result = (|| -> io::Result<()> {
        if digest_operation {
            let digest = if args[1] == L!("md5-string") {
                let text = fish_widestring::shell_text_to_osstring(args[2])?.into_string()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "MD5 string is not Unicode"))?;
                md5::digest(&mut text.as_bytes())?
            } else if args[1] == L!("md5-file") {
                let path = crate::path::native_resolve_path_for_environment(args[2], parser.vars())?;
                md5::digest(&mut std::fs::File::open(path)?)?
            } else {
                let input = streams.stdin_file.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "standard input is closed")
                })?;
                md5::digest(input)?
            };
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut encoded = [0u8; 33];
            for (index, byte) in digest.into_iter().enumerate() {
                encoded[index * 2] = HEX[(byte >> 4) as usize];
                encoded[index * 2 + 1] = HEX[(byte & 15) as usize];
            }
            encoded[32] = b'\n';
            if !streams.out.append_bytes(&encoded) { return Err(io::ErrorKind::BrokenPipe.into()); }
            return Ok(());
        }
        if make_temporary {
            let cwd = crate::path::native_absolute_path(&parser.vars().get_working_directory()?)?;
            let environment = parser.vars().export_native_env()?;
            let caller = parser.scope().native_helper_owner;
            let runtime = crate::proc::native_runtime(parser)?;
            let path = crate::native_adapters::create_helper_resource(
                args[1] == L!("make-temp-directory"), caller, &cwd, &environment, runtime)?;
            let path = crate::path::native_path_text(&path)?;
            if !streams.out.appendln(&path) {
                let native = crate::path::native_absolute_path(&path)?;
                crate::native_adapters::release_helper_resource(&native, false)?;
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            return Ok(());
        }
        let path = crate::path::native_resolve_path_for_environment(args[2], parser.vars())?;
        match args[1] {
            operation if operation == L!("mkdir") => std::fs::create_dir_all(path),
            operation if operation == L!("remove") => {
                if crate::native_adapters::release_helper_resource(&path, false)? { Ok(()) }
                else { std::fs::remove_file(path) }
            }
            operation if operation == L!("remove-temp-directory") => {
                if crate::native_adapters::release_helper_resource(&path, false)? { Ok(()) }
                else { crate::fs::remove_editor_directory(&path) }
            }
            operation if operation == L!("transfer-temp-file") => {
                if crate::native_adapters::release_helper_resource(&path, true)? { Ok(()) }
                else { Err(io::Error::new(io::ErrorKind::PermissionDenied, "not an owned transferable temporary file")) }
            }
            operation if operation == L!("is-text") => {
                use std::io::Read;
                // Source-editor eligibility needs only a bounded content probe,
                // not a Unix `file` command or extension-based executable guess.
                let mut input = std::fs::File::open(path)?;
                let mut bytes = [0u8; 65536];
                let mut size = 0;
                while size < bytes.len() {
                    let count = input.read(&mut bytes[size..])?;
                    if count == 0 { break; }
                    size += count;
                }
                let truncated = size == bytes.len() && input.read(&mut [0u8; 1])? != 0;
                let bytes = &bytes[..size];
                predicate = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
                    let little_endian = bytes[0] == 0xff;
                    let end = if truncated {
                        let pair = [bytes[size - 2], bytes[size - 1]];
                        let last = if little_endian { u16::from_le_bytes(pair) } else { u16::from_be_bytes(pair) };
                        if (0xd800..=0xdbff).contains(&last) { size - 2 } else { size }
                    } else { size };
                    let units = bytes[2..end].chunks_exact(2).map(|pair| {
                        if little_endian { u16::from_le_bytes([pair[0], pair[1]]) }
                        else { u16::from_be_bytes([pair[0], pair[1]]) }
                    });
                    (size % 2 == 0) && char::decode_utf16(units).all(|unit| unit.is_ok_and(|c| c != '\0'))
                } else {
                    !bytes.contains(&0) && match std::str::from_utf8(bytes) {
                        Ok(_) => true,
                        Err(error) => truncated && error.error_len().is_none(),
                    }
                };
                Ok(())
            }
            operation if operation == L!("read") => {
                use std::io::Read;
                let mut input = std::fs::File::open(path)?;
                let mut bytes = [0u8; 65536];
                loop {
                    let count = input.read(&mut bytes)?;
                    if count == 0 { break; }
                    if !streams.out.append_bytes(&bytes[..count]) {
                        return Err(io::ErrorKind::BrokenPipe.into());
                    }
                }
                Ok(())
            }
            operation if operation == L!("atomic-write") => {
                let path = crate::path::native_path_text(&path)?;
                let input = streams.stdin_file.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "standard input is closed")
                })?;
                use std::io::{Read, Write};
                let input = std::cell::RefCell::new(input);
                let cancellation = crate::signal::console_control_cancellation();
                crate::fs::rewrite_via_temporary_file(&path, |_, temporary| {
                    let cancelled = || cancellation.as_ref().is_some_and(|event| event.is_cancelled())
                        || crate::signal::signal_check_cancel() != 0;
                    let mut bytes = [0u8; 65536];
                    loop {
                        if cancelled() { return Err(io::ErrorKind::Interrupted.into()); }
                        let count = input.borrow_mut().read(&mut bytes)?;
                        if count == 0 { break; }
                        temporary.write_all(&bytes[..count])?;
                    }
                    if cancelled() { return Err(io::ErrorKind::Interrupted.into()); }
                    Ok(crate::fs::PotentialUpdate {
                        do_save: true,
                        data: (),
                    })
                })?;
                Ok(())
            }
            _ => unreachable!(),
        }
    })();
    match result {
        Ok(()) => if predicate { Ok(SUCCESS) } else { Err(STATUS_CMD_ERROR) },
        Err(error) => {
            streams.err.appendln(&fish_widestring::str2wcstring(format!(
                "__fish_native_fs: {} '{}': {error}",
                args[1],
                args.get(2).copied().unwrap_or(L!(""))
            )));
            Err(STATUS_CMD_ERROR)
        }
    }
}
