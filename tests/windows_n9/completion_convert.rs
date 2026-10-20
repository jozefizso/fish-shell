use super::{Family, fixture::{Captured, Fixture, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, fs, path::{Path, PathBuf}};

pub(super) const FAMILIES: &[Family] = &[
    ("real-native-legacy-convert-completion-pixels-format-and-absence", convert),
];

// The version-matched first-party dispatcher selects ConvertImageCommand from
// argv[0]'s basename, including on Windows; this is not a `magick convert` test.
// https://github.com/ImageMagick/ImageMagick/blob/7.1.2-34/utilities/magick.c

fn evaluate(f: &Fixture, magick: &Path, code: &str) -> Captured {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, magick);
    run(&mut command, b"")
}

fn legacy_success(result: &Captured) {
    assert_eq!(result.code, 0, "genuine legacy Convert: {}", String::from_utf8_lossy(&result.err));
}

fn convert(f: &Fixture) {
    let magick = PathBuf::from(std::env::var_os("FISH_N9_MAGICK").expect("required genuine native ImageMagick fixture"));
    assert!(magick.is_absolute() && magick.is_file(), "real absolute ImageMagick fixture: {}", magick.display());
    let legacy = magick.with_file_name("convert.exe");
    assert!(legacy.is_file(), "required byte-identical first-party multicall alias beside FISH_N9_MAGICK: {}", legacy.display());
    let system32 = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    let filesystem_convert = system32.join("convert.exe");
    assert!(filesystem_convert.is_file(), "required Windows filesystem converter collision");
    assert_ne!(fs::canonicalize(&legacy).unwrap(), fs::canonicalize(&filesystem_convert).unwrap(), "System32 convert.exe is not ImageMagick");

    let identity = evaluate(f, &magick, "command -s convert");
    assert_eq!(identity.code, 0);
    assert!(identity.err.is_empty());
    let resolved = PathBuf::from(String::from_utf8(identity.out).unwrap().trim());
    assert_eq!(fs::canonicalize(resolved).unwrap(), fs::canonicalize(&legacy).unwrap(), "ImageMagick must win over the System32 basename collision");

    let image = evaluate(f, &magick, r#"
set -l format (__fish_complete_magick_options Format | string split -f 1 \t | string match -- PNG)
test (count $format) -eq 1; or exit 81
set -l color (complete -C 'convert -fill re' | string split -f 1 \t | string unescape | string match -- red)
test (count $color) -eq 1; or exit 82
set -l negate (complete -C 'convert -neg' | string split -f 1 \t | string match -- -negate)
test (count $negate) -eq 1; or exit 83
command convert -size 2x3 xc:black -fill $color -colorize 100 $negate $format':legacy 雪.image'
"#);
    legacy_success(&image);
    assert!(image.out.is_empty());
    let png = fs::read(f.root.join("legacy 雪.image")).unwrap();
    assert!(png.len() >= 24);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "selected format must control encoding despite the unrelated extension");
    assert_eq!(&png[12..16], b"IHDR");
    assert_eq!(&png[16..24], &[0, 0, 0, 2, 0, 0, 0, 3]);
    let decoded = evaluate(f, &magick, "command magick identify -format '%m %w %h' 'legacy 雪.image'; or exit; command magick 'legacy 雪.image' -depth 8 rgb:legacy.raw");
    assert_eq!(decoded.code, 0, "decode actual legacy output: {}", String::from_utf8_lossy(&decoded.err));
    assert!(decoded.err.is_empty());
    assert_eq!(decoded.out, b"PNG 2 3");
    assert_eq!(fs::read(f.root.join("legacy.raw")).unwrap(), [0, 255, 255].repeat(6), "selected red fill followed by selected -negate must produce six cyan pixels");

    // Missing commands do not autoload their metadata. Load the real shipped
    // table explicitly so this exercises its unavailable dynamic producer.
    let absent = f.eval("status get-file completions/convert.fish | source; complete -C 'convert -fill re'");
    assert_eq!(absent.code, 0);
    assert!(absent.out.is_empty() && absent.err.is_empty(), "missing ImageMagick dynamic completion: stdout={:?}, stderr={:?}", String::from_utf8_lossy(&absent.out), String::from_utf8_lossy(&absent.err));
    let provider = f.eval("__fish_complete_magick_options Format");
    assert_eq!(provider.code, 127);
    assert!(provider.out.is_empty());
    let missing = f.eval("command convert -version");
    assert_eq!(missing.code, 127);
    assert!(missing.out.is_empty());

    // Search for, but never execute, the filesystem conversion utility. Merely
    // finding that basename must not satisfy the missing ImageMagick provider.
    let mut collision = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new("command -s convert")]);
    collision.env("PATH", &system32);
    let collision = run(&mut collision, b"");
    assert_eq!(collision.code, 0);
    assert!(collision.err.is_empty());
    let resolved = PathBuf::from(String::from_utf8(collision.out).unwrap().trim());
    assert_eq!(fs::canonicalize(resolved).unwrap(), fs::canonicalize(&filesystem_convert).unwrap());
    // Keep the unrelated System32 role excluded even when the genuine Magick
    // enum producer is available later on PATH. Never execute System32 convert.
    for path in [
        std::env::join_paths([system32.as_path()]).unwrap(),
        std::env::join_paths([system32.as_path(), magick.parent().unwrap()]).unwrap(),
    ] {
        let mut unavailable = f.command(&[OsStr::new("-N"), OsStr::new("-c"),
            OsStr::new("complete -C 'convert -fil'; complete -C 'convert -fill re'")]);
        unavailable.env("PATH", path);
        let unavailable = run(&mut unavailable, b"");
        assert_eq!(unavailable.code, 0);
        assert!(unavailable.err.is_empty(), "System32 metadata lookup: {}", String::from_utf8_lossy(&unavailable.err));
        assert!(!String::from_utf8(unavailable.out).unwrap().lines()
            .any(|line| matches!(line.split('\t').next(), Some("-fill" | "red"))),
            "System32 convert.exe must not acquire ImageMagick options or colors");
    }
}
