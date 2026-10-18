//! Consumer-visible native state transaction cases. All paths and children are fixture-owned.
use super::fixture::{Fixture, exact, run};
use std::{
    ffi::{OsStr, OsString, c_void},
    fs::{self, OpenOptions},
    io::{self, Read as _, Seek as _, SeekFrom, Write as _},
    os::windows::{
        ffi::OsStrExt as _,
        fs::OpenOptionsExt as _,
        io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread::JoinHandle,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        security: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn WaitForSingleObject(object: *mut c_void, millis: u32) -> u32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn ReplaceFileW(
        replaced: *const u16,
        replacement: *const u16,
        backup: *const u16,
        flags: u32,
        exclude: *mut c_void,
        reserved: *mut c_void,
    ) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetNamedSecurityInfoW(
        name: *const u16,
        kind: u32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn SetNamedSecurityInfoW(
        name: *mut u16,
        kind: u32,
        information: u32,
        owner: *mut c_void,
        group: *mut c_void,
        dacl: *mut c_void,
        sacl: *mut c_void,
    ) -> u32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn GetSecurityDescriptorDacl(
        descriptor: *const c_void,
        present: *mut i32,
        dacl: *mut *mut c_void,
        defaulted: *mut i32,
    ) -> i32;
    fn GetSecurityDescriptorControl(
        descriptor: *const c_void,
        control: *mut u16,
        revision: *mut u32,
    ) -> i32;
    fn GetAce(acl: *const c_void, index: u32, ace: *mut *mut c_void) -> i32;
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn shell_path(path: &Path) -> fish_widestring::WString {
    fish::path::native_path_text(path).unwrap()
}
fn rewrite(path: &Path, bytes: &[u8]) -> io::Result<fish::wutil::FileId> {
    fish::fs::rewrite_via_temporary_file(&shell_path(path), |_old, temporary| {
        temporary.write_all(bytes)?;
        Ok(fish::fs::PotentialUpdate {
            do_save: true,
            data: (),
        })
    })
    .map(|(id, _)| id)
}

struct Event {
    name: OsString,
    handle: OwnedHandle,
}
impl Event {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = OsString::from(format!(
            "Local\\fish-n4-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(&name).as_ptr()) };
        assert!(
            !raw.is_null(),
            "CreateEventW: {}",
            io::Error::last_os_error()
        );
        Self {
            name,
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
        }
    }
    fn wait(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 30_000) },
            0,
            "required native transaction barrier {}",
            self.name.to_string_lossy()
        );
    }
    fn signal(&self) {
        assert_ne!(unsafe { SetEvent(self.handle.as_raw_handle()) }, 0);
    }
}

fn probe(fixture: &Fixture, args: &[&OsStr]) -> Command {
    let mut command = Command::new(&fixture.probe);
    command
        .args(args)
        .current_dir(&fixture.root)
        .env_clear()
        .env("PATH", "");
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command
}

struct OwnedChild {
    child: Option<Child>,
    out: Option<JoinHandle<Vec<u8>>>,
    err: Option<JoinHandle<Vec<u8>>>,
}
impl OwnedChild {
    fn spawn(command: &mut Command) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        Self {
            child: Some(child),
            out: Some(std::thread::spawn(move || {
                let mut b = Vec::new();
                stdout.read_to_end(&mut b).unwrap();
                b
            })),
            err: Some(std::thread::spawn(move || {
                let mut b = Vec::new();
                stderr.read_to_end(&mut b).unwrap();
                b
            })),
        }
    }
    fn kill(mut self) {
        let child = self.child.as_mut().unwrap();
        child.kill().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle(), 30_000) },
            0
        );
        let status = child.wait().unwrap();
        assert!(
            !status.success(),
            "interrupted writer unexpectedly acknowledged its save"
        );
        self.child.take();
        assert!(self.out.take().unwrap().join().unwrap().is_empty());
        assert!(self.err.take().unwrap().join().unwrap().is_empty());
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(out) = self.out.take() {
            let _ = out.join();
        }
        if let Some(err) = self.err.take() {
            let _ = err.join();
        }
    }
}

