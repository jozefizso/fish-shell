# localization: tier1

# This is a neat function, stolen from zsh. It allows you to edit the
# value of a variable interactively.
#

function vared --description "Edit variable value"
    if test (count $argv) = 1
        switch $argv

            case -h --h --he --hel --help
                __fish_print_help vared
                return 0

            case '-*'
                printf (_ "%s: Unknown option %s\n") vared $argv >&2
                return 1

            case '*'
                # Indirection selects the variable, not a subscript embedded in
                # its name. Resolve a single element before checking cardinality.
                set -l __fish_vared_ref (string match -r -- '^(.+)\[(-?[0-9]+)\]$' "$argv")
                set -l __fish_vared_temp_value $$argv
                if set -q __fish_vared_ref[3]
                    set __fish_vared_temp_value $__fish_vared_ref[2]
                    set __fish_vared_temp_value $$__fish_vared_temp_value
                    set __fish_vared_ref $__fish_vared_ref[3]
                    set __fish_vared_temp_value $__fish_vared_temp_value[$__fish_vared_ref]
                end
                if test (count $__fish_vared_temp_value) -lt 2
                    # Read into private local storage. Only the requested
                    # variable's final assignment chooses or preserves scope.
                    # Decide scope before read: a failed existence query after
                    # read would make set preserve that failure on success.
                    set -l __fish_vared_scope
                    if not set -q "$argv"
                        set __fish_vared_scope --global
                    end
                    read -p 'set_color green; echo '$argv'; set_color --reset; echo "> "' \
                        -c "$__fish_vared_temp_value" \
                        __fish_vared_temp_value
                    or return
                    set $__fish_vared_scope -- "$argv" "$__fish_vared_temp_value"
                else
                    printf (_ '%s: %s is an array variable. Use %svared%s %s[n]%s to edit the n:th element of %s\n') vared $argv (set_color $fish_color_command; echo) (set_color $fish_color_normal; echo) $argv (set_color --reset; echo) $argv >&2
                    return 1
                end
        end
    else
        printf (_ '%s: Expected exactly one argument, got %s.\n\nSynopsis:\n\t%svared%s VARIABLE\n') vared (count $argv) (set_color $fish_color_command; echo) (set_color $fish_color_normal; echo) >&2
        return 1
    end
end
