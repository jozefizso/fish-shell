# localization: skip(private)
function __fish_mktemp_relative
    if builtin -q __fish_native_fs
        argparse d -- $argv
        or return
        if set -q _flag_d
            __fish_native_fs make-temp-directory
        else
            __fish_native_fs make-temp-file
        end
        return $status
    end
    # OSX mktemp is rather restricted - no suffix, no way to automatically use TMPDIR
    if not set -q TMPDIR[1]
        set -f TMPDIR /tmp
    end
    argparse -u -- $argv || return
    if not set -q argv[1]
        set argv fish
    end
    mktemp $argv_opts -- $TMPDIR/$argv.XXXXXX
end
