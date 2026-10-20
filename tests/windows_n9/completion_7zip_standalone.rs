use super::{Family, fixture::{Captured, Fixture, exact, quote, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, fs, path::{Path, PathBuf}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-7za-completion-zip-lifecycle-corrupt-and-absence", seven_za),
    ("real-native-7zr-completion-7z-lifecycle-reduced-format-and-absence", seven_zr),
];

fn evaluate(f: &Fixture, tool: &Path, directory: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    command.current_dir(directory);
    tool_environment(&mut command, tool);
    run(&mut command, b"")
}

fn selected(variable: &str, context: &str, candidate: &str) -> String {
    format!("set -l offered (complete -C {} | string split -f 1 \\t)\nset -l {variable} (string match -- {} $offered)\nif test (count ${variable}) -ne 1\n    printf '%s\\n' {} {} $offered >&2\n    exit 81\nend\n", quote(context), quote(candidate), quote(context), quote(candidate))
}

// Resolve the public constructor's own completion metadata on every invocation.
// Only the selected command and switches enter the genuine executable; values
// and owned Unicode paths are deliberately not fabricated completion providers.
fn consume(f: &Fixture, tool: &Path, directory: &Path, alias: &str, operation: &str, options: &[(&str, &str)], arguments: &str) -> Captured {
    let mut code = selected("operation", &format!("{alias} {operation}"), operation);
    let mut invocation = format!("command {alias} $operation");
    for (index, (option, value)) in options.iter().enumerate() {
        let variable = format!("option_{index}");
        code.push_str(&selected(&variable, &format!("{alias} {operation} {option}"), option));
        invocation.push_str(&format!(" ${variable}"));
        if !value.is_empty() {
            invocation.push_str(&quote(value));
        }
    }
    code.push_str(&invocation);
    code.push(' ');
    code.push_str(arguments);
    evaluate(f, tool, directory, &code)
}

fn success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine standalone 7-Zip consumer: stdout={} stderr={}", String::from_utf8_lossy(&result.out), String::from_utf8_lossy(&result.err));
}

fn seven_za(f: &Fixture) {
    lifecycle(f, "FISH_N9_7ZA", "7za", "zip", "rar");
}

fn seven_zr(f: &Fixture) {
    // Publisher Alone7z/makefile links 7z, Xz, Lzma and Split handlers, not ZIP.
    // This is the publisher's actual I386 7zr.exe, not an alias of AMD64 7z.exe.
    lifecycle(f, "FISH_N9_7ZR", "7zr", "7z", "zip");
}

