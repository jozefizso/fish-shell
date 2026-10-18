# localization: skip(private)
function __fish_man_page
    # Get all commandline tokens not starting with "-", up to and including the cursor's
    set -l args (string match -rv '^-|^$' -- (commandline -cpx && commandline -t))

    # If commandline is empty, exit.
    if not set -q args[1]
        printf \a
        return
    end
    if builtin -q __fish_native_fs; and not type -q man
        echo "fish: manual lookup requires the optional man command" >&2
        return 127
    end

    # Skip leading commands and display the manpage of following command
    while set -q args[2]
        and string match -qr -- '^(and|begin|builtin|caffeinate|command|doas|entr|env|exec|if|mosh|nice|not|or|pipenv|please|prime-run|run0|setsid|sudo|systemd-nspawn|time|watch|while|xargs|.*=.*)$' $args[1]
        set -e args[1]
    end

    # If there are at least two tokens not starting with "-", the second one might be a subcommand.
    # Try "man first-second" and fall back to "man first" if that doesn't work out.
    set -l maincmd (path basename $args[1])
    set -l man_status 0
    # HACK: If stderr is not attached to a terminal `less` (the default pager)
    # wouldn't use the alternate screen.
    # But since we don't know what pager it is, and because `man` is totally underspecified,
    # the best we can do is to *try* the man page, and assume that `man` will return false if it fails.
    # See #7863.
    if set -q args[2]
        and not string match -q -- '*/*' $args[2]
        and man "$maincmd-$args[2]" &>/dev/null
        man "$maincmd-$args[2]"
        set man_status $status
    else
        if man "$maincmd" &>/dev/null
            man "$maincmd"
            set man_status $status
        else
            set man_status $status
            printf \a
        end
    end

    commandline -f repaint
    if builtin -q __fish_native_fs
        return $man_status
    end
end
