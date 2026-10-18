use super::fixture::{Captured, Fixture, exact, run};
use std::{ffi::OsStr, fs};

fn history(f: &Fixture, code: &str) -> Captured {
    run(
        f.command(&[OsStr::new("-c"), OsStr::new(code)])
            .env("fish_history", "n4_locale"),
        b"",
    )
}

fn successful(result: Captured) -> String {
    assert_eq!(result.code, 0, "{}", String::from_utf8_lossy(&result.err));
    assert!(
        result.err.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.err)
    );
    String::from_utf8(result.out).unwrap()
}

pub(super) fn exercise(f: &Fixture) {
    // These are actual builtin printf conversions, not locale-state inspection.
    exact(
        f.eval("set -gx LANG en-US; set -gx LC_NUMERIC de-DE; printf '%.2f\\n' 1234.5; set -gx LC_ALL C; printf '%.2f\\n' 1234.5; set -e LC_ALL; printf '%.2f\\n' 1234.5; set -e LC_NUMERIC; printf '%.2f\\n' 1234.5; set -gx LC_NUMERIC de_DE.UTF-8; printf \"%'0.2f\\n\" 1234.5"),
        0,
        b"1234,50\n1234.50\n1234,50\n1234.50\n1.234,50\n",
        b"",
    );
    exact(
        f.eval("set -gx LC_ALL C; printf '%.2f\\n' 1.5; set -gx LC_ALL fr-FR; printf '%.2f\\n' 1.5; set -gx LC_ALL C; printf '%.2f\\n' 1.5"),
        0,
        b"1.50\n1,50\n1.50\n",
        b"",
    );
    // Remove the fixture's explicit C locale to exercise captured OS defaults.
    // Scalar and empty-list category values must restore that actual baseline.
    exact(
        f.eval("set -e LANG; set -l baseline (printf '%.2f' 1234.5); set -gx LC_NUMERIC de-DE; printf '%.2f\\n' 1234.5; set -e LC_NUMERIC; test (printf '%.2f' 1234.5) = $baseline; or exit 81; set -gx LC_NUMERIC en-US; set -gx LC_NUMERIC ''; test (printf '%.2f' 1234.5) = $baseline; or exit 82; set -gx LC_NUMERIC de-DE; set -gx LC_NUMERIC; test (printf '%.2f' 1234.5) = $baseline; or exit 83"),
        0,
        b"1234,50\n",
        b"",
    );
    // Local settings are not exported locale inputs. The numeric parser still
    // accepts fish's invariant dot grammar while output uses comma punctuation.
    exact(
        f.eval("set -gx LANG en-US; set -l LC_NUMERIC de-DE; printf '%.2f\\n' 1.5; set -e LC_NUMERIC; set -gx LC_NUMERIC de-DE; printf '%.2f\\n' (math '1.5 + 1')"),
        0,
        b"1.50\n2,50\n",
        b"",
    );
    // Principal-stack dispatch must cover exported local scopes, scope pop, and
    // locally changed exported universals, not just global mutations.
    exact(
        f.eval("set -gx LANG en-US; begin; set -lx LC_NUMERIC de-DE; printf '%.2f\\n' 1.5; end; printf '%.2f\\n' 1.5; set -Ux LC_NUMERIC de-DE; printf '%.2f\\n' 1.5; set -Uu LC_NUMERIC $LC_NUMERIC; printf '%.2f\\n' 1.5; set -eU LC_NUMERIC"),
        0,
        b"1,50\n1.50\n1,50\n1.50\n",
        b"",
    );
    // A real variable-event callback reads and mutates the same environment.
    // The shared process timeout bounds a lock-order/reentrant-dispatch deadlock
    // instead of hiding it behind a mock.
    exact(
        f.eval("function n4_locale_event --on-variable LC_NUMERIC; set -g n4_seen $LC_NUMERIC; set -gx LC_TIME en-US; end; set -gx LC_NUMERIC de-DE; test $n4_seen = de-DE; or exit 84; printf '%.2f\\n' 1.5; set -gx LC_NUMERIC en-US; test $n4_seen = en-US; or exit 85; printf '%.2f\\n' 1.5"),
        0,
        b"1,50\n1.50\n",
        b"",
    );
    let invalid = f.eval("set -gx LC_NUMERIC de-DE; set -gx LC_NUMERIC definitely-invalid-locale; printf '%.2f\\n' 1.5; set -e LC_NUMERIC; set -gx LC_NUMERIC C; printf '%.2f\\n' 1.5");
    assert_eq!(invalid.code, 0);
    assert_eq!(invalid.out, b"1,50\n1.50\n");
    let errors = String::from_utf8(invalid.err).unwrap();
    assert!(errors.contains("LC_NUMERIC"), "{errors}");
    assert!(errors.contains("definitely-invalid-locale"), "{errors}");

    fs::create_dir_all(f.state.join("data/fish")).unwrap();
    let file = f.state.join("data/fish/n4_locale_history");
    let bytes = b"- cmd: winter\n  when: 1579089600\n- cmd: summer\n  when: 1594814400\n- cmd: old_rule\n  when: 1142856000\n- cmd: new_rule\n  when: 1174392000\n";
    fs::write(&file, bytes).unwrap();
    let winter = "builtin history search --exact --show-time='%F %T %z|' winter";
    let summer = "builtin history search --exact --show-time='%F %T %z|' summer";
    exact(
        history(f, &format!("set -gx LC_TIME C; set -gx TZ UTC; {winter}; {summer}; set -gx TZ 'Pacific Standard Time'; {winter}; {summer}; builtin history search --exact --show-time='%F %T %z|' old_rule; builtin history search --exact --show-time='%F %T %z|' new_rule")),
        0,
        b"2020-01-15 12:00:00 +0000|winter\n2020-07-15 12:00:00 +0000|summer\n2020-01-15 04:00:00 -0800|winter\n2020-07-15 05:00:00 -0700|summer\n2006-03-20 04:00:00 -0800|old_rule\n2007-03-20 05:00:00 -0700|new_rule\n",
        b"",
    );
    // Locale changes reach UCRT's real localized month-name consumer. LC_ALL
    // overrides LC_TIME; unsetting LC_ALL exposes it again, then LANG takes over.
    let names = successful(history(
        f,
        "set -gx TZ UTC; set -gx LANG en-US; set -gx LC_TIME fr-FR; builtin history search --exact --show-time='%B|' winter; set -gx LC_ALL C; builtin history search --exact --show-time='%B|' winter; set -e LC_ALL; builtin history search --exact --show-time='%B|' winter; set -e LC_TIME; builtin history search --exact --show-time='%B|' winter",
    ));
    assert_eq!(
        names,
        "janvier|winter\nJanuary|winter\njanvier|winter\nJanuary|winter\n"
    );
    let localized = "builtin history search --exact --show-time='%c %z|' winter";
    exact(
        history(
            f,
            &format!(
                "set -e LANG; set -l baseline ({localized}); set -gx TZ UTC; set -gx LC_TIME fr-FR; {winter}; set -e TZ; set -e LC_TIME; set -l restored ({localized}); test \"$restored\" = \"$baseline\"; or exit 86"
            ),
        ),
        0,
        b"2020-01-15 12:00:00 +0000|winter\n",
        b"",
    );
    let invalid = history(
        f,
        "set -gx TZ UTC; set -gx LC_TIME fr-FR; set -gx LC_TIME definitely-invalid-locale; builtin history search --exact --show-time='%B|' winter; set -e LC_TIME; set -gx LC_TIME C; builtin history search --exact --show-time='%B|' winter",
    );
    assert_eq!(invalid.code, 0);
    assert_eq!(invalid.out, b"janvier|winter\nJanuary|winter\n");
    let errors = String::from_utf8(invalid.err).unwrap();
    assert!(errors.contains("LC_TIME"), "{errors}");
    assert!(errors.contains("definitely-invalid-locale"), "{errors}");
    let invalid = history(
        f,
        &format!(
            "set -gx TZ UTC; set -gx TZ America/Los_Angeles; {winter}; set -e TZ; set -gx TZ 'Pacific Standard Time'; {winter}"
        ),
    );
    assert_eq!(invalid.code, 0);
    assert_eq!(
        invalid.out,
        b"2020-01-15 12:00:00 +0000|winter\n2020-01-15 04:00:00 -0800|winter\n"
    );
    let errors = String::from_utf8(invalid.err).unwrap();
    assert!(errors.contains("America/Los_Angeles"), "{errors}");
    for format in ["%Q", "%", "%#c", "%Ec", "%_d"] {
        let invalid = history(
            f,
            &format!(
                "builtin history search --exact --show-time='{}' winter",
                format
            ),
        );
        assert_ne!(invalid.code, 0, "invalid format {format:?} must fail");
        assert!(
            invalid.out.is_empty(),
            "invalid format emitted a fake timestamp"
        );
        let errors = String::from_utf8(invalid.err).unwrap();
        assert!(
            !errors.is_empty(),
            "invalid timestamp format requires a diagnostic"
        );
    }
    // A formatting error must not poison future formatting or modify history.
    exact(
        history(f, &format!("set -gx LC_TIME C; set -gx TZ UTC; {winter}")),
        0,
        b"2020-01-15 12:00:00 +0000|winter\n",
        b"",
    );
    assert_eq!(fs::read(&file).unwrap(), bytes);
}
