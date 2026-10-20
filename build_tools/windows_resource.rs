//! Windows SDK version resources for the three actual executable entrypoints.
//! Included by the root build script, not compiled into the runtime.
use std::{env, fs, io::Write, path::PathBuf, process::Command};

pub fn emit(version: &str) {
    let mut cargo = std::io::stdout().lock();
    writeln!(cargo, "cargo:rerun-if-changed=build_tools/windows_resource.rs")
        .expect("write Cargo resource directive");
    writeln!(cargo, "cargo:rerun-if-env-changed=RC").expect("write Cargo resource directive");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let numeric: [u16; 3] = std::array::from_fn(|_| {
        parts.next().expect("three-part package version").parse().expect("numeric package version")
    });
    assert!(parts.next().is_none(), "three-part package version");
    // RC string literals must not reinterpret backslashes or quote delimiters.
    let version = version.replace('\\', "\\\\").replace('"', "\\\"");
    let source = format!(
        r#"1 VERSIONINFO
FILEVERSION {0},{1},{2},0
PRODUCTVERSION {0},{1},{2},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0
FILEOS 0x40004L
FILETYPE 1
FILESUBTYPE 0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "fish-shell contributors\0"
      VALUE "FileDescription", "fish native shell and tools\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "ProductName", "fish\0"
      VALUE "ProductVersion", "{version}\0"
      VALUE "LegalCopyright", "See packaged COPYING and license.rst\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        numeric[0], numeric[1], numeric[2]
    );
    let rc = out.join("fish-version.rc");
    let resource = out.join("fish-version.res");
    fs::write(&rc, source).expect("write Windows version resource");
    let status = Command::new(env::var_os("RC").unwrap_or_else(|| "rc.exe".into()))
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(&rc)
        .status()
        .expect("Windows SDK rc.exe is required for native executable metadata");
    assert!(status.success(), "Windows SDK resource compilation failed: {status}");
    for binary in ["fish", "fish_indent", "fish_key_reader"] {
        writeln!(cargo, "cargo:rustc-link-arg-bin={binary}={}", resource.display())
            .expect("write Cargo resource directive");
    }
}
