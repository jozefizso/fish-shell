function __attrib_complete_args -d 'Function to generate args'
    set -l current_token (commandline -tc)

    set -l action
    switch $current_token
        case +
            set action Set
        case -
            set action Clear
        case '*'
            return
    end
    for attribute in 'r:Read-only' 'a:Archive' 's:System' 'h:Hidden' 'o:Offline' 'i:Not Content Indexed' 'x:Scrub' 'p:Pinned' 'u:Unpinned' 'b:SMR Blob'
        set -l fields (string split -m 1 : -- $attribute)
        printf '%s%s\t%s the %s file attribute\n' $current_token $fields[1] $action $fields[2]
    end
end

complete -c attrib -a '(__attrib_complete_args)'

complete -c attrib -a /d -d 'Apply to directories'
complete -c attrib -a /l -d 'Apply to the symbolic link rather than its target'

complete -c attrib -a + -d 'Set the file attribute'
complete -c attrib -a - -d 'Clear the file attribute'
complete -c attrib -a /s \
    -d 'Apply to matching files in the current directory and all of its subdirectories'
complete -c attrib -a '/?' -d 'Show help'
