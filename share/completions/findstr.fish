function __findstr_generate_args --description 'Function to generate args'
    set --local current_token (commandline --current-token --cut-at-cursor)
    switch $current_token
        case '/a:*' '/A:*'
            string match -rq '^/[aA]:[0-9A-Fa-f]?$' -- $current_token; or return
            for color in '0:Black' '1:Blue' '2:Green' '3:Aqua' '4:Red' '5:Purple' '6:Yellow' '7:White' '8:Gray' '9:Light blue' 'A:Light green' 'B:Light aqua' 'C:Light red' 'D:Light purple' 'E:Light yellow' 'F:Bright white'
                set -l fields (string split -m 1 : -- $color)
                printf '%s%s\t%s\n' $current_token $fields[1] $fields[2]
            end
        case '*'
            echo -e '/b\tMatch at the beginning of the line
/e\tMatch at the end of the line
/l\tUse literal strings
/r\tUse regular expressions
/s\tSearch a current directory and all subdirectories
/i\tIgnore case
/x\tMatch exactly
/v\tInvert match
/n\tPrint line numbers
/m\tPrint only the file name with matching lines
/o\tPrint character offset
/p\tSkip files with non-printable characters
/off\tDon\'t skip files that have the offline attribute set
/offline\tDon\'t skip files that have the offline attribute set
/f:\tRead a file list from file
/c:\tSpecify a literal search string
/g:\tRead literal search strings from file
/d:\tSpecify the search list of directories
/a:\tUse color attributes with two hexadecimal digits
/?\tShow help'
    end
end

complete --command findstr --arguments '(__findstr_generate_args)'
