# localization: skip(private)
function __fish_posix_shell
    if builtin -q __fish_native_fs
        # A native optional shell must be an actual discoverable executable;
        # /bin/sh is not a Windows fallback and must not imply MSYS availability.
        command -s sh
        and return 0
        printf '%s\n' 'fish: this operation requires an installed native POSIX sh executable' >&2
        return 127
    end
    # NOTE: this is currently duplicated with PATH_BSHELL
    if status build-info | string match -rq '^Target( \(and host\))?: .*-android(eabi)?$'
        echo /system/bin/sh
    else
        echo /bin/sh
    end
end
