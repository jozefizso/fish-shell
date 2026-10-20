# localization: skip(private)
#
# This function is used internally by the fish command completion code
#

# Perform this check once at startup rather than on each invocation
if not builtin -q __fish_native_fs; and not type -q apropos
    function __fish_describe_command
    end
    exit
end

function __fish_describe_command -d "Command used to find descriptions for commands"
    argparse exact -- $argv
    or return 1
    set -l suffix
    set -q _flag_exact && set suffix '$'

    if builtin -q __fish_native_fs
        set -l query (string escape --style=regex -- "$argv")
        __fish_apropos "^$query$suffix" 2>/dev/null | while read -l line
            set -l fields (string match -rg -- '^(.+) \(([18])\) - (.+)$' $line)
            set -q fields[3]; or continue
            printf '%s\t%s\n' $fields[1] $fields[3]
        end
        return $pipestatus[1]
    end
    # $argv will be inserted directly into the awk regex, so it must be escaped
    set -l argv_regex (string escape --style=regex -- "$argv")
    __fish_apropos "^$argv$suffix" 2>/dev/null | awk -v FS=" +- +" '{
        split($1, names, ", ");
        for (name in names)
            if (names[name] ~ /^'"$argv_regex"'.* *\([18]\)/ ) {
                sub( "( |\t)*\\\([18]\\\)", "", names[name] );
                sub( " \\\[.*\\\]", "", names[name] );
                print names[name] "\t" $2;
            }
    }'
end