fn old_new_and_reader(directory: &Path) {
    let target = directory.join("same size state 雪");
    let before = rewrite(&target, b"seed=old\n").unwrap();
    let mut reader = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2 | 4)
        .open(&target)
        .unwrap();
    let after = rewrite(&target, b"seed=new\n").unwrap();
    assert_eq!(before.size, after.size);
    assert_ne!(
        before.native_identity, after.native_identity,
        "equal-size replacement must change full native file identity"
    );
    assert_eq!(fs::read(&target).unwrap(), b"seed=new\n");
    let mut retained = Vec::new();
    reader.read_to_end(&mut retained).unwrap();
    assert_eq!(
        retained, b"seed=old\n",
        "delete-sharing reader must retain old complete bytes"
    );
    assert_eq!(
        fish::wutil::file_id_for_file(&reader)
            .unwrap()
            .native_identity,
        before.native_identity
    );
    let (id, value) = fish::fs::lock_and_load(&shell_path(&target), |mut file, _id| {
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    })
    .unwrap();
    assert_eq!(id.native_identity, after.native_identity);
    assert_eq!(value, b"seed=new\n");
    let (_, update) =
        fish::fs::rewrite_via_temporary_file(&shell_path(&target), |_old, temporary| {
            temporary.write_all(b"must not commit")?;
            Ok(fish::fs::PotentialUpdate {
                do_save: false,
                data: 42,
            })
        })
        .unwrap();
    assert_eq!(update.data, 42);
    assert!(!update.do_save);
    assert_eq!(fs::read(&target).unwrap(), b"seed=new\n");
    reader.seek(SeekFrom::Start(0)).unwrap();
    retained.clear();
    reader.read_to_end(&mut retained).unwrap();
    assert_eq!(retained, b"seed=old\n");
}

fn concurrent_merge(fixture: &Fixture, directory: &Path) {
    let target = directory.join("concurrent state");
    rewrite(&target, b"seed=old\n").unwrap();
    let a = Event::new("merge-a");
    let b = Event::new("merge-b");
    let release = Event::new("merge-release");
    let mut one = probe(
        fixture,
        &[
            OsStr::new("n4-storage-append"),
            target.as_os_str(),
            OsStr::new("alpha"),
            OsStr::new("雪"),
            &a.name,
            &release.name,
        ],
    );
    let mut two = probe(
        fixture,
        &[
            OsStr::new("n4-storage-append"),
            target.as_os_str(),
            OsStr::new("beta"),
            OsStr::new("🐟"),
            &b.name,
            &release.name,
        ],
    );
    std::thread::scope(|scope| {
        let first = scope.spawn(|| run(&mut one, b""));
        let second = scope.spawn(|| run(&mut two, b""));
        a.wait();
        b.wait();
        release.signal();
        exact(first.join().unwrap(), 0, b"", b"");
        exact(second.join().unwrap(), 0, b"", b"");
    });
    assert_eq!(
        fs::read(&target).unwrap(),
        "alpha=雪\nbeta=🐟\nseed=old\n".as_bytes(),
        "exclusive lock must precede reread/merge, preserving both acknowledged keys"
    );
}

