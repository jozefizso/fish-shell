//! Actual APIs plus native-fish consumers. Parent owns the gate registry and probe router.
use super::{Family, Fixture};
use fish::native_services::{accounts, network, processes};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

pub const FAMILIES: &[Family] = &[
    ("services-process-lifecycle", process_lifecycle),
    ("services-token-account-profile", token_account_profile),
    ("services-local-accounts-groups", local_accounts_groups),
    ("services-adapters-hosts", adapters_hosts),
    ("services-hosts-denied", hosts_denied),
    ("services-completion-consumers", completion_consumers),
    ("services-time-accounting-consumers", time_accounting_consumers),
    ("services-profileless-account", profileless_account),
    ("services-fish-owned-child", fish_owned_child),
    ("services-native-helper-byte-streams-and-volume-clock", native_helper_streams),
];

fn native_helper_streams(f: &Fixture) {
    let data = b"literal bytes\0\r\n\xff\x80\n\n";
    let path = f.root.join("literal helper data.bin");
    fs::write(&path, data).unwrap();
    super::fixture::exact(f.eval(&format!("__fish_native_fs read {}", quote(path.to_str().unwrap()))),
        0, data, b"");
    let code = "set -l owned (__fish_native_fs make-temp-file); or exit 91; \
        test -f $owned; or exit 92; printf 'private-data' >$owned; or exit 93; \
        __fish_native_fs read $owned; or exit 94; __fish_native_fs remove $owned; or exit 95; \
        test -e $owned; and exit 96; exit 0";
    let mut command = f.command(&[std::ffi::OsStr::new("-N"), std::ffi::OsStr::new("-c"), std::ffi::OsStr::new(code)]);
    command.env("TEMP", &f.root).env("TMP", &f.root);
    super::fixture::exact(super::fixture::run(&mut command, b""), 0, b"private-data", b"");
    let drives = fish::native_services::desktop::drives().expect("native logical drive inventory");
    assert!(!drives.is_empty(), "native OS drive is a required fixture");
    let expected = drives.into_iter().map(|(root, kind)| format!("{root}\t{kind}\n")).collect::<String>();
    assert_eq!(output(f, "__fish_native_services drives"), expected);
    let now = output(f, "__fish_native_services timestamp now '%H:%M:%S'");
    let fields: Vec<_> = now.trim().split(':').collect();
    assert_eq!(fields.len(), 3, "native clock formatter: {now:?}");
    assert!(fields.iter().all(|field| field.len() == 2 && field.bytes().all(|byte| byte.is_ascii_digit())));
    rejected(f, "__fish_native_services open");
    assert!(fish::native_services::desktop::open("invalid\0target", &f.root).is_err());
    super::resource_helpers::exercise(f);
}

