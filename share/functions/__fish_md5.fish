# localization: skip(private)
function __fish_md5
    if builtin -q __fish_native_fs
        switch (count $argv)
            case 0
                __fish_native_fs md5
            case 1
                if test "$argv[1]" = -
                    __fish_native_fs md5
                else
                    __fish_native_fs md5-file $argv[1]
                end
            case 2
                if test "$argv[1]" != -s
                    printf '%s\n' 'fish_md5: expected a file, stdin, or -s STRING' >&2
                    return 2
                end
                __fish_native_fs md5-string $argv[2]
            case '*'
                printf '%s\n' 'fish_md5: expected a file, stdin, or -s STRING' >&2
                return 2
        end
        return $status
    end
    if type -q md5sum
        # GNU systems
        if set -q argv[2]
            if test $argv[1] = -s
                echo $argv[2] | md5sum | string split ' ' -f 1
            else
                printf "%s: Too many arguments %s\n" fish_md5 $argv >&2
            end
        else
            md5sum $argv[1] | string split ' ' -f 1
        end
        return 0
    else if type -q md5
        # BSD systems
        if set -q argv[2]
            if test $argv[1] = -s
                md5 -s $argv[2]
            else
                printf "%s: Too many arguments %s\n" fish_md5 $argv >&2
            end
        else
            md5 -q $argv[1]
        end
        return 0
    end
    return 1
end
