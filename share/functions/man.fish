# localization: skip(uses-apropos)

if builtin -q __fish_native_fs
    function man
        # Fish's own manuals are native text resources. They do not require an
        # external Unix viewer, including in an installation without embedding.
        set -l own_args $argv
        if test "$own_args[1]" = --
            set -e own_args[1]
        end
        if test (count $own_args) -eq 2; and test "$own_args[1]" = 1
            set -e own_args[1]
        end
        if test (count $own_args) -eq 1; and not string match -q -- '-*' "$own_args[1]"
            set -l topic (__fish_canonicalize_builtin "$own_args[1]")
            if not command -sq man
                __fish_print_help "$topic"
                return $status
            end
            # Do not leak a failed own-manual lookup before delegating a
            # genuinely external topic to the explicitly installed viewer.
            if status native-help "$topic" &>/dev/null
                __fish_print_help "$topic"
                return $status
            end
        end
        if command -sq man
            command man $argv
            return $status
        end
        printf '%s\n' 'man: this request requires an installed native manual viewer; fish command manuals accept one topic or section 1 and a topic' >&2
        return 127
    end
    exit 0
end

if not command -qs man
    # see #5329 and discussion at https://github.com/fish-shell/fish-shell/commit/13e025bdb01cc4dd08463ec497a0a3495873702f
    exit
end

function man
    set -l manpath
    if __fish_tried_to_embed_manpages
        # If we have an embedded page, reuse a function that happens to do
        # the right thing.
        if not set -q argv[2] &&
                status list-files "man/man1/$(__fish_canonicalize_builtin $argv).1" &>/dev/null
            __fish_print_help $argv[1]
            return
        end
    else if set -l fish_manpath (path filter -d $__fish_man_dir)
        # Prepend fish's man directory if available.

        # Work around the "builtin" manpage that everything symlinks to,
        # by prepending our fish datadir to man. This also ensures that man gives fish's
        # man pages priority, without having to put fish's bin directories first in $PATH.

        # Preserve the existing MANPATH, and default to the system path (the empty string).
        set manpath $fish_manpath (
            if set -q MANPATH
                string join -- \n $MANPATH
            else if set -l p (command man -p 2>/dev/null)
                # NetBSD's man uses "-p" to print the path.
                # FreeBSD's man also has a "-p" option, but that requires an argument.
                # Other mans (men?) don't seem to have it.
                #
                # Unfortunately NetBSD prints things like "/usr/share/man/man1",
                # while not allowing them as $MANPATH components.
                # What it needs is just "/usr/share/man".
                #
                # So we strip the last component.
                # This leaves a few wrong directories, but that should be harmless.
                string replace -r '[^/]+$' '' $p
            else
                echo ''
            end
        )

        if test (count $argv) -eq 1
            set argv (__fish_canonicalize_builtin $argv)
        end
    end
    set -q manpath[1]
    and set -lx MANPATH $manpath

    command man $argv
end
