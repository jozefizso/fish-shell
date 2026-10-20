complete -c trap -s l -l list-signals -d 'Display names of all signals'
complete -c trap -s p -l print -d 'Display all currently defined trap handlers'
complete -c trap -s h -l help -d 'Display help and exit'
if not builtin -q __fish_native_services
    complete -c trap -a '(trap -l | string split -n " ")' -d Signal
end
