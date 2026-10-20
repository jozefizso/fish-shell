# localization: skip(private)
function __fish_unexpand_tilde --description 'Replace $HOME with "~"'
    if builtin -q __fish_native_fs
        # HOME and a native path may use either separator spelling. Normalize
        # before matching, and keep a component boundary so HOME-other is not ~.
        set -l realhome (string replace -a '\\' / -- ~ | string escape --style=regex)
        string replace -ri -- "^$realhome(\$|/)" '~$1' (string replace -a '\\' / -- $argv)
    else
        set -l realhome (string escape --style=regex -- ~)
        string replace -r -- "^$realhome(\$|/)" '~$1' $argv
    end
end
