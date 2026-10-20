# localization: skip(private)
function __fish_cache_put
    set -l cache_file $argv[1]
    if builtin -q __fish_native_fs
        # Native files inherit the cache directory's ACL, not a Unix UID/GID.
        if test -f $cache_file
            return 0
        end
        printf '' | __fish_native_fs atomic-write $cache_file
        return $status
    end
    touch $cache_file
    set -l dir (path dirname $cache_file)
    chown --reference=$dir $cache_file 2>/dev/null ||
        chown (
            if stat --version 2>&1 | string match -q 'BusyBox*'
                stat -c '%u:%g' $dir
            else
                stat --format '%u:%g' $dir 2>/dev/null ||
                stat -f '%u:%g' $dir
            end
        ) $cache_file
end
