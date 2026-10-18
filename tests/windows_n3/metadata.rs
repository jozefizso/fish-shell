use super::fixture::{Fixture, exact, quote};
use std::fs;

pub(super) fn exercise(f: &Fixture) {
    exact(
        f.eval("abbr --add g git; abbr --query g; echo $status; abbr --list; set saved (abbr --show --color=never | string collect); abbr --erase g; abbr --query g; echo $status; eval $saved; abbr --rename g h; abbr -q g; echo $status; abbr -q h; echo $status; abbr --erase h; abbr -l"),
        0,
        b"0\ng\n1\n1\n0\n",
        b"",
    );
    exact(
        f.eval("abbr -a --position=anywhere --set-cursor=@ --description='snow token' --regex='x[0-9]+' --function=expand_token --command=tool rx; set saved (abbr -s --color=never | string collect); abbr -e -c tool rx; abbr -q rx; echo $status; eval $saved; abbr -q rx; echo $status; abbr -e -c tool rx; abbr -q rx"),
        1,
        b"1\n0\n",
        b"",
    );
    exact(
        f.eval("abbr -a -g --set-cursor --position command -- ordinary 'echo %'; abbr -a -c tool -c other scoped replacement; set saved (abbr -s --color=never | string collect); abbr -e ordinary; abbr -e -c tool -c other scoped; eval $saved; abbr -q ordinary; echo $status; abbr -e -c tool -c other scoped; abbr -q scoped; echo $status"),
        0,
        b"0\n1\n",
        b"",
    );
    exact(
        f.eval("abbr short echo -n hello; set saved (abbr -s --color=never | string collect); abbr -e short; eval $saved; abbr -q short; echo $status; abbr -e short; abbr -q short"),
        1,
        b"0\n",
        b"",
    );

    for (name, declaration, erase, filter) in [
        (
            "exclusive/short/arguments/description/condition/order",
            "complete --command=tool --short-option=x --arguments='one two' --description='pick one' --condition=true --exclusive --keep-order",
            "complete --erase --command=tool",
            "tool",
        ),
        (
            "no-files/long",
            "complete -c tool -l long -a 'red blue' --no-files",
            "complete -e -c tool",
            "tool",
        ),
        (
            "force-files/old-option",
            "complete -c tool --old-option=old --force-files -a value",
            "complete -e -c tool",
            "tool",
        ),
        (
            "require-parameter/old-short-spelling",
            "complete -c tool -o old --require-parameter -a value",
            "complete -e -c tool",
            "tool",
        ),
        (
            "path/short-flags",
            "complete --path=/virtual/tool -s z -d desc -a value -n true -x -k",
            "complete --erase --path=/virtual/tool",
            "",
        ),
        (
            "long-option/short-require",
            "complete -c tool --long-option=long -r -a value",
            "complete -e -c tool",
            "tool",
        ),
        (
            "args-only/multiple-conditions",
            "complete tool -a 'a b' -n true --condition='test 1 = 1' -f",
            "complete -e -c tool",
            "tool",
        ),
        (
            "short-force/removed-authority-compatibility",
            "complete -c tool -s q -F -A --unauthoritative --authoritative -u",
            "complete -e -c tool",
            "tool",
        ),
    ] {
        let result = f.eval(&format!("{declaration}; set saved (complete --color=never {filter} | string collect); test -n \"$saved\"; echo $status; {erase}; test -z \"$(complete --color=never {filter} | string collect)\"; echo $status; eval $saved; set restored (complete --color=never {filter} | string collect); test \"$saved\" = \"$restored\"; echo $status; {erase}; test -z \"$(complete --color=never {filter} | string collect)\"; echo $status"));
        assert_eq!(
            result.code,
            0,
            "{name}: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert_eq!(
            result.out, b"0\n0\n0\n0\n",
            "{name}: erase must remove metadata and replay must restore it"
        );
        assert!(
            result.err.is_empty(),
            "{name}: {}",
            String::from_utf8_lossy(&result.err)
        );
    }
    exact(
        f.eval("complete -c tool -s b; set survivor (complete --color=never tool | string collect); complete -c tool -s a -l long -o old; complete --erase -c tool --short-option=a --long-option=long --old-option=old; test \"$survivor\" = \"$(complete --color=never tool | string collect)\"; echo $status; complete -e -c tool -s b; test -z \"$(complete --color=never tool | string collect)\"; echo $status"),
        0,
        b"0\n0\n",
        b"",
    );
    exact(
        f.eval("complete -c first --wraps=base; complete -c second -w base; set second_saved (complete --color=never second | string collect); test -n \"$second_saved\"; echo $status; complete -e -c first -w base; test -z \"$(complete --color=never first | string collect)\"; echo $status; test \"$second_saved\" = \"$(complete --color=never second | string collect)\"; echo $status; complete -e -c second; test -z \"$(complete --color=never second | string collect)\"; echo $status; eval $second_saved; test \"$second_saved\" = \"$(complete --color=never second | string collect)\"; echo $status; complete -e -c second -w base; test -z \"$(complete --color=never second | string collect)\"; echo $status"),
        0,
        b"0\n0\n0\n0\n0\n0\n",
        b"",
    );
    // Rule identity/fields are checked inside the owning module. Here each replayed
    // option must survive erasure of its peers, then disappear when selected itself.
    // Declaration order is deliberately not compared: printing reverses rule order.
    for (others, selected) in [
        ("-o old -s z", "-s a"),
        ("-s a -s z", "-o old"),
        ("-s a -o old", "-s z"),
    ] {
        exact(
            f.eval(&format!("complete -c first -s a -a alpha; complete -c second -l bee -a beta; complete -c first -o old -a older; complete -c first -s z -a last; complete -p /virtual/tool -l path -a file; set second_saved (complete --color=never second | string collect); set path_saved (complete --color=never /virtual/tool | string collect); set saved (complete --color=never | string collect); complete -e -c first; complete -e -c second; complete -e -p /virtual/tool; test -z \"$(complete --color=never | string collect)\"; echo $status; eval $saved; complete -e -c first {others}; test -n \"$(complete --color=never first | string collect)\"; echo $status; test \"$second_saved\" = \"$(complete --color=never second | string collect)\"; echo $status; test \"$path_saved\" = \"$(complete --color=never /virtual/tool | string collect)\"; echo $status; complete -e -c first {selected}; test -z \"$(complete --color=never first | string collect)\"; echo $status; test \"$second_saved\" = \"$(complete --color=never second | string collect)\"; echo $status; test \"$path_saved\" = \"$(complete --color=never /virtual/tool | string collect)\"; echo $status; complete -e -c second; complete -e -p /virtual/tool; test -z \"$(complete --color=never | string collect)\"; echo $status")),
            0,
            b"0\n0\n0\n0\n0\n0\n0\n0\n",
            b"",
        );
    }

    let marker = f.root.join("metadata must not evaluate.txt");
    let declarations = f.root.join("declarations 雪.fish");
    let effect = format!("echo forbidden > {}", quote(marker.to_str().unwrap()));
    fs::write(
        &declarations,
        format!(
            "abbr -a saved 'echo batch'\ncomplete -c stored -s x -n {} -a {}\n",
            quote(&effect),
            quote(&format!("({effect})"))
        ),
    )
    .unwrap();
    exact(
        f.eval(&format!("source {}; abbr -q saved; echo $status; test -n \"$(complete --color=never stored | string collect)\"; echo $status", quote(declarations.to_str().unwrap()))),
        0,
        b"0\n0\n",
        b"",
    );
    assert!(
        !marker.exists(),
        "declarative completion evaluated its stored code"
    );

    for (command, status) in [
        ("abbr -a --regex='[' rx replacement", 2),
        ("abbr -a --position=bad x y", 2),
        ("abbr -a --set-cursor= x y", 2),
        ("abbr -a --position=command --command=tool x y", 2),
        ("abbr -a --function='bad name' x", 2),
        ("abbr --query --erase x", 2),
        ("abbr --rename missing present", 1),
        ("complete -c tool -s ''", 2),
        ("complete -c tool -f -F", 2),
        ("complete -c tool -x -F", 2),
        ("complete -c tool -n 'if true'", 1),
        ("complete -c tool -a '('", 1),
    ] {
        let result = f.eval(command);
        assert_eq!(
            result.code,
            status,
            "{command}: {}",
            String::from_utf8_lossy(&result.err)
        );
        assert!(result.out.is_empty(), "{command}");
        assert!(
            !result.err.is_empty(),
            "{command}: invalid declarations must diagnose their failure"
        );
    }
    exact(
        f.eval("complete -c n6_metadata -f -a \"alpha 'space value'\"; complete -C 'n6_metadata a'; complete --do-complete='n6_metadata a'; complete --escape -C 'n6_metadata space'"),
        0,
        b"alpha\nalpha\nspace\\ value\n",
        b"",
    );
    exact(f.eval("abbr -q missing"), 1, b"", b"");
    exact(f.eval("abbr -e missing"), 4, b"", b"");
    let deprecated = f.eval("abbr --global long_global replacement; abbr -U short_universal replacement; abbr --universal long_universal replacement; for name in long_global short_universal long_universal; abbr --query $name; echo $status; end");
    assert_eq!(deprecated.code, 0);
    assert_eq!(deprecated.out, b"0\n0\n0\n");
    assert!(
        !deprecated.err.is_empty(),
        "removed universal flags must warn instead of promising persistence"
    );
    exact(
        f.eval("complete --path=/virtual/tool -s a; complete --erase --path=/virtual/tool; complete --color=never"),
        0,
        b"",
        b"",
    );

    for builtin in ["abbr", "complete"] {
        let declaration = if builtin == "abbr" {
            "set -g fish_color_command red; set -g fish_color_param blue; abbr -a colored 'echo color'"
        } else {
            "set -g fish_color_command red; set -g fish_color_param blue; complete -c colored -s x -d desc"
        };
        let show = if builtin == "abbr" {
            "abbr -s"
        } else {
            "complete colored"
        };
        let plain = f.eval(&format!("{declaration}; {show} --color=never"));
        assert_eq!(plain.code, 0);
        assert!(plain.err.is_empty());
        assert!(!plain.out.contains(&0x1b));
        let automatic = f.eval(&format!("{declaration}; {show} --color=auto"));
        assert_eq!(automatic.code, 0);
        assert_eq!(automatic.out, plain.out);
        assert!(automatic.err.is_empty());
        let colored = f.eval(&format!("{declaration}; {show} --color=always"));
        assert_eq!(colored.code, 0);
        assert!(colored.err.is_empty());
        assert!(colored.out.windows(2).any(|bytes| bytes == b"\x1b["));
        let stripped = f.eval(&format!(
            "{declaration}; {show} --color=always | string replace -ra '\\e\\[[0-9;]*m' ''"
        ));
        assert_eq!(stripped.code, 0);
        assert_eq!(stripped.out, plain.out);
        assert!(stripped.err.is_empty());
    }
}
