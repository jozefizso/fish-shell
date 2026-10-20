function __fish_command_is_wrapper
    # Options on command itself select builtin queries, not a child's grammar.
    for token in (commandline -xpc)[2..]
        switch $token
            case --
                return 0
            case '-*'
                return 1
            case '*=*'
                continue
            case '*'
                return 0
        end
    end
    return 1
end

complete -c command -n 'test (count (commandline -xpc)) -eq 1' -s h -l help -d 'Display help and exit'
complete -c command -n 'test (count (commandline -xpc)) -eq 1' -s a -l all -d 'Print all external commands by the given name'
complete -c command -n 'test (count (commandline -xpc)) -eq 1' -s q -l quiet -l query -d 'Do not print anything, only set exit status'
complete -c command -n 'test (count (commandline -xpc)) -eq 1' -s s -s v -l search -d 'Print the file that would be executed'
complete -c command -n 'not string match -q -- "-*" (commandline -ct); or __fish_command_is_wrapper' -xa "(__fish_complete_subcommand)"
