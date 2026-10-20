# localization: skip(private)
function __fish_no_arguments -d "Internal fish function"
    set -l cmd (commandline -pxc)
    set -l token (commandline -tc)
    # An empty current token after whitespace is not an argument. A quoted
    # empty argument still has its quotes here and must reject the condition.
    if test -n "$token"
        set -a cmd $token
    end
    set -e cmd[1]
    for i in $cmd
        switch $i
            case '-*'

            case '*'
                return 1
        end
    end
    return 0
end