fn lifecycle(f: &Fixture, environment: &str, alias: &str, format: &str, unsupported_format: &str) {
    let tool = PathBuf::from(std::env::var_os(environment).unwrap_or_else(|| panic!("required genuine native {alias} fixture: {environment}")));
    assert!(tool.is_absolute() && tool.is_file(), "required genuine absolute {alias} fixture: {}", tool.display());
    assert_eq!(tool.file_name().unwrap(), OsStr::new(&format!("{alias}.exe")), "use the publisher's shipped basename without renaming");
    let directory = f.root.join(format!("{alias} constructor 雪 🐟"));
    fs::create_dir(&directory).unwrap();
    let resolved = evaluate(f, &tool, &directory, &format!("command -s {alias}"));
    success(&resolved);
    assert!(resolved.err.is_empty());
    assert_eq!(fs::canonicalize(PathBuf::from(String::from_utf8(resolved.out).unwrap().trim())).unwrap(), fs::canonicalize(&tool).unwrap());

    let member = "payload 雪 🐟.bin";
    let renamed = "renamed member café 雪.bin";
    let archive = format!("owned archive 雪 🐟.{format}");
    let original = b"owned native archive payload \0\xff\xfe\r\nUnicode path roundtrip\n";
    fs::write(directory.join(member), original).unwrap();
    let created = consume(f, &tool, &directory, alias, "a", &[("-t", format), ("-m", "x=0"), ("-bd", "")], &format!("{} {}", quote(&archive), quote(member)));
    success(&created);
    let archive_bytes = fs::read(directory.join(&archive)).unwrap();
    assert!(archive_bytes.starts_with(if format == "zip" { b"PK\x03\x04" } else { b"7z\xbc\xaf\x27\x1c" }), "selected -t must control the actual archive format");
    success(&consume(f, &tool, &directory, alias, "t", &[("-bd", "")], &quote(&archive)));
    let listed = consume(f, &tool, &directory, alias, "l", &[("-slt", ""), ("-scc", "UTF-8")], &quote(&archive));
    success(&listed);
    assert!(String::from_utf8(listed.out).unwrap().lines().any(|line| line.trim_end() == format!("Path = {member}")));

    let extraction = "extracted paths 雪 🐟";
    success(&consume(f, &tool, &directory, alias, "x", &[("-o", extraction), ("-y", ""), ("-bd", "")], &quote(&archive)));
    assert_eq!(fs::read(directory.join(extraction).join(member)).unwrap(), original);
    let flattened = "flat extraction café 雪";
    success(&consume(f, &tool, &directory, alias, "e", &[("-o", flattened), ("-y", ""), ("-bd", "")], &quote(&archive)));
    assert_eq!(fs::read(directory.join(flattened).join(member)).unwrap(), original);

    // Change the length as well as bytes, so update does not depend on timestamp
    // granularity in ZIP or on two writes landing in different clock ticks.
    let updated = "updated archive payload 雪 🐟\r\n\0a deliberately different length and byte stream\n".as_bytes();
    fs::write(directory.join(member), updated).unwrap();
    success(&consume(f, &tool, &directory, alias, "u", &[("-bd", "")], &format!("{} {}", quote(&archive), quote(member))));
    let readback = consume(f, &tool, &directory, alias, "e", &[("-so", "")], &format!("{} {}", quote(&archive), quote(member)));
    success(&readback);
    assert_eq!(readback.out, updated);
    success(&consume(f, &tool, &directory, alias, "rn", &[("-bd", "")], &format!("{} {} {}", quote(&archive), quote(member), quote(renamed))));
    let renamed_readback = consume(f, &tool, &directory, alias, "e", &[("-so", "")], &format!("{} {}", quote(&archive), quote(renamed)));
    success(&renamed_readback);
    assert_eq!(renamed_readback.out, updated);
    let renamed_listing = consume(f, &tool, &directory, alias, "l", &[("-slt", ""), ("-scc", "UTF-8")], &quote(&archive));
    success(&renamed_listing);
    let renamed_listing = String::from_utf8(renamed_listing.out).unwrap();
    assert!(renamed_listing.lines().any(|line| line.trim_end() == format!("Path = {renamed}")));
    assert!(!renamed_listing.lines().any(|line| line.trim_end() == format!("Path = {member}")));
    success(&consume(f, &tool, &directory, alias, "d", &[("-bd", "")], &format!("{} {}", quote(&archive), quote(renamed))));
    success(&consume(f, &tool, &directory, alias, "t", &[("-bd", "")], &quote(&archive)));
    let deleted_listing = consume(f, &tool, &directory, alias, "l", &[("-slt", ""), ("-scc", "UTF-8")], &quote(&archive));
    success(&deleted_listing);
    assert!(!String::from_utf8(deleted_listing.out).unwrap().lines().any(|line| line.trim_end() == format!("Path = {renamed}")));

    let corrupt = "corrupt archive 雪.7z";
    fs::write(directory.join(corrupt), b"not a genuine archive\0\xff").unwrap();
    let rejected = consume(f, &tool, &directory, alias, "t", &[("-bd", "")], &quote(corrupt));
    assert_ne!(rejected.code, 0, "corrupt archive must not be provider success");
    let invalid_parser = consume(f, &tool, &directory, alias, "t", &[], &format!("-not-a-7zip-switch {}", quote(&archive)));
    assert_eq!(invalid_parser.code, 7, "invalid native parser status: {}", String::from_utf8_lossy(&invalid_parser.err));

    // -t is generic metadata, not a promise that every format of full 7z is
    // linked into these reduced executables. Keep the unsupported error visible.
    let unsupported_archive = format!("unsupported format 雪.{unsupported_format}");
    let unsupported = consume(f, &tool, &directory, alias, "a", &[("-t", unsupported_format)], &format!("{} {}", quote(&unsupported_archive), quote(member)));
    assert_ne!(unsupported.code, 0, "reduced-format constructor must reject unsupported type");
    assert!(!directory.join(unsupported_archive).exists());

    // Missing root commands keep normal autoload guarded. A failed real
    // invocation must not be mistaken for a successful static table.
    let absent_archive = format!("absent {alias} archive 雪.7z");
    let absent_code = format!("command {alias} a {} {}", quote(&absent_archive), quote(member));
    let absent = f.eval(&absent_code);
    assert_eq!(absent.code, 127, "an absent executable is not static-completion success");
    assert!(absent.out.is_empty());
    assert!(!absent.err.is_empty());
    assert!(!f.root.join(absent_archive).exists());
}
