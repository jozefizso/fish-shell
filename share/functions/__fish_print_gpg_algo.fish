# localization: skip(private)
#
# Helper function for contextual autocompletion of GPG crypto algorithm options

function __fish_print_gpg_algo -d "Complete using all algorithms of the type specified in argv[2] supported by gpg. argv[2] is a regexp" -a __fish_complete_gpg_command
    # Set a known locale, so that the output format of 'gpg --version'
    # is at least somewhat predictable. The locale will automatically
    # expire when the function goes out of scope, and the original locale
    # will take effect again.
    set -lx LC_ALL C

    set -l lines ($__fish_complete_gpg_command --version)
    or return $status
    set -l collecting false
    set -l algorithms
    for line in $lines
        if not $collecting
            string match -qr -- "^$argv[2]:" $line; or continue
            set collecting true
            set line (string replace -r '^[^:]*:' '' -- $line)
        end
        set -a algorithms (string split -n , -- $line | string replace -ar '\s+' '')
        string match -q '*,' -- (string trim -- $line); or break
    end
    set -q algorithms[1]; or return 1
    printf '%s\n' $algorithms
end
