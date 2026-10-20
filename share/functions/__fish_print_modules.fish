# localization: skip(private)
# Helper function for completions that need to enumerate Linux modules
function __fish_print_modules
    command -sq uname; and command -sq find; and command -sq sed; or return 127
    test (command uname -s) = Linux; or return
    find /lib/modules/(uname -r)/{kernel,misc,updates} -type f 2>/dev/null | sed -e 's$/.*/\([^/.]*\).*$\1$'
end
