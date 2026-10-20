# localization: skip(private)
function __fish_anypython
    # Probe the interpreter, not its filename: python.exe may be an App
    # Execution Alias, and python3 need not satisfy the optional tools' floor.
    set -l candidates python3 python
    if builtin -q __fish_native_fs
        set -a candidates py
    end
    for candidate in $candidates
        set -l executable (command -s $candidate)
        set -q executable[1]; or continue
        if test "$executable" = /usr/bin/python3
            and command -sq xcode-select
            and not command xcode-select --print-path &>/dev/null
            continue
        end
        set -l python_args
        if test $candidate = py
            set python_args -3
        end
        if command $executable $python_args -c 'import sys; sys.exit(not ((3, 8) <= sys.version_info < (4, 0)))' &>/dev/null
            printf '%s\n' $executable $python_args
            return 0
        end
    end
    for executable in (path filter -fx -- $PATH/python3.*)
        if command $executable -c 'import sys; sys.exit(not ((3, 8) <= sys.version_info < (4, 0)))' &>/dev/null
            printf '%s\n' $executable
            return 0
        end
    end
    return 1
end
