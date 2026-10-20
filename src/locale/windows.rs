//! N4 native numeric/time consumers, independent of the process environment and CRT locale.
//!
//! LC_ALL > LC_NUMERIC/LC_TIME > LANG selects each supported category. Empty or
//! unexported values are unset; unsetting restores the captured Windows user
//! locale and local timezone. Names are Windows NLS locale names (also accepting
//! underscores and a UTF-8 suffix), or C/POSIX/C.UTF-8. Numeric punctuation comes
//! from NLS without user overrides and feeds fish_printf's existing Locale model.
//! The parser's numeric grammar and the process-global C locale never change.
//!
//! TZ accepts UTC or an installed Windows timezone key name, not IANA names or
//! POSIX rule strings. Windows dynamic timezone data converts the *stored* epoch,
//! including historical DST rules; no current-clock offset is substituted.
//! UCRT per-locale strftime supplies localized names and date/time patterns.
//! Supported directives are a A b B c d H I j m M p S U w W x X y Y %, plus
//! n t z Z and the POSIX aliases h D F R T r. Other directives are diagnosed
//! before entering UCRT (whose invalid-parameter handler may terminate a process).
//! Complete GNU formatting, other LC categories and catalog selection belong to N9.

use super::{C_LOCALE, Locale};
use crate::env::{EnvMode, Environment};
use crate::prelude::*;
use std::ffi::c_void;
use std::io;
use std::ptr::NonNull;
use std::sync::{Arc, LazyLock, Mutex};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn ticks(self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TimeZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: SystemTime,
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: SystemTime,
    daylight_bias: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DynamicTimeZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: SystemTime,
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: SystemTime,
    daylight_bias: i32,
    key_name: [u16; 128],
    dynamic_daylight_disabled: u8,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetUserDefaultLocaleName(name: *mut u16, capacity: i32) -> i32;
    fn IsValidLocaleName(name: *const u16) -> i32;
    fn GetLocaleInfoEx(name: *const u16, kind: u32, data: *mut u16, capacity: i32) -> i32;
    fn EnumSystemLocalesEx(
        callback: unsafe extern "system" fn(*mut u16, u32, isize) -> i32,
        flags: u32,
        parameter: isize,
        reserved: *mut c_void,
    ) -> i32;
    fn GetDynamicTimeZoneInformation(zone: *mut DynamicTimeZoneInformation) -> u32;
    fn EnumDynamicTimeZoneInformation(index: u32, zone: *mut DynamicTimeZoneInformation) -> u32;
    fn SystemTimeToTzSpecificLocalTimeEx(
        zone: *const DynamicTimeZoneInformation,
        utc: *const SystemTime,
        local: *mut SystemTime,
    ) -> i32;
    fn GetTimeZoneInformationForYear(
        year: u16,
        zone: *const DynamicTimeZoneInformation,
        rules: *mut TimeZoneInformation,
    ) -> i32;
    fn FileTimeToSystemTime(file: *const FileTime, time: *mut SystemTime) -> i32;
    fn SystemTimeToFileTime(time: *const SystemTime, file: *mut FileTime) -> i32;
}

unsafe extern "C" {
    fn _wcreate_locale(category: i32, name: *const u16) -> *mut c_void;
    fn _free_locale(locale: *mut c_void);
    fn _wcsftime_l(
        output: *mut u16,
        capacity: usize,
        format: *const u16,
        time: *const libc::tm,
        locale: *mut c_void,
    ) -> usize;
}

struct TimeLocale(NonNull<c_void>);

// A locale object is immutable after _wcreate_locale; _wcsftime_l only borrows it.
// Arc snapshots retain it until all formatting calls finish, before _free_locale.
unsafe impl Send for TimeLocale {}
unsafe impl Sync for TimeLocale {}

impl Drop for TimeLocale {
    fn drop(&mut self) {
        unsafe { _free_locale(self.0.as_ptr()) };
    }
}

impl TimeLocale {
    fn new(name: &[u16]) -> io::Result<Self> {
        NonNull::new(unsafe { _wcreate_locale(libc::LC_TIME, name.as_ptr()) })
            .map(Self)
            .ok_or_else(|| invalid("locale is not supported by the native time formatter"))
    }
}

#[derive(Clone, Copy)]
enum TimeZone {
    Utc,
    Windows(DynamicTimeZoneInformation),
}

