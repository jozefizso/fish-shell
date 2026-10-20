# localization: skip(uses-apropos)
# Use colours in diff output, if supported
set -l probe /dev/null
if builtin -q __fish_native_fs
    set probe (status fish-path)
end
if command -vq diff; and command diff --color=auto $probe $probe >/dev/null 2>&1
    function diff
        command diff --color=auto $argv
    end
end
