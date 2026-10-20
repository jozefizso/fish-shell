use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{ffi::{OsStr, c_void}, fs, os::windows::{ffi::OsStrExt, io::AsRawHandle}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-typst-selected-format-pages-ppi-and-decoded-pixels", render),
    ("real-native-typst-selected-query-field-one-and-unicode-metadata", query),
    ("real-native-typst-selected-font-path-and-isolated-font-availability", fonts),
    ("real-native-typst-selected-watch-and-decoded-image-recompilation", watch),
];

const DOCUMENT: &str = r##"#set page(width: 1in, height: 0.5in, margin: 0pt, fill: rgb("#ff0000"))
#box(width: 1pt, height: 1pt)
#metadata("owned Unicode metadata 雪 🐟") <owned>
#pagebreak()
#set page(fill: rgb("#0000ff"))
#box(width: 1pt, height: 1pt)
#metadata("second Unicode metadata 魚") <second>
"##;

fn required(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("required genuine native tool fixture {name}")));
    assert!(path.is_absolute() && path.is_file(), "real absolute {name} fixture: {}", path.display());
    path
}

fn evaluate(f: &Fixture, typst: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, typst);
    run(&mut command, b"")
}

fn native_command(f: &Fixture, program: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(&f.root).env_clear();
    for (name, value) in f.command(&[]).get_envs() {
        if let Some(value) = value { command.env(name, value); }
    }
    tool_environment(&mut command, program);
    command
}

fn native(f: &Fixture, program: &Path, args: &[&OsStr]) -> Captured {
    run(native_command(f, program).args(args), b"")
}

fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine Typst consumer: {}", String::from_utf8_lossy(&result.err));
}

fn setup(f: &Fixture) -> PathBuf {
    let typst = required("FISH_N9_TYPST");
    fs::write(f.root.join("input 雪 🐟.typ"), DOCUMENT).unwrap();
    let resolved = evaluate(f, &typst, "command -s typst");
    success(&resolved);
    assert!(resolved.err.is_empty());
    let resolved = PathBuf::from(String::from_utf8(resolved.out).unwrap().trim());
    assert_eq!(fs::canonicalize(resolved).unwrap(), fs::canonicalize(&typst).unwrap());
    typst
}

fn candidates(f: &Fixture, typst: &Path, context: &str) -> Vec<String> {
    let result = evaluate(f, typst, &format!("complete -C {}", quote(context)));
    success(&result);
    assert!(result.err.is_empty(), "generated completion context: {context}");
    String::from_utf8(result.out).unwrap().lines()
        .map(|line| line.split('\t').next().unwrap().to_owned()).collect()
}

