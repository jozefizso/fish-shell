# localization: skip(private)
function __fish_cached --description "Cache the command output for a given amount of time"

    argparse --min-args 1 --max-args 1 --stop-nonopt 't/max-age=!_validate_int --min 0' 'k/cache-key=' -- $argv
    or return

    if set -q _flag_cache_key
        set -f cache_key $_flag_cache_key
    else
        set -f cache_key (string trim "$argv" | string split ' ')[1]
    end
    if not string match -q --regex '^[\w+-]+$' -- "$cache_key"
        return 1
    end

    if set -q _flag_max_age
        set -f max_age $_flag_max_age
    else
        set -f max_age -1
    end
    set -l cache_dir (__fish_make_cache_dir)
    or return
    set -l cache_file (path normalize $cache_dir/$cache_key)
    set -l cache_age (path mtime --relative $cache_file)
    if builtin -q __fish_native_fs
        # The public helper accepts fish command text. Run it in the matching
        # fish, never reinterpret it as POSIX sh or interpolate the cache path.
        set -l populate '
            function __fish_native_cache_populate
            set -l temporary (__fish_native_fs make-temp-file)
            or return
            eval "$argv[1]" >$temporary
            set -l result $status
            if test $result = 0
                __fish_native_fs atomic-write $argv[2] <$temporary
                set result $status
            end
            __fish_native_fs remove $temporary
            return $result
            end
            __fish_native_cache_populate $argv
        '
        if not test -f $cache_file
            $(status fish-path) -N -c $populate -- "$argv[1]" $cache_file
            or return
            __fish_native_fs read $cache_file
        else
            __fish_native_fs read $cache_file
            if test $max_age -lt 0; or test $cache_age -gt $max_age
                $(status fish-path) -N -c $populate -- "$argv[1]" $cache_file >/dev/null 2>&1 &
            end
        end
        return $status
    end

    set -l populate_cache (__fish_posix_shell) -c "
        {
            $argv
        } >$cache_file || rm $cache_file 2>/dev/null
    "

    if not test -f $cache_file
        __fish_cache_put $cache_file
        $populate_cache &

        if test -n "$last_pid"
            # wait for at most 1 second if supported
            command --search waitpid &>/dev/null
            and waitpid --exited --timeout 1 $last_pid
            and test -f $cache_file
            and command cat $cache_file
        end
    else
        command cat $cache_file

        if test $cache_age -gt $max_age
            __fish_cache_put $cache_file
            $populate_cache &
        end
    end
end