fn quote(text: &str) -> String { format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'")) }
fn output(f: &Fixture, code: &str) -> String {
    let result = f.eval(code);
    assert_eq!(result.code, 0, "{code}: {}", String::from_utf8_lossy(&result.err));
    assert!(result.err.is_empty(), "{code}: {}", String::from_utf8_lossy(&result.err));
    String::from_utf8(result.out).unwrap()
}
fn rejected(f: &Fixture, code: &str) {
    let result = f.eval(code);
    assert_ne!(result.code, 0, "{code}");
    assert!(result.out.is_empty(), "failure published misleading records: {code}");
    assert!(!result.err.is_empty(), "missing diagnostic: {code}");
}
#[link(name = "kernel32")]
unsafe extern "system" { fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32; }
struct OwnedChild(Child);
impl Drop for OwnedChild { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }

#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(text: *const u16, revision: u32, descriptor: *mut *mut std::ffi::c_void, size: *mut u32) -> i32;
    fn SetFileSecurityW(path: *const u16, information: u32, descriptor: *const std::ffi::c_void) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" { fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void; }

struct DeniedRead { path: Vec<u16>, restore: *mut std::ffi::c_void }
impl DeniedRead {
    fn new(path: &std::path::Path) -> Self {
        use std::os::windows::ffi::OsStrExt;
        fn descriptor(sddl: &str) -> *mut std::ffi::c_void {
            let text: Vec<_> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut descriptor = std::ptr::null_mut();
            assert_ne!(unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(text.as_ptr(), 1, &mut descriptor, std::ptr::null_mut()) }, 0, "{}", std::io::Error::last_os_error());
            descriptor
        }
        let sid = accounts::current_account().unwrap().sid;
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let restore = descriptor(&format!("D:P(A;;FA;;;{sid})"));
        let result = Self { path, restore };
        let deny = descriptor(&format!("D:P(D;;FR;;;{sid})(A;;FA;;;{sid})"));
        let status = unsafe { SetFileSecurityW(result.path.as_ptr(), 0x80000004, deny) };
        let error = std::io::Error::last_os_error();
        unsafe { LocalFree(deny); }
        assert_ne!(status, 0, "{error}");
        result
    }
}
impl Drop for DeniedRead {
    fn drop(&mut self) {
        let status = unsafe { SetFileSecurityW(self.path.as_ptr(), 0x80000004, self.restore) };
        let error = std::io::Error::last_os_error();
        unsafe { LocalFree(self.restore); }
        if !std::thread::panicking() { assert_ne!(status, 0, "restore owned fixture DACL: {error}"); }
    }
}
fn process_lifecycle(f: &Fixture) {
    let mut child = OwnedChild(Command::new(&f.probe).arg("n8-services-hold").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap());
    let stdout = child.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = send.send(result);
    });
    let pid = receive.recv_timeout(Duration::from_secs(30)).expect("child ready barrier").unwrap().trim().parse::<u32>().unwrap();
    assert_eq!(pid, child.0.id());
    let observed = processes::snapshot().unwrap();
    let current = observed.iter().find(|entry| entry.pid == std::process::id()).expect("current process visible");
    assert!(!current.full_image.as_ref().unwrap().is_empty());
    let entry = observed.iter().find(|entry| entry.pid == pid).expect("owned child visible");
    assert_eq!(entry.parent_pid, std::process::id());
    assert_eq!(std::path::Path::new(entry.full_image.as_ref().unwrap()), f.probe);
    let listed = output(f, "builtin __fish_native_services processes");
    assert!(listed.lines().any(|line| line.starts_with(&format!("{pid}\t"))));
    let completed = output(f, "__fish_complete_pids");
    assert!(completed.lines().any(|line| line.starts_with(&format!("{pid}\t"))));
    let image = output(f, &format!("builtin __fish_native_services process-image {pid}"));
    assert_eq!(image.trim_end(), fish::path::native_path_text(&f.probe).unwrap().to_string());
    let expected_name = f.probe.file_name().unwrap().to_str().unwrap();
    assert_eq!(output(f, &format!(
        "complete -c n8-native-proc -f -a '(__fish_complete_proc)'; \
         for candidate in (complete -C 'n8-native-proc '); \
             set -l value (string split -f1 \\t -- \"$candidate\" | string unescape); \
             if test \"$value\" = {}; printf '%s\\n' \"$value\"; exit 0; end; \
         end; exit 1", quote(expected_name)
    )), format!("{expected_name}\n"));
    child.0.stdin.take().unwrap().write_all(b"!").unwrap();
    assert_eq!(unsafe { WaitForSingleObject(child.0.as_raw_handle(), 30000) }, 0, "owned child bounded exit");
    assert!(child.0.wait().unwrap().success());
    assert!(!processes::snapshot().unwrap().iter().any(|entry| entry.pid == pid));
    assert!(!output(f, "builtin __fish_native_services processes").lines().any(|line| line.starts_with(&format!("{pid}\t"))));
    rejected(f, "builtin __fish_native_services process-image 0");
    rejected(f, "__fish_ps -o pid,comm,tty");
}

