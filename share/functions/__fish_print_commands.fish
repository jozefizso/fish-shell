# localization: skip(private)
function __fish_print_commands --description "Print a list of documented fish commands"
    # fish-* manuals are guides; command manuals use their actual command names.
    __fish_man1_pages |
        string match -rv '^fish-'
end
