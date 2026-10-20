# fish completion for mycli                              -*- shell-script -*-

function __fish_mycli_dsn
    command -sq mycli
    or return 127
    command mycli --list-dsn
end

complete -c mycli -s d -l dsn -x -d 'Use DSN configured into the [alias_dsn]' -a '(__fish_mycli_dsn)'
complete -c mycli -a '(__fish_mycli_dsn)'
