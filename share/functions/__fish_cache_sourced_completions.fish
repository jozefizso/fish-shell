# localization: skip(private)
function __fish_cache_sourced_completions
    # Allow a `--name=foo` option which ends up in the filename.
    argparse -s name= -- $argv
    or return

    set -q argv[1]
    or return 1

    set -l cmd (command -s $argv[1])
    or begin
        # If we have no command, we can't get an mtime
        # and so we can't cache
        # The caller can more easily retry
        return 127
    end

    set -l cachedir (__fish_make_cache_dir completions)
    or return

    set -l name (path basename -- $argv[1])
    if set -q _flag_name
        set name $_flag_name
    end
    if string match -qr '[/\\\\:]|^\.\.?$' -- "$name"; or test -z "$name"
        printf '%s\n' '__fish_cache_sourced_completions: invalid cache name' >&2
        return 1
    end
    set -l stampfile $cachedir/$name.stamp
    set -l compfile $cachedir/$name.fish

    set -l mtime (path mtime -- $cmd)
    if builtin -q __fish_native_fs
        set -l identity "$cmd"\n"$mtime"
        set -l cached_identity
        if test -f $stampfile
            read -z cached_identity <$stampfile
        end
        if not test -f $compfile; or test "$identity" != "$cached_identity"
            set -l temporary (__fish_native_fs make-temp-file)
            or return
            $argv >$temporary
            set -l result $status
            if test $result = 0
                __fish_native_fs atomic-write $compfile <$temporary
                set result $status
            end
            __fish_native_fs remove $temporary
            if test $result != 0
                printf '%s\n' "__fish_cache_sourced_completions: generator '$argv[1]' failed ($result)" >&2
                return 2
            end
            printf '%s' "$identity" | __fish_native_fs atomic-write $stampfile
            or return
        end
        source $compfile
        return $status
    end

    set -l cmtime 0
    path is -rf -- $stampfile
    and read cmtime <$stampfile

    # If either the timestamp or the completion file don't exist,
    # or the mtime differs, we rerun.
    #
    # That means we'll rerun if the command was up- or downgraded.
    if path is -vrf -- $stampfile $compfile || test "$cmtime" -ne "$mtime" 2>/dev/null
        __fish_cache_put $compfile
        $argv >$compfile
        # If the command exited unsuccessfully, we assume it didn't work.
        or return 2
        __fish_cache_put $stampfile
        echo -- $mtime >$stampfile
    end

    if path is -rf -- $compfile
        source $compfile
        return 0
    end
    return 3
end
