# localization: skip(private)
function __fish_ps
    if builtin -q __fish_native_services
        # Native callers use the PID<TAB>image service, never POSIX ps columns/options.
        builtin __fish_native_services processes $argv
        return $status
    end
    switch (command -s ps | path resolve | path basename)
        case busybox
            command ps $argv
        case '*'
            command ps axc $argv
    end
end
