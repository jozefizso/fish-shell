# localization: skip(private)
function __fish_uname
    if builtin -q __fish_native_fs
        echo Windows
        return 0
    end

    if not set -q __fish_uname
        set -g __fish_uname (uname)
    end
    echo -- $__fish_uname
end
