# localization: skip(private)
function __fish_print_xdg_data_directories --description 'Print XDG data directories'
    set -f data_home $XDG_DATA_HOME
    if test -z "$data_home"
        set data_home $HOME/.local/share/
    end

    set -f data_dirs $data_home
    if builtin -q __fish_native_fs
        # A colon belongs to a Windows drive root, not a directory separator.
        set -a data_dirs (string split ';' -- $XDG_DATA_DIRS)
    else
        set -l system_dirs $XDG_DATA_DIRS
        if test -z "$system_dirs"
            set system_dirs /usr/local/share/:/usr/share/
        end
        set -a data_dirs (string split : -- $system_dirs)
    end

    for path in $data_dirs
        if test -d $path
            echo $path
        end
    end
end
