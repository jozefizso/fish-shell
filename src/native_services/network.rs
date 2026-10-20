//! Local adapter and hosts-file observations. No DNS/domain discovery or network tool dependency.
use super::{Handle, decode, invalid, text, unsupported, wide};
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

#[repr(C)]
struct SocketAddress { address: *const u8, length: i32 }
#[repr(C)]
struct Unicast { header: u64, next: *const Unicast, address: SocketAddress }
#[repr(C)]
struct Adapter {
    header: u64,
    next: *const Adapter,
    name: *const u8,
    unicast: *const Unicast,
    anycast: *const c_void,
    multicast: *const c_void,
    dns: *const c_void,
    suffix: *const u16,
    description: *const u16,
    friendly: *const u16,
    physical: [u8; 8],
    physical_length: u32,
    flags: u32,
    mtu: u32,
    kind: u32,
    status: i32,
    ipv6_index: u32,
    zones: [u32; 16],
}
#[link(name = "iphlpapi")]
unsafe extern "system" {
    fn GetAdaptersAddresses(family: u32, flags: u32, reserved: *mut c_void, addresses: *mut Adapter, size: *mut u32) -> u32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(key: Handle, subkey: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" { fn GetWindowsDirectoryW(buffer: *mut u16, size: u32) -> u32; }

#[derive(Debug)]
pub struct Interface {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub loopback: bool,
    pub up: bool,
    /// IPv6 scope IDs are retained, including link-local interface scope.
    pub addresses: Vec<String>,
}

pub fn interfaces() -> io::Result<Vec<Interface>> {
    let mut size = 16384u32;
    // A changing adapter table can grow between calls. Retries are bounded and preserve
    // the real overflow error rather than publishing a partial list.
    for _ in 0..4 {
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let status = unsafe { GetAdaptersAddresses(0, 0x10e, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
        if status == 111 { continue; }
        if status != 0 { return Err(io::Error::from_raw_os_error(status as i32)); }
        let mut next = buffer.as_ptr().cast::<Adapter>();
        let mut result = Vec::new();
        while !next.is_null() {
            let adapter = unsafe { &*next };
            let mut addresses = Vec::new();
            let mut unicast = adapter.unicast;
            while !unicast.is_null() {
                let address = unsafe { &*unicast };
                if address.address.address.is_null() || address.address.length < 2 { return Err(invalid("invalid native socket address")); }
                let data = unsafe { std::slice::from_raw_parts(address.address.address, address.address.length as usize) };
                let family = u16::from_ne_bytes([data[0], data[1]]);
                match family {
                    2 if data.len() >= 16 => addresses.push(Ipv4Addr::new(data[4], data[5], data[6], data[7]).to_string()),
                    23 if data.len() >= 28 => {
                        let value = Ipv6Addr::from(<[u8; 16]>::try_from(&data[8..24]).unwrap());
                        let scope = u32::from_ne_bytes(data[24..28].try_into().unwrap());
                        addresses.push(if scope == 0 { value.to_string() } else { format!("{value}%{scope}") });
                    }
                    _ => return Err(unsupported("adapter returned an unsupported socket address family or size")),
                }
                unicast = address.next;
            }
            result.push(Interface {
                index: (adapter.header >> 32) as u32,
                name: unsafe { text(adapter.friendly)? },
                description: unsafe { text(adapter.description)? },
                loopback: adapter.kind == 24,
                up: adapter.status == 1,
                addresses,
            });
            next = adapter.next;
        }
        return Ok(result);
    }
    Err(io::Error::from_raw_os_error(111))
}

/// Locate the configured Windows TCP/IP database directory. Expansion is deliberately
/// limited to the OS-reported SystemRoot, never user-controlled inherited variables.
pub fn hosts_path() -> io::Result<PathBuf> {
    let key = wide("SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters")?;
    let value = wide("DataBasePath")?;
    let machine = 0x80000002u32 as i32 as isize as Handle;
    let flags = 2 | 4 | 0x10000000;
    let mut size = 0;
    let status = unsafe { RegGetValueW(machine, key.as_ptr(), value.as_ptr(), flags, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) };
    if status != 0 { return Err(io::Error::from_raw_os_error(status)); }
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe { RegGetValueW(machine, key.as_ptr(), value.as_ptr(), flags, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
    if status != 0 { return Err(io::Error::from_raw_os_error(status)); }
    let length = buffer.iter().position(|&c| c == 0).ok_or_else(|| invalid("unterminated TCP/IP database path"))?;
    let mut path = decode(&buffer[..length])?;
    if path.get(..12).is_some_and(|prefix| prefix.eq_ignore_ascii_case("%SystemRoot%")) {
        let mut windows = vec![0u16; 32768];
        let length = unsafe { GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) } as usize;
        if length == 0 { return Err(io::Error::last_os_error()); }
        if length >= windows.len() { return Err(invalid("Windows directory exceeds API buffer")); }
        path.replace_range(..12, &decode(&windows[..length])?);
    }
    if path.contains('%') { return Err(unsupported("TCP/IP database path uses unsupported environment expansion")); }
    let path = crate::path::native_path_text(&PathBuf::from(path))?;
    Ok(crate::path::native_absolute_path(&path)?.join("hosts"))
}

/// Parse a caller-owned fixture or the actual Windows hosts database. Strict UTF-8
/// (with optional BOM) or BOM-marked UTF-16; malformed identities are errors.
pub fn hosts(path: &Path) -> io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let contents = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 { return Err(invalid("odd-length UTF-16 hosts file")); }
        let little = bytes[0] == 0xff;
        let units: Vec<_> = bytes[2..].chunks_exact(2).map(|b| if little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) }).collect();
        decode(&units)?
    } else {
        std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes)).map_err(|_| invalid("hosts file is not valid UTF-8 or BOM-marked UTF-16"))?.to_owned()
    };
    let mut names = BTreeSet::new();
    for (number, line) in contents.lines().enumerate() {
        let mut words = line.split('#').next().unwrap().split_whitespace();
        let Some(address) = words.next() else { continue; };
        address.parse::<IpAddr>().map_err(|_| invalid(&format!("invalid hosts address on line {}", number + 1)))?;
        for name in words {
            if name.contains('\0') || name.chars().any(char::is_control) { return Err(invalid("invalid control character in hosts name")); }
            names.insert(name.to_owned());
        }
    }
    Ok(names.into_iter().collect())
}