fn render(f: &Fixture) {
    let typst = setup(f);
    let magick = required("FISH_N9_MAGICK");
    let formats = candidates(f, &typst, "typst compile --format ");
    assert!(formats.iter().any(|value| value == "png"));
    assert!(!formats.iter().any(|value| value == "json" || value == "yaml"));
    for (context, forbidden) in [
        ("typst --ppi", "--ppi"),
        ("typst query --ppi", "--ppi"),
        ("typst compile --field", "--field"),
        ("typst compile --one", "--one"),
    ] {
        assert!(!candidates(f, &typst, context).iter().any(|value| value == forbidden),
            "wrong-subcommand option leaked: {context}");
    }

    // The selected PNG format must override the unrelated .image extension.
    // Pages 1 and 2 differ in every pixel; the PPI change doubles both axes.
    for (page, ppi, width, height, pixel) in [
        (1, 72, 72u32, 36u32, [255u8, 0, 0]),
        (2, 72, 72, 36, [0, 0, 255]),
        (2, 144, 144, 72, [0, 0, 255]),
    ] {
        let output = format!("page {page} ppi {ppi} 雪 🐟.image");
        let compiled = evaluate(f, &typst, &format!(r#"
set -l compile (complete -C 'typst comp' | string split -f 1 \t | string match -- compile)
set -l format_option (complete -C 'typst compile --for' | string split -f 1 \t | string match -- --format)
set -l png (complete -C 'typst compile --format p' | string split -f 1 \t | string match -- png)
set -l ppi (complete -C 'typst compile --pp' | string split -f 1 \t | string match -- --ppi)
set -l pages (complete -C 'typst compile --pag' | string split -f 1 \t | string match -- --pages)
for selected in compile format_option png ppi pages
    test (count $$selected) -eq 1; or exit 81
end
command typst $compile $format_option $png $ppi {ppi} $pages {page} 'input 雪 🐟.typ' {output}
"#, output = quote(&output)));
        success(&compiled);
        assert!(compiled.out.is_empty() && compiled.err.is_empty());
        let bytes = fs::read(f.root.join(&output)).unwrap();
        assert!(bytes.len() >= 24, "PNG header required");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), width);
        assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), height);

        let identified = native(f, &magick, &[
            OsStr::new("identify"), OsStr::new("-format"), OsStr::new("%m %w %h"), OsStr::new(&output),
        ]);
        success(&identified);
        assert!(identified.err.is_empty());
        assert_eq!(identified.out, format!("PNG {width} {height}").as_bytes());
        let decoded = native(f, &magick, &[
            OsStr::new(&output), OsStr::new("-depth"), OsStr::new("8"), OsStr::new("rgb:-"),
        ]);
        success(&decoded);
        assert!(decoded.err.is_empty());
        assert_eq!(decoded.out.len(), (width * height * 3) as usize);
        assert!(decoded.out.chunks_exact(3).all(|actual| actual == pixel), "selected page {page} must supply every decoded pixel at PPI {ppi}");
    }

    // Explicitly source the shipped producer: normal absent-command autoload
    // avoidance alone would not cover its true-absence guard.
    let absent = f.eval("status get-file completions/typst.fish | source; complete -C 'typst comp'");
    success(&absent);
    assert!(absent.err.is_empty(), "missing Typst producer must be quiet: {}", String::from_utf8_lossy(&absent.err));
    assert!(!String::from_utf8(absent.out).unwrap().lines()
        .any(|line| line.split('\t').next() == Some("compile")), "missing producer must not register generated commands");
    let missing = f.eval("command typst --version");
    assert_eq!(missing.code, 127);
    assert!(missing.out.is_empty());
}

