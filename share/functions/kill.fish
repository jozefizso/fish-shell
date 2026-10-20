# localization: tier1
if builtin -q __fish_native_fs
    function kill
        printf '%s\n' 'kill: Unix signal delivery is unsupported on native Windows; use an explicit native process tool' >&2
        return 1
    end
else if command -q kill
    # Only define this if something to wrap exists
    # this allows a nice "command not found" error to be triggered.
    function kill
        set -l args (__fish_expand_pid_args $argv)
        and command kill $args
    end
end
