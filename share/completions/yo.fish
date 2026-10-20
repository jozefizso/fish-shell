complete -c yo -s h -l help -d 'Show help'
complete -c yo -s v -l version -d 'Show version'

complete -c yo -s f -l force -d 'Allow overwrite files that already exist'
complete -c yo -l no-color -d 'Disable colors'
complete -c yo -l generators -d 'List available generators'
complete -c yo -l local-only -n '__fish_seen_subcommand_from --generators' -d 'Disable lookup of globally-installed generators'

function __fish_yo_generators
    command -sq yo
    or return 127
    set -l generators (command yo --generators)
    set -l producer_status $status
    if set -q generators[4]
        printf '%s\n' $generators[3..-2] | string replace --regex '^\s+' ''
    end
    return $producer_status
end

complete -c yo -a '(__fish_yo_generators)'