fn query(f: &Fixture) {
    let typst = setup(f);
    let formats = candidates(f, &typst, "typst query --format ");
    assert!(formats.iter().any(|value| value == "json"));
    assert!(formats.iter().any(|value| value == "yaml"));
    assert!(!formats.iter().any(|value| value == "png" || value == "pdf"));
    for (context, forbidden) in [
        ("typst --field", "--field"),
        ("typst query --pages", "--pages"),
        ("typst fonts --field", "--field"),
        ("typst query comp", "compile"),
    ] {
        assert!(!candidates(f, &typst, context).iter().any(|value| value == forbidden),
            "wrong-context metadata leaked: {context}");
    }

    // This release deprecates query and legitimately warns on stderr. Its
    // success is established by exact Unicode data, not warning/help wording.
    for (format, selector, one, expected) in [
        ("json", "<owned>", true, "\"owned Unicode metadata 雪 🐟\""),
        ("json", "<owned>", false, "(\"owned Unicode metadata 雪 🐟\",)"),
        ("yaml", "<second>", true, "\"second Unicode metadata 魚\""),
    ] {
        let result = evaluate(f, &typst, &format!(r#"
set -l query (complete -C 'typst que' | string split -f 1 \t | string match -- query)
set -l field (complete -C 'typst query --fie' | string split -f 1 \t | string match -- --field)
set -l one (complete -C 'typst query --on' | string split -f 1 \t | string match -- --one)
set -l format_option (complete -C 'typst query --for' | string split -f 1 \t | string match -- --format)
set -l format (complete -C 'typst query --format {format}' | string split -f 1 \t | string match -- {format})
for selected in query field one format_option format
    test (count $$selected) -eq 1; or exit 82
end
command typst $query $field value {one} $format_option $format 'input 雪 🐟.typ' {selector}
"#, one = if one { "$one" } else { "" }, selector = quote(selector)));
        success(&result);
        // Parse the real serialization through Typst's independent data loader.
        // Whitespace/quoting choices are not part of JSON/YAML value semantics.
        let data = format!("received {format} 雪.data");
        let validator = format!("validate {format} 雪.typ");
        fs::write(f.root.join(&data), result.out).unwrap();
        fs::write(f.root.join(&validator),
            format!("#metadata({format}(\"{data}\") == {expected}) <validated>\n")).unwrap();
        let validated = evaluate(f, &typst, &format!("command typst query {} '<validated>' --field value --one --format json", quote(&validator)));
        success(&validated);
        assert_eq!(String::from_utf8(validated.out).unwrap().trim(), "true", "selected format must roundtrip the Unicode value and scalar/array type");
    }

    // --one is a real cardinality constraint: both metadata elements must not
    // silently become a scalar, and a missing selector must not invent data.
    for selector in ["metadata", "<missing>"] {
        let result = evaluate(f, &typst, &format!(r#"
set -l query (complete -C 'typst que' | string split -f 1 \t | string match -- query)
set -l field (complete -C 'typst query --fie' | string split -f 1 \t | string match -- --field)
set -l one (complete -C 'typst query --on' | string split -f 1 \t | string match -- --one)
for selected in query field one
    test (count $$selected) -eq 1; or exit 83
end
command typst $query $field value $one 'input 雪 🐟.typ' {selector}
"#, selector = quote(selector)));
        assert_eq!(result.code, 1);
        assert!(result.out.is_empty());
    }
}

struct OwnedDirectory(PathBuf);
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only this family's owned Typst files");
    }
}

struct CopiedFont(PathBuf);
impl Drop for CopiedFont {
    fn drop(&mut self) {
        let mut permissions = fs::metadata(&self.0).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&self.0, permissions).expect("clear read-only on only the owned font copy");
        fs::remove_file(&self.0).expect("remove only the owned font copy before its directory");
    }
}

fn fonts(f: &Fixture) {
    let typst = setup(f);
    let font = required("FISH_N9_TYPST_FONT");
    let directory = OwnedDirectory(f.root.join("isolated fonts 雪 🐟"));
    fs::create_dir(&directory.0).expect("create a new owned font directory, never reuse one");
    let consumer = format!(r#"
set -l fonts (complete -C 'typst fon' | string split -f 1 \t | string match -- fonts)
set -l font_path (complete -C 'typst fonts --font-p' | string split -f 1 \t | string match -- --font-path)
set -l ignore_system (complete -C 'typst fonts --ignore-system-f' | string split -f 1 \t | string match -- --ignore-system-fonts)
set -l ignore_embedded (complete -C 'typst fonts --ignore-embedded-f' | string split -f 1 \t | string match -- --ignore-embedded-fonts)
for selected in fonts font_path ignore_system ignore_embedded
    test (count $$selected) -eq 1; or exit 84
end
command typst $fonts $ignore_system $ignore_embedded $font_path {directory}
"#, directory = quote(directory.0.to_str().unwrap()));

    // Neither embedded nor installed fonts may hide a broken custom search
    // path. The same real consumer first sees an empty, isolated directory.
    let baseline = evaluate(f, &typst, &consumer);
    success(&baseline);
    assert!(baseline.out.is_empty() && baseline.err.is_empty(), "isolated font baseline must contain no available families");

    // The provisioned fixture is a byte-identical, read-only copy of the
    // host's Segoe UI face; only this private copy is ever added/removed.
    let copied = directory.0.join("owned font 雪 🐟.ttf");
    fs::copy(&font, &copied).unwrap();
    let copied = CopiedFont(copied);
    let mut permissions = fs::metadata(&copied.0).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&copied.0, permissions).unwrap();
    let discovered = evaluate(f, &typst, &consumer);
    success(&discovered);
    assert!(discovered.err.is_empty());
    assert_eq!(String::from_utf8(discovered.out).unwrap().lines().collect::<Vec<_>>(), ["Segoe UI"],
        "completion-selected custom path must make the genuine copied font available");
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32) -> *mut c_void;
    fn FindNextChangeNotification(handle: *mut c_void) -> i32;
    fn FindCloseChangeNotification(handle: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32;
    fn WaitForMultipleObjects(count: u32, handles: *const *mut c_void, all: i32, timeout: u32) -> u32;
}

struct Changes(*mut c_void);
impl Changes {
    fn new(directory: &Path) -> Self {
        let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
        // FILE_NOTIFY_CHANGE_FILE_NAME | SIZE | LAST_WRITE. Subscribe before
        // launch so a quick first compilation cannot precede observation.
        let handle = unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, 1 | 8 | 16) };
        assert!(!handle.is_null() && handle as isize != -1, "observe only the owned Typst directory: {}", std::io::Error::last_os_error());
        Self(handle)
    }
}
impl Drop for Changes {
    fn drop(&mut self) {
        assert_ne!(unsafe { FindCloseChangeNotification(self.0) }, 0, "close the owned change notification");
    }
}

