# localization: skip(private)
function __fish_print_encodings -d "Complete using available character encodings"
    command -sq iconv; or return 127
    if iconv --usage &>/dev/null # only GNU has this flag
        # Note: GNU iconv changes its output to contain the forward slashes below when stdout is not a tty.
        iconv --list | string replace -ra // ''
    else # non-GNU
        iconv --list | string replace -ra '\s+' '\n'
    end
end
