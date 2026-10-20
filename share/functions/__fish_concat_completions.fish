# localization: skip(private)
#
# This function is intended to be used to generate completions in `-a "(...)"` calls for arguments that take 1..n values
# separated by commas and we dynamically or statically generated a list of n values that must be combined and permuted
# in order to appease both the `complete` machinery and the target application.
#
function __fish_concat_completions -d "Generate completions that are specified as comma-separated values from stdin source"
    set -l token (commandline -ct)
    # We want to filter out suggestions the user has already entered.
    set -l extant (string split -- ',' $token)
    set -l prefix (string replace -rf -- '^(.*),[^,]*$' '$1,' $token; or printf '')
    while read -l line
        set -l fields (string split -m 1 \t -- $line)
        set -l candidate $fields[1]
        test -n "$candidate"; or continue
        contains -- $candidate $extant; and continue
        if set -q fields[2]
            printf '%s%s\t%s\n' "$prefix" $candidate $fields[2]
        else
            printf '%s%s\n' "$prefix" $candidate
        end
    end
end
