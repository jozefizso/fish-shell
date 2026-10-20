# localization: skip(private)
function __fish_print_svn_rev --description 'Print svn revisions'
    command -sq svnversion; or return 1
    printf '%s' (command svnversion | string replace -ra '[^0-9:]' '')
end
