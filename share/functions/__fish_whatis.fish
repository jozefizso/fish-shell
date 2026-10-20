# localization: skip(private)

# uses `whatis` if available to describe a command

function __fish_whatis
    set -l cmd $argv[1]
    set -l fallback
    if set -q argv[2]
        set fallback $argv[2]
    end

    set -l description
    if builtin -q __fish_native_fs
        set -l query (string escape --style=regex -- $cmd)
        set description (__fish_apropos "^$query\$" 2>/dev/null | string replace -r '.*? - ' '')[1]
    else
        set description (whatis $cmd 2>/dev/null | string replace -r '.*? - ' '')[1]
    end
    if string match -qr -- "." "$description"
        printf '%s\n' $description
        return 0
    else if not string match -q -- "$fallback" ""
        printf '%s\n' $fallback
        return 0
    else
        return 1
    end
end
