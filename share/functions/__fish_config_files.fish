# localization: skip(private)
# NOTE: This outputs a mix of absolute and relative paths!
function __fish_config_files
    set -l _flag_user_dir
    argparse user-dir= -- $argv
    or return
    set -l prefix $argv[1]
    set -l suffix $argv[2]
    set -e argv[1..2]
    set -l paths
    set -l separator /
    if builtin -q __fish_native_fs
        set separator '\\'
    end
    if not set -q argv[1]
        set paths $_flag_user_dir$separator*$suffix
    else
        set paths (path filter $_flag_user_dir$separator$argv$suffix)
    end
    if not set -q argv[1]
        set -a paths (status list-files $prefix)
    else
        for file in "$prefix/"$argv$suffix
            if status get-file $file >/dev/null
                set -a paths $file
            end
        end
    end
    string join \n $paths
end
