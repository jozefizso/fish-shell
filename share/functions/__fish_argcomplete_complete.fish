# localization: skip(private)
function __fish_argcomplete_complete
    if builtin -q __fish_native_adapter
        # The native contract deliberately requires a cooperating Python .exe
        # and .py application as separate literal argv, not arbitrary fd 8/9
        # redirections on an unrelated executable or a shell command string.
        # NUL separates the two newline-terminated commandline fields without
        # command-substitution splitting or trimming meaningful trailing LFs.
        begin
            commandline -p
            printf '\0'
            commandline -pc
        end | __fish_native_adapter argcomplete-current -- $argv
        return $status
    end
    set -lx _ARGCOMPLETE 1
    set -lx _ARGCOMPLETE_IFS \n
    set -lx _ARGCOMPLETE_SUPPRESS_SPACE 1
    set -lx _ARGCOMPLETE_SHELL fish
    set -lx COMP_LINE (commandline -pc)
    set -lx COMP_POINT (string length (commandline -cp))
    set -lx COMP_TYPE
    $argv 8>&1 9>&2 2>/dev/null
end
