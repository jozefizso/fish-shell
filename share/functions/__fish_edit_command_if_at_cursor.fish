# localization: tier1
function __fish_edit_command_if_at_cursor --description 'If cursor is at the command token, edit the command source file'
    set -l tokens (commandline -xpc)
    set -l command
    if not set -q tokens[1]
        set command (commandline -xp)[1]
    else if test (count $tokens) = 1 && test -z "$(commandline -t)"
        set command $tokens[1]
    end

    set -q command[1]
    or return 1
    set -l command_path (command -v -- $command)
    or return 1
    test -r $command_path
    or return 1
    if builtin -q __fish_native_fs
        __fish_native_fs is-text $command_path
        or return 1
    else
        string match -q 'text/*' (file --brief --mime-type -L -- $command_path)
        or return 1
    end

    set -l editor (__fish_anyeditor)
    or begin
        # 1 means "not a source-editor action"; 2 means the requested action
        # failed and the caller must not turn it into a successful preview.
        if builtin -q __fish_native_fs
            return 2
        end
        return 0 # Unix callers historically finish after the warning.
    end
    $editor $command_path
    set -l editor_status $status
    if builtin -q __fish_native_fs; and test $editor_status -ne 0
        return 2
    end
    return 0
end
