# localization: skip(private)
function __fish_print_xdg_mimetypes --description 'Print XDG mime types'
    set -l files (__fish_print_xdg_applications_directories)/mimeinfo.cache
    # No configured application directories means no MIME inventory.
    set -q files[1] || return 1
    set -l result 1
    for file in $files
        test -r $file; or continue
        string replace -rf '^([^#=\r\n]+)=(.*)$' '$1\t$2' <$file
        test $status -eq 0; and set result 0
    end
    return $result
end
