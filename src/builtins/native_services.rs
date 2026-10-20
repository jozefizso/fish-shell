//! Private native helper protocol. No Unix command-line compatibility is implied.
use super::prelude::*;
use crate::native_services::{accounts, network, processes};
use crate::env::Environment as _;
use fish_widestring::{native_to_shell_text_owned, shell_text_to_osstring, str2wcstring};
use std::io;

fn argument(value: &wstr) -> io::Result<String> {
    shell_text_to_osstring(value)?.into_string().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid native service Unicode"))
}
fn field(value: &str) -> String {
    value.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}
fn name(value: &str) -> io::Result<&str> {
    if value.chars().any(char::is_control) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "native identity contains a control character not representable by the completion line protocol"));
    }
    Ok(value)
}
fn line(streams: &mut IoStreams, value: &str) {
    streams.out.appendln(&native_to_shell_text_owned(str2wcstring(value)));
}
fn identity(streams: &mut IoStreams, account: accounts::AccountIdentity) -> io::Result<()> {
    line(streams, &format!("{}\\{}\t{}\t{}", name(&account.domain)?, name(&account.name)?, account.sid, account.kind));
    Ok(())
}

pub fn native_services(parser: &mut Parser, streams: &mut IoStreams, args: &mut [&wstr]) -> BuiltinResult {
    let result = (|| -> io::Result<()> {
        let operation = args.get(1).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected native service operation"))?;
        let operands = &args[2..];
        match *operation {
            op if op == L!("users") && operands.is_empty() => {
                for entry in accounts::local_users()? { line(streams, &format!("{}\t{}", name(&entry.name)?, field(&entry.description))); }
            }
            op if op == L!("groups") && operands.is_empty() => {
                for entry in accounts::local_groups()? { line(streams, &format!("{}\t{}", name(&entry.name)?, field(&entry.description))); }
            }
            op if op == L!("token-groups") && operands.is_empty() => {
                for (sid, attributes) in accounts::token_groups()? { line(streams, &format!("{sid}\t{attributes}")); }
            }
            op if op == L!("account") && operands.len() <= 1 => {
                identity(streams, match operands.first() { Some(name) => accounts::lookup(&argument(name)?)?, None => accounts::current_account()? })?;
            }
            op if op == L!("profile") && operands.len() <= 1 => {
                let profile = match operands.first() { Some(name) => accounts::profile_for_user(name)?, None => accounts::current_profile()? };
                streams.out.appendln(&profile);
            }
            op if op == L!("processes") && operands.is_empty() => {
                for entry in processes::snapshot()? {
                    // Snapshot image names are available even where full-image access is denied.
                    line(streams, &format!("{}\t{}", entry.pid, name(&entry.image)?));
                }
            }
            op if op == L!("process-image") && operands.len() == 1 => {
                let pid = argument(operands[0])?.parse::<u32>().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "expected a Windows process ID"))?;
                let image = processes::image(pid)?;
                streams.out.appendln(&crate::path::native_path_text(std::path::Path::new(&image))?);
            }
            op if (op == L!("interfaces") || op == L!("addresses")) && operands.is_empty() => {
                for interface in network::interfaces()? {
                    if op == L!("interfaces") {
                        line(streams, name(&interface.name)?);
                    } else {
                        for address in interface.addresses { line(streams, &format!("{}\t{}\t{}", address, name(&interface.name)?, interface.index)); }
                    }
                }
            }
            op if op == L!("hostname") && operands.is_empty() => { streams.out.appendln(&crate::identity_windows::hostname()?); }
            op if op == L!("username") && operands.is_empty() => { streams.out.appendln(&crate::identity_windows::username()?); }
            op if op == L!("hosts") && operands.len() <= 1 => {
                let path = match operands.first() {
                    Some(path) => crate::path::native_resolve_path_for_environment(path, parser.vars())?,
                    None => network::hosts_path()?,
                };
                for name in network::hosts(&path)? { line(streams, &name); }
            }
            op if op == L!("drives") && operands.is_empty() => {
                for (root, kind) in crate::native_services::desktop::drives()? {
                    line(streams, &format!("{}\t{kind}", name(&root)?));
                }
            }
            op if op == L!("open") && !operands.is_empty() => {
                let cwd = crate::path::native_absolute_path(&parser.vars().get_working_directory()?)?;
                for operand in operands {
                    crate::native_services::desktop::open(&argument(operand)?, &cwd)?;
                }
            }
            op if op == L!("accounting") && operands.is_empty() => {
                let snapshot = crate::native_accounting::current_process_snapshot()?;
                for (key, value) in [
                    ("user-ns", snapshot.times.cpu.user.as_nanos()),
                    ("kernel-ns", snapshot.times.cpu.kernel.as_nanos()),
                    ("elapsed-ns", snapshot.times.elapsed.as_nanos()),
                    ("working-set-bytes", snapshot.working_set_bytes as u128),
                    ("peak-working-set-bytes", snapshot.peak_working_set_bytes as u128),
                    ("read-bytes", snapshot.io.read_bytes as u128),
                    ("write-bytes", snapshot.io.write_bytes as u128),
                ] { line(streams, &format!("{key}\t{value}")); }
            }
            op if op == L!("locales") && operands.is_empty() => {
                for name in crate::locale::native_locale_names()? {
                    streams.out.appendln(&name);
                }
            }
            op if op == L!("timestamp") && operands.len() == 2 => {
                let seconds = if operands[0] == L!("now") {
                    i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                        .map_err(io::Error::other)?.as_secs()).map_err(io::Error::other)?
                } else {
                    argument(operands[0])?.parse::<i64>().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "expected Unix epoch seconds or now, not date utility options"))?
                };
                streams.out.appendln(&crate::locale::format_history_timestamp(seconds, &argument(operands[1])?)?);
            }
            op if [L!("uid"), L!("gid"), L!("domain-users"), L!("domain-groups")].contains(&op) => {
                return Err(io::Error::new(io::ErrorKind::Unsupported, "Windows accounts use SIDs, not numeric Unix identity; enumeration is local SAM only; explicit DOMAIN\\name lookup uses Windows account resolution and can fail offline"));
            }
            op if op == L!("capabilities") && operands.is_empty() => {
                // This is a scope ledger, not a claim that privileged queries must succeed.
                for value in [
                    "processes\tToolhelp snapshot; full-image query may be denied or race exit",
                    "users/groups\tlocal SAM only; NetAPI errors returned; no domain enumeration",
                    "account/profile\tWindows SID/token; explicit domain lookup may fail offline; profile must exist locally",
                    "token-groups\tprocess-token SIDs and Windows SE_GROUP attributes, including disabled/deny-only membership",
                    "interfaces/addresses/hostname/hosts\tlocal Windows APIs and configured hosts database; no DNS discovery",
                    "timestamp\tN4 locale/timezone formatter; epoch seconds and explicit format only",
                    "locales\tWindows NLS names accepted by native numeric/time consumers; C/POSIX/UTF-8 aliases; no alternate-sort names",
                    "accounting/time\tN5 process counters and time decorator; ns/bytes, no POSIX rusage invention",
                    "drives/open\tWin32 logical volume inventory and registered desktop open verb; operands are data, not shell syntax",
                    "uid/gid/domain-enumeration\tunsupported; no optional executable fallback",
                ] { line(streams, value); }
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected users|groups|token-groups|account [NAME]|profile [USER]|processes|process-image PID|interfaces|addresses|hostname|username|hosts [PATH]|drives|open TARGET...|accounting|locales|timestamp SECONDS|now FORMAT|capabilities; no Unix utility options")),
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(SUCCESS),
        Err(error) => {
            streams.err.appendln(&str2wcstring(format!("__fish_native_services: {error}")));
            Err(STATUS_CMD_ERROR)
        }
    }
}