fn locked_and_changed(directory: &Path) {
    let target = directory.join("locked state");
    rewrite(&target, b"prior=valid\n").unwrap();
    let pinned = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&target)
        .unwrap();
    assert!(
        rewrite(&target, b"not=acknowledged\n").is_err(),
        "denied delete sharing falsely saved"
    );
    assert_eq!(fs::read(&target).unwrap(), b"prior=valid\n");
    drop(pinned);
    let result = fish::fs::rewrite_via_temporary_file(&shell_path(&target), |_old, temporary| {
        temporary.write_all(b"not=flushed\n")?;
        // A real read-only native handle makes FlushFileBuffers fail with access denied.
        // Exercise the production error boundary without invalid handles or a test hook.
        *temporary = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2 | 4)
            .open(&target)?;
        Ok(fish::fs::PotentialUpdate {
            do_save: true,
            data: (),
        })
    });
    assert!(
        result.is_err(),
        "failed FlushFileBuffers falsely acknowledged a save"
    );
    assert_eq!(fs::read(&target).unwrap(), b"prior=valid\n");
    let competing = directory.join("external replacement");
    fs::write(&competing, b"other=valid\n").unwrap();
    let result =
        fish::fs::rewrite_via_temporary_file(&shell_path(&target), |mut old, temporary| {
            assert_eq!(fs::read(&target).unwrap(), b"prior=valid\n");
            assert_eq!(fs::read(&competing).unwrap(), b"other=valid\n");
            assert_ne!(
                unsafe {
                    ReplaceFileW(
                        wide(target.as_os_str()).as_ptr(),
                        wide(competing.as_os_str()).as_ptr(),
                        std::ptr::null(),
                        0,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                },
                0,
                "independent ReplaceFileW must install the competing identity: {}",
                io::Error::last_os_error()
            );
            assert_eq!(fs::read(&target).unwrap(), b"other=valid\n");
            assert!(
                !competing.exists(),
                "successful replacement must consume the competing name"
            );
            let mut prior = Vec::new();
            old.read_to_end(&mut prior)?;
            assert_eq!(
                prior, b"prior=valid\n",
                "open snapshot lost the prior object's bytes"
            );
            temporary.write_all(b"must=not overwrite\n")?;
            Ok(fish::fs::PotentialUpdate {
                do_save: true,
                data: (),
            })
        });
    assert!(
        result.is_err(),
        "observed identity replacement falsely saved"
    );
    assert_eq!(fs::read(&target).unwrap(), b"other=valid\n");
    let first = directory.join("first create race");
    let result = fish::fs::rewrite_via_temporary_file(&shell_path(&first), |_old, temporary| {
        fs::write(&first, b"external=first\n")?;
        temporary.write_all(b"must=not overwrite\n")?;
        Ok(fish::fs::PotentialUpdate {
            do_save: true,
            data: (),
        })
    });
    assert!(
        result.is_err(),
        "first-create commit blindly replaced an independent file"
    );
    assert_eq!(fs::read(&first).unwrap(), b"external=first\n");
    let absent = directory.join("absent no-save");
    let (_, update) =
        fish::fs::rewrite_via_temporary_file(&shell_path(&absent), |_old, temporary| {
            temporary.write_all(b"must stay temporary")?;
            Ok(fish::fs::PotentialUpdate {
                do_save: false,
                data: (),
            })
        })
        .unwrap();
    assert!(!update.do_save);
    assert!(!absent.exists(), "no-save created an absent target");
    let snapshot = fish::fs::LockedFile::new(
        fish::fs::LockingMode::Exclusive(fish::fs::WriteMethod::RenameIntoPlace),
        &shell_path(&absent),
    )
    .unwrap();
    assert_eq!(snapshot.get().metadata().unwrap().len(), 0);
    assert!(
        !absent.exists(),
        "opening a rewrite snapshot created an absent target"
    );
    drop(snapshot);
    let lock =
        fish::fs::LockedFile::new(fish::fs::LockingMode::Shared, &shell_path(&target)).unwrap();
    assert!(
        fs::remove_file(directory.join(".fish_lock")).is_err(),
        "live stable lock was deletable"
    );
    assert!(
        fs::rename(directory.join(".fish_lock"), directory.join("moved lock")).is_err(),
        "live stable lock was replaceable"
    );
    drop(lock);
}

