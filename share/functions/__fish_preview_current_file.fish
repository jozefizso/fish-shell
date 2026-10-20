# localization: skip(private)
if builtin -q __fish_native_fs
    function __fish_preview_current_file_text --argument-names file
        # The decoder's own redirection selects stdin without changing the
        # wrapper's argument/input semantics or consuming its setup input.
        string sub --start 1 <$file
    end
end

function __fish_preview_current_file --description "Open the file at the cursor in a pager"
    __fish_edit_command_if_at_cursor
    set -l edit_status $status
    if test $edit_status -eq 0
        return 0
    else if builtin -q __fish_native_fs; and test $edit_status -ne 1
        return $edit_status
    end

    set -l direct false
    set -l pager (__fish_anypager 2>/dev/null)
    or begin
        if builtin -q __fish_native_fs
            # The Unix fallback is cat. Native text preview uses the same shared
            # stream decoder/output path instead of requiring a coreutils tool.
            set direct true
        else
            set pager cat
        end
    end

    # commandline -t will never return an empty list. However, the token
    # could comprise multiple lines, so join them into a single string.
    set -l file (commandline -t | string collect)
    set -l prefix eval set

    if test -z $file
        # $backslash will parsed as regex which may need additional escaping.
        set -l backslash '\\\\'
        not status test-feature regex-easyesc && set backslash $backslash$backslash
        set file (string replace -ra -- '([ ;#^<>&|()"\'])' "$backslash\$1" (commandline -xc)[-1])
        set prefix set
    end

    set -q file[1] || return

    # strip -option= from token if present
    set file (string replace -r -- '^-[^\s=]*=' '' $file | string collect)

    $prefix -l files $file || return # Bail if $file does not tokenize.

    if set -q files[1] && test -f $files[1]
        if $direct
            for file in $files
                __fish_echo __fish_preview_current_file_text $file
                or return
            end
        else
            __fish_echo $pager $files
        end
    else if builtin -q __fish_native_fs
        echo "fish: no regular file to preview at the cursor" >&2
        return 1
    end
end
