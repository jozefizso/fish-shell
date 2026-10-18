function fish_print_hg_root
    # If hg isn't installed, there's nothing we can do
    if not command -sq hg
        return 1
    end

    # Find an hg directory above $PWD
    # without calling `hg root` because that's too slow
    set -l root
    set -l dir "$(pwd -P 2>/dev/null)"
    or return 1
    set -l separator /
    if builtin -q __fish_native_fs
        set separator '\\'
    end

    while not contains -- "$dir" "" / .
        if test -f $dir$separator'.hg'$separator'dirstate'
            echo $dir$separator'.hg'
            return 0
        end
        # Go up one directory
        set -l parent (path dirname -- $dir)
        # Drive and UNC roots are their own parent on Windows.
        test "$parent" = "$dir"; and break
        set dir $parent
    end

    return 1
end
