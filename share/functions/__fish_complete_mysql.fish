# localization: tier3
function __fish_mysql_query -a query
    command -sq mysql
    or return 127
    argparse -u 'u/user=' 'P/port=' 'h/host=' 'p/password=?' 'S/socket=' no-defaults no-login-paths protocol= connect-timeout= -- (commandline -px)
    set -l mysql_cmd mysql
    # MySQL requires these configuration controls before other client options.
    if set -q _flag_no_defaults
        set -a mysql_cmd --no-defaults
    end
    if set -q _flag_no_login_paths
        set -a mysql_cmd --no-login-paths
    end
    for flag in protocol connect-timeout
        set -l flagvar _flag_(string replace -a -- - _ $flag)
        if set -q $flagvar
            set -a mysql_cmd --$flag=$$flagvar
        end
    end
    for flag in u P h S
        if set -q _flag_$flag
            set -l flagvar _flag_$flag
            set -a mysql_cmd -$flag $$flagvar
        end
    end
    if test -n "$_flag_p"
        set -a mysql_cmd -p$_flag_p
    end
    # Metadata is consumed as UTF-8 by fish, not the client's Windows code page.
    printf '%s\n' $query | $mysql_cmd --default-character-set=utf8mb4 --batch --raw --skip-column-names 2>/dev/null | string replace -r '\r$' ''
    return $pipestatus[2]
end

function __fish_complete_mysql_databases
    __fish_mysql_query 'show databases'
end

function __fish_complete_mysql
    set -l command $argv[1]

    complete -c $command -s D -l database -x -d 'The database to use' -a '(__fish_complete_mysql_databases)'
    complete -c $command -a '(__fish_complete_mysql_databases)'
end
