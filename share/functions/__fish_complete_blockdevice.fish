# localization: skip(private)
#
# Helper function for completions that need to enumerate block devices.
function __fish_complete_blockdevice
    set -l cmd (commandline -ct)
    test -z "$cmd"
    and set cmd /dev/
    for f in $cmd*
        # Native Windows has no POSIX block-device predicate.
        if not builtin -q __fish_native_fs
            test -b $f
            and printf "%s\t%s\n" $f "Block device"
        end
        test -d $f
        and printf "%s\n" $f/
    end
end
