//! Owned-process save barriers. These call the production transaction and stop inside its real
//! rewrite closure; no environment switch or alternate persistence implementation exists.
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString, c_void},
    fs,
    io::{self, Read as _, Write as _},
    os::windows::{
        ffi::OsStrExt as _,
        io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    path::Path,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(object: *mut c_void, millis: u32) -> u32;
}

fn event(name: &OsStr, access: u32) -> io::Result<OwnedHandle> {
    let name: Vec<u16> = name.encode_wide().chain(Some(0)).collect();
    let raw = unsafe { OpenEventW(access, 0, name.as_ptr()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

pub fn barrier(ready: &OsStr, release: &OsStr) -> io::Result<()> {
    let ready = event(ready, 2)?;
    let release = event(release, 0x0010_0000)?;
    if unsafe { SetEvent(ready.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    match unsafe { WaitForSingleObject(release.as_raw_handle(), 30_000) } {
        0 => Ok(()),
        0xffff_ffff => Err(io::Error::last_os_error()),
        result => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("native save-stage release wait failed ({result:#x})"),
        )),
    }
}

fn argument(args: &mut impl Iterator<Item = OsString>, label: &str) -> io::Result<OsString> {
    args.next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("missing {label}")))
}

fn text(value: &OsStr) -> io::Result<&str> {
    value.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage key/value/stage must be Unicode",
        )
    })
}

fn staged_write(
    file: &mut fs::File,
    bytes: &[u8],
    ready: &OsStr,
    release: &OsStr,
    stage: &str,
) -> io::Result<()> {
    match stage {
        "partial" => {
            let half = bytes.len() / 2;
            file.write_all(&bytes[..half])?;
            file.flush()?;
            barrier(ready, release)?;
            file.write_all(&bytes[half..])?;
        }
        "complete" => {
            file.write_all(bytes)?;
            file.flush()?;
            barrier(ready, release)?;
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected partial|complete",
            ));
        }
    }
    Ok(())
}

pub fn run(mode: &str, mut args: impl Iterator<Item = OsString>) -> io::Result<()> {
    let path = argument(&mut args, "target path")?;
    let path = fish::path::native_path_text(Path::new(&path))?;
    if mode == "n4-rewrite" {
        let payload = fs::read(argument(&mut args, "serialized payload file")?)?;
        let ready = argument(&mut args, "ready event")?;
        let release = argument(&mut args, "release event")?;
        let stage = argument(&mut args, "partial|complete")?;
        if args.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unexpected rewrite argument",
            ));
        }
        fish::fs::rewrite_via_temporary_file(&path, |_old, temporary| {
            staged_write(temporary, &payload, &ready, &release, text(&stage)?)?;
            Ok(fish::fs::PotentialUpdate {
                do_save: true,
                data: (),
            })
        })?;
        return Ok(());
    }
    let key = argument(&mut args, "key")?;
    let value = argument(&mut args, "value")?;
    let ready = argument(&mut args, "ready event")?;
    let release = argument(&mut args, "release event")?;
    let stage = if mode == "n4-storage-rewrite" {
        Some(argument(&mut args, "partial|complete")?)
    } else if mode == "n4-storage-append" {
        None
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown storage probe mode",
        ));
    };
    if args.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unexpected storage argument",
        ));
    }
    if stage.is_none() {
        barrier(&ready, &release)?;
    }
    fish::fs::rewrite_via_temporary_file(&path, |mut old, temporary| {
        let mut source = String::new();
        old.read_to_string(&mut source)?;
        let mut entries = BTreeMap::new();
        for line in source.lines() {
            let (key, value) = line.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid storage fixture key/value",
                )
            })?;
            entries.insert(key.to_owned(), value.to_owned());
        }
        entries.insert(text(&key)?.to_owned(), text(&value)?.to_owned());
        let mut bytes = Vec::new();
        for (key, value) in entries {
            writeln!(bytes, "{key}={value}")?;
        }
        if let Some(stage) = &stage {
            staged_write(temporary, &bytes, &ready, &release, text(stage)?)?;
        } else {
            temporary.write_all(&bytes)?;
        }
        Ok(fish::fs::PotentialUpdate {
            do_save: true,
            data: (),
        })
    })?;
    Ok(())
}
