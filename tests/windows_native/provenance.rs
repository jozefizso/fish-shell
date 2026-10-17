//! Consuming evaluator/OS-boundary regressions, not conversion-only assertions.
use super::*;

pub(super) fn exercise(f: &Fixture) {
    // Cover the complete marker and byte-encoding alphabets, not a special-case sentinel.
    let reserved: String = (0xf500..0xf700)
        .chain(0xfdd0..0xfdf0)
        .map(|value| char::from_u32(value).unwrap())
        .collect();
    exact(
        f.eval(&format!("printf '%s' {}", quote(&reserved))),
        0,
        reserved.as_bytes(),
        b"",
    );
    let source_escapes: String = reserved
        .chars()
        .map(|ch| format!("\\u{:04X}", ch as u32))
        .collect();
    exact(
        f.eval(&format!("printf '%s' {source_escapes}")),
        0,
        reserved.as_bytes(),
        b"",
    );
    let script = OsStr::new("set -g saved $argv[1]; printf '%s' $saved");
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-c"),
                script,
                OsStr::new(&reserved),
            ]),
            b"",
        ),
        0,
        reserved.as_bytes(),
        b"",
    );
    let mut inherited = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("printf '%s' $FISH_ACCEPT_PROVENANCE"),
    ]);
    inherited.env("FISH_ACCEPT_PROVENANCE", &reserved);
    exact(run(&mut inherited, b""), 0, reserved.as_bytes(), b"");

    let child_script = format!(
        "set -gx FISH_ACCEPT_PROVENANCE $argv[1]; {} report $argv[1]",
        f.probe()
    );
    let expected = format!(
        "cwd={}\narg0={}\nFISH_ACCEPT_PROVENANCE={}\n",
        hex(f.root.to_str().unwrap()),
        hex(&reserved),
        hex(&reserved)
    );
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-c"),
                OsStr::new(&child_script),
                OsStr::new(&reserved),
            ]),
            b"",
        ),
        0,
        expected.as_bytes(),
        b"",
    );
    let inherited_key = format!("FISH_ACCEPT_{reserved}");
    let report = format!("{} report", f.probe());
    let mut inherited = f.command(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&report)]);
    inherited.env(&inherited_key, &reserved);
    let expected = format!(
        "cwd={}\n{inherited_key}={}\n",
        hex(f.root.to_str().unwrap()),
        hex(&reserved)
    );
    exact(run(&mut inherited, b""), 0, expected.as_bytes(), b"");

    let markers: String = (0xfdd0..0xfdf0)
        .map(|value| char::from_u32(value).unwrap())
        .collect();
    let cwd = f.root.join(format!("cwd {markers}"));
    fs::create_dir(&cwd).unwrap();
    let mut command = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("printf '%s' $PWD"),
    ]);
    command.current_dir(&cwd);
    let observed = run(&mut command, b"");
    assert_eq!(observed.code, 0);
    assert!(observed.err.is_empty());
    assert_cwd(std::str::from_utf8(&observed.out).unwrap(), &cwd);

    // Both spellings exist as genuine, distinguishable executables. Dropping any reserved
    // marker would launch the probe, whose -N invocation cannot produce this fish result.
    let intended = f.root.join(format!("identity-a{markers}b.exe"));
    let alias = f.root.join("identity-ab.exe");
    fs::copy(&f.fish, &intended).unwrap();
    fs::copy(&f.probe, &alias).unwrap();
    let invocation = " $argv[1] -N -c 'echo intended'";
    exact(
        run(
            &mut f.command(&[
                OsStr::new("-N"),
                OsStr::new("-c"),
                OsStr::new(invocation),
                intended.as_os_str(),
            ]),
            b"",
        ),
        0,
        b"intended\n",
        b"",
    );
    exact(
        f.eval(&format!(
            "{} -N -c 'echo intended'",
            quote(intended.to_str().unwrap())
        )),
        0,
        b"intended\n",
        b"",
    );
    let mut inherited = f.command(&[
        OsStr::new("-N"),
        OsStr::new("-c"),
        OsStr::new("$FISH_ACCEPT_PROVENANCE -N -c 'echo intended'"),
    ]);
    inherited.env("FISH_ACCEPT_PROVENANCE", &intended);
    exact(run(&mut inherited, b""), 0, b"intended\n", b"");

    // Arbitrary source bytes remain data, but cannot be reinterpreted as literal Unicode names.
    exact(f.eval("printf '%s' \\xFF"), 0, b"\xff", b"");
    exact(
        f.eval("set -g saved \\xFF; printf '%s' $saved"),
        0,
        b"\xff",
        b"",
    );
    exact(
        f.eval("printf '%s' '\u{f6ff}'"),
        0,
        "\u{f6ff}".as_bytes(),
        b"",
    );
    let pua_exe = f.root.join("\u{f6ff}.exe");
    fs::copy(&f.probe, &pua_exe).unwrap();
    let prefix = quote(&format!("{}/", f.root.to_str().unwrap().replace('\\', "/")));
    let marker = f.root.join("invalid-byte-must-not-launch");
    rejected(f.eval(&format!(
        "{prefix}\\xFF.exe marker {}",
        quote(marker.to_str().unwrap())
    )));
    assert!(
        !marker.exists(),
        "byte-derived name aliased a literal-PUA executable"
    );
    for script in [
        format!(
            "{} marker {} \\xFF",
            f.probe(),
            quote(marker.to_str().unwrap())
        ),
        format!(
            "set -gx FISH_ACCEPT_PROVENANCE \\xFF; {} marker {}",
            f.probe(),
            quote(marker.to_str().unwrap())
        ),
    ] {
        rejected(f.eval(&script));
        assert!(
            !marker.exists(),
            "invalid native argument/environment was passed to a child"
        );
    }
    // A genuine literal-PUA path and its Unicode escape must still be accepted.
    exact(
        f.eval(&format!(
            "{prefix}\\uF6FF.exe marker {}",
            quote(marker.to_str().unwrap())
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(&marker).unwrap(), b"launched");
    fs::remove_file(&marker).unwrap();

    let replacement_exe = f.root.join("\u{fffd}.exe");
    fs::copy(&f.probe, &replacement_exe).unwrap();
    for invalid in ["\\uD800", "\\uDFFF", "\\U0000D800", "\\U00110000"] {
        rejected(f.eval(&format!(
            "{prefix}{invalid}.exe marker {}",
            quote(marker.to_str().unwrap())
        )));
        assert!(
            !marker.exists(),
            "invalid scalar escape aliased a replacement-character executable"
        );
    }
    rejected(f.eval("printf '%s' \\uD800"));
    exact(
        f.eval(&format!(
            "{} marker {}",
            quote(replacement_exe.to_str().unwrap()),
            quote(marker.to_str().unwrap())
        )),
        0,
        b"",
        b"",
    );
    assert_eq!(fs::read(&marker).unwrap(), b"launched");
}
