# localization: skip(private)
function __fish_print_windows_users --description 'Print Windows user names'
    if builtin -q __fish_native_services
        builtin __fish_native_services users | string replace -r '^([^\t]*)\t.*' '$1'
        return $pipestatus[1]
    end
    # Cygwin/MSYS retain their external Windows account provider.
    wmic useraccount get name | tail +2
end
