# localization: tier1
# This function is typically bound to Alt-L, it is used to list the contents
# of the directory under the cursor.

function __fish_list_current_token -d "List contents of token under the cursor if it is a directory, otherwise list the contents of the current directory"
    set -l val "$(commandline -t | string replace -r '^~' "$HOME")"
    set -l cmd
    if test -d $val
        set cmd ls $val
    else
        set -l dir (path dirname -- $val)
        if test $dir != . -a -d $dir
            set cmd ls $dir
        else
            set cmd ls
        end
    end
    if builtin -q __fish_native_fs
        # Native directory listing needs no ls executable or POSIX path parser.
        # Keep this action useful even when no optional integration is installed.
        set -l dir .
        set -q cmd[2]; and set dir $cmd[2]
        set -l entries $dir/* $dir/.*
        if set -q entries[1]
            __fish_echo string join \n -- (path basename -- $entries)
        else
            __fish_echo printf '%s' ''
        end
        return $status
    end
    __fish_echo $cmd
end
