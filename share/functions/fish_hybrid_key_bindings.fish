function fish_hybrid_key_bindings --description "Vi-style bindings that inherit emacs-style bindings in all modes"
    bind --erase --all --preset # clear earlier bindings, if any

    if test "$fish_key_bindings" != fish_hybrid_key_bindings
        __fish_change_key_bindings fish_hybrid_key_bindings
    end

    for mode in default insert visual
        fish_default_key_bindings -M $mode
    end
    fish_vi_key_bindings --no-erase
end
