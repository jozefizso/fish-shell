# localization: tier3
function __fish_cmd__complete_args
    set -l current_token (commandline -tc)
    set -l prefix (string sub -l 3 -- $current_token)

    switch $current_token
        case '/t:*'
            echo -e '0\tBlack
1\tBlue
2\tGreen
3\tAqua
4\tRed
5\tPurple
6\tYellow
7\tWhite
8\tGray
9\tLight blue
A\tLight green
B\tLight aqua
C\tLight red
D\tLight purple
E\tLight yellow
F\tBright white' | string replace -r '^' "$prefix"
        case '/e:*'
            echo -e 'on\tEnable command extensions
off\tDisable command extensions' | string replace -r '^' "$prefix"
        case '/f:*'
            echo -e 'on\tEnable file and directory name completion
off\tDisable file and directory name completion' | string replace -r '^' "$prefix"
        case '/v:*'
            echo -e 'on\tEnable delayed environment variable expansion
off\tDisable delayed environment variable expansion' | string replace -r '^' "$prefix"
    end
end