/// Save/restore real DACLs on fixture-owned objects. ACL setup failure is a failed mandatory case.
struct RestrictedAcl {
    path: PathBuf,
    descriptor: *mut c_void,
    dacl: *mut c_void,
    protected: bool,
}
impl RestrictedAcl {
    fn new(path: &Path, sddl: &str) -> Self {
        let mut descriptor = std::ptr::null_mut();
        let mut dacl = std::ptr::null_mut();
        let result = unsafe {
            GetNamedSecurityInfoW(
                wide(path.as_os_str()).as_ptr(),
                1,
                4,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(
            result, 0,
            "required ACL fixture GetNamedSecurityInfoW: {result}"
        );
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
            0
        );
        let saved = Self {
            path: path.to_owned(),
            descriptor,
            dacl,
            protected: control & 0x1000 != 0,
        };
        let mut restricted = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    wide(OsStr::new(sddl)).as_ptr(),
                    1,
                    &mut restricted,
                    std::ptr::null_mut(),
                )
            },
            0,
            "required ACL fixture SDDL: {}",
            io::Error::last_os_error()
        );
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                GetSecurityDescriptorDacl(restricted, &mut present, &mut acl, &mut defaulted)
            },
            0
        );
        assert_ne!(present, 0);
        let result = unsafe {
            SetNamedSecurityInfoW(
                wide(path.as_os_str()).as_mut_ptr(),
                1,
                4 | 0x8000_0000,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            LocalFree(restricted);
        }
        assert_eq!(
            result, 0,
            "required ACL fixture SetNamedSecurityInfoW: {result}"
        );
        saved
    }
}
impl Drop for RestrictedAcl {
    fn drop(&mut self) {
        let result = unsafe {
            SetNamedSecurityInfoW(
                wide(self.path.as_os_str()).as_mut_ptr(),
                1,
                4 | if self.protected {
                    0x8000_0000
                } else {
                    0x2000_0000
                },
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                self.dacl,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            LocalFree(self.descriptor);
        }
        assert_eq!(result, 0, "restore fixture-owned DACL: {result}");
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Dacl {
    present: bool,
    protected: bool,
    entries: Option<Vec<Vec<u8>>>,
}

fn dacl(path: &Path) -> Dacl {
    let mut descriptor = std::ptr::null_mut();
    let result = unsafe {
        GetNamedSecurityInfoW(
            wide(path.as_os_str()).as_ptr(),
            1,
            4,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    assert_eq!(result, 0);
    struct Descriptor(*mut c_void);
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    let descriptor = Descriptor(descriptor);
    let (mut present, mut defaulted, mut control, mut revision) = (0, 0, 0, 0);
    let mut acl = std::ptr::null_mut();
    assert_ne!(
        unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted) },
        0
    );
    assert_ne!(
        unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) },
        0
    );
    let entries = if acl.is_null() {
        None
    } else {
        // ACL's ABI header has AceCount at byte 4; compare actual ordered access
        // entries, not SDDL's incidental AUTO_INHERITED serialization flag.
        let count = unsafe { std::ptr::read_unaligned(acl.cast::<u8>().add(4).cast::<u16>()) };
        let mut entries = Vec::with_capacity(usize::from(count));
        for index in 0..u32::from(count) {
            let mut ace = std::ptr::null_mut();
            assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
            let size = unsafe { std::ptr::read_unaligned(ace.cast::<u8>().add(2).cast::<u16>()) };
            entries.push(
                unsafe { std::slice::from_raw_parts(ace.cast::<u8>(), usize::from(size)) }.to_vec(),
            );
        }
        Some(entries)
    };
    Dacl {
        present: present != 0,
        protected: control & 0x1000 != 0,
        entries,
    }
}

fn denied_acl(directory: &Path) {
    let directory = directory.join("restricted ACL");
    fs::create_dir(&directory).unwrap();
    let target = directory.join("state");
    rewrite(&target, b"prior=valid\n").unwrap();
    let parent_acl = RestrictedAcl::new(&directory, "D:P(D;;DC;;;WD)(A;OICI;FA;;;WD)");
    let file_acl = RestrictedAcl::new(&target, "D:P(D;;SD;;;WD)(A;;FR;;;WD)(A;;WD;;;OW)");
    assert!(
        rewrite(&target, b"must=not acknowledge\n").is_err(),
        "restrictive DACL falsely saved"
    );
    assert_eq!(fs::read(&target).unwrap(), b"prior=valid\n");
    drop(file_acl);
    drop(parent_acl);
    rewrite(&target, b"restored=valid\n").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"restored=valid\n");
    let acl = RestrictedAcl::new(&target, "D:P(A;;FA;;;OW)(A;;FR;;;WD)");
    let before = dacl(&target);
    rewrite(&target, b"preserved=ACL\n").unwrap();
    assert_eq!(
        dacl(&target),
        before,
        "replacement lost the existing protected DACL"
    );
    drop(acl);
}