struct NativeLocale {
    numeric: Locale,
    time: Arc<TimeLocale>,
    zone: TimeZone,
}

// Native defaults are captured once, without consulting fish's global
// environment. The active state lock never nests with environment locks:
// configuration reads all variables and builds services before publishing, and
// consumers retain an immutable snapshot before calling any native service.
static DEFAULTS: LazyLock<Result<Arc<NativeLocale>, String>> = LazyLock::new(read_defaults);
static ACTIVE: Mutex<Option<Arc<NativeLocale>>> = Mutex::new(None);

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn terminated(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn decode(text: &[u16]) -> io::Result<String> {
    let end = text
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(text.len());
    String::from_utf16(&text[..end]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "native locale returned invalid UTF-16",
        )
    })
}

fn read_defaults() -> Result<Arc<NativeLocale>, String> {
    let create = || -> io::Result<Arc<NativeLocale>> {
        let mut name = [0; 85]; // LOCALE_NAME_MAX_LENGTH
        let count = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        if count == 0 {
            return Err(io::Error::last_os_error());
        }
        let name = &name[..count as usize];
        let mut zone = std::mem::MaybeUninit::uninit();
        if unsafe { GetDynamicTimeZoneInformation(zone.as_mut_ptr()) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(Arc::new(NativeLocale {
            numeric: numeric_from_name(name)?,
            time: Arc::new(TimeLocale::new(name)?),
            zone: TimeZone::Windows(unsafe { zone.assume_init() }),
        }))
    };
    create().map_err(|error| format!("cannot initialize native locale services: {error}"))
}

fn defaults() -> Result<&'static Arc<NativeLocale>, String> {
    DEFAULTS.as_ref().map_err(Clone::clone)
}

fn snapshot() -> io::Result<Arc<NativeLocale>> {
    if let Some(locale) = ACTIVE.lock().unwrap().as_ref().map(Arc::clone) {
        return Ok(locale);
    }
    defaults().map(Arc::clone).map_err(io::Error::other)
}

pub(super) fn numeric_locale() -> Locale {
    // Native startup must report an initialization failure before evaluation.
    // There is deliberately no silent C fallback for a failed native service.
    if let Some(locale) = ACTIVE.lock().unwrap().as_ref() {
        return locale.numeric;
    }
    defaults()
        .expect("native locale services are unavailable")
        .numeric
}

fn selected(vars: &dyn Environment, category: &wstr) -> Option<(String, String)> {
    for key in [L!("LC_ALL"), category, L!("LANG")] {
        if let Some(value) = vars.getf_unless_empty(key, EnvMode::EXPORTED) {
            return Some((key.to_string(), value.as_string().to_string()));
        }
    }
    None
}

fn locale_name(value: &str) -> io::Result<Vec<u16>> {
    if matches!(value, "C" | "POSIX" | "C.UTF-8" | "C.utf8") {
        return Ok(terminated("C"));
    }
    let value = value
        .strip_suffix(".UTF-8")
        .or_else(|| value.strip_suffix(".utf8"))
        .unwrap_or(value);
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid(
            "expected C/POSIX or a Windows locale name (for example de-DE); other codesets/modifiers are not supported in N4",
        ));
    }
    let name = terminated(&value.replace('_', "-"));
    if unsafe { IsValidLocaleName(name.as_ptr()) } == 0 {
        return Err(invalid("unknown Windows locale name"));
    }
    Ok(name)
}