fn token_account_profile(f: &Fixture) {
    let token = accounts::current_account().unwrap();
    let qualified = format!("{}\\{}", token.domain, token.name);
    let looked_up = accounts::lookup(&qualified).unwrap();
    assert_eq!(looked_up.sid, token.sid);
    assert_eq!(looked_up.kind, 1);
    let profile = accounts::current_profile().unwrap();
    let user = fish_widestring::native_to_shell_text_owned(fish_widestring::str2wcstring(&qualified));
    assert_eq!(accounts::profile_for_user(&user).unwrap(), profile);
    let (_, known_profile) = fish::path::native_known_folder_roots().unwrap();
    assert_eq!(fish::wutil::native_file_identity(&profile, true).unwrap(), fish::wutil::native_file_identity(&fish::path::native_path_text(&known_profile).unwrap(), true).unwrap());
    assert_eq!(output(f, &format!("builtin __fish_native_services account {}", quote(&qualified))), format!("{qualified}\t{}\t1\n", token.sid));
    assert_eq!(output(f, &format!("builtin __fish_native_services profile {}", quote(&qualified))), format!("{profile}\n"));
    assert_eq!(output(f, "builtin __fish_native_services profile"), format!("{profile}\n"));
    // HOME is intentionally overridden by Fixture, while ~USER/profile retain actual identity.
    assert_eq!(output(f, "printf '%s\\n' ~"), format!("{}\n", fish::path::native_path_text(&f.state.join("home")).unwrap()));
    assert_eq!(output(f, &format!("printf '%s\\n' ~{}", quote(&qualified))), format!("{profile}\n"));
    // '/' delimits the path tail; '\\' inside the qualified account retains its identity.
    assert_eq!(output(f, &format!("printf '%s\\n' ~{}/literal", quote(&qualified))), format!("{profile}/literal\n"));
    assert_eq!(output(f, &format!("printf '%s\\n' {}", quote(&format!("{profile}/literal")))), format!("{profile}/literal\n"));
    let completed = output(f, &format!("complete -C {}", quote(&format!("echo ~{}", quote(&qualified)))));
    assert!(completed.lines().any(|line| line.split('\t').next() == Some(format!("~{qualified}").as_str())),
        "qualified native home completion: {completed:?}");
    rejected(f, "printf '%s\\n' ~'fish-N8-unknown-account-847900129'");
    let groups = accounts::token_groups().unwrap();
    assert!(groups.iter().any(|(sid, _)| sid == "S-1-1-0"), "token contains Everyone SID");
    let observed = output(f, "builtin __fish_native_services token-groups");
    for (sid, attributes) in groups { assert!(observed.lines().any(|line| line == format!("{sid}\t{attributes}"))); }
    rejected(f, "builtin __fish_native_services profile '.\\fish-N8-unknown-account-847900129'");
    rejected(f, "builtin __fish_native_services account '.\\fish-N8-unknown-account-847900129'");
}

// Obtain the expected user SID from SAM's SID-bearing record, independently of
// LookupAccountNameW and account-name qualification.
fn local_user_sid(name: &str) -> String {
    use std::ffi::c_void;
    #[repr(C)]
    struct User23 {
        name: *const u16, full_name: *const u16, comment: *const u16,
        flags: u32, sid: *const c_void,
    }
    #[link(name = "netapi32")]
    unsafe extern "system" {
        fn NetUserGetInfo(server: *const u16, name: *const u16, level: u32, buffer: *mut *mut u8) -> u32;
        fn NetApiBufferFree(buffer: *mut c_void) -> u32;
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertSidToStringSidW(sid: *const c_void, value: *mut *mut u16) -> i32;
    }
    struct NetBuffer(*mut u8);
    impl Drop for NetBuffer {
        fn drop(&mut self) { unsafe { NetApiBufferFree(self.0.cast()); } }
    }
    let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    let mut buffer = std::ptr::null_mut();
    let status = unsafe { NetUserGetInfo(std::ptr::null(), wide.as_ptr(), 23, &mut buffer) };
    let buffer = NetBuffer(buffer);
    assert_eq!(status, 0, "real local SAM user {name:?}: NetUserGetInfo={status}");
    let sid = unsafe { (*buffer.0.cast::<User23>()).sid };
    let mut text = std::ptr::null_mut();
    assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut text) }, 0, "{}", std::io::Error::last_os_error());
    let mut length = 0;
    while unsafe { *text.add(length) } != 0 { length += 1; }
    let result = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe { LocalFree(text.cast()); }
    result.unwrap()
}

