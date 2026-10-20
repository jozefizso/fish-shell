# localization: skip(private)
function __fish_print_lpr_printers --description 'Print lpr printers'
    command -sq lpstat; or return 1
    command lpstat -p 2>/dev/null | string replace -r '^\S+\s+(\S+)\s+(.*)$' '$1\t$2'

end
