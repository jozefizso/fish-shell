# localization: skip(private)
function __fish_backup_config_files
    for relative_filename in $argv
        set -l filename $__fish_config_dir/$relative_filename
        if builtin -q __fish_native_fs
            set filename $__fish_config_dir'\\'(string replace -a / '\\' -- $relative_filename)
        end
        if path is $filename
            if builtin -q __fish_native_fs
                __fish_native_fs read $filename |
                    __fish_native_fs atomic-write $filename.bak
            else
                cp $filename{,.bak}
            end
            or return
        end
    end
end