fn local_accounts_groups(f: &Fixture) {
    let users = accounts::local_users().unwrap();
    let groups = accounts::local_groups().unwrap();
    fs::write(f.root.join("local-account-inventory.txt"), format!("users={users:#?}\ngroups={groups:#?}\n")).unwrap();
    assert!(!users.is_empty(), "required real local SAM users");
    assert!(!groups.is_empty(), "required real local SAM groups");
    let printed_users = output(f, "__fish_print_users");
    let printed_windows_users = output(f, "__fish_print_windows_users");
    let printed_groups = output(f, "__fish_print_groups");
    let mut records = fs::File::create(f.root.join("local-account-identities.txt")).unwrap();
    for user in users {
        let expected_sid = local_user_sid(&user.name);
        writeln!(records, "user {:?}\texpected SID={expected_sid}", user.name).unwrap();
        let identity = accounts::lookup(&format!(".\\{}", user.name))
            .unwrap_or_else(|error| panic!("local user {:?}, expected SID={expected_sid}: {error}", user.name));
        writeln!(records, "resolved {identity:?}").unwrap();
        assert_eq!(identity.kind, 1, "{identity:?}");
        assert_eq!(identity.sid, expected_sid, "{identity:?}");
        let qualified = format!("{}\\{}", identity.domain, identity.name);
        assert_eq!(accounts::lookup(&qualified).unwrap().sid, expected_sid);
        assert_eq!(output(f, &format!("builtin __fish_native_services account {}", quote(&qualified))), format!("{qualified}\t{expected_sid}\t1\n"));
        assert!(printed_users.lines().any(|name| name == user.name));
        assert!(printed_windows_users.lines().any(|name| name == user.name));
    }
    for group in groups {
        // NetLocalGroupEnum includes builtin aliases. Their actual localized domain/name
        // comes from SID lookup, not the machine-only .\\ prefix or English spellings.
        writeln!(records, "group {:?}", group.name).unwrap();
        let identity = accounts::lookup(&group.name)
            .unwrap_or_else(|error| panic!("enumerated local group {:?}: {error}", group.name));
        writeln!(records, "resolved {identity:?}").unwrap();
        assert!(identity.kind == 2 || identity.kind == 4 || identity.kind == 5, "{identity:?}");
        let qualified = format!("{}\\{}", identity.domain, identity.name);
        assert_eq!(accounts::lookup(&qualified).unwrap().sid, identity.sid, "{identity:?}");
        assert_eq!(output(f, &format!("builtin __fish_native_services account {}", quote(&qualified))), format!("{qualified}\t{}\t{}\n", identity.sid, identity.kind));
        assert!(printed_groups.lines().any(|name| name == group.name));
    }
    for operation in ["uid", "gid", "domain-users", "domain-groups"] { rejected(f, &format!("builtin __fish_native_services {operation}")); }
    rejected(f, "__fish_complete_user_ids");
    rejected(f, "__fish_complete_group_ids");
}

