# Interpreter switches stop at the first script, module or command payload.
function __fish_python_no_arg
    set -l tokens (commandline -pxc)
    set -e tokens[1]
    while set -q tokens[1]
        switch $tokens[1]
            case -X -W --check-hash-based-pycs
                set -e tokens[1..2]
            case -c -m
                not set -q tokens[2]
                return $status
            case - -- '-c?*' '-m?*'
                return 1
            case '-*'
                set -e tokens[1]
            case '*'
                return 1
        end
    end
    return 0
end

complete -c python -n __fish_python_no_arg -s B -d 'Don\'t write .py[co] files on import'
complete -c python -n __fish_python_no_arg -s c -x -d "Execute argument as command"
complete -c python -n __fish_python_no_arg -l check-hash-based-pycs -a "default always never" -d "Control validation behaviour of pyc files"
complete -c python -n __fish_python_no_arg -s d -d "Debug on"
complete -c python -n __fish_python_no_arg -s E -d "Ignore all PYTHON* env vars"
complete -c python -n __fish_python_no_arg -s h -s '?' -l help -d "Display help and exit"
complete -c python -n __fish_python_no_arg -s i -d "Interactive mode after executing commands"
complete -c python -n __fish_python_no_arg -s m -d 'Run library module as a script (terminates option list)' -xa '(command -sq python; and command python -c "import pkgutil; print(\'\n\'.join([p[1] for p in pkgutil.iter_modules()]))" | string trim -r -c \r)'
complete -c python -n __fish_python_no_arg -s O -d "Enable optimizations"
complete -c python -n __fish_python_no_arg -o OO -d "Remove doc-strings in addition to the -O optimizations"
complete -c python -n __fish_python_no_arg -s s -d 'Don\'t add user site directory to sys.path'
complete -c python -n __fish_python_no_arg -s S -d "Disable import of site module"
complete -c python -n __fish_python_no_arg -s u -d "Unbuffered input and output"
complete -c python -n __fish_python_no_arg -s v -d "Verbose mode"
complete -c python -n __fish_python_no_arg -o vv -d "Even more verbose mode"
complete -c python -n __fish_python_no_arg -s V -l version -d "Display version and exit"
complete -c python -n __fish_python_no_arg -s W -x -d "Warning control" -a "ignore default all module once error"
complete -c python -n __fish_python_no_arg -s x -d 'Skip first line of source, allowing use of non-Unix forms of #!cmd'
complete -c python -n __fish_python_no_arg -f -k -a "(__fish_complete_suffix .py)"
complete -c python -n __fish_python_no_arg -f -a - -d 'Read program from stdin'

# Version-specific completions
# We have to detect this at runtime because pyenv etc can change
# what `python` refers to.
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s2"' -s 3 -d 'Warn about Python 3.x incompatibilities that 2to3 cannot trivially fix'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s2"' -s t -d "Warn on mixed tabs and spaces"
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s2"' -s Q -x -a "old new warn warnall" -d "Division control"
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -s q -d 'Don\'t print version and copyright messages on interactive startup'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -s X -x -d 'Set implementation-specific option'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -s b -d 'Warn when comparing bytes with str or int'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -o bb -d 'Error when comparing bytes with str or int'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -s R -d 'Turn on hash randomization'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -s I -d 'Run in isolated mode'
complete -c python -n 'command -sq python; and command python -V 2>&1 | string match -rq "^.*\s3" && __fish_python_no_arg' -o VV -d 'Print further version info'
