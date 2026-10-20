# localization: skip(private)
function __fish_print_windows_drives --description 'Print Windows drives'
    if builtin -q __fish_native_services
        builtin __fish_native_services drives
        return $status
    end
    wmic logicaldisk get name | tail +2
end
