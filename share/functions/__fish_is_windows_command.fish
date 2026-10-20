# localization: skip(private)
function __fish_is_windows_command --argument-names name
    builtin -q __fish_native_fs; or return 1
    set -q SystemRoot; or return 1
    set -l program (command -s -- $name); or return 1
    test "$(string lower -- (path resolve -- $program))" = "$(string lower -- (path resolve -- "$SystemRoot/System32/$name.exe"))"
end
