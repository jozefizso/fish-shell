function fish_update_completions --description "Update man-page based completions"
    set -l detach false
    if test "$fish_update_completions_detach" = true
        set detach true
    end
    set -l python (__fish_anypython)
    or begin
        printf "%s\n" (_ "Python 3.8 or newer executable not found") >&2
        return 1
    end

    set -l update_argv \
        $python \
        # Don't write .pyc files.
        -B \
        - \
        # Use the manpath
        --manpath \
        --directory=$__fish_cache_dir/generated_completions \
        # Clean up old completions
        --cleanup-in $__fish_user_data_dir/generated_completions

    if builtin -q __fish_native_fs
        if $detach
            # Disowned work must not retain a shell-hosted stdin bridge. Publish
            # the embedded program privately, then transfer its lifetime to the
            # real Python child. The child removes it even on ordinary cancel.
            set -l script (__fish_native_fs make-temp-file)
            or return
            status get-file tools/create_manpage_completions.py >$script
            or begin
                __fish_native_fs remove $script
                return 1
            end
            set -l wrapper 'import os, runpy, sys
script = sys.argv.pop(1)
sys.argv[0] = script
try:
    runpy.run_path(script, run_name="__main__")
finally:
    os.unlink(script)'
            $python -B -c $wrapper $script --manpath \
                --directory=$__fish_cache_dir/generated_completions \
                --cleanup-in $__fish_user_data_dir/generated_completions \
                $argv </dev/null >/dev/null 2>&1 &
            set -l launch_status $status
            set -l child $last_pid
            if test $launch_status != 0; or test -z "$child"
                __fish_native_fs remove $script
                return 1
            end
            disown $child
            or return
            __fish_native_fs transfer-temp-file $script
            return $status
        end
        status get-file tools/create_manpage_completions.py |
            $update_argv --progress $argv
        return $pipestatus[-1]
    end
    status get-file tools/create_manpage_completions.py |
        if $detach
            # Run python directly in the background and swallow all output
            # Orphan the job so that it continues to run in case of an early exit (#6269)
            # Note that some distros split the manpage completion script out (#7183).
            # In that case, we silence Python's failure.
            $(__fish_posix_shell) -c '
                c=$(cat)
                ( printf %s "$c" | "$@" ) >/dev/null 2>&1 &
            ' -- $update_argv $argv
        else
            # Display progress
            $update_argv --progress $argv
        end
end
