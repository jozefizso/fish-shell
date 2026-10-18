# To know if the user is root. Used by several prompts to display something
# else if the user is root.

function fish_is_root_user --description "Check if the user is root"
    # Windows has no POSIX uid 0. An elevated administrator token is a different
    # security concept, not Unix root; never infer it from USER or inherited EUID.
    if builtin -q __fish_native_fs
        return 1
    end

    if test "$EUID" = 0
        return 0
    end

    if contains -- $USER root toor Administrator
        return 0
    end

    return 1
end
