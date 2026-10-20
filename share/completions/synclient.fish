set -l __fish_synclient_keys
if command -sq synclient
    set __fish_synclient_keys (command synclient | string replace -r '\s*(\w+).*' '$1')
end

complete -c synclient -s l -d "List current user settings"
complete -c synclient -s '?' -d "Show help"
complete -c synclient -s V -d "Show version"
complete -c synclient -xa "$__fish_synclient_keys"
