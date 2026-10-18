# localization: tier1
function funcsave --description "Save the current definition of all specified functions to file"
    set -l options q/quiet h/help d/directory=
    argparse -n funcsave $options -- $argv
    or return

    if set -q _flag_help
        __fish_print_help funcsave
        return 0
    end

    set -l native_fs 0
    set -l dir_sep /
    if builtin -q __fish_native_fs
        set native_fs 1
        set dir_sep '\\'
    end

    set -l funcdir
    if set -q _flag_directory
        set funcdir $_flag_directory
    else
        set funcdir $__fish_config_dir$dir_sep'functions'
    end

    if not set -q argv[1]
        printf (_ "%s: Expected at least %d args, got only %d\n") funcsave 1 0 >&2
        return 1
    end

    if test $native_fs = 1
        builtin __fish_native_fs mkdir $funcdir
    else
        command mkdir -p $funcdir
    end
    if test $status -ne 0
        printf (_ "%s: Could not create configuration directory '%s'\n") funcsave $funcdir >&2
        return 1
    end

    set -l retval 0
    for funcname in $argv
        set -l funcpath "$funcdir$dir_sep$funcname.fish"
        if functions -q -- $funcname
            if test $native_fs = 1
                functions --no-details -- $funcname | builtin __fish_native_fs atomic-write $funcpath
            else
                functions --no-details -- $funcname >$funcpath
            end
            if test $status -ne 0
                set retval 1
            else if not set -q _flag_quiet
                printf (_ "%s: wrote %s\n") funcsave $funcpath
            end
        else if test -f $funcpath
            if test $native_fs = 1
                builtin __fish_native_fs remove $funcpath
            else
                command rm $funcpath
            end
            if test $status -ne 0
                set retval 1
            else if not set -q _flag_quiet
                printf (_ "%s: removed %s\n") funcsave $funcpath
            end
        else
            printf (_ "%s: Unknown function '%s'\n") funcsave $funcname >&2
            set retval 1
        end
    end

    return $retval
end
