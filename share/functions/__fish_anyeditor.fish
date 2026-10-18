# localization: tier1
function __fish_anyeditor
    set -l editor
    if set -q VISUAL
        echo $VISUAL | read -at editor
    else if set -q EDITOR
        echo $EDITOR | read -at editor
    else
        __fish_echo string join \n -- \
            (_ 'External editor requested but $VISUAL or $EDITOR not set.') \
            (_ 'Please set VISUAL or EDITOR to your preferred editor.')
        return 1
    end
    if builtin -q __fish_native_fs
        if not set -q editor[1]; or test -z "$editor[1]"
            echo "fish: EDITOR or VISUAL names no editor command" >&2
            return 1
        end
        if not type -q -- $editor[1]
            echo "fish: editor command is unavailable: $editor[1]" >&2
            return 127
        end
    end
    string join \n -- $editor
    return 0
end
