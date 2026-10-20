if __fish_is_windows_command sort
    set -l windows_sort -c sort -n 'not __fish_prev_arg_in /m /l /rec /t /o'
    complete $windows_sort -a /r -d 'Reverse sort order'
    complete $windows_sort -a /+ -d 'Start comparison at the specified character position'
    complete $windows_sort -a /m -d 'Set memory limit in kilobytes'
    complete $windows_sort -a /l -d 'Override sort locale'
    complete -c sort -n '__fish_prev_arg_in /l' -xa C -d 'Binary character order'
    complete $windows_sort -a /rec -d 'Set maximum record length'
    complete -c sort -n '__fish_prev_arg_in /m /rec' -f
    complete $windows_sort -a /t -d 'Set temporary directory'
    complete -c sort -n '__fish_prev_arg_in /t' -xa '(__fish_complete_directories)'
    complete $windows_sort -a /o -d 'Write output to file'
    complete $windows_sort -a /unique -d 'Return unique lines'
    complete $windows_sort -a '/?' -d 'Show help'
    return
end

complete -c sort -s b -l ignore-leading-blanks -d "Ignore leading blanks"
complete -c sort -s d -l dictionary-order -d "Consider only blanks and alphanumerics"
complete -c sort -s f -l ignore-case -d "Ignore case"
complete -c sort -s g -l general-numeric-sort -d "Compare general numeric value"
complete -c sort -s i -l ignore-nonprinting -d "Consider only printable"
complete -c sort -s h -l human-numeric-sort -d "Compare human readable numbers [2K 1G]"
complete -c sort -s M -l month-sort -d "Compare month names"
complete -c sort -s n -l numeric-sort -d "Compare string numerical value"
complete -c sort -s R -l random-sort -d "Sort by random hash of keys"
complete -c sort -l random-source -d "Get random bytes from FILE"
complete -c sort -s r -l reverse -d "Reverse results"
complete -c sort -s c -l check -d "Only check if sorted"
complete -c sort -s k -l key -d "Define key"
complete -c sort -s m -l merge -d "Merge sorted files"
complete -c sort -s o -l output -f -d "Write to file"
complete -c sort -s s -l stable -d "Stabilize sort"
complete -c sort -s S -l buffer-size -r -d "Set memory buffer size"
complete -c sort -s t -l field-separator -d "Field separator"
complete -c sort -s T -l temporary-directory -r -d "Set temporary directory"
complete -c sort -s u -l unique -d "Output only first of equal lines"
complete -c sort -s V -l version-sort -d "natural sort of (version) numbers within text"
complete -c sort -s z -l zero-terminated -d "Lines end with 0 byte"
complete -c sort -l help -d "Display help and exit"
complete -c sort -l version -d "Display version and exit"
