use super::{Family, fixture::{Captured, Fixture, quote, run}, functions_tools::tool_environment};
use std::{ffi::OsStr, fs, path::PathBuf, process::Command};

pub(super) const FAMILIES: &[Family] = &[
    ("native-clang-driver-autocomplete-and-real-compilation", clang),
];

fn configured(f: &Fixture, compiler: &std::path::Path, code: &str) -> Command {
    let mut command = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(code)]);
    tool_environment(&mut command, compiler);
    let sdk = std::env::var_os("FISH_NATIVE_N8_TOOL_PATH").expect("real SDK/MSVC executable paths required for native Clang link consumer");
    let mut paths = vec![compiler.parent().unwrap().to_owned()];
    paths.extend(std::env::split_paths(&sdk));
    paths.push(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32"));
    command.env("PATH", std::env::join_paths(paths).unwrap())
        .env("LIB", std::env::var_os("FISH_NATIVE_N8_LIB").expect("real SDK/MSVC library paths required for native Clang link consumer"));
    for key in ["VCToolsInstallDir", "VCINSTALLDIR", "VSINSTALLDIR", "WindowsSdkDir", "WindowsSDKVersion", "UniversalCRTSdkDir", "UCRTVersion", "INCLUDE"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
    command
}
fn evaluate(f: &Fixture, compiler: &std::path::Path, code: &str) -> Captured {
    run(&mut configured(f, compiler, code), b"")
}
fn clang(f: &Fixture) {
    let compiler = PathBuf::from(std::env::var_os("FISH_N9_CLANG").expect("required genuine native Clang compiler fixture"));
    assert!(compiler.is_absolute() && compiler.is_file(), "real absolute Clang compiler: {}", compiler.display());
    assert!(compiler.with_file_name("clang++.exe").is_file(), "matching genuine C++ driver required");
    assert!(compiler.with_file_name("lld-link.exe").is_file(), "matching genuine LLVM linker required");
    fs::write(f.root.join("unit 雪.c"), "int n9_value(void) { return 30; }\n").unwrap();
    fs::write(f.root.join("main 🐟.cpp"), "extern \"C\" int n9_value(void);\nconsteval int n9_extra() { return 7; }\nint main() { return n9_value() + n9_extra(); }\n").unwrap();
    let c = evaluate(f, &compiler, "set -l flag (complete -C 'clang -std=c1' | string split -f 1 \\t | string match -- '-std=c17'); test (count $flag) -eq 1; or exit 81; command clang $flag -c 'unit 雪.c' -o 'unit 雪.obj'");
    assert_eq!(c.code, 0, "real Clang C completion and object compilation: {}", String::from_utf8_lossy(&c.err));
    let cpp = evaluate(f, &compiler, "set -l flag (complete -C 'clang++ -std=c++2' | string split -f 1 \\t | string match -- '-std=c++20'); test (count $flag) -eq 1; or exit 82; set -l object (complete -C 'clang++ unit' | string split -f 1 \\t | string unescape | string match -- 'unit 雪.obj'); test (count $object) -eq 1; or exit 83; command clang++ $flag 'main 🐟.cpp' $object -o 'compiled 雪.exe'; or exit $status; './compiled 雪.exe'");
    assert_eq!(cpp.code, 37, "actual C++ completion, native object selection, link and executable result: {}", String::from_utf8_lossy(&cpp.err));
    // The literal executable path and its Unicode arguments use the same driver.
    let absolute = evaluate(f, &compiler, &format!("complete -C {}", quote(&format!("{} -std=c1", quote(compiler.to_str().unwrap())))));
    assert_eq!(absolute.code, 0, "absolute native Clang completion: {}", String::from_utf8_lossy(&absolute.err));
    assert!(String::from_utf8(absolute.out).unwrap().lines().any(|line| line.split('\t').next() == Some("-std=c17")));
    fs::write(f.root.join("invalid 雪.c"), "int invalid = ;\n").unwrap();
    let invalid = evaluate(f, &compiler, "command clang -fsyntax-only 'invalid 雪.c'");
    assert_ne!(invalid.code, 0, "real compiler must reject invalid C input");
    let no_match = evaluate(f, &compiler, "complete -C 'clang --n9-nonexistent-option-'");
    assert_eq!(no_match.code, 0);
    assert!(no_match.out.is_empty() && no_match.err.is_empty());
    let absent = f.eval("complete -C 'clang -std=c1'");
    assert_eq!(absent.code, 0);
    assert!(absent.out.is_empty() && absent.err.is_empty(), "absent compiler must not invoke a provider");
}