fn staging_paths(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fish_tmp_")
        })
        .collect()
}

fn protected_staging(fixture: &Fixture, directory: &Path) {
    let directory = directory.join("protected staging ACL");
    fs::create_dir(&directory).unwrap();
    // Everyone can read inherited files in this parent; the existing target permits only its
    // owner. Query real object descriptors, including at the empty staging-file boundary.
    let parent_acl = RestrictedAcl::new(&directory, "D:P(A;OICI;FA;;;OW)(A;OICI;FR;;;WD)");
    let target = directory.join("state");
    rewrite(&target, b"prior=valid\n").unwrap();
    let inherited = directory.join("parent inheritance control");
    fs::write(&inherited, b"public control\n").unwrap();
    assert_eq!(
        dacl(&target),
        dacl(&inherited),
        "first-created state must retain ordinary parent inheritance"
    );
    let target_acl = RestrictedAcl::new(&target, "D:P(A;;FA;;;OW)");
    let protected = dacl(&target);
    assert_ne!(
        protected.entries,
        dacl(&inherited).entries,
        "parent control must have broader effective protection"
    );
    fish::fs::rewrite_via_temporary_file(&shell_path(&target), |_old, temporary| {
        let temporary_path =
            fish::path::native_absolute_path(&fish::wutil::native_final_path(temporary)?)?;
        assert_eq!(temporary.metadata()?.len(), 0);
        assert_eq!(
            dacl(&temporary_path),
            protected,
            "empty staging file already inherited the broader parent DACL"
        );
        temporary.write_all(b"secret=")?;
        assert_eq!(
            dacl(&temporary_path),
            protected,
            "partial staged contents were exposed"
        );
        temporary.write_all(b"complete\n")?;
        assert_eq!(
            dacl(&temporary_path),
            protected,
            "complete staged contents were exposed"
        );
        assert_eq!(
            fs::read(&target)?,
            b"prior=valid\n",
            "staging changed the target before commit"
        );
        Ok(fish::fs::PotentialUpdate {
            do_save: true,
            data: (),
        })
    })
    .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"secret=complete\n");
    assert_eq!(dacl(&target), protected);
    assert!(
        staging_paths(&directory).is_empty(),
        "successful commit left staging debris"
    );

    for stage in ["partial", "complete"] {
        rewrite(&target, b"prior=valid\n").unwrap();
        let ready = Event::new(stage);
        let release = Event::new("protected never released");
        let writer = OwnedChild::spawn(&mut probe(
            fixture,
            &[
                OsStr::new("n4-storage-rewrite"),
                target.as_os_str(),
                OsStr::new("new"),
                OsStr::new("not acknowledged"),
                &ready.name,
                &release.name,
                OsStr::new(stage),
            ],
        ));
        ready.wait();
        let paths = staging_paths(&directory);
        assert_eq!(
            paths.len(),
            1,
            "existing-target rewrite must have one owned staging object"
        );
        let staged = &paths[0];
        let complete = b"new=not acknowledged\nprior=valid\n";
        let expected = if stage == "partial" {
            &complete[..complete.len() / 2]
        } else {
            &complete[..]
        };
        assert_eq!(
            dacl(staged),
            protected,
            "live staged bytes inherited broader parent access"
        );
        assert_eq!(
            fs::read(staged).unwrap(),
            expected,
            "writer did not reach its required stage"
        );
        assert_eq!(fs::read(&target).unwrap(), b"prior=valid\n");
        writer.kill();
        assert_eq!(
            fs::read(&target).unwrap(),
            b"prior=valid\n",
            "interruption changed prior state"
        );
        assert_eq!(
            dacl(staged),
            protected,
            "orphaned staged bytes lost target protection"
        );
        assert_eq!(
            fs::read(staged).unwrap(),
            expected,
            "orphan did not retain its staged contents"
        );
        rewrite(&target, b"recovered=valid\n").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"recovered=valid\n");
        assert_eq!(dacl(&target), protected);
        assert_eq!(
            dacl(staged),
            protected,
            "subsequent save exposed an earlier orphan"
        );
        fs::remove_file(staged).expect("fixture owner must clean up its protected orphan");
    }
    drop(target_acl);
    drop(parent_acl);
}

