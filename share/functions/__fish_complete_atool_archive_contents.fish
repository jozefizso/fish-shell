# localization: skip(private)
function __fish_complete_atool_archive_contents --description 'List archive contents'
    set -l cmd (commandline -cxp)
    set -e cmd[1]
    set -l file
    for arg in $cmd
        switch $arg
            case '-*'
            case '*'
                if test -f $arg
                    set file $arg
                    break
                end
        end
    end
    if not set -q file[1]
        return 1
    end
    if builtin -q __fish_native_fs
        set -l archive_tool (command -s 7z)
        set -q archive_tool[1]
        or begin
            printf '%s\n' 'fish: native archive member completion requires 7-Zip' >&2
            return 127
        end
        # Technical, header-free records preserve member spaces and Unicode;
        # do not parse the column-aligned Unix `als` presentation.
        command $archive_tool l -slt -ba -sccUTF-8 -- $file | string replace -rf '^Path = (.+?)\r?$' '$1'
        return $pipestatus[1]
    end
    als $file -qq | string replace -r '^\s*[0-9]+\s+[0-9-]+\s+[0-9:]+\s+(.*)$' '$1'
    return $pipestatus[1]

end