fn adapters_hosts(f: &Fixture) {
    let adapters = network::interfaces().unwrap();
    let loopback = adapters.iter().find(|interface| interface.loopback).expect("required native loopback interface");
    assert!(loopback.addresses.iter().any(|address| address == "127.0.0.1" || address == "::1"));
    let names = output(f, "__fish_print_interfaces");
    let addresses = output(f, "builtin __fish_native_services addresses");
    for adapter in adapters {
        assert!(names.lines().any(|name| name == adapter.name));
        for address in adapter.addresses { assert!(addresses.lines().any(|line| line == format!("{address}\t{}\t{}", adapter.name, adapter.index))); }
    }
    let hosts = f.root.join("hosts fixture 雪 ' quoted.txt");
    fs::write(&hosts, "# fixture only\n127.0.0.1 localhost loopback-alias\n192.0.2.8 service.example 雪.example # ignored.example\n2001:db8::8 ipv6.example service.example\n").unwrap();
    let expected = "ipv6.example\nlocalhost\nloopback-alias\nservice.example\n雪.example\n";
    assert_eq!(network::hosts(&hosts).unwrap().join("\n") + "\n", expected);
    assert_eq!(output(f, &format!("builtin __fish_native_services hosts {}", quote(hosts.to_str().unwrap()))), expected);
    let utf16 = f.root.join("hosts UTF16.txt");
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend("192.0.2.1 unicode-雪.example\n".encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(&utf16, bytes).unwrap();
    assert_eq!(network::hosts(&utf16).unwrap(), ["unicode-雪.example"]);
    fs::write(&hosts, b"invalid-address false.example\n").unwrap();
    rejected(f, &format!("builtin __fish_native_services hosts {}", quote(hosts.to_str().unwrap())));
    fs::write(&hosts, [0xff, 0xfe, 0]).unwrap();
    assert!(network::hosts(&hosts).is_err());
    assert!(network::hosts_path().unwrap().is_absolute(), "configured native hosts identity");
}

fn hosts_denied(f: &Fixture) {
    use std::os::windows::fs::OpenOptionsExt;
    let path = f.root.join("hosts sharing denied.txt");
    fs::write(&path, b"192.0.2.1 denied.example\n").unwrap();
    let held = fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&path).unwrap();
    assert_eq!(network::hosts(&path).unwrap_err().raw_os_error(), Some(32));
    rejected(f, &format!("builtin __fish_native_services hosts {}", quote(path.to_str().unwrap())));
    drop(held);
    assert_eq!(network::hosts(&path).unwrap(), ["denied.example"]);
    let denied = DeniedRead::new(&path);
    assert_eq!(network::hosts(&path).unwrap_err().raw_os_error(), Some(5), "actual access denial, distinct from sharing violation");
    rejected(f, &format!("builtin __fish_native_services hosts {}", quote(path.to_str().unwrap())));
    drop(denied);
    assert_eq!(network::hosts(&path).unwrap(), ["denied.example"]);
}

fn completion_consumers(f: &Fixture) {
    // No executable PATH: these public helper consumers cannot accidentally use Unix tools.
    let account = accounts::current_account().unwrap();
    let printed = output(f, "builtin __fish_native_services username");
    assert_eq!(printed.trim_end(), account.name);
    assert_eq!(output(f, "builtin __fish_native_services hostname").trim_end(), fish::identity_windows::hostname().unwrap().to_string());
    let ssh = f.state.join("home/.ssh");
    fs::create_dir_all(&ssh).unwrap();
    fs::write(ssh.join("known_hosts"), "fixture-ssh.example ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA==\n").unwrap();
    let known = f.eval("__fish_print_hostnames");
    assert!(String::from_utf8(known.out).unwrap().lines().any(|line| line == "fixture-ssh.example"), "native branch retains SSH static sources even if hosts capability fails");
    let completed = f.eval("complete -c n8-native-static -f -a static-choice; complete -c n8-native-static -a '(__fish_complete_user_ids)'; complete -C 'n8-native-static '");
    assert_eq!(completed.code, 0);
    assert!(String::from_utf8(completed.out).unwrap().lines().any(|line| line.starts_with("static-choice")), "unavailable numeric Unix identity cannot remove static completions");
}

