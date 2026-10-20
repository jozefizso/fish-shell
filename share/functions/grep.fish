# localization: skip(uses-apropos)
#
# Match colors for grep, if supported
#

if command -sq grep; and echo | command grep --color=auto "" >/dev/null 2>&1
    function grep
        command grep --color=auto $argv
    end
end
