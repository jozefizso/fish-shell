# localization: skip(private)
function __fish_print_lpr_options --description 'Print lpr options'
    command -sq lpoptions; or return 1
    command lpoptions -l 2>/dev/null | string replace -r '^(.*)/(.*):.*$' '$1\t$2'

end
