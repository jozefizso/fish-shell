# localization: skip(private)
function __fish_data_with_directory
    set -l relative_directory $argv[1]
    set -l file_regex $argv[2]
    set -l cmd $argv[3..]
    set -l temp
    set -l directory_ref
    set -l saved_status 0
    if test $relative_directory = man/man1 && not __fish_tried_to_embed_manpages
        set directory_ref $__fish_man_dir/man1
    else
        set temp (__fish_mktemp_relative -d fish-data)
        or return
        if set -l paths (status list-files $relative_directory |
            string match -r \
                "^$(string escape --style=regex $relative_directory)/(?:$file_regex)\$")
            for directory in (string join \n $paths | path dirname | path sort -u)
                if builtin -q __fish_native_fs
                    __fish_native_fs mkdir $temp/$directory
                else
                    mkdir -p -- $temp/$directory
                end
                or begin
                    set saved_status $status
                    break
                end
            end
            if test $saved_status = 0
                for path in $paths
                    status get-file $path >$temp/$path
                    or begin
                        set saved_status $status
                        break
                    end
                end
            end
        end
        set directory_ref $temp/$relative_directory
    end
    if test $saved_status = 0
        $cmd $directory_ref
        set saved_status $status
    end
    if set -q temp[1]
        if builtin -q __fish_native_fs
            __fish_native_fs remove-temp-directory $temp
        else
            command rm -r $temp
        end
        set -l cleanup_status $status
        if test $saved_status = 0
            set saved_status $cleanup_status
        end
    end
    return $saved_status
end