struct Watcher(Child);
impl Drop for Watcher {
    fn drop(&mut self) {
        // A failed assertion must not leave a native watcher or its open output
        // behind. No process-name lookup, process-tree kill, or shell child.
        if self.0.try_wait().expect("observe only the owned Typst watcher").is_none() {
            let _ = self.0.kill();
        }
        assert_eq!(unsafe { WaitForSingleObject(self.0.as_raw_handle(), 10_000) }, 0, "owned Typst watcher cleanup must finish");
        self.0.wait().expect("collect only the owned Typst watcher");
    }
}

fn await_image(f: &Fixture, magick: &Path, watcher: &mut Watcher, changes: &Changes, output: &Path, pixel: [u8; 3], deadline: Instant) {
    loop {
        // Rearm before examining the artifact: a write during decoding remains
        // signalled instead of being lost between the check and the wait.
        assert_ne!(unsafe { FindNextChangeNotification(changes.0) }, 0, "rearm owned directory observation");
        assert!(watcher.0.try_wait().unwrap().is_none(), "owned Typst watcher exited before rendering");
        assert!(Instant::now() < deadline, "bounded Typst decoded-image readiness expired");
        if output.is_file() {
            // A write notification can precede completion of the PNG. Only a
            // successfully decoded, fully expected image establishes readiness.
            let decoded = native(f, magick, &[output.as_os_str(), OsStr::new("-depth"), OsStr::new("8"), OsStr::new("rgb:-")]);
            if decoded.code == 0 && decoded.err.is_empty() && decoded.out.len() == 72 * 36 * 3
                && decoded.out.chunks_exact(3).all(|actual| actual == pixel) {
                let identified = native(f, magick, &[OsStr::new("identify"), OsStr::new("-format"), OsStr::new("%m %w %h"), output.as_os_str()]);
                success(&identified);
                assert!(identified.err.is_empty());
                assert_eq!(identified.out, b"PNG 72 36", "selected watch format and PPI must control the decoded image");
                return;
            }
        }
        let remaining = deadline.checked_duration_since(Instant::now()).expect("bounded Typst decoded-image readiness expired");
        let handles = [changes.0, watcher.0.as_raw_handle()];
        assert_eq!(unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, remaining.as_millis().max(1) as u32) }, 0,
            "watch must publish another owned file change before exit or readiness deadline");
    }
}

fn watch(f: &Fixture) {
    let typst = setup(f);
    let magick = required("FISH_N9_MAGICK");
    let directory = OwnedDirectory(f.root.join("watch outputs 雪 🐟"));
    fs::create_dir(&directory.0).expect("create a new owned watch directory, never reuse one");
    let source = directory.0.join("watched source 雪 🐟.typ");
    let output = directory.0.join("watched output 雪 🐟.image");
    fs::write(&source, DOCUMENT).unwrap();
    let selected = evaluate(f, &typst, r#"
set -l watch (complete -C 'typst wat' | string split -f 1 \t | string match -- watch)
set -l format_option (complete -C 'typst watch --for' | string split -f 1 \t | string match -- --format)
set -l png (complete -C 'typst watch --format p' | string split -f 1 \t | string match -- png)
set -l ppi (complete -C 'typst watch --pp' | string split -f 1 \t | string match -- --ppi)
set -l pages (complete -C 'typst watch --pag' | string split -f 1 \t | string match -- --pages)
for selected in watch format_option png ppi pages
    test (count $$selected) -eq 1; or exit 85
end
printf '%s\n' $watch $format_option $png $ppi 72 $pages 2
"#);
    success(&selected);
    assert!(selected.err.is_empty());
    let arguments = String::from_utf8(selected.out).unwrap();
    let changes = Changes::new(&directory.0);
    let mut command = native_command(f, &typst);
    // Launch the actual selected consumer directly so the retained Child is
    // precisely the native watcher that cleanup terminates and waits for.
    command.args(arguments.lines()).arg(&source).arg(&output)
        .stdin(Stdio::null()).stdout(Stdio::null())
        .stderr(fs::File::create(directory.0.join("watch stderr.txt")).unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut watcher = Watcher(command.spawn().expect("start only the owned genuine Typst watcher"));
    await_image(f, &magick, &mut watcher, &changes, &output, [0, 0, 255], deadline);
    fs::write(&source, DOCUMENT.replace("#0000ff", "#00ff00")).unwrap();
    await_image(f, &magick, &mut watcher, &changes, &output, [0, 255, 0], deadline);
    // Reverse declaration order stops/waits the child and closes notification
    // and inherited log handles before OwnedDirectory removes every artifact.
}