/// Enumerate canonical Windows locale names usable by both native categories.
/// Alternate-sort names contain underscores that our parser normalizes as
/// language/region separators, so they are deliberately not completion values.
pub fn native_locale_names() -> io::Result<Vec<WString>> {
    unsafe extern "system" fn collect(name: *mut u16, flags: u32, parameter: isize) -> i32 {
        if flags & 0x04 != 0 {
            // LOCALE_ALTERNATE_SORTS
            return 1;
        }
        // EnumSystemLocalesEx supplies a terminated LOCALE_NAME_MAX_LENGTH name.
        let mut length = 0;
        while unsafe { *name.add(length) } != 0 {
            length += 1;
        }
        let name = unsafe { std::slice::from_raw_parts(name, length + 1) };
        // Canonical names must pass the same ASCII grammar and NLS validation as
        // locale_name, without its optional suffix/underscore normalization.
        if length == 0
            || !name[..length].iter().all(|&unit| {
                matches!(unit, 0x2d | 0x30..=0x39 | 0x41..=0x5a | 0x61..=0x7a)
            })
            || unsafe { IsValidLocaleName(name.as_ptr()) } == 0
            || numeric_from_name(name).is_err()
            || TimeLocale::new(name).is_err()
        {
            return 1;
        }
        // The validated ASCII name needs only one allocation, directly into
        // shell text; do not round-trip NLS UTF-16 through UTF-8 and back.
        let names = unsafe { &mut *(parameter as *mut Vec<WString>) };
        names.push(name[..length].iter().map(|&unit| char::from(unit as u8)).collect());
        1
    }

    let mut names = vec![
        L!("C").to_owned(),
        L!("POSIX").to_owned(),
        L!("C.UTF-8").to_owned(),
        L!("C.utf8").to_owned(),
    ];
    // LOCALE_ALL includes installed Windows, supplemental, and neutral locales;
    // the synchronous callback receives only this invocation's local state.
    if unsafe {
        EnumSystemLocalesEx(
            collect,
            0,
            &mut names as *mut Vec<WString> as isize,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

fn locale_info(name: &[u16], kind: u32) -> io::Result<String> {
    let mut value = [0; 128];
    let count = unsafe {
        GetLocaleInfoEx(
            name.as_ptr(),
            kind | 0x8000_0000,
            value.as_mut_ptr(),
            value.len() as i32,
        )
    }; // LOCALE_NOUSEROVERRIDE
    if count == 0 {
        return Err(io::Error::last_os_error());
    }
    decode(&value[..count as usize])
}

fn single_character(value: &str) -> io::Result<Option<char>> {
    let mut chars = value.chars();
    let result = chars.next();
    if chars.next().is_some() {
        return Err(invalid(
            "native numeric locale has punctuation not representable by fish's single-character locale model",
        ));
    }
    Ok(result)
}

fn numeric_from_name(name: &[u16]) -> io::Result<Locale> {
    if name == [b'C' as u16, 0] {
        return Ok(C_LOCALE);
    }
    let decimal_point = single_character(&locale_info(name, 0x0e)?)?
        .ok_or_else(|| invalid("native numeric locale has an empty decimal separator"))?;
    let thousands_sep = single_character(&locale_info(name, 0x0f)?)?;
    let groups = locale_info(name, 0x10)?;
    let mut grouping = [0; 4];
    let mut count = 0;
    let mut group_repeat = false;
    let mut fields = groups.split(';').peekable();
    while let Some(field) = fields.next() {
        let group = field
            .parse::<u8>()
            .map_err(|_| invalid("invalid native digit grouping"))?;
        if group == 0 {
            if fields.peek().is_some() {
                return Err(invalid("invalid native digit grouping terminator"));
            }
            group_repeat = count > 0;
            if group_repeat {
                let repeat = grouping[count - 1];
                grouping[count..].fill(repeat);
            }
            break;
        }
        if group > 9 || count == grouping.len() {
            return Err(invalid(
                "native digit grouping exceeds fish's numeric locale model",
            ));
        }
        grouping[count] = group;
        count += 1;
    }
    Ok(Locale {
        decimal_point,
        thousands_sep,
        grouping,
        group_repeat,
    })
}

fn timezone(value: &str) -> io::Result<TimeZone> {
    if value.eq_ignore_ascii_case("UTC") {
        return Ok(TimeZone::Utc);
    }
    if value.contains('\0') {
        return Err(invalid("TZ contains a NUL character"));
    }
    let mut index = 0;
    loop {
        let mut zone = std::mem::MaybeUninit::<DynamicTimeZoneInformation>::uninit();
        let status = unsafe { EnumDynamicTimeZoneInformation(index, zone.as_mut_ptr()) };
        if status == 259 {
            // ERROR_NO_MORE_ITEMS
            return Err(invalid(
                "expected UTC or an installed Windows timezone ID (for example Pacific Standard Time); IANA names and POSIX TZ rules are deferred to N9",
            ));
        }
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let mut zone = unsafe { zone.assume_init() };
        if decode(&zone.key_name)?.eq_ignore_ascii_case(value) {
            zone.dynamic_daylight_disabled = 0;
            return Ok(TimeZone::Windows(zone));
        }
        index += 1;
    }
}

/// Reconfigure and invalidate the native numeric/time consumers from this exact
/// shell environment. No process-global environment or CRT locale is modified.
/// All selected categories and TZ are validated before atomic publication; any
/// diagnostic leaves the previous complete configuration intact. Call outside
/// environment locks, on the variable-dispatch thread (including initial setup).
/// Only exported, nonempty values participate, as on the Unix locale path.
/// LC_ALL/LANG affect the two N4 categories, not catalog/collation selection.
pub fn configure_native_locale(vars: &dyn Environment) -> Result<(), Vec<String>> {
    let defaults = defaults().map_err(|error| vec![error])?;
    let numeric = selected(vars, L!("LC_NUMERIC"));
    let time = selected(vars, L!("LC_TIME"));
    let zone = vars
        .getf_unless_empty(L!("TZ"), EnvMode::EXPORTED)
        .map(|value| value.as_string().to_string());
    let mut errors = Vec::new();
    let numeric_result = match &numeric {
        Some((_, value)) => locale_name(value).and_then(|name| numeric_from_name(&name)),
        None => Ok(defaults.numeric),
    };
    let time_result = match &time {
        Some((_, value)) => locale_name(value)
            .and_then(|name| TimeLocale::new(&name))
            .map(Arc::new),
        None => Ok(Arc::clone(&defaults.time)),
    };
    let zone_result = match &zone {
        Some(value) => timezone(value),
        None => Ok(defaults.zone),
    };
    let describe =
        |category: &str, selection: &Option<(String, String)>, error: &io::Error| match selection {
            Some((key, value)) => format!("native {category}: {key}={value:?}: {error}"),
            None => format!("native {category} default: {error}"),
        };
    if let Err(error) = &numeric_result {
        errors.push(describe("LC_NUMERIC", &numeric, error));
    }
    if let Err(error) = &time_result {
        errors.push(describe("LC_TIME", &time, error));
    }
    if let Err(error) = &zone_result {
        errors.push(format!("native TZ={zone:?}: {error}"));
    }
    match (numeric_result, time_result, zone_result) {
        (Ok(numeric), Ok(time), Ok(zone)) => {
            *ACTIVE.lock().unwrap() = Some(Arc::new(NativeLocale {
                numeric,
                time,
                zone,
            }));
            Ok(())
        }
        _ => Err(errors),
    }
}

struct LocalTimestamp {
    time: libc::tm,
    offset_seconds: i64,
    zone_name: String,
}

fn local_timestamp(seconds: i64, zone: &TimeZone) -> io::Result<LocalTimestamp> {
    const EPOCH_OFFSET: i128 = 11_644_473_600;
    const TICKS_PER_SECOND: i128 = 10_000_000;
    let ticks = u64::try_from((i128::from(seconds) + EPOCH_OFFSET) * TICKS_PER_SECOND)
        .map_err(|_| invalid("history timestamp is outside the native Windows date range"))?;
    let file = FileTime {
        low: ticks as u32,
        high: (ticks >> 32) as u32,
    };
    let mut utc = SystemTime::default();
    if unsafe { FileTimeToSystemTime(&file, &mut utc) } == 0 {
        return Err(invalid(
            "history timestamp is outside the native Windows date range",
        ));
    }
    let mut local = utc;
    let mut daylight = false;
    let mut zone_name = "UTC".to_owned();
    let mut wall_file = file;
    if let TimeZone::Windows(zone) = zone {
        if unsafe { SystemTimeToTzSpecificLocalTimeEx(zone, &utc, &mut local) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut rules = std::mem::MaybeUninit::uninit();
        if unsafe { GetTimeZoneInformationForYear(local.year, zone, rules.as_mut_ptr()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let rules = unsafe { rules.assume_init() };
        if unsafe { SystemTimeToFileTime(&local, &mut wall_file) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let offset = (i128::from(wall_file.ticks()) - i128::from(ticks)) / TICKS_PER_SECOND;
        daylight = zone.dynamic_daylight_disabled == 0
            && rules.daylight_date.month != 0
            && offset == -60 * (i128::from(rules.bias) + i128::from(rules.daylight_bias))
            && rules.daylight_bias != rules.standard_bias;
        zone_name = decode(if daylight {
            &rules.daylight_name
        } else {
            &rules.standard_name
        })?;
    }
    // UCRT validates tm_year even if a particular directive does not use it.
    if !(1900..=9999).contains(&local.year) {
        return Err(invalid(
            "native history formatting supports years 1900 through 9999",
        ));
    }
    let offset_seconds =
        ((i128::from(wall_file.ticks()) - i128::from(ticks)) / TICKS_PER_SECOND) as i64;
    let leap = local.year % 4 == 0 && (local.year % 100 != 0 || local.year % 400 == 0);
    let month_starts = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let yday = month_starts[usize::from(local.month - 1)] + i32::from(local.day) - 1
        + i32::from(leap && local.month > 2);
    Ok(LocalTimestamp {
        time: libc::tm {
            tm_sec: i32::from(local.second),
            tm_min: i32::from(local.minute),
            tm_hour: i32::from(local.hour),
            tm_mday: i32::from(local.day),
            tm_mon: i32::from(local.month) - 1,
            tm_year: i32::from(local.year) - 1900,
            tm_wday: i32::from(local.day_of_week),
            tm_yday: yday,
            tm_isdst: i32::from(daylight),
        },
        offset_seconds,
        zone_name,
    })
}

fn checked_format(format: &str, timestamp: &LocalTimestamp) -> io::Result<Vec<u16>> {
    // Prefix a literal sentinel: an empty successful expansion must be distinct
    // from _wcsftime_l's zero return for insufficient output space.
    let mut result = String::from("x");
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch == '\0' {
            return Err(invalid("history time format contains a NUL character"));
        }
        if ch != '%' {
            result.push(ch);
            continue;
        }
        let spec = chars
            .next()
            .ok_or_else(|| invalid("incomplete history time format directive"))?;
        match spec {
            'a' | 'A' | 'b' | 'B' | 'c' | 'd' | 'H' | 'I' | 'j' | 'm' | 'M' | 'p' | 'S' | 'U'
            | 'w' | 'W' | 'x' | 'X' | 'y' | 'Y' | '%' => {
                result.push('%');
                result.push(spec);
            }
            'n' => result.push('\n'),
            't' => result.push('\t'),
            'h' => result.push_str("%b"),
            'D' => result.push_str("%m/%d/%y"),
            'F' => result.push_str("%Y-%m-%d"),
            'R' => result.push_str("%H:%M"),
            'T' => result.push_str("%H:%M:%S"),
            'r' => result.push_str("%I:%M:%S %p"),
            'z' => {
                let offset = timestamp.offset_seconds;
                let sign = if offset < 0 { '-' } else { '+' };
                let minutes = offset.unsigned_abs() / 60;
                result.push_str(&format!("{sign}{:02}{:02}", minutes / 60, minutes % 60));
            }
            'Z' => {
                // A localized timezone name is a literal, never a format string.
                result.push_str(&timestamp.zone_name.replace('%', "%%"));
            }
            _ => {
                return Err(invalid(format!(
                    "unsupported native history time directive %{spec}; full GNU strftime formatting is deferred to N9"
                )));
            }
        }
    }
    Ok(terminated(&result))
}

/// Format a stored history epoch using the latest complete native LC_TIME/TZ
/// snapshot. Errors propagate to history's normal diagnostic/nonzero path.
pub fn format_history_timestamp(seconds: i64, format: &str) -> io::Result<WString> {
    let locale = snapshot()?;
    let timestamp = local_timestamp(seconds, &locale.zone)?;
    let format = checked_format(format, &timestamp)?;
    const MAX_OUTPUT: usize = 65_536;
    let mut capacity = format.len().saturating_add(256).clamp(512, MAX_OUTPUT);
    loop {
        let mut output = vec![0; capacity];
        let count = unsafe {
            _wcsftime_l(
                output.as_mut_ptr(),
                output.len(),
                format.as_ptr(),
                &timestamp.time,
                locale.time.0.as_ptr(),
            )
        };
        if count != 0 {
            let text = String::from_utf16(&output[1..count]).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "native time formatter returned invalid UTF-16",
                )
            })?;
            return Ok(WString::from(text.as_str()));
        }
        if capacity == MAX_OUTPUT {
            return Err(invalid(
                "native history time format exceeds the formatter's output limit",
            ));
        }
        capacity = capacity.saturating_mul(2).min(MAX_OUTPUT);
    }
}
