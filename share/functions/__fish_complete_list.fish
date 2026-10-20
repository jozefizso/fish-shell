# localization: skip(private)
function __fish_complete_list --argument-names div cmd prefix iprefix
    if not set -q cmd[1]
        echo "Usage:
    __fish_complete_list <separator> <function> <prefix> <itemprefix>
where:
  separator - a symbol, separating individual entries
  function - a function which prints a completion list to complete each entry
  prefix - a prefix, which is printed before the list
  itemprefix - a prefix, which is printed before each item" >/dev/stderr
        return 1
    end
    set -q iprefix[1]
    or set -l iprefix ""
    set -q prefix[1]
    or set -l prefix ""
    set -l pat "$(commandline -t)"
    if set -q __fish_stripprefix[1]
        set pat "$(string replace -r -- "$__fish_stripprefix" "" $pat)"
    end
    switch $pat
        case "*$div*"
            set -l div_regex (string escape --style=regex -- $div)
            set -l iprefix_regex (string escape --style=regex -- $iprefix)
            set -l head (string unescape -- $pat | string replace -r -- "^(.*$div_regex)$iprefix_regex.*\$" '$1')
            for i in $head$iprefix(eval $cmd)
                printf %s\n $i
            end
        case '*'
            for i in $prefix$iprefix(eval $cmd)
                printf %s\n $i
            end
    end

end
