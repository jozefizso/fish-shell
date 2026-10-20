# name: Informative
# http://michal.karzynski.pl/blog/2009/11/19/my-informative-shell-prompt/

function fish_prompt --description 'Informative prompt'
    #Save the return status of the previous command
    set -l last_pipestatus $pipestatus
    set -lx __fish_last_status $status # Export for __fish_print_pipestatus.

    if functions -q fish_is_root_user; and fish_is_root_user
        printf '%s@%s %s%s%s# ' $USER (prompt_hostname) (set -q fish_color_cwd_root
                                                         and set_color $fish_color_cwd_root
                                                         or set_color $fish_color_cwd) \
            (prompt_pwd) (set_color --reset)
    else
        set -l status_color (set_color $fish_color_status)
        set -l statusb_color (set_color --bold $fish_color_status)
        set -l pipestatus_string (__fish_print_pipestatus "[" "]" "|" "$status_color" "$statusb_color" $last_pipestatus)
        set -l clock
        if builtin -q __fish_native_services
            set clock (__fish_native_services timestamp now '%H:%M:%S')
        else
            set clock (date '+%H:%M:%S')
        end

        printf '[%s] %s%s@%s %s%s %s%s%s \n> ' $clock (set_color brblue) \
            $USER (prompt_hostname) (set_color $fish_color_cwd) (prompt_pwd) $pipestatus_string \
            (set_color --reset)
    end
end
