# localization: skip(private)
function __fish_cursor_xterm -d 'Set cursor (xterm)'
    # Cursor-shape controls are meaningful only on a capable terminal stream.
    # In particular, a redirected binding/helper must not contaminate its data.
    isatty stdout; and test "$TERM" != dumb
    or return 1
    set -l shape $argv[1]

    switch "$shape"
        case block
            set shape 2
        case underscore
            set shape 4
        case line
            set shape 6
        case '*'
            # Unknown shape
            return
    end
    if contains blink $argv
        set shape (math $shape - 1)
    end
    echo -en "\e[$shape q"
end
