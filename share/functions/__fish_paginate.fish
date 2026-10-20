# localization: skip(private)
function __fish_paginate -d "Paginate the current command using the users default pager"
    set -l cmd (__fish_anypager)
    or return 1

    # The pager is an argv list, not shell source. Preserve quoted executable
    # paths and literal pager arguments when constructing the editable pipeline.
    set -l pager (string join ' ' -- (string escape -- $cmd))
    set -l pipe " &| $pager"
    if string match -rq -- ' \n\.$' "$(commandline -j; echo .)"
        set pipe "&| $pager"
    end
    fish_commandline_append $pipe
end