fn interrupted(fixture: &Fixture, directory: &Path) {
    let target = directory.join("interrupted state");
    for stage in ["partial", "complete"] {
        rewrite(&target, b"prior=valid\n").unwrap();
        let ready = Event::new(stage);
        let release = Event::new("never released");
        let writer = OwnedChild::spawn(&mut probe(
            fixture,
            &[
                OsStr::new("n4-storage-rewrite"),
                target.as_os_str(),
                OsStr::new("new"),
                OsStr::new("not acknowledged"),
                &ready.name,
                &release.name,
                OsStr::new(stage),
            ],
        ));
        ready.wait();
        assert_eq!(
            fs::read(&target).unwrap(),
            b"prior=valid\n",
            "temp stage changed target before commit"
        );
        writer.kill();
        assert_eq!(
            fs::read(&target).unwrap(),
            b"prior=valid\n",
            "killed writer lost prior valid state"
        );
        rewrite(&target, b"recovered=valid\n").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"recovered=valid\n");
    }
}

pub(super) fn exercise_sharing(fixture: &Fixture) {
    let directory = fixture.root.join("storage sharing 雪");
    fs::create_dir(&directory).unwrap();
    old_new_and_reader(&directory);
    concurrent_merge(fixture, &directory);
}

pub(super) fn exercise_recovery(fixture: &Fixture) {
    let directory = fixture.root.join("storage recovery 雪");
    fs::create_dir(&directory).unwrap();
    locked_and_changed(&directory);
    denied_acl(&directory);
    interrupted(fixture, &directory);
    protected_staging(fixture, &directory);
    let unc = std::env::var_os("FISH_NATIVE_N1_UNC_ROOT")
        .expect("mandatory FISH_NATIVE_N1_UNC_ROOT writable UNC fixture is missing");
    let text = unc.to_string_lossy();
    assert!(
        text.starts_with("\\\\")
            && (!text.starts_with("\\\\?\\")
                || text.to_ascii_uppercase().starts_with("\\\\?\\UNC\\")),
        "mandatory remote fixture must be a real UNC share, not a local path: {text}"
    );
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let remote = PathBuf::from(unc).join(format!(
        "fish-n4-state 雪 {} {}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&remote).expect("mandatory UNC fixture must be writable");
    struct Remote(PathBuf);
    impl Drop for Remote {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let remote = Remote(remote);
    old_new_and_reader(&remote.0);
    concurrent_merge(fixture, &remote.0);
    locked_and_changed(&remote.0);
    interrupted(fixture, &remote.0);
}