fn time_accounting_consumers(f: &Fixture) {
    let result = f.eval(&format!("time {} n8-services-cpu", quote(f.probe.to_str().unwrap())));
    assert_eq!(result.code, 0);
    assert!(std::str::from_utf8(&result.out).unwrap().trim().parse::<u128>().unwrap() >= 40_000_000);
    let timing = std::str::from_utf8(&result.err).unwrap();
    assert!(timing.contains("Executed in") && timing.contains("external") && timing.contains("External scope:"));
    let external_cpu: f64 = timing.lines().filter(|line| line.contains("usr time") || line.contains("sys time")).map(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields[fields.len() - 2].parse::<f64>().unwrap()
    }).sum();
    assert!(external_cpu > 0.0, "time decorator must account for actual native child CPU");
    let counters = output(f, "builtin __fish_native_services accounting");
    let values: BTreeMap<_, _> = counters.lines().map(|line| { let (key, value) = line.split_once('\t').unwrap(); (key, value.parse::<u128>().unwrap()) }).collect();
    assert!(values["working-set-bytes"] > 0);
    assert!(values["peak-working-set-bytes"] >= values["working-set-bytes"]);
    assert!(values["elapsed-ns"] > 0);
    assert_eq!(output(f, "set -gx TZ UTC; builtin __fish_native_services timestamp 0 '%Y-%m-%d %H:%M:%S %z'"), "1970-01-01 00:00:00 +0000\n");
    rejected(f, "builtin __fish_native_services timestamp now +%s");
    rejected(f, "builtin __fish_native_services accounting --posix");
}

// This required family mutates local SAM only on an explicitly attested disposable
// machine and dedicated acceptance account. Missing fixture/privilege fails the gate.
fn profileless_account(f: &Fixture) {
    assert_eq!(
        std::env::var("FISH_NATIVE_N8_DEDICATED_ACCOUNT").as_deref(),
        Ok("1"),
        "BLOCKED required profileless-account fixture: provision an elevated disposable dedicated acceptance account; set FISH_NATIVE_N8_DEDICATED_ACCOUNT=1 only there; never mutate SAM on the current non-disposable host"
    );
    assert_eq!(
        std::env::var("FISH_NATIVE_N8_DISPOSABLE_MACHINE").as_deref(),
        Ok("1"),
        "BLOCKED required profileless-account fixture: local SAM mutation needs a disposable clean machine explicitly attested by FISH_NATIVE_N8_DISPOSABLE_MACHINE=1; do not elevate or modify an existing user machine/VM"
    );
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU32, Ordering};
    #[repr(C)]
    struct User1 {
        name: *const u16, password: *const u16, password_age: u32, privilege: u32,
        home: *const u16, comment: *const u16, flags: u32, script: *const u16,
    }
    #[link(name = "netapi32")]
    unsafe extern "system" {
        fn NetUserAdd(server: *const u16, level: u32, user: *const u8, parameter_error: *mut u32) -> u32;
        fn NetUserDel(server: *const u16, name: *const u16) -> u32;
    }
    #[link(name = "bcrypt")]
    unsafe extern "system" { fn BCryptGenRandom(algorithm: *mut c_void, buffer: *mut u8, size: u32, flags: u32) -> i32; }
    struct OwnedAccount(Vec<u16>);
    impl Drop for OwnedAccount {
        fn drop(&mut self) {
            let status = unsafe { NetUserDel(std::ptr::null(), self.0.as_ptr()) };
            if !std::thread::panicking() { assert_eq!(status, 0, "delete only owned disabled account"); }
        }
    }
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name = format!("fishN8{:x}{:x}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    assert!(name.len() <= 20);
    let name_wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    let mut random = [0u8; 24];
    assert!(unsafe { BCryptGenRandom(std::ptr::null_mut(), random.as_mut_ptr(), random.len() as u32, 2) } >= 0);
    let password = format!("fN8!{}aA1", random.iter().map(|byte| format!("{byte:02x}")).collect::<String>());
    let password: Vec<_> = password.encode_utf16().chain(Some(0)).collect();
    let user = User1 { name: name_wide.as_ptr(), password: password.as_ptr(), password_age: 0, privilege: 1, home: std::ptr::null(), comment: std::ptr::null(), flags: 0x203, script: std::ptr::null() };
    let mut parameter = 0;
    let status = unsafe { NetUserAdd(std::ptr::null(), 1, std::ptr::from_ref(&user).cast(), &mut parameter) };
    assert_eq!(status, 0, "required elevated local SAM fixture: NetUserAdd status={status}, parameter={parameter}");
    let owner = OwnedAccount(name_wide);
    let qualified = format!(".\\{name}");
    let account = accounts::lookup(&qualified).unwrap();
    assert_eq!(account.kind, 1);
    assert!(accounts::local_users().unwrap().iter().any(|user| user.name == name));
    let user_text = fish_widestring::native_to_shell_text_owned(fish_widestring::str2wcstring(&qualified));
    assert_eq!(accounts::profile_for_user(&user_text).unwrap_err().raw_os_error(), Some(2), "known account with absent registered profile must not guess a home directory");
    rejected(f, &format!("builtin __fish_native_services profile {}", quote(&qualified)));
    rejected(f, &format!("printf '%s\\n' ~{}", quote(&qualified)));
    drop(owner);
    assert!(accounts::lookup(&qualified).is_err(), "owned account deleted after observation");
}

