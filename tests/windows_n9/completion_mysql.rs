use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{collections::BTreeSet, ffi::{OsStr, c_void}, fs, net::TcpListener, os::windows::io::AsRawHandle, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::atomic::{AtomicU64, Ordering}};

pub(super) const FAMILIES: &[Family] = &[
    ("native-mysql-schema-context-and-real-query-consumers", mysql),
];
#[link(name = "kernel32")]
unsafe extern "system" { fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32; }
struct Storage(PathBuf);
impl Drop for Storage {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).expect("remove only this fixture's created MySQL storage"); }
}
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        if self.0.try_wait().expect("observe owned MySQL process").is_none() { self.0.kill().expect("terminate only owned MySQL process"); }
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), 20_000) }, 0, "owned MySQL process must finish cleanup");
        self.0.wait().expect("collect owned MySQL process result");
    }
}
fn external(f: &Fixture, program: &Path, storage: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(&f.root).env_clear();
    for (name, value) in f.command(&[]).get_envs() { if let Some(value) = value { command.env(name, value); } }
    tool_environment(&mut command, program);
    command.env("TEMP", storage.join("temp")).env("TMP", storage.join("temp"))
        .env("MYSQL_HOME", storage).env("MYSQL_TEST_LOGIN_FILE", storage.join("absent-login.cnf"));
    command
}
fn client(f: &Fixture, program: &Path, storage: &Path, port: u16, sql: &[u8]) -> Captured {
    run(external(f, program, storage).args(["--no-defaults", "--no-login-paths", "--protocol=TCP", "--host=127.0.0.1", &format!("--port={port}"), "--user=root", "--connect-timeout=2", "--default-character-set=utf8mb4", "--batch", "--raw", "--skip-column-names"]), sql)
}
fn shell(f: &Fixture, program: &Path, storage: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, program);
    command.env("MYSQL_HOME", storage).env("MYSQL_TEST_LOGIN_FILE", storage.join("absent-login.cnf"));
    run(&mut command, b"")
}
fn records(bytes: Vec<u8>) -> BTreeSet<String> {
    String::from_utf8(bytes).unwrap().lines().map(|line| line.split('\t').next().unwrap().to_owned()).collect()
}
fn mysql(f: &Fixture) {
    let program = PathBuf::from(std::env::var_os("FISH_N9_MYSQL").expect("required genuine native MySQL client/server fixture"));
    assert!(program.is_absolute() && program.is_file(), "real absolute MySQL client: {}", program.display());
    let bin = program.parent().unwrap();
    let daemon = bin.join("mysqld.exe");
    let admin = bin.join("mysqladmin.exe");
    assert!(daemon.is_file() && admin.is_file(), "matching genuine MySQL server/admin binaries required");
    let parent = PathBuf::from(std::env::var_os("FISH_N9_MYSQL_WORK_ROOT").expect("declared owned ASCII MySQL storage root required; MySQL26.7 server initialization rejects Unicode filesystem paths even under direct wide launch"));
    assert!(parent.is_absolute() && parent.as_os_str().as_encoded_bytes().is_ascii(), "MySQL server storage capability requires an absolute ASCII root");
    fs::create_dir_all(&parent).unwrap();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let storage = Storage(parent.join(format!("fish-mysql-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    fs::create_dir(&storage.0).expect("never reuse or delete an existing unknown MySQL fixture root");
    fs::create_dir(storage.0.join("temp")).unwrap();
    let data = storage.0.join("data");
    let initialized = run(external(f, &daemon, &storage.0).args(["--no-defaults", "--initialize-insecure", "--console"]).arg(format!("--basedir={}", bin.parent().unwrap().display())).arg(format!("--datadir={}", data.display())).arg(format!("--tmpdir={}", storage.0.join("temp").display())), b"");
    fs::write(f.root.join("mysql-initialize.txt"), format!("exit={}\nstdout:\n{}\nstderr:\n{}", initialized.code, String::from_utf8_lossy(&initialized.out), String::from_utf8_lossy(&initialized.err))).unwrap();
    assert_eq!(initialized.code, 0, "real owned MySQL initialization: {}", String::from_utf8_lossy(&initialized.err));
    let reservation = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let log = fs::File::create(f.root.join("mysql-server.log")).unwrap();
    let mut command = external(f, &daemon, &storage.0);
    // RESTART is not exercised: retain one real server process, not MySQL's
    // self-launching monitor (which also rejects this Unicode cwd with error267).
    command.args(["--no-defaults", "--no-monitor", "--bind-address=127.0.0.1", "--mysqlx=0", "--skip-log-bin", "--console"])
        .arg(format!("--basedir={}", bin.parent().unwrap().display())).arg(format!("--datadir={}", data.display()))
        .arg(format!("--tmpdir={}", storage.0.join("temp").display())).arg(format!("--secure-file-priv={}", storage.0.display()))
        .arg(format!("--port={port}")).stdin(Stdio::null()).stdout(log.try_clone().unwrap()).stderr(log);
    drop(reservation);
    let server = Server(command.spawn().expect("start only the declared owned MySQL server"));
    let ready = run(external(f, &admin, &storage.0).args(["--no-defaults", "--no-login-paths", "--protocol=TCP", "--host=127.0.0.1", &format!("--port={port}"), "--user=root", "--connect-timeout=1", "--wait=3", "--silent", "ping"]), b"");
    assert_eq!(ready.code, 0, "real MySQL readiness protocol: {}; server log: {}", String::from_utf8_lossy(&ready.err), fs::read_to_string(f.root.join("mysql-server.log")).unwrap());
    let identity = client(f, &program, &storage.0, port, b"SELECT @@datadir;\n");
    assert_eq!(identity.code, 0, "owned MySQL identity query: {}", String::from_utf8_lossy(&identity.err));
    let identity = String::from_utf8(identity.out).unwrap();
    assert_eq!(fs::canonicalize(identity.trim()).unwrap(), fs::canonicalize(&data).unwrap(), "refuse SQL mutations on any server except the owned storage identity");
    let created = client(f, &program, &storage.0, port, "CREATE DATABASE `n9 data 雪` CHARACTER SET utf8mb4; CREATE DATABASE n9_ascii; CREATE TABLE `n9 data 雪`.owned_payload(value VARCHAR(32)) CHARACTER SET utf8mb4; INSERT INTO `n9 data 雪`.owned_payload VALUES ('fish-雪🐟'); CREATE USER 'n9_owner'@'localhost' IDENTIFIED BY 'n9 pass 雪 &'; GRANT SELECT ON `n9 data 雪`.* TO 'n9_owner'@'localhost';\n".as_bytes());
    assert_eq!(created.code, 0, "real owned schema/data creation: {}", String::from_utf8_lossy(&created.err));
    let baseline = client(f, &program, &storage.0, port, b"SHOW DATABASES;\n");
    assert_eq!(baseline.code, 0);
    let expected = records(baseline.out);
    let connection = format!("mysql --no-defaults --no-login-paths --host=127.0.0.1 --port={port} --user=root --default-character-set=utf8mb4 --connect-timeout=2");
    let all = shell(f, &program, &storage.0, &format!("complete -C {}", quote(&format!("{connection} -D "))));
    assert_eq!(all.code, 0, "real schema completion: {}", String::from_utf8_lossy(&all.err));
    assert_eq!(records(all.out), expected, "completion records must be genuine schemas, without headers or CR payload");
    let selected = format!("set -l database (complete -C {} | string split -f 1 \\t | string unescape | string match -- 'n9 data 雪'); test (count $database) -eq 1; or exit 81; {connection} --database=$database --batch --raw --skip-column-names -e {}", quote(&format!("{connection} -D n9")), quote("SELECT CONCAT(DATABASE(), ':', value) FROM owned_payload"));
    let result = shell(f, &program, &storage.0, &selected);
    assert_eq!(result.code, 0, "real Unicode database argv and query consumer: {}", String::from_utf8_lossy(&result.err));
    assert_eq!(String::from_utf8(result.out).unwrap().trim_end_matches(['\r', '\n']), "n9 data 雪:fish-雪🐟");
    let short = shell(f, &program, &storage.0, &format!("complete -C {}", quote(&format!("mysql --no-defaults --no-login-paths -h127.0.0.1 -P{port} -uroot -D n9"))));
    assert_eq!(short.code, 0);
    assert_eq!(records(short.out), BTreeSet::from(["n9 data 雪".to_owned(), "n9_ascii".to_owned()]));
    for credentials in [format!("--user=n9_owner --password={}", quote("n9 pass 雪 &")), format!("-un9_owner -p{}", quote("n9 pass 雪 &"))] {
        let connection = format!("mysql --no-defaults --no-login-paths -h127.0.0.1 -P{port} {credentials} --default-character-set=utf8mb4 --connect-timeout=2");
        let commandline = format!("{connection} --database=n9");
        let authenticated = shell(f, &program, &storage.0, &format!("complete -C {}", quote(&commandline)));
        assert_eq!(authenticated.code, 0, "authenticated real schema completion: {}", String::from_utf8_lossy(&authenticated.err));
        assert_eq!(records(authenticated.out), BTreeSet::from(["--database=n9 data 雪".to_owned()]), "limited credentials must not reveal the root-only ASCII schema");
        let consumed = shell(f, &program, &storage.0, &format!("set -l database (complete -C {} | string split -f 1 \\t | string unescape); test (count $database) -eq 1; or exit 81; {connection} $database --batch --raw --skip-column-names -e {}", quote(&commandline), quote("SELECT CONCAT(DATABASE(), ':', value) FROM owned_payload")));
        assert_eq!(consumed.code, 0, "real credential/Unicode selection consumer: {}", String::from_utf8_lossy(&consumed.err));
        assert_eq!(String::from_utf8(consumed.out).unwrap().trim_end_matches(['\r', '\n']), "n9 data 雪:fish-雪🐟");
    }
    let denied = shell(f, &program, &storage.0, &format!("complete -C {}", quote(&format!("mysql --no-defaults --no-login-paths -h127.0.0.1 -P{port} -un9_missing -D n9"))));
    assert_eq!(denied.code, 0);
    assert!(denied.out.is_empty() && denied.err.is_empty(), "failed authentication must not invent schemas");
    drop(server);
    let disconnected = shell(f, &program, &storage.0, &format!("complete -C {}", quote(&format!("{connection} -D n9"))));
    assert_eq!(disconnected.code, 0);
    assert!(disconnected.out.is_empty() && disconnected.err.is_empty(), "closed owned endpoint must not return stale schemas");
    let absent = f.eval("complete -C 'mysql -D n9'");
    assert_eq!(absent.code, 0);
    assert!(absent.out.is_empty() && absent.err.is_empty());
}
