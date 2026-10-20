# localization: skip(private)
#
# This file is explicitly sourced by the npm and yarn completions
# It is used to enumerate entries from the npm registry, interact with a locally installed
# package.json file, and more.
# Other completions that would depend on this include jspm, pnpm, etc.

# If all-the-package-names is installed, it will be used to generate npm completions.
# Install globally with `sudo npm install -g all-the-package-names`. Keep it up to date.
function __npm_helper_installed
    # This function takes the command to globally install a package as $argv[1]
    if not type -q all-the-package-names
        if status is-interactive; and not set -qg __fish_npm_pkg_info_shown
            set -l old (commandline)
            commandline -r ""
            echo \nfish: Run `$argv[1] all-the-package-names` to gain intelligent \
                package completion >&2
            commandline -f repaint
            commandline -r $old
            set -g __fish_npm_pkg_info_shown 1
        end
        return 1
    end
end

# Entire list of packages is too long to be used efficiently in a `complete` subcommand.
# Search it for matches instead.
function __npm_filtered_list_packages
    set -l token (commandline -ct)
    test -n "$token"; or return

    # npm knows its native module layout (Windows does not use prefix/lib).
    set -l roots
    if command -sq npm
        set -a roots (command npm root -g 2>/dev/null)
    end
    set -a roots $HOME/.config/yarn/global/node_modules $HOME/.npm-global/lib/node_modules /usr/local/lib/node_modules
    for root in $roots
        set -l names "$root/all-the-package-names/names.json"
        test -f "$names"; or continue
        __npm_read_json "$names" names "$token"
        return $status
    end
    __npm_helper_installed $argv[1]
end

function __npm_find_package_json
    set -l parents (__fish_parent_directories (pwd -P))
    set -l separator /
    if builtin -q __fish_native_fs
        set separator \\
    end

    for p in $parents
        set -l manifest "$(string trim --right --chars "$separator" -- "$p")$separator"package.json
        if test -f "$manifest"
            printf '%s\n' "$manifest"
            return 0
        end
    end

    return 1
end

function __npm_installed_global_packages
    command -sq npm; or return 1
    set -l node_modules (command npm root -g 2>/dev/null)
    test -n "$node_modules"; and test -d "$node_modules"; or return 1

    for path in $node_modules/*
        set -l mod (path basename -- $path)

        if string match -rq "^@" $mod
            for sub_path in $path/*
                set -l sub_mod (path basename -- $sub_path)
                echo $mod/$sub_mod
            end
        else
            echo $mod
        end
    end
end

function __npm_installed_local_packages
    set -l package_json (__npm_find_package_json)
    if not test $status -eq 0
        # no package.json in tree
        return 1
    end

    __npm_read_json "$package_json" dependencies
end

# Parse JSON with the runtime npm already requires, or Python on installations
# where Node is not on PATH. Never infer JSON structure from physical lines.
function __npm_read_json --argument-names file mode prefix
    test -f "$file"; or return 1
    if command -sq node
        command node -e '
const fs = require("fs");
try {
    const [file, mode, prefix = ""] = process.argv.slice(1);
    const data = JSON.parse(fs.readFileSync(file, "utf8"));
    const object = value => value && typeof value === "object" && !Array.isArray(value) ? value : {};
    const line = value => (typeof value === "string" ? value : JSON.stringify(value)).replace(/[\r\n\t]/g, " ");
    if (mode === "names") {
        if (Array.isArray(data)) for (const name of data) {
            if (typeof name === "string" && name.startsWith(prefix)) console.log(line(name));
        }
    } else if (mode === "scripts") {
        for (const [name, value] of Object.entries(object(object(data).scripts))) {
            console.log(line(name) + "\t" + line(value).slice(0, 20));
        }
    } else {
        for (const key of ["dependencies", "devDependencies"])
            for (const name of Object.keys(object(object(data)[key]))) console.log(line(name));
    }
} catch (_) { process.exitCode = 1; }
' -- "$file" "$mode" "$prefix" 2>/dev/null
    else if set -l python (__fish_anypython)
        $python -X utf8 -S -c '
import json, sys
try:
    with open(sys.argv[1], encoding="utf-8") as source:
        data = json.load(source)
    mode, prefix = sys.argv[2:4]
    def obj(value):
        return value if isinstance(value, dict) else {}
    def line(value):
        if not isinstance(value, str):
            value = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
        return value.replace("\r", " ").replace("\n", " ").replace("\t", " ")
    if mode == "names":
        for name in data if isinstance(data, list) else []:
            if isinstance(name, str) and name.startswith(prefix):
                print(line(name))
    elif mode == "scripts":
        for name, value in obj(obj(data).get("scripts")).items():
            print(line(name) + "\t" + line(value)[:20])
    else:
        for key in ("dependencies", "devDependencies"):
            for name in obj(obj(data).get(key)):
                print(line(name))
except (OSError, ValueError, TypeError):
    sys.exit(1)
' "$file" "$mode" "$prefix" 2>/dev/null
    else if command -sq jq
        switch "$mode"
            case names
                command jq -r --arg prefix "$prefix" '.[] | select(type == "string" and startswith($prefix))' "$file" 2>/dev/null
            case scripts
                command jq -r '(.scripts // {}) | to_entries[] | [.key, (.value | tostring | gsub("[\\r\\n\\t]"; " ") | .[0:20])] | join("\t")' "$file" 2>/dev/null
            case '*'
                command jq -r '(.dependencies // {}), (.devDependencies // {}) | keys[]' "$file" 2>/dev/null
        end
    else
        return 1
    end
end

function __npm_package_scripts
    __npm_read_json package.json scripts
end

function __fish_npm_helper
    # dummy function to allow sourcing this file
end