fn fish_owned_child(f: &Fixture) {
    use std::io::Read;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use std::sync::atomic::{AtomicU32, Ordering};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(attributes: *const std::ffi::c_void, manual: i32, initial: i32, name: *const u16) -> *mut std::ffi::c_void;
        fn SetEvent(event: *mut std::ffi::c_void) -> i32;
    }
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let event = |suffix: &str| {
        let name = format!("Local\\fish-N8-services-{}-{serial}-{suffix}", std::process::id());
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(!handle.is_null(), "{}", std::io::Error::last_os_error());
        (name, unsafe { OwnedHandle::from_raw_handle(handle) })
    };
    let (ready_name, ready) = event("ready");
    let (release_name, release) = event("release");
    let script = format!(
        "{} n8-services-event {} {} &; set -l child $last_pid; \
         printf 'CHILD=%s\\n' $child; \
         builtin __fish_native_services processes | string match -r '^'$child'\\t'; \
         echo BEFORE; wait $child; \
         builtin __fish_native_services processes | string match -r '^'$child'\\t'; \
         echo AFTER",
        quote(f.probe.to_str().unwrap()), quote(&ready_name), quote(&release_name)
    );
    let mut child = OwnedChild(f.command(&[std::ffi::OsStr::new("-N"), std::ffi::OsStr::new("-c"), std::ffi::OsStr::new(&script)]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap());
    let stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if send.send(line).is_err() { break; }
        }
    });
    let (error_send, error_receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        let _ = error_send.send(bytes);
    });
    assert_eq!(unsafe { WaitForSingleObject(ready.as_raw_handle(), 30000) }, 0, "fish-owned child ready barrier");
    let line = || receive.recv_timeout(Duration::from_secs(30)).expect("bounded fish output barrier").unwrap();
    let declared = line();
    let pid: u32 = declared.strip_prefix("CHILD=").unwrap().parse().unwrap();
    let record = line();
    assert!(record.starts_with(&format!("{pid}\t")), "fish-owned child must appear before release: {record}");
    assert_eq!(line(), "BEFORE");
    assert_ne!(unsafe { SetEvent(release.as_raw_handle()) }, 0);
    assert_eq!(unsafe { WaitForSingleObject(child.0.as_raw_handle(), 30000) }, 0, "bounded fish wait");
    assert!(child.0.wait().unwrap().success());
    assert_eq!(line(), "AFTER", "exited owned child must not remain in the second snapshot");
    assert!(matches!(receive.recv_timeout(Duration::from_secs(30)), Err(mpsc::RecvTimeoutError::Disconnected)), "stdout reached EOF with no stale records");
    assert!(error_receive.recv_timeout(Duration::from_secs(30)).unwrap().is_empty());
}
