# localization: tier1
# This function is typically bound to Alt-W, it is used to list man page entries
# for the command under the cursor.
function __fish_whatis_current_token -d "Show man page entries or function description related to the token under the cursor"
    set -l token (commandline -pt)

    test -n "$token"
    or return

    set -l desc "$token: nothing appropriate."

    set -l tokentype (type --type $token 2>/dev/null)

    switch "$tokentype"
        case function
            set -l funcinfo (functions $token --details --verbose)

            test $funcinfo[5] != n/a
            and set desc "$token - $funcinfo[5]"

        case builtin
            # Since man pages can be localized, we can't look for "NAME".
            if builtin -q __fish_native_fs
                set -l help (__fish_print_help $token)
                or return
                set -l summary (string match -r -- '^[^ \t]+ - .+' $help)[1]
                set -q summary[1]; or return 1
                set desc (string trim -- $summary)
            else
                set desc (__fish_print_help $token | sed -n 4p | string trim)
            end

        case file
            if builtin -q __fish_native_fs
                set -l query (string escape --style=regex -- $token)
                set -l tmpdesc (__fish_apropos "^$query\$")
                or return $status
                set desc $tmpdesc
            else
                set -l tmpdesc (whatis $token 2>/dev/null)
                and set desc $tmpdesc
            end
    end

    __fish_echo printf '%s\n' $desc
end
